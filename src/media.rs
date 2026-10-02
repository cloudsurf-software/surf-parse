//! Video sources, the `media:` scheme and the host's media resolver (0.37.0).
//!
//! A `::video` block (and `::hero[video=]`, and a `::embed` that names a
//! video file) carries its source as the author wrote it:
//!
//! - `media:<file-id>` — a file in the workspace's library. The parser never
//!   turns this into a URL. The HOST does, through [`install_media_resolver`]:
//!   a site maps the id to a path on its own origin, a native app to a URL it
//!   can authorise. `media:<file-id>/poster` names that file's processed
//!   poster picture.
//! - a site-relative path (`/assets/intro.mp4`, `clips/a.webm`);
//! - an `https:` URL.
//!
//! Anything else (`http:`, `javascript:`, `data:`, a protocol-relative
//! `//host/…`) is not a usable source: the renderers draw the placeholder and
//! the lint reports it (L047).
//!
//! Everything the renderers must agree on lives here, once: what a source is
//! ([`classify_media_src`]), the effective playback flags ([`video_flags`]),
//! the `<source type>` ([`video_mime`]), the layout box ([`video_aspect`],
//! [`video_width`]) and the resolved player ([`video_player`]).
//!
//! # The resolver seam
//!
//! The renderers take a document and nothing else, so the host's knowledge
//! rides an ambient, thread-local context with an RAII guard — the same shape
//! as `render_typst::install_image_context`:
//!
//! ```
//! use surf_parse::media::{install_media_resolver, MediaUse};
//!
//! let _media = install_media_resolver(|id, media_use| match media_use {
//!     MediaUse::Video => Some(format!("/media/{id}")),
//!     MediaUse::VideoLoop => Some(format!("/media/{id}?r=loop")),
//!     MediaUse::Poster => Some(format!("/media/{id}/poster")),
//! });
//! let html = surf_parse::parse("::video[src=\"media:abc\"]\n::").doc.to_html_fragment();
//! assert!(html.contains("src=\"/media/abc\""));
//! ```
//!
//! With no resolver installed, or when the resolver answers `None`, a
//! `media:` source is UNRESOLVED: the block draws a poster-less placeholder
//! with its caption, never a `<video src="media:…">` a browser cannot load.

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::types::{Block, EmbedType, SurfDoc};

/// The scheme of a library file reference: `media:<file-id>`.
pub const MEDIA_SCHEME: &str = "media:";

/// The suffix that names a library video's processed poster picture:
/// `media:<file-id>/poster`.
pub const MEDIA_POSTER_SUFFIX: &str = "/poster";

/// Longest file id a `media:` reference may carry.
pub const MEDIA_ID_MAX_LEN: usize = 128;

/// The `<source media>` query that keeps an autoplaying video from loading
/// when the viewer asked for reduced motion (see `render_html::video_html`).
pub const MOTION_OK_MEDIA_QUERY: &str = "(prefers-reduced-motion: no-preference)";

/// What the host is asked to resolve a library file FOR. An autoplaying
/// video plays muted, so the host may answer [`MediaUse::VideoLoop`] with a
/// smaller, silent rendition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaUse {
    /// The full rendition, played on request with its sound.
    Video,
    /// The rendition an autoplaying (muted) video plays.
    VideoLoop,
    /// The poster picture.
    Poster,
}

/// One library file a document references, and what for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaRef {
    /// The file id — the text after `media:`.
    pub id: String,
    /// What the document uses it for.
    pub media_use: MediaUse,
}

/// What a `src` / `poster` value is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaSrc<'a> {
    /// `media:<id>` (`poster: false`) or `media:<id>/poster` (`poster: true`).
    Media { id: &'a str, poster: bool },
    /// An absolute `https:` URL.
    Https(&'a str),
    /// A site-relative path or relative URL (no scheme).
    Relative(&'a str),
    /// The value is empty.
    Missing,
    /// `media:` followed by something that is not a file id.
    MalformedMedia,
    /// Any other scheme, a protocol-relative URL, or a value with control
    /// characters — never emitted as a source.
    Forbidden,
}

impl MediaSrc<'_> {
    /// A source the renderers may emit once resolved.
    pub fn is_usable(&self) -> bool {
        matches!(self, MediaSrc::Media { .. } | MediaSrc::Https(_) | MediaSrc::Relative(_))
    }
}

fn valid_media_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MEDIA_ID_MAX_LEN
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Classify a `src` / `poster` value. Pure; never resolves anything.
pub fn classify_media_src(value: &str) -> MediaSrc<'_> {
    let v = value.trim();
    if v.is_empty() {
        return MediaSrc::Missing;
    }
    // A browser drops tabs and newlines inside a URL before reading its
    // scheme, so a value carrying any control character is refused outright.
    if v.chars().any(|c| c.is_control()) {
        return MediaSrc::Forbidden;
    }
    if let Some(rest) = strip_prefix_ignore_case(v, MEDIA_SCHEME) {
        let (id, poster) = match rest.strip_suffix(MEDIA_POSTER_SUFFIX) {
            Some(id) => (id, true),
            None => (rest, false),
        };
        return if valid_media_id(id) {
            MediaSrc::Media { id, poster }
        } else {
            MediaSrc::MalformedMedia
        };
    }
    if let Some(rest) = strip_prefix_ignore_case(v, "https://") {
        return if rest.is_empty() { MediaSrc::Forbidden } else { MediaSrc::Https(v) };
    }
    // Protocol-relative (`//host`), and the backslash spellings a browser
    // reads the same way.
    if v.starts_with("//") || v.starts_with('\\') || v.starts_with("/\\") {
        return MediaSrc::Forbidden;
    }
    if has_scheme(v) {
        return MediaSrc::Forbidden;
    }
    MediaSrc::Relative(v)
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

/// Does `v` open with a URL scheme (`name:` before any `/`, `?` or `#`)?
fn has_scheme(v: &str) -> bool {
    let Some(colon) = v.find(':') else { return false };
    let head = &v[..colon];
    if head.contains(['/', '?', '#']) {
        return false;
    }
    let mut chars = head.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// The `<source type>` for a video source: by file extension (a query string
/// or fragment is ignored), and `video/mp4` for a `media:` source — the
/// platform always serves a processed MP4. `None` when the extension is not
/// one of `.mp4 .m4v .webm .mov .ogv`.
pub fn video_mime(src: &str) -> Option<&'static str> {
    match classify_media_src(src) {
        MediaSrc::Media { poster: false, .. } => Some("video/mp4"),
        MediaSrc::Https(v) | MediaSrc::Relative(v) => {
            let path = v.split(['?', '#']).next().unwrap_or(v);
            let name = path.rsplit('/').next().unwrap_or(path);
            let (_, ext) = name.rsplit_once('.')?;
            match ext.to_ascii_lowercase().as_str() {
                "mp4" | "m4v" => Some("video/mp4"),
                "webm" => Some("video/webm"),
                "mov" => Some("video/quicktime"),
                "ogv" => Some("video/ogg"),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Is `src` a video FILE — a `media:` id, or a path / `https:` URL ending in
/// a video extension — rather than a provider's page? This is what sends a
/// `::embed[type=video]` to the player instead of the link card.
pub fn is_direct_video_src(src: &str) -> bool {
    match classify_media_src(src) {
        MediaSrc::Media { poster, .. } => !poster,
        MediaSrc::Https(_) | MediaSrc::Relative(_) => video_mime(src).is_some(),
        _ => false,
    }
}

/// Does this `::embed` draw as a video player (see [`is_direct_video_src`])?
pub fn embed_is_video_file(embed_type: Option<EmbedType>, src: &str) -> bool {
    embed_type == Some(EmbedType::Video) && is_direct_video_src(src)
}

/// The playback flags a renderer acts on, after the browser's rules are
/// applied to the authored ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoFlags {
    pub autoplay: bool,
    pub loops: bool,
    pub muted: bool,
    pub controls: bool,
    /// Always true: a video plays in its box, not full screen, on a phone.
    pub playsinline: bool,
}

/// The ONE place the flag rules live; every renderer calls it.
///
/// - `autoplay` forces `muted` and `playsinline` — a browser plays nothing
///   else unasked; there is no override.
/// - `controls` defaults ON, and OFF with `autoplay` unless the author
///   stated `controls` (`Some(true)`); a stated `controls=false` is kept.
pub fn video_flags(autoplay: bool, loops: bool, muted: bool, controls: Option<bool>) -> VideoFlags {
    VideoFlags {
        autoplay,
        loops,
        muted: muted || autoplay,
        controls: controls.unwrap_or(!autoplay),
        playsinline: true,
    }
}

fn plain_number(s: &str) -> bool {
    let digits = s.bytes().filter(u8::is_ascii_digit).count();
    !s.is_empty()
        && s.len() <= 8
        && digits > 0
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && s.bytes().filter(|b| *b == b'.').count() <= 1
        && s.parse::<f64>().is_ok_and(|n| n > 0.0)
}

/// `aspect=16/9` (or `16:9`) as a CSS `aspect-ratio` value (`16 / 9`).
/// `None` for anything that is not two positive numbers.
pub fn video_aspect(aspect: &str) -> Option<String> {
    let (w, h) = aspect.trim().split_once(['/', ':'])?;
    let (w, h) = (w.trim(), h.trim());
    (plain_number(w) && plain_number(h)).then(|| format!("{w} / {h}"))
}

/// `width=` as a CSS length: a number (pixels) or a number with one of
/// `px % rem em vw`. `None` for anything else, so nothing authored reaches a
/// `style` attribute unchecked.
pub fn video_width(width: &str) -> Option<String> {
    let w = width.trim();
    let split = w.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(w.len());
    let (num, unit) = w.split_at(split);
    if !plain_number(num) {
        return None;
    }
    match unit {
        "" => Some(format!("{num}px")),
        "px" | "%" | "rem" | "em" | "vw" => Some(format!("{num}{unit}")),
        _ => None,
    }
}

// ───────────────────────────────────────────────────────────────────────────
// The resolver seam
// ───────────────────────────────────────────────────────────────────────────

type Resolver = Rc<dyn Fn(&str, MediaUse) -> Option<String>>;

thread_local! {
    static ACTIVE_RESOLVER: RefCell<Option<Resolver>> = const { RefCell::new(None) };
}

/// RAII guard of [`install_media_resolver`]: dropping it puts back whatever
/// resolver (or none) was installed before.
pub struct MediaScope {
    previous: Option<Resolver>,
}

impl Drop for MediaScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        ACTIVE_RESOLVER.with(|c| *c.borrow_mut() = previous);
    }
}

/// Install the host's media resolver for the current thread, for the
/// lifetime of the returned guard. The resolver is asked for a file id (the
/// text after `media:`) and a [`MediaUse`]; it answers the URL to emit, or
/// `None` when the host does not know the file (the block then draws its
/// placeholder). Every renderer consults it; the parser never does.
pub fn install_media_resolver<F>(resolver: F) -> MediaScope
where
    F: Fn(&str, MediaUse) -> Option<String> + 'static,
{
    let previous = ACTIVE_RESOLVER.with(|c| c.borrow_mut().replace(Rc::new(resolver)));
    MediaScope { previous }
}

/// A resolver built from three URL templates, each with `{id}` where the
/// file id goes — for hosts that cannot hand over a closure (the wasm
/// surface): `media_templates("/media/{id}", "/media/{id}?r=loop",
/// "/media/{id}/poster")`.
pub fn media_templates(
    video: &str,
    video_loop: &str,
    poster: &str,
) -> impl Fn(&str, MediaUse) -> Option<String> + 'static {
    let (video, video_loop, poster) = (video.to_string(), video_loop.to_string(), poster.to_string());
    move |id, media_use| {
        let template = match media_use {
            MediaUse::Video => &video,
            MediaUse::VideoLoop => &video_loop,
            MediaUse::Poster => &poster,
        };
        (!template.is_empty()).then(|| template.replace("{id}", id))
    }
}

/// Ask the ambient resolver for a library file's URL. `None` with no
/// resolver installed, when it does not know the file, or when it answers
/// an empty string.
pub fn resolve_media(id: &str, media_use: MediaUse) -> Option<String> {
    let resolver = ACTIVE_RESOLVER.with(|c| c.borrow().clone())?;
    resolver(id, media_use).filter(|url| !url.trim().is_empty())
}

/// The URL a renderer may emit for a `src` / `poster` value used as
/// `media_use`: a `media:` reference through the resolver, a path or
/// `https:` URL as written, and `None` for everything else.
pub fn media_url(value: &str, media_use: MediaUse) -> Option<String> {
    match classify_media_src(value) {
        MediaSrc::Media { id, poster } => {
            if poster != (media_use == MediaUse::Poster) {
                return None;
            }
            resolve_media(id, media_use)
        }
        MediaSrc::Https(v) | MediaSrc::Relative(v) => Some(v.to_string()),
        _ => None,
    }
}

/// The poster a video shows: the stated one, else — for a `media:` source —
/// that file's processed poster (`media:<id>/poster`), still unresolved.
pub fn effective_poster(src: &str, poster: Option<&str>) -> Option<String> {
    if let Some(p) = poster.map(str::trim).filter(|p| !p.is_empty()) {
        return Some(p.to_string());
    }
    match classify_media_src(src) {
        MediaSrc::Media { id, poster: false } => Some(format!("{MEDIA_SCHEME}{id}{MEDIA_POSTER_SUFFIX}")),
        _ => None,
    }
}

/// A video ready to draw: resolved URLs and effective flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoPlayer {
    /// The URL for `<source src>`.
    pub url: String,
    /// The `<source type>`, when known.
    pub mime: Option<&'static str>,
    /// The resolved poster URL, when there is one.
    pub poster: Option<String>,
    pub flags: VideoFlags,
}

/// Resolve a video for drawing. `None` when the source is unusable or an
/// unresolved `media:` id — the caller draws the placeholder.
pub fn video_player(
    src: &str,
    poster: Option<&str>,
    autoplay: bool,
    loops: bool,
    muted: bool,
    controls: Option<bool>,
) -> Option<VideoPlayer> {
    let flags = video_flags(autoplay, loops, muted, controls);
    let media_use = if flags.autoplay { MediaUse::VideoLoop } else { MediaUse::Video };
    let url = media_url(src, media_use)?;
    let poster = effective_poster(src, poster).and_then(|p| media_url(&p, MediaUse::Poster));
    Some(VideoPlayer { url, mime: video_mime(src), poster, flags })
}

/// The authored fields of one video, borrowed — what the HTML renderer and
/// its DOM twin draw from (a `::video`, or a `::embed` that names a file).
#[derive(Debug, Clone, Copy)]
pub(crate) struct VideoSpec<'a> {
    pub src: &'a str,
    pub poster: Option<&'a str>,
    pub autoplay: bool,
    pub loops: bool,
    pub muted: bool,
    pub controls: Option<bool>,
    pub caption: Option<&'a str>,
    pub alt: Option<&'a str>,
    pub width: Option<&'a str>,
    pub aspect: Option<&'a str>,
}

impl<'a> VideoSpec<'a> {
    /// A `::embed[type=video]` whose source is a video file: a player with
    /// its controls, the embed's title as the caption.
    pub(crate) fn from_embed(src: &'a str, title: Option<&'a str>, width: Option<&'a str>) -> Self {
        VideoSpec {
            src,
            poster: None,
            autoplay: false,
            loops: false,
            muted: false,
            controls: Some(true),
            caption: title,
            alt: None,
            width,
            aspect: None,
        }
    }

    pub(crate) fn player(&self) -> Option<VideoPlayer> {
        video_player(self.src, self.poster, self.autoplay, self.loops, self.muted, self.controls)
    }

    /// `style` value for the `<figure>` (`max-width:…`), when `width` is a
    /// checked CSS length.
    pub(crate) fn figure_style(&self) -> Option<String> {
        self.width.and_then(video_width).map(|w| format!("max-width:{w}"))
    }

    /// `style` value for the player box (`aspect-ratio:…`).
    pub(crate) fn box_style(&self) -> Option<String> {
        self.aspect.and_then(video_aspect).map(|a| format!("aspect-ratio:{a}"))
    }
}

/// Which `<video>` element of a block is being drawn (see
/// `render_html::video_html` for why an autoplaying video draws two).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VideoTwin {
    /// A video that does not autoplay: the one element.
    Plain,
    /// The autoplaying element — shown, and loaded, only when the viewer
    /// has not asked for reduced motion.
    Motion,
    /// Its reduced-motion stand-in: the poster with controls, never
    /// autoplaying, loading nothing until asked.
    Still,
}

impl VideoTwin {
    pub(crate) fn class(self) -> &'static str {
        match self {
            VideoTwin::Plain => "surfdoc-video-player",
            VideoTwin::Motion => "surfdoc-video-player surfdoc-video-motion",
            VideoTwin::Still => "surfdoc-video-player surfdoc-video-still",
        }
    }

    /// The boolean attributes this element carries, in emission order.
    pub(crate) fn bool_attrs(self, flags: &VideoFlags) -> Vec<&'static str> {
        let mut out = Vec::with_capacity(5);
        let (controls, autoplay) = match self {
            VideoTwin::Plain => (flags.controls, false),
            VideoTwin::Motion => (flags.controls, true),
            VideoTwin::Still => (true, false),
        };
        if controls {
            out.push("controls");
        }
        if autoplay {
            out.push("autoplay");
        }
        if flags.muted {
            out.push("muted");
        }
        if flags.loops {
            out.push("loop");
        }
        out.push("playsinline");
        out
    }

    /// `preload`: nothing is fetched before play when a poster stands in
    /// (and never for the hidden reduced-motion stand-in); an autoplaying
    /// element and a poster-less one fetch the metadata.
    pub(crate) fn preload(self, has_poster: bool) -> &'static str {
        match self {
            VideoTwin::Plain if has_poster => "none",
            VideoTwin::Still => "none",
            _ => "metadata",
        }
    }
}

/// A URL made safe inside a CSS `url('…')`: the characters that could close
/// the string or the function are percent-encoded.
pub(crate) fn css_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            '\'' => out.push_str("%27"),
            '"' => out.push_str("%22"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '\\' => out.push_str("%5C"),
            c if c.is_whitespace() || c.is_control() => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

// ───────────────────────────────────────────────────────────────────────────
// media_refs — what a document needs from the library
// ───────────────────────────────────────────────────────────────────────────

/// Every library file a document references, with its use, in
/// first-appearance order and without repeats: `::video` sources and
/// posters, `::hero[video= poster=]`, and `::embed` blocks that name a
/// `media:` video — through every container block. A video's default poster
/// (`media:<id>/poster` when none is stated) is listed, because the
/// renderers ask for it. A host's publish step records this set.
pub fn media_refs(doc: &SurfDoc) -> Vec<MediaRef> {
    media_refs_in(&doc.blocks)
}

/// [`media_refs`] over a block list.
pub fn media_refs_in(blocks: &[Block]) -> Vec<MediaRef> {
    let mut out = Vec::new();
    walk_refs(blocks, &mut out);
    out
}

fn push_ref(out: &mut Vec<MediaRef>, value: &str, media_use: MediaUse) {
    if let MediaSrc::Media { id, poster } = classify_media_src(value)
        && poster == (media_use == MediaUse::Poster)
    {
        let r = MediaRef { id: id.to_string(), media_use };
        if !out.contains(&r) {
            out.push(r);
        }
    }
}

fn push_video_refs(out: &mut Vec<MediaRef>, src: &str, poster: Option<&str>, autoplay: bool) {
    let media_use = if autoplay { MediaUse::VideoLoop } else { MediaUse::Video };
    push_ref(out, src, media_use);
    if let Some(p) = effective_poster(src, poster) {
        push_ref(out, &p, MediaUse::Poster);
    }
}

fn walk_refs(blocks: &[Block], out: &mut Vec<MediaRef>) {
    for b in blocks {
        match b {
            Block::Video { src, poster, autoplay, .. } => {
                push_video_refs(out, src, poster.as_deref(), *autoplay);
            }
            Block::Hero { video: Some(video), poster, image, .. } => {
                push_video_refs(out, video, hero_poster(poster.as_deref(), image.as_deref()), true);
            }
            Block::Embed { src, embed_type, .. } if embed_is_video_file(*embed_type, src) => {
                push_video_refs(out, src, None, false);
            }
            Block::Page { children, .. }
            | Block::Slide { children, .. }
            | Block::Section { children, .. }
            | Block::App { children, .. }
            | Block::AppShell { children, .. }
            | Block::PanelSlot { children, .. }
            | Block::Sidebar { children, .. }
            | Block::Panel { children, .. }
            | Block::TabContent { children, .. }
            | Block::Drawer { children, .. }
            | Block::Modal { children, .. }
            | Block::When { children, .. } => walk_refs(children, out),
            Block::SplitPane { left, right, .. } => {
                walk_refs(left, out);
                walk_refs(right, out);
            }
            Block::Flow { steps, .. } => {
                for step in steps {
                    walk_refs(&step.children, out);
                }
            }
            _ => {}
        }
    }
}

/// A hero's poster: the stated `poster=`, else its `image=` (which stays the
/// fallback picture as well).
pub fn hero_poster<'a>(poster: Option<&'a str>, image: Option<&'a str>) -> Option<&'a str> {
    poster
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .or(image.map(str::trim).filter(|i| !i.is_empty()))
}

/// A hero's background video, ready to draw: always autoplaying, muted,
/// looping, inline and without controls. `None` when there is no `video=`,
/// or it is unusable or unresolved — the hero then draws exactly as it does
/// without one (its `image` is the fallback).
pub fn hero_video_player(video: Option<&str>, poster: Option<&str>, image: Option<&str>) -> Option<VideoPlayer> {
    let video = video.map(str::trim).filter(|v| !v.is_empty())?;
    video_player(video, hero_poster(poster, image), true, true, true, Some(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_covers_every_source_shape() {
        assert_eq!(classify_media_src("media:3f6c-ab_1"), MediaSrc::Media { id: "3f6c-ab_1", poster: false });
        assert_eq!(classify_media_src("media:3f6c/poster"), MediaSrc::Media { id: "3f6c", poster: true });
        assert_eq!(classify_media_src("MEDIA:abc"), MediaSrc::Media { id: "abc", poster: false });
        assert_eq!(classify_media_src("https://cdn.example.com/a.mp4"), MediaSrc::Https("https://cdn.example.com/a.mp4"));
        assert_eq!(classify_media_src("/assets/a.mp4"), MediaSrc::Relative("/assets/a.mp4"));
        assert_eq!(classify_media_src("clips/a.webm?t=1:2"), MediaSrc::Relative("clips/a.webm?t=1:2"));
        assert_eq!(classify_media_src("  "), MediaSrc::Missing);
        assert_eq!(classify_media_src("media:"), MediaSrc::MalformedMedia);
        assert_eq!(classify_media_src("media:a b"), MediaSrc::MalformedMedia);
        assert_eq!(classify_media_src("media:a/b/poster"), MediaSrc::MalformedMedia);
        assert_eq!(classify_media_src(&format!("media:{}", "a".repeat(129))), MediaSrc::MalformedMedia);
        for bad in [
            "http://example.com/a.mp4",
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:video/mp4;base64,AAAA",
            "//evil.example/a.mp4",
            "\\\\evil.example\\a.mp4",
            "/\\evil.example/a.mp4",
            "java\tscript:alert(1)",
            "blob:https://x/y",
            "file:///etc/passwd",
            "https://",
        ] {
            assert_eq!(classify_media_src(bad), MediaSrc::Forbidden, "{bad}");
        }
    }

    #[test]
    fn flags_autoplay_forces_muted_and_inline_and_drops_default_controls() {
        let plain = video_flags(false, false, false, None);
        assert!(plain.controls && plain.playsinline && !plain.muted && !plain.autoplay && !plain.loops);
        let auto = video_flags(true, true, false, None);
        assert!(auto.autoplay && auto.muted && auto.playsinline && auto.loops && !auto.controls);
        // `controls` stated wins in both directions.
        assert!(video_flags(true, false, false, Some(true)).controls);
        assert!(!video_flags(false, false, false, Some(false)).controls);
        // There is no way to autoplay with sound.
        assert!(video_flags(true, false, false, Some(true)).muted);
        assert!(video_flags(false, false, true, None).muted);
    }

    #[test]
    fn mime_from_extension_and_media_is_mp4() {
        assert_eq!(video_mime("media:abc"), Some("video/mp4"));
        assert_eq!(video_mime("/a/b.MP4"), Some("video/mp4"));
        assert_eq!(video_mime("/a/b.m4v"), Some("video/mp4"));
        assert_eq!(video_mime("https://x.example/a.webm?token=1.2#t=3"), Some("video/webm"));
        assert_eq!(video_mime("/a.mov"), Some("video/quicktime"));
        assert_eq!(video_mime("/a.ogv"), Some("video/ogg"));
        assert_eq!(video_mime("/stream"), None);
        assert_eq!(video_mime("/a.mp4/page"), None);
        assert_eq!(video_mime("media:abc/poster"), None);
        assert_eq!(video_mime("javascript:a.mp4"), None);
    }

    #[test]
    fn direct_file_versus_provider_page() {
        assert!(is_direct_video_src("media:abc"));
        assert!(is_direct_video_src("/clips/a.mp4?v=2"));
        assert!(is_direct_video_src("https://cdn.example.com/a.webm"));
        assert!(!is_direct_video_src("https://youtube.com/watch?v=abc"));
        assert!(!is_direct_video_src("https://vimeo.com/12345"));
        assert!(!is_direct_video_src("media:abc/poster"));
        assert!(!is_direct_video_src("http://example.com/a.mp4"));
        assert!(embed_is_video_file(Some(EmbedType::Video), "/a.mp4"));
        assert!(!embed_is_video_file(Some(EmbedType::Generic), "/a.mp4"));
        assert!(!embed_is_video_file(Some(EmbedType::Video), "https://youtu.be/x"));
    }

    #[test]
    fn layout_values_are_checked_before_they_reach_a_style() {
        assert_eq!(video_aspect("16/9").as_deref(), Some("16 / 9"));
        assert_eq!(video_aspect(" 9 / 16 ").as_deref(), Some("9 / 16"));
        assert_eq!(video_aspect("1:1").as_deref(), Some("1 / 1"));
        assert_eq!(video_aspect("2.39/1").as_deref(), Some("2.39 / 1"));
        for bad in ["16", "0/9", "16/0", "a/b", "16/9;color:red", "1e9/1", "-1/1", "./."] {
            assert_eq!(video_aspect(bad), None, "{bad}");
        }
        assert_eq!(video_width("640").as_deref(), Some("640px"));
        assert_eq!(video_width("80%").as_deref(), Some("80%"));
        assert_eq!(video_width("32.5rem").as_deref(), Some("32.5rem"));
        for bad in ["", "wide", "100%;position:fixed", "calc(1px)", "0", "10pt"] {
            assert_eq!(video_width(bad), None, "{bad}");
        }
    }

    #[test]
    fn resolver_is_scoped_and_restores_the_previous_one() {
        assert_eq!(resolve_media("abc", MediaUse::Video), None, "no resolver, no URL");
        {
            let _outer = install_media_resolver(|id, _| Some(format!("/outer/{id}")));
            assert_eq!(resolve_media("abc", MediaUse::Video).as_deref(), Some("/outer/abc"));
            {
                let _inner = install_media_resolver(media_templates("/m/{id}", "/m/{id}?r=loop", "/m/{id}/poster"));
                assert_eq!(resolve_media("abc", MediaUse::Video).as_deref(), Some("/m/abc"));
                assert_eq!(resolve_media("abc", MediaUse::VideoLoop).as_deref(), Some("/m/abc?r=loop"));
                assert_eq!(resolve_media("abc", MediaUse::Poster).as_deref(), Some("/m/abc/poster"));
            }
            assert_eq!(resolve_media("abc", MediaUse::Video).as_deref(), Some("/outer/abc"));
        }
        assert_eq!(resolve_media("abc", MediaUse::Video), None, "the guard cleared it");
    }

    #[test]
    fn resolver_none_and_empty_answers_are_unresolved() {
        let _m = install_media_resolver(|id, _| match id {
            "known" => Some("/media/known".to_string()),
            "blank" => Some("  ".to_string()),
            _ => None,
        });
        assert!(resolve_media("known", MediaUse::Video).is_some());
        assert_eq!(resolve_media("blank", MediaUse::Video), None);
        assert_eq!(resolve_media("other", MediaUse::Video), None);
    }

    #[test]
    fn player_picks_the_loop_rendition_for_autoplay_and_the_default_poster() {
        let _m = install_media_resolver(media_templates("/m/{id}", "/m/{id}?r=loop", "/m/{id}/poster"));
        let plain = video_player("media:abc", None, false, false, false, None).expect("resolved");
        assert_eq!(plain.url, "/m/abc");
        assert_eq!(plain.mime, Some("video/mp4"));
        assert_eq!(plain.poster.as_deref(), Some("/m/abc/poster"));
        let auto = video_player("media:abc", None, true, true, false, None).expect("resolved");
        assert_eq!(auto.url, "/m/abc?r=loop");
        // A stated poster wins; a direct file has no default poster.
        let stated = video_player("media:abc", Some("/img/p.jpg"), false, false, false, None).unwrap();
        assert_eq!(stated.poster.as_deref(), Some("/img/p.jpg"));
        let file = video_player("/a.webm", None, false, false, false, None).unwrap();
        assert_eq!((file.url.as_str(), file.mime, file.poster), ("/a.webm", Some("video/webm"), None));
        // A poster that is not a poster reference is dropped, not emitted.
        let wrong = video_player("/a.mp4", Some("media:abc"), false, false, false, None).unwrap();
        assert_eq!(wrong.poster, None);
        assert!(video_player("http://x.example/a.mp4", None, false, false, false, None).is_none());
        assert!(video_player("media:abc/poster", None, false, false, false, None).is_none());
    }

    #[test]
    fn player_is_none_for_unresolved_media() {
        assert!(video_player("media:abc", None, false, false, false, None).is_none());
        assert!(video_player("/a.mp4", None, false, false, false, None).is_some());
    }

    #[test]
    fn media_refs_cover_video_hero_embed_and_containers() {
        let src = "\
::video[src=\"media:one\" caption=\"A\"]
::

::video[src=\"media:two\" poster=\"media:tp/poster\" autoplay]
::

::video[src=\"/local.mp4\" poster=\"media:three/poster\"]
::

::hero[video=\"media:bg\" image=\"/img/h.jpg\"]
# Headline
::

::embed[src=\"media:emb\" type=video]
::

::embed[src=\"https://youtube.com/watch?v=x\"]
::

::section
:::video[src=\"media:nested\"]
:::
:::video[src=\"media:one\"]
:::
::
";
        let refs = media_refs(&crate::parse(src).doc);
        let got: Vec<(&str, MediaUse)> = refs.iter().map(|r| (r.id.as_str(), r.media_use)).collect();
        assert_eq!(
            got,
            vec![
                ("one", MediaUse::Video),
                ("one", MediaUse::Poster),
                ("two", MediaUse::VideoLoop),
                ("tp", MediaUse::Poster),
                ("three", MediaUse::Poster),
                // The hero's image is its poster, so no library poster is asked for.
                ("bg", MediaUse::VideoLoop),
                ("emb", MediaUse::Video),
                ("emb", MediaUse::Poster),
                ("nested", MediaUse::Video),
                ("nested", MediaUse::Poster),
            ]
        );
    }

    #[test]
    fn media_refs_reach_pages_and_a_bare_hero_uses_the_library_poster() {
        let src = "::page[route=/ title=\"Home\"]\n:::hero[video=\"media:bg\"]\n# H\n:::\n::\n";
        let refs = media_refs(&crate::parse(src).doc);
        assert_eq!(
            refs,
            vec![
                MediaRef { id: "bg".into(), media_use: MediaUse::VideoLoop },
                MediaRef { id: "bg".into(), media_use: MediaUse::Poster },
            ]
        );
        assert!(media_refs(&crate::parse("# Plain\n").doc).is_empty());
    }
}

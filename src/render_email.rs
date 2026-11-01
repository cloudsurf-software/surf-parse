//! The EMAIL profile (0.43.0): one SurfDoc → the two parts of an email.
//!
//! A newsletter issue is a SurfDoc; the host sends it as a multipart email
//! whose HTML part has to survive mail clients (Gmail strips `<style>` from
//! most places, Outlook lays out with Word) and whose text part is the
//! markdown degradation. [`to_email`] returns both, from one
//! [`EmailProfile`]:
//!
//! - **Inline styles only.** Every element carries its own `style=""`,
//!   derived from the house tokens in `assets/surfdoc.css` (the light
//!   palette, the type scale, the card / button / image radii — pinned by a
//!   drift test). The `<head>` is the minimum clients tolerate: charset,
//!   viewport, `color-scheme`, the title. No `<style>`, no class names.
//! - **Tables for layout.** A full-width outer table carries the page
//!   colour; a 600 px centred table carries the card; buttons are
//!   bulletproof table cells.
//! - **Absolute URLs.** Every `src` / `href` is joined onto
//!   [`EmailProfile::base_url`] (`/a.png` → `https://site.example/a.png`).
//!   A link with a scheme other than http(s) / mailto / tel is dropped to
//!   its text; an image other than http(s) is dropped.
//! - **No scripts, no forms.** Interactive blocks degrade deterministically
//!   (the table on [`email_disposition`]): a `::form` becomes a link to the
//!   issue's web page when the profile names one, else nothing; a
//!   `::subscribe` is dropped (the reader is already subscribed); a video or
//!   embed with an http(s) source becomes a link to it. App chrome and
//!   infrastructure blocks are dropped. A final scrub removes any
//!   `<script>`, `<style>`, `<form>` or form control that got through.
//! - **The footer slot.** The caller fills [`EmailFooter`]: the postal
//!   address and the unsubscribe link always render (the law asks for
//!   both), the "Sent with CloudSurf" line only when asked.
//!
//! The text part is [`crate::render_md`]'s degradation of the same blocks,
//! with the same drops and links, every relative link made absolute, and the
//! footer as plain lines.

use crate::render_html::{escape_html, render_inline_markdown_phrasing, render_markdown};
use crate::types::{Block, CalloutType, SurfDoc};

// ---------------------------------------------------------------------------
// The house tokens, as email-safe literals
// ---------------------------------------------------------------------------

/// The house tokens the email profile paints with — the LIGHT palette of
/// `assets/surfdoc.css` `:root` (mail clients render light; the doc never
/// gets a `prefers-color-scheme` query). Each constant names its CSS
/// variable; `tests::tokens_match_the_stylesheet` pins the values.
pub mod tokens {
    /// `--background` — the page behind the card.
    pub const PAGE: &str = "#f7f8fc";
    /// `--surface` — the card.
    pub const SURFACE: &str = "#ffffff";
    /// `--surface-alt` — callouts, quotes, table heads, the banner band.
    pub const SURFACE_ALT: &str = "#eef1f7";
    /// `--border` — every rule and outline.
    pub const BORDER: &str = "#dde2ec";
    /// `--text` — body copy and headings.
    pub const TEXT: &str = "#0f1422";
    /// `--text-muted` — captions, attributions, the footer.
    pub const MUTED: &str = "#636a7e";
    /// `--accent` — links and the primary button.
    pub const ACCENT: &str = "#2563eb";
    /// `--accent-soft` — the info callout's tint.
    pub const ACCENT_SOFT: &str = "#e3ebff";
    /// `--accent-text` — text on the accent.
    pub const ACCENT_TEXT: &str = "#ffffff";
    /// `--code-bg` — code blocks.
    pub const CODE_BG: &str = "#f7f8fc";
    /// `--danger` — the danger callout's outline.
    pub const DANGER: &str = "#ef4444";
    /// `--success` — the success callout's outline.
    pub const SUCCESS: &str = "#22c55e";
    /// `--warning` — the warning callout's outline.
    pub const WARNING: &str = "#f59e0b";
    /// `--font-sans`, with the two fallbacks Windows mail clients need.
    pub const FONT: &str = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif";
    /// `--font-mono`.
    pub const FONT_MONO: &str = "'SF Mono', 'Fira Code', 'Cascadia Code', Menlo, Consolas, monospace";
    /// `--font-size-body`.
    pub const SIZE_BODY: &str = "17px";
    /// `--font-size-caption`.
    pub const SIZE_CAPTION: &str = "14px";
    /// `--font-size-micro`.
    pub const SIZE_MICRO: &str = "12px";
    /// `--font-size-h1`.
    pub const SIZE_H1: &str = "28px";
    /// `--font-size-h2`.
    pub const SIZE_H2: &str = "24px";
    /// `--font-size-h3`.
    pub const SIZE_H3: &str = "21px";
    /// `--font-size-display` — the hero headline.
    pub const SIZE_DISPLAY: &str = "38px";
    /// `--ws-radius-card`.
    pub const RADIUS_CARD: &str = "16px";
    /// `--ws-radius-btn`.
    pub const RADIUS_BTN: &str = "10px";
    /// `--ws-radius-img`.
    pub const RADIUS_IMG: &str = "10px";
}

use tokens::*;

/// The card's width in CSS pixels — the email convention every client lays
/// out without scaling.
pub const EMAIL_WIDTH_PX: u32 = 600;
/// The card's inner padding on each side.
const CARD_PAD_PX: u32 = 32;
/// The content column inside the card (600 − 2 × 32).
const CONTENT_PX: u32 = EMAIL_WIDTH_PX - 2 * CARD_PAD_PX;

/// Where the "Sent with CloudSurf" line links.
pub const SENT_WITH_CLOUDSURF_URL: &str = "https://cloudsurf.com";

// ---------------------------------------------------------------------------
// The API
// ---------------------------------------------------------------------------

/// The footer slot the caller fills — the sender's facts, not the doc's.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmailFooter {
    /// The sender's postal address. Always rendered (one line per `\n`).
    pub postal_address: String,
    /// The one-tap unsubscribe page for THIS recipient. Always rendered;
    /// joined onto the profile's `base_url` when relative.
    pub unsubscribe_url: String,
    /// The list's name for the reason line ("You're receiving this because
    /// you subscribed to {sender_name}."); `None` = no reason line.
    pub sender_name: Option<String>,
    /// Draw the "Sent with CloudSurf" line (the host decides — the free
    /// tier).
    pub sent_with_cloudsurf: bool,
}

/// How a SurfDoc becomes an email: where its relative URLs resolve, the
/// footer slot, and two optional lines.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmailProfile {
    /// The absolute origin (and optional path) relative `src` / `href`
    /// values join onto: `https://site.example` or
    /// `https://site.example/news/`. Empty = relative URLs stay as written.
    pub base_url: String,
    /// The footer slot.
    pub footer: EmailFooter,
    /// The preview text an inbox shows after the subject (hidden in the
    /// body). `None` = none.
    pub preheader: Option<String>,
    /// The issue's web page: a "View in your browser" line on top, and the
    /// destination a degraded `::form` links to. `None` = neither.
    pub web_url: Option<String>,
}

impl EmailProfile {
    /// A profile with a base URL and a footer, no preheader, no web page.
    pub fn new(base_url: impl Into<String>, footer: EmailFooter) -> Self {
        Self { base_url: base_url.into(), footer, preheader: None, web_url: None }
    }
}

/// The two parts of one email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailParts {
    /// The `text/html` part: a whole document, inline styles only.
    pub html: String,
    /// The `text/plain` part: the markdown degradation plus the footer.
    pub text: String,
}

/// Render `doc` as an email: [`to_email_html`] and [`to_email_text`].
pub fn to_email(doc: &SurfDoc, profile: &EmailProfile) -> EmailParts {
    EmailParts { html: to_email_html(doc, profile), text: to_email_text(doc, profile) }
}

impl SurfDoc {
    /// Render this document as an email's HTML and text parts (0.43.0) —
    /// see [`crate::render_email`].
    pub fn to_email(&self, profile: &EmailProfile) -> EmailParts {
        to_email(self, profile)
    }
}

/// What the email profile does with one block. Total and deterministic:
/// every block maps to exactly one disposition, and a block kind added to
/// the crate later is [`EmailDisposition::Drop`] until it is given one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailDisposition {
    /// The block has an email arm of its own.
    Render,
    /// The block degrades through its markdown form (`render_md`).
    Degrade,
    /// The block becomes one link: `(label, url as authored)`.
    Link(String, String),
    /// The block is left out.
    Drop,
}

/// The table: how each block crosses into an email under `profile`.
pub fn email_disposition(block: &Block, profile: &EmailProfile) -> EmailDisposition {
    use EmailDisposition::*;
    match block {
        Block::Markdown { .. }
        | Block::Callout { .. }
        | Block::Quote { .. }
        | Block::Testimonial { .. }
        | Block::Code { .. }
        | Block::Figure { .. }
        | Block::HeroImage { .. }
        | Block::Gallery { .. }
        | Block::Cta { .. }
        | Block::Hero { .. }
        | Block::Banner { .. }
        | Block::Divider { .. }
        | Block::Data { .. }
        | Block::Details { .. }
        | Block::Columns { .. }
        | Block::Tabs { .. }
        | Block::Section { .. }
        | Block::Page { .. } => Render,

        Block::Tasks { .. }
        | Block::Decision { .. }
        | Block::Metric { .. }
        | Block::Summary { .. }
        | Block::Faq { .. }
        | Block::PricingTable { .. }
        | Block::Features { .. }
        | Block::Steps { .. }
        | Block::Stats { .. }
        | Block::Comparison { .. }
        | Block::Timeline { .. }
        | Block::ProductCard { .. }
        | Block::Hours { .. }
        | Block::Carousel { .. }
        | Block::Marquee { .. }
        | Block::Related { .. }
        | Block::Footnote { .. }
        | Block::Bibliography { .. }
        | Block::Cite { .. } => Degrade,

        // Interactive: a form opens on the issue's web page; a subscribe
        // block is dropped — whoever reads the issue already subscribed.
        Block::Form { .. } => match &profile.web_url {
            Some(url) => Link("Open the form on the web".to_string(), url.clone()),
            None => Drop,
        },
        Block::Video { src, caption, alt, .. } => {
            let label = caption.clone().or_else(|| alt.clone()).unwrap_or_else(|| "Watch the video".to_string());
            web_link(label, src, profile)
        }
        Block::Embed { src, title, .. } => {
            web_link(title.clone().unwrap_or_else(|| "Open the embed".to_string()), src, profile)
        }
        Block::Player { title, .. } => match &profile.web_url {
            Some(url) => Link(
                match title {
                    Some(t) if !t.trim().is_empty() => format!("Listen to {} on the web", t.trim()),
                    _ => "Listen on the web".to_string(),
                },
                url.clone(),
            ),
            None => Drop,
        },

        _ => Drop,
    }
}

/// A media block's link: its own source when it is an http(s) URL after
/// joining, else nothing.
fn web_link(label: String, src: &str, profile: &EmailProfile) -> EmailDisposition {
    match absolutize_url(src, &profile.base_url) {
        Some(url) if is_http(&url) => EmailDisposition::Link(label, url),
        _ => EmailDisposition::Drop,
    }
}

fn is_http(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// Join `raw` onto `base_url` the way an email needs it, or `None` when the
/// URL may not appear in an email.
///
/// - `http:` / `https:` / `mailto:` / `tel:` → unchanged; any other scheme
///   (`javascript:`, `data:`, `media:` …) → `None`.
/// - `//host/x` → `https://host/x`.
/// - `#anchor` → unchanged (an in-message anchor).
/// - `/path` → the base's origin + `/path`; `path` → the base's directory
///   + `path` (a base with no path is its own directory).
/// - An empty `raw` → `None`; an empty or non-http(s) base leaves a
///   relative URL as written.
pub fn absolutize_url(raw: &str, base_url: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("//") {
        return Some(format!("https:{raw}"));
    }
    if raw.starts_with('#') {
        return Some(raw.to_string());
    }
    if let Some(i) = raw.find([':', '/', '?', '#'])
        && raw.as_bytes()[i] == b':'
    {
        let scheme = raw[..i].to_ascii_lowercase();
        let is_scheme = scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if is_scheme {
            return matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel").then(|| raw.to_string());
        }
    }
    let base = base_url.trim();
    if !is_http(base) {
        return Some(raw.to_string());
    }
    let after_scheme = base.find("://").map(|i| i + 3).unwrap_or(0);
    let origin_end = base[after_scheme..]
        .find(['/', '?', '#'])
        .map(|i| after_scheme + i)
        .unwrap_or(base.len());
    let origin = &base[..origin_end];
    if raw.starts_with('/') {
        return Some(format!("{origin}{raw}"));
    }
    // The directory: the base's path up to its last `/` (query and fragment
    // ignored); no path at all = the origin's root.
    let path = &base[origin_end..];
    let path = path.split(['?', '#']).next().unwrap_or("");
    let dir = match path.rfind('/') {
        Some(i) => &path[..=i],
        None => "/",
    };
    let rel = raw.strip_prefix("./").unwrap_or(raw);
    Some(format!("{origin}{dir}{rel}"))
}

/// An image source fit for an email: an http(s) URL after joining.
fn image_url(raw: &str, profile: &EmailProfile) -> Option<String> {
    absolutize_url(raw, &profile.base_url).filter(|u| is_http(u))
}

/// A link target fit for an email (see [`absolutize_url`]).
fn link_url(raw: &str, profile: &EmailProfile) -> Option<String> {
    absolutize_url(raw, &profile.base_url)
}

// ---------------------------------------------------------------------------
// The HTML part
// ---------------------------------------------------------------------------

/// The document's title: front matter `title:`, else the first `# ` line
/// of the first markdown block, else empty.
fn doc_title(doc: &SurfDoc) -> String {
    if let Some(t) = doc.front_matter.as_ref().and_then(|fm| fm.title.as_deref())
        && !t.trim().is_empty()
    {
        return t.trim().to_string();
    }
    for block in &doc.blocks {
        if let Block::Markdown { content, .. } = block
            && let Some(line) = content.lines().find_map(|l| l.trim().strip_prefix("# "))
        {
            return line.trim().to_string();
        }
    }
    String::new()
}

/// The `text/html` part of [`to_email`].
pub fn to_email_html(doc: &SurfDoc, profile: &EmailProfile) -> String {
    let _cite_scope = crate::citation::install_context(crate::citation::build_context(
        &doc.blocks,
        doc.front_matter.as_ref().and_then(|fm| fm.format),
    ));
    let mut body = String::new();
    for block in &doc.blocks {
        body.push_str(&block_html(block, profile));
    }
    let body = scrub(&body);

    let title = escape_html(&doc_title(doc));
    let mut out = String::with_capacity(body.len() + 4096);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\" xmlns=\"http://www.w3.org/1999/xhtml\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\">\n");
    out.push_str("<meta name=\"x-apple-disable-message-reformatting\">\n");
    out.push_str("<meta name=\"color-scheme\" content=\"light\">\n");
    out.push_str("<meta name=\"supported-color-schemes\" content=\"light\">\n");
    out.push_str(&format!("<title>{title}</title>\n</head>\n"));
    out.push_str(&format!(
        "<body style=\"margin:0;padding:0;width:100%;background-color:{PAGE};-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%;\">\n"
    ));
    if let Some(pre) = profile.preheader.as_deref().filter(|p| !p.trim().is_empty()) {
        out.push_str(&format!(
            "<div style=\"display:none;max-height:0;max-width:0;overflow:hidden;opacity:0;mso-hide:all;font-size:1px;line-height:1px;color:{PAGE};\">{}</div>\n",
            escape_html(pre.trim())
        ));
    }
    out.push_str(&format!(
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" bgcolor=\"{PAGE}\" style=\"width:100%;background-color:{PAGE};\">\n<tr>\n<td align=\"center\" style=\"padding:24px 12px;\">\n"
    ));
    if let Some(web) = profile.web_url.as_deref().and_then(|u| link_url(u, profile)) {
        out.push_str(&format!(
            "<table role=\"presentation\" width=\"{EMAIL_WIDTH_PX}\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:100%;max-width:{EMAIL_WIDTH_PX}px;\">\n<tr>\n<td align=\"center\" style=\"padding:0 0 12px;font-family:{FONT};font-size:{SIZE_MICRO};line-height:1.5;color:{MUTED};\"><a href=\"{}\" style=\"color:{MUTED};text-decoration:underline;\">View this email in your browser</a></td>\n</tr>\n</table>\n",
            escape_html(&web)
        ));
    }
    out.push_str(&format!(
        "<table role=\"presentation\" width=\"{EMAIL_WIDTH_PX}\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" bgcolor=\"{SURFACE}\" style=\"width:100%;max-width:{EMAIL_WIDTH_PX}px;background-color:{SURFACE};border:1px solid {BORDER};border-radius:{RADIUS_CARD};\">\n<tr>\n<td style=\"padding:{CARD_PAD_PX}px;font-family:{FONT};font-size:{SIZE_BODY};line-height:1.6;color:{TEXT};text-align:left;\">\n"
    ));
    out.push_str(&body);
    out.push_str("</td>\n</tr>\n</table>\n");
    out.push_str(&footer_html(&profile.footer, profile));
    out.push_str("</td>\n</tr>\n</table>\n</body>\n</html>\n");
    out
}

/// The footer under the card: the reason line, the unsubscribe link, the
/// postal address, and the "Sent with CloudSurf" line when asked.
fn footer_html(footer: &EmailFooter, profile: &EmailProfile) -> String {
    let cell = format!("font-family:{FONT};font-size:{SIZE_MICRO};line-height:1.6;color:{MUTED};");
    let mut rows = String::new();
    if let Some(name) = footer.sender_name.as_deref().filter(|n| !n.trim().is_empty()) {
        rows.push_str(&format!(
            "<p style=\"margin:0 0 8px;{cell}\">You're receiving this because you subscribed to {}.</p>\n",
            escape_html(name.trim())
        ));
    }
    let unsubscribe = link_url(&footer.unsubscribe_url, profile).unwrap_or_default();
    rows.push_str(&format!(
        "<p style=\"margin:0 0 8px;{cell}\"><a href=\"{}\" style=\"color:{MUTED};text-decoration:underline;\">Unsubscribe</a></p>\n",
        escape_html(&unsubscribe)
    ));
    let address: Vec<String> = footer
        .postal_address
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(escape_html)
        .collect();
    rows.push_str(&format!("<p style=\"margin:0 0 8px;{cell}\">{}</p>\n", address.join("<br>")));
    if footer.sent_with_cloudsurf {
        rows.push_str(&format!(
            "<p style=\"margin:0;{cell}\">Sent with <a href=\"{SENT_WITH_CLOUDSURF_URL}\" style=\"color:{MUTED};text-decoration:underline;\">CloudSurf</a></p>\n"
        ));
    }
    format!(
        "<table role=\"presentation\" width=\"{EMAIL_WIDTH_PX}\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:100%;max-width:{EMAIL_WIDTH_PX}px;\">\n<tr>\n<td align=\"center\" style=\"padding:24px {CARD_PAD_PX}px 0;text-align:center;{cell}\">\n{rows}</td>\n</tr>\n</table>\n"
    )
}

/// One block's email HTML (empty for a dropped block).
fn block_html(block: &Block, profile: &EmailProfile) -> String {
    match email_disposition(block, profile) {
        EmailDisposition::Drop => String::new(),
        EmailDisposition::Link(label, url) => link_paragraph(&label, &url, profile),
        EmailDisposition::Degrade => md_html(&crate::render_md::render_block(block), profile),
        EmailDisposition::Render => render_arm(block, profile),
    }
}

fn link_paragraph(label: &str, url: &str, profile: &EmailProfile) -> String {
    match link_url(url, profile) {
        Some(href) => format!(
            "<p style=\"{}\"><a href=\"{}\" style=\"{}\">{}</a></p>\n",
            p_style(),
            escape_html(&href),
            a_style(),
            escape_html(label)
        ),
        None => String::new(),
    }
}

fn p_style() -> String {
    format!("margin:0 0 16px;font-family:{FONT};font-size:{SIZE_BODY};line-height:1.6;color:{TEXT};")
}

fn a_style() -> String {
    format!("color:{ACCENT};text-decoration:underline;")
}

fn muted_style(align: &str) -> String {
    format!(
        "margin:0 0 16px;font-family:{FONT};font-size:{SIZE_CAPTION};line-height:1.5;color:{MUTED};text-align:{align};"
    )
}

fn heading_style(size: &str, weight: u32, top: u32) -> String {
    format!(
        "margin:{top}px 0 12px;font-family:{FONT};font-size:{size};line-height:1.25;font-weight:{weight};color:{TEXT};"
    )
}

/// A markdown body, rendered and sanitized by the crate's markdown path,
/// then styled for email.
fn md_html(content: &str, profile: &EmailProfile) -> String {
    if content.trim().is_empty() {
        return String::new();
    }
    style_markup(&render_markdown(&task_markers_as_text(content)), profile)
}

/// A task-list item's `[ ]` / `[x]` marker kept as text: the sanitizer
/// drops the checkbox `<input>` the markdown path would draw, so the item
/// would lose its state.
fn task_markers_as_text(content: &str) -> String {
    if !content.contains("] ") {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len() + 16);
    for line in content.split_inclusive('\n') {
        let indent = line.len() - line.trim_start().len();
        let body = &line[indent..];
        let marker_len = if body.starts_with("- ") || body.starts_with("* ") || body.starts_with("+ ") {
            Some(2)
        } else {
            let digits = body.chars().take_while(|c| c.is_ascii_digit()).count();
            (digits > 0 && (body[digits..].starts_with(". ") || body[digits..].starts_with(") "))).then_some(digits + 2)
        };
        match marker_len {
            Some(m) if ["[ ] ", "[x] ", "[X] "].iter().any(|t| body[m..].starts_with(t)) => {
                out.push_str(&line[..indent + m]);
                out.push_str("\\[");
                out.push_str(&body[m + 1..m + 2]);
                out.push_str("\\]");
                out.push_str(&body[m + 3..]);
            }
            _ => out.push_str(line),
        }
    }
    out
}

/// Inline markdown with no `<p>` wrapper, styled for email.
fn md_inline(content: &str, profile: &EmailProfile) -> String {
    style_markup(&render_inline_markdown_phrasing(content), profile)
}

/// An image row: a block `<img>` at the content width, optionally linked.
fn image_html(src: &str, alt: &str, profile: &EmailProfile) -> String {
    let Some(url) = image_url(src, profile) else {
        return String::new();
    };
    format!(
        "<img src=\"{}\" alt=\"{}\" width=\"{CONTENT_PX}\" style=\"display:block;width:100%;max-width:{CONTENT_PX}px;height:auto;border:0;outline:none;text-decoration:none;border-radius:{RADIUS_IMG};margin:0 0 12px;\">\n",
        escape_html(&url),
        escape_html(alt)
    )
}

/// A bulletproof button: a table cell painted with the accent (primary) or
/// outlined (secondary), the link filling it.
fn button_html(label: &str, href: &str, primary: bool, profile: &EmailProfile) -> String {
    let Some(url) = link_url(href, profile) else {
        return String::new();
    };
    let (bg, fg, border) = if primary { (ACCENT, ACCENT_TEXT, ACCENT) } else { (SURFACE, ACCENT, ACCENT) };
    format!(
        "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin:0 0 16px;\">\n<tr>\n<td align=\"center\" bgcolor=\"{bg}\" style=\"background-color:{bg};border:1px solid {border};border-radius:{RADIUS_BTN};\"><a href=\"{}\" style=\"display:inline-block;padding:12px 22px;font-family:{FONT};font-size:{SIZE_BODY};line-height:1.2;font-weight:600;color:{fg};text-decoration:none;border-radius:{RADIUS_BTN};\">{}</a></td>\n</tr>\n</table>\n",
        escape_html(&url),
        escape_html(label)
    )
}

/// A full-width one-cell table: the tinted panel callouts, quotes and the
/// banner draw in. The outline goes all the way round.
fn panel_html(inner: &str, bg: &str, border: &str, align: &str) -> String {
    format!(
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:100%;margin:0 0 16px;\">\n<tr>\n<td bgcolor=\"{bg}\" style=\"padding:16px 20px;background-color:{bg};border:1px solid {border};border-radius:{RADIUS_BTN};text-align:{align};\">\n{inner}</td>\n</tr>\n</table>\n"
    )
}

/// A panel's last paragraph keeps no bottom margin.
fn trim_last_margin(html: &str) -> String {
    match html.rfind("margin:0 0 16px;") {
        Some(i) => format!("{}margin:0;{}", &html[..i], &html[i + "margin:0 0 16px;".len()..]),
        None => html.to_string(),
    }
}

fn render_arm(block: &Block, profile: &EmailProfile) -> String {
    match block {
        Block::Markdown { content, .. } => md_html(content, profile),

        Block::Callout { callout_type, title, content, .. } => {
            let (bg, border) = match callout_type {
                CalloutType::Info | CalloutType::Note | CalloutType::Tip | CalloutType::Context => {
                    (ACCENT_SOFT, ACCENT)
                }
                CalloutType::Warning => (SURFACE_ALT, WARNING),
                CalloutType::Danger => (SURFACE_ALT, DANGER),
                CalloutType::Success => (SURFACE_ALT, SUCCESS),
            };
            let mut inner = String::new();
            if let Some(t) = title.as_deref().filter(|t| !t.trim().is_empty()) {
                inner.push_str(&format!(
                    "<p style=\"margin:0 0 6px;font-family:{FONT};font-size:{SIZE_BODY};line-height:1.4;font-weight:700;color:{TEXT};\">{}</p>\n",
                    escape_html(t.trim())
                ));
            }
            inner.push_str(&md_html(content, profile));
            panel_html(&trim_last_margin(&inner), bg, border, "left")
        }

        Block::Quote { content, attribution, .. } => {
            quote_html(content, attribution.as_deref(), profile)
        }
        Block::Testimonial { content, author, role, company, .. } => {
            let who: Vec<&str> = [author.as_deref(), role.as_deref(), company.as_deref()]
                .into_iter()
                .flatten()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            let who = (!who.is_empty()).then(|| who.join(", "));
            quote_html(content, who.as_deref(), profile)
        }

        Block::Code { content, .. } => format!(
            "<pre style=\"margin:0 0 16px;padding:14px 16px;background-color:{CODE_BG};border:1px solid {BORDER};border-radius:{RADIUS_BTN};font-family:{FONT_MONO};font-size:{SIZE_CAPTION};line-height:1.5;color:{TEXT};white-space:pre-wrap;word-break:break-word;\">{}</pre>\n",
            escape_html(content.trim_end_matches('\n'))
        ),

        Block::Figure { src, caption, alt, .. } => {
            let mut html = image_html(src, alt.as_deref().unwrap_or(""), profile);
            if html.is_empty() {
                return String::new();
            }
            if let Some(c) = caption.as_deref().filter(|c| !c.trim().is_empty()) {
                html.push_str(&format!("<p style=\"{}\">{}</p>\n", muted_style("center"), escape_html(c.trim())));
            }
            html
        }
        Block::HeroImage { src, alt, .. } => image_html(src, alt.as_deref().unwrap_or(""), profile),
        Block::Gallery { items, .. } => {
            let mut html = String::new();
            for item in items {
                let img = image_html(&item.src, item.alt.as_deref().unwrap_or(""), profile);
                if img.is_empty() {
                    continue;
                }
                html.push_str(&img);
                if let Some(c) = item.caption.as_deref().filter(|c| !c.trim().is_empty()) {
                    html.push_str(&format!("<p style=\"{}\">{}</p>\n", muted_style("center"), escape_html(c.trim())));
                }
            }
            html
        }

        Block::Cta { label, href, primary, .. } => button_html(label, href, *primary, profile),

        Block::Hero { headline, subtitle, badge, image, image_alt, buttons, content, .. } => {
            let mut html = String::new();
            if let Some(src) = image {
                html.push_str(&image_html(src, image_alt.as_deref().unwrap_or(""), profile));
            }
            if let Some(b) = badge.as_deref().filter(|b| !b.trim().is_empty()) {
                html.push_str(&format!(
                    "<p style=\"margin:0 0 8px;font-family:{FONT};font-size:{SIZE_MICRO};line-height:1.4;font-weight:700;letter-spacing:0.05em;text-transform:uppercase;color:{ACCENT};\">{}</p>\n",
                    escape_html(b.trim())
                ));
            }
            if let Some(h) = headline.as_deref().filter(|h| !h.trim().is_empty()) {
                html.push_str(&format!(
                    "<h1 style=\"{}\">{}</h1>\n",
                    heading_style(SIZE_DISPLAY, 900, 0),
                    md_inline(h.trim(), profile)
                ));
            }
            if let Some(s) = subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
                html.push_str(&format!(
                    "<p style=\"margin:0 0 16px;font-family:{FONT};font-size:{SIZE_H3};line-height:1.45;color:{MUTED};\">{}</p>\n",
                    md_inline(s.trim(), profile)
                ));
            }
            html.push_str(&md_html(content, profile));
            for b in buttons {
                html.push_str(&button_html(&b.label, &b.href, b.primary, profile));
            }
            html
        }

        Block::Banner { headline, subtitle, buttons, content, .. } => {
            let mut inner = String::new();
            if let Some(h) = headline.as_deref().filter(|h| !h.trim().is_empty()) {
                inner.push_str(&format!(
                    "<h2 style=\"{}\">{}</h2>\n",
                    heading_style(SIZE_H2, 800, 0),
                    md_inline(h.trim(), profile)
                ));
            }
            if let Some(s) = subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
                inner.push_str(&format!("<p style=\"{}\">{}</p>\n", p_style(), md_inline(s.trim(), profile)));
            }
            inner.push_str(&md_html(content, profile));
            for b in buttons {
                // Centre the button table inside the centred band.
                inner.push_str(&button_html(&b.label, &b.href, b.primary, profile).replacen(
                    "style=\"margin:0 0 16px;\"",
                    "align=\"center\" style=\"margin:0 auto 16px;\"",
                    1,
                ));
            }
            panel_html(&trim_last_margin(&inner), SURFACE_ALT, BORDER, "center")
        }

        Block::Divider { label, .. } => {
            let mut html = String::new();
            if let Some(l) = label.as_deref().filter(|l| !l.trim().is_empty()) {
                html.push_str(&format!(
                    "<p style=\"margin:24px 0 8px;font-family:{FONT};font-size:{SIZE_MICRO};line-height:1.4;font-weight:700;letter-spacing:0.05em;text-transform:uppercase;color:{MUTED};text-align:center;\">{}</p>\n",
                    escape_html(l.trim())
                ));
            }
            html.push_str(&rule_html());
            html
        }

        Block::Data { headers, rows, caption, .. } => data_html(headers, rows, caption.as_deref(), profile),

        Block::Details { title, content, .. } => {
            let mut html = String::new();
            if let Some(t) = title.as_deref().filter(|t| !t.trim().is_empty()) {
                html.push_str(&format!("<h3 style=\"{}\">{}</h3>\n", heading_style(SIZE_H3, 700, 8), escape_html(t.trim())));
            }
            html.push_str(&md_html(content, profile));
            html
        }

        // Columns stack: a phone reads them top to bottom anyway, and
        // stacking is the one layout every client draws the same.
        Block::Columns { columns, .. } => {
            columns.iter().map(|c| md_html(&c.content, profile)).collect()
        }
        Block::Tabs { tabs, .. } => {
            let mut html = String::new();
            for tab in tabs {
                html.push_str(&format!("<h3 style=\"{}\">{}</h3>\n", heading_style(SIZE_H3, 700, 8), escape_html(tab.label.trim())));
                html.push_str(&md_html(&tab.content, profile));
            }
            html
        }

        Block::Section { headline, subtitle, content, children, .. } => {
            let mut html = String::new();
            if let Some(h) = headline.as_deref().filter(|h| !h.trim().is_empty()) {
                html.push_str(&format!("<h2 style=\"{}\">{}</h2>\n", heading_style(SIZE_H2, 800, 8), md_inline(h.trim(), profile)));
            }
            if let Some(s) = subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
                html.push_str(&format!("<p style=\"{}\">{}</p>\n", muted_style("left"), md_inline(s.trim(), profile)));
            }
            if children.is_empty() {
                html.push_str(&md_html(content, profile));
            } else {
                for child in children {
                    html.push_str(&block_html(child, profile));
                }
            }
            html
        }
        Block::Page { children, content, .. } => {
            if children.is_empty() {
                md_html(content, profile)
            } else {
                children.iter().map(|c| block_html(c, profile)).collect()
            }
        }

        // `email_disposition` routes only the kinds above here.
        _ => String::new(),
    }
}

fn quote_html(content: &str, who: Option<&str>, profile: &EmailProfile) -> String {
    let mut inner = format!(
        "<p style=\"margin:0;font-family:{FONT};font-size:{SIZE_H3};line-height:1.5;font-style:italic;color:{TEXT};\">{}</p>\n",
        md_inline(content.trim(), profile)
    );
    if let Some(w) = who.filter(|w| !w.trim().is_empty()) {
        inner.push_str(&format!(
            "<p style=\"margin:8px 0 0;font-family:{FONT};font-size:{SIZE_CAPTION};line-height:1.4;color:{MUTED};\">&mdash; {}</p>\n",
            escape_html(w.trim())
        ));
    }
    panel_html(&inner, SURFACE_ALT, BORDER, "left")
}

/// A horizontal rule as a one-pixel table row (an `<hr>` draws differently
/// in every client).
fn rule_html() -> String {
    format!(
        "<table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:100%;margin:16px 0 24px;\">\n<tr>\n<td height=\"1\" bgcolor=\"{BORDER}\" style=\"height:1px;line-height:1px;font-size:1px;background-color:{BORDER};\">&nbsp;</td>\n</tr>\n</table>\n"
    )
}

fn data_html(headers: &[String], rows: &[Vec<String>], caption: Option<&str>, profile: &EmailProfile) -> String {
    let th = format!(
        "padding:8px 10px;border:1px solid {BORDER};background-color:{SURFACE_ALT};font-family:{FONT};font-size:{SIZE_CAPTION};line-height:1.4;font-weight:700;color:{TEXT};text-align:left;"
    );
    let td = format!(
        "padding:8px 10px;border:1px solid {BORDER};font-family:{FONT};font-size:{SIZE_CAPTION};line-height:1.4;color:{TEXT};text-align:left;"
    );
    let mut html = String::from(
        "<table role=\"table\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"width:100%;border-collapse:collapse;margin:0 0 16px;\">\n",
    );
    if !headers.is_empty() {
        html.push_str("<tr>");
        for h in headers {
            html.push_str(&format!("<th style=\"{th}\">{}</th>", md_inline(h, profile)));
        }
        html.push_str("</tr>\n");
    }
    for row in rows {
        html.push_str("<tr>");
        for cell in row {
            html.push_str(&format!("<td style=\"{td}\">{}</td>", md_inline(cell, profile)));
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</table>\n");
    if let Some(c) = caption.filter(|c| !c.trim().is_empty()) {
        html.push_str(&format!("<p style=\"{}\">{}</p>\n", muted_style("left"), escape_html(c.trim())));
    }
    html
}

// ---------------------------------------------------------------------------
// Styling sanitized markdown markup
// ---------------------------------------------------------------------------

/// One parsed start tag: its name (lowercased) and attributes in order.
struct StartTag {
    name: String,
    attrs: Vec<(String, Option<String>)>,
    self_closing: bool,
}

/// Parse the start tag at `s` (which begins with `<` + a letter). Returns
/// the tag and the byte length consumed, or `None` when it never closes.
fn parse_start_tag(s: &str) -> Option<(StartTag, usize)> {
    let bytes = s.as_bytes();
    let mut i = 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    let name = s[1..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        match bytes[i] {
            b'>' => return Some((StartTag { name, attrs, self_closing }, i + 1)),
            b'/' => {
                self_closing = true;
                i += 1;
                continue;
            }
            _ => {}
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && !matches!(bytes[i], b'=' | b'>' | b'/') {
            i += 1;
        }
        let attr_name = s[start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() {
                return None;
            }
            let value = if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                let vstart = i + 1;
                let end = s[vstart..].find(quote as char)? + vstart;
                i = end + 1;
                s[vstart..end].to_string()
            } else {
                let vstart = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                s[vstart..i].to_string()
            };
            attrs.push((attr_name, Some(value)));
        } else if !attr_name.is_empty() {
            attrs.push((attr_name, None));
        } else {
            i += 1;
        }
    }
}

/// Tags removed together with everything inside them.
const DROP_WITH_CONTENT: &[&str] = &["script", "style", "iframe", "object", "noscript", "template", "svg", "form", "select", "textarea", "video", "audio", "canvas"];
/// Tags removed, their content kept (or void).
const DROP_TAG_ONLY: &[&str] = &["input", "button", "label", "fieldset", "legend", "source", "embed", "link", "meta", "base"];

/// The inline style each markdown element wears in an email.
fn tag_style(name: &str, in_pre: bool) -> Option<String> {
    Some(match name {
        "p" => p_style(),
        "h1" => heading_style(SIZE_H1, 800, 0),
        "h2" => heading_style(SIZE_H2, 800, 24),
        "h3" => heading_style(SIZE_H3, 700, 20),
        "h4" | "h5" | "h6" => heading_style(SIZE_BODY, 700, 16),
        "a" => a_style(),
        "ul" | "ol" => "margin:0 0 16px;padding:0 0 0 24px;".to_string(),
        "li" => format!("margin:0 0 6px;font-family:{FONT};font-size:{SIZE_BODY};line-height:1.6;color:{TEXT};"),
        "blockquote" => format!(
            "margin:0 0 16px;padding:12px 16px;background-color:{SURFACE_ALT};border:1px solid {BORDER};border-radius:{RADIUS_BTN};color:{TEXT};"
        ),
        "pre" => format!(
            "margin:0 0 16px;padding:14px 16px;background-color:{CODE_BG};border:1px solid {BORDER};border-radius:{RADIUS_BTN};font-family:{FONT_MONO};font-size:{SIZE_CAPTION};line-height:1.5;color:{TEXT};white-space:pre-wrap;word-break:break-word;"
        ),
        "code" if in_pre => format!("font-family:{FONT_MONO};"),
        "code" => format!(
            "font-family:{FONT_MONO};font-size:0.9em;background-color:{SURFACE_ALT};padding:1px 4px;border-radius:4px;"
        ),
        "img" => format!(
            "display:block;max-width:100%;height:auto;border:0;outline:none;text-decoration:none;border-radius:{RADIUS_IMG};"
        ),
        "hr" => format!("border:0;height:1px;background-color:{BORDER};margin:24px 0;"),
        "table" => "width:100%;border-collapse:collapse;margin:0 0 16px;".to_string(),
        "th" => format!(
            "padding:8px 10px;border:1px solid {BORDER};background-color:{SURFACE_ALT};font-family:{FONT};font-size:{SIZE_CAPTION};line-height:1.4;font-weight:700;color:{TEXT};text-align:left;"
        ),
        "td" => format!(
            "padding:8px 10px;border:1px solid {BORDER};font-family:{FONT};font-size:{SIZE_CAPTION};line-height:1.4;color:{TEXT};text-align:left;"
        ),
        "strong" | "b" => "font-weight:700;".to_string(),
        _ => return None,
    })
}

/// Restyle sanitized markup for email: every known element gets its inline
/// style, `class` / `id` / `style` / `data-*` attributes go, `href` / `src`
/// join onto the base (an unusable link keeps its text, an unusable image is
/// dropped), task-list checkboxes become `[x]` / `[ ]`, and anything in
/// [`DROP_WITH_CONTENT`] / [`DROP_TAG_ONLY`] is removed.
fn style_markup(html: &str, profile: &EmailProfile) -> String {
    let mut out = String::with_capacity(html.len() * 2);
    let mut rest = html;
    let mut in_pre = false;
    // `<a>` tags whose href was refused: their `</a>` is dropped too.
    let mut dead_anchors: Vec<bool> = Vec::new();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let next = tail.as_bytes().get(1).copied().unwrap_or(b' ');
        if next == b'/' {
            let Some(close) = tail.find('>') else {
                out.push_str(tail);
                return out;
            };
            let name = tail[2..close].trim().to_ascii_lowercase();
            rest = &tail[close + 1..];
            if DROP_TAG_ONLY.contains(&name.as_str()) || DROP_WITH_CONTENT.contains(&name.as_str()) {
                continue;
            }
            if name == "a" && dead_anchors.pop().unwrap_or(false) {
                continue;
            }
            if name == "pre" {
                in_pre = false;
            }
            out.push_str(&format!("</{name}>"));
            continue;
        }
        if !next.is_ascii_alphabetic() {
            // A comment / doctype / stray `<`: comments go, the rest is text.
            if tail.starts_with("<!--") {
                rest = match tail.find("-->") {
                    Some(end) => &tail[end + 3..],
                    None => "",
                };
            } else {
                out.push_str("&lt;");
                rest = &tail[1..];
            }
            continue;
        }
        let Some((tag, len)) = parse_start_tag(tail) else {
            // An unterminated tag: drop the rest.
            return out;
        };
        rest = &tail[len..];
        let name = tag.name.as_str();
        if DROP_WITH_CONTENT.contains(&name) {
            if !tag.self_closing {
                let closing = format!("</{name}");
                rest = match rest.to_ascii_lowercase().find(&closing) {
                    Some(end) => match rest[end..].find('>') {
                        Some(gt) => &rest[end + gt + 1..],
                        None => "",
                    },
                    None => "",
                };
            }
            continue;
        }
        if name == "input" {
            let is_checkbox = tag.attrs.iter().any(|(k, v)| k == "type" && v.as_deref() == Some("checkbox"));
            if is_checkbox {
                let checked = tag.attrs.iter().any(|(k, _)| k == "checked");
                out.push_str(if checked { "[x] " } else { "[ ] " });
            }
            continue;
        }
        if DROP_TAG_ONLY.contains(&name) {
            continue;
        }
        let mut attrs = String::new();
        let mut refused = false;
        for (k, v) in &tag.attrs {
            let k = k.as_str();
            if k == "class" || k == "id" || k == "style" || k.starts_with("data-") || k.starts_with("on") {
                continue;
            }
            match (k, v) {
                ("href", Some(v)) => match link_url(&decode_attr(v), profile) {
                    Some(u) => attrs.push_str(&format!(" href=\"{}\"", escape_html(&u))),
                    None => refused = true,
                },
                ("src", Some(v)) => match image_url(&decode_attr(v), profile) {
                    Some(u) => attrs.push_str(&format!(" src=\"{}\"", escape_html(&u))),
                    None => refused = true,
                },
                (_, Some(v)) => attrs.push_str(&format!(" {k}=\"{}\"", escape_html(&decode_attr(v)))),
                (_, None) => attrs.push_str(&format!(" {k}")),
            }
        }
        if name == "a" {
            let has_href = tag.attrs.iter().any(|(k, _)| k == "href");
            if refused || !has_href {
                dead_anchors.push(true);
                continue;
            }
            dead_anchors.push(false);
        }
        if name == "img" && (refused || !tag.attrs.iter().any(|(k, _)| k == "src")) {
            continue;
        }
        if let Some(style) = tag_style(name, in_pre) {
            attrs.push_str(&format!(" style=\"{style}\""));
        }
        if name == "pre" {
            in_pre = true;
        }
        out.push('<');
        out.push_str(name);
        out.push_str(&attrs);
        out.push('>');
    }
    out.push_str(rest);
    out
}

/// Undo the entity escapes a serializer puts in an attribute value, so the
/// value can be re-escaped once.
fn decode_attr(v: &str) -> String {
    v.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The last line of defence over the whole body: no `<script>`, `<style>`,
/// `<iframe>`, `<form>` or form control survives, whatever arm wrote it.
fn scrub(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let forbidden = DROP_WITH_CONTENT.iter().chain(DROP_TAG_ONLY.iter());
    let mut hit = false;
    for name in forbidden {
        if lower.contains(&format!("<{name}")) {
            hit = true;
            break;
        }
    }
    if !hit {
        return html.to_string();
    }
    // Re-run the styler's drop rules over the markup, keeping every style.
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let name: String = tail[1..]
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        let end = tail.find('>').map(|e| e + 1).unwrap_or(tail.len());
        if DROP_WITH_CONTENT.contains(&name.as_str()) && !tail.starts_with("</") {
            let closing = format!("</{name}");
            let after = &tail[end..];
            rest = match after.to_ascii_lowercase().find(&closing) {
                Some(c) => match after[c..].find('>') {
                    Some(gt) => &after[c + gt + 1..],
                    None => "",
                },
                None => "",
            };
            continue;
        }
        if DROP_WITH_CONTENT.contains(&name.as_str()) || DROP_TAG_ONLY.contains(&name.as_str()) {
            rest = &tail[end..];
            continue;
        }
        out.push_str(&tail[..end]);
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// The text part
// ---------------------------------------------------------------------------

/// The `text/plain` part of [`to_email`]: the blocks' markdown degradation
/// (the email's drops and links applied, relative links made absolute),
/// then the footer as plain lines.
pub fn to_email_text(doc: &SurfDoc, profile: &EmailProfile) -> String {
    let _cite_scope = crate::citation::install_context(crate::citation::build_context(
        &doc.blocks,
        doc.front_matter.as_ref().and_then(|fm| fm.format),
    ));
    let mut parts: Vec<String> = Vec::new();
    if let Some(web) = profile.web_url.as_deref().and_then(|u| link_url(u, profile)) {
        parts.push(format!("View this email in your browser: {web}"));
    }
    for block in &doc.blocks {
        block_text(block, profile, &mut parts);
    }
    let mut text = parts
        .iter()
        .map(|p| p.trim_matches('\n'))
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    text.push_str("\n\n--\n");
    let footer = &profile.footer;
    if let Some(name) = footer.sender_name.as_deref().filter(|n| !n.trim().is_empty()) {
        text.push_str(&format!("You're receiving this because you subscribed to {}.\n", name.trim()));
    }
    text.push_str(&format!(
        "Unsubscribe: {}\n",
        link_url(&footer.unsubscribe_url, profile).unwrap_or_default()
    ));
    for line in footer.postal_address.lines().map(str::trim).filter(|l| !l.is_empty()) {
        text.push_str(line);
        text.push('\n');
    }
    if footer.sent_with_cloudsurf {
        text.push_str(&format!("Sent with CloudSurf: {SENT_WITH_CLOUDSURF_URL}\n"));
    }
    text
}

fn block_text(block: &Block, profile: &EmailProfile, parts: &mut Vec<String>) {
    match email_disposition(block, profile) {
        EmailDisposition::Drop => {}
        EmailDisposition::Link(label, url) => {
            if let Some(u) = link_url(&url, profile) {
                parts.push(format!("{label}: {u}"));
            }
        }
        _ => match block {
            Block::Section { headline, subtitle, children, .. } if !children.is_empty() => {
                if let Some(h) = headline.as_deref().filter(|h| !h.trim().is_empty()) {
                    parts.push(format!("## {}", h.trim()));
                }
                if let Some(s) = subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
                    parts.push(s.trim().to_string());
                }
                for child in children {
                    block_text(child, profile, parts);
                }
            }
            Block::Page { children, .. } if !children.is_empty() => {
                for child in children {
                    block_text(child, profile, parts);
                }
            }
            // Raw markup a markdown block carried (`<script>` …) is removed
            // from the text part too, by the same table as the HTML part; the
            // blank run it leaves closes up to one paragraph break.
            _ => {
                let md = absolutize_markdown_links(&crate::render_md::render_block(block), profile);
                let mut clean = scrub(&md);
                if clean != md {
                    while clean.contains("\n\n\n") {
                        clean = clean.replace("\n\n\n", "\n\n");
                    }
                }
                parts.push(clean);
            }
        },
    }
}

/// Join every inline markdown link / image target (`](target)`) onto the
/// profile's base; a refused target keeps its text and loses the link.
fn absolutize_markdown_links(md: &str, profile: &EmailProfile) -> String {
    let mut out = String::with_capacity(md.len());
    let mut rest = md;
    while let Some(i) = rest.find("](") {
        out.push_str(&rest[..i + 2]);
        let after = &rest[i + 2..];
        let Some(close) = after.find(')') else {
            out.push_str(after);
            return out;
        };
        let target = &after[..close];
        let (url, title) = match target.find(' ') {
            Some(sp) => (&target[..sp], &target[sp..]),
            None => (target, ""),
        };
        match link_url(url, profile) {
            Some(u) => {
                out.push_str(&u);
                out.push_str(title);
            }
            None => out.push('#'),
        }
        out.push(')');
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> EmailProfile {
        EmailProfile::new(
            "https://news.example.com/issues/",
            EmailFooter {
                postal_address: "CloudSurf Software LLC\n1 Main St, Reno NV 89501".into(),
                unsubscribe_url: "/n/u/tok123".into(),
                sender_name: Some("The Example Letter".into()),
                sent_with_cloudsurf: false,
            },
        )
    }

    #[test]
    fn tokens_match_the_stylesheet() {
        let css = crate::SURFDOC_CSS;
        for (var, value) in [
            ("--background", PAGE),
            ("--surface", SURFACE),
            ("--surface-alt", SURFACE_ALT),
            ("--border", BORDER),
            ("--text", TEXT),
            ("--text-muted", MUTED),
            ("--accent", ACCENT),
            ("--accent-soft", ACCENT_SOFT),
            ("--accent-text", ACCENT_TEXT),
            ("--code-bg", CODE_BG),
            ("--danger", DANGER),
            ("--success", SUCCESS),
            ("--warning", WARNING),
            ("--font-size-body", SIZE_BODY),
            ("--font-size-caption", SIZE_CAPTION),
            ("--font-size-micro", SIZE_MICRO),
            ("--font-size-h1", SIZE_H1),
            ("--font-size-h2", SIZE_H2),
            ("--font-size-h3", SIZE_H3),
            ("--font-size-display", SIZE_DISPLAY),
            ("--ws-radius-card", RADIUS_CARD),
            ("--ws-radius-btn", RADIUS_BTN),
            ("--ws-radius-img", RADIUS_IMG),
        ] {
            let decl = format!("{var}: {value};");
            assert!(css.contains(&decl), "assets/surfdoc.css no longer declares `{decl}`");
        }
        assert!(css.contains("--font-sans: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, sans-serif;"));
        assert!(css.contains("--font-mono: \"SF Mono\", \"Fira Code\", \"Cascadia Code\", Menlo, Consolas, monospace;"));
    }

    #[test]
    fn absolutize_joins_and_refuses() {
        let base = "https://news.example.com/issues/42";
        assert_eq!(absolutize_url("/a.png", base).as_deref(), Some("https://news.example.com/a.png"));
        assert_eq!(absolutize_url("a.png", base).as_deref(), Some("https://news.example.com/issues/a.png"));
        assert_eq!(absolutize_url("./a.png", base).as_deref(), Some("https://news.example.com/issues/a.png"));
        assert_eq!(absolutize_url("a.png", "https://x.example").as_deref(), Some("https://x.example/a.png"));
        assert_eq!(absolutize_url("//cdn.example/a.png", base).as_deref(), Some("https://cdn.example/a.png"));
        assert_eq!(absolutize_url("https://o.example/x", base).as_deref(), Some("https://o.example/x"));
        assert_eq!(absolutize_url("mailto:a@b.example", base).as_deref(), Some("mailto:a@b.example"));
        assert_eq!(absolutize_url("#top", base).as_deref(), Some("#top"));
        assert_eq!(absolutize_url("javascript:alert(1)", base), None);
        assert_eq!(absolutize_url("JavaScript:alert(1)", base), None);
        assert_eq!(absolutize_url("media:abc", base), None);
        assert_eq!(absolutize_url("", base), None);
        assert_eq!(absolutize_url("/a.png", "").as_deref(), Some("/a.png"));
    }

    #[test]
    fn style_markup_inlines_and_strips() {
        let p = profile();
        let html = style_markup(
            "<p class=\"x\" id=\"y\">Hi <a href=\"/about\">us</a> <a href=\"javascript:x\">bad</a></p><img src=\"/a.png\" alt=\"A\"><img src=\"data:image/png;base64,AA\" alt=\"B\"><script>alert(1)</script>",
            &p,
        );
        assert!(html.starts_with("<p style=\"margin:0 0 16px;"), "{html}");
        assert!(html.contains("<a href=\"https://news.example.com/about\" style=\"color:#2563eb;"));
        assert!(html.contains(" bad</p>"), "a refused link keeps its text: {html}");
        assert!(html.contains("<img src=\"https://news.example.com/a.png\" alt=\"A\" style=\"display:block;"));
        assert!(!html.contains("data:image"));
        assert!(!html.contains("script") && !html.contains("alert"));
        assert!(!html.contains("class=") && !html.contains("id="));
    }

    #[test]
    fn task_boxes_become_text() {
        let html = md_html("- [x] done\n- [ ] open\n", &profile());
        assert!(html.contains("[x] done") && html.contains("[ ] open"), "{html}");
        assert!(!html.contains("<input"));
    }

    #[test]
    fn scrub_removes_forbidden_markup() {
        let out = scrub("<p>a</p><form action=\"/x\"><input name=\"e\"><button>Go</button></form><script>x()</script><p>b</p>");
        assert_eq!(out, "<p>a</p><p>b</p>");
    }

    #[test]
    fn markdown_link_targets_join() {
        let out = absolutize_markdown_links("See [the post](/p/1) and ![a](img/a.png \"t\") and [x](javascript:y).", &profile());
        assert_eq!(
            out,
            "See [the post](https://news.example.com/p/1) and ![a](https://news.example.com/issues/img/a.png \"t\") and [x](#)."
        );
    }
}

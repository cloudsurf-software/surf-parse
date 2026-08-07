//! HTML fragment renderer.
//!
//! Produces semantic HTML with `surfdoc-*` CSS classes. Markdown blocks are
//! rendered through `pulldown-cmark`. All other content is HTML-escaped to
//! prevent XSS.

use crate::citation::{self, CiteRef};
use crate::icons::get_icon;
use crate::types::{Block, CalloutType, ChartType, DecisionStatus, Format, FormFieldType, HttpMethod, ListDisplay, NavGroup, NavItem, RowState, StyleProperty, SurfDoc, Trend};

/// Render a markdown string to HTML using pulldown-cmark with GFM extensions.
///
/// Tables are wrapped in `<div class="surfdoc-table-wrap">` for responsive scrolling.
fn render_markdown(content: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    let parser = pulldown_cmark::Parser::new_ext(content, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    // Sanitize HTML to prevent XSS — strips dangerous tags (script, iframe,
    // object, embed, etc.) and event-handler attributes while preserving safe
    // formatting produced by pulldown-cmark.
    let html_output = ammonia::clean(&html_output);
    // Wrap bare <table> tags in scroll containers for mobile responsiveness
    let html_output = html_output.replace("<table>", "<div class=\"surfdoc-table-wrap\"><table>");
    let html_output = html_output.replace("</table>", "</table></div>");
    // Resolve inline `[@key]` citations against the ambient citation context.
    substitute_cites_html(&html_output)
}

/// Replace inline `[@key]` citation tokens in rendered HTML with anchored
/// in-text citations, using the ambient [`citation::CiteContext`]. The tokens
/// survive markdown/sanitisation as literal text; we splice our own (trusted)
/// anchor HTML in afterwards so `class`/`id`/`href` are preserved. No-op without
/// an installed context or references.
fn substitute_cites_html(html: &str) -> String {
    citation::with_active(|ctx| match ctx {
        Some(ctx) if !ctx.references.is_empty() => {
            let cites = crate::inline::find_inline_cites(html);
            if cites.is_empty() {
                return html.to_string();
            }
            let mut out = String::with_capacity(html.len());
            let mut last = 0;
            for (s, e, cr) in cites {
                out.push_str(&html[last..s]);
                out.push_str(&render_cite_html(&cr, ctx));
                last = e;
            }
            out.push_str(&html[last..]);
            out
        }
        _ => html.to_string(),
    })
}

/// Render a single inline citation group as an anchor linking to its first
/// reference's bibliography entry. Numbered styles wrap in `<sup>`.
fn render_cite_html(cr: &CiteRef, ctx: &citation::CiteContext) -> String {
    let text = citation::format_in_text(&ctx.references, cr, ctx.style, &ctx.numbers);
    let first_key = cr.items.first().map(|i| i.key.as_str()).unwrap_or("");
    let href = format!("#ref-{}", escape_html(first_key));
    let inner = escape_html(&text);
    if citation::is_numbered(ctx.style) {
        format!(
            "<sup class=\"surfdoc-cite\"><a href=\"{href}\">{inner}</a></sup>"
        )
    } else {
        format!("<a class=\"surfdoc-cite\" href=\"{href}\">{inner}</a>")
    }
}

/// Render a `::bibliography` block from the ambient citation context.
fn render_bibliography_html(style_override: Option<Format>) -> String {
    citation::with_active(|ctx| {
        let Some(ctx) = ctx else {
            return String::new();
        };
        if ctx.references.is_empty() {
            return String::new();
        }
        let style = style_override.unwrap_or(ctx.style);
        // With no override, use citation-number order for numbered styles; with
        // an override, pass definition order (the keyed list sorts author styles
        // and numbers numbered styles by input order).
        let refs = if style_override.is_some() {
            ctx.references.clone()
        } else {
            citation::ordered_references(ctx)
        };
        let entries = citation::reference_list_keyed(&refs, style);
        let heading = citation::bibliography_heading(style);
        let style_slug = format_slug(style);
        let mut body = String::new();
        if citation::is_numbered(style) {
            body.push_str("<ol class=\"surfdoc-bibliography-list\">");
            for (key, formatted) in &entries {
                body.push_str(&format!(
                    "<li id=\"ref-{}\" class=\"surfdoc-bibliography-entry\">{}</li>",
                    escape_html(key),
                    render_inline_markdown_phrasing(formatted)
                ));
            }
            body.push_str("</ol>");
        } else {
            body.push_str("<div class=\"surfdoc-bibliography-list\">");
            for (key, formatted) in &entries {
                body.push_str(&format!(
                    "<p id=\"ref-{}\" class=\"surfdoc-bibliography-entry\">{}</p>",
                    escape_html(key),
                    render_inline_markdown_phrasing(formatted)
                ));
            }
            body.push_str("</div>");
        }
        format!(
            "<section class=\"surfdoc-bibliography surfdoc-bib-{style_slug}\" aria-label=\"{heading}\"><h2 class=\"surfdoc-bibliography-heading\">{heading}</h2>{body}</section>"
        )
    })
}

/// Lowercase slug for a citation [`Format`] (used in CSS class names).
fn format_slug(f: Format) -> &'static str {
    match f {
        Format::Ieee => "ieee",
        Format::Acm => "acm",
        Format::Article => "article",
        Format::Mla => "mla",
        Format::Apa => "apa",
        Format::Chicago => "chicago",
    }
}

/// Render inline markdown (bold, italic, links, code) while escaping raw HTML.
///
/// Used for block content (callouts, decisions, etc.) where we want markdown
/// formatting but must prevent HTML injection. Escapes `<`, `>`, `&` first,
/// then runs through pulldown-cmark so `*italic*` and `**bold**` still render.
///
/// `«FILL: …»` / `«IMG: …»` generation slot markers (resolved later, on the
/// final page HTML, by [`crate::slots::resolve_slot_markers`]) are treated as
/// atomic: markdown syntax authors put inside a FILL default or IMG
/// description (e.g. `**bold**`) must NOT be parsed here. If it were,
/// pulldown-cmark would splice a `<strong>` tag into the middle of the
/// marker's own `«…»` body, and `resolve_slot_markers`'s grammar rejects any
/// marker containing raw `<`/`>` — so the whole marker leaks verbatim into
/// the served page instead of resolving to its default text.
fn render_inline_markdown(content: &str) -> String {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let escaped = escape_markdown_in_slot_markers(&escaped);
    render_markdown(&escaped)
}

/// Backslash-escape CommonMark special characters (`* _ ` ~ [ ] \`) that
/// appear inside `«…»` spans, so [`render_markdown`]'s pulldown-cmark pass
/// leaves them as literal text instead of parsing emphasis/code/link/
/// strikethrough syntax across the marker boundary. See
/// [`render_inline_markdown`] for why this matters.
fn escape_markdown_in_slot_markers(s: &str) -> String {
    if !s.contains('\u{ab}') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 16);
    let mut in_marker = false;
    for ch in s.chars() {
        match ch {
            '\u{ab}' => {
                in_marker = true;
                out.push(ch);
            }
            '\u{bb}' => {
                in_marker = false;
                out.push(ch);
            }
            '*' | '_' | '`' | '~' | '[' | ']' | '\\' if in_marker => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Render inline markdown WITHOUT a wrapping `<p>` element.
///
/// Use in phrasing contexts where a `<p>` is invalid (e.g. inside `<label>`
/// of a task list checkbox row). pulldown-cmark always wraps a single
/// paragraph of input in `<p>...</p>\n`; this strips that wrapper when
/// present and returns the inner phrasing content untouched otherwise.
fn render_inline_markdown_phrasing(content: &str) -> String {
    let html = render_inline_markdown(content);
    let trimmed = html.trim_end_matches('\n');
    if let Some(inner) = trimmed.strip_prefix("<p>").and_then(|s| s.strip_suffix("</p>")) {
        if !inner.contains("<p>") {
            return inner.to_string();
        }
    }
    html
}

/// A `<link rel="icon">` / apple-touch-icon entry for the page head.
#[derive(Debug, Clone)]
pub struct HeadIcon {
    /// `rel` value: `icon`, `apple-touch-icon`, `mask-icon`, …
    pub rel: String,
    /// Optional `type` (e.g. `image/png`).
    pub icon_type: Option<String>,
    /// Optional `sizes` (e.g. `16x16`, `any`).
    pub sizes: Option<String>,
    pub href: String,
}

/// A `<script src>` entry for the page head.
#[derive(Debug, Clone)]
pub struct HeadScript {
    pub src: String,
    /// Emit the `defer` attribute.
    pub defer: bool,
}

/// Reading-frame chrome for long documents (papers, legal, support): a sticky
/// toolbar with a back-link + title over a centered reading "sheet". Set on
/// [`PageConfig::reading_frame`] to wrap the shell page content.
#[derive(Debug, Clone, Default)]
pub struct ReadingFrame {
    /// Document title shown centered in the toolbar.
    pub title: Option<String>,
    /// Back-link target. Defaults to "/".
    pub back_href: Option<String>,
    /// Back-link label. Defaults to "Home".
    pub back_label: Option<String>,
}

/// Configuration for full-page HTML rendering with SurfDoc discovery metadata.
#[derive(Debug, Clone)]
pub struct PageConfig {
    /// Path to the original `.surf` source file (served alongside the built site).
    /// Used in `<link rel="alternate">` and the HTML comment.
    pub source_path: String,
    /// Page title. Falls back to front matter `title`, then "SurfDoc".
    pub title: Option<String>,
    /// Optional canonical URL for `<link rel="canonical">`.
    pub canonical_url: Option<String>,
    /// Optional meta description. Falls back to front matter `description`.
    pub description: Option<String>,
    /// Optional language code (default: "en").
    pub lang: Option<String>,
    /// Optional OG image URL for social sharing.
    pub og_image: Option<String>,
    /// Twitter card type: `summary` (default) or `summary_large_image`.
    pub twitter_card: Option<String>,
    /// Favicon / apple-touch-icon links, emitted in order.
    pub icons: Vec<HeadIcon>,
    /// External stylesheet hrefs, emitted in order (after the inlined base CSS
    /// when `embed_css` is true). Cache-bust by including `?v=N` in the href.
    pub stylesheets: Vec<String>,
    /// External scripts, emitted in `<head>` in order.
    pub scripts: Vec<HeadScript>,
    /// Emit the inline pre-paint theme resolver + `toggleTheme`/`toggleDrawer`/
    /// `closeDrawer` helpers in `<head>` (FOUC-safe; must run before paint).
    pub theme_init: bool,
    /// `localStorage` key for the persisted theme. Defaults to `theme`.
    pub theme_key: Option<String>,
    /// Inline the full SurfDoc CSS as a `<style>` block (default `true`). Set
    /// `false` when the consumer links the stylesheet itself (via `stylesheets`).
    pub embed_css: bool,
    /// Arbitrary extra markup injected at the end of `<head>`.
    pub head_extra: Option<String>,
    /// When set, [`to_shell_page`] wraps the page content in a reading-frame
    /// (sticky toolbar + centered sheet). `None` leaves pages unchanged.
    pub reading_frame: Option<ReadingFrame>,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            source_path: "source.surf".to_string(),
            title: None,
            canonical_url: None,
            description: None,
            lang: None,
            og_image: None,
            twitter_card: None,
            icons: Vec::new(),
            stylesheets: Vec::new(),
            scripts: Vec::new(),
            theme_init: false,
            theme_key: None,
            embed_css: true,
            head_extra: None,
            reading_frame: None,
        }
    }
}

impl PageConfig {
    /// Build a `PageConfig` whose head config is read from the document's
    /// `::site` block, so the page is self-describing. Recognised `::site`
    /// properties:
    ///
    /// - `description`, `og-image`, `twitter-card`, `lang`, `theme-key`
    /// - `canonical` / `url` (canonical URL)
    /// - `favicon` (rel=icon, sizes=any), `icon-16` / `icon-32` (png),
    ///   `apple-touch-icon`
    /// - `stylesheet` (repeatable), `script` / `script-defer` (repeatable)
    /// - `theme-init: true`, `embed-css: false`
    ///
    /// Per-page fields (`title`, `description`, `canonical_url`) can still be
    /// overridden by the caller after construction.
    pub fn from_site_doc(doc: &SurfDoc) -> Self {
        let mut cfg = PageConfig::default();
        let props = doc.blocks.iter().find_map(|b| match b {
            Block::Site { properties, .. } => Some(properties),
            _ => None,
        });
        let Some(props) = props else { return cfg };
        for p in props {
            let v = p.value.clone();
            match p.key.as_str() {
                "description" => cfg.description = Some(v),
                "og-image" | "og_image" => cfg.og_image = Some(v),
                "twitter-card" | "twitter_card" => cfg.twitter_card = Some(v),
                "lang" => cfg.lang = Some(v),
                "theme-key" | "theme_key" => cfg.theme_key = Some(v),
                "canonical" | "url" => cfg.canonical_url = Some(v),
                "favicon" => cfg.icons.push(HeadIcon {
                    rel: "icon".into(),
                    icon_type: None,
                    sizes: Some("any".into()),
                    href: v,
                }),
                "icon-16" => cfg.icons.push(HeadIcon {
                    rel: "icon".into(),
                    icon_type: Some("image/png".into()),
                    sizes: Some("16x16".into()),
                    href: v,
                }),
                "icon-32" => cfg.icons.push(HeadIcon {
                    rel: "icon".into(),
                    icon_type: Some("image/png".into()),
                    sizes: Some("32x32".into()),
                    href: v,
                }),
                "apple-touch-icon" => cfg.icons.push(HeadIcon {
                    rel: "apple-touch-icon".into(),
                    icon_type: None,
                    sizes: None,
                    href: v,
                }),
                "stylesheet" => cfg.stylesheets.push(v),
                "script" => cfg.scripts.push(HeadScript { src: v, defer: false }),
                "script-defer" => cfg.scripts.push(HeadScript { src: v, defer: true }),
                "theme-init" | "theme_init" => cfg.theme_init = v == "true" || v == "yes",
                "embed-css" | "embed_css" => cfg.embed_css = !(v == "false" || v == "no"),
                _ => {}
            }
        }
        cfg
    }
}

/// Render a `SurfDoc` as an HTML fragment.
///
/// The output is a sequence of semantic HTML elements with `surfdoc-*` CSS
/// classes. No `<html>`, `<head>`, or `<body>` wrapper is added.
/// If a `::site` block sets an accent color, a `<style>` override scoped to
/// `.surfdoc` is injected (not `:root`, to avoid leaking into editor chrome).
// Font presets, hex/luminance/contrast math, accent text + ink colors, and
// the style-pack token registry live in the shared resolve step — the single
// owner of token semantics for BOTH render targets (web CSS here, NativeTheme
// over FFI in render_native). Public re-exports preserve this module's
// historical API (`render_html::contrast_ratio`, `::accent_ink_color`).
pub use crate::resolve::{accent_ink_color, contrast_ratio};
use crate::resolve::{accent_text_color, resolve_font_preset};

/// Apply font/style properties from a `StyleProperty` list to CSS overrides.
/// Collects any required font imports into the `imports` set.
fn apply_style_overrides(properties: &[StyleProperty], css_overrides: &mut String, imports: &mut Vec<&'static str>) {
    for prop in properties {
        match prop.key.as_str() {
            "accent" => {
                let safe = sanitize_css_value(&prop.value);
                if !safe.is_empty() {
                    css_overrides.push_str(&format!("--accent: {};", safe));
                    // Compute ADA-compliant text color for accent backgrounds
                    let text = accent_text_color(&prop.value);
                    css_overrides.push_str(&format!("--accent-text: {};", text));
                }
            }
            "font" => {
                // Legacy: sets both heading and body
                if let Some(preset) = resolve_font_preset(&prop.value) {
                    css_overrides.push_str(&format!("--font-heading: {};", preset.stack));
                    css_overrides.push_str(&format!("--font-body: {};", preset.stack));
                    if let Some(url) = preset.import
                        && !imports.contains(&url) { imports.push(url); }
                }
            }
            "heading-font" => {
                if let Some(preset) = resolve_font_preset(&prop.value) {
                    css_overrides.push_str(&format!("--font-heading: {};", preset.stack));
                    if let Some(url) = preset.import
                        && !imports.contains(&url) { imports.push(url); }
                }
            }
            "body-font" => {
                if let Some(preset) = resolve_font_preset(&prop.value) {
                    css_overrides.push_str(&format!("--font-body: {};", preset.stack));
                    if let Some(url) = preset.import
                        && !imports.contains(&url) { imports.push(url); }
                }
            }
            _ => {}
        }
    }
}

/// Render the nav shell: a sticky topbar (hamburger + logo + theme toggle)
/// with a slide-in left drawer holding the author-supplied nav items.
///
/// The `<input class="surfdoc-nav-toggle">` is the first child of `.surfdoc-nav`
/// so the CSS `:checked ~ ...` sibling selectors drive the drawer/scrim.
fn render_nav_shell_html(items: &[crate::types::NavItem], effective_logo: Option<&str>) -> String {
    let mut html = String::from(
        "<nav class=\"surfdoc-nav\" role=\"navigation\" aria-label=\"Page navigation\">",
    );
    // Toggle checkbox MUST be the first child so sibling selectors work.
    html.push_str(
        "<input type=\"checkbox\" class=\"surfdoc-nav-toggle\" id=\"surfdoc-nav-toggle\" aria-hidden=\"true\">",
    );

    // Topbar: hamburger + logo on the left, theme toggle on the right.
    html.push_str("<div class=\"surfdoc-nav-topbar\"><div class=\"surfdoc-nav-topbar-left\">");
    html.push_str(
        "<label for=\"surfdoc-nav-toggle\" class=\"surfdoc-nav-hamburger\" aria-label=\"Toggle navigation\"><span></span><span></span><span></span></label>",
    );
    if let Some(logo_text) = effective_logo {
        let is_image = logo_text.ends_with(".svg")
            || logo_text.ends_with(".png")
            || logo_text.ends_with(".jpg")
            || logo_text.ends_with(".jpeg")
            || logo_text.ends_with(".webp")
            || logo_text.ends_with(".gif")
            || logo_text.starts_with("http://")
            || logo_text.starts_with("https://")
            || logo_text.starts_with("data:");
        if is_image {
            html.push_str(&format!(
                "<a class=\"surfdoc-nav-logo\" href=\"/\"><img src=\"{}\" alt=\"Logo\" class=\"surfdoc-nav-logo-img\" onerror=\"this.style.display='none';this.parentElement.insertAdjacentText('afterbegin','Surf');this.onerror=null\"></a>",
                escape_html(logo_text),
            ));
        } else {
            html.push_str(&format!(
                "<a class=\"surfdoc-nav-logo\" href=\"/\">{}</a>",
                escape_html(logo_text),
            ));
        }
    }
    html.push_str("</div><div class=\"surfdoc-nav-topbar-right\">");
    html.push_str(
        "<button type=\"button\" class=\"surfdoc-nav-theme-toggle\" aria-label=\"Toggle dark mode\" title=\"Toggle dark mode\" onclick=\"(function(d){var t=d.getAttribute('data-theme')==='dark'?'light':'dark';d.setAttribute('data-theme',t);try{localStorage.setItem('surfdoc-theme',t)}catch(e){}})(document.documentElement)\">",
    );
    html.push_str(
        "<svg class=\"surfdoc-nav-icon-moon\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z\"></path></svg>",
    );
    html.push_str(
        "<svg class=\"surfdoc-nav-icon-sun\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"5\"></circle><line x1=\"12\" y1=\"1\" x2=\"12\" y2=\"3\"></line><line x1=\"12\" y1=\"21\" x2=\"12\" y2=\"23\"></line><line x1=\"4.22\" y1=\"4.22\" x2=\"5.64\" y2=\"5.64\"></line><line x1=\"18.36\" y1=\"18.36\" x2=\"19.78\" y2=\"19.78\"></line><line x1=\"1\" y1=\"12\" x2=\"3\" y2=\"12\"></line><line x1=\"21\" y1=\"12\" x2=\"23\" y2=\"12\"></line><line x1=\"4.22\" y1=\"19.78\" x2=\"5.64\" y2=\"18.36\"></line><line x1=\"18.36\" y1=\"5.64\" x2=\"19.78\" y2=\"4.22\"></line></svg>",
    );
    html.push_str("</button></div></div>");

    // Drawer: one row per author-supplied nav item.
    html.push_str(
        "<aside class=\"surfdoc-nav-drawer\" aria-label=\"Navigation\"><nav class=\"surfdoc-nav-drawer-nav\">",
    );
    for item in items {
        let icon_html = item
            .icon
            .as_deref()
            .and_then(get_icon)
            .map(|svg| format!("<span class=\"surfdoc-icon\">{}</span>", svg))
            .unwrap_or_default();
        html.push_str(&format!(
            "<a href=\"{}\" class=\"surfdoc-nav-drawer-row\">{}<span class=\"surfdoc-nav-drawer-label\">{}</span></a>",
            escape_html(&item.href),
            icon_html,
            escape_html(&item.label),
        ));
    }
    html.push_str("</nav></aside>");

    // Scrim: clicking it (a label for the toggle) closes the drawer.
    html.push_str("<label for=\"surfdoc-nav-toggle\" class=\"surfdoc-nav-scrim\" aria-hidden=\"true\"></label>");

    // Apply persisted/preferred theme early and mark the active drawer row.
    html.push_str("<script>(function(){var d=document.documentElement;var s=null;try{s=localStorage.getItem('surfdoc-theme')}catch(e){}var t=s||((window.matchMedia&&matchMedia('(prefers-color-scheme: dark)').matches)?'dark':'light');d.setAttribute('data-theme',t);var p=location.pathname;document.querySelectorAll('.surfdoc-nav-drawer-row').forEach(function(a){var h=a.getAttribute('href');if(h&&(p===h||p.endsWith(h))){a.classList.add('is-active');a.setAttribute('aria-current','page');}});})();</script>");

    html.push_str("</nav>");
    html
}

// Rich-shell inline SVGs (mirror the cloudsurf.com chrome). The theme-toggle
// sun/moon carry `surfdoc-shell-icon-*` classes so CSS shows the right one per
// theme; the menu/close glyphs are plain.
const SHELL_ICON_MENU: &str = "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><line x1=\"3\" y1=\"6\" x2=\"21\" y2=\"6\"/><line x1=\"3\" y1=\"12\" x2=\"21\" y2=\"12\"/><line x1=\"3\" y1=\"18\" x2=\"21\" y2=\"18\"/></svg>";
const SHELL_ICON_CLOSE: &str = "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><line x1=\"18\" y1=\"6\" x2=\"6\" y2=\"18\"/><line x1=\"6\" y1=\"6\" x2=\"18\" y2=\"18\"/></svg>";
const SHELL_ICON_SUN: &str = "<svg class=\"surfdoc-shell-icon-sun\" width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"5\"/><line x1=\"12\" y1=\"1\" x2=\"12\" y2=\"3\"/><line x1=\"12\" y1=\"21\" x2=\"12\" y2=\"23\"/><line x1=\"4.22\" y1=\"4.22\" x2=\"5.64\" y2=\"5.64\"/><line x1=\"18.36\" y1=\"18.36\" x2=\"19.78\" y2=\"19.78\"/><line x1=\"1\" y1=\"12\" x2=\"3\" y2=\"12\"/><line x1=\"21\" y1=\"12\" x2=\"23\" y2=\"12\"/><line x1=\"4.22\" y1=\"19.78\" x2=\"5.64\" y2=\"18.36\"/><line x1=\"18.36\" y1=\"5.64\" x2=\"19.78\" y2=\"4.22\"/></svg>";
const SHELL_ICON_MOON: &str = "<svg class=\"surfdoc-shell-icon-moon\" width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z\"/></svg>";

/// True when a logo/emblem value points at an image (vs. plain wordmark text).
fn is_image_src(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.ends_with(".svg")
        || l.ends_with(".png")
        || l.ends_with(".jpg")
        || l.ends_with(".jpeg")
        || l.ends_with(".webp")
        || l.ends_with(".gif")
        || s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("data:")
        || s.starts_with('/')
}

/// Brand mark inner HTML: optional logo image + optional wordmark with an
/// optional `®` superscript. Shared by the nav topbar, drawer head, and footer.
fn shell_brand_inner(logo: Option<&str>, brand: Option<&str>, brand_reg: bool, emblem_class: &str) -> String {
    let mut s = String::new();
    if let Some(l) = logo
        && is_image_src(l)
    {
        s.push_str(&format!(
            "<img src=\"{}\" alt=\"\" width=\"34\" height=\"34\" class=\"{}\">",
            escape_html(l),
            emblem_class,
        ));
    }
    if let Some(b) = brand {
        let reg = if brand_reg { "<sup class=\"surfdoc-shell-reg\">&reg;</sup>" } else { "" };
        s.push_str(&format!("<span>{}{}</span>", escape_html(b), reg));
    }
    s
}

/// Render a single rich-shell drawer row: emblem/icon lead + label, with an
/// optional `target="_blank"` for external links.
fn render_shell_drawer_link(it: &NavItem) -> String {
    let tgt = if it.external { " target=\"_blank\" rel=\"noopener\"" } else { "" };
    let lead = if let Some(img) = &it.image {
        // Emit as a masked span (not an <img>) so the emblem tints to a single
        // color via currentColor and recolors with the row on hover, matching
        // the monochrome line icons. The asset path rides the --emblem property.
        format!(
            "<span class=\"surfdoc-shell-drawer-link-emblem\" style=\"--emblem:url('{}')\"></span>",
            escape_html(img),
        )
    } else if let Some(svg) = it.icon.as_deref().and_then(get_icon) {
        format!("<span class=\"surfdoc-shell-drawer-link-icon\">{}</span>", svg)
    } else {
        String::new()
    };
    format!(
        "<a href=\"{}\" class=\"surfdoc-shell-drawer-link\"{}>{}{}</a>",
        escape_html(&it.href),
        tgt,
        lead,
        escape_html(&it.label),
    )
}

/// Render the rich cloudsurf.com-quality nav shell: a sticky topbar (hamburger
/// + brand + per-page CTA + theme toggle), a JS-driven slide-in drawer with
/// grouped/iconned links + product emblems, and a scrim.
///
/// The toggle/drawer controls call `toggleTheme()` / `toggleDrawer()` /
/// `closeDrawer()`, which [`to_html_page`] emits in `<head>` when
/// `PageConfig::theme_init` is set — so this shell is meant to be served via the
/// full-page renderer (not the bare `to_html` fragment).
fn render_shell_nav_html(
    items: &[NavItem],
    groups: &[NavGroup],
    logo: Option<&str>,
    brand: Option<&str>,
    brand_reg: bool,
    cta: Option<&NavItem>,
) -> String {
    let brand_top = shell_brand_inner(logo, brand, brand_reg, "surfdoc-shell-emblem");
    let brand_drawer = shell_brand_inner(logo, brand, brand_reg, "surfdoc-shell-drawer-emblem");

    let cta_html = cta
        .map(|c| {
            let tgt = if c.external { " target=\"_blank\" rel=\"noopener\"" } else { "" };
            format!(
                "<div class=\"surfdoc-shell-nav-links\"><a href=\"{}\" class=\"surfdoc-shell-nav-cta\"{}>{}</a></div>",
                escape_html(&c.href),
                tgt,
                escape_html(&c.label),
            )
        })
        .unwrap_or_default();

    // Flat items also surface as inline text links on the bar itself at larger
    // screen sizes (CSS hides `.surfdoc-shell-nav-inline` below 1024px); the
    // grouped/product links remain drawer-only.
    let mut inline_links = String::new();
    for it in items {
        let tgt = if it.external { " target=\"_blank\" rel=\"noopener\"" } else { "" };
        inline_links.push_str(&format!(
            "<a href=\"{}\" class=\"surfdoc-shell-nav-link\"{}>{}</a>",
            escape_html(&it.href),
            tgt,
            escape_html(&it.label),
        ));
    }
    let inline_nav = if inline_links.is_empty() {
        String::new()
    } else {
        format!(
            "<nav class=\"surfdoc-shell-nav-inline\" aria-label=\"Primary\">{}</nav>",
            inline_links,
        )
    };

    // Flat items render as an unlabelled lead group; named groups follow.
    let mut links = String::new();
    for it in items {
        links.push_str(&render_shell_drawer_link(it));
    }
    for g in groups {
        if let Some(label) = &g.label {
            links.push_str(&format!(
                "<span class=\"surfdoc-shell-drawer-label\">{}</span>",
                escape_html(label),
            ));
        }
        for it in &g.items {
            links.push_str(&render_shell_drawer_link(it));
        }
    }

    format!(
        "<nav class=\"surfdoc-shell-nav\" aria-label=\"Main navigation\">\
           <div class=\"surfdoc-shell-nav-inner\">\
             <button class=\"surfdoc-shell-menu-btn\" type=\"button\" aria-label=\"Open menu\" aria-controls=\"surfdoc-shell-drawer\" aria-expanded=\"false\" onclick=\"toggleDrawer()\">{menu}</button>\
             <a href=\"/\" class=\"surfdoc-shell-brand\">{brand_top}</a>\
             {inline_nav}\
             <div class=\"surfdoc-shell-nav-spacer\"></div>{cta}\
             <button class=\"surfdoc-shell-theme-toggle\" type=\"button\" onclick=\"toggleTheme()\" aria-label=\"Toggle theme\">{sun}{moon}</button>\
           </div>\
         </nav>\
         <div class=\"surfdoc-shell-scrim\" onclick=\"closeDrawer()\" aria-hidden=\"true\"></div>\
         <aside class=\"surfdoc-shell-drawer\" id=\"surfdoc-shell-drawer\" aria-label=\"Site menu\">\
           <div class=\"surfdoc-shell-drawer-inner\">\
             <div class=\"surfdoc-shell-drawer-head\">\
               <span class=\"surfdoc-shell-drawer-brand\">{brand_drawer}</span>\
               <button class=\"surfdoc-shell-drawer-close\" type=\"button\" aria-label=\"Close menu\" onclick=\"closeDrawer()\">{close}</button>\
             </div>\
             <nav class=\"surfdoc-shell-drawer-nav\" aria-label=\"Site sections\">{links}</nav>\
           </div>\
         </aside>\
         <script>(function(){{var p=location.pathname;document.querySelectorAll('.surfdoc-shell-nav-link,.surfdoc-shell-drawer-link').forEach(function(a){{var h=a.getAttribute('href');if(h&&(h===p||(h!=='/'&&p.indexOf(h)===0))){{a.classList.add('is-active');a.setAttribute('aria-current','page')}}}})}})();</script>",
        menu = SHELL_ICON_MENU,
        inline_nav = inline_nav,
        brand_top = brand_top,
        cta = cta_html,
        sun = SHELL_ICON_SUN,
        moon = SHELL_ICON_MOON,
        brand_drawer = brand_drawer,
        close = SHELL_ICON_CLOSE,
        links = links,
    )
}

/// Resolve a product-tile `bg` spec into an inline style + light-text flag.
///
/// Spec forms (5th pipe field on a `::product-grid[tiles]` row):
/// `image:<src>` | `color:<css-color>` | `gradient:<css-gradient>` |
/// `transparent`/absent (no style). A trailing ` dark` token marks the
/// background as dark → the tile renders light text. A trailing ` theme`
/// token makes the ink follow the site theme anyway — for translucent
/// (low-alpha) backgrounds where the page backdrop dominates and the
/// pinned-ink rationale doesn't apply.
fn product_tile_bg(bg: Option<&str>) -> (String, bool, bool, bool) {
    let Some(raw) = bg else {
        return (String::new(), false, false, false);
    };
    let mut spec = raw.trim();
    let mut dark = false;
    let mut bottom = false;
    let mut theme_ink = false;
    loop {
        if let Some(stripped) = spec.strip_suffix(" dark") {
            spec = stripped.trim();
            dark = true;
        } else if let Some(stripped) = spec.strip_suffix(" bottom") {
            // ` bottom` token: media/imagery on top, title/CTA at the bottom
            // (apple.com AirPods-tile shape). Default is copy on top.
            spec = stripped.trim();
            bottom = true;
        } else if let Some(stripped) = spec.strip_suffix(" theme") {
            spec = stripped.trim();
            theme_ink = true;
        } else {
            break;
        }
    }
    let style = if let Some(src) = spec.strip_prefix("image:") {
        // Rendered as a media area BELOW the tile copy (apple.com tile shape),
        // not as a cover background: the caller puts this on the media div.
        format!("--tile-image:url('{}')", escape_html(src.trim()))
    } else if let Some(color) = spec.strip_prefix("color:") {
        format!("background:{}", escape_html(color.trim()))
    } else if let Some(gradient) = spec.strip_prefix("gradient:") {
        format!("background:{}", escape_html(gradient.trim()))
    } else {
        // "transparent" (explicit), "surface" (opaque theme surface — the
        // caller styles it via the .surfdoc-pg-tile-surface class so the ink
        // keeps following the theme, unlike color:/gradient:), or an
        // unrecognized spec: no inline style.
        String::new()
    };
    (style, dark, bottom, theme_ink)
}

/// True when a `Block::Nav`'s fields request the rich shell layout.
fn nav_is_rich(groups: &[NavGroup], brand: &Option<String>, drawer: bool) -> bool {
    drawer || !groups.is_empty() || brand.is_some()
}

/// Render a stripped topbar: brand/logo + theme toggle only. No hamburger menu,
/// drawer, or scrim. Reuses the shell-nav chrome classes (and CSS var-hooks) so
/// it sits inline with the rich shell visually.
fn render_minimal_nav_html(logo: Option<&str>, brand: Option<&str>, brand_reg: bool) -> String {
    let brand_top = shell_brand_inner(logo, brand, brand_reg, "surfdoc-shell-emblem");
    format!(
        "<nav class=\"surfdoc-shell-nav surfdoc-shell-nav-minimal\" aria-label=\"Main navigation\">\
           <div class=\"surfdoc-shell-nav-inner\">\
             <a href=\"/\" class=\"surfdoc-shell-brand\">{brand_top}</a>\
             <div class=\"surfdoc-shell-nav-spacer\"></div>\
             <button class=\"surfdoc-shell-theme-toggle\" type=\"button\" onclick=\"toggleTheme()\" aria-label=\"Toggle theme\">{sun}{moon}</button>\
           </div>\
         </nav>",
        brand_top = brand_top,
        sun = SHELL_ICON_SUN,
        moon = SHELL_ICON_MOON,
    )
}

/// Convert heading text to a URL anchor slug: lowercase, runs of non-alphanumeric
/// characters collapsed to a single `-`, no leading/trailing `-`.
fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Strip HTML tags from a fragment, leaving the text content (for slug + label).
fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Split an explicit `{#slug}` anchor suffix off a heading's inner HTML.
///
/// `Our Menu {#our-menu}` → `("Our Menu", "our-menu")`. Slug charset is
/// `[A-Za-z0-9_-]+`; anything else (prose braces, template syntax) is left
/// untouched. Returns `None` when no valid trailing anchor exists.
fn split_explicit_anchor(inner: &str) -> Option<(&str, &str)> {
    let trimmed = inner.trim_end();
    if !trimmed.ends_with('}') {
        return None;
    }
    let open = trimmed.rfind("{#")?;
    let slug = &trimmed[open + 2..trimmed.len() - 1];
    if slug.is_empty()
        || !slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    Some((trimmed[..open].trim_end(), slug))
}

/// Document-level post-pass: give every prose (class-less) `<hN>` heading a unique
/// anchor `id`, then fill any empty `<nav class="surfdoc-toc" data-depth="N">` with
/// links to those headings (filtered to level ≤ N). Block-internal headings carry a
/// class (e.g. `surfdoc-hero-headline`) and are left untouched, so they stay out of
/// the table of contents. Slug de-duplication is global and runs in document order.
fn wire_headings_and_toc(html: &str) -> String {
    // Pass 1 — collect class-less headings and inject `id` anchors.
    let mut result = String::with_capacity(html.len() + 256);
    let mut headings: Vec<(u32, String, String)> = Vec::new(); // (level, slug, text)
    let mut slug_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        result.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let b = after.as_bytes();
        let is_heading = b.len() >= 4 && b[1] == b'h' && b[2].is_ascii_digit() && b[3] == b'>';
        let level = if is_heading { (b[2] - b'0') as u32 } else { 0 };
        if is_heading && (1..=6).contains(&level) {
            let close = format!("</h{level}>");
            if let Some(close_rel) = after[4..].find(&close) {
                let raw_inner = &after[4..4 + close_rel];
                // An explicit `{#slug}` anchor at the end of the heading text
                // (`## Our Menu {#our-menu}`) becomes the heading id instead
                // of leaking as visible copy.
                let (inner, explicit) = match split_explicit_anchor(raw_inner) {
                    Some((clean, slug)) => (clean, Some(slug)),
                    None => (raw_inner, None),
                };
                let text = strip_tags(inner).trim().to_string();
                let base = match explicit {
                    Some(slug) => slug.to_string(),
                    None => slugify(&text),
                };
                let slug = if base.is_empty() {
                    format!("section-{}", headings.len() + 1)
                } else {
                    let n = slug_counts.entry(base.clone()).or_insert(0);
                    *n += 1;
                    if *n == 1 {
                        base.clone()
                    } else {
                        format!("{base}-{n}")
                    }
                };
                result.push_str(&format!("<h{level} id=\"{slug}\">{inner}{close}"));
                headings.push((level, slug, text));
                rest = &after[4 + close_rel + close.len()..];
                continue;
            }
        }
        // Not a class-less heading — copy the '<' and keep scanning.
        result.push('<');
        rest = &after[1..];
    }
    result.push_str(rest);

    // Pass 2 — fill empty TOC navs from the collected headings.
    let needle = "<nav class=\"surfdoc-toc\" data-depth=\"";
    if !result.contains(needle) {
        return result;
    }
    let mut out = String::with_capacity(result.len() + 256);
    let mut rest = result.as_str();
    while let Some(pos) = rest.find(needle) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let depth: u32 = after[needle.len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(3);
        match after.find("</nav>") {
            Some(close_rel) => {
                let nav_end = close_rel + "</nav>".len();
                let items: Vec<&(u32, String, String)> =
                    headings.iter().filter(|(lvl, _, _)| *lvl <= depth).collect();
                if items.is_empty() {
                    out.push_str(&after[..nav_end]);
                } else {
                    out.push_str(&format!(
                        "<nav class=\"surfdoc-toc\" data-depth=\"{depth}\"><div class=\"surfdoc-toc-label\">Contents</div>"
                    ));
                    out.push_str(&toc_nested_ol(&items));
                    out.push_str("</nav>");
                }
                rest = &after[nav_end..];
            }
            None => {
                out.push_str(after);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Build a hierarchically nested `<ol>` from a flat list of (level, slug, text) tuples.
///
/// Sub-headings are wrapped in nested `<ol>` elements so CSS `list-style: decimal`
/// produces hierarchical numbers (1, 2, 2.1, 3, …) natively. The algorithm
/// is a depth-tracking state machine:
///
/// - `stack` tracks the heading levels of currently-open `<ol>` containers.
/// - Each new item opens a nested `<ol>` when deeper than the current level,
///   closes `</ol></li>` pairs when shallower, and closes `</li>` for siblings.
#[allow(unused_assignments)]
fn toc_nested_ol(items: &[&(u32, String, String)]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut html = String::new();
    // stack[i] = heading level of the i-th open <ol>
    let mut stack: Vec<u32> = Vec::new();
    // whether the current top-of-stack <ol> has an open (unclosed) <li>
    let mut li_open = false;

    for (lvl, slug, text) in items {
        let lvl = *lvl;

        if stack.is_empty() {
            // First item: open the root <ol>.
            html.push_str("<ol>");
            stack.push(lvl);
        } else {
            let cur = *stack.last().unwrap();
            if lvl > cur {
                // Going deeper: open a nested <ol> inside the current (open) <li>.
                html.push_str("<ol>");
                stack.push(lvl);
                li_open = false;
            } else if lvl == cur {
                // Sibling: close the previous <li>.
                if li_open {
                    html.push_str("</li>");
                    li_open = false;
                }
            } else {
                // Going up: close levels until we reach the right one.
                while stack.len() > 1 && *stack.last().unwrap() > lvl {
                    if li_open {
                        html.push_str("</li>");
                        li_open = false;
                    }
                    html.push_str("</ol>");
                    stack.pop();
                    // After closing a nested <ol>, we're back inside the parent <li>
                    // which was left open (without </li>) when we went deeper.
                    li_open = true;
                }
                // Now close the parent <li> that was holding the sub-list.
                if li_open {
                    html.push_str("</li>");
                    li_open = false;
                }
            }
        }

        // Emit the new <li> (left open — will be closed on next iteration or teardown).
        html.push_str(&format!(
            "<li><a href=\"#{}\">{}</a>",
            escape_html(slug),
            escape_html(text)
        ));
        li_open = true;
    }

    // Teardown: close remaining open levels.
    while !stack.is_empty() {
        if li_open {
            html.push_str("</li>");
            li_open = false;
        }
        html.push_str("</ol>");
        stack.pop();
        // After closing a nested <ol> we're back in the parent <li>.
        if !stack.is_empty() {
            li_open = true;
        }
    }

    html
}

pub fn to_html(doc: &SurfDoc) -> String {
    // Install the citation context so inline `[@key]` cites + `::bibliography`
    // resolve during this render (cleared on drop).
    let _cite_scope = citation::install_context(citation::build_context(
        &doc.blocks,
        doc.front_matter.as_ref().and_then(|fm| fm.format),
    ));

    let mut parts: Vec<String> = Vec::new();
    let mut css_overrides = String::new();
    let mut font_imports: Vec<&'static str> = Vec::new();

    // Scan for CSS variable overrides from ::site and ::style blocks.
    for block in &doc.blocks {
        match block {
            Block::Site { properties, .. } => apply_style_overrides(properties, &mut css_overrides, &mut font_imports),
            Block::Style { properties, .. } => apply_style_overrides(properties, &mut css_overrides, &mut font_imports),
            _ => {}
        }
    }

    // Emit @import rules for Google Fonts (must come before other styles)
    for url in &font_imports {
        parts.push(format!("<style>@import url('{}');</style>", url));
    }

    if !css_overrides.is_empty() {
        // Scope overrides to .surfdoc (not :root) so accent colors don't leak
        // into the parent page when rendered as a fragment inside the editor.
        parts.push(format!("<style>.surfdoc {{ {} }}</style>", css_overrides));
    }

    // Extract site name for nav logo fallback
    let site_name: Option<String> = doc.blocks.iter().find_map(|b| {
        if let Block::Site { properties, .. } = b {
            properties.iter().find(|p| p.key == "name").map(|p| p.value.clone())
        } else {
            None
        }
    });

    // Render nav blocks first (before section wrapping)
    for block in &doc.blocks {
        if let Block::Nav { items, logo, groups, brand, brand_reg, cta, drawer, minimal, .. } = block {
            if *minimal {
                parts.push(render_minimal_nav_html(logo.as_deref(), brand.as_deref(), *brand_reg));
            } else if nav_is_rich(groups, brand, *drawer) {
                parts.push(render_shell_nav_html(items, groups, logo.as_deref(), brand.as_deref(), *brand_reg, cta.as_ref()));
            } else {
                // Use explicit logo, fall back to ::site name
                let effective_logo = logo.as_deref().or(site_name.as_deref());
                parts.push(render_nav_shell_html(items, effective_logo));
            }
        }
    }

    let mut in_section = false;
    let mut cta_group: Vec<String> = Vec::new();

    for block in &doc.blocks {
        // Skip nav blocks — already rendered above
        if matches!(block, Block::Nav { .. }) {
            continue;
        }

        // Group consecutive CTA blocks into a centered wrapper
        if matches!(block, Block::Cta { .. }) {
            cta_group.push(render_block(block));
            continue;
        }
        if !cta_group.is_empty() {
            parts.push(format!("<div class=\"surfdoc-cta-group\">{}</div>", cta_group.join("\n")));
            cta_group.clear();
        }

        let rendered = render_block(block);

        // Detect section boundaries: h1 or h2 starts a new visual section
        let starts_section = rendered.starts_with("<h1>") || rendered.starts_with("<h2>");
        if starts_section {
            if in_section {
                parts.push("</section>".to_string());
            }
            parts.push("<section class=\"surfdoc-section\">".to_string());
            in_section = true;
        }

        parts.push(rendered);
    }

    // Flush any trailing CTA group
    if !cta_group.is_empty() {
        parts.push(format!("<div class=\"surfdoc-cta-group\">{}</div>", cta_group.join("\n")));
    }

    if in_section {
        parts.push("</section>".to_string());
    }

    wire_headings_and_toc(&parts.join("\n"))
}

/// Render a slice of blocks as bare HTML fragments.
///
/// Unlike [`to_html()`], this does NOT:
/// - Scan for `::site`/`::style` blocks or emit CSS variable overrides
/// - Extract or render navigation blocks separately
/// - Wrap content in auto-sectioning (h1/h2 boundary detection)
///
/// Each block is rendered through [`render_block()`] and the results are joined
/// with newlines. The caller controls the CSS context — fragment HTML assumes
/// `surfdoc-*` class names are available from a parent stylesheet.
///
/// Returns an empty string for an empty slice.
pub fn to_html_fragment(blocks: &[Block]) -> String {
    if blocks.is_empty() {
        return String::new();
    }
    // Resolve cites within the fragment using the fragment's own ::cite blocks
    // (default APA style — fragments carry no front matter).
    let _cite_scope = citation::install_context(citation::build_context(blocks, None));
    let mut parts: Vec<String> = Vec::new();
    let mut cta_group: Vec<String> = Vec::new();
    for block in blocks {
        if matches!(block, Block::Cta { .. }) {
            cta_group.push(render_block(block));
            continue;
        }
        if !cta_group.is_empty() {
            parts.push(format!("<div class=\"surfdoc-cta-group\">{}</div>", cta_group.join("\n")));
            cta_group.clear();
        }
        parts.push(render_block(block));
    }
    if !cta_group.is_empty() {
        parts.push(format!("<div class=\"surfdoc-cta-group\">{}</div>", cta_group.join("\n")));
    }
    wire_headings_and_toc(&parts.join("\n"))
}

/// Render a `SurfDoc` as a complete HTML page with SurfDoc discovery metadata.
///
/// Produces a full `<!DOCTYPE html>` document with:
/// - `<meta name="generator" content="SurfDoc v0.1">`
/// - `<link rel="alternate" type="text/surfdoc" href="...">` pointing to source
/// - HTML comment identifying the source file
/// - Standard viewport and charset meta tags
/// - Embedded dark-theme CSS for all SurfDoc block types
pub fn to_html_page(doc: &SurfDoc, config: &PageConfig) -> String {
    let body = to_html(doc);
    let lang = config.lang.as_deref().unwrap_or("en");

    // Resolve title: explicit config > front matter > fallback
    let title = config
        .title
        .clone()
        .or_else(|| {
            doc.front_matter
                .as_ref()
                .and_then(|fm| fm.title.clone())
        })
        .unwrap_or_else(|| "SurfDoc".to_string());

    // Resolve description: explicit config > front matter > ::site property
    let description = config
        .description
        .clone()
        .or_else(|| {
            doc.front_matter
                .as_ref()
                .and_then(|fm| fm.description.clone())
        })
        .or_else(|| {
            doc.blocks.iter().find_map(|b| {
                if let Block::Site { properties, .. } = b {
                    properties.iter().find(|p| p.key == "description").map(|p| p.value.clone())
                } else {
                    None
                }
            })
        });

    let source_path = escape_html(&config.source_path);
    let title_escaped = escape_html(&title);
    let head_tail = build_head_tail(&title_escaped, description.as_deref(), config);

    format!(
        r#"<!-- Built with SurfDoc — source: {source_path} -->
<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="generator" content="SurfDoc v0.1">
    <link rel="alternate" type="text/surfdoc" href="{source_path}">
    <title>{title}</title>{head_tail}
</head>
<body>
<article class="surfdoc">
{body}
</article>
</body>
</html>"#,
        source_path = source_path,
        lang = escape_html(lang),
        title = title_escaped,
        head_tail = head_tail,
        body = body,
    )
}

/// Build the `<head>` tail — everything after `<title>…</title>` up to (not
/// including) `</head>`: meta/OG/Twitter, favicons, stylesheets, FOUC theme
/// init, scripts, and `head_extra`. Shared by [`to_html_page`] and
/// [`to_shell_page`]. `title_escaped` must already be HTML-escaped.
fn build_head_tail(title_escaped: &str, description: Option<&str>, config: &PageConfig) -> String {
    let mut meta_extra = String::new();
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(url) = &config.canonical_url {
        let url_escaped = escape_html(url);
        meta_extra.push_str(&format!("\n    <link rel=\"canonical\" href=\"{}\">", url_escaped));
        meta_extra.push_str(&format!("\n    <meta property=\"og:url\" content=\"{}\">", url_escaped));
    }
    meta_extra.push_str(&format!("\n    <meta property=\"og:title\" content=\"{}\">", title_escaped));
    meta_extra.push_str("\n    <meta property=\"og:type\" content=\"website\">");
    if let Some(desc) = description {
        meta_extra.push_str(&format!("\n    <meta property=\"og:description\" content=\"{}\">", escape_html(desc)));
    }
    if let Some(img) = &config.og_image {
        meta_extra.push_str(&format!("\n    <meta property=\"og:image\" content=\"{}\">", escape_html(img)));
    }
    let twitter_card = config.twitter_card.as_deref().unwrap_or("summary");
    meta_extra.push_str(&format!("\n    <meta name=\"twitter:card\" content=\"{}\">", escape_html(twitter_card)));
    meta_extra.push_str(&format!("\n    <meta name=\"twitter:title\" content=\"{}\">", title_escaped));
    if let Some(desc) = description {
        meta_extra.push_str(&format!("\n    <meta name=\"twitter:description\" content=\"{}\">", escape_html(desc)));
    }
    if let Some(img) = &config.og_image {
        meta_extra.push_str(&format!("\n    <meta name=\"twitter:image\" content=\"{}\">", escape_html(img)));
    }

    // Favicons / apple-touch-icon.
    for icon in &config.icons {
        let type_attr = icon
            .icon_type
            .as_deref()
            .map(|t| format!(" type=\"{}\"", escape_html(t)))
            .unwrap_or_default();
        let sizes_attr = icon
            .sizes
            .as_deref()
            .map(|s| format!(" sizes=\"{}\"", escape_html(s)))
            .unwrap_or_default();
        meta_extra.push_str(&format!(
            "\n    <link rel=\"{}\"{}{} href=\"{}\">",
            escape_html(&icon.rel),
            type_attr,
            sizes_attr,
            escape_html(&icon.href),
        ));
    }

    // CSS: inline the base stylesheet (default) and/or link external sheets.
    let mut style_block = String::new();
    if config.embed_css {
        style_block.push_str(&format!("\n    <style>{}</style>", SURFDOC_CSS));
    }
    for href in &config.stylesheets {
        style_block.push_str(&format!("\n    <link rel=\"stylesheet\" href=\"{}\">", escape_html(href)));
    }

    // FOUC-safe theme init + drawer/toggle helpers (must run before paint, so
    // emitted inline and NOT deferred). The rich shell nav buttons call these.
    let theme_block = if config.theme_init {
        let key = sanitize_js_key(config.theme_key.as_deref().unwrap_or("theme"));
        format!(
            "\n    <script>\
             (function(){{var k='{key}';var t=null;try{{t=localStorage.getItem(k)}}catch(e){{}}\
             if(!t)t=(window.matchMedia&&matchMedia('(prefers-color-scheme:dark)').matches)?'dark':'light';\
             document.documentElement.setAttribute('data-theme',t)}})();\
             function toggleTheme(){{var d=document.documentElement,k='{key}',t=d.getAttribute('data-theme')==='dark'?'light':'dark';d.setAttribute('data-theme',t);try{{localStorage.setItem(k,t)}}catch(e){{}}}}\
             function toggleDrawer(){{var o=document.documentElement.classList.toggle('drawer-open');var b=document.querySelector('.surfdoc-shell-menu-btn');if(b)b.setAttribute('aria-expanded',o)}}\
             function closeDrawer(){{document.documentElement.classList.remove('drawer-open');var b=document.querySelector('.surfdoc-shell-menu-btn');if(b)b.setAttribute('aria-expanded','false')}}\
             document.addEventListener('keydown',function(e){{if(e.key==='Escape')closeDrawer()}});\
             document.addEventListener('DOMContentLoaded',function(){{\
             if(window.matchMedia&&matchMedia('(prefers-reduced-motion: reduce)').matches)return;\
             var pending=Array.prototype.slice.call(document.querySelectorAll('main.surfdoc .surfdoc-banner,main.surfdoc .surfdoc-product-grid:not(.surfdoc-pg-tiles-band),main.surfdoc .surfdoc-pg-tile,main.surfdoc .surfdoc-features,main.surfdoc .surfdoc-section,main.surfdoc .surfdoc-post-grid,main.surfdoc .surfdoc-reveal-target'));\
             if(!pending.length)return;\
             pending.forEach(function(el){{el.classList.add('surfdoc-reveal')}});\
             var t=null;\
             function sweep(){{t=null;var vh=window.innerHeight;\
             pending=pending.filter(function(el){{var r=el.getBoundingClientRect();\
             if(r.top<vh*0.92||r.bottom<0){{el.classList.add('surfdoc-reveal-in');return false}}return true}});\
             if(!pending.length){{removeEventListener('scroll',onmove);removeEventListener('resize',onmove)}}}}\
             function onmove(){{if(t===null)t=requestAnimationFrame(sweep)}}\
             addEventListener('scroll',onmove,{{passive:true}});\
             addEventListener('resize',onmove);\
             sweep()}});\
             </script>"
        )
    } else {
        String::new()
    };

    // External scripts (htmx / analytics / custom).
    let mut script_block = String::new();
    for s in &config.scripts {
        let defer = if s.defer { " defer" } else { "" };
        script_block.push_str(&format!("\n    <script src=\"{}\"{}></script>", escape_html(&s.src), defer));
    }

    let head_extra = config
        .head_extra
        .as_deref()
        .map(|h| format!("\n    {}", h))
        .unwrap_or_default();

    format!("{meta_extra}{style_block}{theme_block}{script_block}{head_extra}")
}

/// Render a full HTML page from a **site shell** plus pre-rendered body HTML.
///
/// This is the site-shell entrypoint for consumers (e.g. cloudsurf.com) that
/// keep a shared `::site`/`::nav`/`::footer` shell in one `.surf` file and feed
/// per-route content as already-rendered HTML. The `<head>` is built from
/// `config` (favicons, stylesheets, scripts, FOUC theme init, OG/Twitter); the
/// `Block::Nav` blocks in `shell` render before `<main>` (rich shell when their
/// fields request it) and the `Block::Footer` blocks render after it.
/// `body_html` is inserted verbatim inside `<main class="surfdoc">`.
pub fn to_shell_page(shell: &SurfDoc, body_html: &str, config: &PageConfig) -> String {
    let lang = config.lang.as_deref().unwrap_or("en");
    let title = config
        .title
        .clone()
        .or_else(|| shell.front_matter.as_ref().and_then(|fm| fm.title.clone()))
        .unwrap_or_else(|| "SurfDoc".to_string());
    let description = config.description.clone().or_else(|| {
        shell.blocks.iter().find_map(|b| {
            if let Block::Site { properties, .. } = b {
                properties.iter().find(|p| p.key == "description").map(|p| p.value.clone())
            } else {
                None
            }
        })
    });
    let title_escaped = escape_html(&title);
    let head_tail = build_head_tail(&title_escaped, description.as_deref(), config);

    // Site-level CSS variable overrides (accent/font) from ::site / ::style.
    let mut css_overrides = String::new();
    let mut font_imports: Vec<&'static str> = Vec::new();
    let mut font_import_tags = String::new();
    for block in &shell.blocks {
        match block {
            Block::Site { properties, .. } | Block::Style { properties, .. } => {
                apply_style_overrides(properties, &mut css_overrides, &mut font_imports)
            }
            _ => {}
        }
    }
    for url in &font_imports {
        font_import_tags.push_str(&format!("<style>@import url('{}');</style>\n", url));
    }
    let override_tag = if css_overrides.is_empty() {
        String::new()
    } else {
        format!("<style>.surfdoc {{ {} }}</style>\n", css_overrides)
    };

    let nav_html: String = shell
        .blocks
        .iter()
        .filter(|b| matches!(b, Block::Nav { .. }))
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n");
    let footer_html: String = shell
        .blocks
        .iter()
        .filter(|b| matches!(b, Block::Footer { .. }))
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n");

    // Reading-frame: wrap the body in a sticky toolbar + centered sheet.
    let body_wrapped = match &config.reading_frame {
        Some(rf) => {
            let back_href = rf.back_href.as_deref().unwrap_or("/");
            let back_label = rf.back_label.as_deref().unwrap_or("Home");
            let title_html = rf
                .title
                .as_deref()
                .map(escape_html)
                .unwrap_or_default();
            // Title + balancing spacer render only when a title is supplied;
            // otherwise the toolbar is a back button only (a clean floating pill).
            let title_markup = if title_html.is_empty() {
                String::new()
            } else {
                format!(
                    "<span class=\"surfdoc-doc-title\">{title}</span>\
                     <span class=\"surfdoc-doc-toolbar-spacer\"></span>",
                    title = title_html,
                )
            };
            format!(
                "<div class=\"surfdoc-doc-viewer\">\
                   <div class=\"surfdoc-doc-toolbar\"><div class=\"surfdoc-doc-toolbar-inner\">\
                     <a class=\"surfdoc-doc-back\" href=\"{back_href}\">&lsaquo; {back_label}</a>\
                     {title_markup}\
                   </div></div>\
                   <div class=\"surfdoc-doc-stage\">\
                     <article class=\"surfdoc-doc-page surfdoc\">{content}</article>\
                   </div>\
                 </div>",
                back_href = escape_html(back_href),
                back_label = escape_html(back_label),
                title_markup = title_markup,
                content = body_html,
            )
        }
        None => body_html.to_string(),
    };

    // The reading frame hosts its own centered sheet; give <main> a class that
    // lifts the base `.surfdoc` document clamp (max-width/padding) so the
    // toolbar + stage run full-bleed. Plain pages keep the clamp.
    let main_class = if config.reading_frame.is_some() {
        "surfdoc surfdoc-doc-main"
    } else {
        "surfdoc"
    };

    let source_path = escape_html(&config.source_path);
    format!(
        r#"<!-- Built with SurfDoc — source: {source_path} -->
<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="generator" content="SurfDoc v0.1">
    <title>{title}</title>{head_tail}
</head>
<body>
{font_imports}{override_tag}{nav}
<main class="{main_class}">
{body}
</main>
{footer}
</body>
</html>"#,
        source_path = source_path,
        lang = escape_html(lang),
        title = title_escaped,
        head_tail = head_tail,
        font_imports = font_import_tags,
        override_tag = override_tag,
        nav = nav_html,
        main_class = main_class,
        body = body_wrapped,
        footer = footer_html,
    )
}

// SURFDOC_CSS is now a public constant in lib.rs via include_str!("../assets/surfdoc.css").
// It's referenced here as crate::SURFDOC_CSS.
use crate::SURFDOC_CSS;

// The old inline CSS has been moved to assets/surfdoc.css and is loaded via include_str! in lib.rs.

/// Escape HTML special characters to prevent XSS.
pub(crate) fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render inline markdown (links, bold, italic) within a table cell value.
///
/// Handles:
/// - `[text](url)` → `<a>` tags (relative URLs get `/wiki/` prefix, absolute are kept as-is)
/// - `**text**` → `<strong>`
/// - `*text*` → `<em>`
///
/// All non-markdown text is HTML-escaped to prevent XSS.
fn render_cell_inline_markdown(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // `«FILL: …»` / `«IMG: …»` generation slot markers are atomic: they
        // are resolved later, on the final page HTML, by
        // `crate::slots::resolve_slot_markers`. Markdown syntax inside a FILL
        // default / IMG description (e.g. `**bold**`, `[text](url)`) must not
        // be parsed here — splicing a tag into the marker body would break
        // its `«…»` grammar and leak the raw marker into the page.
        if chars[i] == '\u{ab}' {
            if let Some(close) = (i + 1..len).find(|&j| chars[j] == '\u{bb}') {
                for &c in &chars[i..=close] {
                    match c {
                        '&' => result.push_str("&amp;"),
                        '<' => result.push_str("&lt;"),
                        '>' => result.push_str("&gt;"),
                        '"' => result.push_str("&quot;"),
                        c => result.push(c),
                    }
                }
                i = close + 1;
                continue;
            }
        }

        // Check for markdown link: [text](url)
        if chars[i] == '[' {
            if let Some((link_html, advance)) = try_parse_link(&chars, i) {
                result.push_str(&link_html);
                i += advance;
                continue;
            }
        }

        // Check for bold: **text**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some((bold_html, advance)) = try_parse_bold(&chars, i) {
                result.push_str(&bold_html);
                i += advance;
                continue;
            }
        }

        // Check for italic: *text* (but not **)
        if chars[i] == '*' && (i + 1 >= len || chars[i + 1] != '*') {
            if let Some((em_html, advance)) = try_parse_italic(&chars, i) {
                result.push_str(&em_html);
                i += advance;
                continue;
            }
        }

        // Plain character — escape HTML
        match chars[i] {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            c => result.push(c),
        }
        i += 1;
    }

    result
}

/// Try to parse a markdown link `[text](url)` starting at position `pos`.
/// Returns the rendered HTML and the number of characters consumed, or `None`.
fn try_parse_link(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let len = chars.len();
    debug_assert!(chars[pos] == '[');

    // Find closing ]
    let mut j = pos + 1;
    let mut depth = 1;
    while j < len && depth > 0 {
        if chars[j] == '[' { depth += 1; }
        if chars[j] == ']' { depth -= 1; }
        j += 1;
    }
    if depth != 0 { return None; }
    // j is now past the ']'
    let close_bracket = j - 1;

    // Must be followed by '('
    if j >= len || chars[j] != '(' { return None; }
    let paren_start = j + 1;

    // Find closing )
    let mut k = paren_start;
    let mut paren_depth = 1;
    while k < len && paren_depth > 0 {
        if chars[k] == '(' { paren_depth += 1; }
        if chars[k] == ')' { paren_depth -= 1; }
        k += 1;
    }
    if paren_depth != 0 { return None; }
    let close_paren = k - 1;

    let text: String = chars[pos + 1..close_bracket].iter().collect();
    let url: String = chars[paren_start..close_paren].iter().collect();

    if text.is_empty() || url.is_empty() { return None; }

    // Determine href: absolute URLs stay as-is, relative get /wiki/ prefix
    let href = if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") || url.starts_with('/') {
        escape_html(&url)
    } else {
        format!("/wiki/{}", escape_html(&url))
    };

    let html = format!("<a href=\"{}\">{}</a>", href, escape_html(&text));
    Some((html, k - pos))
}

/// Try to parse `**text**` starting at position `pos`.
/// Returns the rendered HTML and the number of characters consumed, or `None`.
/// Inner content is processed for inline markdown (links, etc.).
fn try_parse_bold(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let len = chars.len();
    debug_assert!(pos + 1 < len && chars[pos] == '*' && chars[pos + 1] == '*');

    let start = pos + 2;
    let mut j = start;
    while j + 1 < len {
        if chars[j] == '*' && chars[j + 1] == '*' {
            if j == start { return None; } // empty bold
            let inner: String = chars[start..j].iter().collect();
            let html = format!("<strong>{}</strong>", render_cell_inline_markdown(&inner));
            return Some((html, j + 2 - pos));
        }
        j += 1;
    }
    None
}

/// Try to parse `*text*` starting at position `pos`.
/// Returns the rendered HTML and the number of characters consumed, or `None`.
/// Inner content is processed for inline markdown (links, etc.).
fn try_parse_italic(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let len = chars.len();
    debug_assert!(chars[pos] == '*');

    let start = pos + 1;
    let mut j = start;
    while j < len {
        if chars[j] == '*' && (j + 1 >= len || chars[j + 1] != '*') {
            if j == start { return None; } // empty italic
            let inner: String = chars[start..j].iter().collect();
            let html = format!("<em>{}</em>", render_cell_inline_markdown(&inner));
            return Some((html, j + 1 - pos));
        }
        // If we hit **, this is not a valid *italic* span
        if chars[j] == '*' && j + 1 < len && chars[j + 1] == '*' {
            return None;
        }
        j += 1;
    }
    None
}

/// Sanitize a value for use inside a CSS declaration.
///
/// Strips characters that could break out of a CSS property value context:
/// semicolons, braces, angle brackets, backslashes, and url()/expression().
fn sanitize_css_value(s: &str) -> String {
    let stripped: String = s.chars()
        .filter(|c| !matches!(c, ';' | '{' | '}' | '<' | '>' | '\\' | '"' | '\''))
        .collect();
    // Block CSS function injection (url(), expression(), etc.)
    let lower = stripped.to_lowercase();
    if lower.contains("url(") || lower.contains("expression(") || lower.contains("javascript:") {
        return String::new();
    }
    stripped
}

/// Self-contained client script for every `::booking` widget on a page.
///
/// Guarded by a window flag so it initialises all `[data-booking]` widgets
/// exactly once regardless of how many `::booking` blocks emit it. Reads each
/// widget's inlined JSON, builds the month calendar with the browser `Date`
/// API, and drives service → date → slot → confirmation entirely client-side
/// (static data-bound; no network). Hidden SPA sections still init fine.
const BOOKING_WIDGET_JS: &str = r#"<script>(function(){
if(window.__surfBookingInit)return;window.__surfBookingInit=1;
var MON=['January','February','March','April','May','June','July','August','September','October','November','December'];
function fmt(iso){var p=iso.split('-');var d=new Date(+p[0],+p[1]-1,+p[2]);return d.toLocaleDateString(undefined,{weekday:'long',month:'long',day:'numeric'});}
function init(root){
 var dataEl=root.querySelector('[data-bk-data]');if(!dataEl)return;
 var data={};try{data=JSON.parse(dataEl.textContent)}catch(e){return;}
 var days=data.days||[],byDate={},months=[],seen={};
 days.forEach(function(d){byDate[d.date]=d.slots||[];var m=d.date.slice(0,7);if(!seen[m]){seen[m]=1;months.push(m);}});
 months.sort();if(!months.length)return;
 var mi=0,selService=null,selDate=null,selSlot=null;
 var svcWrap=root.querySelector('[data-bk-services]');
 if(svcWrap&&data.services&&data.services.length){
  data.services.forEach(function(s,i){
   var b=document.createElement('button');b.type='button';b.className='surfdoc-booking-service';b.setAttribute('role','radio');b.setAttribute('aria-checked',i===0?'true':'false');
   var meta=[s.duration,s.price].filter(Boolean).join(' · ');
   var n=document.createElement('span');n.className='surfdoc-booking-service-name';n.textContent=s.name;b.appendChild(n);
   if(meta){var mEl=document.createElement('span');mEl.className='surfdoc-booking-service-meta';mEl.textContent=meta;b.appendChild(mEl);}
   b.addEventListener('click',function(){selService=s.name;svcWrap.querySelectorAll('.surfdoc-booking-service').forEach(function(x){x.classList.remove('is-sel');x.setAttribute('aria-checked','false');});b.classList.add('is-sel');b.setAttribute('aria-checked','true');});
   if(i===0){selService=s.name;b.classList.add('is-sel');}
   svcWrap.appendChild(b);
  });
 }
 var monthEl=root.querySelector('[data-bk-month]'),daysEl=root.querySelector('[data-bk-days]'),slotsEl=root.querySelector('[data-bk-slots]');
 var form=root.querySelector('[data-bk-form]'),summaryEl=root.querySelector('[data-bk-summary]'),confirmEl=root.querySelector('[data-bk-confirm]');
 var prev=root.querySelector('[data-bk-prev]'),next=root.querySelector('[data-bk-next]');
 function renderMonth(){
  var m=months[mi],y=+m.slice(0,4),mo=+m.slice(5,7)-1;
  monthEl.textContent=MON[mo]+' '+y;
  var first=new Date(y,mo,1).getDay(),dim=new Date(y,mo+1,0).getDate();
  daysEl.innerHTML='';
  for(var i=0;i<first;i++){var pad=document.createElement('span');pad.className='surfdoc-booking-day is-pad';daysEl.appendChild(pad);}
  for(var d=1;d<=dim;d++){
   var iso=m+'-'+(d<10?'0'+d:d),slots=byDate[iso],cell;
   if(slots&&slots.length){cell=document.createElement('button');cell.type='button';cell.className='surfdoc-booking-day is-open';(function(x){cell.addEventListener('click',function(){selectDate(x);});})(iso);}
   else{cell=document.createElement('span');cell.className='surfdoc-booking-day'+(byDate[iso]!==undefined?' is-full':' is-empty');}
   cell.textContent=d;if(iso===selDate)cell.classList.add('is-sel');daysEl.appendChild(cell);
  }
  if(prev)prev.disabled=mi<=0;if(next)next.disabled=mi>=months.length-1;
 }
 function selectDate(iso){selDate=iso;selSlot=null;renderMonth();renderSlots();if(form)form.hidden=true;if(confirmEl)confirmEl.hidden=true;}
 function renderSlots(){
  slotsEl.innerHTML='';var slots=byDate[selDate]||[];
  var h=document.createElement('div');h.className='surfdoc-booking-slots-head';h.textContent=fmt(selDate);slotsEl.appendChild(h);
  var grid=document.createElement('div');grid.className='surfdoc-booking-slot-grid';
  slots.forEach(function(t){var b=document.createElement('button');b.type='button';b.className='surfdoc-booking-slot';b.textContent=t;b.addEventListener('click',function(){selSlot=t;grid.querySelectorAll('.surfdoc-booking-slot').forEach(function(x){x.classList.remove('is-sel');});b.classList.add('is-sel');showForm();});grid.appendChild(b);});
  slotsEl.appendChild(grid);
 }
 function showForm(){if(!form)return;form.hidden=false;var bits=[];if(selService)bits.push(selService);bits.push(fmt(selDate));if(selSlot)bits.push(selSlot);if(summaryEl)summaryEl.textContent=bits.join(' · ');}
 if(form){form.addEventListener('submit',function(e){e.preventDefault();if(!selDate||!selSlot)return;var nf=form.querySelector('[name=name]');var name=nf?nf.value:'';var ref='BK-'+selDate.replace(/-/g,'')+'-'+(selSlot.replace(/[^0-9]/g,'').slice(0,4)||'0000');confirmEl.innerHTML='';var card=document.createElement('div');card.className='surfdoc-booking-confirm-card';var t=document.createElement('div');t.className='surfdoc-booking-confirm-title';t.textContent='Booking confirmed';var p=document.createElement('p');p.className='surfdoc-booking-confirm-detail';p.textContent=(name?name+', your ':'Your ')+(selService?selService+' ':'')+'is booked for '+fmt(selDate)+' at '+selSlot+'.';var r=document.createElement('p');r.className='surfdoc-booking-confirm-ref';r.textContent='Confirmation '+ref;card.appendChild(t);card.appendChild(p);card.appendChild(r);confirmEl.appendChild(card);confirmEl.hidden=false;form.hidden=true;});}
 if(prev)prev.addEventListener('click',function(){if(mi>0){mi--;renderMonth();}});
 if(next)next.addEventListener('click',function(){if(mi<months.length-1){mi++;renderMonth();}});
 renderMonth();
}
function boot(){document.querySelectorAll('[data-booking]').forEach(init);}
if(document.readyState!=='loading')boot();else document.addEventListener('DOMContentLoaded',boot);
})();</script>"#;

/// Self-contained client script for every `::store` widget on a page.
///
/// Guarded by a window flag (inits all `[data-store]` widgets once). Reads each
/// widget's inlined JSON and drives category filter → add-to-cart → qty/line
/// totals → checkout → order confirmation entirely client-side. Static
/// data-bound; payment is out of scope (links out).
const STORE_WIDGET_JS: &str = r#"<script>(function(){
if(window.__surfStoreInit)return;window.__surfStoreInit=1;
function init(root){
 var dataEl=root.querySelector('[data-st-data]');if(!dataEl)return;
 var data={};try{data=JSON.parse(dataEl.textContent)}catch(e){return;}
 var items=data.items||[],cur=data.currency||'$';
 var cats=[],seen={};items.forEach(function(it){var c=it.category||'All';if(!seen[c]){seen[c]=1;cats.push(c);}});
 var hasCats=cats.length>1||(cats.length===1&&cats[0]!=='All');
 var filter='All',cart={};
 var gridEl=root.querySelector('[data-st-grid]'),filtEl=root.querySelector('[data-st-filters]');
 var itemsEl=root.querySelector('[data-st-items]'),totalEl=root.querySelector('[data-st-total]');
 var checkoutBtn=root.querySelector('[data-st-checkout]'),form=root.querySelector('[data-st-form]'),confirmEl=root.querySelector('[data-st-confirm]');
 function money(n){return cur+(Number.isInteger(n)?n:n.toFixed(2));}
 function price(it){var p=parseFloat(it.price);return isNaN(p)?0:p;}
 function renderFilters(){
  if(!filtEl||!hasCats)return;
  var all=['All'].concat(cats.filter(function(c){return c!=='All';}));
  filtEl.innerHTML='';
  all.forEach(function(c){var b=document.createElement('button');b.type='button';b.className='surfdoc-store-chip'+(c===filter?' is-sel':'');b.textContent=c;b.addEventListener('click',function(){filter=c;renderFilters();renderGrid();});filtEl.appendChild(b);});
 }
 function renderGrid(){
  gridEl.innerHTML='';
  items.filter(function(it){return filter==='All'||(it.category||'All')===filter;}).forEach(function(it){
   var card=document.createElement('div');card.className='surfdoc-store-card';
   if(it.badge){var bd=document.createElement('span');bd.className='surfdoc-store-badge';bd.textContent=it.badge;card.appendChild(bd);}
   var nm=document.createElement('div');nm.className='surfdoc-store-name';nm.textContent=it.name;card.appendChild(nm);
   if(it.blurb){var bl=document.createElement('p');bl.className='surfdoc-store-blurb';bl.textContent=it.blurb;card.appendChild(bl);}
   var foot=document.createElement('div');foot.className='surfdoc-store-card-foot';
   var pr=document.createElement('span');pr.className='surfdoc-store-price';pr.textContent=money(price(it));foot.appendChild(pr);
   var add=document.createElement('button');add.type='button';add.className='surfdoc-store-add';add.textContent='Add';add.addEventListener('click',function(){addToCart(it);});foot.appendChild(add);
   card.appendChild(foot);gridEl.appendChild(card);
  });
 }
 function addToCart(it){var k=it.name;if(!cart[k])cart[k]={item:it,qty:0};cart[k].qty++;renderCart();}
 function renderCart(){
  var keys=Object.keys(cart);itemsEl.innerHTML='';var total=0;
  if(!keys.length){var e=document.createElement('p');e.className='surfdoc-store-empty';e.textContent='Your cart is empty.';itemsEl.appendChild(e);}
  keys.forEach(function(k){var c=cart[k];total+=price(c.item)*c.qty;
   var row=document.createElement('div');row.className='surfdoc-store-line';
   var nm=document.createElement('span');nm.className='surfdoc-store-line-name';nm.textContent=c.item.name;row.appendChild(nm);
   var qty=document.createElement('div');qty.className='surfdoc-store-qty';
   var minus=document.createElement('button');minus.type='button';minus.textContent='−';minus.setAttribute('aria-label','Decrease quantity');minus.addEventListener('click',function(){c.qty--;if(c.qty<=0)delete cart[k];renderCart();});
   var n=document.createElement('span');n.className='surfdoc-store-qty-n';n.textContent=c.qty;
   var plus=document.createElement('button');plus.type='button';plus.textContent='+';plus.setAttribute('aria-label','Increase quantity');plus.addEventListener('click',function(){c.qty++;renderCart();});
   qty.appendChild(minus);qty.appendChild(n);qty.appendChild(plus);row.appendChild(qty);
   var lt=document.createElement('span');lt.className='surfdoc-store-line-total';lt.textContent=money(price(c.item)*c.qty);row.appendChild(lt);
   itemsEl.appendChild(row);
  });
  if(totalEl)totalEl.textContent=money(total);
  if(checkoutBtn)checkoutBtn.disabled=!keys.length;
  if(form&&!keys.length)form.hidden=true;
 }
 if(checkoutBtn)checkoutBtn.addEventListener('click',function(){if(form){form.hidden=false;form.scrollIntoView({behavior:'smooth',block:'nearest'});}});
 if(form)form.addEventListener('submit',function(e){
  e.preventDefault();var keys=Object.keys(cart);if(!keys.length)return;
  var count=0,total=0;keys.forEach(function(k){count+=cart[k].qty;total+=price(cart[k].item)*cart[k].qty;});
  var nf=form.querySelector('[name=name]');var name=nf?nf.value:'';
  var ref='ORD-'+(String(Math.round(total))+'').slice(0,4)+'-'+count;
  confirmEl.innerHTML='';
  var card=document.createElement('div');card.className='surfdoc-store-confirm-card';
  var t=document.createElement('div');t.className='surfdoc-store-confirm-title';t.textContent='Order placed';
  var p=document.createElement('p');p.className='surfdoc-store-confirm-detail';p.textContent=(name?name+', your':'Your')+' order of '+count+' item'+(count===1?'':'s')+' ('+money(total)+') is confirmed.';
  var r=document.createElement('p');r.className='surfdoc-store-confirm-ref';r.textContent='Order '+ref;
  card.appendChild(t);card.appendChild(p);card.appendChild(r);confirmEl.appendChild(card);
  cart={};renderCart();form.hidden=true;confirmEl.hidden=false;confirmEl.scrollIntoView({behavior:'smooth',block:'nearest'});
 });
 renderFilters();renderGrid();renderCart();
}
function boot(){document.querySelectorAll('[data-store]').forEach(init);}
if(document.readyState!=='loading')boot();else document.addEventListener('DOMContentLoaded',boot);
})();</script>"#;

/// Canonical lowercase slug for a [`ChartType`] (used in class names + data
/// attributes). Single source of truth for the HTML renderer.
pub(crate) fn chart_type_str(t: ChartType) -> &'static str {
    match t {
        ChartType::Line => "line",
        ChartType::Bar => "bar",
        ChartType::Pie => "pie",
        ChartType::Area => "area",
        ChartType::Scatter => "scatter",
        ChartType::Donut => "donut",
        ChartType::StackedBar => "stacked-bar",
        ChartType::Radar => "radar",
    }
}

pub(crate) fn render_block(block: &Block) -> String {
    match block {
        Block::Markdown { content, .. } => render_markdown(content),

        Block::Callout {
            callout_type,
            title,
            content,
            ..
        } => {
            let type_str = callout_type_str(*callout_type);
            let role = if matches!(callout_type, CalloutType::Danger) { "alert" } else { "note" };
            let icon = callout_icon_svg(*callout_type);
            let title_html = match title {
                Some(t) => format!(
                    "<div class=\"surfdoc-callout-title\">{}</div>",
                    escape_html(t)
                ),
                None => String::new(),
            };
            format!(
                "<div class=\"surfdoc-callout surfdoc-callout-{type_str}\" role=\"{role}\">{icon}<div class=\"surfdoc-callout-body\">{title_html}{}</div></div>",
                render_inline_markdown(content),
            )
        }

        Block::Data {
            headers, rows, ..
        } => {
            let mut html = String::from("<div class=\"surfdoc-table-wrap\"><table class=\"surfdoc-data\">");
            if !headers.is_empty() {
                html.push_str("<thead><tr>");
                for h in headers {
                    html.push_str(&format!(
                        "<th scope=\"col\" aria-sort=\"none\">{}</th>",
                        render_cell_inline_markdown(h)
                    ));
                }
                html.push_str("</tr></thead>");
            }
            html.push_str("<tbody>");
            for row in rows {
                html.push_str("<tr>");
                for cell in row {
                    let num_class = if is_numeric_cell(cell) { " class=\"num\"" } else { "" };
                    html.push_str(&format!(
                        "<td{num_class}>{}</td>",
                        render_cell_inline_markdown(cell)
                    ));
                }
                html.push_str("</tr>");
            }
            html.push_str("</tbody></table></div>");
            html
        }

        Block::Code {
            lang,
            file,
            highlight,
            content,
            ..
        } => {
            let class = match lang {
                Some(l) => format!(" class=\"language-{}\"", escape_html(l)),
                None => String::new(),
            };
            let aria = match lang {
                Some(l) => format!(" aria-label=\"{} code\"", escape_html(l)),
                None => String::new(),
            };
            let data_lang = match lang {
                Some(l) => format!(" data-lang=\"{}\"", escape_html(l)),
                None => String::new(),
            };
            // Build the figcaption header. Render the file span only when present;
            // the lang badge (uppercased) is shown whenever a language is known.
            let file_span = match file {
                Some(f) => format!("<span class=\"surfdoc-code-file\">{}</span>", escape_html(f)),
                None => String::new(),
            };
            let lang_span = match lang {
                Some(l) => format!(
                    "<span class=\"surfdoc-code-lang\">{}</span>",
                    escape_html(&l.to_uppercase())
                ),
                None => String::new(),
            };
            let head = if file_span.is_empty() && lang_span.is_empty() {
                String::new()
            } else {
                format!(
                    "<figcaption class=\"surfdoc-code-head\">{file_span}{lang_span}</figcaption>"
                )
            };
            let code_body = render_code_with_highlights(content, highlight);
            format!(
                "<figure class=\"surfdoc-code\">{head}<pre{}{}><code{}>{}</code></pre></figure>",
                aria,
                data_lang,
                class,
                code_body,
            )
        }

        Block::Tasks { items, .. } => {
            let mut html = String::from("<ul class=\"surfdoc-tasks\">");
            for item in items {
                let li_class = if item.done { "surfdoc-task is-done" } else { "surfdoc-task" };
                let check = if item.done { "\u{2713}" } else { "" };
                let assignee_html = match &item.assignee {
                    Some(a) => format!("<span class=\"surfdoc-assignee\">@{}</span>", escape_html(a)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<li class=\"{li_class}\"><span class=\"surfdoc-check\">{check}</span><span class=\"surfdoc-task-text\">{}</span>{assignee_html}</li>",
                    render_inline_markdown_phrasing(&item.text),
                ));
            }
            html.push_str("</ul>");
            html
        }

        Block::Decision {
            status,
            date,
            deciders,
            content,
            ..
        } => {
            let status_str = decision_status_str(*status);
            let date_html = match date {
                Some(d) => format!("<span class=\"surfdoc-decision-date\">{}</span>", escape_html(d)),
                None => String::new(),
            };
            let deciders_html = if deciders.is_empty() {
                String::new()
            } else {
                let joined = deciders
                    .iter()
                    .map(|d| escape_html(d))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("<footer class=\"surfdoc-decision-meta\">Deciders: {joined}</footer>")
            };
            format!(
                "<article class=\"surfdoc-decision surfdoc-decision-{status_str}\" role=\"note\" aria-label=\"Decision: {status_str}\"><div class=\"surfdoc-decision-header\"><span class=\"surfdoc-decision-status surfdoc-decision-status--{status_str}\">{}</span>{date_html}</div><div class=\"surfdoc-decision-body\">{}</div>{deciders_html}</article>",
                capitalize(status_str),
                render_inline_markdown(content),
            )
        }

        Block::Metric {
            label,
            value,
            trend,
            unit,
            ..
        } => {
            let trend_html = match trend {
                Some(Trend::Up) => "<span class=\"surfdoc-trend surfdoc-trend--up\">\u{25B2}</span>".to_string(),
                Some(Trend::Down) => "<span class=\"surfdoc-trend surfdoc-trend--down\">\u{25BC}</span>".to_string(),
                Some(Trend::Flat) => "<span class=\"surfdoc-trend surfdoc-trend--flat\">\u{2192}</span>".to_string(),
                None => String::new(),
            };
            let unit_html = match unit {
                Some(u) => format!(" <span class=\"surfdoc-metric-unit\">{}</span>", escape_html(u)),
                None => String::new(),
            };
            let trend_text = match trend {
                Some(Trend::Up) => ", trending up",
                Some(Trend::Down) => ", trending down",
                Some(Trend::Flat) => ", flat",
                None => "",
            };
            let unit_text = match unit {
                Some(u) => format!(" {}", u),
                None => String::new(),
            };
            let aria_label = format!("{}: {}{}{}", label, value, unit_text, trend_text);
            // Metrics render one per block; still wrap the single card in a
            // `surfdoc-metric-row` so adjacent metrics lay out as a flex row and
            // each card carries the per-metric value/label structure.
            format!(
                "<div class=\"surfdoc-metric-row\"><div class=\"surfdoc-metric\" role=\"group\" aria-label=\"{}\"><div class=\"surfdoc-metric-value\">{}{unit_html}</div><div class=\"surfdoc-metric-label\">{}{trend_html}</div></div></div>",
                escape_html(&aria_label),
                escape_html(value),
                escape_html(label),
            )
        }

        // `::cite` is a definition only — it renders nothing visible.
        Block::Cite { .. } => String::new(),

        Block::Bibliography { style, .. } => render_bibliography_html(*style),

        Block::Summary { content, .. } => {
            format!(
                "<div class=\"surfdoc-summary\" role=\"doc-abstract\"><div class=\"surfdoc-summary-label\">Summary</div><p>{}</p></div>",
                render_inline_markdown(content),
            )
        }

        Block::Figure {
            src,
            caption,
            alt,
            ..
        } => {
            let alt_attr = alt.as_deref().unwrap_or("");
            let caption_html = match caption {
                Some(c) => format!("<figcaption class=\"surfdoc-figure-cap\">{}</figcaption>", escape_html(c)),
                None => String::new(),
            };
            // Wrap the image in a div so that onerror can hide the <img> and
            // the placeholder div still fills the aspect-ratio box cleanly.
            format!(
                "<figure class=\"surfdoc-figure\"><div class=\"surfdoc-figure-img\"><img src=\"{}\" alt=\"{}\" onerror=\"this.style.display='none';this.onerror=null\" /></div>{caption_html}</figure>",
                escape_html(src),
                escape_html(alt_attr),
            )
        }

        Block::Diagram {
            diagram_type,
            title,
            content,
            ..
        } => {
            let caption_html = match title {
                Some(t) => format!(
                    "<figcaption class=\"surfdoc-diagram-cap\">{}</figcaption>",
                    escape_html(t)
                ),
                None => String::new(),
            };
            // Mermaid-syntax bodies (sniffed, or explicit `type=mermaid`)
            // translate to the native DSL first; the translated type and
            // body then flow through the exact same pipeline below. The
            // prose fallback always shows the AUTHOR'S source, never the
            // translation.
            let translated = crate::mermaid_compat::translate(diagram_type, content);
            let (eff_type, eff_content) = match &translated {
                Some(t) => (t.diagram_type, t.content.as_str()),
                None => (diagram_type.as_str(), content.as_str()),
            };
            // Chart-alias types (pie/donut/radar/xychart) forward the body to
            // the `::chart` pipeline — the body is the same pipe-delimited
            // table `::chart` accepts. An unusable body degrades to the same
            // prose fallback as any other diagram.
            if let Some(chart_type) = crate::diagram::chart_alias(eff_type) {
                return match crate::blocks::parse_chart_data(eff_content) {
                    Some(data) => {
                        let svg = crate::chart::render_svg(chart_type, &data, title.as_deref());
                        format!(
                            "<figure class=\"surfdoc-diagram surfdoc-diagram-{}\">{caption_html}{svg}</figure>",
                            escape_html(eff_type),
                        )
                    }
                    None => format!(
                        "<figure class=\"surfdoc-diagram surfdoc-diagram-fallback\">{caption_html}<pre class=\"surfdoc-diagram-src\">{}</pre></figure>",
                        escape_html(content),
                    ),
                };
            }
            match crate::diagram::parse_diagram_source(eff_type, eff_content) {
                Ok(model) => {
                    let svg = crate::diagram::render_svg(&model, title.as_deref());
                    format!(
                        "<figure class=\"surfdoc-diagram surfdoc-diagram-{}\">{caption_html}{svg}</figure>",
                        escape_html(eff_type),
                    )
                }
                // Malformed DSL or empty/unknown type NEVER fails the render —
                // degrade to a preformatted prose fallback.
                Err(_) => format!(
                    "<figure class=\"surfdoc-diagram surfdoc-diagram-fallback\">{caption_html}<pre class=\"surfdoc-diagram-src\">{}</pre></figure>",
                    escape_html(content),
                ),
            }
        }

        Block::Tabs { tabs, .. } => {
            let mut html = String::from("<div class=\"surfdoc-tabs\">");
            html.push_str("<nav role=\"tablist\">");
            for (i, tab) in tabs.iter().enumerate() {
                let selected = if i == 0 { "true" } else { "false" };
                let tabindex = if i == 0 { "0" } else { "-1" };
                html.push_str(&format!(
                    "<button class=\"tab-btn{}\" role=\"tab\" aria-selected=\"{}\" aria-controls=\"surfdoc-panel-{}\" id=\"surfdoc-tab-{}\" tabindex=\"{}\">{}</button>",
                    if i == 0 { " active" } else { "" },
                    selected,
                    i,
                    i,
                    tabindex,
                    escape_html(&tab.label)
                ));
            }
            html.push_str("</nav>");
            for (i, tab) in tabs.iter().enumerate() {
                let active = if i == 0 { " active" } else { "" };
                let hidden = if i == 0 { "" } else { " hidden" };
                let content_html = render_markdown(&tab.content);
                html.push_str(&format!(
                    "<div class=\"tab-panel{}\" role=\"tabpanel\" id=\"surfdoc-panel-{}\" aria-labelledby=\"surfdoc-tab-{}\" tabindex=\"0\"{}>{}</div>",
                    active, i, i, hidden, content_html
                ));
            }
            html.push_str(r#"<script>document.querySelectorAll('.surfdoc-tabs').forEach(t=>{t.querySelectorAll('[role="tab"]').forEach(b=>{b.onclick=()=>{t.querySelectorAll('[role="tab"]').forEach(e=>{e.classList.remove('active');e.setAttribute('aria-selected','false');e.tabIndex=-1});b.classList.add('active');b.setAttribute('aria-selected','true');b.tabIndex=0;t.querySelectorAll('[role="tabpanel"]').forEach(p=>{p.classList.remove('active');p.hidden=true});var panel=document.getElementById(b.getAttribute('aria-controls'));if(panel){panel.classList.add('active');panel.hidden=false}}})})</script>"#);
            html.push_str("</div>");
            html
        }

        Block::Columns { columns, .. } => {
            let count = columns.len();
            let mut html = format!(
                "<div class=\"surfdoc-columns\" role=\"group\" data-cols=\"{}\">",
                count
            );
            for col in columns {
                let col_html = render_markdown(&col.content);
                html.push_str(&format!(
                    "<div class=\"surfdoc-column\">{}</div>",
                    col_html
                ));
            }
            html.push_str("</div>");
            html
        }

        Block::Quote {
            content,
            attribution,
            cite,
            ..
        } => {
            let mut html = String::from("<blockquote class=\"surfdoc-quote\"><p>");
            html.push_str(&render_inline_markdown_phrasing(content));
            html.push_str("</p>");
            if let Some(attr) = attribution {
                let cite_part = match cite {
                    Some(c) => format!(", <cite>{}</cite>", escape_html(c)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<div class=\"surfdoc-quote-by\">{}{}</div>",
                    escape_html(attr),
                    cite_part,
                ));
            }
            html.push_str("</blockquote>");
            html
        }

        Block::Cta {
            label,
            href,
            primary,
            icon,
            ..
        } => {
            let class = if *primary { "surfdoc-cta surfdoc-cta-primary" } else { "surfdoc-cta surfdoc-cta-secondary" };
            let icon_html = icon
                .as_deref()
                .and_then(get_icon)
                .map(|svg| format!("<span class=\"surfdoc-icon\">{}</span> ", svg))
                .unwrap_or_default();
            format!(
                "<a class=\"{}\" href=\"{}\">{}{}</a>",
                class,
                escape_html(href),
                icon_html,
                escape_html(label),
            )
        }

        Block::HeroImage { src, alt, .. } => {
            let alt_attr = alt.as_deref().unwrap_or("");
            let role_attr = if !alt_attr.is_empty() {
                format!(" role=\"img\" aria-label=\"{}\"", escape_html(alt_attr))
            } else {
                String::new()
            };
            format!(
                "<div class=\"surfdoc-hero-image\"{}><img src=\"{}\" alt=\"{}\" onerror=\"this.classList.add('broken');this.onerror=null\" /></div>",
                role_attr,
                escape_html(src),
                escape_html(alt_attr),
            )
        }

        Block::Testimonial {
            content,
            author,
            role,
            company,
            ..
        } => {
            let aria_label = match author {
                Some(a) => format!(" aria-label=\"Testimonial from {}\"", escape_html(a)),
                None => " aria-label=\"Testimonial\"".to_string(),
            };
            // Reference look (.sd-testimonial): quoted (not italic) blockquote, then
            // a figcaption with an initials avatar + name/role meta.
            let mut html =
                format!("<figure class=\"surfdoc-testimonial\" role=\"figure\"{aria_label}>");
            html.push_str(&format!(
                "<blockquote>\u{201C}{}\u{201D}</blockquote>",
                render_inline_markdown_phrasing(content)
            ));
            if author.is_some() || role.is_some() || company.is_some() {
                html.push_str("<figcaption class=\"surfdoc-testimonial-by\">");
                if let Some(a) = author {
                    // Avatar: up to two initials from the author's name.
                    let initials: String = a
                        .split_whitespace()
                        .filter_map(|w| w.chars().next())
                        .take(2)
                        .collect::<String>()
                        .to_uppercase();
                    if !initials.is_empty() {
                        html.push_str(&format!(
                            "<span class=\"surfdoc-testimonial-avatar\" aria-hidden=\"true\">{}</span>",
                            escape_html(&initials)
                        ));
                    }
                }
                html.push_str("<span class=\"surfdoc-testimonial-meta\">");
                if let Some(a) = author {
                    html.push_str(&format!("<strong>{}</strong>", escape_html(a)));
                }
                let details: Vec<&str> = [role.as_deref(), company.as_deref()]
                    .iter()
                    .filter_map(|v| *v)
                    .collect();
                if !details.is_empty() {
                    html.push_str(&format!("<br><span>{}</span>", escape_html(&details.join(", "))));
                }
                html.push_str("</span>");
                html.push_str("</figcaption>");
            }
            html.push_str("</figure>");
            html
        }

        Block::Style { properties, .. } => {
            // Style blocks are metadata — rendered as a hidden data element
            let pairs: Vec<String> = properties
                .iter()
                .map(|p| format!("{}={}", escape_html(&p.key), escape_html(&p.value)))
                .collect();
            format!(
                "<div class=\"surfdoc-style\" aria-hidden=\"true\" data-properties=\"{}\"></div>",
                escape_html(&pairs.join(";"))
            )
        }

        Block::Faq { items, .. } => {
            let mut html = String::from("<div class=\"surfdoc-faq\">");
            for item in items {
                html.push_str(&format!(
                    "<details class=\"surfdoc-faq-item\"><summary>{}</summary><div class=\"surfdoc-faq-answer\">{}</div></details>",
                    escape_html(&item.question),
                    escape_html(&item.answer),
                ));
            }
            html.push_str("</div>");
            html
        }

        Block::PricingTable {
            headers, rows, ..
        } => {
            // Reference look (.sd-pricing): a grid of tier CARDS, not a table.
            // Each row is one tier — col 0 = tier name, col 1 = price, remaining
            // cols become "<value> <header>" feature bullets. A tier whose name
            // is bold (`**Team**`) is the featured tier (accent border). A `/...`
            // suffix in the price (e.g. `$6.99/seat`) renders as a muted span.
            let mut html = String::from("<div class=\"surfdoc-pricing\" aria-label=\"Pricing\">");
            for row in rows {
                if row.is_empty() {
                    continue;
                }
                let raw_name = row[0].trim();
                let (name, featured) = if raw_name.len() >= 4
                    && raw_name.starts_with("**")
                    && raw_name.ends_with("**")
                {
                    (raw_name[2..raw_name.len() - 2].trim(), true)
                } else {
                    (raw_name, false)
                };
                let tier_cls = if featured {
                    "surfdoc-tier surfdoc-tier-featured"
                } else {
                    "surfdoc-tier"
                };
                html.push_str(&format!("<div class=\"{tier_cls}\">"));
                html.push_str(&format!(
                    "<div class=\"surfdoc-tier-name\">{}</div>",
                    escape_html(name)
                ));
                if let Some(price) = row.get(1) {
                    let price = price.trim();
                    if let Some(slash) = price.find('/') {
                        let (amount, suffix) = price.split_at(slash);
                        html.push_str(&format!(
                            "<div class=\"surfdoc-tier-price\">{}<span>{}</span></div>",
                            escape_html(amount.trim()),
                            escape_html(suffix)
                        ));
                    } else {
                        html.push_str(&format!(
                            "<div class=\"surfdoc-tier-price\">{}</div>",
                            escape_html(price)
                        ));
                    }
                }
                if row.len() > 2 {
                    html.push_str("<ul>");
                    for (i, cell) in row.iter().enumerate().skip(2) {
                        let val = cell.trim();
                        if val.is_empty() {
                            continue;
                        }
                        let header = headers.get(i).map(|h| h.trim()).unwrap_or("");
                        let bullet = if header.is_empty() {
                            val.to_string()
                        } else {
                            format!("{} {}", val, header.to_lowercase())
                        };
                        html.push_str(&format!(
                            "<li>{}</li>",
                            render_cell_inline_markdown(&bullet)
                        ));
                    }
                    html.push_str("</ul>");
                }
                // Subscribe CTA — every tier card gets an action button so the
                // pricing grid is actionable. Featured tier = primary fill;
                // free/$0 tier reads "Get started", paid tiers "Subscribe".
                let price_l = row
                    .get(1)
                    .map(|p| p.trim().to_lowercase())
                    .unwrap_or_default();
                let is_free = price_l.is_empty()
                    || price_l.contains("free")
                    || price_l.starts_with("$0")
                    || price_l == "0";
                let cta_label = if is_free { "Get started" } else { "Subscribe" };
                let cta_cls = if featured {
                    "surfdoc-tier-cta surfdoc-tier-cta-primary"
                } else {
                    "surfdoc-tier-cta surfdoc-tier-cta-secondary"
                };
                html.push_str(&format!(
                    "<a class=\"{cta_cls}\" href=\"#\">{}</a>",
                    escape_html(cta_label)
                ));
                html.push_str("</div>");
            }
            html.push_str("</div>");
            html
        }

        Block::Site { properties, domain, .. } => {
            // Site config is metadata — hidden element with data attributes
            let domain_attr = match domain {
                Some(d) => format!(" data-domain=\"{}\"", escape_html(d)),
                None => String::new(),
            };
            let pairs: Vec<String> = properties
                .iter()
                .map(|p| format!("{}={}", escape_html(&p.key), escape_html(&p.value)))
                .collect();
            format!(
                "<div class=\"surfdoc-site\" aria-hidden=\"true\"{} data-properties=\"{}\"></div>",
                domain_attr,
                escape_html(&pairs.join(";")),
            )
        }

        Block::Page {
            route, layout, title, children, ..
        } => {
            let layout_attr = match layout {
                Some(l) => format!(" data-layout=\"{}\"", escape_html(l)),
                None => String::new(),
            };
            let aria_label = match title {
                Some(t) => format!(" aria-label=\"{}\"", escape_html(t)),
                None => format!(" aria-label=\"Page: {}\"", escape_html(route)),
            };
            let mut html = format!("<section class=\"surfdoc-page\"{layout_attr}{aria_label}>");
            for child in children {
                html.push_str(&render_block(child));
            }
            html.push_str("</section>");
            html
        }

        // A `::deck` is a config block; rendered inline (in a non-deck context)
        // it produces nothing — the presentation chrome lives in render_slides.
        Block::Deck { .. } => String::new(),

        // A `::slide` rendered inline (outside the deck renderer) degrades to a
        // labeled section of its child blocks.
        Block::Slide {
            kicker, children, ..
        } => {
            let kicker_html = kicker
                .as_ref()
                .map(|k| format!("<p class=\"surfdoc-kicker\">{}</p>", escape_html(k)))
                .unwrap_or_default();
            let mut html = format!("<section class=\"surfdoc-slide\">{kicker_html}");
            for child in children {
                html.push_str(&render_block(child));
            }
            html.push_str("</section>");
            html
        }

        Block::Nav { items, logo, groups, brand, brand_reg, cta, drawer, minimal, .. } => {
            if *minimal {
                render_minimal_nav_html(logo.as_deref(), brand.as_deref(), *brand_reg)
            } else if nav_is_rich(groups, brand, *drawer) {
                render_shell_nav_html(items, groups, logo.as_deref(), brand.as_deref(), *brand_reg, cta.as_ref())
            } else {
                render_nav_shell_html(items, logo.as_deref())
            }
        }

        Block::Embed {
            src, embed_type, title, width, height, ..
        } => {
            let title_text = title.as_deref().unwrap_or(src.as_str());
            // Generic embeds (no recognized provider type) with a real src and an
            // explicit height are first-class iframes — e.g. a Termly policy viewer
            // or any hosted page meant to render inline. Provider types (video/
            // audio/map) keep the safe link-card so unembeddable share URLs
            // (youtu.be/xyz etc.) don't produce an error box.
            let is_generic = matches!(embed_type, None | Some(crate::types::EmbedType::Generic));
            if is_generic && !src.is_empty() && height.is_some() {
                let h = height.as_deref().unwrap();
                let w = width.as_deref().unwrap_or("100%");
                let title_attr = match title {
                    Some(t) => format!(" title=\"{}\"", escape_html(t)),
                    None => String::new(),
                };
                return format!(
                    "<iframe class=\"surfdoc-embed-frame\" src=\"{}\"{} style=\"width:{};height:{};border:0\" loading=\"lazy\"></iframe>",
                    escape_html(src),
                    title_attr,
                    escape_html(w),
                    escape_html(h),
                );
            }
            let type_label = match embed_type {
                Some(crate::types::EmbedType::Map) => "map",
                Some(crate::types::EmbedType::Video) => "video",
                Some(crate::types::EmbedType::Audio) => "audio",
                _ => "embed",
            };
            // Compact link-card: icon + title + source URL + type badge.
            // Matches the reference .sd-embed design — no iframe so broken URLs
            // (youtu.be/xyz etc.) don't produce an error box.
            let icon_svg = match embed_type {
                Some(crate::types::EmbedType::Video) => {
                    "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M8 5v14l11-7z\"/></svg>"
                }
                Some(crate::types::EmbedType::Audio) => {
                    "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M9 18V5l12-2v13\"/><circle cx=\"6\" cy=\"18\" r=\"3\"/><circle cx=\"18\" cy=\"16\" r=\"3\"/></svg>"
                }
                Some(crate::types::EmbedType::Map) => {
                    "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><polygon points=\"1 6 1 22 8 18 16 22 23 18 23 2 16 6 8 2 1 6\"/><line x1=\"8\" y1=\"2\" x2=\"8\" y2=\"18\"/><line x1=\"16\" y1=\"6\" x2=\"16\" y2=\"22\"/></svg>"
                }
                _ => {
                    "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><rect x=\"3\" y=\"3\" width=\"7\" height=\"7\"/><rect x=\"14\" y=\"3\" width=\"7\" height=\"7\"/><rect x=\"14\" y=\"14\" width=\"7\" height=\"7\"/><rect x=\"3\" y=\"14\" width=\"7\" height=\"7\"/></svg>"
                }
            };
            format!(
                "<div class=\"surfdoc-embed\"><div class=\"surfdoc-embed-icon\">{icon_svg}</div><div class=\"surfdoc-embed-body\"><div class=\"surfdoc-embed-title\">{}</div><div class=\"surfdoc-embed-src\">{} · {}</div></div></div>",
                escape_html(title_text),
                escape_html(src),
                escape_html(type_label),
            )
        }

        Block::Form {
            fields, submit_label, action, method, honeypot, ..
        } => {
            let btn_label = submit_label.as_deref().unwrap_or("Submit");
            // A real submission target makes the form POST to a server route.
            let target_attrs = match action {
                Some(a) => {
                    let m = method.as_deref().unwrap_or("post");
                    format!(" method=\"{}\" action=\"{}\"", escape_html(m), escape_html(a))
                }
                None => String::new(),
            };
            let mut html = format!("<form class=\"surfdoc-form\"{target_attrs}>");
            if *honeypot {
                html.push_str("<input type=\"text\" name=\"_honey\" class=\"surfdoc-form-honey\" tabindex=\"-1\" autocomplete=\"off\" aria-hidden=\"true\">");
            }
            for field in fields {
                let req = if field.required { " required" } else { "" };
                let req_star = if field.required { " <span class=\"required\">*</span>" } else { "" };
                html.push_str(&format!(
                    "<div class=\"surfdoc-form-field\"><label>{}{}</label>",
                    escape_html(&field.label),
                    req_star,
                ));
                match field.field_type {
                    FormFieldType::Textarea => {
                        let ph = field.placeholder.as_deref().unwrap_or("");
                        html.push_str(&format!(
                            "<textarea name=\"{}\" placeholder=\"{}\" rows=\"4\"{}></textarea>",
                            escape_html(&field.name),
                            escape_html(ph),
                            req,
                        ));
                    }
                    FormFieldType::Select => {
                        html.push_str(&format!(
                            "<select name=\"{}\"{}>",
                            escape_html(&field.name),
                            req,
                        ));
                        html.push_str("<option value=\"\">Select...</option>");
                        for opt in &field.options {
                            html.push_str(&format!(
                                "<option value=\"{}\">{}</option>",
                                escape_html(opt),
                                escape_html(opt),
                            ));
                        }
                        html.push_str("</select>");
                    }
                    _ => {
                        let input_type = match field.field_type {
                            FormFieldType::Email => "email",
                            FormFieldType::Tel => "tel",
                            FormFieldType::Date => "date",
                            FormFieldType::Number => "number",
                            FormFieldType::Password => "password",
                            _ => "text",
                        };
                        let ph = field.placeholder.as_deref().unwrap_or("");
                        html.push_str(&format!(
                            "<input type=\"{}\" name=\"{}\" placeholder=\"{}\"{}/>",
                            input_type,
                            escape_html(&field.name),
                            escape_html(ph),
                            req,
                        ));
                    }
                }
                html.push_str("</div>");
            }
            html.push_str(&format!(
                "<button type=\"submit\" class=\"surfdoc-form-submit\">{}</button>",
                escape_html(btn_label),
            ));
            html.push_str("</form>");
            html
        }

        Block::Banner {
            headline,
            subtitle,
            buttons,
            id,
            ..
        } => {
            let id_attr = id
                .as_ref()
                .map(|i| format!(" id=\"{}\"", escape_html(i)))
                .unwrap_or_default();
            let mut parts = Vec::new();
            parts.push(format!("<section class=\"surfdoc-banner\"{id_attr}>"));
            parts.push("<div class=\"surfdoc-banner-inner\">".to_string());
            if let Some(h) = headline {
                parts.push(format!(
                    "<h2 class=\"surfdoc-banner-headline\">{}</h2>",
                    render_inline_markdown_phrasing(h)
                ));
            }
            if let Some(s) = subtitle {
                parts.push(format!(
                    "<p class=\"surfdoc-banner-subtitle\">{}</p>",
                    render_inline_markdown_phrasing(s)
                ));
            }
            if !buttons.is_empty() {
                parts.push("<div class=\"surfdoc-banner-actions\">".to_string());
                for btn in buttons {
                    let cls = if btn.primary {
                        "surfdoc-banner-btn surfdoc-banner-btn-primary"
                    } else {
                        "surfdoc-banner-btn surfdoc-banner-btn-secondary"
                    };
                    let target = if btn.external { " target=\"_blank\" rel=\"noopener\"" } else { "" };
                    parts.push(format!(
                        "<a href=\"{}\" class=\"{}\"{}>{}</a>",
                        escape_html(&btn.href),
                        cls,
                        target,
                        escape_html(&btn.label)
                    ));
                }
                parts.push("</div>".to_string());
            }
            parts.push("</div>".to_string());
            parts.push("</section>".to_string());
            parts.join("")
        }

        Block::ProductGrid { groups, tiles: true, .. } => {
            // apple.com-style promo tiles: a full-bleed (site-capped) 2-up band;
            // each tile is one link with centered copy at the top over a per-card
            // background (image/color/gradient/transparent via the bg field).
            let mut parts = Vec::new();
            parts.push("<section class=\"surfdoc-product-grid surfdoc-pg-tiles-band\">".to_string());
            parts.push("<div class=\"surfdoc-pg-tiles\">".to_string());
            for group in groups {
                if let Some(label) = &group.label {
                    parts.push(format!(
                        "<h3 class=\"surfdoc-pg-label surfdoc-pg-tiles-label\">{}</h3>",
                        render_inline_markdown_phrasing(label)
                    ));
                }
                // Each group is one row; {cols=N} (1–3) sets its column count,
                // default 2. Mobile always collapses to 1 (CSS).
                parts.push(format!(
                    "<div class=\"surfdoc-pg-tiles-row\" data-cols=\"{}\">",
                    group.cols.unwrap_or(2).clamp(1, 3)
                ));
                for item in &group.items {
                    let slug: String = item
                        .name
                        .to_lowercase()
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect::<String>()
                        .split('-')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("-");
                    let (bg_style, dark, copy_bottom, theme_ink) = product_tile_bg(item.bg.as_deref());
                    let is_image = item
                        .bg
                        .as_deref()
                        .is_some_and(|b| b.trim().starts_with("image:"));
                    // `surface` keyword: opaque theme surface via a class (no
                    // inline style, no -bg ink pinning — text follows the theme).
                    let is_surface = item
                        .bg
                        .as_deref()
                        .is_some_and(|b| b.split_whitespace().next() == Some("surface"));
                    // image: → media div below the copy; color:/gradient: →
                    // tile background. Only the latter pins the -bg ink scheme
                    // (an image tile's surface still follows the theme).
                    let style_attr = if bg_style.is_empty() || is_image {
                        String::new()
                    } else {
                        format!(" style=\"{}\"", bg_style)
                    };
                    // An explicit background doesn't change with the site theme,
                    // so its copy can't follow var(--text): -bg pins dark ink,
                    // the ` dark` flag flips it to white. Transparent tiles
                    // keep the theme-following default.
                    let mut scheme_class = String::new();
                    if !bg_style.is_empty() && !is_image {
                        scheme_class.push_str(" surfdoc-pg-tile-bg");
                    }
                    if is_surface {
                        scheme_class.push_str(" surfdoc-pg-tile-surface");
                    }
                    if dark {
                        scheme_class.push_str(" surfdoc-pg-tile-dark");
                    }
                    if theme_ink {
                        scheme_class.push_str(" surfdoc-pg-tile-theme");
                    }
                    if copy_bottom {
                        scheme_class.push_str(" surfdoc-pg-tile-copy-bottom");
                    }
                    // Whole-tile click: the primary action is the explicit
                    // first CTA when given, else the card href. A linked tile
                    // carries an invisible stretched anchor to it (the pills
                    // stay clickable above); linkless tiles keep no affordance.
                    let (p_label, p_href) = match (&item.cta1_label, &item.cta1_href) {
                        (Some(l), Some(h)) => (l.as_str(), h.as_str()),
                        _ => ("Learn more", item.href.as_str()),
                    };
                    let linked = !p_href.is_empty() && p_href != "-";
                    let p_rel = if p_href.contains("://") { " target=\"_blank\" rel=\"noopener\"" } else { "" };
                    // An href-less tile whose only action is the secondary
                    // pill (the one-link grammar form) stretches to that
                    // instead — it IS the tile's primary action.
                    let stretch_href = if linked {
                        Some(p_href)
                    } else {
                        item.cta2_href
                            .as_deref()
                            .filter(|h| !h.is_empty() && *h != "-")
                    };
                    if stretch_href.is_some() {
                        scheme_class.push_str(" surfdoc-pg-tile-linked");
                    }
                    parts.push(format!(
                        "<div class=\"surfdoc-pg-tile{}\" data-product=\"{}\"{}>",
                        scheme_class,
                        escape_html(&slug),
                        style_attr,
                    ));
                    if let Some(sh) = stretch_href {
                        // aria-hidden + tabindex=-1: the visible pill is the
                        // accessible link; the stretch is mouse-only.
                        let s_rel = if sh.contains("://") { " target=\"_blank\" rel=\"noopener\"" } else { "" };
                        parts.push(format!(
                            "<a href=\"{}\" class=\"surfdoc-pg-tile-stretch\" tabindex=\"-1\" aria-hidden=\"true\"{}></a>",
                            escape_html(sh),
                            s_rel,
                        ));
                    }
                    let media = if is_image && !bg_style.is_empty() {
                        Some(format!(
                            "<div class=\"surfdoc-pg-tile-media\" style=\"{}\" aria-hidden=\"true\"></div>",
                            bg_style
                        ))
                    } else {
                        None
                    };
                    if copy_bottom {
                        if let Some(m) = &media {
                            parts.push(m.clone());
                        }
                    }
                    parts.push("<div class=\"surfdoc-pg-tile-copy\">".to_string());
                    if let Some(emblem) = &item.emblem {
                        // `icon:<lucide-name>` renders the built-in icon as an
                        // accent chip instead of an image (unknown name → omit).
                        if let Some(name) = emblem.strip_prefix("icon:") {
                            if let Some(svg) = get_icon(name.trim()) {
                                parts.push(format!(
                                    "<span class=\"surfdoc-pg-tile-icon\">{}</span>",
                                    svg
                                ));
                            }
                        } else {
                            parts.push(format!(
                                "<img src=\"{}\" alt=\"\" width=\"48\" height=\"48\" class=\"surfdoc-pg-tile-emblem\" loading=\"lazy\" onerror=\"this.classList.add('broken');this.onerror=null\">",
                                escape_html(emblem)
                            ));
                        }
                    }
                    parts.push(format!(
                        "<h3 class=\"surfdoc-pg-tile-name\">{}</h3>",
                        escape_html(&item.name)
                    ));
                    if let Some(tagline) = &item.tagline {
                        parts.push(format!(
                            "<p class=\"surfdoc-pg-tile-tagline\">{}</p>",
                            render_inline_markdown_phrasing(tagline)
                        ));
                    }
                    // CTA row: primary ("Learn more" -> the card href) + the
                    // optional secondary outline pill from the 6th field. A
                    // linkless card (href `-`/empty, no override) gets no
                    // primary pill — there is nowhere to go yet.
                    parts.push("<span class=\"surfdoc-pg-tile-actions\">".to_string());
                    if linked {
                        parts.push(format!(
                            "<a href=\"{}\" class=\"surfdoc-pg-tile-cta\"{}>{}</a>",
                            escape_html(p_href),
                            p_rel,
                            escape_html(p_label),
                        ));
                    }
                    if let (Some(l), Some(h)) = (&item.cta2_label, &item.cta2_href) {
                        let rel2 = if h.contains("://") { " target=\"_blank\" rel=\"noopener\"" } else { "" };
                        parts.push(format!(
                            "<a href=\"{}\" class=\"surfdoc-pg-tile-cta surfdoc-pg-tile-cta-secondary\"{}>{}</a>",
                            escape_html(h),
                            rel2,
                            escape_html(l),
                        ));
                    }
                    parts.push("</span>".to_string()); // .surfdoc-pg-tile-actions
                    parts.push("</div>".to_string()); // .surfdoc-pg-tile-copy
                    if !copy_bottom {
                        if let Some(m) = &media {
                            parts.push(m.clone());
                        }
                    }
                    parts.push("</div>".to_string()); // .surfdoc-pg-tile
                }
                parts.push("</div>".to_string()); // .surfdoc-pg-tiles-row
            }
            parts.push("</div>".to_string()); // .surfdoc-pg-tiles
            parts.push("</section>".to_string());
            parts.join("")
        }

        Block::ProductGrid { groups, .. } => {
            const ARROW: &str = "<svg class=\"surfdoc-pg-arrow\" width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"5\" y1=\"12\" x2=\"19\" y2=\"12\"/><polyline points=\"12 5 19 12 12 19\"/></svg>";
            let mut parts = Vec::new();
            parts.push("<section class=\"surfdoc-product-grid\">".to_string());
            parts.push("<div class=\"surfdoc-pg-inner\">".to_string());
            for group in groups {
                if let Some(label) = &group.label {
                    parts.push(format!(
                        "<h3 class=\"surfdoc-pg-label\">{}</h3>",
                        render_inline_markdown_phrasing(label)
                    ));
                }
                parts.push("<div class=\"surfdoc-pg-row\">".to_string());
                for item in &group.items {
                    let slug: String = item
                        .name
                        .to_lowercase()
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect::<String>()
                        .split('-')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("-");
                    // Linkless card (href `-`/empty): a static div — no link,
                    // no arrow affordance, no hover invitation.
                    let linkless = item.href.is_empty() || item.href == "-";
                    if linkless {
                        parts.push(format!(
                            "<div class=\"surfdoc-pg-card surfdoc-pg-card-static\" data-product=\"{}\">",
                            escape_html(&slug),
                        ));
                    } else {
                        let external = item.href.contains("://");
                        let rel = if external {
                            " target=\"_blank\" rel=\"noopener\""
                        } else {
                            ""
                        };
                        parts.push(format!(
                            "<a href=\"{}\" class=\"surfdoc-pg-card\" data-product=\"{}\"{}>",
                            escape_html(&item.href),
                            escape_html(&slug),
                            rel
                        ));
                    }
                    parts.push("<div class=\"surfdoc-pg-body\">".to_string());
                    if let Some(emblem) = &item.emblem {
                        // `icon:<lucide-name>` — same grammar as tiles mode.
                        if let Some(name) = emblem.strip_prefix("icon:") {
                            if let Some(svg) = get_icon(name.trim()) {
                                parts.push(format!(
                                    "<span class=\"surfdoc-pg-icon\">{}</span>",
                                    svg
                                ));
                            }
                        } else {
                            parts.push(format!(
                                "<img src=\"{}\" alt=\"\" width=\"40\" height=\"40\" class=\"surfdoc-pg-emblem\" loading=\"lazy\" onerror=\"this.classList.add('broken');this.onerror=null\">",
                                escape_html(emblem)
                            ));
                        }
                    }
                    parts.push("<div class=\"surfdoc-pg-text\">".to_string());
                    parts.push(format!(
                        "<span class=\"surfdoc-pg-name\">{}</span>",
                        escape_html(&item.name)
                    ));
                    if let Some(tagline) = &item.tagline {
                        parts.push(format!(
                            "<p class=\"surfdoc-pg-tagline\">{}</p>",
                            render_inline_markdown_phrasing(tagline)
                        ));
                    }
                    parts.push("</div>".to_string()); // .surfdoc-pg-text
                    parts.push("</div>".to_string()); // .surfdoc-pg-body
                    if linkless {
                        parts.push("</div>".to_string());
                    } else {
                        parts.push(ARROW.to_string());
                        parts.push("</a>".to_string());
                    }
                }
                parts.push("</div>".to_string()); // .surfdoc-pg-row
            }
            parts.push("</div>".to_string()); // .surfdoc-pg-inner
            parts.push("</section>".to_string());
            parts.join("")
        }

        Block::PostGrid { title, subtitle, items, .. } => {
            let mut parts = Vec::new();
            parts.push("<section class=\"surfdoc-post-grid\">".to_string());
            parts.push("<div class=\"surfdoc-pg2-inner\">".to_string());
            if title.is_some() || subtitle.is_some() {
                parts.push("<header class=\"surfdoc-post-grid-head\">".to_string());
                if let Some(t) = title {
                    parts.push(format!(
                        "<h1 class=\"surfdoc-post-grid-title\">{}</h1>",
                        render_inline_markdown_phrasing(t)
                    ));
                }
                if let Some(s) = subtitle {
                    parts.push(format!(
                        "<p class=\"surfdoc-post-grid-subtitle\">{}</p>",
                        render_inline_markdown_phrasing(s)
                    ));
                }
                parts.push("</header>".to_string());
            }
            parts.push("<div class=\"surfdoc-post-cards\">".to_string());
            for item in items {
                let rel = if item.external {
                    " target=\"_blank\" rel=\"noopener\""
                } else {
                    ""
                };
                parts.push(format!(
                    "<a class=\"surfdoc-post-card\" href=\"{}\"{}>",
                    escape_html(&item.href),
                    rel
                ));
                if let Some(image) = &item.image {
                    parts.push(format!(
                        "<span class=\"surfdoc-post-card-img\" style=\"--img:url('{}')\"></span>",
                        escape_html(image)
                    ));
                }
                parts.push("<span class=\"surfdoc-post-card-body\">".to_string());
                if let Some(meta) = &item.meta {
                    parts.push(format!(
                        "<span class=\"surfdoc-post-card-meta\">{}</span>",
                        render_inline_markdown_phrasing(meta)
                    ));
                }
                parts.push(format!(
                    "<h3 class=\"surfdoc-post-card-title\">{}</h3>",
                    escape_html(&item.title)
                ));
                if let Some(excerpt) = &item.excerpt {
                    parts.push(format!(
                        "<p class=\"surfdoc-post-card-excerpt\">{}</p>",
                        render_inline_markdown_phrasing(excerpt)
                    ));
                }
                parts.push(
                    "<span class=\"surfdoc-post-card-more\">Read more →</span>".to_string(),
                );
                parts.push("</span>".to_string()); // .surfdoc-post-card-body
                parts.push("</a>".to_string());
            }
            parts.push("</div>".to_string()); // .surfdoc-post-cards
            parts.push("</div>".to_string()); // .surfdoc-pg2-inner
            parts.push("</section>".to_string());
            parts.join("")
        }

        Block::Gate { title, subtitle, action, field_label, submit_label, error, .. } => {
            let placeholder = field_label.as_deref().unwrap_or("Access code");
            let submit = submit_label.as_deref().unwrap_or("Continue");
            let mut parts = Vec::new();
            parts.push("<section class=\"surfdoc-gate\">".to_string());
            parts.push("<div class=\"surfdoc-gate-card\">".to_string());
            if let Some(t) = title {
                parts.push(format!(
                    "<h1 class=\"surfdoc-gate-title\">{}</h1>",
                    render_inline_markdown_phrasing(t)
                ));
            }
            if let Some(s) = subtitle {
                parts.push(format!(
                    "<p class=\"surfdoc-gate-subtitle\">{}</p>",
                    render_inline_markdown_phrasing(s)
                ));
            }
            if let Some(e) = error {
                parts.push(format!(
                    "<p class=\"surfdoc-gate-error\" role=\"alert\">{}</p>",
                    escape_html(e)
                ));
            }
            parts.push(format!(
                "<form method=\"post\" action=\"{}\"><input type=\"password\" name=\"code\" class=\"surfdoc-gate-input\" placeholder=\"{}\" required autofocus autocomplete=\"off\"><button class=\"surfdoc-gate-submit\" type=\"submit\">{}</button></form>",
                escape_html(action),
                escape_html(placeholder),
                escape_html(submit),
            ));
            parts.push("</div>".to_string()); // .surfdoc-gate-card
            parts.push("</section>".to_string());
            parts.join("")
        }

        Block::Gallery { items, columns, .. } => {
            let cols = columns.unwrap_or(3);
            // Collect unique categories for filter
            let categories: Vec<&str> = {
                let mut cats: Vec<&str> = items.iter()
                    .filter_map(|i| i.category.as_deref())
                    .collect();
                cats.sort();
                cats.dedup();
                cats
            };
            let mut html = format!("<div class=\"surfdoc-gallery\" data-cols=\"{}\">", cols);
            if !categories.is_empty() {
                html.push_str("<div class=\"surfdoc-gallery-filters\">");
                html.push_str("<button class=\"filter-btn active\" data-filter=\"all\">All</button>");
                for cat in &categories {
                    html.push_str(&format!(
                        "<button class=\"filter-btn\" data-filter=\"{}\">{}</button>",
                        escape_html(cat),
                        escape_html(cat),
                    ));
                }
                html.push_str("</div>");
            }
            html.push_str("<div class=\"surfdoc-gallery-grid\">");
            for (i, item) in items.iter().enumerate() {
                let alt = item.alt.as_deref().unwrap_or("");
                let cat_attr = match &item.category {
                    Some(c) => format!(" data-category=\"{}\"", escape_html(c)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<figure class=\"surfdoc-gallery-item\" data-index=\"{}\" style=\"cursor:pointer\" tabindex=\"0\" role=\"button\" aria-label=\"Open image in lightbox\"{cat_attr}>",
                    i
                ));
                html.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\" loading=\"lazy\" onerror=\"this.style.display='none';this.onerror=null\" />",
                    escape_html(&item.src),
                    escape_html(alt),
                ));
                if let Some(cap) = &item.caption {
                    html.push_str(&format!("<figcaption>{}</figcaption>", escape_html(cap)));
                }
                html.push_str("</figure>");
            }
            html.push_str("</div>");
            // Lightbox overlay
            html.push_str(concat!(
                "<div class=\"surfdoc-lightbox\" hidden>",
                "<button class=\"sl-close\" aria-label=\"Close\">&times;</button>",
                "<button class=\"sl-prev\" aria-label=\"Previous\">&#8249;</button>",
                "<button class=\"sl-next\" aria-label=\"Next\">&#8250;</button>",
                "<div class=\"sl-img-wrap\"><img class=\"sl-img\" src=\"\" alt=\"\" /></div>",
                "<div class=\"sl-caption\"></div>",
                "<div class=\"sl-counter\"></div>",
                "</div>",
            ));
            // Gallery filter JS
            if !categories.is_empty() {
                html.push_str(r#"<script>document.querySelectorAll('.surfdoc-gallery').forEach(g=>{g.querySelectorAll('.filter-btn').forEach(b=>{b.onclick=()=>{g.querySelectorAll('.filter-btn').forEach(e=>e.classList.remove('active'));b.classList.add('active');var f=b.dataset.filter;g.querySelectorAll('.surfdoc-gallery-item').forEach(i=>{i.style.display=f==='all'||i.dataset.category===f?'':'none'})}})})</script>"#);
            }
            // Lightbox JS
            html.push_str(r#"<script>document.querySelectorAll('.surfdoc-gallery').forEach(function(g){var lb=g.querySelector('.surfdoc-lightbox');if(!lb)return;var si=lb.querySelector('.sl-img'),sc=lb.querySelector('.sl-caption'),sn=lb.querySelector('.sl-counter'),items,idx;function vis(){return Array.prototype.filter.call(g.querySelectorAll('.surfdoc-gallery-item'),function(i){return i.style.display!=='none'})}function show(i){items=vis();idx=i;var f=items[idx],im=f.querySelector('img'),fc=f.querySelector('figcaption');si.src=im.src;si.alt=im.alt||'';sc.textContent=fc?fc.textContent:'';sn.textContent=(idx+1)+' / '+items.length;lb.hidden=false;document.body.style.overflow='hidden'}function hide(){lb.hidden=true;document.body.style.overflow=''}function nav(d){show((idx+d+items.length)%items.length)}g.querySelectorAll('.surfdoc-gallery-item').forEach(function(f){f.onclick=function(){var v=vis();show(v.indexOf(f))};f.onkeydown=function(e){if(e.key==='Enter'||e.key===' '){e.preventDefault();f.onclick()}}});lb.querySelector('.sl-close').onclick=hide;lb.querySelector('.sl-prev').onclick=function(){nav(-1)};lb.querySelector('.sl-next').onclick=function(){nav(1)};lb.onclick=function(e){if(e.target===lb)hide()};document.addEventListener('keydown',function(e){if(lb.hidden)return;if(e.key==='Escape')hide();if(e.key==='ArrowLeft')nav(-1);if(e.key==='ArrowRight')nav(1)});var tx;lb.addEventListener('touchstart',function(e){tx=e.touches[0].clientX});lb.addEventListener('touchend',function(e){var dx=e.changedTouches[0].clientX-tx;if(Math.abs(dx)>50)nav(dx>0?-1:1)})})</script>"#);
            html.push_str("</div>");
            html
        }

        Block::Footer {
            sections, copyright, social, brand, brand_reg, brand_logo, tagline, ..
        } => {
            // Rich shell footer: brand column + column grid + copyright,
            // matching the cloudsurf.com chrome. Opt-in via brand/logo/tagline.
            if brand.is_some() || brand_logo.is_some() || tagline.is_some() {
                let mut html = String::from("<footer class=\"surfdoc-shell-footer\"><div class=\"surfdoc-shell-footer-inner\"><div class=\"surfdoc-shell-footer-grid\">");
                html.push_str("<div class=\"surfdoc-shell-footer-brand-col\"><a href=\"/\" class=\"surfdoc-shell-footer-brand\">");
                html.push_str(&shell_brand_inner(brand_logo.as_deref(), brand.as_deref(), *brand_reg, "surfdoc-shell-footer-emblem"));
                html.push_str("</a>");
                if let Some(t) = tagline {
                    html.push_str(&format!("<p class=\"surfdoc-shell-footer-tagline\">{}</p>", escape_html(t)));
                }
                html.push_str("</div>");
                for section in sections {
                    html.push_str("<div class=\"surfdoc-shell-footer-col\">");
                    html.push_str(&format!("<h4 class=\"surfdoc-shell-footer-heading\">{}</h4>", escape_html(&section.heading)));
                    for link in &section.links {
                        let tgt = if link.external { " target=\"_blank\" rel=\"noopener\"" } else { "" };
                        if link.href.is_empty() {
                            html.push_str(&format!("<span>{}</span>", escape_html(&link.label)));
                        } else {
                            html.push_str(&format!(
                                "<a href=\"{}\"{}>{}</a>",
                                escape_html(&link.href),
                                tgt,
                                escape_html(&link.label),
                            ));
                        }
                    }
                    html.push_str("</div>");
                }
                html.push_str("</div>");
                if !social.is_empty() {
                    html.push_str("<div class=\"surfdoc-shell-footer-social\">");
                    for link in social {
                        html.push_str(&format!(
                            "<a href=\"{}\" class=\"social-link\" aria-label=\"{}\">{}</a>",
                            escape_html(&link.href),
                            escape_html(&link.platform),
                            escape_html(&link.platform),
                        ));
                    }
                    html.push_str("</div>");
                }
                if let Some(cr) = copyright {
                    html.push_str(&format!("<p class=\"surfdoc-shell-footer-copyright\">{}</p>", escape_html(cr)));
                }
                html.push_str("</div></footer>");
                return html;
            }
            let mut html = String::from("<footer class=\"surfdoc-footer\">");
            if !sections.is_empty() {
                html.push_str("<div class=\"surfdoc-footer-sections\">");
                for section in sections {
                    html.push_str("<div class=\"surfdoc-footer-col\">");
                    html.push_str(&format!("<strong class=\"surfdoc-footer-col-head\">{}</strong>", escape_html(&section.heading)));
                    html.push_str("<ul>");
                    for link in &section.links {
                        if link.href.is_empty() {
                            html.push_str(&format!("<li>{}</li>", escape_html(&link.label)));
                        } else {
                            html.push_str(&format!(
                                "<li><a href=\"{}\">{}</a></li>",
                                escape_html(&link.href),
                                escape_html(&link.label),
                            ));
                        }
                    }
                    html.push_str("</ul></div>");
                }
                html.push_str("</div>");
            }
            if !social.is_empty() {
                html.push_str("<div class=\"surfdoc-footer-social\">");
                for link in social {
                    html.push_str(&format!(
                        "<a href=\"{}\" class=\"social-link\" aria-label=\"{}\">{}</a>",
                        escape_html(&link.href),
                        escape_html(&link.platform),
                        escape_html(&link.platform),
                    ));
                }
                html.push_str("</div>");
            }
            if let Some(cr) = copyright {
                html.push_str(&format!(
                    "<div class=\"surfdoc-footer-copyright\">{}</div>",
                    escape_html(cr),
                ));
            }
            html.push_str("</footer>");
            html
        }

        Block::Details {
            title,
            open,
            content,
            ..
        } => {
            let open_attr = if *open { " open" } else { "" };
            let summary = title.as_deref().unwrap_or("Details");
            format!(
                "<details class=\"surfdoc-details\"{open_attr}>\
                 <summary class=\"surfdoc-details-summary\">{}</summary>\
                 <div class=\"surfdoc-details-body\">{}</div>\
                 </details>",
                escape_html(summary),
                render_markdown(content),
            )
        }

        Block::Divider { label, .. } => {
            match label {
                Some(text) => format!(
                    "<div class=\"surfdoc-divider\" role=\"separator\">\
                     <span>{}</span>\
                     </div>",
                    escape_html(text)
                ),
                None => "<hr class=\"surfdoc-divider-plain\" />".to_string(),
            }
        }

        Block::Hero {
            headline,
            subtitle,
            badge,
            align,
            image,
            image_alt,
            layout,
            transparent,
            buttons,
            content: _,
            ..
        } => {
            // `layout=cover` makes the image a full-bleed hero BACKGROUND (with
            // a darkening overlay + white text, see surfdoc.css §50) — the
            // home-page "header" look. `layout=stacked` forces the image above
            // the headline; otherwise the legacy align-driven placement applies
            // (centered → above, left → side).
            let cover = layout.as_deref() == Some("cover") && image.is_some();
            let stacked = layout.as_deref() == Some("stacked");
            let image_above = !cover && (stacked || align != "left");
            let image_side = !cover && !image_above;
            let align_cls = if align == "left" { " surfdoc-hero-left" } else { "" };
            let layout_cls = layout
                .as_deref()
                .map(|l| format!(" surfdoc-hero-{}", l))
                .unwrap_or_default();
            let transparent_cls = if *transparent { " surfdoc-hero-transparent" } else { "" };
            let alt = image_alt.as_deref().unwrap_or("");
            let cover_style = if cover {
                format!(
                    " style=\"background-image:url('{}')\"",
                    escape_html(image.as_deref().unwrap_or(""))
                )
            } else {
                String::new()
            };
            let mut parts = Vec::new();
            parts.push(format!("<section class=\"surfdoc-hero{}{}{}\"{}>", align_cls, layout_cls, transparent_cls, cover_style));
            parts.push("<div class=\"surfdoc-hero-inner\">".to_string());
            // Centered/stacked layout: image above text (logo/product image)
            if image_above {
                if let Some(img) = image {
                    parts.push(format!("<div class=\"surfdoc-hero-image\"><img src=\"{}\" alt=\"{}\" onerror=\"this.classList.add('broken');this.onerror=null\"></div>", escape_html(img), escape_html(alt)));
                }
            }
            if let Some(b) = badge {
                parts.push(format!("<span class=\"surfdoc-hero-badge\">{}</span>", escape_html(b)));
            }
            if let Some(h) = headline {
                parts.push(format!("<h1 class=\"surfdoc-hero-headline\">{}</h1>", render_inline_markdown_phrasing(h)));
            }
            if let Some(s) = subtitle {
                parts.push(format!("<p class=\"surfdoc-hero-subtitle\">{}</p>", render_inline_markdown_phrasing(s)));
            }
            if !buttons.is_empty() {
                parts.push("<div class=\"surfdoc-hero-actions\">".to_string());
                for btn in buttons {
                    let cls = if btn.primary { "surfdoc-hero-btn surfdoc-hero-btn-primary" } else { "surfdoc-hero-btn surfdoc-hero-btn-secondary" };
                    let target = if btn.external { " target=\"_blank\" rel=\"noopener\"" } else { "" };
                    parts.push(format!("<a href=\"{}\" class=\"{}\"{}>{}</a>", escape_html(&btn.href), cls, target, escape_html(&btn.label)));
                }
                parts.push("</div>".to_string());
            }
            parts.push("</div>".to_string());
            // Left-aligned layout: image to the side (side-by-side)
            if image_side {
                if let Some(img) = image {
                    parts.push(format!("<div class=\"surfdoc-hero-image-side\"><img src=\"{}\" alt=\"{}\" onerror=\"this.classList.add('broken');this.onerror=null\"></div>", escape_html(img), escape_html(alt)));
                }
            }
            parts.push("</section>".to_string());
            parts.join("")
        }

        Block::Features { cards, cols, .. } => {
            let col_attr = cols.map(|c| format!(" data-cols=\"{}\"", c)).unwrap_or_default();
            let mut parts = Vec::new();
            parts.push(format!("<div class=\"surfdoc-features\"{}>", col_attr));
            for card in cards {
                parts.push("<div class=\"surfdoc-feature-card\">".to_string());
                if let Some(icon) = &card.icon {
                    if let Some(svg) = get_icon(icon) {
                        parts.push(format!("<span class=\"surfdoc-feature-icon\">{}</span>", svg));
                    }
                }
                parts.push(format!("<h3 class=\"surfdoc-feature-title\">{}</h3>", render_inline_markdown_phrasing(&card.title)));
                if !card.body.is_empty() {
                    parts.push(format!("<p class=\"surfdoc-feature-body\">{}</p>", render_inline_markdown_phrasing(&card.body)));
                }
                if let (Some(label), Some(href)) = (&card.link_label, &card.link_href) {
                    parts.push(format!("<a href=\"{}\" class=\"surfdoc-feature-link\">{} \u{2192}</a>", escape_html(href), escape_html(label)));
                }
                parts.push("</div>".to_string());
            }
            parts.push("</div>".to_string());
            parts.join("")
        }

        Block::Steps { steps, .. } => {
            let mut parts = Vec::new();
            parts.push("<ol class=\"surfdoc-steps\">".to_string());
            for (i, step) in steps.iter().enumerate() {
                parts.push("<li class=\"surfdoc-step\">".to_string());
                parts.push(format!("<span class=\"surfdoc-step-number\">{}</span>", i + 1));
                parts.push("<div class=\"surfdoc-step-content\">".to_string());
                let time_html = step.time.as_ref().map(|t| format!("<span class=\"surfdoc-step-time\">{}</span>", escape_html(t))).unwrap_or_default();
                parts.push(format!("<h3 class=\"surfdoc-step-title\">{}{}</h3>", render_inline_markdown_phrasing(&step.title), time_html));
                if !step.body.is_empty() {
                    parts.push(format!("<p class=\"surfdoc-step-body\">{}</p>", render_inline_markdown_phrasing(&step.body)));
                }
                parts.push("</div>".to_string());
                parts.push("</li>".to_string());
            }
            parts.push("</ol>".to_string());
            parts.join("")
        }

        Block::Stats { items, .. } => {
            let mut parts = Vec::new();
            parts.push("<div class=\"surfdoc-stats\">".to_string());
            for item in items {
                let style = item.color.as_ref().map(|c| format!(" style=\"color:{}\"", escape_html(c))).unwrap_or_default();
                parts.push(format!(
                    "<div class=\"surfdoc-stat\"><span class=\"surfdoc-stat-value\"{}>{}</span><span class=\"surfdoc-stat-label\">{}</span></div>",
                    style, escape_html(&item.value), escape_html(&item.label)
                ));
            }
            parts.push("</div>".to_string());
            parts.join("")
        }

        Block::Comparison {
            headers,
            rows,
            highlight,
            ..
        } => {
            let mut parts = Vec::new();
            parts.push("<div class=\"surfdoc-table-wrap\"><table class=\"surfdoc-comparison\">".to_string());
            parts.push("<thead><tr>".to_string());
            for h in headers {
                let cls = if highlight.as_deref() == Some(h.as_str()) { " class=\"surfdoc-comparison-highlight\"" } else { "" };
                parts.push(format!("<th{}>{}</th>", cls, render_cell_inline_markdown(h)));
            }
            parts.push("</tr></thead>".to_string());
            parts.push("<tbody>".to_string());
            for row in rows {
                parts.push("<tr>".to_string());
                for (i, cell) in row.iter().enumerate() {
                    let cls = if headers.get(i).and_then(|h| highlight.as_ref().map(|hi| h == hi)).unwrap_or(false) {
                        " class=\"surfdoc-comparison-highlight\""
                    } else {
                        ""
                    };
                    let rendered = comparison_cell(cell);
                    parts.push(format!("<td{}>{}</td>", cls, rendered));
                }
                parts.push("</tr>".to_string());
            }
            parts.push("</tbody></table></div>".to_string());
            parts.join("")
        }

        Block::Logo { src, alt, size, .. } => {
            let alt_attr = alt.as_ref().map(|a| escape_html(a)).unwrap_or_default();
            let style = size.map(|s| format!(" style=\"max-width:{}px\"", s)).unwrap_or_default();
            format!(
                "<div class=\"surfdoc-logo\"><img src=\"{}\" alt=\"{}\"{}></div>",
                escape_html(src), alt_attr, style
            )
        }

        Block::Toc { depth, entries, .. } => {
            if entries.is_empty() {
                format!("<nav class=\"surfdoc-toc\" data-depth=\"{}\"></nav>", depth)
            } else {
                let items: Vec<(u32, String, String)> = entries
                    .iter()
                    .map(|e| (e.level, e.id.clone(), e.text.clone()))
                    .collect();
                let refs: Vec<&(u32, String, String)> = items.iter().collect();
                format!(
                    "<nav class=\"surfdoc-toc\" data-depth=\"{}\"><div class=\"surfdoc-toc-label\">Contents</div>{}</nav>",
                    depth,
                    toc_nested_ol(&refs)
                )
            }
        }

        Block::BeforeAfter {
            before_items,
            after_items,
            transition,
            ..
        } => {
            let mut parts = Vec::new();
            parts.push("<div class=\"surfdoc-before-after\">".to_string());
            parts.push("<div class=\"surfdoc-ba-before\">".to_string());
            parts.push("<h3 class=\"surfdoc-ba-heading\">Before</h3>".to_string());
            for item in before_items {
                parts.push(format!(
                    "<div class=\"surfdoc-ba-item\"><span class=\"surfdoc-ba-dot surfdoc-ba-dot-red\"></span><strong>{}</strong><span>{}</span></div>",
                    escape_html(&item.label),
                    escape_html(&item.detail)
                ));
            }
            parts.push("</div>".to_string());
            if let Some(t) = transition {
                parts.push(format!(
                    "<div class=\"surfdoc-ba-transition\"><span class=\"surfdoc-ba-line\"></span><span class=\"surfdoc-ba-label\">{}</span><span class=\"surfdoc-ba-line\"></span></div>",
                    escape_html(t)
                ));
            }
            parts.push("<div class=\"surfdoc-ba-after\">".to_string());
            parts.push("<h3 class=\"surfdoc-ba-heading\">After</h3>".to_string());
            for item in after_items {
                parts.push(format!(
                    "<div class=\"surfdoc-ba-item surfdoc-ba-item-green\"><span class=\"surfdoc-ba-dot surfdoc-ba-dot-green\"></span><strong>{}</strong><span>{}</span></div>",
                    escape_html(&item.label),
                    escape_html(&item.detail)
                ));
            }
            parts.push("</div>".to_string());
            parts.push("</div>".to_string());
            parts.join("")
        }

        Block::Pipeline { steps, .. } => {
            let mut parts = Vec::new();
            parts.push("<div class=\"surfdoc-pipeline\">".to_string());
            for (i, step) in steps.iter().enumerate() {
                if i > 0 {
                    parts.push("<span class=\"surfdoc-pipeline-arrow\">\u{2192}</span>".to_string());
                }
                parts.push("<div class=\"surfdoc-pipeline-step\">".to_string());
                parts.push(format!("<strong class=\"surfdoc-pipeline-label\">{}</strong>", escape_html(&step.label)));
                if let Some(desc) = &step.description {
                    parts.push(format!("<span class=\"surfdoc-pipeline-desc\">{}</span>", escape_html(desc)));
                }
                parts.push("</div>".to_string());
            }
            parts.push("</div>".to_string());
            parts.join("")
        }

        Block::Section {
            bg,
            headline,
            subtitle,
            children,
            ..
        } => {
            let bg_cls = bg.as_ref().map(|b| format!(" section-{}", escape_html(b))).unwrap_or_default();
            let mut html = format!("<section class=\"surfdoc-section{bg_cls}\">");
            html.push_str("<div class=\"surfdoc-section-inner\">");
            if headline.is_some() || subtitle.is_some() {
                html.push_str("<div class=\"surfdoc-section-header\">");
                if let Some(h) = headline {
                    html.push_str(&format!("<h2>{}</h2>", render_inline_markdown_phrasing(h)));
                }
                if let Some(s) = subtitle {
                    html.push_str(&format!("<p>{}</p>", render_inline_markdown_phrasing(s)));
                }
                html.push_str("</div>");
            }
            for child in children {
                html.push_str(&render_block(child));
            }
            html.push_str("</div>");
            html.push_str("</section>");
            html
        }

        Block::ProductCard {
            title,
            subtitle,
            badge,
            badge_color,
            body,
            features,
            cta_label,
            cta_href,
            ..
        } => {
            let mut parts = Vec::new();
            parts.push("<div class=\"surfdoc-product-card\">".to_string());
            parts.push("<div class=\"surfdoc-product-header\">".to_string());
            parts.push("<div class=\"surfdoc-product-titles\">".to_string());
            parts.push(format!("<h3 class=\"surfdoc-product-title\">{}</h3>", escape_html(title)));
            if let Some(s) = subtitle {
                parts.push(format!("<p class=\"surfdoc-product-subtitle\">{}</p>", render_inline_markdown_phrasing(s)));
            }
            parts.push("</div>".to_string());
            if let Some(b) = badge {
                let color_cls = badge_color.as_ref().map(|c| format!(" surfdoc-badge-{}", escape_html(c))).unwrap_or_default();
                parts.push(format!("<span class=\"surfdoc-badge{color_cls}\">{}</span>", escape_html(b)));
            }
            parts.push("</div>".to_string());
            if !body.is_empty() {
                parts.push(format!("<div class=\"surfdoc-product-body\">{}</div>", render_markdown(body)));
            }
            if !features.is_empty() {
                parts.push("<ul class=\"surfdoc-product-features\">".to_string());
                for f in features {
                    parts.push(format!("<li>{}</li>", render_inline_markdown_phrasing(f)));
                }
                parts.push("</ul>".to_string());
            }
            if let (Some(label), Some(href)) = (cta_label, cta_href) {
                parts.push(format!(
                    "<a href=\"{}\" class=\"surfdoc-product-cta\">{}</a>",
                    escape_html(href),
                    escape_html(label)
                ));
            }
            parts.push("</div>".to_string());
            parts.join("")
        }

        // ----- App description blocks -----

        Block::List {
            source,
            display,
            item_template,
            filters,
            sort,
            preload,
            ..
        } => {
            let display_cls = match display {
                ListDisplay::Card => "card",
                ListDisplay::Table => "table",
                ListDisplay::Compact => "compact",
            };
            let mut html = format!(
                "<div class=\"surfdoc-list surfdoc-list-{}\" data-surf-source=\"{}\"",
                display_cls,
                escape_html(source),
            );
            if *preload {
                html.push_str(" data-surf-preload");
            }
            if let Some(s) = sort {
                html.push_str(&format!(
                    " data-surf-sort=\"{}\" data-surf-sort-dir=\"{}\"",
                    escape_html(&s.field),
                    if s.descending { "desc" } else { "asc" },
                ));
            }
            html.push('>');
            // Render filter controls
            if !filters.is_empty() {
                html.push_str("<div class=\"surfdoc-list-filters\">");
                for f in filters {
                    html.push_str(&format!(
                        "<label class=\"surfdoc-list-filter\">{}</label>",
                        escape_html(&f.field),
                    ));
                }
                html.push_str("</div>");
            }
            // Item template stored as data attribute for runtime
            if !item_template.is_empty() {
                html.push_str(&format!(
                    "<template class=\"surfdoc-list-item-template\">{}</template>",
                    escape_html(item_template),
                ));
            }
            // Static preview: sample cards so the block looks complete without a live API
            html.push_str("<div class=\"surfdoc-list-items\" aria-live=\"polite\">\
              <div class=\"surfdoc-list-item surfdoc-list-item-sample\">\
                <strong class=\"surfdoc-list-item-title\">Design API schema</strong>\
                <span class=\"surfdoc-list-item-meta\">Status: In Progress &middot; Assigned: Taylor</span>\
              </div>\
              <div class=\"surfdoc-list-item surfdoc-list-item-sample\">\
                <strong class=\"surfdoc-list-item-title\">Write integration tests</strong>\
                <span class=\"surfdoc-list-item-meta\">Status: Todo &middot; Assigned: Alex</span>\
              </div>\
              <div class=\"surfdoc-list-item surfdoc-list-item-sample\">\
                <strong class=\"surfdoc-list-item-title\">Deploy to staging</strong>\
                <span class=\"surfdoc-list-item-meta\">Status: Done &middot; Assigned: Jordan</span>\
              </div>\
            </div>");
            html.push_str("</div>");
            html
        }

        Block::Board {
            source,
            columns,
            card_template,
            preload,
            ..
        } => {
            let mut html = format!(
                "<div class=\"surfdoc-board surfdoc-board-preview\" data-surf-source=\"{}\"",
                escape_html(source),
            );
            if *preload {
                html.push_str(" data-surf-preload");
            }
            html.push('>');
            if let Some(tmpl) = card_template {
                html.push_str(&format!(
                    "<template class=\"surfdoc-board-card-template\">{}</template>",
                    escape_html(tmpl),
                ));
            }
            // Sample cards per column for preview state
            let sample_cards: &[(&str, &[&str])] = &[
                ("Todo", &["Design API schema", "Write integration tests"]),
                ("In Progress", &["Build board renderer"]),
                ("Done", &["Set up CI/CD", "Deploy to staging"]),
            ];
            html.push_str("<div class=\"surfdoc-board-columns\">");
            let col_list: Vec<&str> = columns.iter().map(|c| c.as_str()).collect();
            // If author gave explicit columns use them; otherwise fall back to sample labels
            let effective_cols: Vec<(&str, &[&str])> = if col_list.is_empty() {
                sample_cards.to_vec()
            } else {
                col_list.iter().enumerate().map(|(i, &col)| {
                    let cards = sample_cards.get(i).map(|&(_, cards)| cards).unwrap_or(&[]);
                    (col, cards)
                }).collect()
            };
            for (col, cards) in &effective_cols {
                html.push_str(&format!(
                    "<div class=\"surfdoc-board-column\" data-column=\"{}\"><h3 class=\"surfdoc-board-column-title\">{}</h3><div class=\"surfdoc-board-cards\" aria-live=\"polite\">",
                    escape_html(col),
                    escape_html(col),
                ));
                for card in *cards {
                    html.push_str(&format!(
                        "<div class=\"surfdoc-board-card surfdoc-board-card-sample\">{}</div>",
                        escape_html(card),
                    ));
                }
                html.push_str("</div></div>");
            }
            html.push_str("</div></div>");
            html
        }

        Block::Action {
            method,
            target,
            label,
            fields,
            confirm,
            ..
        } => {
            let method_str = match method {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Put => "put",
                HttpMethod::Patch => "patch",
                HttpMethod::Delete => "delete",
            };
            let mut html = format!(
                "<form class=\"surfdoc-action\" method=\"{}\" action=\"{}\" data-surf-method=\"{}\" data-surf-action=\"{}\"",
                method_str,
                escape_html(target),
                method_str,
                escape_html(target),
            );
            if let Some(c) = confirm {
                html.push_str(&format!(" data-surf-confirm=\"{}\"", escape_html(c)));
            }
            html.push('>');
            for field in fields {
                let req = if field.required { " required" } else { "" };
                let req_star = if field.required { " <span class=\"required\">*</span>" } else { "" };
                html.push_str(&format!(
                    "<div class=\"surfdoc-form-field\"><label>{}{}</label>",
                    escape_html(&field.label),
                    req_star,
                ));
                match field.field_type {
                    FormFieldType::Textarea => {
                        let ph = field.placeholder.as_deref().unwrap_or("");
                        html.push_str(&format!(
                            "<textarea name=\"{}\" placeholder=\"{}\" rows=\"4\"{}></textarea>",
                            escape_html(&field.name),
                            escape_html(ph),
                            req,
                        ));
                    }
                    FormFieldType::Select => {
                        html.push_str(&format!(
                            "<select name=\"{}\"{}>",
                            escape_html(&field.name),
                            req,
                        ));
                        html.push_str("<option value=\"\">Select...</option>");
                        for opt in &field.options {
                            html.push_str(&format!(
                                "<option value=\"{}\">{}</option>",
                                escape_html(opt),
                                escape_html(opt),
                            ));
                        }
                        html.push_str("</select>");
                    }
                    _ => {
                        let input_type = match field.field_type {
                            FormFieldType::Email => "email",
                            FormFieldType::Tel => "tel",
                            FormFieldType::Date => "date",
                            FormFieldType::Number => "number",
                            FormFieldType::Password => "password",
                            _ => "text",
                        };
                        let ph = field.placeholder.as_deref().unwrap_or("");
                        html.push_str(&format!(
                            "<input type=\"{}\" name=\"{}\" placeholder=\"{}\"{}/>",
                            input_type,
                            escape_html(&field.name),
                            escape_html(ph),
                            req,
                        ));
                    }
                }
                html.push_str("</div>");
            }
            html.push_str(&format!(
                "<button type=\"submit\" class=\"surfdoc-cta surfdoc-cta-primary\">{}</button>",
                escape_html(label),
            ));
            html.push_str("</form>");
            html
        }

        Block::FilterBar {
            target_selector,
            fields,
            ..
        } => {
            let mut html = format!(
                "<div class=\"surfdoc-filter-bar\" data-surf-target=\"{}\">",
                escape_html(target_selector),
            );
            if fields.is_empty() {
                // Static preview: sample filter chips so the bar is never empty
                html.push_str("\
                  <label class=\"surfdoc-filter-field\">Status\
                    <select name=\"status\">\
                      <option value=\"all\">All</option>\
                      <option value=\"todo\">Todo</option>\
                      <option value=\"done\">Done</option>\
                    </select>\
                  </label>\
                  <label class=\"surfdoc-filter-field\">Assignee\
                    <select name=\"assignee\">\
                      <option value=\"all\">All</option>\
                      <option value=\"me\">Me</option>\
                    </select>\
                  </label>");
            } else {
                for field in fields {
                    html.push_str(&format!(
                        "<label class=\"surfdoc-filter-field\">{}<select name=\"{}\">",
                        escape_html(&field.label),
                        escape_html(&field.name),
                    ));
                    for opt in &field.options {
                        html.push_str(&format!(
                            "<option value=\"{}\">{}</option>",
                            escape_html(opt),
                            escape_html(opt),
                        ));
                    }
                    html.push_str("</select></label>");
                }
            }
            html.push_str("</div>");
            html
        }

        Block::Search {
            source,
            placeholder,
            ..
        } => {
            let ph = placeholder.as_deref().unwrap_or("Search...");
            format!(
                "<div class=\"surfdoc-search\" data-surf-source=\"{}\"><input type=\"search\" placeholder=\"{}\" aria-label=\"{}\" autocomplete=\"off\"/><div class=\"surfdoc-search-results\" aria-live=\"polite\"></div></div>",
                escape_html(source),
                escape_html(ph),
                escape_html(ph),
            )
        }

        Block::Dashboard {
            source, refresh, ..
        } => {
            let mut html = format!(
                "<div class=\"surfdoc-dashboard surfdoc-dashboard-preview\" data-surf-source=\"{}\"",
                escape_html(source),
            );
            if let Some(r) = refresh {
                html.push_str(&format!(" data-surf-refresh=\"{}\"", r));
            }
            html.push_str(">\
                <div class=\"surfdoc-dashboard-grid\" aria-live=\"polite\">\
                  <div class=\"surfdoc-dashboard-tile\">\
                    <span class=\"surfdoc-dashboard-tile-value\">1,284</span>\
                    <span class=\"surfdoc-dashboard-tile-label\">Active users</span>\
                    <span class=\"surfdoc-dashboard-tile-trend surfdoc-trend--up\">&uarr; 12%</span>\
                  </div>\
                  <div class=\"surfdoc-dashboard-tile\">\
                    <span class=\"surfdoc-dashboard-tile-value\">$4,820</span>\
                    <span class=\"surfdoc-dashboard-tile-label\">MRR</span>\
                    <span class=\"surfdoc-dashboard-tile-trend surfdoc-trend--up\">&uarr; 8%</span>\
                  </div>\
                  <div class=\"surfdoc-dashboard-tile\">\
                    <span class=\"surfdoc-dashboard-tile-value\">98.2%</span>\
                    <span class=\"surfdoc-dashboard-tile-label\">Uptime</span>\
                    <span class=\"surfdoc-dashboard-tile-trend surfdoc-trend--flat\">&mdash;</span>\
                  </div>\
                  <div class=\"surfdoc-dashboard-tile\">\
                    <span class=\"surfdoc-dashboard-tile-value\">142ms</span>\
                    <span class=\"surfdoc-dashboard-tile-label\">P95 latency</span>\
                    <span class=\"surfdoc-dashboard-tile-trend surfdoc-trend--down\">&darr; 5%</span>\
                  </div>\
                </div>\
                </div>");
            html
        }

        Block::ChatInput {
            action,
            placeholder,
            modes,
            ..
        } => {
            let ph = placeholder.as_deref().unwrap_or("Type a message...");
            let mut html = format!(
                "<div class=\"surfdoc-chat-input\" data-surf-action=\"{}\">",
                escape_html(action),
            );
            if !modes.is_empty() {
                html.push_str("<div class=\"surfdoc-chat-modes\">");
                for (i, mode) in modes.iter().enumerate() {
                    let active = if i == 0 { " active" } else { "" };
                    html.push_str(&format!(
                        "<button type=\"button\" class=\"surfdoc-chat-mode{}\" data-mode=\"{}\">{}</button>",
                        active,
                        escape_html(mode),
                        escape_html(mode),
                    ));
                }
                html.push_str("</div>");
            }
            html.push_str(&format!(
                "<form class=\"surfdoc-chat-form\"><input type=\"text\" placeholder=\"{}\" aria-label=\"{}\" autocomplete=\"off\"/><button type=\"submit\">Send</button></form>",
                escape_html(ph),
                escape_html(ph),
            ));
            html.push_str("</div>");
            html
        }

        Block::Feed {
            source, stream, ..
        } => {
            let stream_attr = if *stream { " data-surf-stream" } else { "" };
            format!(
                "<div class=\"surfdoc-feed surfdoc-feed-preview\" data-surf-source=\"{}\"{}>\
                 <div class=\"surfdoc-feed-items\" aria-live=\"polite\">\
                   <div class=\"surfdoc-feed-item\"><span class=\"surfdoc-feed-avatar\">JD</span><div class=\"surfdoc-feed-body\"><span class=\"surfdoc-feed-name\">Jane Doe</span><span class=\"surfdoc-feed-text\">Merged PR #142: Add board renderer</span><span class=\"surfdoc-feed-time\">2m ago</span></div></div>\
                   <div class=\"surfdoc-feed-item\"><span class=\"surfdoc-feed-avatar\">BM</span><div class=\"surfdoc-feed-body\"><span class=\"surfdoc-feed-name\">Brady M</span><span class=\"surfdoc-feed-text\">Deployed v1.4.0 to production</span><span class=\"surfdoc-feed-time\">18m ago</span></div></div>\
                   <div class=\"surfdoc-feed-item\"><span class=\"surfdoc-feed-avatar\">AI</span><div class=\"surfdoc-feed-body\"><span class=\"surfdoc-feed-name\">Mako</span><span class=\"surfdoc-feed-text\">Generated 3 task descriptions</span><span class=\"surfdoc-feed-time\">1h ago</span></div></div>\
                 </div>\
                 </div>",
                escape_html(source),
                stream_attr,
            )
        }

        Block::Store {
            title,
            currency,
            items,
            ..
        } => {
            let js = |s: &str| -> String {
                let mut o = String::from("\"");
                for c in s.chars() {
                    match c {
                        '"' => o.push_str("\\\""),
                        '\\' => o.push_str("\\\\"),
                        '\n' => o.push_str("\\n"),
                        '\r' => o.push_str("\\r"),
                        '\t' => o.push_str("\\t"),
                        '<' => o.push_str("\\u003c"),
                        '&' => o.push_str("\\u0026"),
                        c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                        c => o.push(c),
                    }
                }
                o.push('"');
                o
            };
            let opt = |v: &Option<String>| -> String {
                v.as_deref().map(js).unwrap_or_else(|| "null".to_string())
            };
            let cur = currency.as_deref().unwrap_or("$");
            let items_json = items
                .iter()
                .map(|it| {
                    format!(
                        "{{\"name\":{},\"price\":{},\"blurb\":{},\"badge\":{},\"category\":{}}}",
                        js(&it.name),
                        js(&it.price),
                        opt(&it.blurb),
                        opt(&it.badge),
                        opt(&it.category),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let data_json = format!("{{\"currency\":{},\"items\":[{items_json}]}}", js(cur));

            let mut html = String::from("<div class=\"surfdoc-store\" data-store>");
            if let Some(t) = title {
                html.push_str(&format!(
                    "<div class=\"surfdoc-store-head\">{}</div>",
                    escape_html(t)
                ));
            }
            html.push_str(
                "<div class=\"surfdoc-store-layout\">\
                   <div class=\"surfdoc-store-main\">\
                     <div class=\"surfdoc-store-filters\" data-st-filters></div>\
                     <div class=\"surfdoc-store-grid\" data-st-grid></div>\
                   </div>\
                   <aside class=\"surfdoc-store-cart\" data-st-cart aria-label=\"Cart\">\
                     <div class=\"surfdoc-store-cart-head\">Your cart</div>\
                     <div class=\"surfdoc-store-cart-items\" data-st-items aria-live=\"polite\"></div>\
                     <div class=\"surfdoc-store-cart-foot\">\
                       <div class=\"surfdoc-store-total\">Total <span data-st-total></span></div>\
                       <button type=\"button\" class=\"surfdoc-store-checkout-btn\" data-st-checkout disabled>Checkout</button>\
                     </div>\
                   </aside>\
                 </div>",
            );
            html.push_str(
                "<form class=\"surfdoc-store-form\" data-st-form hidden>\
                   <div class=\"surfdoc-store-form-title\">Checkout</div>\
                   <label class=\"surfdoc-store-field\">Name<input type=\"text\" name=\"name\" autocomplete=\"name\" required></label>\
                   <label class=\"surfdoc-store-field\">Email<input type=\"email\" name=\"email\" autocomplete=\"email\" required></label>\
                   <label class=\"surfdoc-store-field\">Shipping address<input type=\"text\" name=\"address\" autocomplete=\"street-address\" required></label>\
                   <button type=\"submit\" class=\"surfdoc-store-place-btn\">Place order</button>\
                 </form>\
                 <div class=\"surfdoc-store-confirm\" data-st-confirm hidden aria-live=\"polite\"></div>",
            );
            html.push_str(&format!(
                "<script type=\"application/json\" data-st-data>{data_json}</script>"
            ));
            html.push_str(STORE_WIDGET_JS);
            html.push_str("</div>");
            html
        }

        Block::Booking {
            title,
            service_label,
            services,
            days,
            ..
        } => {
            // JSON-encode a string for safe embedding in an inline
            // <script type="application/json"> (escape quote/backslash/control
            // chars plus `<` and `&` to defuse any `</script>`/entity tricks).
            let js = |s: &str| -> String {
                let mut o = String::from("\"");
                for c in s.chars() {
                    match c {
                        '"' => o.push_str("\\\""),
                        '\\' => o.push_str("\\\\"),
                        '\n' => o.push_str("\\n"),
                        '\r' => o.push_str("\\r"),
                        '\t' => o.push_str("\\t"),
                        '<' => o.push_str("\\u003c"),
                        '&' => o.push_str("\\u0026"),
                        c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                        c => o.push(c),
                    }
                }
                o.push('"');
                o
            };
            let opt = |v: &Option<String>| -> String {
                v.as_deref().map(js).unwrap_or_else(|| "null".to_string())
            };

            let services_json = services
                .iter()
                .map(|s| {
                    format!(
                        "{{\"name\":{},\"duration\":{},\"price\":{}}}",
                        js(&s.name),
                        opt(&s.duration),
                        opt(&s.price),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let days_json = days
                .iter()
                .map(|d| {
                    let slots = d
                        .slots
                        .iter()
                        .map(|s| js(s))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{{\"date\":{},\"slots\":[{}]}}", js(&d.date), slots)
                })
                .collect::<Vec<_>>()
                .join(",");
            let data_json =
                format!("{{\"services\":[{services_json}],\"days\":[{days_json}]}}");

            let mut html = String::from("<div class=\"surfdoc-booking\" data-booking>");
            if let Some(t) = title {
                html.push_str(&format!(
                    "<div class=\"surfdoc-booking-head\">{}</div>",
                    escape_html(t)
                ));
            }
            html.push_str("<div class=\"surfdoc-booking-grid\">");
            if !services.is_empty() {
                let label = service_label.as_deref().unwrap_or("Service");
                html.push_str(&format!(
                    "<div class=\"surfdoc-booking-col surfdoc-booking-svc-col\"><div class=\"surfdoc-booking-label\">{}</div><div class=\"surfdoc-booking-services\" data-bk-services role=\"radiogroup\" aria-label=\"{}\"></div></div>",
                    escape_html(label),
                    escape_html(label),
                ));
            }
            html.push_str(
                "<div class=\"surfdoc-booking-col surfdoc-booking-cal\">\
                   <div class=\"surfdoc-booking-cal-head\">\
                     <button type=\"button\" class=\"surfdoc-booking-nav\" data-bk-prev aria-label=\"Previous month\">&lsaquo;</button>\
                     <span class=\"surfdoc-booking-month\" data-bk-month aria-live=\"polite\"></span>\
                     <button type=\"button\" class=\"surfdoc-booking-nav\" data-bk-next aria-label=\"Next month\">&rsaquo;</button>\
                   </div>\
                   <div class=\"surfdoc-booking-weekdays\"><span>Sun</span><span>Mon</span><span>Tue</span><span>Wed</span><span>Thu</span><span>Fri</span><span>Sat</span></div>\
                   <div class=\"surfdoc-booking-days\" data-bk-days></div>\
                 </div>\
                 <div class=\"surfdoc-booking-col surfdoc-booking-slots\" data-bk-slots>\
                   <p class=\"surfdoc-booking-hint\">Select an available date to see open times.</p>\
                 </div>",
            );
            html.push_str("</div>"); // .surfdoc-booking-grid
            html.push_str(
                "<form class=\"surfdoc-booking-form\" data-bk-form hidden>\
                   <div class=\"surfdoc-booking-summary\" data-bk-summary></div>\
                   <label class=\"surfdoc-booking-field\">Name<input type=\"text\" name=\"name\" autocomplete=\"name\" required></label>\
                   <label class=\"surfdoc-booking-field\">Email<input type=\"email\" name=\"email\" autocomplete=\"email\" required></label>\
                   <button type=\"submit\" class=\"surfdoc-booking-confirm-btn\">Confirm booking</button>\
                 </form>\
                 <div class=\"surfdoc-booking-confirm\" data-bk-confirm hidden aria-live=\"polite\"></div>",
            );
            html.push_str(&format!(
                "<script type=\"application/json\" data-bk-data>{data_json}</script>"
            ));
            html.push_str(BOOKING_WIDGET_JS);
            html.push_str("</div>");
            html
        }

        // Compound widget mount points

        Block::Editor {
            source, lang, preview, ..
        } => {
            let mut html = String::from("<div class=\"surfdoc-editor\"");
            if let Some(s) = source {
                html.push_str(&format!(" data-surf-source=\"{}\"", escape_html(s)));
            }
            if let Some(l) = lang {
                html.push_str(&format!(" data-lang=\"{}\"", escape_html(l)));
            }
            if *preview {
                html.push_str(" data-preview");
            }
            html.push_str(">\
              <div class=\"surfdoc-editor-preview\">\
                <div class=\"surfdoc-block-editor-block surfdoc-block-editor-h1\">Untitled document</div>\
                <div class=\"surfdoc-block-editor-block surfdoc-block-editor-p\">Start writing or press <kbd>/</kbd> to add a block&hellip;</div>\
              </div></div>");
            html
        }

        Block::Chart {
            chart_type, source, period, title, data, ..
        } => {
            let type_str = chart_type_str(*chart_type);
            // Inline data → render a real deterministic SVG chart.
            if let Some(d) = data {
                let svg = crate::chart::render_svg(*chart_type, d, title.as_deref());
                let caption = match title {
                    Some(t) => format!(
                        "<figcaption class=\"surfdoc-chart-cap\">{}</figcaption>",
                        escape_html(t)
                    ),
                    None => String::new(),
                };
                return format!(
                    "<figure class=\"surfdoc-chart surfdoc-chart-{type_str}\" data-chart-type=\"{type_str}\">{caption}{svg}</figure>",
                );
            }
            // No inline data → keep the live-data mount point / static placeholder.
            let mut html = format!(
                "<div class=\"surfdoc-chart surfdoc-chart-preview\" data-chart-type=\"{}\" data-surf-source=\"{}\"",
                type_str,
                escape_html(source),
            );
            if let Some(p) = period {
                html.push_str(&format!(" data-period=\"{}\"", escape_html(p)));
            }
            // Static preview: SVG sparkline axes + a sample polyline
            html.push_str(">\
                <div class=\"surfdoc-chart-header\">\
                  <span class=\"surfdoc-chart-type-label\">");
            html.push_str(type_str);
            html.push_str(" chart</span>\
                  <span class=\"surfdoc-chart-source-label\">");
            html.push_str(&escape_html(source));
            html.push_str("</span>\
                </div>\
                <svg class=\"surfdoc-chart-svg\" viewBox=\"0 0 320 120\" xmlns=\"http://www.w3.org/2000/svg\" aria-label=\"Sample chart preview\">\
                  <line x1=\"32\" y1=\"8\" x2=\"32\" y2=\"100\" stroke=\"currentColor\" stroke-width=\"1\" opacity=\"0.25\"/>\
                  <line x1=\"32\" y1=\"100\" x2=\"312\" y2=\"100\" stroke=\"currentColor\" stroke-width=\"1\" opacity=\"0.25\"/>\
                  <text x=\"28\" y=\"104\" font-size=\"9\" fill=\"currentColor\" opacity=\"0.4\" text-anchor=\"end\">0</text>\
                  <text x=\"28\" y=\"70\" font-size=\"9\" fill=\"currentColor\" opacity=\"0.4\" text-anchor=\"end\">50</text>\
                  <text x=\"28\" y=\"36\" font-size=\"9\" fill=\"currentColor\" opacity=\"0.4\" text-anchor=\"end\">100</text>\
                  <polyline points=\"40,80 80,60 120,70 160,40 200,55 240,30 280,45 310,20\" fill=\"none\" stroke=\"var(--accent)\" stroke-width=\"2\" stroke-linejoin=\"round\"/>\
                  <polyline points=\"40,80 80,60 120,70 160,40 200,55 240,30 280,45 310,20 310,100 40,100\" fill=\"var(--accent)\" opacity=\"0.08\" stroke=\"none\"/>\
                  <circle cx=\"310\" cy=\"20\" r=\"3\" fill=\"var(--accent)\"/>\
                </svg>\
                </div>");
            html
        }

        Block::SplitPane { ratio, .. } => {
            format!(
                "<div class=\"surfdoc-split-pane\" data-ratio=\"{}\"><div class=\"surfdoc-split-left\"></div><div class=\"surfdoc-split-right\"></div></div>",
                escape_html(ratio),
            )
        }

        // ----- Infrastructure manifest blocks -----

        Block::App { name, binary, region, port, platform, children, .. } => {
            let mut meta = vec![format!("<strong>{}</strong>", escape_html(name))];
            if let Some(b) = binary { meta.push(format!("binary: {}", escape_html(b))); }
            if let Some(r) = region { meta.push(format!("region: {}", escape_html(r))); }
            if let Some(p) = port { meta.push(format!("port: {p}")); }
            if let Some(pl) = platform { meta.push(format!("platform: {}", escape_html(pl))); }
            let mut html = format!(
                "<section class=\"surfdoc-app\" aria-label=\"App: {}\"><div class=\"surfdoc-app-header\">{}</div>",
                escape_html(name), meta.join(" &middot; "),
            );
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</section>");
            html
        }

        Block::Build { base, runtime, edition, properties, .. } => {
            let mut items = Vec::new();
            if let Some(b) = base { items.push(format!("<li>base: {}</li>", escape_html(b))); }
            if let Some(r) = runtime { items.push(format!("<li>runtime: {}</li>", escape_html(r))); }
            if let Some(e) = edition { items.push(format!("<li>edition: {}</li>", escape_html(e))); }
            for p in properties { items.push(format!("<li>{}: {}</li>", escape_html(&p.key), escape_html(&p.value))); }
            format!("<div class=\"surfdoc-infra-card surfdoc-build\"><strong class=\"surfdoc-infra-label\">Build</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::InfraDatabase { name, shared_auth, volume_gb, properties, .. } => {
            let mut items = Vec::new();
            if let Some(n) = name { items.push(format!("<li>name: {}</li>", escape_html(n))); }
            if *shared_auth { items.push("<li>shared_auth: true</li>".to_string()); }
            if let Some(v) = volume_gb { items.push(format!("<li>volume: {v} GB</li>")); }
            for p in properties { items.push(format!("<li>{}: {}</li>", escape_html(&p.key), escape_html(&p.value))); }
            format!("<div class=\"surfdoc-infra-card surfdoc-database\"><strong class=\"surfdoc-infra-label\">Database</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Deploy { env, app, machines, memory, auto_stop, min_machines, strategy, properties, .. } => {
            let env_str = env.as_deref().unwrap_or("unknown");
            let mut items = Vec::new();
            if let Some(a) = app { items.push(format!("<li>app: {}</li>", escape_html(a))); }
            if let Some(m) = machines { items.push(format!("<li>machines: {m}</li>")); }
            if let Some(m) = memory { items.push(format!("<li>memory: {m} MB</li>")); }
            if let Some(a) = auto_stop { items.push(format!("<li>auto_stop: {}</li>", escape_html(a))); }
            if let Some(m) = min_machines { items.push(format!("<li>min_machines: {m}</li>")); }
            if let Some(s) = strategy { items.push(format!("<li>strategy: {}</li>", escape_html(s))); }
            for p in properties { items.push(format!("<li>{}: {}</li>", escape_html(&p.key), escape_html(&p.value))); }
            format!(
                "<div class=\"surfdoc-infra-card surfdoc-deploy\"><strong class=\"surfdoc-infra-label\">Deploy</strong><span class=\"surfdoc-deploy-badge\">{}</span><ul>{}</ul></div>",
                escape_html(env_str), items.join(""),
            )
        }

        Block::InfraEnv { tier, entries, .. } => {
            let tier_str = tier.as_deref().unwrap_or("env");
            let items: Vec<String> = entries.iter().map(|e| {
                match &e.default_value {
                    Some(v) => format!("<li><code>{}</code> = <code>{}</code></li>", escape_html(&e.name), escape_html(v)),
                    None => format!("<li><code>{}</code></li>", escape_html(&e.name)),
                }
            }).collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-env\"><strong class=\"surfdoc-infra-label\">Env ({})</strong><ul>{}</ul></div>", escape_html(tier_str), items.join(""))
        }

        Block::Health { path, method, grace, interval, timeout, .. } => {
            let mut items = Vec::new();
            if let Some(p) = path { items.push(format!("<li>path: {}</li>", escape_html(p))); }
            if let Some(m) = method { items.push(format!("<li>method: {}</li>", escape_html(m))); }
            if let Some(g) = grace { items.push(format!("<li>grace: {}</li>", escape_html(g))); }
            if let Some(i) = interval { items.push(format!("<li>interval: {}</li>", escape_html(i))); }
            if let Some(t) = timeout { items.push(format!("<li>timeout: {}</li>", escape_html(t))); }
            format!("<div class=\"surfdoc-infra-card surfdoc-health\"><strong class=\"surfdoc-infra-label\">Health Check</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Concurrency { concurrency_type, hard_limit, soft_limit, force_https, .. } => {
            let mut items = Vec::new();
            if let Some(t) = concurrency_type { items.push(format!("<li>type: {}</li>", escape_html(t))); }
            if let Some(h) = hard_limit { items.push(format!("<li>hard_limit: {h}</li>")); }
            if let Some(s) = soft_limit { items.push(format!("<li>soft_limit: {s}</li>")); }
            if *force_https { items.push("<li>force_https: true</li>".to_string()); }
            format!("<div class=\"surfdoc-infra-card surfdoc-concurrency\"><strong class=\"surfdoc-infra-label\">Concurrency</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Cicd { provider, properties, .. } => {
            let prov = provider.as_deref().unwrap_or("CI/CD");
            let items: Vec<String> = properties.iter()
                .map(|p| format!("<li>{}: {}</li>", escape_html(&p.key), escape_html(&p.value)))
                .collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-cicd\"><strong class=\"surfdoc-infra-label\">{}</strong><ul>{}</ul></div>", escape_html(prov), items.join(""))
        }

        Block::Smoke { script, checks, .. } => {
            let script_attr = script.as_ref()
                .map(|s| format!(" data-script=\"{}\"", escape_html(s)))
                .unwrap_or_default();
            let rows: Vec<String> = checks.iter().map(|c| {
                format!(
                    "<tr><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                    escape_html(&c.method), escape_html(&c.path), c.expected,
                )
            }).collect();
            format!(
                "<table class=\"surfdoc-smoke\"{}><thead><tr><th>Method</th><th>Path</th><th>Expected</th></tr></thead><tbody>{}</tbody></table>",
                script_attr, rows.join(""),
            )
        }

        Block::Domains { entries, .. } => {
            let items: Vec<String> = entries.iter().map(|e| {
                match &e.description {
                    Some(d) => format!("<li><strong>{}</strong> &mdash; {}</li>", escape_html(&e.domain), escape_html(d)),
                    None => format!("<li><strong>{}</strong></li>", escape_html(&e.domain)),
                }
            }).collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-domains\"><strong class=\"surfdoc-infra-label\">Domains</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Crates { entries, .. } => {
            let items: Vec<String> = entries.iter().map(|e| {
                let mut detail = Vec::new();
                if let Some(s) = &e.source { detail.push(format!("source: {}", escape_html(s))); }
                if let Some(f) = &e.features { detail.push(format!("features: {}", escape_html(f))); }
                if detail.is_empty() {
                    format!("<li><code>{}</code></li>", escape_html(&e.name))
                } else {
                    format!("<li><code>{}</code> ({})</li>", escape_html(&e.name), detail.join(", "))
                }
            }).collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-crates\"><strong class=\"surfdoc-infra-label\">Crates</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::DeployUrls { entries, .. } => {
            let items: Vec<String> = entries.iter()
                .map(|p| format!("<li>{}: <a href=\"{}\">{}</a></li>", escape_html(&p.key), escape_html(&p.value), escape_html(&p.value)))
                .collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-deploy-urls\"><strong class=\"surfdoc-infra-label\">Deploy URLs</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Volumes { entries, .. } => {
            let items: Vec<String> = entries.iter()
                .map(|v| format!("<li><code>{}</code> &rarr; <code>{}</code></li>", escape_html(&v.name), escape_html(&v.mount)))
                .collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-volumes\"><strong class=\"surfdoc-infra-label\">Volumes</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Model { name, fields, .. } => {
            let mut rows = String::new();
            for f in fields {
                let type_str = model_field_type_str(&f.field_type);
                let constraints_str: Vec<String> = f.constraints.iter().map(|c| constraint_str(c)).collect();
                let constraints_html = if constraints_str.is_empty() {
                    String::new()
                } else {
                    format!(" <span class=\"surfdoc-model-constraints\">[{}]</span>", constraints_str.join(", "))
                };
                rows.push_str(&format!(
                    "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>",
                    escape_html(&f.name), escape_html(&type_str), constraints_html,
                ));
            }
            format!(
                "<div class=\"surfdoc-infra-card surfdoc-model\"><strong class=\"surfdoc-infra-label\">Model: {}</strong><table class=\"surfdoc-model-table\">\
                 <thead><tr><th>Field</th><th>Type</th><th>Constraints</th></tr></thead>\
                 <tbody>{}</tbody></table></div>",
                escape_html(name), rows,
            )
        }

        Block::Route { method, path, auth, returns, body, handler, content, .. } => {
            let method_str = http_method_str(*method);
            if let Some(code) = handler {
                // Route with embedded handler — render as documentation block.
                let auth_html = match auth {
                    Some(a) => format!(" <span class=\"surfdoc-route-auth\">[auth: {}]</span>", escape_html(a)),
                    None => String::new(),
                };
                format!(
                    "<div class=\"surfdoc-infra-card surfdoc-route\">\
                     <div class=\"surfdoc-route-head\"><span class=\"surfdoc-route-method surfdoc-method-{}\">{}</span> \
                     <code class=\"surfdoc-route-path\">{}</code>{}</div>\
                     <pre class=\"surfdoc-code\"><code>{}</code></pre></div>",
                    method_str.to_lowercase(), method_str, escape_html(path), auth_html, escape_html(code),
                )
            } else {
                let mut details = Vec::new();
                if let Some(a) = auth { details.push(format!("<li>auth: {}</li>", escape_html(a))); }
                if let Some(r) = returns { details.push(format!("<li>returns: <code>{}</code></li>", escape_html(r))); }
                if let Some(b) = body { details.push(format!("<li>body: <code>{}</code></li>", escape_html(b))); }
                if !content.is_empty() { details.push(format!("<li>{}</li>", escape_html(content))); }
                let details_html = if details.is_empty() { String::new() } else { format!("<ul>{}</ul>", details.join("")) };
                format!(
                    "<div class=\"surfdoc-infra-card surfdoc-route\"><span class=\"surfdoc-route-method surfdoc-method-{}\">{}</span> \
                     <code class=\"surfdoc-route-path\">{}</code>{}</div>",
                    method_str.to_lowercase(), method_str, escape_html(path), details_html,
                )
            }
        }

        Block::Auth { provider, session, roles, default_role, .. } => {
            let provider_str = auth_provider_str(*provider);
            let mut items = vec![format!("<li>provider: {}</li>", provider_str)];
            if let Some(s) = session { items.push(format!("<li>session: {}</li>", escape_html(s))); }
            if !roles.is_empty() { items.push(format!("<li>roles: {}</li>", roles.iter().map(|r| escape_html(r)).collect::<Vec<_>>().join(", "))); }
            if let Some(dr) = default_role { items.push(format!("<li>default role: {}</li>", escape_html(dr))); }
            format!("<div class=\"surfdoc-infra-card surfdoc-auth\"><strong class=\"surfdoc-infra-label\">Authentication</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Binding { source, target, events, .. } => {
            let mut items = vec![
                format!("<li>source: <code>{}</code></li>", escape_html(source)),
                format!("<li>target: <code>{}</code></li>", escape_html(target)),
            ];
            for e in events {
                items.push(format!("<li>{}: {}</li>", escape_html(&e.event), escape_html(&e.action)));
            }
            format!("<div class=\"surfdoc-infra-card surfdoc-binding\"><strong class=\"surfdoc-infra-label\">Binding</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::Schema { name, fields, .. } => {
            let mut rows = String::new();
            for f in fields {
                let constraints_html = if f.constraints.is_empty() {
                    String::new()
                } else {
                    f.constraints.iter().map(|c| escape_html(&constraint_str(c))).collect::<Vec<_>>().join(", ")
                };
                rows.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape_html(&f.name), escape_html(&model_field_type_str(&f.field_type)), constraints_html,
                ));
            }
            format!(
                "<div class=\"surfdoc-schema\">\
                 <h4 class=\"surfdoc-schema-name\">{}</h4>\
                 <table class=\"surfdoc-data\">\
                 <thead><tr><th>Column</th><th>Type</th><th>Constraints</th></tr></thead>\
                 <tbody>{}</tbody></table></div>",
                escape_html(name), rows,
            )
        }

        Block::Use { crates, .. } => {
            let items: Vec<String> = crates.iter().map(|c| {
                let mut parts = vec![escape_html(&c.name)];
                if let Some(v) = &c.version {
                    parts.push(escape_html(v));
                }
                if !c.features.is_empty() {
                    parts.push(format!("[{}]", c.features.iter().map(|f| escape_html(f)).collect::<Vec<_>>().join(", ")));
                }
                format!("<li><code>{}</code></li>", parts.join(" "))
            }).collect();
            format!("<div class=\"surfdoc-infra-card surfdoc-use\"><strong class=\"surfdoc-infra-label\">Dependencies</strong><ul>{}</ul></div>", items.join(""))
        }

        Block::AppEnv { vars, .. } => {
            let mut rows = String::new();
            for v in vars {
                let req = if v.required { "yes" } else { "no" };
                let desc = v.description.as_deref().unwrap_or("");
                rows.push_str(&format!(
                    "<tr><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                    escape_html(&v.name), req, escape_html(desc),
                ));
            }
            format!(
                "<div class=\"surfdoc-infra-card surfdoc-app-env\">\
                 <strong class=\"surfdoc-infra-label\">Environment Variables</strong>\
                 <table class=\"surfdoc-data\">\
                 <thead><tr><th>Name</th><th>Required</th><th>Description</th></tr></thead>\
                 <tbody>{}</tbody></table></div>",
                rows,
            )
        }

        Block::AppDeploy { region, scale, domain, memory, properties, .. } => {
            let mut items = Vec::new();
            if let Some(r) = region { items.push(format!("<li>region: {}</li>", escape_html(r))); }
            if let Some(s) = scale { items.push(format!("<li>scale: {s}</li>")); }
            if let Some(d) = domain { items.push(format!("<li>domain: {}</li>", escape_html(d))); }
            if let Some(m) = memory { items.push(format!("<li>memory: {}</li>", escape_html(m))); }
            for (k, v) in properties {
                items.push(format!("<li>{}: {}</li>", escape_html(k), escape_html(v)));
            }
            format!(
                "<div class=\"surfdoc-infra-card surfdoc-app-deploy\"><strong class=\"surfdoc-infra-label\">Deploy Configuration</strong><ul>{}</ul></div>",
                items.join(""),
            )
        }

        Block::Row { icon, title, description, href, state, unread, .. } => {
            let state_class = match state {
                RowState::Loading => " surfdoc-row--loading",
                RowState::Empty => " surfdoc-row--empty",
                _ => "",
            };
            let icon_svg = row_icon_svg(icon);
            let arrow_svg = "<svg width=\"14\" height=\"14\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\"><polyline points=\"9 18 15 12 9 6\"/></svg>";
            let tag = if href.is_some() { "a" } else { "div" };
            let href_attr = match href {
                Some(h) => format!(" href=\"{}\"", escape_html(h)),
                None => String::new(),
            };
            // Unread: right-side dot only — accent-left-border is BANNED.
            let unread_dot = if *unread {
                "<span class=\"surfdoc-unread-dot\" aria-label=\"Unread\"></span>"
            } else {
                ""
            };
            format!(
                "<{tag} class=\"surfdoc-row{state_class}\"{href_attr}>\
                 <span class=\"surfdoc-row-icon\">{icon_svg}</span>\
                 <span class=\"surfdoc-row-body\">\
                   <span class=\"surfdoc-row-title\">{title}</span>\
                   <span class=\"surfdoc-row-desc\">{desc}</span>\
                 </span>\
                 {unread_dot}\
                 <span class=\"surfdoc-row-arrow\">{arrow_svg}</span>\
                 </{tag}>",
                title = escape_html(title),
                desc = escape_html(description),
            )
        }

        Block::InfoCard { intent, title, subtitle, summary, image, facts, steps, state, .. } => {
            let state_class = match state {
                RowState::Loading => " surfdoc-infocard--loading",
                RowState::Empty => " surfdoc-infocard--empty",
                _ => "",
            };
            let mut html = format!("<div class=\"surfdoc-infocard{state_class}\">");
            html.push_str("<div class=\"surfdoc-infocard-header\">");
            if let Some(img) = image {
                html.push_str(&format!("<img class=\"surfdoc-infocard-image\" src=\"{}\" alt=\"{}\">", escape_html(img), escape_html(title)));
            }
            html.push_str("<div class=\"surfdoc-infocard-info\">");
            html.push_str(&format!("<h3 class=\"surfdoc-infocard-title\">{}</h3>", escape_html(title)));
            if !subtitle.is_empty() {
                html.push_str(&format!("<div class=\"surfdoc-infocard-subtitle\">{}</div>", escape_html(subtitle)));
            }
            html.push_str(&format!("<span class=\"surfdoc-infocard-badge {}\">{}</span>", escape_html(intent), escape_html(intent)));
            html.push_str("</div></div>");
            if !summary.is_empty() {
                html.push_str(&format!("<p class=\"surfdoc-infocard-summary\">{}</p>", escape_html(summary)));
            }
            if !steps.is_empty() {
                html.push_str("<div class=\"surfdoc-infocard-steps\">");
                for (i, step) in steps.iter().enumerate() {
                    html.push_str(&format!(
                        "<div class=\"surfdoc-infocard-step\"><span class=\"surfdoc-infocard-step-num\">{}</span><span class=\"surfdoc-infocard-step-text\">{}</span></div>",
                        i + 1, escape_html(step)
                    ));
                }
                html.push_str("</div>");
            } else if !facts.is_empty() {
                html.push_str("<div class=\"surfdoc-infocard-facts\">");
                for fact in facts {
                    html.push_str(&format!(
                        "<span class=\"surfdoc-infocard-fact-label\">{}</span><span class=\"surfdoc-infocard-fact-value\">{}</span>",
                        escape_html(&fact[0]), escape_html(&fact[1])
                    ));
                }
                html.push_str("</div>");
            }
            html.push_str("</div>");
            html
        }

        // ── Interactive / application blocks ──────────────────────

        Block::AppShell { layout, children, .. } => {
            let mut html = format!(
                "<div class=\"surfdoc-app-shell surfdoc-layout-{}\">",
                escape_html(layout),
            );
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</div>");
            html
        }

        Block::Sidebar { position, collapsible, width, children, .. } => {
            let style = match width {
                Some(w) => format!(" style=\"width:{}px\"", w),
                None => String::new(),
            };
            let mut html = format!(
                "<aside class=\"surfdoc-sidebar surfdoc-sidebar-{}\" data-collapsible=\"{}\"{}>",
                escape_html(position), collapsible, style,
            );
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</aside>");
            html
        }

        Block::Panel { position, resizable, height, desktop_only, children, .. } => {
            let style = match height {
                Some(h) => format!(" style=\"height:{}px\"", h),
                None => String::new(),
            };
            let mut html = format!(
                "<div class=\"surfdoc-panel surfdoc-panel-{}\" data-resizable=\"{}\" data-desktop-only=\"{}\"{}>",
                escape_html(position), resizable, desktop_only, style,
            );
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</div>");
            html
        }

        Block::TabBar { active, items, .. } => {
            let mut html = String::from("<nav class=\"surfdoc-tab-bar\" role=\"tablist\">");
            for item in items {
                let is_active = active.as_ref().is_some_and(|a| a == &item.id);
                let active_cls = if is_active { " class=\"active\"" } else { "" };
                let icon_attr = item
                    .icon
                    .as_ref()
                    .map(|i| format!(" data-icon=\"{}\"", escape_html(i)))
                    .unwrap_or_default();
                // Unread: right-side dot only — accent-left-border is BANNED.
                let unread_dot = if item.unread {
                    "<span class=\"surfdoc-unread-dot\" aria-label=\"Unread\"></span>"
                } else {
                    ""
                };
                html.push_str(&format!(
                    "<button role=\"tab\" data-tab=\"{}\"{}{} aria-selected=\"{}\">{}{}</button>",
                    escape_html(&item.id),
                    icon_attr,
                    active_cls,
                    is_active,
                    escape_html(&item.label),
                    unread_dot,
                ));
            }
            html.push_str("</nav>");
            // Inline JS for tab switching — self-contained, no external deps
            html.push_str(r#"<script>document.querySelectorAll('.surfdoc-tab-bar').forEach(bar=>{bar.querySelectorAll('[role="tab"]').forEach(btn=>{btn.onclick=()=>{const tab=btn.dataset.tab;const parent=bar.parentElement||document;bar.querySelectorAll('[role="tab"]').forEach(b=>{b.classList.remove('active');b.setAttribute('aria-selected','false')});btn.classList.add('active');btn.setAttribute('aria-selected','true');parent.querySelectorAll('.surfdoc-tab-content').forEach(tc=>{if(tc.dataset.tab===tab){tc.classList.add('active');tc.style.display='block'}else{tc.classList.remove('active');tc.style.display='none'}})}})})</script>"#);
            html
        }

        Block::SegmentedControl { active, size, action, segments, .. } => {
            // A filter idiom, not a content-pane switcher: radiogroup role,
            // no tablist markup and no tab-switching script.
            let action_attr = match action {
                Some(a) => format!(" data-action=\"{}\"", escape_html(a)),
                None => String::new(),
            };
            let mut html = format!(
                "<div class=\"surfdoc-segmented-control\" role=\"radiogroup\" data-size=\"{}\"{}>",
                escape_html(size), action_attr,
            );
            for seg in segments {
                let is_active = active.as_ref().is_some_and(|a| a == &seg.id);
                let active_cls = if is_active { " is-active" } else { "" };
                html.push_str(&format!(
                    "<button type=\"button\" role=\"radio\" class=\"surfdoc-segment{}\" data-id=\"{}\" aria-checked=\"{}\">{}</button>",
                    active_cls,
                    escape_html(&seg.id),
                    is_active,
                    escape_html(&seg.label),
                ));
            }
            html.push_str("</div>");
            html
        }

        Block::TabContent { tab, children, .. } => {
            // First tab-content is visible by default, others hidden
            let mut html = format!(
                "<div class=\"surfdoc-tab-content\" data-tab=\"{}\" role=\"tabpanel\">",
                escape_html(tab),
            );
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</div>");
            html
        }

        Block::Toolbar { items, .. } => {
            let mut html = String::from("<div class=\"surfdoc-toolbar\">");
            for item in items {
                match item {
                    crate::types::ToolbarItem::Button { label, action, style, .. } => {
                        let cls = match style {
                            Some(s) => format!(" surfdoc-toolbar-btn-{}", escape_html(s)),
                            None => String::new(),
                        };
                        let action_attr = match action {
                            Some(a) => format!(" data-action=\"{}\"", escape_html(a)),
                            None => String::new(),
                        };
                        html.push_str(&format!(
                            "<button class=\"surfdoc-toolbar-btn{}\"{}>{}</button>",
                            cls, action_attr, escape_html(label.as_deref().unwrap_or("")),
                        ));
                    }
                    crate::types::ToolbarItem::Separator => {
                        html.push_str("<span class=\"surfdoc-toolbar-separator\"></span>");
                    }
                    crate::types::ToolbarItem::Spacer => {
                        html.push_str("<span class=\"surfdoc-toolbar-spacer\"></span>");
                    }
                    crate::types::ToolbarItem::Badge { value, color } => {
                        let cls = match color {
                            Some(c) => format!(" surfdoc-badge-{}", escape_html(c)),
                            None => String::new(),
                        };
                        html.push_str(&format!(
                            "<span class=\"surfdoc-badge{}\">{}</span>",
                            cls, escape_html(value),
                        ));
                    }
                    crate::types::ToolbarItem::Dropdown { label, .. } => {
                        html.push_str(&format!(
                            "<select class=\"surfdoc-toolbar-dropdown\"><option>{}</option></select>",
                            escape_html(label),
                        ));
                    }
                    crate::types::ToolbarItem::Text { value, .. } => {
                        html.push_str(&format!(
                            "<span class=\"surfdoc-toolbar-text\">{}</span>",
                            escape_html(value),
                        ));
                    }
                }
            }
            html.push_str("</div>");
            html
        }

        Block::Drawer { name, position, width, children, .. } => {
            let style = match width {
                Some(w) => format!(" style=\"width:{}px\"", w),
                None => String::new(),
            };
            let mut html = format!(
                "<div class=\"surfdoc-drawer surfdoc-drawer-{}\" data-name=\"{}\"{}>",
                escape_html(position), escape_html(name), style,
            );
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</div>");
            html
        }

        Block::Modal { name, title, width, placement, dismissible, children, .. } => {
            let style = match width {
                Some(w) => format!(" style=\"width:{}px\"", w),
                None => String::new(),
            };
            let dismiss_attr = if *dismissible { String::new() } else { " data-dismissible=\"false\"".to_string() };
            let mut html = format!(
                "<dialog class=\"surfdoc-modal\" data-name=\"{}\" data-placement=\"{}\"{}{}>",
                escape_html(name), escape_html(placement), dismiss_attr, style,
            );
            // Dismiss canon: the header always carries the title and a
            // top-right close control, regardless of `dismissible` —
            // `dismissible` only governs backdrop/escape semantics.
            let heading = title.as_deref().unwrap_or(name);
            html.push_str("<header class=\"surfdoc-modal-header\">");
            html.push_str(&format!("<strong class=\"surfdoc-modal-title\">{}</strong>", escape_html(heading)));
            html.push_str("<button type=\"button\" class=\"surfdoc-modal-close\" aria-label=\"Close\">&#10005;</button>");
            html.push_str("</header>");
            for child in children { html.push_str(&render_block(child)); }
            html.push_str("</dialog>");
            html
        }

        Block::DropdownSelect { label, icon, selected, align, options, .. } => {
            let icon_attr = match icon {
                Some(i) => format!(" data-icon=\"{}\"", escape_html(i)),
                None => String::new(),
            };
            let mut html = format!(
                "<div class=\"surfdoc-dropdown-select\" data-align=\"{}\">",
                escape_html(align),
            );
            html.push_str(&format!("<button type=\"button\" class=\"surfdoc-dropdown-trigger\"{}>", icon_attr));
            if let Some(l) = label {
                html.push_str(&format!("<span class=\"surfdoc-dropdown-label\">{}</span>", escape_html(l)));
            }
            if let Some(s) = selected {
                html.push_str(&format!("<span class=\"surfdoc-dropdown-selected\">{}</span>", escape_html(s)));
            }
            html.push_str("<span class=\"surfdoc-dropdown-caret\" aria-hidden=\"true\">&#9662;</span></button>");
            html.push_str("<ul class=\"surfdoc-dropdown-options\">");
            for opt in options {
                let sel_class = if selected.as_deref() == Some(opt.label.as_str()) {
                    " is-selected"
                } else {
                    ""
                };
                let action_attr = match &opt.action {
                    Some(a) => format!(" data-action=\"{}\"", escape_html(a)),
                    None => String::new(),
                };
                let opt_icon_attr = match &opt.icon {
                    Some(i) => format!(" data-icon=\"{}\"", escape_html(i)),
                    None => String::new(),
                };
                let desc = match &opt.description {
                    Some(d) => format!("<span class=\"surfdoc-dropdown-option-desc\">{}</span>", escape_html(d)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<li class=\"surfdoc-dropdown-option{}\"{}{}><span class=\"surfdoc-dropdown-option-label\">{}</span>{}</li>",
                    sel_class, action_attr, opt_icon_attr, escape_html(&opt.label), desc,
                ));
            }
            html.push_str("</ul></div>");
            html
        }

        Block::CommandPalette { items, .. } => {
            let mut html = String::from("<div class=\"surfdoc-command-palette\"><input type=\"search\" placeholder=\"Search commands...\" class=\"surfdoc-command-search\">");
            html.push_str("<ul class=\"surfdoc-command-list\">");
            for item in items {
                let group_attr = match &item.group {
                    Some(g) => format!(" data-group=\"{}\"", escape_html(g)),
                    None => String::new(),
                };
                let desc = match &item.description {
                    Some(d) => format!("<span class=\"surfdoc-command-desc\">{}</span>", escape_html(d)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<li class=\"surfdoc-command-item\"{}><span class=\"surfdoc-command-label\">{}</span>{}</li>",
                    group_attr, escape_html(&item.label), desc,
                ));
            }
            html.push_str("</ul></div>");
            html
        }

        Block::CodeEditor { lang, source, line_numbers, content, .. } => {
            let lang_attr = match lang {
                Some(l) => format!(" data-lang=\"{}\"", escape_html(l)),
                None => String::new(),
            };
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            let ln_attr = if *line_numbers { " data-line-numbers=\"true\"" } else { "" };
            format!(
                "<div class=\"surfdoc-code-editor\"{}{}{}><pre><code>{}</code></pre></div>",
                lang_attr, source_attr, ln_attr, escape_html(content),
            )
        }

        Block::BlockEditor { source, .. } => {
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            format!(
                "<div class=\"surfdoc-block-editor surfdoc-block-editor-preview\"{}>\
                 <div class=\"surfdoc-block-editor-block surfdoc-block-editor-h1\">Untitled document</div>\
                 <div class=\"surfdoc-block-editor-block surfdoc-block-editor-p\">Start writing or press <kbd>/</kbd> to add a block&hellip;</div>\
                 <div class=\"surfdoc-block-editor-add\" aria-label=\"Add block\">+ Add block</div>\
                 </div>",
                source_attr,
            )
        }

        Block::Terminal { shell, cwd, .. } => {
            let shell_attr = match shell {
                Some(s) => format!(" data-shell=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            let cwd_attr = match cwd {
                Some(c) => format!(" data-cwd=\"{}\"", escape_html(c)),
                None => String::new(),
            };
            let cwd_display = cwd.as_deref().unwrap_or("~");
            let shell_display = shell.as_deref().unwrap_or("bash");
            format!(
                "<div class=\"surfdoc-terminal surfdoc-terminal-preview\"{shell_attr}{cwd_attr}>\
                 <div class=\"surfdoc-terminal-bar\"><span class=\"surfdoc-terminal-dot surfdoc-terminal-dot-red\"></span><span class=\"surfdoc-terminal-dot surfdoc-terminal-dot-yellow\"></span><span class=\"surfdoc-terminal-dot surfdoc-terminal-dot-green\"></span><span class=\"surfdoc-terminal-title\">{shell_display} &mdash; {cwd_display}</span></div>\
                 <pre class=\"surfdoc-terminal-body\"><code><span class=\"surfdoc-terminal-prompt\">$</span> cargo build --release\n<span class=\"surfdoc-terminal-out\">   Compiling surf-parse v1.0.0\n   Finished release [optimized] target(s) in 4.82s</span>\n<span class=\"surfdoc-terminal-prompt\">$</span> <span class=\"surfdoc-terminal-cursor\"> </span></code></pre>\
                 </div>",
                shell_display = escape_html(shell_display),
                cwd_display = escape_html(cwd_display),
            )
        }

        Block::NavTree { source, .. } => {
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            format!(
                "<nav class=\"surfdoc-nav-tree surfdoc-nav-tree-preview\"{}>\
                 <ul class=\"surfdoc-tree-list\">\
                   <li class=\"surfdoc-tree-folder surfdoc-tree-open\"><span class=\"surfdoc-tree-toggle\">&#9660;</span> <span class=\"surfdoc-tree-name\">src</span>\
                     <ul class=\"surfdoc-tree-list\">\
                       <li class=\"surfdoc-tree-file surfdoc-tree-active\"><span class=\"surfdoc-tree-name\">main.rs</span></li>\
                       <li class=\"surfdoc-tree-file\"><span class=\"surfdoc-tree-name\">render_html.rs</span></li>\
                       <li class=\"surfdoc-tree-file\"><span class=\"surfdoc-tree-name\">blocks.rs</span></li>\
                     </ul>\
                   </li>\
                   <li class=\"surfdoc-tree-folder\"><span class=\"surfdoc-tree-toggle\">&#9658;</span> <span class=\"surfdoc-tree-name\">assets</span></li>\
                   <li class=\"surfdoc-tree-file\"><span class=\"surfdoc-tree-name\">Cargo.toml</span></li>\
                 </ul>\
                 </nav>",
                source_attr,
            )
        }

        Block::Badge { value, color, .. } => {
            let cls = match color {
                Some(c) => format!(" surfdoc-badge-{}", escape_html(c)),
                None => String::new(),
            };
            format!(
                "<span class=\"surfdoc-badge{}\">{}</span>",
                cls, escape_html(value),
            )
        }

        Block::SuggestionChips { source, max, dismissible, .. } => {
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            let max_attr = match max {
                Some(m) => format!(" data-max=\"{}\"", m),
                None => String::new(),
            };
            // Static preview: sample chips so the block is never empty
            format!(
                "<div class=\"surfdoc-suggestion-chips\"{} data-dismissible=\"{}\"{}>\
                  <button class=\"surfdoc-chip\">Show open tasks</button>\
                  <button class=\"surfdoc-chip\">Summarise last standup</button>\
                  <button class=\"surfdoc-chip\">What&rsquo;s blocking the team?</button>\
                </div>",
                source_attr, dismissible, max_attr,
            )
        }

        Block::ChatThread { source, .. } => {
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            // Static preview: sample 2-message thread so the block is never empty
            format!("<div class=\"surfdoc-chat-thread\"{source_attr}>\
              <div class=\"surfdoc-chat-msg surfdoc-chat-msg-user\">How do I add a new task?</div>\
              <div class=\"surfdoc-chat-msg surfdoc-chat-msg-assistant\">Click <strong>+ New Task</strong> in the board toolbar, fill in the title, and assign an owner — it&rsquo;ll appear in the Todo column instantly.</div>\
            </div>")
        }

        Block::ChatInputSimple { placeholder, action, .. } => {
            let ph = placeholder.as_deref().unwrap_or("");
            let action_attr = match action {
                Some(a) => format!(" data-action=\"{}\"", escape_html(a)),
                None => String::new(),
            };
            format!(
                "<div class=\"surfdoc-chat-input\"><input type=\"text\" placeholder=\"{}\"><button{}>Send</button></div>",
                escape_html(ph), action_attr,
            )
        }

        Block::Progress { steps, .. } => {
            let mut html = String::from("<ol class=\"surfdoc-progress\">");
            for step in steps {
                html.push_str(&format!(
                    "<li class=\"surfdoc-step-{}\">{}</li>",
                    escape_html(&step.status), escape_html(&step.label),
                ));
            }
            html.push_str("</ol>");
            html
        }

        Block::LogStream { source, tail, .. } => {
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            let tail_attr = match tail {
                Some(t) => format!(" data-tail=\"{}\"", t),
                None => String::new(),
            };
            format!(
                "<div class=\"surfdoc-log-stream surfdoc-log-stream-preview\"{}{}>\
                 <pre><code><span class=\"surfdoc-log-info\">INFO</span>  [2026-05-26T10:42:01Z] Server listening on :8080\n<span class=\"surfdoc-log-info\">INFO</span>  [2026-05-26T10:42:03Z] GET /healthz 200 1ms\n<span class=\"surfdoc-log-info\">INFO</span>  [2026-05-26T10:42:11Z] POST /api/tasks 201 12ms\n<span class=\"surfdoc-log-warn\">WARN</span>  [2026-05-26T10:42:30Z] DB connection pool at 80%% capacity\n<span class=\"surfdoc-log-info\">INFO</span>  [2026-05-26T10:43:01Z] GET /api/tasks 200 4ms</code></pre>\
                 </div>",
                source_attr, tail_attr,
            )
        }

        Block::ProblemList { source, .. } => {
            let source_attr = match source {
                Some(s) => format!(" data-source=\"{}\"", escape_html(s)),
                None => String::new(),
            };
            format!(
                "<div class=\"surfdoc-problem-list surfdoc-problem-list-preview\"{}>\
                 <div class=\"surfdoc-problem-empty\">\
                   <svg width=\"16\" height=\"16\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" aria-hidden=\"true\"><polyline points=\"20 6 9 17 4 12\"/></svg>\
                   No problems detected\
                 </div>\
                 </div>",
                source_attr,
            )
        }

        Block::Unknown {
            name, content, ..
        } => {
            // Render the content as markdown (sanitized) rather than dumping
            // the raw escaped source: a hallucinated block name (`::team`)
            // must not surface literal `##` / list dashes as visible copy.
            // Wrapper contract unchanged: class + role + data-name.
            format!(
                "<div class=\"surfdoc-unknown\" role=\"note\" data-name=\"{}\">{}</div>",
                escape_html(name),
                render_markdown(content),
            )
        }

    }
}

/// SVG icons for row blocks
fn row_icon_svg(icon: &str) -> &'static str {
    match icon {
        "sparkle" | "ai" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><path d=\"M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z\"/></svg>",
        "book" | "wiki" | "knowledge" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><path d=\"M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H20v20H6.5a2.5 2.5 0 0 1 0-5H20\"/><path d=\"M8 7h6\"/><path d=\"M8 11h8\"/></svg>",
        "folder" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><path d=\"M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z\"/></svg>",
        "doc" | "file" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><path d=\"M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z\"/><polyline points=\"14 2 14 8 20 8\"/></svg>",
        "search" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><circle cx=\"11\" cy=\"11\" r=\"8\"/><line x1=\"21\" y1=\"21\" x2=\"16.65\" y2=\"16.65\"/></svg>",
        "login" | "signin" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><path d=\"M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4\"/><polyline points=\"10 17 15 12 10 7\"/><line x1=\"15\" y1=\"12\" x2=\"3\" y2=\"12\"/></svg>",
        "task" | "check" => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><polyline points=\"9 11 12 14 22 4\"/><path d=\"M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11\"/></svg>",
        _ => "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><circle cx=\"12\" cy=\"12\" r=\"10\"/></svg>",
    }
}

/// Render a comparison cell value: "yes"/"true"/"✓" → green check, "no"/"false"/"✗"/"-" → muted dash, else literal with inline markdown.
fn comparison_cell(cell: &str) -> String {
    match cell.trim().to_lowercase().as_str() {
        "yes" | "true" | "✓" | "✔" => "<span class=\"surfdoc-check\">\u{2713}</span>".to_string(),
        "no" | "false" | "✗" | "✘" | "-" | "—" => "<span class=\"surfdoc-dash\">\u{2014}</span>".to_string(),
        _ => render_cell_inline_markdown(cell),
    }
}

fn model_field_type_str(ft: &crate::types::ModelFieldType) -> String {
    use crate::types::ModelFieldType;
    match ft {
        ModelFieldType::Uuid => "uuid".to_string(),
        ModelFieldType::String => "string".to_string(),
        ModelFieldType::Int => "int".to_string(),
        ModelFieldType::Float => "float".to_string(),
        ModelFieldType::Bool => "bool".to_string(),
        ModelFieldType::Datetime => "datetime".to_string(),
        ModelFieldType::Text => "text".to_string(),
        ModelFieldType::Json => "json".to_string(),
        ModelFieldType::Money => "money".to_string(),
        ModelFieldType::Image => "image".to_string(),
        ModelFieldType::Email => "email".to_string(),
        ModelFieldType::Url => "url".to_string(),
        ModelFieldType::Enum(variants) => format!("enum({})", variants.join(", ")),
        ModelFieldType::Ref(target) => format!("ref({target})"),
    }
}

fn constraint_str(c: &crate::types::FieldConstraint) -> String {
    use crate::types::FieldConstraint;
    match c {
        FieldConstraint::Primary => "primary".to_string(),
        FieldConstraint::Auto => "auto".to_string(),
        FieldConstraint::Required => "required".to_string(),
        FieldConstraint::Optional => "optional".to_string(),
        FieldConstraint::Unique => "unique".to_string(),
        FieldConstraint::Index => "index".to_string(),
        FieldConstraint::Max(n) => format!("max={n}"),
        FieldConstraint::Min(n) => format!("min={n}"),
        FieldConstraint::Default(v) => format!("default={v}"),
    }
}

fn http_method_str(m: crate::types::HttpMethod) -> &'static str {
    use crate::types::HttpMethod;
    match m {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn auth_provider_str(p: crate::types::AuthProvider) -> &'static str {
    use crate::types::AuthProvider;
    match p {
        AuthProvider::Email => "email",
        AuthProvider::OAuth => "oauth",
        AuthProvider::ApiKey => "api-key",
        AuthProvider::Token => "token",
    }
}

fn callout_type_str(ct: CalloutType) -> &'static str {
    match ct {
        CalloutType::Info => "info",
        CalloutType::Warning => "warning",
        CalloutType::Danger => "danger",
        CalloutType::Tip => "tip",
        CalloutType::Note => "note",
        CalloutType::Success => "success",
        CalloutType::Context => "context",
    }
}

/// Returns true when a data-table cell should be right-aligned as a number.
/// Detects plain numbers, percentages, and currency-prefixed amounts after
/// stripping common formatting (commas, surrounding whitespace, leading sign,
/// and a single trailing `%` or leading currency symbol).
fn is_numeric_cell(cell: &str) -> bool {
    let trimmed = cell.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Strip one leading currency symbol and one trailing percent sign.
    let mut s = trimmed;
    if let Some(rest) = s.strip_prefix(['$', '€', '£', '¥']) {
        s = rest;
    }
    let s = s.strip_suffix('%').unwrap_or(s);
    // Strip a leading sign.
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    // Remove thousands separators.
    let cleaned: String = s.chars().filter(|c| *c != ',').collect();
    if cleaned.is_empty() {
        return false;
    }
    // Must contain at least one digit and parse as a float.
    cleaned.chars().any(|c| c.is_ascii_digit()) && cleaned.parse::<f64>().is_ok()
}

/// Renders escaped code content, wrapping highlighted lines in
/// `<span class="surfdoc-code-hl">…</span>`.
///
/// `highlight` is a list of 1-based line specs as parsed in `blocks.rs`:
/// each entry is either a single line number (e.g. `"3"`) or an inclusive
/// range (e.g. `"5-7"`). Non-numeric or unparseable entries are ignored
/// (best-effort). When no lines match, the content is returned escaped as-is.
fn render_code_with_highlights(content: &str, highlight: &[String]) -> String {
    if highlight.is_empty() {
        return escape_html(content);
    }
    // Collect the set of 1-based line numbers to highlight.
    let mut lines_to_hl: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for spec in highlight {
        let spec = spec.trim();
        if let Some((start, end)) = spec.split_once('-') {
            if let (Ok(s), Ok(e)) = (start.trim().parse::<usize>(), end.trim().parse::<usize>()) {
                let (lo, hi) = if s <= e { (s, e) } else { (e, s) };
                for n in lo..=hi {
                    lines_to_hl.insert(n);
                }
            }
        } else if let Ok(n) = spec.parse::<usize>() {
            lines_to_hl.insert(n);
        }
        // else: non-numeric spec ignored (best-effort).
    }
    if lines_to_hl.is_empty() {
        return escape_html(content);
    }
    let mut out = String::new();
    for (idx, line) in content.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let line_no = idx + 1; // 1-based
        if lines_to_hl.contains(&line_no) {
            out.push_str("<span class=\"surfdoc-code-hl\">");
            out.push_str(&escape_html(line));
            out.push_str("</span>");
        } else {
            out.push_str(&escape_html(line));
        }
    }
    out
}

/// Inline SVG icon for a callout, per the reference block spec (Feather style).
/// Emitted with the `surfdoc-callout-icon` class so CSS can size/color it.
fn callout_icon_svg(ct: CalloutType) -> &'static str {
    // Paths sourced from A1-reference-block-spec.md §Callout.
    match ct {
        CalloutType::Warning => {
            "<svg class=\"surfdoc-callout-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><path d=\"M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z\"/><line x1=\"12\" y1=\"9\" x2=\"12\" y2=\"13\"/><line x1=\"12\" y1=\"17\" x2=\"12.01\" y2=\"17\"/></svg>"
        }
        CalloutType::Danger => {
            "<svg class=\"surfdoc-callout-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"10\"/><line x1=\"15\" y1=\"9\" x2=\"9\" y2=\"15\"/><line x1=\"9\" y1=\"9\" x2=\"15\" y2=\"15\"/></svg>"
        }
        // Tip + info/note/success/context share the lightbulb tip icon.
        CalloutType::Tip
        | CalloutType::Info
        | CalloutType::Note
        | CalloutType::Success
        | CalloutType::Context => {
            "<svg class=\"surfdoc-callout-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><path d=\"M9 18h6M10 22h4M12 2a7 7 0 0 0-4 12.7c.6.5 1 1.3 1 2.1h6c0-.8.4-1.6 1-2.1A7 7 0 0 0 12 2z\"/></svg>"
        }
    }
}

fn decision_status_str(ds: DecisionStatus) -> &'static str {
    match ds {
        DecisionStatus::Proposed => "proposed",
        DecisionStatus::Accepted => "accepted",
        DecisionStatus::Rejected => "rejected",
        DecisionStatus::Superseded => "superseded",
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

// -- Multi-page site extraction and rendering --------------------------

/// Extracted site-level configuration from a `::site` block.
#[derive(Debug, Clone, Default)]
pub struct SiteConfig {
    pub domain: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tagline: Option<String>,
    /// The palette's *designed-for* hint ("light" | "dark"). NOT a pin: the
    /// rendered page follows the visitor's stored choice, then the device
    /// theme — this hint only orders the no-preference fallback
    /// (BR-SITE-THEME).
    pub theme: Option<String>,
    pub accent: Option<String>,
    pub font: Option<String>,
    /// The site's serving prefix (e.g. `/s/{slug}`), used to scope the
    /// visitor's persisted theme choice per site on a shared origin. `None`
    /// (standalone export on its own domain) scopes to `/`.
    pub base_path: Option<String>,
    pub properties: Vec<StyleProperty>,
}

/// A single page extracted from a `::page` block.
#[derive(Debug, Clone)]
pub struct PageEntry {
    pub route: String,
    pub layout: Option<String>,
    pub title: Option<String>,
    pub sidebar: bool,
    pub children: Vec<Block>,
}

impl PageEntry {
    /// Returns the human-readable display title for this page.
    ///
    /// If the page has an explicit `title`, returns that. Otherwise, converts
    /// the route to a readable label using [`humanize_route`].
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| humanize_route(&self.route))
    }
}

/// Convert a route path to a human-readable nav label.
///
/// `"/"` → `"Home"`, `"/gallery"` → `"Gallery"`, `"/about-us"` → `"About Us"`.
pub fn humanize_route(route: &str) -> String {
    let r = route.trim_matches('/');
    if r.is_empty() {
        return "Home".to_string();
    }
    r.split('-')
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract site config and page list from a parsed SurfDoc.
///
/// Returns `(site_config, pages, loose_blocks)` where `loose_blocks` are
/// top-level blocks that are neither `Site` nor `Page`.
pub fn extract_site(doc: &SurfDoc) -> (Option<SiteConfig>, Vec<PageEntry>, Vec<Block>) {
    let mut site_config: Option<SiteConfig> = None;
    let mut pages: Vec<PageEntry> = Vec::new();
    let mut loose: Vec<Block> = Vec::new();

    for block in &doc.blocks {
        match block {
            Block::Site {
                domain,
                properties,
                ..
            } => {
                let mut config = SiteConfig {
                    domain: domain.clone(),
                    properties: properties.clone(),
                    ..Default::default()
                };
                for prop in properties {
                    match prop.key.as_str() {
                        "name" => config.name = Some(prop.value.clone()),
                        "description" => config.description = Some(prop.value.clone()),
                        "tagline" => config.tagline = Some(prop.value.clone()),
                        "theme" => config.theme = Some(prop.value.clone()),
                        "accent" => config.accent = Some(prop.value.clone()),
                        "font" => config.font = Some(prop.value.clone()),
                        _ => {}
                    }
                }
                site_config = Some(config);
            }
            Block::Page {
                route,
                layout,
                title,
                sidebar,
                children,
                ..
            } => {
                pages.push(PageEntry {
                    route: route.clone(),
                    layout: layout.clone(),
                    title: title.clone(),
                    sidebar: *sidebar,
                    children: children.clone(),
                });
            }
            other => {
                loose.push(other.clone());
            }
        }
    }

    (site_config, pages, loose)
}

/// CSS for site-level navigation and footer (uses unified variable names).
const SITE_NAV_CSS: &str = r#"
/* Skip link (BR-SITE-A11Y): first focusable on every site page; visually
   hidden until keyboard focus, then a pill above the sticky nav. */
.surfdoc-skip-link { position: absolute; left: -9999px; top: 0.75rem; z-index: 200; background: var(--surface); color: var(--accent-ink, var(--accent)); padding: 0.5rem 1rem; border: 1px solid var(--border); border-radius: 6px; font-size: 0.875rem; font-weight: 600; text-decoration: none; }
.surfdoc-skip-link:focus { left: 0.75rem; }

/* Site navigation — a sticky topbar (hamburger + logo on the left, theme
   toggle far right) that opens a LEFT DRAWER of page links at ALL screen sizes
   (the wavesite "app" nav, matching the Surf shell). Pure-CSS checkbox toggle
   + click-anywhere scrim; the SPA router also unchecks the toggle on navigate.
   Drawer look mirrors the Surf rich shell (.surfdoc-shell-drawer): a floating
   rounded panel with a brand head, close affordance, group label, and
   line-icon links. */
.surfdoc-site-nav { display: flex; align-items: center; gap: 10px; height: 60px; padding: 0 16px; background: var(--surface); border-bottom: 1px solid var(--border); max-width: 100%; position: sticky; top: 0; z-index: 100; }
.surfdoc-site-nav .site-name { order: 1; display: inline-flex; align-items: center; font-weight: 700; color: var(--text); font-size: 1rem; text-decoration: none; }
/* Monogram logo — accent rounded square standing in for a brand emblem
   (generated apps ship no logo asset). Used in the topbar anchor AND the
   drawer-head brand. */
.site-nav-logo { display: inline-flex; align-items: center; justify-content: center; width: 26px; height: 26px; flex-shrink: 0; border-radius: 7px; background: var(--accent); color: var(--accent-text, #ffffff); font-size: 0.8rem; font-weight: 700; line-height: 1; margin-right: 8px; }
/* Hamburger — always visible, far left; a circular shell-style control that
   animates to an X when the drawer opens. */
.site-nav-hamburger { order: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 5px; width: 38px; height: 38px; flex-shrink: 0; cursor: pointer; border: 1px solid var(--border); border-radius: 50%; color: var(--text); transition: background 0.15s; }
.site-nav-hamburger:hover { background: var(--surface-hover); }
.site-nav-hamburger span { display: block; width: 18px; height: 2px; background: currentColor; border-radius: 1px; transition: transform 0.2s, opacity 0.2s; }
/* Toggle checkbox: visually hidden but focusable (keyboard-operable menu). */
.site-nav-toggle { position: absolute; width: 1px; height: 1px; margin: -1px; opacity: 0; pointer-events: none; }
.site-nav-toggle:focus-visible ~ .site-nav-hamburger { outline: 2px solid var(--accent-ink, var(--accent)); outline-offset: 2px; }
/* Left drawer holding the page links — a floating rounded panel inset 12px
   from the viewport edges, floating OVER the topbar (z 200 > nav z 100),
   matching the Surf shell drawer. Close via the head button or the scrim. */
.site-nav-links { position: fixed; top: 12px; bottom: 12px; left: 12px; z-index: 200; width: min(320px, calc(100vw - 24px)); display: flex; flex-direction: column; align-items: stretch; gap: 2px; padding: 18px 14px 24px; background: var(--surface); border: 1px solid var(--border); border-radius: 20px; box-shadow: 0 20px 60px rgba(15, 23, 42, 0.25); transform: translateX(calc(-100% - 16px)); transition: transform 280ms cubic-bezier(0.22, 1, 0.36, 1); overflow-y: auto; }
/* Drawer head: brand (monogram + name) left, circular close button right. */
.site-nav-drawer-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.site-nav-drawer-brand { display: inline-flex; align-items: center; font-weight: 700; font-size: 1rem; color: var(--text); }
.site-nav-close { display: inline-flex; align-items: center; justify-content: center; width: 34px; height: 34px; flex-shrink: 0; border: 1px solid var(--border); border-radius: 50%; background: var(--surface); color: var(--text); cursor: pointer; transition: background 0.15s; }
.site-nav-close:hover { background: var(--surface-hover); }
.site-nav-close svg { width: 18px; height: 18px; }
/* Uppercase section label above the page links. */
.site-nav-group-label { font-size: 0.6875rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--text-muted); padding: 14px 12px 6px; }
.site-nav-links a { display: flex; align-items: center; gap: 12px; color: var(--text); text-decoration: none; font-size: 0.9375rem; font-weight: 500; padding: 11px 12px; border-radius: 12px; transition: color 0.15s, background 0.15s; }
.site-nav-links a:hover { color: var(--accent); background: var(--surface-hover); }
.site-nav-links a.active { color: var(--accent-ink, var(--accent)); background: var(--accent-soft); font-weight: 600; }
/* Per-link line icon; follows the link color on hover/active. */
.site-nav-link-icon { display: inline-flex; width: 18px; height: 18px; flex-shrink: 0; color: var(--text-muted); transition: color 0.15s; }
.site-nav-link-icon svg { width: 18px; height: 18px; }
.site-nav-links a:hover .site-nav-link-icon { color: var(--accent); }
.site-nav-links a.active .site-nav-link-icon { color: var(--accent-ink, var(--accent)); }
.site-nav-toggle:checked ~ .site-nav-links { transform: translateX(0); }
/* Scrim behind the open drawer (click to close) — full-viewport, under the
   floating panel. */
.site-nav-scrim { position: fixed; inset: 0; z-index: 150; background: rgba(8, 12, 24, 0.30); opacity: 0; visibility: hidden; transition: opacity 220ms ease, visibility 220ms; cursor: pointer; }
.site-nav-toggle:checked ~ .site-nav-scrim { opacity: 1; visibility: visible; }
/* Hamburger → X when the drawer is open. */
.site-nav-toggle:checked ~ .site-nav-hamburger span:nth-child(1) { transform: translateY(7px) rotate(45deg); }
.site-nav-toggle:checked ~ .site-nav-hamburger span:nth-child(2) { opacity: 0; }
.site-nav-toggle:checked ~ .site-nav-hamburger span:nth-child(3) { transform: translateY(-7px) rotate(-45deg); }
/* Deeper panel shadow on dark surfaces — both theme arms, mirroring the
   theme-icon-swap pattern below. */
[data-theme="dark"] .site-nav-links { box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) .site-nav-links { box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); }
}
@media (prefers-reduced-motion: reduce) {
  .site-nav-links, .site-nav-scrim { transition: none; }
}

/* Theme toggle (BR-SITE-THEME): ≥24px target, theme-aware icon swap; same
   circular shell-style control as the hamburger. */
.site-nav-theme-toggle { order: 2; display: inline-flex; align-items: center; justify-content: center; width: 38px; height: 38px; flex-shrink: 0; margin-left: auto; padding: 0; background: none; border: 1px solid var(--border); border-radius: 50%; color: var(--text-muted); cursor: pointer; transition: color 0.15s, background 0.15s; }
.site-nav-theme-toggle:hover { background: var(--surface-hover); color: var(--text); }
.site-nav-theme-toggle svg { width: 18px; height: 18px; }
.site-nav-theme-toggle .site-theme-icon-sun { display: none; }
[data-theme="dark"] .site-nav-theme-toggle .site-theme-icon-sun { display: block; }
[data-theme="dark"] .site-nav-theme-toggle .site-theme-icon-moon { display: none; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) .site-nav-theme-toggle .site-theme-icon-sun { display: block; }
  :root:not([data-theme]) .site-nav-theme-toggle .site-theme-icon-moon { display: none; }
}

/* Focus visibility (BR-SITE-A11Y): every interactive chrome element shows a
   ring; offset keeps it outside style-pack borders (Comic's 3px). */
.surfdoc-site-nav a:focus-visible, .site-nav-theme-toggle:focus-visible, .surfdoc-skip-link:focus-visible, .site-nav-close:focus-visible { outline: 2px solid var(--accent-ink, var(--accent)); outline-offset: 2px; }

@media (max-width: 640px) {
  .surfdoc-site-nav { padding: 0 12px; }
}

/* Site footer */
.surfdoc-site-footer { margin-top: 4rem; padding: 1.5rem; border-top: 1px solid var(--border); text-align: center; color: var(--text-faint); font-size: 0.8rem; }

/* Single-file multi-page (SPA): one <section> per route; the client router
   toggles [hidden] to show exactly one at a time. !important defends against
   any block style that sets display on the section. */
.surfdoc-page[hidden] { display: none !important; }
"#;

/// Sanitize a localStorage key for embedding inside a single-quoted JS string
/// in emitted markup: strip everything outside a conservative allowlist
/// (alphanumerics, `/ : _ -`). The platform constructs these keys from slugs,
/// so this is suspenders, not the law.
fn sanitize_js_key(key: &str) -> String {
    key.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | ':' | '_' | '-'))
        .collect()
}

/// The accent monogram standing in for a logo emblem: first character of the
/// trimmed site name, uppercased and HTML-escaped ("?" for an empty name).
fn site_nav_monogram(site_name: &str) -> String {
    match site_name.trim().chars().next() {
        Some(c) => escape_html(&c.to_uppercase().to_string()),
        None => "?".to_string(),
    }
}

/// Keyword-match a nav route/title to a small built-in line-icon set
/// (24×24 stroke `currentColor`, the lucide subset the Surf shell drawer
/// uses). Route is matched first, then the title; unknown pages get the
/// generic document icon. Purely cosmetic — a miss is never an error.
fn site_nav_link_icon(route: &str, title: &str) -> &'static str {
    const ICON_HOUSE: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>"#;
    const ICON_BAG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z"/><line x1="3" y1="6" x2="21" y2="6"/><path d="M16 10a4 4 0 0 1-8 0"/></svg>"#;
    const ICON_USERS: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#;
    const ICON_MAIL: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/><polyline points="22,6 12,13 2,6"/></svg>"#;
    const ICON_NEWS: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-2 2zm0 0a2 2 0 0 1-2-2v-9c0-1.1.9-2 2-2h2"/><line x1="18" y1="14" x2="10" y2="14"/><line x1="15" y1="18" x2="10" y2="18"/><rect x="10" y="6" width="8" height="4"/></svg>"#;
    const ICON_TAG: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>"#;
    const ICON_IMAGE: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>"#;
    const ICON_LAYERS: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 17 12 22 22 17"/><polyline points="2 12 12 17 22 12"/></svg>"#;
    const ICON_HELP: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>"#;
    const ICON_CALENDAR: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>"#;
    const ICON_UTENSILS: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 2v7c0 1.1.9 2 2 2h4a2 2 0 0 0 2-2V2"/><path d="M7 2v20"/><path d="M21 15V2a5 5 0 0 0-5 5v6c0 1.1.9 2 2 2h3zm0 0v7"/></svg>"#;
    const ICON_BOOK: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg>"#;
    const ICON_FILE_TEXT: &str = r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>"#;
    // (keywords, icon) in priority order; first hit on the haystack wins.
    const MAP: &[(&[&str], &str)] = &[
        (&["home"], ICON_HOUSE),
        (&["shop", "store", "product"], ICON_BAG),
        (&["about", "team"], ICON_USERS),
        (&["contact"], ICON_MAIL),
        (&["blog", "news"], ICON_NEWS),
        (&["pricing", "plan"], ICON_TAG),
        (&["gallery", "portfolio", "photo"], ICON_IMAGE),
        (&["service"], ICON_LAYERS),
        (&["faq", "help", "support"], ICON_HELP),
        (&["event", "schedule", "calendar"], ICON_CALENDAR),
        (&["menu", "food"], ICON_UTENSILS),
        (&["docs", "guide", "learn"], ICON_BOOK),
    ];
    fn pick(hay: &str) -> Option<&'static str> {
        MAP.iter()
            .find(|(keys, _)| keys.iter().any(|k| hay.contains(k)))
            .map(|(_, icon)| *icon)
    }
    let route = route.to_lowercase();
    if route == "/" || route.is_empty() {
        return ICON_HOUSE;
    }
    pick(&route)
        .or_else(|| pick(&title.to_lowercase()))
        .unwrap_or(ICON_FILE_TEXT)
}

/// Build the site-level navigation HTML bar.
///
/// Produces a `<nav class="surfdoc-site-nav">` element with:
/// - Site name (monogram + wordmark) as a linked logo
/// - Keyboard-operable hamburger toggle for mobile (focusable checkbox)
/// - A drawer panel: brand head with close affordance, "Pages" group label,
///   navigation links with per-page line icons, active-class +
///   `aria-current="page"` detection
/// - A light/dark theme toggle button writing `theme_key` (BR-SITE-THEME)
///
/// This is separate from the inline `Block::Nav` rendering in `to_html()`,
/// which uses `surfdoc-nav` class names and a different data model
/// (NavItem structs vs route/title pairs).
fn build_site_nav_html(
    site_name: &str,
    nav_items: &[(String, String)],
    current_route: &str,
    theme_key: &str,
    // When true, emit a single-file (SPA) nav: links are `#route` fragments and
    // carry `data-route` so the client router can switch sections and manage
    // active state. When false, links are clean URLs (one HTML file per route).
    spa: bool,
) -> String {
    let home_href = if spa { "#/" } else { "/" };
    let monogram = site_nav_monogram(site_name);
    let name = escape_html(site_name);
    // The topbar anchor's opening tag is byte-pinned by surf's container
    // (LOGO_NEEDLE, first occurrence rewritten to the site's base path) —
    // only its CONTENTS may change.
    let mut nav_html = format!(
        "<nav class=\"surfdoc-site-nav\" role=\"navigation\" aria-label=\"Site navigation\">\n  <a href=\"{home_href}\" class=\"site-name\"><span class=\"site-nav-logo\" aria-hidden=\"true\">{monogram}</span>{name}</a>\n",
    );
    // Hamburger toggle for mobile: the checkbox is focusable (visually hidden
    // by CSS, never display:none/aria-hidden) so the menu is keyboard-operable.
    nav_html.push_str("  <input type=\"checkbox\" class=\"site-nav-toggle\" id=\"site-nav-toggle\" aria-label=\"Toggle menu\">\n");
    nav_html.push_str("  <label for=\"site-nav-toggle\" class=\"site-nav-hamburger\" aria-hidden=\"true\"><span></span><span></span><span></span></label>\n");
    nav_html.push_str("  <div class=\"site-nav-links\">\n");
    // Drawer head — brand (a <span>, NOT a link: keeps LOGO_NEEDLE's
    // first-occurrence semantics trivially safe) + a close label for the
    // toggle. Decorative like the hamburger/scrim labels; keyboard users
    // operate the focusable checkbox.
    nav_html.push_str(&format!(
        "    <div class=\"site-nav-drawer-head\">\n      <span class=\"site-nav-drawer-brand\"><span class=\"site-nav-logo\" aria-hidden=\"true\">{monogram}</span>{name}</span>\n      <label for=\"site-nav-toggle\" class=\"site-nav-close\" aria-hidden=\"true\" title=\"Close menu\"><svg viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><line x1=\"18\" y1=\"6\" x2=\"6\" y2=\"18\"/><line x1=\"6\" y1=\"6\" x2=\"18\" y2=\"18\"/></svg></label>\n    </div>\n",
    ));
    nav_html.push_str("    <span class=\"site-nav-group-label\">Pages</span>\n");
    for (route, nav_title) in nav_items {
        let href = if spa {
            format!("#{route}")
        } else {
            route.to_string()
        };
        let active = *route == current_route;
        // In SPA mode the client router reads `data-route` to switch sections;
        // it is appended AFTER the active class so clean-URL output (the
        // `else` branch) stays byte-identical to the pre-SPA renderer.
        let data_attr = if spa {
            format!(" data-route=\"{}\"", escape_html(route))
        } else {
            String::new()
        };
        nav_html.push_str(&format!(
            "    <a href=\"{}\"{}{}><span class=\"site-nav-link-icon\" aria-hidden=\"true\">{}</span>{}</a>\n",
            escape_html(&href),
            if active {
                " class=\"active\" aria-current=\"page\"".to_string()
            } else {
                String::new()
            },
            data_attr,
            site_nav_link_icon(route, nav_title),
            escape_html(nav_title),
        ));
    }
    nav_html.push_str("  </div>\n");
    // Theme toggle (BR-SITE-THEME): flips data-theme and persists the choice
    // under the per-site key. Markup order (after .site-nav-links) puts it at
    // the far right on desktop and keeps it visible in the mobile topbar.
    let key = sanitize_js_key(theme_key);
    nav_html.push_str(&format!(
        "  <button type=\"button\" class=\"site-nav-theme-toggle\" aria-label=\"Switch between light and dark theme\" title=\"Toggle theme\" onclick=\"(function(d){{var t=d.getAttribute('data-theme')==='dark'?'light':'dark';d.setAttribute('data-theme',t);try{{localStorage.setItem('{key}',t)}}catch(e){{}}}})(document.documentElement)\">",
    ));
    nav_html.push_str("<svg class=\"site-theme-icon-moon\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z\"></path></svg>");
    nav_html.push_str("<svg class=\"site-theme-icon-sun\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><circle cx=\"12\" cy=\"12\" r=\"5\"></circle><line x1=\"12\" y1=\"1\" x2=\"12\" y2=\"3\"></line><line x1=\"12\" y1=\"21\" x2=\"12\" y2=\"23\"></line><line x1=\"4.22\" y1=\"4.22\" x2=\"5.64\" y2=\"5.64\"></line><line x1=\"18.36\" y1=\"18.36\" x2=\"19.78\" y2=\"19.78\"></line><line x1=\"1\" y1=\"12\" x2=\"3\" y2=\"12\"></line><line x1=\"21\" y1=\"12\" x2=\"23\" y2=\"12\"></line><line x1=\"4.22\" y1=\"19.78\" x2=\"5.64\" y2=\"18.36\"></line><line x1=\"18.36\" y1=\"5.64\" x2=\"19.78\" y2=\"4.22\"></line></svg>");
    // Scrim behind the open left drawer — a <label> for the toggle, so clicking
    // it closes the drawer with no JS. Sibling of the toggle ⇒ CSS `:checked ~`
    // drives it. Additive: keeps LOGO_NEEDLE / NAV_CLOSE_NEEDLE intact.
    nav_html.push_str("</button>\n  <label for=\"site-nav-toggle\" class=\"site-nav-scrim\" aria-hidden=\"true\"></label>\n</nav>");
    nav_html
}

/// Render a full HTML page for one route within a multi-page site.
///
/// Produces a `<!DOCTYPE html>` page with site-level `<nav>`, page content,
/// and a footer. Theme and accent from `SiteConfig` are applied via CSS variables.
pub fn render_site_page(
    page: &PageEntry,
    site: &SiteConfig,
    nav_items: &[(String, String)], // (route, title) pairs
    config: &PageConfig,
) -> String {
    // Render page children as HTML, grouping consecutive CTAs
    let mut body_parts: Vec<String> = Vec::new();
    let mut cta_group: Vec<String> = Vec::new();
    for child in &page.children {
        if matches!(child, Block::Cta { .. }) {
            cta_group.push(render_block(child));
            continue;
        }
        if !cta_group.is_empty() {
            body_parts.push(format!("<div class=\"surfdoc-cta-group\">{}</div>", cta_group.join("\n")));
            cta_group.clear();
        }
        body_parts.push(render_block(child));
    }
    if !cta_group.is_empty() {
        body_parts.push(format!("<div class=\"surfdoc-cta-group\">{}</div>", cta_group.join("\n")));
    }
    // Heading/TOC post-pass (same as to_html): anchors prose headings —
    // including explicit `{#slug}` suffixes, which otherwise leak as copy.
    let body = wire_headings_and_toc(&body_parts.join("\n"));

    let lang = config.lang.as_deref().unwrap_or("en");
    let site_name = site
        .name
        .as_deref()
        .unwrap_or("SurfDoc Site");

    // Title: page title > humanized route + site name
    let title = match &page.title {
        Some(t) => format!("{} — {}", t, site_name),
        None if page.route == "/" => site_name.to_string(),
        None => format!("{} — {}", humanize_route(&page.route), site_name),
    };

    let source_path = escape_html(&config.source_path);

    // Per-site theme persistence key (BR-SITE-THEME): scoped to the site's
    // serving prefix so two sites on one origin keep independent choices.
    let base = site.base_path.as_deref().filter(|b| !b.is_empty()).unwrap_or("/");
    let theme_key = sanitize_js_key(&format!("surfdoc-site-theme:{base}"));

    // Build navigation HTML (clean URLs — no /index.html suffix)
    let nav_html = build_site_nav_html(site_name, nav_items, &page.route, &theme_key, false);

    // Build footer
    let footer_html = format!(
        "<footer class=\"surfdoc-site-footer\">{}</footer>",
        escape_html(site_name),
    );

    // Build CSS variable overrides from site config. The accent-ink pair
    // (BR-SITE-A11Y) is ALWAYS emitted: the accent rendered AS text (active
    // nav link, prose links) adjusted to WCAG AA against each theme's
    // surfaces — the toggle means every palette faces both backgrounds.
    let mut css_overrides = String::new();
    if let Some(accent) = &site.accent {
        css_overrides.push_str(&format!("--accent: {};\n", escape_html(accent)));
        let text = accent_text_color(accent);
        css_overrides.push_str(&format!("--accent-text: {};\n", text));
    }
    let effective_accent = site.accent.as_deref().unwrap_or("#2563eb");
    let ink_light = accent_ink_color(effective_accent, false);
    let ink_dark = accent_ink_color(effective_accent, true);
    css_overrides.push_str(&format!("--accent-ink: {ink_light};\n"));
    let override_block = format!(
        "\n:root {{\n{css_overrides}}}\n\
         [data-theme=\"dark\"] {{ --accent-ink: {ink_dark}; }}\n\
         @media (prefers-color-scheme: dark) {{ :root:not([data-theme]) {{ --accent-ink: {ink_dark}; }} }}"
    );

    // Build meta tags: description, canonical, OG, Twitter
    let mut meta_extra = String::new();
    let description = config.description.as_deref().or(site.description.as_deref());
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(url) = &config.canonical_url {
        let url_escaped = escape_html(url);
        meta_extra.push_str(&format!(
            "\n    <link rel=\"canonical\" href=\"{}\">",
            url_escaped
        ));
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:url\" content=\"{}\">",
            url_escaped
        ));
    }
    let title_escaped = escape_html(&title);
    meta_extra.push_str(&format!(
        "\n    <meta property=\"og:title\" content=\"{}\">",
        title_escaped
    ));
    meta_extra.push_str("\n    <meta property=\"og:type\" content=\"website\">");
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(img) = &config.og_image {
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:image\" content=\"{}\">",
            escape_html(img)
        ));
    }
    meta_extra.push_str("\n    <meta name=\"twitter:card\" content=\"summary\">");
    meta_extra.push_str(&format!(
        "\n    <meta name=\"twitter:title\" content=\"{}\">",
        title_escaped
    ));
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"twitter:description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(img) = &config.og_image {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"twitter:image\" content=\"{}\">",
            escape_html(img)
        ));
    }

    // BR-SITE-THEME: no hard-pinned data-theme. The pre-paint resolver sets
    // it before first paint: stored per-site choice → device preference →
    // the palette's designed-for hint (only when the device expresses
    // nothing). Without JS, the :root:not([data-theme]) media arms in
    // SURFDOC_CSS follow the device theme.
    let theme_hint = match site.theme.as_deref() {
        Some("dark") => "dark",
        _ => "light",
    };
    let theme_resolver = format!(
        "<script>(function(){{var d=document.documentElement;var s=null;\
         try{{s=localStorage.getItem('{theme_key}')}}catch(e){{}}\
         var mm=window.matchMedia;\
         var t=s||((mm&&mm('(prefers-color-scheme: dark)').matches)?'dark':\
         (mm&&mm('(prefers-color-scheme: light)').matches)?'light':'{theme_hint}');\
         d.setAttribute('data-theme',t);}})();</script>"
    );

    crate::slots::resolve_slot_markers(format!(
        r##"<!-- Built with SurfDoc — source: {source_path} -->
<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="generator" content="SurfDoc v0.1">
    <link rel="alternate" type="text/surfdoc" href="{source_path}">
    <title>{title}</title>{meta_extra}
    <style>{css}{nav_css}{override_block}</style>
    {theme_resolver}
</head>
<body>
<a class="surfdoc-skip-link" href="#surfdoc-main">Skip to content</a>
{nav}
<main id="surfdoc-main" class="surfdoc">
{body}
</main>
{footer}
</body>
</html>"##,
        source_path = source_path,
        lang = escape_html(lang),
        title = title_escaped,
        meta_extra = meta_extra,
        css = SURFDOC_CSS,
        nav_css = SITE_NAV_CSS,
        override_block = override_block,
        theme_resolver = theme_resolver,
        nav = nav_html,
        body = body,
        footer = footer_html,
    ))
}

/// Assemble a complete `<!DOCTYPE html>` site document from an already-rendered
/// `nav_html` and `body`. `extra_body` is injected just before `</body>` (e.g.
/// the single-file SPA router). Mirrors the document chrome emitted by
/// [`render_site_page`]; used by [`render_site_single_file`].
fn render_site_document(
    site: &SiteConfig,
    config: &PageConfig,
    title: &str,
    nav_html: &str,
    body: &str,
    extra_body: &str,
) -> String {
    let lang = config.lang.as_deref().unwrap_or("en");
    let site_name = site.name.as_deref().unwrap_or("SurfDoc Site");
    let source_path = escape_html(&config.source_path);

    let base = site
        .base_path
        .as_deref()
        .filter(|b| !b.is_empty())
        .unwrap_or("/");
    let theme_key = sanitize_js_key(&format!("surfdoc-site-theme:{base}"));

    let footer_html = format!(
        "<footer class=\"surfdoc-site-footer\">{}</footer>",
        escape_html(site_name),
    );

    // CSS variable overrides from site config (accent + WCAG-AA accent-ink pair,
    // BR-SITE-A11Y) — identical to render_site_page().
    let mut css_overrides = String::new();
    if let Some(accent) = &site.accent {
        css_overrides.push_str(&format!("--accent: {};\n", escape_html(accent)));
        let text = accent_text_color(accent);
        css_overrides.push_str(&format!("--accent-text: {};\n", text));
    }
    let effective_accent = site.accent.as_deref().unwrap_or("#2563eb");
    let ink_light = accent_ink_color(effective_accent, false);
    let ink_dark = accent_ink_color(effective_accent, true);
    css_overrides.push_str(&format!("--accent-ink: {ink_light};\n"));
    let override_block = format!(
        "\n:root {{\n{css_overrides}}}\n\
         [data-theme=\"dark\"] {{ --accent-ink: {ink_dark}; }}\n\
         @media (prefers-color-scheme: dark) {{ :root:not([data-theme]) {{ --accent-ink: {ink_dark}; }} }}"
    );

    let mut meta_extra = String::new();
    let description = config.description.as_deref().or(site.description.as_deref());
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(url) = &config.canonical_url {
        let url_escaped = escape_html(url);
        meta_extra.push_str(&format!(
            "\n    <link rel=\"canonical\" href=\"{}\">",
            url_escaped
        ));
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:url\" content=\"{}\">",
            url_escaped
        ));
    }
    let title_escaped = escape_html(title);
    meta_extra.push_str(&format!(
        "\n    <meta property=\"og:title\" content=\"{}\">",
        title_escaped
    ));
    meta_extra.push_str("\n    <meta property=\"og:type\" content=\"website\">");
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(img) = &config.og_image {
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:image\" content=\"{}\">",
            escape_html(img)
        ));
    }
    meta_extra.push_str("\n    <meta name=\"twitter:card\" content=\"summary\">");
    meta_extra.push_str(&format!(
        "\n    <meta name=\"twitter:title\" content=\"{}\">",
        title_escaped
    ));
    if let Some(desc) = description {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"twitter:description\" content=\"{}\">",
            escape_html(desc)
        ));
    }
    if let Some(img) = &config.og_image {
        meta_extra.push_str(&format!(
            "\n    <meta name=\"twitter:image\" content=\"{}\">",
            escape_html(img)
        ));
    }

    let theme_hint = match site.theme.as_deref() {
        Some("dark") => "dark",
        _ => "light",
    };
    let theme_resolver = format!(
        "<script>(function(){{var d=document.documentElement;var s=null;\
         try{{s=localStorage.getItem('{theme_key}')}}catch(e){{}}\
         var mm=window.matchMedia;\
         var t=s||((mm&&mm('(prefers-color-scheme: dark)').matches)?'dark':\
         (mm&&mm('(prefers-color-scheme: light)').matches)?'light':'{theme_hint}');\
         d.setAttribute('data-theme',t);}})();</script>"
    );

    format!(
        r##"<!-- Built with SurfDoc — source: {source_path} -->
<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="generator" content="SurfDoc v0.1">
    <link rel="alternate" type="text/surfdoc" href="{source_path}">
    <title>{title}</title>{meta_extra}
    <style>{css}{nav_css}{override_block}</style>
    {theme_resolver}
</head>
<body>
<a class="surfdoc-skip-link" href="#surfdoc-main">Skip to content</a>
{nav}
<main id="surfdoc-main" class="surfdoc">
{body}
</main>
{footer}
{extra_body}
</body>
</html>"##,
        source_path = source_path,
        lang = escape_html(lang),
        title = title_escaped,
        meta_extra = meta_extra,
        css = SURFDOC_CSS,
        nav_css = SITE_NAV_CSS,
        override_block = override_block,
        theme_resolver = theme_resolver,
        nav = nav_html,
        body = body,
        footer = footer_html,
        extra_body = extra_body,
    )
}

/// Render a page's child blocks to body HTML, grouping consecutive CTAs into a
/// `.surfdoc-cta-group` wrapper. Shared by the single-file multi-page renderer.
fn render_page_body(children: &[Block]) -> String {
    let mut body_parts: Vec<String> = Vec::new();
    let mut cta_group: Vec<String> = Vec::new();
    for child in children {
        if matches!(child, Block::Cta { .. }) {
            cta_group.push(render_block(child));
            continue;
        }
        if !cta_group.is_empty() {
            body_parts.push(format!(
                "<div class=\"surfdoc-cta-group\">{}</div>",
                cta_group.join("\n")
            ));
            cta_group.clear();
        }
        body_parts.push(render_block(child));
    }
    if !cta_group.is_empty() {
        body_parts.push(format!(
            "<div class=\"surfdoc-cta-group\">{}</div>",
            cta_group.join("\n")
        ));
    }
    body_parts.join("\n")
}

/// The client-side router for single-file multi-page sites.
///
/// All routes live in one HTML document as `<section class="surfdoc-page"
/// data-route="…">` elements; exactly one is visible at a time. The router:
/// - shows the section matching `location.hash` (default `/`, falling back to
///   the first route if the hash is unknown),
/// - keeps the nav's active class + `aria-current` in sync,
/// - intercepts clicks on internal `#route` and absolute `/route` links that
///   match a known route, converting them to hash navigation (so authored
///   `[Browse](/browse)` links resolve without a server),
/// - closes the mobile menu and resets scroll on each navigation.
///
/// `{routes}` is replaced with a JSON array of the site's routes.
const SITE_SPA_ROUTER_JS: &str = r#"<script>(function(){
var routes=__ROUTES__;
function norm(h){h=(h||'').replace(/^#/,'');if(!h||h==='/')return '/';return h.replace(/\/+$/,'')||'/';}
function pick(r){return routes.indexOf(r)>=0?r:(routes.indexOf('/')>=0?'/':routes[0]);}
function show(r){
 r=pick(r);
 var secs=document.querySelectorAll('.surfdoc-page');
 for(var i=0;i<secs.length;i++){secs[i].hidden=(secs[i].getAttribute('data-route')!==r);}
 var links=document.querySelectorAll('.site-nav-links a[data-route]');
 for(var j=0;j<links.length;j++){
  var on=links[j].getAttribute('data-route')===r;
  links[j].classList.toggle('active',on);
  if(on){links[j].setAttribute('aria-current','page');}else{links[j].removeAttribute('aria-current');}
 }
 var t=document.getElementById('site-nav-toggle');if(t){t.checked=false;}
 var m=document.getElementById('surfdoc-main');if(m){m.scrollTop=0;}
 try{window.scrollTo(0,0);}catch(e){}
}
function onhash(){show(norm(location.hash));}
document.addEventListener('click',function(e){
 var a=e.target&&e.target.closest?e.target.closest('a[href]'):null;
 if(!a)return;
 var href=a.getAttribute('href');if(!href)return;
 var r=null;
 if(href.charAt(0)==='#'&&href.charAt(1)==='/'){r=norm(href);}
 else if(href.charAt(0)==='/'&&href.charAt(1)!=='/'){var c=norm(href);if(routes.indexOf(c)>=0){r=c;}}
 if(r!==null){e.preventDefault();if(norm(location.hash)===r){onhash();}else{location.hash=r;}}
});
window.addEventListener('hashchange',onhash);
if(document.readyState!=='loading'){onhash();}else{document.addEventListener('DOMContentLoaded',onhash);}
})();</script>"#;

/// Render an entire multi-page site as a SINGLE self-contained HTML document.
///
/// Every `::page` route becomes a `<section class="surfdoc-page" data-route>`
/// inside one `<main>`; a shared `::nav`/footer chrome wraps them and the
/// inlined [`SITE_SPA_ROUTER_JS`] switches sections client-side. This is the
/// model used by the example showcase, where each app is one artifact loaded
/// in an iframe (absolute-path nav can't work across nested iframe origins).
///
/// Contrast [`render_site_page`], which emits one file per route (clean URLs,
/// needs a server that maps routes → files).
pub fn render_site_single_file(
    site: &SiteConfig,
    pages: &[PageEntry],
    nav_items: &[(String, String)],
    config: &PageConfig,
) -> String {
    let site_name = site.name.as_deref().unwrap_or("SurfDoc Site");
    // The document title is the home/site title; per-route <title> updates are
    // not attempted (single document, single <title>).
    let title = site_name.to_string();

    // Per-site theme persistence key (BR-SITE-THEME) for the nav theme toggle.
    let base = site
        .base_path
        .as_deref()
        .filter(|b| !b.is_empty())
        .unwrap_or("/");
    let theme_key = sanitize_js_key(&format!("surfdoc-site-theme:{base}"));

    // SPA nav: `#route` links + data-route for the client router. Initial
    // active route is "/" (or the first route if there is no home).
    let has_home = pages.iter().any(|p| p.route == "/");
    let initial_route = if has_home {
        "/"
    } else {
        pages.first().map(|p| p.route.as_str()).unwrap_or("/")
    };
    let nav_html = build_site_nav_html(site_name, nav_items, initial_route, &theme_key, true);

    // One <section> per route; the initial route is visible, the rest hidden.
    let mut sections = String::new();
    for page in pages {
        let inner = render_page_body(&page.children);
        let visible = page.route == initial_route;
        sections.push_str(&format!(
            "<section class=\"surfdoc-page\" data-route=\"{}\"{}>\n{}\n</section>\n",
            escape_html(&page.route),
            if visible { "" } else { " hidden" },
            inner,
        ));
    }

    // Routes as a JSON array for the inlined router.
    let routes_json = format!(
        "[{}]",
        pages
            .iter()
            .map(|p| format!("\"{}\"", p.route.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",")
    );
    let router = SITE_SPA_ROUTER_JS.replace("__ROUTES__", &routes_json);

    // Heading/TOC post-pass over all sections at once so anchor slugs stay
    // unique across the whole single-file document.
    let sections = wire_headings_and_toc(&sections);

    crate::slots::resolve_slot_markers(render_site_document(
        site, config, &title, &nav_html, &sections, &router,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn span() -> Span {
        Span {
            start_line: 1,
            end_line: 1,
            start_offset: 0,
            end_offset: 0,
        }
    }

    fn doc_with(blocks: Vec<Block>) -> SurfDoc {
        SurfDoc {
            front_matter: None,
            blocks,
            source: String::new(),
        }
    }

    #[test]
    fn html_callout() {
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Warning,
            title: Some("Caution".into()),
            content: "Be careful.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-callout surfdoc-callout-warning\""));
        assert!(html.contains("class=\"surfdoc-callout-icon\""));
        assert!(html.contains("<div class=\"surfdoc-callout-title\">Caution</div>"));
        assert!(html.contains("class=\"surfdoc-callout-body\""));
        assert!(html.contains("Be careful."));
    }

    #[test]
    fn html_data_table() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Name".into(), "Age".into()],
            rows: vec![vec!["Alice".into(), "30".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<table class=\"surfdoc-data\">"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<th scope=\"col\" aria-sort=\"none\">Name</th>"));
        assert!(html.contains("<td>Alice</td>"));
        // "30" is numeric and should be right-aligned via the num class.
        assert!(html.contains("<td class=\"num\">30</td>"));
    }

    #[test]
    fn data_table_cell_inline_link_relative() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Company".into()],
            rows: vec![vec!["[ZAPiT Games](zapit-games)".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<td><a href=\"/wiki/zapit-games\">ZAPiT Games</a></td>"),
            "Relative link should get /wiki/ prefix. Got: {}",
            html
        );
    }

    #[test]
    fn data_table_cell_inline_link_absolute() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Link".into()],
            rows: vec![vec!["[Example](https://example.com)".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<td><a href=\"https://example.com\">Example</a></td>"),
            "Absolute link should keep full URL. Got: {}",
            html
        );
    }

    #[test]
    fn data_table_cell_inline_bold() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Status".into()],
            rows: vec![vec!["**Active**".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<td><strong>Active</strong></td>"),
            "Bold should render as <strong>. Got: {}",
            html
        );
    }

    #[test]
    fn data_table_cell_inline_italic() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Note".into()],
            rows: vec![vec!["*pending*".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<td><em>pending</em></td>"),
            "Italic should render as <em>. Got: {}",
            html
        );
    }

    // p1-fill-render: a `«FILL: label | default»` marker surviving intact
    // through a table cell (split_pipe_row must not split on the marker's own
    // `|`) and through cell rendering (render_cell_inline_markdown must not
    // parse markdown inside the marker) resolves cleanly end-to-end.
    #[test]
    fn data_table_cell_fill_marker_resolves_cleanly() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Item".into()],
            rows: vec![vec!["\u{ab}FILL: deck price | $120\u{bb}".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = crate::slots::resolve_slot_markers(to_html(&doc));
        assert!(
            !html.contains('\u{ab}') && !html.contains('\u{bb}'),
            "marker must not leak raw. Got: {html}"
        );
        assert!(html.contains("<td>$120</td>"), "default text must render. Got: {html}");
    }

    #[test]
    fn data_table_cell_inline_mixed() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Info".into()],
            rows: vec![vec!["See **[Docs](https://docs.example.com)** for *details*".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<strong><a href=\"https://docs.example.com\">Docs</a></strong>"),
            "Bold wrapping a link should render both. Got: {}",
            html
        );
        assert!(
            html.contains("<em>details</em>"),
            "Italic should render. Got: {}",
            html
        );
    }

    #[test]
    fn data_table_cell_inline_xss_safety() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Input".into()],
            rows: vec![vec!["<script>alert(1)</script> and [safe](link)".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            !html.contains("<script>"),
            "Script tags must be escaped. Got: {}",
            html
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "Script tags must be HTML-escaped. Got: {}",
            html
        );
        assert!(
            html.contains("<a href=\"/wiki/link\">safe</a>"),
            "Link should still render. Got: {}",
            html
        );
    }

    #[test]
    fn data_table_header_inline_markdown() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["**Bold Header**".into(), "*Italic Header*".into()],
            rows: vec![vec!["a".into(), "b".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<th scope=\"col\" aria-sort=\"none\"><strong>Bold Header</strong></th>"),
            "Header bold should render. Got: {}",
            html
        );
        assert!(
            html.contains("<th scope=\"col\" aria-sort=\"none\"><em>Italic Header</em></th>"),
            "Header italic should render. Got: {}",
            html
        );
    }

    #[test]
    fn pricing_table_cell_inline_link() {
        // Row = tier: col 0 name, col 1 price, remaining cols = feature bullets
        // (which render inline markdown, so links work).
        let doc = doc_with(vec![Block::PricingTable {
            headers: vec!["Tier".into(), "Price".into(), "Docs".into()],
            rows: vec![vec![
                "Pro".into(),
                "$9".into(),
                "[Details](https://example.com/pricing)".into(),
            ]],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<a href=\"https://example.com/pricing\">Details</a>"),
            "Pricing tier feature bullets should render inline links. Got: {}",
            html
        );
    }

    #[test]
    fn data_table_cell_plain_text_unchanged() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Name".into()],
            rows: vec![vec!["Plain text".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<td>Plain text</td>"),
            "Plain text should remain unchanged. Got: {}",
            html
        );
    }

    #[test]
    fn html_code() {
        let doc = doc_with(vec![Block::Code {
            lang: Some("rust".into()),
            file: None,
            highlight: vec![],
            content: "fn main() { println!(\"<hello>\"); }".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<figure class=\"surfdoc-code\">"));
        assert!(html.contains("<pre aria-label=\"rust code\" data-lang=\"rust\">"));
        assert!(html.contains("class=\"language-rust\""));
        // No file given, but lang badge should still render in the head.
        assert!(html.contains("<figcaption class=\"surfdoc-code-head\">"));
        assert!(html.contains("<span class=\"surfdoc-code-lang\">RUST</span>"));
        assert!(!html.contains("surfdoc-code-file"), "No file span when file is None");
        assert!(html.contains("&lt;hello&gt;"), "Angle brackets should be escaped");
    }

    #[test]
    fn html_code_with_file_and_highlight() {
        let doc = doc_with(vec![Block::Code {
            lang: Some("rust".into()),
            file: Some("main.rs".into()),
            highlight: vec!["2".into(), "4-5".into()],
            content: "line one\nline two\nline three\nline four\nline five".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<span class=\"surfdoc-code-file\">main.rs</span>"));
        assert!(html.contains("<span class=\"surfdoc-code-lang\">RUST</span>"));
        // Lines 2, 4, 5 highlighted; lines 1 and 3 not.
        assert!(html.contains("<span class=\"surfdoc-code-hl\">line two</span>"));
        assert!(html.contains("<span class=\"surfdoc-code-hl\">line four</span>"));
        assert!(html.contains("<span class=\"surfdoc-code-hl\">line five</span>"));
        assert!(!html.contains("<span class=\"surfdoc-code-hl\">line one</span>"));
        assert!(!html.contains("<span class=\"surfdoc-code-hl\">line three</span>"));
    }

    #[test]
    fn html_tasks() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![
                TaskItem {
                    done: true,
                    text: "Done item".into(),
                    assignee: None,
                },
                TaskItem {
                    done: false,
                    text: "Pending item".into(),
                    assignee: None,
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<li class=\"surfdoc-task is-done\">"));
        assert!(html.contains("<li class=\"surfdoc-task\">"));
        assert!(html.contains("<span class=\"surfdoc-check\">\u{2713}</span>"));
        assert!(html.contains("<span class=\"surfdoc-check\"></span>"));
        assert!(html.contains("<span class=\"surfdoc-task-text\">Done item</span>"));
        assert!(html.contains("<span class=\"surfdoc-task-text\">Pending item</span>"));
    }

    #[test]
    fn html_tasks_assignee() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![TaskItem {
                done: false,
                text: "Ship it".into(),
                assignee: Some("brady".into()),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<span class=\"surfdoc-assignee\">@brady</span>"));
    }

    #[test]
    fn html_metric() {
        let doc = doc_with(vec![Block::Metric {
            label: "Revenue".into(),
            value: "$10K".into(),
            trend: Some(Trend::Up),
            unit: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-metric-row\""));
        assert!(html.contains("class=\"surfdoc-metric\""));
        assert!(html.contains("<div class=\"surfdoc-metric-value\">$10K</div>"));
        assert!(html.contains("<div class=\"surfdoc-metric-label\">Revenue"));
        assert!(html.contains("class=\"surfdoc-trend surfdoc-trend--up\""));
    }

    #[test]
    fn html_figure() {
        let doc = doc_with(vec![Block::Figure {
            src: "arch.png".into(),
            caption: Some("Architecture diagram".into()),
            alt: Some("System architecture".into()),
            width: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<figure class=\"surfdoc-figure\">"));
        assert!(html.contains("img src=\"arch.png\" alt=\"System architecture\""));
        assert!(html.contains("onerror="));
        assert!(html.contains("Architecture diagram"));
    }

    #[test]
    fn html_diagram_architecture_success() {
        let doc = doc_with(vec![Block::Diagram {
            diagram_type: "architecture".into(),
            title: Some("System Map".into()),
            content: "web: Web\napi: API\nweb -> api: HTTPS".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<figure class=\"surfdoc-diagram surfdoc-diagram-architecture\">"));
        assert!(html.contains("<figcaption class=\"surfdoc-diagram-cap\">System Map</figcaption>"));
        assert!(html.contains("<svg class=\"surfdoc-diagram-svg\""));
        assert!(html.contains("surfdoc-diagram-node"));
        assert!(!html.contains("surfdoc-diagram-fallback"));
    }

    #[test]
    fn html_diagram_erd_success() {
        let doc = doc_with(vec![Block::Diagram {
            diagram_type: "erd".into(),
            title: None,
            content: "users: id pk\norders: id pk, user_id fk\nusers 1--* orders".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<figure class=\"surfdoc-diagram surfdoc-diagram-erd\">"));
        assert!(html.contains("surfdoc-diagram-entity"));
        // No title attr -> no figcaption.
        assert!(!html.contains("surfdoc-diagram-cap"));
    }

    #[test]
    fn html_diagram_unknown_type_falls_back() {
        // render_block directly: page chrome has its own inline svg icons,
        // so the no-svg assertion must scope to the block fragment.
        let html = render_block(&Block::Diagram {
            diagram_type: "venn".into(),
            title: Some("Later".into()),
            content: "start => end".into(),
            span: span(),
        });
        assert!(html.contains("<figure class=\"surfdoc-diagram surfdoc-diagram-fallback\">"));
        assert!(html.contains("<figcaption class=\"surfdoc-diagram-cap\">Later</figcaption>"));
        assert!(html.contains("<pre class=\"surfdoc-diagram-src\">start =&gt; end</pre>"));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn html_diagram_malformed_dsl_falls_back() {
        // Malformed DSL must NEVER fail a render — prose fallback always.
        let doc = doc_with(vec![Block::Diagram {
            diagram_type: "architecture".into(),
            title: None,
            content: "this is not a node or an edge".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-diagram-fallback"));
        assert!(html.contains("<pre class=\"surfdoc-diagram-src\">this is not a node or an edge</pre>"));
    }

    #[test]
    fn html_diagram_chart_alias_renders_chart_svg() {
        // pie/donut/radar/xychart inside ::diagram forward to the ::chart
        // pipeline: diagram figure wrapper, chart SVG inside.
        for (alias, marker) in [
            ("pie", "surfdoc-chart-svg"),
            ("donut", "surfdoc-chart-svg"),
            ("radar", "surfdoc-chart-svg"),
            // xychart is an alias for the line chart.
            ("xychart", "surfdoc-chart-svg"),
        ] {
            let html = render_block(&Block::Diagram {
                diagram_type: alias.into(),
                title: Some("Share".into()),
                content: "Segment | Value\nA | 40\nB | 60".into(),
                span: span(),
            });
            assert!(
                html.contains(&format!("<figure class=\"surfdoc-diagram surfdoc-diagram-{alias}\">")),
                "{alias} keeps the diagram figure wrapper"
            );
            assert!(html.contains("<figcaption class=\"surfdoc-diagram-cap\">Share</figcaption>"));
            assert!(html.contains(marker), "{alias} renders through the chart pipeline");
            assert!(!html.contains("surfdoc-diagram-svg"), "{alias} is not a geometry scene");
            assert!(!html.contains("surfdoc-diagram-fallback"));
        }
    }

    #[test]
    fn html_diagram_chart_alias_bad_body_falls_back() {
        // A body the chart pipeline cannot use degrades to prose, exactly
        // like malformed geometry DSL.
        let html = render_block(&Block::Diagram {
            diagram_type: "pie".into(),
            title: None,
            content: "not a table".into(),
            span: span(),
        });
        assert!(html.contains("<figure class=\"surfdoc-diagram surfdoc-diagram-fallback\">"));
        assert!(html.contains("<pre class=\"surfdoc-diagram-src\">not a table</pre>"));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn html_diagram_stage2_kinds_render_svg() {
        for (kind, body, marker) in [
            ("timeline", "2026-01: Kickoff\n2026-02: Beta", "surfdoc-diagram-spine"),
            ("journey", "section S\nSign up: 3", "surfdoc-diagram-lane"),
            ("quadrant", "x-axis L --> R\nA: 0.4, 0.6", "surfdoc-diagram-frame"),
            ("kanban", "column Todo\n  Card", "surfdoc-diagram-column"),
            ("usecase", "actor u: User\nusecase c: Case\nu -> c", "surfdoc-diagram-boundary"),
        ] {
            let html = render_block(&Block::Diagram {
                diagram_type: kind.into(),
                title: None,
                content: body.into(),
                span: span(),
            });
            assert!(
                html.contains(&format!("<figure class=\"surfdoc-diagram surfdoc-diagram-{kind}\">")),
                "{kind} renders a diagram figure"
            );
            assert!(html.contains("<svg class=\"surfdoc-diagram-svg\""), "{kind} renders SVG");
            assert!(html.contains(marker), "{kind} missing its structural marker");
            assert!(!html.contains("surfdoc-diagram-fallback"));
        }
    }

    #[test]
    fn html_markdown_rendered() {
        let doc = doc_with(vec![Block::Markdown {
            content: "# Hello\n\nWorld".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        // Prose headings get an auto-generated anchor id (for TOC linking).
        assert!(html.contains("<h1 id=\"hello\">Hello</h1>"));
    }

    #[test]
    fn toc_auto_generates_from_headings() {
        // ::toc[depth=2] should list document headings up to level 2, linked to
        // the auto-generated heading anchors. Deeper headings (### …) are excluded.
        let doc = doc_with(vec![
            Block::Toc { depth: 2, entries: Vec::new(), span: span() },
            Block::Markdown {
                content: "# Overview\n\n## Setup\n\n### Detail\n\n## Usage".into(),
                span: span(),
            },
        ]);
        let html = to_html(&doc);
        // TOC is populated (no longer an empty <nav>) and carries a "Contents" label.
        assert!(html.contains("<nav class=\"surfdoc-toc\" data-depth=\"2\"><div class=\"surfdoc-toc-label\">Contents</div><ol>"));
        assert!(html.contains("<a href=\"#overview\">Overview</a>"));
        assert!(html.contains("<a href=\"#setup\">Setup</a>"));
        assert!(html.contains("<a href=\"#usage\">Usage</a>"));
        // Level-3 heading is below depth=2 → not in the TOC, but still anchored.
        assert!(!html.contains("#detail\">Detail</a>"));
        assert!(html.contains("<h3 id=\"detail\">Detail</h3>"));
        // Heading anchors match the TOC hrefs.
        assert!(html.contains("<h1 id=\"overview\">Overview</h1>"));
        assert!(html.contains("<h2 id=\"setup\">Setup</h2>"));
    }

    #[test]
    fn toc_dedupes_duplicate_slugs() {
        // Two headings with the same text get distinct anchors (foo, foo-2).
        let doc = doc_with(vec![
            Block::Toc { depth: 3, entries: Vec::new(), span: span() },
            Block::Markdown {
                content: "# Setup\n\n## Setup\n\n## Setup".into(),
                span: span(),
            },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("<h1 id=\"setup\">Setup</h1>"));
        assert!(html.contains("<h2 id=\"setup-2\">Setup</h2>"));
        assert!(html.contains("<h2 id=\"setup-3\">Setup</h2>"));
        assert!(html.contains("<a href=\"#setup-2\">Setup</a>"));
    }

    #[test]
    fn toc_empty_when_no_headings() {
        // With no document headings, the TOC stays an empty <nav> (no <ul>).
        let doc = doc_with(vec![
            Block::Toc { depth: 2, entries: Vec::new(), span: span() },
            Block::Markdown { content: "Just a paragraph.".into(), span: span() },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("<nav class=\"surfdoc-toc\" data-depth=\"2\"></nav>"));
        assert!(!html.contains("<ol>"));
    }

    #[test]
    fn html_escaping() {
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Info,
            title: None,
            content: "<script>alert('xss')</script>".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            !html.contains("<script>"),
            "Script tags must be escaped"
        );
        assert!(html.contains("&lt;script&gt;"));
    }

    // -- New block types (tabs, columns, quote) -------------------------

    #[test]
    fn html_tabs() {
        let doc = doc_with(vec![Block::Tabs {
            tabs: vec![
                crate::types::TabPanel {
                    label: "Overview".into(),
                    content: "Intro text.".into(),
                },
                crate::types::TabPanel {
                    label: "Details".into(),
                    content: "Technical info.".into(),
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-tabs\""));
        assert!(html.contains("Overview"));
        assert!(html.contains("Details"));
        assert!(html.contains("Intro text."));
        assert!(html.contains("Technical info."));
        assert!(html.contains("tab-btn"));
        assert!(html.contains("tab-panel"));
    }

    #[test]
    fn html_columns() {
        let doc = doc_with(vec![Block::Columns {
            columns: vec![
                crate::types::ColumnContent {
                    content: "Left side.".into(),
                },
                crate::types::ColumnContent {
                    content: "Right side.".into(),
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-columns\""));
        assert!(html.contains("data-cols=\"2\""));
        assert!(html.contains("class=\"surfdoc-column\""));
        assert!(html.contains("Left side."));
        assert!(html.contains("Right side."));
    }

    #[test]
    fn html_quote_with_attribution() {
        let doc = doc_with(vec![Block::Quote {
            content: "The best way to predict the future is to invent it.".into(),
            attribution: Some("Alan Kay".into()),
            cite: Some("ACM 1971".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<blockquote class=\"surfdoc-quote\">"));
        assert!(html.contains("<p>The best way to predict the future is to invent it.</p>"));
        assert!(html.contains("class=\"surfdoc-quote-by\""));
        assert!(html.contains("Alan Kay"));
        assert!(html.contains("<cite>ACM 1971</cite>"));
    }

    #[test]
    fn html_quote_no_attribution() {
        let doc = doc_with(vec![Block::Quote {
            content: "Anonymous wisdom.".into(),
            attribution: None,
            cite: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-quote\""));
        assert!(html.contains("Anonymous wisdom."));
        assert!(!html.contains("attribution"));
    }

    // -- Web blocks (cta, hero-image, testimonial, style) ---------------

    #[test]
    fn html_cta_primary() {
        let doc = doc_with(vec![Block::Cta {
            label: "Get Started".into(),
            href: "/signup".into(),
            primary: true,
            icon: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-cta surfdoc-cta-primary\""));
        assert!(html.contains("href=\"/signup\""));
        assert!(html.contains("Get Started"));
    }

    #[test]
    fn html_cta_secondary() {
        let doc = doc_with(vec![Block::Cta {
            label: "Learn More".into(),
            href: "https://example.com".into(),
            primary: false,
            icon: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-cta-secondary"));
        assert!(html.contains("Learn More"));
    }

    #[test]
    fn html_hero_image() {
        let doc = doc_with(vec![Block::HeroImage {
            src: "screenshot.png".into(),
            alt: Some("App screenshot".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-hero-image\""));
        assert!(html.contains("src=\"screenshot.png\""));
        assert!(html.contains("alt=\"App screenshot\""));
    }

    #[test]
    fn html_hero_no_image_backward_compatible() {
        // A hero with no image/layout attrs must render byte-identically to the
        // pre-image-alt/-layout output.
        let mk = |image_alt, layout| Block::Hero {
            headline: Some("CloudSurf".into()),
            subtitle: Some("Innovate".into()),
            badge: None,
            align: "center".into(),
            image: None,
            image_alt,
            layout,
            transparent: false,
            buttons: vec![],
            content: String::new(),
            span: span(),
        };
        let baseline = to_html(&doc_with(vec![mk(None, None)]));
        assert!(baseline.contains("<section class=\"surfdoc-hero\">"));
        assert!(!baseline.contains("surfdoc-hero-image"));
    }

    #[test]
    fn html_post_grid() {
        let doc = doc_with(vec![Block::PostGrid {
            title: Some("Blog".into()),
            subtitle: Some("Latest".into()),
            items: vec![
                PostItem {
                    title: "First".into(),
                    href: "/blog/first".into(),
                    meta: Some("News · 2026".into()),
                    excerpt: Some("An excerpt".into()),
                    image: Some("/img/a.png".into()),
                    external: true,
                },
                PostItem {
                    title: "Bare".into(),
                    href: "/blog/bare".into(),
                    meta: None,
                    excerpt: None,
                    image: None,
                    external: false,
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-post-grid\""));
        assert!(html.contains("class=\"surfdoc-pg2-inner\""));
        assert!(html.contains("<h1 class=\"surfdoc-post-grid-title\">Blog</h1>"));
        assert!(html.contains("class=\"surfdoc-post-cards\""));
        assert!(html.contains("href=\"/blog/first\""));
        assert!(html.contains("target=\"_blank\" rel=\"noopener\""));
        assert!(html.contains("style=\"--img:url('/img/a.png')\""));
        assert!(html.contains("<span class=\"surfdoc-post-card-meta\">News · 2026</span>"));
        assert!(html.contains("<h3 class=\"surfdoc-post-card-title\">First</h3>"));
        assert!(html.contains("<p class=\"surfdoc-post-card-excerpt\">An excerpt</p>"));
        assert!(html.contains("Read more"));
        // Bare card: no meta/excerpt/img.
        assert!(html.contains("<h3 class=\"surfdoc-post-card-title\">Bare</h3>"));
    }

    #[test]
    fn html_gate() {
        let doc = doc_with(vec![Block::Gate {
            title: Some("Members".into()),
            subtitle: Some("Enter code".into()),
            action: "/unlock".into(),
            field_label: Some("Your code".into()),
            submit_label: Some("Unlock".into()),
            error: Some("Wrong".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-gate\""));
        assert!(html.contains("class=\"surfdoc-gate-card\""));
        assert!(html.contains("<h1 class=\"surfdoc-gate-title\">Members</h1>"));
        assert!(html.contains("<p class=\"surfdoc-gate-subtitle\">Enter code</p>"));
        assert!(html.contains("<p class=\"surfdoc-gate-error\" role=\"alert\">Wrong</p>"));
        assert!(html.contains("method=\"post\" action=\"/unlock\""));
        // Password field name MUST be `code` (matches website AccessForm).
        assert!(html.contains("type=\"password\" name=\"code\""));
        assert!(html.contains("placeholder=\"Your code\""));
        assert!(html.contains(">Unlock</button>"));
    }

    #[test]
    fn html_gate_defaults() {
        let doc = doc_with(vec![Block::Gate {
            title: None,
            subtitle: None,
            action: String::new(),
            field_label: None,
            submit_label: None,
            error: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("name=\"code\""));
        assert!(html.contains("placeholder=\"Access code\""));
        assert!(html.contains(">Continue</button>"));
        assert!(!html.contains("surfdoc-gate-error"));
    }

    #[test]
    fn html_nav_minimal_renders_topbar_only() {
        let doc = doc_with(vec![Block::Nav {
            items: vec![nav_item("Home", "/", None)],
            logo: None,
            groups: vec![],
            brand: Some("Acme".into()),
            brand_reg: false,
            cta: None,
            drawer: false,
            minimal: true,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-shell-nav-minimal"));
        assert!(html.contains("surfdoc-shell-brand"));
        assert!(html.contains("onclick=\"toggleTheme()\""));
        // No hamburger, drawer, or scrim in minimal mode.
        assert!(!html.contains("toggleDrawer()"));
        assert!(!html.contains("surfdoc-shell-drawer"));
        assert!(!html.contains("surfdoc-shell-scrim"));
        assert!(!html.contains("surfdoc-shell-menu-btn"));
    }

    #[test]
    fn shell_page_reading_frame_wraps_when_set() {
        let shell = doc_with(vec![]);
        let mut config = PageConfig::default();
        config.reading_frame = Some(ReadingFrame {
            title: Some("White Paper".into()),
            back_href: Some("/papers".into()),
            back_label: Some("Papers".into()),
        });
        let html = to_shell_page(&shell, "<p>Body</p>", &config);
        assert!(html.contains("class=\"surfdoc-doc-viewer\""));
        assert!(html.contains("class=\"surfdoc-doc-toolbar\""));
        assert!(html.contains("href=\"/papers\""));
        assert!(html.contains("Papers"));
        assert!(html.contains("<span class=\"surfdoc-doc-title\">White Paper</span>"));
        assert!(html.contains("class=\"surfdoc-doc-page surfdoc\""));
        assert!(html.contains("<p>Body</p>"));
        // Framed pages unclamp <main> so the frame runs full-bleed.
        assert!(html.contains("<main class=\"surfdoc surfdoc-doc-main\">"));
    }

    #[test]
    fn shell_page_reading_frame_absent_by_default() {
        let shell = doc_with(vec![]);
        let config = PageConfig::default();
        let html = to_shell_page(&shell, "<p>Body</p>", &config);
        assert!(!html.contains("surfdoc-doc-viewer"));
        // Plain pages keep the bare .surfdoc main (document clamp applies).
        assert!(html.contains("<main class=\"surfdoc\">"));
        assert!(!html.contains("<main class=\"surfdoc surfdoc-doc-main\">"));
        assert!(html.contains("<p>Body</p>"));
    }

    #[test]
    fn html_hero_stacked_image_with_alt() {
        let doc = doc_with(vec![Block::Hero {
            headline: Some("CloudSurf".into()),
            subtitle: None,
            badge: None,
            align: "center".into(),
            image: Some("/logo.png".into()),
            image_alt: Some("CloudSurf logo".into()),
            layout: Some("stacked".into()),
            transparent: true,
            buttons: vec![],
            content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-hero-stacked"));
        assert!(html.contains("surfdoc-hero-transparent"));
        assert!(html.contains("class=\"surfdoc-hero-image\""));
        assert!(html.contains("src=\"/logo.png\""));
        assert!(html.contains("alt=\"CloudSurf logo\""));
    }

    #[test]
    fn html_form_action_method_honeypot() {
        let doc = doc_with(vec![Block::Form {
            fields: vec![],
            submit_label: Some("Send".into()),
            action: Some("/contact".into()),
            method: Some("post".into()),
            honeypot: true,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("method=\"post\""));
        assert!(html.contains("action=\"/contact\""));
        assert!(html.contains("name=\"_honey\""));
        assert!(html.contains(">Send</button>"));
    }

    #[test]
    fn html_form_no_action_omits_target() {
        let doc = doc_with(vec![Block::Form {
            fields: vec![],
            submit_label: None,
            action: None,
            method: None,
            honeypot: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<form class=\"surfdoc-form\">"));
        assert!(!html.contains("action="));
        assert!(!html.contains("_honey"));
    }

    #[test]
    fn html_banner_renders_band_with_buttons() {
        let doc = doc_with(vec![Block::Banner {
            headline: Some("Building something custom?".into()),
            subtitle: Some("Tell us what you need.".into()),
            buttons: vec![crate::types::HeroButton {
                label: "Start a project".into(),
                href: "/contact".into(),
                primary: true,
                external: false,
            }],
            id: Some("contact".into()),
            content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-banner\""));
        assert!(html.contains("id=\"contact\""));
        assert!(html.contains("surfdoc-banner-headline"));
        assert!(html.contains("Building something custom?"));
        assert!(html.contains("surfdoc-banner-btn-primary"));
        assert!(html.contains("href=\"/contact\""));
    }

    #[test]
    fn html_hero_external_button_opens_new_tab() {
        let src = "::hero\n# H\n\n[Go](https://x.com){primary external}\n[Stay](/here){primary}\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        assert!(html.contains(
            "href=\"https://x.com\" class=\"surfdoc-hero-btn surfdoc-hero-btn-primary\" target=\"_blank\" rel=\"noopener\""
        ));
        // The internal button must NOT pick up the target.
        assert!(html.contains("href=\"/here\" class=\"surfdoc-hero-btn surfdoc-hero-btn-primary\">"));
    }

    #[test]
    fn html_product_grid_linkless_card_renders_static_div() {
        let src = "::product-grid\n- Live | | /live | Ships\n- Soon | | - | Not public yet\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        // Linked card: an anchor with the arrow affordance.
        assert!(html.contains("<a href=\"/live\" class=\"surfdoc-pg-card\""));
        // Linkless card: a static div, no href, no arrow inside it.
        assert!(html.contains("<div class=\"surfdoc-pg-card surfdoc-pg-card-static\" data-product=\"soon\">"));
        assert!(!html.contains("href=\"-\""));
        let static_card = html.split("surfdoc-pg-card-static").nth(1).unwrap();
        let card_inner = static_card.split("</div>").next().unwrap();
        assert!(!card_inner.contains("surfdoc-pg-arrow"));
        // The static-card CSS ships with the sheet.
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-card-static"));
    }

    #[test]
    fn html_product_grid_tiles_icon_emblem_and_surface_bg() {
        let src = "::product-grid[tiles]\n- Innovate | icon:code | - | Invent things. | surface\n- Bad | icon:no-such-icon | - | x | surface\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        // Known icon → accent chip with the inline svg; no <img>.
        assert!(html.contains("<span class=\"surfdoc-pg-tile-icon\"><svg"));
        assert!(!html.contains("surfdoc-pg-tile-emblem"));
        // Unknown icon name → omitted entirely (never a broken img).
        assert_eq!(html.matches("surfdoc-pg-tile-icon").count(), 1);
        assert!(!html.contains("icon:no-such-icon"));
        // `surface` → class-based opaque bg, no inline style, no -bg ink pin.
        assert!(html.contains("surfdoc-pg-tile-surface"));
        assert!(!html.contains("surfdoc-pg-tile-bg"));
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-surface"));
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-icon"));
    }

    #[test]
    fn html_product_grid_tiles_theme_ink_token() {
        let src = "::product-grid[tiles]\n- A | | /a | x | gradient:linear-gradient(160deg,rgba(1,2,3,0.2),rgba(4,5,6,0.2)) theme\n- B | | /b | y | color:#123456 dark\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        // ` theme` → -bg class (background applies) + -theme (ink follows the
        // site theme); the token itself never leaks into the style attr.
        assert!(html.contains("surfdoc-pg-tile-bg surfdoc-pg-tile-theme"));
        assert!(html.contains("style=\"background:linear-gradient(160deg,rgba(1,2,3,0.2),rgba(4,5,6,0.2))\""));
        // ` dark` unaffected.
        assert!(html.contains("surfdoc-pg-tile-bg surfdoc-pg-tile-dark"));
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-theme"));
    }

    #[test]
    fn html_product_grid_classic_icon_emblem() {
        let src = "::product-grid\n- Thing | icon:globe | /thing | Does things\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        assert!(html.contains("<span class=\"surfdoc-pg-icon\"><svg"));
        assert!(!html.contains("surfdoc-pg-emblem"));
    }

    #[test]
    fn html_product_grid_tiles_linkless_card_drops_default_cta() {
        let src = "::product-grid[tiles]\n- Live | | /live | Ships\n- Soon | | - | Not public yet\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        // Linked tile keeps the default pill; linkless tile has none.
        assert!(html.contains("<a href=\"/live\" class=\"surfdoc-pg-tile-cta\""));
        assert!(!html.contains("href=\"-\""));
        let soon_tile = html.split("data-product=\"soon\"").nth(1).unwrap();
        let tile_inner = soon_tile.split("surfdoc-pg-tile-actions").nth(1).unwrap();
        assert!(!tile_inner.split("</span>").next().unwrap().contains("surfdoc-pg-tile-cta"));
    }

    #[test]
    fn html_product_grid_tiles_whole_card_stretch_link_and_hover_outline() {
        let src = "::product-grid[tiles]\n- Surfspace | | https://surf.space/ | Workspace. | | [Get started](https://surf.space/)\n- Grid | | - | Fleet.\n- Innovate | icon:code | - | Invent. | surface | [View Research](/research)\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        // A linked tile (explicit CTA or card href) is marked and carries the
        // invisible stretched anchor to its primary action; external stays
        // external, internal doesn't grow a target.
        assert!(html.contains("<a href=\"https://surf.space/\" class=\"surfdoc-pg-tile-stretch\" tabindex=\"-1\" aria-hidden=\"true\" target=\"_blank\" rel=\"noopener\"></a>"));
        assert!(html.contains("<a href=\"/research\" class=\"surfdoc-pg-tile-stretch\" tabindex=\"-1\" aria-hidden=\"true\"></a>"));
        assert_eq!(html.matches("surfdoc-pg-tile-linked").count(), 2);
        assert_eq!(html.matches("surfdoc-pg-tile-stretch").count(), 2);
        // The linkless tile keeps no affordance at all.
        let grid_tile = html.split("data-product=\"grid\"").nth(1).unwrap();
        assert!(!grid_tile.split("data-product").next().unwrap().contains("surfdoc-pg-tile-stretch"));
        // CSS ships the stretch geometry, the pointer-events layering, and the
        // hover outline with its host re-skin hook.
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-stretch { position: absolute; inset: 0;"));
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-linked .surfdoc-pg-tile-copy { pointer-events: none; }"));
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-linked .surfdoc-pg-tile-actions { pointer-events: auto; }"));
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-tile-linked:hover"));
        assert!(SURFDOC_CSS.contains("--ws-pg-tile-hover-outline"));
        // Hosts can round the tiles and re-skin/kill the hover glow; both
        // default to no-op (square, glow follows the outline accent).
        assert!(SURFDOC_CSS.contains("border-radius: var(--ws-pg-tile-radius, 0)"));
        assert!(SURFDOC_CSS.contains("--ws-pg-tile-hover-shadow"));
    }

    #[test]
    fn html_product_grid_tiles_with_backgrounds() {
        let src = "::product-grid[tiles]\n- Mac | | /mac | Now with M5. | image:/assets/mac.jpg dark\n- Watch | | /watch | Health. | color:#f5f5f7\n- Pad | | /pad | AI. | gradient:linear-gradient(180deg,#0af,#03d) dark\n::";
        let doc = crate::parse(src).doc;
        let html = to_html(&doc);
        // Tiles band, not the classic card grid
        assert!(html.contains("surfdoc-pg-tiles"));
        assert!(!html.contains("surfdoc-pg-card"));
        // image: → media div below the copy (custom prop); color/gradient → tile bg
        assert!(html.contains("surfdoc-pg-tile-media"));
        assert!(html.contains("--tile-image:url('/assets/mac.jpg')"));
        assert!(html.contains("background:#f5f5f7"));
        assert!(html.contains("background:linear-gradient(180deg,#0af,#03d)"));
        // ` dark` flag → light-text class on tiles 1 and 3 only
        assert_eq!(html.matches("surfdoc-pg-tile-dark").count(), 2);
        // Copy + CTA pill
        assert!(html.contains("surfdoc-pg-tile-name"));
        assert!(html.contains("Learn more"));
        // Round-trip: builder re-emits the [tiles] attr and the bg field
        let surf = crate::builder::to_surf_source(&doc);
        assert!(surf.contains("::product-grid[tiles]"));
        assert!(surf.contains("| image:/assets/mac.jpg dark"));
    }

    #[test]
    fn html_product_grid_groups_and_cards() {
        let doc = doc_with(vec![Block::ProductGrid {
            groups: vec![crate::types::ProductGroup {
                label: Some("Platforms".into()),
                cols: None,
                items: vec![
                    crate::types::ProductItem {
                        name: "Surf CLI".into(),
                        href: "https://app.surf".into(),
                        emblem: Some("/surf.png".into()),
                        tagline: Some("Docs & tasks.".into()),
                        cta1_label: None,
                        cta1_href: None,
                        cta2_label: None,
                        cta2_href: None,
                        bg: None,
                    },
                    crate::types::ProductItem {
                        name: "Local".into(),
                        href: "/local".into(),
                        emblem: None,
                        tagline: None,
                        cta1_label: None,
                        cta1_href: None,
                        cta2_label: None,
                        cta2_href: None,
                        bg: None,
                    },
                ],
            }],
            tiles: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-product-grid\""));
        assert!(html.contains("surfdoc-pg-label"));
        assert!(html.contains(">Platforms</h3>"));
        assert!(html.contains("data-product=\"surf-cli\""));
        assert!(html.contains("href=\"https://app.surf\""));
        assert!(html.contains("target=\"_blank\""));
        assert!(html.contains("surfdoc-pg-emblem"));
        // Internal link with no emblem: no target, no img.
        assert!(html.contains("data-product=\"local\""));
        assert!(html.contains("surfdoc-pg-arrow"));
    }

    #[test]
    fn html_testimonial() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "Amazing product!".into(),
            author: Some("Jane Dev".into()),
            role: Some("Engineer".into()),
            company: Some("Acme".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-testimonial\""));
        assert!(html.contains("Amazing product!"));
        assert!(html.contains("Jane Dev"));
        assert!(html.contains("Engineer, Acme"));
        // Avatar with initials derived from the author name ("Jane Dev" -> "JD").
        assert!(html.contains("class=\"surfdoc-testimonial-by\""));
        assert!(html.contains("class=\"surfdoc-testimonial-avatar\""));
        assert!(html.contains(">JD</span>"));
    }

    #[test]
    fn html_testimonial_anonymous() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "Great tool.".into(),
            author: None,
            role: None,
            company: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("Great tool."));
        assert!(!html.contains("class=\"author\""));
        // No author -> no avatar / figcaption.
        assert!(!html.contains("surfdoc-testimonial-avatar"));
        assert!(!html.contains("surfdoc-testimonial-by"));
    }

    #[test]
    fn html_style_hidden() {
        let doc = doc_with(vec![Block::Style {
            properties: vec![
                crate::types::StyleProperty { key: "accent".into(), value: "#6366f1".into() },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-style\""));
    }

    #[test]
    fn html_cta_escapes_xss() {
        let doc = doc_with(vec![Block::Cta {
            label: "<script>alert('xss')</script>".into(),
            href: "javascript:alert(1)".into(),
            primary: true,
            icon: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_faq() {
        let doc = doc_with(vec![Block::Faq {
            items: vec![
                crate::types::FaqItem {
                    question: "Is it free?".into(),
                    answer: "Yes, the free tier is forever.".into(),
                },
                crate::types::FaqItem {
                    question: "Can I self-host?".into(),
                    answer: "Docker image available.".into(),
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-faq\""));
        assert!(html.contains("<details class=\"surfdoc-faq-item\">"));
        assert!(html.contains("<summary>Is it free?</summary>"));
        assert!(html.contains("<summary>Can I self-host?</summary>"));
        assert!(html.contains("class=\"surfdoc-faq-answer\""));
        assert!(html.contains("Yes, the free tier is forever."));
    }

    #[test]
    fn html_pricing_table() {
        // Each row is a tier card. A bold name (**Team**) is the featured tier;
        // a `/suffix` price renders as a muted span; remaining cols are bullets.
        let doc = doc_with(vec![Block::PricingTable {
            headers: vec!["Tier".into(), "Price".into(), "Seats".into()],
            rows: vec![
                vec!["Free".into(), "$0".into(), "3".into()],
                vec!["**Team**".into(), "$9/seat".into(), "5".into()],
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-pricing\""));
        assert!(html.contains("<div class=\"surfdoc-tier-name\">Free</div>"));
        // Bold name → featured tier, with the ** stripped from the label.
        assert!(html.contains("surfdoc-tier surfdoc-tier-featured"));
        assert!(html.contains("<div class=\"surfdoc-tier-name\">Team</div>"));
        // Price `/seat` suffix becomes a muted span.
        assert!(html.contains("<div class=\"surfdoc-tier-price\">$9<span>/seat</span></div>"));
        // Feature bullet = "<value> <header lowercased>".
        assert!(html.contains("<li>3 seats</li>"));
        // Old table markup must be gone.
        assert!(!html.contains("<th scope=\"col\">"));
    }

    #[test]
    fn html_faq_escapes_xss() {
        let doc = doc_with(vec![Block::Faq {
            items: vec![crate::types::FaqItem {
                question: "<script>alert('q')</script>".into(),
                answer: "<img onerror=alert(1)>".into(),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_site_hidden() {
        let doc = doc_with(vec![Block::Site {
            domain: Some("notesurf.io".into()),
            properties: vec![
                crate::types::StyleProperty { key: "name".into(), value: "NoteSurf".into() },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-site\""));
        assert!(html.contains("data-domain=\"notesurf.io\""));
    }

    #[test]
    fn html_page_hero_layout() {
        let doc = doc_with(vec![Block::Page {
            route: "/".into(),
            layout: Some("hero".into()),
            title: None,
            sidebar: false,
            content: "# Welcome".into(),
            children: vec![
                Block::Markdown {
                    content: "# Welcome".into(),
                    span: span(),
                },
                Block::Cta {
                    label: "Get Started".into(),
                    href: "/signup".into(),
                    primary: true,
                    icon: None,
                    span: span(),
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-page\""));
        assert!(html.contains("data-layout=\"hero\""));
        assert!(html.contains("Get Started")); // CTA rendered
        assert!(html.contains("surfdoc-cta")); // CTA has class
    }

    #[test]
    fn html_page_renders_children() {
        let doc = doc_with(vec![Block::Page {
            route: "/pricing".into(),
            layout: None,
            title: Some("Pricing".into()),
            sidebar: false,
            content: String::new(),
            children: vec![
                Block::Markdown {
                    content: "# Pricing".into(),
                    span: span(),
                },
                Block::HeroImage {
                    src: "pricing.png".into(),
                    alt: Some("Plans".into()),
                    span: span(),
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<section class=\"surfdoc-page\" aria-label=\"Pricing\">"));
        assert!(html.contains("<h1 id=\"pricing\">Pricing</h1>")); // Markdown rendered (with anchor id)
        assert!(html.contains("surfdoc-hero-image")); // Hero image rendered
    }

    // -- Full-page discovery mechanism ---------------------------------

    #[test]
    fn html_page_has_generator_meta() {
        let doc = doc_with(vec![Block::Markdown {
            content: "# Hello".into(),
            span: span(),
        }]);
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta name=\"generator\" content=\"SurfDoc v0.1\">"));
    }

    #[test]
    fn html_page_has_link_alternate() {
        let doc = doc_with(vec![]);
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains(
            "<link rel=\"alternate\" type=\"text/surfdoc\" href=\"source.surf\">"
        ));
    }

    #[test]
    fn html_page_has_source_comment() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            source_path: "site.surf".to_string(),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.starts_with("<!-- Built with SurfDoc — source: site.surf -->"));
    }

    #[test]
    fn html_page_uses_front_matter_title() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("My Site".to_string()),
                ..Default::default()
            }),
            blocks: vec![],
            source: String::new(),
        };
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<title>My Site</title>"));
    }

    #[test]
    fn html_page_config_title_overrides_front_matter() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("FM Title".to_string()),
                ..Default::default()
            }),
            blocks: vec![],
            source: String::new(),
        };
        let config = PageConfig {
            title: Some("Override Title".to_string()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<title>Override Title</title>"));
        assert!(!html.contains("FM Title"));
    }

    #[test]
    fn html_page_has_doctype_and_structure() {
        let doc = doc_with(vec![Block::Markdown {
            content: "Hello".into(),
            span: span(),
        }]);
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("<meta charset=\"utf-8\">"));
        assert!(html.contains("<meta name=\"viewport\""));
        assert!(html.contains("<body>"));
        assert!(html.contains("</body>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn html_page_includes_description_and_canonical() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            description: Some("A test page".to_string()),
            canonical_url: Some("https://example.com/page".to_string()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta name=\"description\" content=\"A test page\">"));
        assert!(html.contains(
            "<link rel=\"canonical\" href=\"https://example.com/page\">"
        ));
    }

    #[test]
    fn html_page_custom_source_path() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            source_path: "/docs/readme.surf".to_string(),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("href=\"/docs/readme.surf\""));
        assert!(html.contains("source: /docs/readme.surf"));
    }

    #[test]
    fn html_page_escapes_title_xss() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            title: Some("<script>alert('xss')</script>".to_string()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    // -- SEO meta tag tests -----------------------------------------------

    #[test]
    fn html_page_og_tags_from_config() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            title: Some("My Page".into()),
            description: Some("A great page".into()),
            canonical_url: Some("https://example.com".into()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta property=\"og:title\" content=\"My Page\">"));
        assert!(html.contains("<meta property=\"og:description\" content=\"A great page\">"));
        assert!(html.contains("<meta property=\"og:url\" content=\"https://example.com\">"));
        assert!(html.contains("<meta property=\"og:type\" content=\"website\">"));
    }

    #[test]
    fn html_page_twitter_card_tags() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            title: Some("My Page".into()),
            description: Some("A great page".into()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta name=\"twitter:card\" content=\"summary\">"));
        assert!(html.contains("<meta name=\"twitter:title\" content=\"My Page\">"));
        assert!(html.contains("<meta name=\"twitter:description\" content=\"A great page\">"));
    }

    #[test]
    fn html_page_og_image() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            title: Some("My Page".into()),
            og_image: Some("https://example.com/img.png".into()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta property=\"og:image\" content=\"https://example.com/img.png\">"));
        assert!(html.contains("<meta name=\"twitter:image\" content=\"https://example.com/img.png\">"));
    }

    #[test]
    fn html_page_description_from_front_matter() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("FM Title".into()),
                description: Some("FM description".into()),
                ..Default::default()
            }),
            blocks: vec![],
            source: String::new(),
        };
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta name=\"description\" content=\"FM description\">"));
        assert!(html.contains("<meta property=\"og:description\" content=\"FM description\">"));
    }

    #[test]
    fn html_page_description_from_site_block() {
        let doc = doc_with(vec![Block::Site {
            domain: None,
            properties: vec![
                StyleProperty { key: "name".into(), value: "My Site".into() },
                StyleProperty { key: "description".into(), value: "Site block desc".into() },
            ],
            span: span(),
        }]);
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta name=\"description\" content=\"Site block desc\">"));
        assert!(html.contains("<meta property=\"og:description\" content=\"Site block desc\">"));
    }

    #[test]
    fn html_page_config_description_overrides_front_matter() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                description: Some("FM desc".into()),
                ..Default::default()
            }),
            blocks: vec![],
            source: String::new(),
        };
        let config = PageConfig {
            description: Some("Config desc".into()),
            ..Default::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("Config desc"));
        assert!(!html.contains("FM desc"));
    }

    // -- ARIA accessibility tests -----------------------------------------

    #[test]
    fn aria_callout_danger_role_alert() {
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Danger,
            title: None,
            content: "Critical error.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"alert\""));
    }

    #[test]
    fn aria_callout_info_role_note() {
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Info,
            title: None,
            content: "FYI.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"note\""));
    }

    #[test]
    fn aria_data_table_scope_col() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Col1".into()],
            rows: vec![],
            raw_content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("scope=\"col\""));
    }

    #[test]
    fn aria_code_label() {
        let doc = doc_with(vec![Block::Code {
            lang: Some("python".into()),
            file: None,
            highlight: vec![],
            content: "print()".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("aria-label=\"python code\""));
    }

    #[test]
    fn tasks_open_item_structure() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![TaskItem {
                done: false,
                text: "Do thing".into(),
                assignee: None,
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<li class=\"surfdoc-task\">"));
        assert!(html.contains("<span class=\"surfdoc-check\"></span>"));
        assert!(html.contains("<span class=\"surfdoc-task-text\">Do thing</span>"));
    }

    #[test]
    fn aria_decision_role_note() {
        let doc = doc_with(vec![Block::Decision {
            status: DecisionStatus::Accepted,
            date: None,
            deciders: vec![],
            content: "We decided.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"note\""));
        assert!(html.contains("aria-label=\"Decision: accepted\""));
    }

    #[test]
    fn html_decision_structure_and_deciders() {
        let doc = doc_with(vec![Block::Decision {
            status: DecisionStatus::Accepted,
            date: Some("2026-02-08".into()),
            deciders: vec!["Brady".into(), "Alice".into()],
            content: "We will ship.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<article class=\"surfdoc-decision surfdoc-decision-accepted\""));
        assert!(html.contains("<div class=\"surfdoc-decision-header\">"));
        assert!(html.contains(
            "<span class=\"surfdoc-decision-status surfdoc-decision-status--accepted\">Accepted</span>"
        ));
        assert!(html.contains("<span class=\"surfdoc-decision-date\">2026-02-08</span>"));
        assert!(html.contains("<div class=\"surfdoc-decision-body\">"));
        assert!(html.contains("<footer class=\"surfdoc-decision-meta\">Deciders: Brady, Alice</footer>"));
    }

    #[test]
    fn html_generic_embed_with_height_renders_iframe_not_card() {
        // A generic embed (no provider type) with a real src + explicit height —
        // e.g. a Termly policy viewer — must render a responsive <iframe>, NOT a
        // bare link-preview card. Regression: the 3 legal pages showed no content.
        let doc = doc_with(vec![Block::Embed {
            src: "https://app.termly.io/policy-viewer/policy.html?policyUUID=abc123".into(),
            embed_type: None,
            width: None,
            height: Some("80vh".into()),
            title: Some("Privacy Policy".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<iframe"), "generic embed must render an iframe");
        assert!(html.contains("src=\"https://app.termly.io/policy-viewer/policy.html?policyUUID=abc123\""));
        assert!(html.contains("height:80vh"), "explicit height must carry through");
        assert!(html.contains("title=\"Privacy Policy\""));
        assert!(html.contains("loading=\"lazy\""));
        // Must NOT degrade to a link-preview card.
        assert!(!html.contains("surfdoc-embed-title"), "generic embed must not be a link card");
    }

    #[test]
    fn html_generic_embed_explicit_type_renders_iframe() {
        // type=generic is equally first-class for iframe rendering.
        let doc = doc_with(vec![Block::Embed {
            src: "https://example.com/widget".into(),
            embed_type: Some(EmbedType::Generic),
            width: Some("600px".into()),
            height: Some("400px".into()),
            title: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<iframe"));
        assert!(html.contains("width:600px"));
        assert!(html.contains("height:400px"));
    }

    #[test]
    fn html_video_embed_stays_link_card() {
        // Provider embeds (video/audio/map) keep the safe link-card so
        // unembeddable share URLs don't produce an iframe error box.
        let doc = doc_with(vec![Block::Embed {
            src: "https://youtu.be/abc".into(),
            embed_type: Some(EmbedType::Video),
            width: None,
            height: Some("400px".into()),
            title: Some("My Video".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-embed-title"), "video embed stays a link card");
        assert!(!html.contains("<iframe"));
    }

    #[test]
    fn html_generic_embed_without_height_stays_card() {
        // No height → no responsive iframe target, so keep the link card.
        let doc = doc_with(vec![Block::Embed {
            src: "https://example.com/page".into(),
            embed_type: None,
            width: None,
            height: None,
            title: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-embed-title"));
        assert!(!html.contains("<iframe"));
    }

    #[test]
    fn html_decision_no_deciders_omits_footer() {
        let doc = doc_with(vec![Block::Decision {
            status: DecisionStatus::Proposed,
            date: None,
            deciders: vec![],
            content: "Maybe.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("surfdoc-decision-meta"));
        assert!(html.contains("surfdoc-decision-status--proposed"));
    }

    #[test]
    fn html_details_body_and_summary_classes() {
        let doc = doc_with(vec![Block::Details {
            title: Some("Notes".into()),
            open: true,
            content: "Body text.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<details class=\"surfdoc-details\" open>"));
        assert!(html.contains("<summary class=\"surfdoc-details-summary\">Notes</summary>"));
        assert!(html.contains("<div class=\"surfdoc-details-body\">"));
    }

    #[test]
    fn html_summary_has_label() {
        let doc = doc_with(vec![Block::Summary {
            content: "TL;DR here.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<div class=\"surfdoc-summary-label\">Summary</div>"));
    }

    #[test]
    fn is_numeric_cell_detection() {
        assert!(is_numeric_cell("30"));
        assert!(is_numeric_cell(" $0 "));
        assert!(is_numeric_cell("$6.99"));
        assert!(is_numeric_cell("12%"));
        assert!(is_numeric_cell("-5"));
        assert!(is_numeric_cell("1,200"));
        assert!(!is_numeric_cell("Free"));
        assert!(!is_numeric_cell(""));
        assert!(!is_numeric_cell("N/A"));
        assert!(!is_numeric_cell("$"));
    }

    #[test]
    fn aria_metric_group_label() {
        let doc = doc_with(vec![Block::Metric {
            label: "MRR".into(),
            value: "$5K".into(),
            trend: Some(Trend::Up),
            unit: Some("USD".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"group\""));
        assert!(html.contains("aria-label=\"MRR: $5K USD, trending up\""));
    }

    #[test]
    fn aria_summary_doc_abstract() {
        let doc = doc_with(vec![Block::Summary {
            content: "TL;DR.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"doc-abstract\""));
    }

    #[test]
    fn aria_tabs_tablist_pattern() {
        let doc = doc_with(vec![Block::Tabs {
            tabs: vec![
                TabPanel { label: "A".into(), content: "First.".into() },
                TabPanel { label: "B".into(), content: "Second.".into() },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"tablist\""));
        assert!(html.contains("role=\"tab\""));
        assert!(html.contains("role=\"tabpanel\""));
        assert!(html.contains("aria-selected=\"true\""));
        assert!(html.contains("aria-selected=\"false\""));
        assert!(html.contains("aria-controls=\"surfdoc-panel-0\""));
        assert!(html.contains("aria-labelledby=\"surfdoc-tab-0\""));
    }

    #[test]
    fn aria_hero_image_role_img() {
        let doc = doc_with(vec![Block::HeroImage {
            src: "hero.png".into(),
            alt: Some("Product shot".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"img\""));
        assert!(html.contains("aria-label=\"Product shot\""));
    }

    #[test]
    fn aria_testimonial_role_figure() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "Great!".into(),
            author: Some("Ada".into()),
            role: None,
            company: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"figure\""));
        assert!(html.contains("aria-label=\"Testimonial from Ada\""));
    }

    #[test]
    fn aria_style_hidden() {
        let doc = doc_with(vec![Block::Style {
            properties: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn aria_site_hidden() {
        let doc = doc_with(vec![Block::Site {
            domain: None,
            properties: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn aria_page_label_from_title() {
        let doc = doc_with(vec![Block::Page {
            route: "/about".into(),
            layout: None,
            title: Some("About Us".into()),
            sidebar: false,
            content: String::new(),
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("aria-label=\"About Us\""));
    }

    #[test]
    fn aria_page_label_from_route() {
        let doc = doc_with(vec![Block::Page {
            route: "/pricing".into(),
            layout: None,
            title: None,
            sidebar: false,
            content: String::new(),
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("aria-label=\"Page: /pricing\""));
    }

    #[test]
    fn aria_unknown_role_note() {
        let doc = doc_with(vec![Block::Unknown {
            name: "custom".into(),
            attrs: Default::default(),
            content: "stuff".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"note\""));
    }

    // -- p3-copy-sweep: unknown blocks + explicit heading anchors ----------

    #[test]
    fn unknown_block_renders_markdown_not_raw() {
        // A hallucinated block name (`::team`) renders its content as
        // markdown, not as a raw escaped dump with visible `##`.
        let doc = doc_with(vec![Block::Unknown {
            name: "team".into(),
            attrs: Default::default(),
            content: "## Crew\n- Ada\n- Lin".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        // Wrapper contract intact.
        assert!(html.contains("class=\"surfdoc-unknown\""), "{html}");
        assert!(html.contains("role=\"note\""), "{html}");
        assert!(html.contains("data-name=\"team\""), "{html}");
        // Content is rendered markdown.
        assert!(html.contains("Crew</h2>"), "heading rendered: {html}");
        assert!(html.contains("<li>Ada</li>"), "list rendered: {html}");
        assert!(!html.contains("## Crew"), "literal ## leaked: {html}");
        // Sanitizer still applies inside unknown blocks.
        let doc = doc_with(vec![Block::Unknown {
            name: "team".into(),
            attrs: Default::default(),
            content: "<script>alert(1)</script>Hi".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>alert"), "script must be stripped: {html}");
        assert!(html.contains("Hi"));
    }

    #[test]
    fn heading_anchor_brace_strips_and_sets_id() {
        let doc = doc_with(vec![
            Block::Markdown {
                content: "## Our Menu {#our-menu}\n\nFood.\n".into(),
                span: span(),
            },
            Block::Toc { depth: 2, entries: Vec::new(), span: span() },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("id=\"our-menu\""), "explicit anchor as id: {html}");
        assert!(!html.contains("{#"), "anchor braces leaked: {html}");
        // TOC label is the clean text.
        assert!(html.contains(">Our Menu</a>"), "clean TOC label: {html}");
        // Non-anchor braces are left untouched.
        assert_eq!(split_explicit_anchor("Costs {2 + 2}"), None);
        assert_eq!(split_explicit_anchor("Menu {#our menu}"), None);
        assert_eq!(
            split_explicit_anchor("Our Menu {#our-menu}"),
            Some(("Our Menu", "our-menu"))
        );
    }

    #[test]
    fn aria_pricing_table_label() {
        let doc = doc_with(vec![Block::PricingTable {
            headers: vec!["Tier".into(), "Price".into()],
            rows: vec![vec!["Basic".into(), "$0".into()]],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("aria-label=\"Pricing\""));
        assert!(html.contains("class=\"surfdoc-tier\""));
    }

    #[test]
    fn aria_columns_role_group() {
        let doc = doc_with(vec![Block::Columns {
            columns: vec![
                ColumnContent { content: "A".into() },
                ColumnContent { content: "B".into() },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("role=\"group\""));
    }

    // -- extract_site() unit tests -----------------------------------------

    #[test]
    fn extract_site_separates_blocks() {
        let doc = doc_with(vec![
            Block::Site {
                domain: Some("example.com".into()),
                properties: vec![
                    StyleProperty { key: "name".into(), value: "My Site".into() },
                    StyleProperty { key: "accent".into(), value: "#ff0000".into() },
                ],
                span: span(),
            },
            Block::Markdown {
                content: "Loose block".into(),
                span: span(),
            },
            Block::Page {
                route: "/".into(),
                layout: Some("hero".into()),
                title: Some("Home".into()),
                sidebar: false,
                content: "# Welcome".into(),
                children: vec![Block::Markdown {
                    content: "# Welcome".into(),
                    span: span(),
                }],
                span: span(),
            },
            Block::Page {
                route: "/about".into(),
                layout: None,
                title: Some("About".into()),
                sidebar: false,
                content: "# About".into(),
                children: vec![Block::Markdown {
                    content: "# About".into(),
                    span: span(),
                }],
                span: span(),
            },
        ]);

        let (site, pages, loose) = extract_site(&doc);

        // Site config extracted
        let site = site.expect("should have site config");
        assert_eq!(site.domain.as_deref(), Some("example.com"));
        assert_eq!(site.name.as_deref(), Some("My Site"));
        assert_eq!(site.accent.as_deref(), Some("#ff0000"));

        // Pages extracted
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].route, "/");
        assert_eq!(pages[0].title.as_deref(), Some("Home"));
        assert_eq!(pages[1].route, "/about");

        // Loose blocks
        assert_eq!(loose.len(), 1);
    }

    #[test]
    fn extract_site_no_site_block() {
        let doc = doc_with(vec![
            Block::Markdown {
                content: "Just markdown".into(),
                span: span(),
            },
        ]);

        let (site, pages, loose) = extract_site(&doc);
        assert!(site.is_none());
        assert!(pages.is_empty());
        assert_eq!(loose.len(), 1);
    }

    #[test]
    fn extract_site_config_fields() {
        let doc = doc_with(vec![Block::Site {
            domain: Some("test.io".into()),
            properties: vec![
                StyleProperty { key: "name".into(), value: "Test".into() },
                StyleProperty { key: "tagline".into(), value: "A tagline".into() },
                StyleProperty { key: "theme".into(), value: "dark".into() },
                StyleProperty { key: "accent".into(), value: "#00ff00".into() },
                StyleProperty { key: "font".into(), value: "inter".into() },
                StyleProperty { key: "custom".into(), value: "value".into() },
            ],
            span: span(),
        }]);

        let (site, _, _) = extract_site(&doc);
        let site = site.unwrap();
        assert_eq!(site.name.as_deref(), Some("Test"));
        assert_eq!(site.tagline.as_deref(), Some("A tagline"));
        assert_eq!(site.theme.as_deref(), Some("dark"));
        assert_eq!(site.accent.as_deref(), Some("#00ff00"));
        assert_eq!(site.font.as_deref(), Some("inter"));
        assert_eq!(site.properties.len(), 6); // all properties preserved
    }

    // -- render_site_page() unit tests ------------------------------------

    #[test]
    fn render_site_page_produces_valid_html() {
        let site = SiteConfig {
            name: Some("Test Site".into()),
            accent: Some("#3b82f6".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            children: vec![Block::Markdown {
                content: "# Hello World".into(),
                span: span(),
            }],
        };
        let nav_items = vec![
            ("/".into(), "Home".into()),
            ("/about".into(), "About".into()),
        ];
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &nav_items, &config);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("surfdoc-site-nav"));
        assert!(html.contains("Test Site"));
        assert!(html.contains("Hello World"));
        assert!(html.contains("surfdoc-site-footer"));
        assert!(html.contains("#3b82f6")); // accent override
    }

    #[test]
    fn render_site_page_theme_is_hint_not_pin() {
        // BR-SITE-THEME (flipped 2026-06-11, deliberately — was
        // `render_site_page_respects_theme_light`): `theme` no longer pins
        // `data-theme` on <html>. The pre-paint resolver decides instead:
        // stored per-site choice → device preference → this hint.
        let site = SiteConfig {
            name: Some("Dark-designed Site".into()),
            theme: Some("dark".into()),
            base_path: Some("/s/midnight-co".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            children: vec![],
        };
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &[], &config);

        assert!(
            !html.contains("data-theme=\"dark\"") || html.contains("setAttribute('data-theme'"),
            "no served data-theme pin"
        );
        assert!(
            html.contains("<html lang=\"en\">"),
            "the <html> tag carries NO data-theme attribute — the resolver sets it pre-paint"
        );
        // The hint survives as the resolver's no-preference fallback…
        assert!(html.contains(":'dark');"), "palette hint feeds the resolver fallback");
        // …and the persistence key is scoped to the site's base path.
        assert!(
            html.contains("localStorage.getItem('surfdoc-site-theme:/s/midnight-co')"),
            "per-site persistence key"
        );
    }

    #[test]
    fn render_site_page_no_theme_attr_when_unset() {
        let site = SiteConfig {
            name: Some("Default Site".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            children: vec![],
        };
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &[], &config);

        assert!(html.contains("<html lang=\"en\">"), "bare <html> tag when no theme set");
        // No theme + no base_path: resolver still emitted, hint defaults to
        // light, key scopes to "/".
        assert!(html.contains(":'light');"), "default hint is light");
        assert!(
            html.contains("localStorage.getItem('surfdoc-site-theme:/')"),
            "default key scopes to /"
        );
    }

    #[test]
    fn render_site_page_sets_accent_text_for_light_accent() {
        let site = SiteConfig {
            name: Some("Green Site".into()),
            accent: Some("#22c55e".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            children: vec![],
        };
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &[], &config);

        // Light accent (#22c55e) must get dark text for WCAG contrast
        assert!(html.contains("--accent-text: #1a1a2e"), "light accent must set dark --accent-text");
        assert!(html.contains("--accent: #22c55e"), "accent color must be set");
    }

    #[test]
    fn render_site_page_sets_accent_text_for_dark_accent() {
        let site = SiteConfig {
            name: Some("Blue Site".into()),
            accent: Some("#3b82f6".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            children: vec![],
        };
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &[], &config);

        // Dark accent (#3b82f6) must get white text
        assert!(html.contains("--accent-text: #fff"), "dark accent must set white --accent-text");
    }

    #[test]
    fn render_site_page_has_nav_links() {
        let site = SiteConfig {
            name: Some("Nav Test".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/about".into(),
            layout: None,
            title: Some("About".into()),
            sidebar: false,
            children: vec![],
        };
        let nav_items = vec![
            ("/".into(), "Home".into()),
            ("/about".into(), "About".into()),
            ("/pricing".into(), "Pricing".into()),
        ];
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &nav_items, &config);

        assert!(html.contains("href=\"/\""));
        assert!(html.contains("href=\"/about\""));
        assert!(html.contains("href=\"/pricing\""));
        // Active link for about page (carries aria-current for AT users);
        // the c1-drawer icon span sits between the tag and the label.
        assert!(html.contains("class=\"active\" aria-current=\"page\"><span class=\"site-nav-link-icon\""));
        assert!(html.contains("</span>About</a>"));
    }

    #[test]
    fn render_site_page_title_format() {
        let site = SiteConfig {
            name: Some("My Site".into()),
            ..Default::default()
        };

        // Page with title
        let page = PageEntry {
            route: "/about".into(),
            layout: None,
            title: Some("About Us".into()),
            sidebar: false,
            children: vec![],
        };
        let html = render_site_page(&page, &site, &[], &PageConfig::default());
        assert!(html.contains("<title>About Us — My Site</title>"));

        // Home page without title
        let home = PageEntry {
            route: "/".into(),
            layout: None,
            title: None,
            sidebar: false,
            children: vec![],
        };
        let html = render_site_page(&home, &site, &[], &PageConfig::default());
        assert!(html.contains("<title>My Site</title>"));
    }

    // -- Bug regression: CTA specificity fix (.surfdoc .surfdoc-cta beats .surfdoc a) --

    #[test]
    fn css_cta_selectors_use_parent_scope() {
        // The CSS must use `.surfdoc .surfdoc-cta-primary` (specificity 0-2-0) to beat
        // `.surfdoc a` (0-1-1). This works for both <a> and <button> elements.
        assert!(SURFDOC_CSS.contains(".surfdoc .surfdoc-cta-primary"));
        assert!(SURFDOC_CSS.contains(".surfdoc .surfdoc-cta-secondary"));
        assert!(SURFDOC_CSS.contains(".surfdoc .surfdoc-cta {"));
        // Must NOT use element-qualified selectors (a.surfdoc-cta) since <button> also uses these classes
        assert!(!SURFDOC_CSS.contains("a.surfdoc-cta"));
    }

    #[test]
    fn cta_renders_as_anchor_with_classes() {
        // CTA must render as <a> tag with both base and variant class so the
        // element-qualified CSS selectors match.
        let doc = doc_with(vec![Block::Cta {
            label: "Download".into(),
            href: "https://example.com/dl".into(),
            primary: true,
            icon: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<a "));
        assert!(html.contains("class=\"surfdoc-cta surfdoc-cta-primary\""));
        assert!(html.contains("href=\"https://example.com/dl\""));
    }

    #[test]
    fn cta_primary_css_sets_white_text() {
        // Verify the CSS uses --accent-text for ADA-compliant button text color
        assert!(SURFDOC_CSS.contains(".surfdoc .surfdoc-cta-primary { background: var(--accent); color: var(--accent-text, #fff);"));
    }

    /// DOG-3: hero/feature web-block component tokens are declared in `:root`
    /// AND consumed by their rules, so a consumer WaveSite can retheme these
    /// blocks via token VALUES (not specificity-tie overrides). Each `:root`
    /// default equals the literal it replaced, so unset == visual no-op.
    #[test]
    fn css_web_block_component_tokens_declared_and_consumed() {
        for (decl, default) in [
            ("--ws-hero-btn-radius:", "var(--ws-radius-btn)"),
            ("--ws-cta-radius:", "var(--ws-radius-btn)"),
            (
                "--ws-feature-card-bg:",
                "color-mix(in srgb, var(--surface) 50%, var(--surface-alt) 50%)",
            ),
            ("--ws-feature-card-pad:", "1.5rem"),
            ("--ws-feature-card-radius:", "var(--ws-radius-card)"),
            ("--ws-feature-card-hover-transform:", "translateY(-2px)"),
            // SS-1 (0.9.3): same pattern for the remaining component tokens —
            // each :root default == the per-element fallback it mirrors.
            ("--ws-tile-surface-bg:", "var(--surface)"),
            ("--ws-post-card-bg:", "var(--surface)"),
            ("--ws-post-card-radius:", "var(--ws-radius-card)"),
            ("--ws-pg-card-bg:", "color-mix(in srgb, var(--surface) 72%, transparent)"),
            ("--ws-pg-card-radius:", "var(--ws-radius-card-lg, 20px)"),
            ("--ws-pg-tile-radius:", "0"),
            ("--ws-details-bg:", "var(--surface-alt)"),
            ("--ws-details-radius:", "var(--radius-sm)"),
            ("--ws-form-input-bg:", "var(--surface)"),
            ("--ws-form-submit-radius:", "var(--ws-control-radius, var(--radius-sm))"),
            ("--ws-doc-page-bg:", "var(--surface)"),
            ("--ws-doc-page-radius:", "var(--ws-radius-card)"),
            ("--ws-drawer-link-size:", "0.9375rem"),
            ("--ws-drawer-link-weight:", "500"),
        ] {
            assert!(
                SURFDOC_CSS.contains(&format!("{decl} {default};")),
                "{decl} must be declared in :root with its no-op default `{default}`"
            );
        }
        // Consumed by the component rules (not just declared).
        assert!(SURFDOC_CSS.contains("border-radius: var(--ws-hero-btn-radius);"));
        assert!(SURFDOC_CSS.contains("border-radius: var(--ws-cta-radius);"));
        assert!(SURFDOC_CSS.contains("padding: var(--ws-feature-card-pad);"));
        assert!(SURFDOC_CSS.contains("border-radius: var(--ws-feature-card-radius);"));
        assert!(SURFDOC_CSS.contains("background: var(--ws-feature-card-bg);"));
        assert!(SURFDOC_CSS.contains("transform: var(--ws-feature-card-hover-transform);"));
        // SS-1 tokens keep their per-element consumption (fallback form), so
        // container-level overrides still reach the rules by VALUE.
        assert!(SURFDOC_CSS.contains("var(--ws-tile-surface-bg, var(--surface))"));
        assert!(SURFDOC_CSS.contains("var(--ws-form-input-bg, var(--surface))"));
        assert!(SURFDOC_CSS.contains("var(--ws-drawer-link-size, 0.9375rem)"));
        assert!(SURFDOC_CSS.contains("var(--ws-drawer-link-weight, 500)"));
        assert!(SURFDOC_CSS.contains("var(--ws-pg-tile-radius, 0)"));
        // --ws-control-radius is deliberately NOT :root-declared (divergent
        // per-element defaults: forms --radius-sm, app controls 8px).
        assert!(!SURFDOC_CSS.contains("--ws-control-radius:"));
    }

    /// The ::banner and ::product-grid blocks are emitted by render_html, so
    /// their structural geometry (band/inner/actions/button + grid/card/body/
    /// emblem/name/tagline/arrow) must ship with the crate — not live only in a
    /// consumer's stylesheet. Guards against the geometry regressing to
    /// consumer-only (the DOG-2 upstreaming).
    #[test]
    fn css_banner_and_product_grid_geometry_shipped() {
        for sel in [
            ".surfdoc-banner-inner {",
            ".surfdoc-banner-actions {",
            ".surfdoc-banner-btn {",
            ".surfdoc-banner-btn-secondary {",
            ".surfdoc-pg-inner {",
            ".surfdoc-pg-label {",
            ".surfdoc-pg-row {",
            ".surfdoc-pg-card {",
            ".surfdoc-pg-body {",
            ".surfdoc-pg-emblem {",
            ".surfdoc-pg-name {",
            ".surfdoc-pg-tagline {",
            ".surfdoc-pg-arrow {",
        ] {
            assert!(
                SURFDOC_CSS.contains(sel),
                "{sel} structural geometry must ship in surfdoc.css (DOG-2)"
            );
        }
        // Geometry references only standard surfdoc tokens (self-contained).
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-card { display: flex;"));
        assert!(SURFDOC_CSS.contains("background: var(--surface); border: 1px solid var(--border);"));
        // The 2-col grid collapses to 1 on mobile.
        assert!(SURFDOC_CSS.contains(".surfdoc-pg-row { grid-template-columns: 1fr; }"));
    }

    /// Regression: hero and product-card CTA buttons are `<a>` tags inside
    /// `.surfdoc`. Without the `.surfdoc` prefix their specificity (0,1,0)
    /// loses to `.surfdoc a { color: var(--accent) }` (0,1,1), making button
    /// text the same color as the background — invisible on accent-colored buttons.
    #[test]
    fn accent_colored_buttons_beat_link_color_specificity() {
        // Hero primary button must have .surfdoc prefix
        assert!(
            SURFDOC_CSS.contains(".surfdoc .surfdoc-hero-btn-primary {"),
            "hero primary button needs .surfdoc prefix to beat .surfdoc a specificity"
        );
        // Product card CTA must have .surfdoc prefix
        assert!(
            SURFDOC_CSS.contains(".surfdoc .surfdoc-product-cta {"),
            "product-card CTA needs .surfdoc prefix to beat .surfdoc a specificity"
        );
        // CTA primary (already correct, guard against regression)
        assert!(
            SURFDOC_CSS.contains(".surfdoc .surfdoc-cta-primary {"),
            "CTA primary needs .surfdoc prefix to beat .surfdoc a specificity"
        );
    }

    // -- Bug regression: web-block prose fields render inline markdown --------

    /// Regression: hero headline/subtitle and feature title/body went through
    /// escape_html() only, so `**bold**` / `*italic*` showed literal asterisks
    /// on published sites. Prose fields must run the inline markdown pass.
    #[test]
    fn web_block_prose_renders_inline_markdown() {
        let doc = doc_with(vec![
            Block::Hero {
                headline: Some("Home of **Scouter**".into()),
                subtitle: Some("Mixed by **Darrell Thorp**".into()),
                badge: None,
                align: "center".into(),
                image: None,
                image_alt: None,
                layout: None,
                transparent: false,
                buttons: vec![],
                content: String::new(),
                span: span(),
            },
            Block::Features {
                cards: vec![crate::types::FeatureCard {
                    title: "*Time Traveler*".into(),
                    icon: None,
                    body: "Mastered by **Darrell Thorp** — vinyl-ready.".into(),
                    link_label: None,
                    link_href: None,
                }],
                cols: None,
                span: span(),
            },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("<strong>Scouter</strong>"), "hero headline bold: {html}");
        assert!(html.contains("<strong>Darrell Thorp</strong>"), "hero subtitle / feature body bold");
        assert!(html.contains("<em>Time Traveler</em>"), "feature title italic");
        assert!(!html.contains("**"), "no literal asterisks may survive");
    }

    /// The inline pass must still escape raw HTML (injection safety) — the
    /// content is escaped BEFORE pulldown-cmark sees it.
    #[test]
    fn web_block_prose_escapes_raw_html() {
        let doc = doc_with(vec![Block::Hero {
            headline: Some("<script>alert(1)</script>".into()),
            subtitle: None,
            badge: None,
            align: "center".into(),
            image: None,
            image_alt: None,
            layout: None,
            transparent: false,
            buttons: vec![],
            content: String::new(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>"), "raw HTML must be escaped");
        assert!(html.contains("&lt;script&gt;"), "escaped form must be present");
    }

    // p1-fill-render: a FILL default containing markdown emphasis must not
    // let pulldown-cmark splice a <strong> tag into the marker body — that
    // breaks resolve_slot_markers's grammar (it bails on raw `<`/`>` inside
    // `«…»`) and the raw marker leaks straight into the hero H1.
    #[test]
    fn hero_headline_fill_marker_with_emphasis_resolves_cleanly() {
        let doc = doc_with(vec![Block::Hero {
            headline: Some("\u{ab}FILL: brand tag | Skate **or** Die\u{bb}".into()),
            subtitle: None,
            badge: None,
            align: "center".into(),
            image: None,
            image_alt: None,
            layout: None,
            transparent: false,
            buttons: vec![],
            content: String::new(),
            span: span(),
        }]);
        let html = crate::slots::resolve_slot_markers(to_html(&doc));
        assert!(
            !html.contains('\u{ab}') && !html.contains('\u{bb}'),
            "marker must not leak raw. Got: {html}"
        );
        assert!(
            html.contains("<h1 class=\"surfdoc-hero-headline\">"),
            "hero H1 must still render. Got: {html}"
        );
    }

    // -- Style-pack contract (rendering annex) ---------------------------------

    /// The --ws-* tokens are the style-pack contract: packs (container-injected
    /// overrides) re-set ONLY these. Defaults = Surf Simple (rounded-rect
    /// controls, 16px cards — the app-shell language). Web blocks must consume
    /// the tokens, not --radius-sm, or packs silently stop reaching them.
    #[test]
    fn style_pack_tokens_defined_with_surf_simple_defaults() {
        for decl in [
            "--ws-radius-card: 16px",
            "--ws-radius-btn: 10px",
            "--ws-radius-chip: 999px",
            "--ws-radius-img: 10px",
            "--ws-border-w: 1px",
            "--ws-shadow: 0 1px 2px rgba(15, 23, 42, 0.04)",
            "--ws-shadow-hover:",
            "--ws-bg-texture: none",
            "--ws-hero-bg:",
            // SS-1 (0.9.3) additive contract tokens — declared with their
            // identity (no-op) defaults so packs reach them by VALUE.
            "--ws-hero-btn-radius: var(--ws-radius-btn)",
            "--ws-banner-btn-radius: var(--radius-sm)",
            "--ws-cta-radius: var(--ws-radius-btn)",
            "--ws-form-submit-radius: var(--ws-control-radius, var(--radius-sm))",
            "--ws-feature-card-radius: var(--ws-radius-card)",
            "--ws-feature-card-pad: 1.5rem",
            "--ws-feature-card-hover-transform: translateY(-2px)",
            "--ws-feature-card-bg: color-mix(in srgb, var(--surface) 50%, var(--surface-alt) 50%)",
            "--ws-tile-surface-bg: var(--surface)",
            "--ws-post-card-bg: var(--surface)",
            "--ws-post-card-radius: var(--ws-radius-card)",
            "--ws-pg-card-bg: color-mix(in srgb, var(--surface) 72%, transparent)",
            "--ws-pg-card-radius: var(--ws-radius-card-lg, 20px)",
            "--ws-pg-tile-radius: 0",
            "--ws-details-bg: var(--surface-alt)",
            "--ws-details-radius: var(--radius-sm)",
            "--ws-form-input-bg: var(--surface)",
            "--ws-doc-page-bg: var(--surface)",
            "--ws-doc-page-radius: var(--ws-radius-card)",
            "--ws-drawer-link-size: 0.9375rem",
            "--ws-drawer-link-weight: 500",
        ] {
            assert!(SURFDOC_CSS.contains(decl), "missing contract token default: {decl}");
        }
    }

    #[test]
    fn web_blocks_consume_style_pack_tokens() {
        for (selector, token) in [
            (".surfdoc-hero {", "--ws-radius-card"),
            (".surfdoc-hero-badge {", "--ws-radius-chip"),
            // hero-btn/cta/feature-card now read a dedicated component token that
            // CHAINS to the shared radius default (DOG-3), so the style-pack
            // contract still holds transitively while staying consumer-overridable.
            (".surfdoc-hero-btn {", "--ws-hero-btn-radius"),
            (".surfdoc-feature-card {", "--ws-feature-card-radius"),
            (".surfdoc-stat {", "--ws-radius-card"),
            (".surfdoc-testimonial {", "--ws-radius-card"),
            (".surfdoc-product-card {", "--ws-radius-card"),
            (".surfdoc .surfdoc-cta {", "--ws-cta-radius"),
            (".surfdoc .surfdoc-product-cta {", "--ws-radius-btn"),
        ] {
            let rule = SURFDOC_CSS
                .split(selector)
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("selector not found: {selector}"));
            assert!(
                rule.contains(token),
                "{selector} must use var({token}), got: {rule}"
            );
        }
    }

    // -- Sections wrap h1/h2 boundaries (no background alternation) -----------

    #[test]
    fn sections_wrap_h1_boundaries() {
        let doc = doc_with(vec![
            Block::Markdown { content: "# Section One".into(), span: span() },
            Block::Markdown { content: "Content under section one.".into(), span: span() },
            Block::Markdown { content: "# Section Two".into(), span: span() },
            Block::Markdown { content: "Content under section two.".into(), span: span() },
        ]);
        let html = to_html(&doc);
        // Every section is a uniform .surfdoc-section — no auto-alternation.
        assert_eq!(html.matches("<section class=\"surfdoc-section\">").count(), 2);
        assert!(!html.contains("surfdoc-section-alt"));
        // Both sections should be closed
        assert_eq!(html.matches("</section>").count(), 2);
    }

    #[test]
    fn sections_wrap_h2_boundaries() {
        let doc = doc_with(vec![
            Block::Markdown { content: "## First".into(), span: span() },
            Block::Markdown { content: "Body A.".into(), span: span() },
            Block::Markdown { content: "## Second".into(), span: span() },
            Block::Markdown { content: "Body B.".into(), span: span() },
        ]);
        let html = to_html(&doc);
        assert_eq!(html.matches("<section class=\"surfdoc-section\">").count(), 2);
        assert!(!html.contains("surfdoc-section-alt"));
        assert_eq!(html.matches("</section>").count(), 2);
    }

    #[test]
    fn sections_do_not_alternate_backgrounds() {
        let doc = doc_with(vec![
            Block::Markdown { content: "# A".into(), span: span() },
            Block::Markdown { content: "# B".into(), span: span() },
            Block::Markdown { content: "# C".into(), span: span() },
        ]);
        let html = to_html(&doc);
        // No section ever gets the alt class (auto-alternation removed).
        assert_eq!(html.matches("surfdoc-section-alt").count(), 0);
        assert_eq!(html.matches("<section class=\"surfdoc-section\">").count(), 3);
        assert_eq!(html.matches("</section>").count(), 3);
    }

    #[test]
    fn no_sections_without_headings() {
        let doc = doc_with(vec![
            Block::Markdown { content: "Just a paragraph.".into(), span: span() },
            Block::Cta { label: "Go".into(), href: "/".into(), primary: true, icon: None, span: span() },
        ]);
        let html = to_html(&doc);
        assert!(!html.contains("<section"));
        assert!(!html.contains("</section>"));
    }

    #[test]
    fn section_css_exists() {
        assert!(SURFDOC_CSS.contains(".surfdoc-section {"));
        // Auto-alternating backgrounds were removed — the alt rule must be gone.
        assert!(!SURFDOC_CSS.contains(".surfdoc-section-alt"));
    }

    // -- Bug regression: to_html_page embeds CSS ------------------------------

    #[test]
    fn html_page_embeds_surfdoc_css() {
        let doc = doc_with(vec![Block::Markdown {
            content: "# Test".into(),
            span: span(),
        }]);
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        // Must contain key CSS rules from SURFDOC_CSS
        assert!(html.contains("<style>"));
        assert!(html.contains("--background:"));
        assert!(html.contains(".surfdoc {"));
        assert!(html.contains(".surfdoc .surfdoc-cta-primary"));
    }

    #[test]
    fn html_page_wraps_body_in_surfdoc_div() {
        let doc = doc_with(vec![Block::Markdown {
            content: "Hello".into(),
            span: span(),
        }]);
        let config = PageConfig::default();
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<article class=\"surfdoc\">"));
    }

    // -- Nav block tests --------------------------------------------------

    #[test]
    fn html_nav_renders_links() {
        let doc = doc_with(vec![Block::Nav {
            items: vec![
                crate::types::NavItem { label: "Home".into(), href: "/".into(), icon: None, image: None, external: false },
                crate::types::NavItem { label: "About".into(), href: "#about".into(), icon: None, image: None, external: false },
            ],
            logo: Some("MySite".into()),
            groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-nav\""));
        assert!(html.contains("surfdoc-nav-logo"));
        assert!(html.contains("MySite"));
        // New nav shell: sticky topbar + slide-in left drawer + theme toggle.
        assert!(html.contains("surfdoc-nav-topbar"));
        assert!(html.contains("surfdoc-nav-drawer"));
        assert!(html.contains("surfdoc-nav-theme-toggle"));
        // Toggle checkbox must be the first child so :checked ~ selectors work.
        let nav_open = html.find("<nav class=\"surfdoc-nav\"").unwrap();
        let toggle = html.find("class=\"surfdoc-nav-toggle\"").unwrap();
        let topbar = html.find("surfdoc-nav-topbar").unwrap();
        assert!(nav_open < toggle && toggle < topbar, "toggle must precede topbar");
        // Drawer rows come from the block's items.
        assert!(html.contains("href=\"/\""));
        assert!(html.contains("href=\"#about\""));
        assert!(html.contains("class=\"surfdoc-nav-drawer-row\""));
        assert!(html.contains(">Home</span></a>"));
        assert!(html.contains(">About</span></a>"));
    }

    #[test]
    fn html_nav_renders_before_sections() {
        let doc = doc_with(vec![
            Block::Markdown { content: "# Section One".into(), span: span() },
            Block::Nav {
                items: vec![
                    crate::types::NavItem { label: "Top".into(), href: "#top".into(), icon: None, image: None, external: false },
                ],
                logo: None,
                groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
                span: span(),
            },
        ]);
        let html = to_html(&doc);
        let nav_pos = html.find("surfdoc-nav").unwrap();
        let section_pos = html.find("surfdoc-section").unwrap();
        assert!(nav_pos < section_pos, "Nav must render before sections");
    }

    #[test]
    fn html_nav_uses_site_name_as_logo_fallback() {
        let doc = doc_with(vec![
            Block::Site {
                domain: None,
                properties: vec![StyleProperty { key: "name".into(), value: "Surf".into() }],
                span: span(),
            },
            Block::Nav {
                items: vec![
                    crate::types::NavItem { label: "Docs".into(), href: "/docs".into(), icon: None, image: None, external: false },
                ],
                logo: None,
                groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
                span: span(),
            },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-nav-logo"));
        assert!(html.contains("Surf"));
    }

    #[test]
    fn html_nav_with_icons() {
        let doc = doc_with(vec![Block::Nav {
            items: vec![
                crate::types::NavItem {
                    label: "GitHub".into(),
                    href: "https://github.com".into(),
                    icon: Some("github".into()),
                    image: None,
                    external: false,
                },
            ],
            logo: None,
            groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        // Icon support is preserved on drawer rows.
        assert!(html.contains("surfdoc-icon"));
        assert!(html.contains("<svg"));
        assert!(html.contains("surfdoc-nav-drawer-row"));
        assert!(html.contains(">GitHub</span></a>"));
    }

    #[test]
    fn html_nav_escapes_xss() {
        let doc = doc_with(vec![Block::Nav {
            items: vec![
                crate::types::NavItem {
                    label: "<script>alert('x')</script>".into(),
                    href: "javascript:alert(1)".into(),
                    icon: None,
                    image: None,
                    external: false,
                },
            ],
            logo: Some("<img onerror=alert(1)>".into()),
            groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>alert"));
        assert!(!html.contains("<img onerror"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_nav_css_exists() {
        assert!(SURFDOC_CSS.contains(".surfdoc-nav {"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-logo"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-topbar"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-drawer"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-theme-toggle"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-scrim"));
        assert!(SURFDOC_CSS.contains(".surfdoc-icon"));
    }

    // -- Rich shell nav/footer tests -------------------------------------

    fn nav_item(label: &str, href: &str, icon: Option<&str>) -> NavItem {
        NavItem {
            label: label.into(),
            href: href.into(),
            icon: icon.map(|s| s.into()),
            image: None,
            external: false,
        }
    }

    #[test]
    fn shell_nav_renders_groups_brand_cta_and_drawer() {
        let doc = doc_with(vec![Block::Nav {
            items: vec![],
            logo: Some("/logo.png".into()),
            groups: vec![
                NavGroup {
                    label: Some("Explore".into()),
                    items: vec![nav_item("Research", "/research", Some("flask"))],
                },
                NavGroup {
                    label: Some("Products".into()),
                    items: vec![NavItem {
                        label: "Surf".into(),
                        href: "https://app.surf".into(),
                        icon: None,
                        image: Some("/surf.png".into()),
                        external: true,
                    }],
                },
            ],
            brand: Some("CloudSurf".into()),
            brand_reg: true,
            cta: Some(nav_item("Get in touch", "#contact", None)),
            drawer: true,
            minimal: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        // Rich shell markup + JS hooks.
        assert!(html.contains("class=\"surfdoc-shell-nav\""));
        assert!(html.contains("onclick=\"toggleDrawer()\""));
        assert!(html.contains("onclick=\"toggleTheme()\""));
        assert!(html.contains("id=\"surfdoc-shell-drawer\""));
        assert!(html.contains("surfdoc-shell-scrim"));
        // Brand wordmark with ® superscript.
        assert!(html.contains("CloudSurf<sup class=\"surfdoc-shell-reg\">&reg;</sup>"));
        // Grouped labels + icon + product emblem + external target.
        assert!(html.contains(">Explore</span>"));
        assert!(html.contains(">Products</span>"));
        assert!(html.contains("surfdoc-shell-drawer-link-icon"));
        // Emblem is a masked span carrying the asset path via --emblem (no <img>).
        assert!(html.contains(
            "<span class=\"surfdoc-shell-drawer-link-emblem\" style=\"--emblem:url('/surf.png')\">"
        ));
        assert!(html.contains("target=\"_blank\""));
        // Per-page CTA.
        assert!(html.contains("surfdoc-shell-nav-cta"));
        assert!(html.contains(">Get in touch</a>"));
    }

    #[test]
    fn plain_nav_stays_legacy() {
        // No groups/brand/drawer → legacy checkbox nav (back-compat).
        let doc = doc_with(vec![Block::Nav {
            items: vec![nav_item("Home", "/", None)],
            logo: Some("Acme".into()),
            groups: vec![],
            brand: None,
            brand_reg: false,
            cta: None,
            drawer: false,
            minimal: false,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-nav\""));
        assert!(!html.contains("surfdoc-shell-nav"));
    }

    #[test]
    fn shell_footer_renders_brand_column() {
        let doc = doc_with(vec![Block::Footer {
            sections: vec![crate::types::FooterSection {
                heading: "Products".into(),
                links: vec![nav_item("Surf", "https://app.surf", None)],
            }],
            copyright: Some("(c) 2026 CloudSurf".into()),
            social: vec![],
            brand: Some("CloudSurf".into()),
            brand_reg: true,
            brand_logo: Some("/logo.png".into()),
            tagline: Some("Innovate • Simplify • Scale".into()),
            span: span(),
        }]);
        let html = render_block(&doc.blocks[0]);
        assert!(html.contains("surfdoc-shell-footer"));
        assert!(html.contains("surfdoc-shell-footer-brand-col"));
        assert!(html.contains("surfdoc-shell-footer-tagline"));
        assert!(html.contains("Innovate"));
        assert!(html.contains("surfdoc-shell-footer-heading"));
        assert!(html.contains("surfdoc-shell-footer-copyright"));
    }

    #[test]
    fn page_head_emits_icons_scripts_theme_init() {
        let doc = doc_with(vec![]);
        let config = PageConfig {
            theme_init: true,
            theme_key: Some("theme".into()),
            embed_css: false,
            twitter_card: Some("summary_large_image".into()),
            og_image: Some("https://x/og.png".into()),
            stylesheets: vec!["/static/css/surf-ui.css".into(), "/static/css/app.css?v=14".into()],
            scripts: vec![HeadScript { src: "/static/js/htmx.min.js".into(), defer: true }],
            icons: vec![HeadIcon {
                rel: "icon".into(),
                icon_type: None,
                sizes: Some("any".into()),
                href: "/favicon.ico".into(),
            }],
            ..PageConfig::default()
        };
        let html = to_html_page(&doc, &config);
        assert!(html.contains("<meta name=\"twitter:card\" content=\"summary_large_image\">"));
        assert!(html.contains("rel=\"icon\" sizes=\"any\" href=\"/favicon.ico\""));
        assert!(html.contains("<link rel=\"stylesheet\" href=\"/static/css/app.css?v=14\">"));
        assert!(html.contains("src=\"/static/js/htmx.min.js\" defer>"));
        assert!(html.contains("function toggleTheme()"));
        assert!(html.contains("function toggleDrawer()"));
        // embed_css=false → no inlined base stylesheet.
        assert!(!html.contains("<style>"));
    }

    #[test]
    fn page_config_from_site_doc_reads_head() {
        let doc = doc_with(vec![Block::Site {
            domain: Some("cloudsurf.com".into()),
            properties: vec![
                StyleProperty { key: "og-image".into(), value: "/og.png".into() },
                StyleProperty { key: "twitter-card".into(), value: "summary_large_image".into() },
                StyleProperty { key: "theme-init".into(), value: "true".into() },
                StyleProperty { key: "theme-key".into(), value: "theme".into() },
                StyleProperty { key: "favicon".into(), value: "/favicon.ico".into() },
                StyleProperty { key: "stylesheet".into(), value: "/a.css".into() },
                StyleProperty { key: "stylesheet".into(), value: "/b.css".into() },
                StyleProperty { key: "script-defer".into(), value: "/x.js".into() },
            ],
            span: span(),
        }]);
        let cfg = PageConfig::from_site_doc(&doc);
        assert_eq!(cfg.og_image.as_deref(), Some("/og.png"));
        assert_eq!(cfg.twitter_card.as_deref(), Some("summary_large_image"));
        assert!(cfg.theme_init);
        assert_eq!(cfg.theme_key.as_deref(), Some("theme"));
        assert_eq!(cfg.icons.len(), 1);
        assert_eq!(cfg.stylesheets, vec!["/a.css".to_string(), "/b.css".to_string()]);
        assert_eq!(cfg.scripts.len(), 1);
        assert!(cfg.scripts[0].defer);
    }

    // -- Icon on CTA tests ------------------------------------------------

    #[test]
    fn html_cta_with_icon() {
        let doc = doc_with(vec![Block::Cta {
            label: "Download".into(),
            href: "/dl".into(),
            primary: true,
            icon: Some("download".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-icon"));
        assert!(html.contains("<svg"));
        assert!(html.contains("Download</a>"));
    }

    #[test]
    fn html_cta_unknown_icon_omitted() {
        let doc = doc_with(vec![Block::Cta {
            label: "Go".into(),
            href: "/go".into(),
            primary: true,
            icon: Some("nonexistent-icon".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("surfdoc-icon"));
        assert!(html.contains(">Go</a>"));
    }

    #[test]
    fn html_cta_no_icon_no_svg() {
        let doc = doc_with(vec![Block::Cta {
            label: "Click".into(),
            href: "/click".into(),
            primary: false,
            icon: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("surfdoc-icon"));
        assert!(!html.contains("<svg"));
    }

    // -- Features icon tests ----------------------------------------------

    #[test]
    fn html_features_with_known_icon() {
        let doc = doc_with(vec![Block::Features {
            cards: vec![crate::types::FeatureCard {
                title: "Fast".into(),
                icon: Some("zap".into()),
                body: "Lightning fast".into(),
                link_label: None,
                link_href: None,
            }],
            cols: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-feature-icon"), "should have icon wrapper");
        assert!(html.contains("<svg"), "should contain inline SVG");
        assert!(!html.contains(">zap<"), "should NOT render icon name as text");
    }

    #[test]
    fn html_features_with_unknown_icon_omitted() {
        let doc = doc_with(vec![Block::Features {
            cards: vec![crate::types::FeatureCard {
                title: "Mystery".into(),
                icon: Some("nonexistent-icon".into()),
                body: "No icon".into(),
                link_label: None,
                link_href: None,
            }],
            cols: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("surfdoc-feature-icon"), "unknown icon should be omitted");
        assert!(!html.contains("nonexistent-icon"), "icon name should not appear as text");
        assert!(html.contains("Mystery"), "title should still render");
    }

    #[test]
    fn html_features_no_icon_no_svg() {
        let doc = doc_with(vec![Block::Features {
            cards: vec![crate::types::FeatureCard {
                title: "Plain".into(),
                icon: None,
                body: "No icon".into(),
                link_label: None,
                link_href: None,
            }],
            cols: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("surfdoc-feature-icon"));
        assert!(!html.contains("<svg"));
        assert!(html.contains("Plain"));
    }

    #[test]
    fn html_features_new_icons_resolve() {
        // Test all the newly added icons render as SVGs
        let new_icons = &[
            "clock", "edit", "pencil", "shield", "zap", "lock", "phone",
            "map-pin", "calendar", "users", "truck", "message-circle",
            "image", "briefcase", "award", "layers", "package",
            "trending-up", "coffee", "scissors", "wrench",
        ];
        for icon_name in new_icons {
            let doc = doc_with(vec![Block::Features {
                cards: vec![crate::types::FeatureCard {
                    title: format!("Test {}", icon_name),
                    icon: Some(icon_name.to_string()),
                    body: String::new(),
                    link_label: None,
                    link_href: None,
                }],
                cols: None,
                span: span(),
            }]);
            let html = to_html(&doc);
            assert!(
                html.contains("<svg"),
                "Icon '{}' should render as SVG in features block",
                icon_name
            );
            assert!(
                html.contains("surfdoc-feature-icon"),
                "Icon '{}' should have feature-icon wrapper",
                icon_name
            );
        }
    }

    #[test]
    fn html_features_edit_and_pencil_are_same() {
        let doc_edit = doc_with(vec![Block::Features {
            cards: vec![crate::types::FeatureCard {
                title: "Edit".into(),
                icon: Some("edit".into()),
                body: String::new(),
                link_label: None,
                link_href: None,
            }],
            cols: None,
            span: span(),
        }]);
        let doc_pencil = doc_with(vec![Block::Features {
            cards: vec![crate::types::FeatureCard {
                title: "Edit".into(),
                icon: Some("pencil".into()),
                body: String::new(),
                link_label: None,
                link_href: None,
            }],
            cols: None,
            span: span(),
        }]);
        let html_edit = to_html(&doc_edit);
        let html_pencil = to_html(&doc_pencil);
        // Both should produce the same SVG
        assert!(html_edit.contains("<svg"));
        assert_eq!(html_edit, html_pencil);
    }

    // -- Font preset tests ------------------------------------------------

    #[test]
    fn font_presets_resolve() {
        assert!(resolve_font_preset("system").unwrap().stack.contains("apple-system"));
        assert!(resolve_font_preset("sans").unwrap().stack.contains("apple-system"));
        assert!(resolve_font_preset("serif").unwrap().stack.contains("Georgia"));
        assert!(resolve_font_preset("editorial").unwrap().stack.contains("Georgia"));
        assert!(resolve_font_preset("mono").unwrap().stack.contains("Menlo"));
        assert!(resolve_font_preset("monospace").unwrap().stack.contains("Menlo"));
        assert!(resolve_font_preset("technical").unwrap().stack.contains("Menlo"));
        assert!(resolve_font_preset("inter").unwrap().stack.contains("Inter"));
        assert!(resolve_font_preset("montserrat").unwrap().stack.contains("Montserrat"));
        assert!(resolve_font_preset("jetbrains-mono").unwrap().stack.contains("JetBrains Mono"));
        assert!(resolve_font_preset("playfair").unwrap().stack.contains("Playfair Display"));
        assert!(resolve_font_preset("playfair-display").unwrap().stack.contains("Playfair Display"));
        assert!(resolve_font_preset("lato").unwrap().stack.contains("Lato"));
        assert!(resolve_font_preset("unknown").is_none());
    }

    #[test]
    fn font_presets_case_insensitive() {
        assert!(resolve_font_preset("Serif").is_some());
        assert!(resolve_font_preset("MONO").is_some());
        assert!(resolve_font_preset("System").is_some());
        assert!(resolve_font_preset("Inter").is_some());
    }

    #[test]
    fn google_font_presets_have_imports() {
        assert!(resolve_font_preset("inter").unwrap().import.is_some());
        assert!(resolve_font_preset("montserrat").unwrap().import.is_some());
        assert!(resolve_font_preset("jetbrains-mono").unwrap().import.is_some());
        assert!(resolve_font_preset("playfair").unwrap().import.is_some());
        assert!(resolve_font_preset("lato").unwrap().import.is_some());
        // System fonts have no imports
        assert!(resolve_font_preset("system").unwrap().import.is_none());
        assert!(resolve_font_preset("serif").unwrap().import.is_none());
    }

    #[test]
    fn style_block_sets_heading_font() {
        let doc = doc_with(vec![
            Block::Style {
                properties: vec![StyleProperty { key: "heading-font".into(), value: "serif".into() }],
                span: span(),
            },
            Block::Markdown { content: "# Hello".into(), span: span() },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("--font-heading: Georgia"));
    }

    #[test]
    fn style_block_sets_body_font() {
        let doc = doc_with(vec![
            Block::Style {
                properties: vec![StyleProperty { key: "body-font".into(), value: "mono".into() }],
                span: span(),
            },
            Block::Markdown { content: "Hello".into(), span: span() },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("--font-body:"));
        assert!(html.contains("Menlo"));
    }

    #[test]
    fn font_legacy_sets_both() {
        let doc = doc_with(vec![Block::Site {
            domain: None,
            properties: vec![StyleProperty { key: "font".into(), value: "serif".into() }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("--font-heading: Georgia"));
        assert!(html.contains("--font-body: Georgia"));
    }

    #[test]
    fn css_has_font_variables() {
        assert!(SURFDOC_CSS.contains("--font-heading:"));
        assert!(SURFDOC_CSS.contains("--font-body:"));
        // Body/display fonts route through the style-pack hooks (--ws-font-body
        // / --ws-font-display), which default to the base stacks — so a pack can
        // swap the display/body face while Surf stays byte-identical.
        assert!(SURFDOC_CSS.contains("font-family: var(--ws-font-body)"));
        assert!(SURFDOC_CSS.contains("font-family: var(--ws-font-display)"));
        assert!(SURFDOC_CSS.contains("--ws-font-body: var(--font-body)"));
        assert!(SURFDOC_CSS.contains("--ws-font-display: var(--font-heading)"));
        // h5/h6 micro-eyebrows stay on the UI heading font.
        assert!(SURFDOC_CSS.contains("font-family: var(--font-heading)"));
    }

    #[test]
    fn css_type_scale_style_v2_defaults() {
        // Style-v2 scale, proven on cloudsurf.com then upstreamed as the spec
        // default (every step ~+2–6px over the pre-v2 values).
        assert!(SURFDOC_CSS.contains("--font-size-body: 17px"));
        assert!(SURFDOC_CSS.contains("--font-size-h3: 21px"));
        assert!(SURFDOC_CSS.contains("--font-size-h2: 24px"));
        assert!(SURFDOC_CSS.contains("--font-size-h1: 28px"));
        assert!(SURFDOC_CSS.contains("--font-size-display: 38px"));
        // Display headings h1–h3 + the hero headline default to the black
        // weight; packs still win via --ws-heading-weight.
        assert!(SURFDOC_CSS.contains("--font-weight-black: 900"));
        assert!(SURFDOC_CSS.contains("var(--ws-heading-weight, var(--font-weight-black))"));
        assert!(SURFDOC_CSS.contains("font-size: clamp(2rem, 4.5vw, 3rem)"));
        // The pre-v2 hero clamp must not linger anywhere.
        assert!(!SURFDOC_CSS.contains("clamp(2.25rem, 5vw, 3.5rem)"));
    }

    #[test]
    fn css_glass_surface_token_hooks() {
        // Consumer-overridable surface/radius hooks (proven on cloudsurf.com's
        // glass-over-sky skin, upstreamed as no-op inline fallbacks): each
        // resolves to the pre-hook literal unless the consumer sets a value.
        assert!(SURFDOC_CSS.contains("var(--ws-post-card-bg, var(--surface))"));
        assert!(SURFDOC_CSS.contains("var(--ws-post-card-radius, var(--ws-radius-card))"));
        assert!(SURFDOC_CSS.contains("var(--ws-pg-card-radius, var(--ws-radius-card-lg, 20px))"));
        assert!(SURFDOC_CSS.contains("var(--ws-form-input-bg, var(--surface))"));
        assert!(SURFDOC_CSS.contains("var(--ws-form-submit-radius, var(--ws-control-radius, var(--radius-sm)))"));
        assert!(SURFDOC_CSS.contains("var(--ws-details-bg, var(--surface-alt))"));
        assert!(SURFDOC_CSS.contains("var(--ws-details-radius, var(--radius-sm))"));
        assert!(SURFDOC_CSS.contains("var(--ws-doc-page-bg, var(--surface))"));
        assert!(SURFDOC_CSS.contains("var(--ws-doc-page-radius, var(--ws-radius-card))"));
    }

    // -- Bug regression: accent color must not leak into editor chrome ---------

    #[test]
    fn accent_override_scoped_to_surfdoc_not_root() {
        let doc = doc_with(vec![Block::Site {
            domain: None,
            properties: vec![StyleProperty {
                key: "accent".into(),
                value: "#ec4899".into(),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        // Must scope to .surfdoc, NOT :root
        assert!(html.contains("<style>.surfdoc { --accent: #ec4899;--accent-text: #fff; }</style>"),
            "accent override should be scoped to .surfdoc with accent-text, got: {}", html);
        assert!(!html.contains(":root { --accent:"),
            "accent override must NOT use :root (leaks into editor chrome)");
    }

    #[test]
    fn no_style_tag_without_overrides() {
        let doc = doc_with(vec![Block::Markdown {
            content: "Hello".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<style>"),
            "no style tag when there are no CSS overrides");
    }

    // -- humanize_route() unit tests ----------------------------------------

    #[test]
    fn humanize_route_home() {
        assert_eq!(humanize_route("/"), "Home");
    }

    #[test]
    fn humanize_route_simple() {
        assert_eq!(humanize_route("/gallery"), "Gallery");
    }

    #[test]
    fn humanize_route_hyphenated() {
        assert_eq!(humanize_route("/about-us"), "About Us");
    }

    #[test]
    fn humanize_route_contact() {
        assert_eq!(humanize_route("/contact"), "Contact");
    }

    #[test]
    fn humanize_route_multi_hyphen() {
        assert_eq!(humanize_route("/terms-of-service"), "Terms Of Service");
    }

    #[test]
    fn humanize_route_no_leading_slash() {
        assert_eq!(humanize_route("pricing"), "Pricing");
    }

    #[test]
    fn humanize_route_trailing_slash() {
        assert_eq!(humanize_route("/blog/"), "Blog");
    }

    #[test]
    fn humanize_route_empty_string() {
        assert_eq!(humanize_route(""), "Home");
    }

    // -- PageEntry::display_title() tests -----------------------------------

    #[test]
    fn display_title_uses_explicit_title() {
        let page = PageEntry {
            route: "/about".into(),
            layout: None,
            title: Some("About Our Team".into()),
            sidebar: false,
            children: vec![],
        };
        assert_eq!(page.display_title(), "About Our Team");
    }

    #[test]
    fn display_title_falls_back_to_humanized_route() {
        let page = PageEntry {
            route: "/about-us".into(),
            layout: None,
            title: None,
            sidebar: false,
            children: vec![],
        };
        assert_eq!(page.display_title(), "About Us");
    }

    #[test]
    fn display_title_home_route() {
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: None,
            sidebar: false,
            children: vec![],
        };
        assert_eq!(page.display_title(), "Home");
    }

    // -- render_site_page title uses humanize_route -------------------------

    #[test]
    fn render_site_page_humanizes_untitled_page() {
        let site = SiteConfig {
            name: Some("My Site".into()),
            ..Default::default()
        };
        let page = PageEntry {
            route: "/about-us".into(),
            layout: None,
            title: None,
            sidebar: false,
            children: vec![],
        };
        let html = render_site_page(&page, &site, &[], &PageConfig::default());
        assert!(html.contains("<title>About Us — My Site</title>"));
    }

    // -- E2E: full parse → extract → render pipeline -----------------------

    #[test]
    fn e2e_multipage_site_nav_labels() {
        let source = r#"::site
name = "E2E Test"
::

::page[route="/"]
# Home
::

::page[route="/gallery"]
# Photos
::

::page[route="/about-us"]
About our company
::

::page[route="/terms-of-service"]
Legal text
::"#;

        let result = crate::parse(source);
        let (site_config, pages, _) = extract_site(&result.doc);
        let site = site_config.unwrap();

        // Build nav items the way all consumers should
        let nav_items: Vec<(String, String)> = pages
            .iter()
            .map(|p| (p.route.clone(), p.display_title()))
            .collect();

        assert_eq!(nav_items.len(), 4);
        assert_eq!(nav_items[0], ("/".into(), "Home".into()));
        assert_eq!(nav_items[1], ("/gallery".into(), "Gallery".into()));
        assert_eq!(nav_items[2], ("/about-us".into(), "About Us".into()));
        assert_eq!(nav_items[3], ("/terms-of-service".into(), "Terms Of Service".into()));

        // Render home page and verify nav in HTML
        let config = PageConfig::default();
        let html = render_site_page(&pages[0], &site, &nav_items, &config);

        assert!(html.contains(">Home</a>"));
        assert!(html.contains(">Gallery</a>"));
        assert!(html.contains(">About Us</a>"));
        assert!(html.contains(">Terms Of Service</a>"));
    }

    #[test]
    fn e2e_explicit_titles_not_overridden() {
        let source = r#"::site
name = "Title Test"
::

::page[route="/" title="Welcome"]
# Welcome
::

::page[route="/team" title="Our Team"]
# Team
::"#;

        let result = crate::parse(source);
        let (_, pages, _) = extract_site(&result.doc);

        let nav_items: Vec<(String, String)> = pages
            .iter()
            .map(|p| (p.route.clone(), p.display_title()))
            .collect();

        assert_eq!(nav_items[0].1, "Welcome");
        assert_eq!(nav_items[1].1, "Our Team");
    }

    #[test]
    fn e2e_all_consumers_get_same_nav() {
        // Simulates the three consumer patterns:
        // 1. Wavesite (publish/preview): pages.iter().map(|p| (p.route.clone(), p.display_title()))
        // 2. Surf Browser: same pattern (after fix)
        // 3. iOS: delegates to server's /api/preview which uses pattern 1

        let source = r#"::site
name = "Consistency"
::

::page[route="/"]
Home
::

::page[route="/about-us"]
About
::"#;

        let result = crate::parse(source);
        let (_, pages, _) = extract_site(&result.doc);

        // All consumers use display_title() — verify it's deterministic
        for _ in 0..3 {
            let labels: Vec<String> = pages.iter().map(|p| p.display_title()).collect();
            assert_eq!(labels, vec!["Home", "About Us"]);
        }
    }

    #[test]
    fn html_hero_image_missing_alt_renders_empty() {
        let doc = doc_with(vec![Block::HeroImage {
            src: "https://example.com/photo.jpg".into(),
            alt: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("alt=\"\""), "Missing alt should render as empty string, got: {html}");
        assert!(html.contains("src=\"https://example.com/photo.jpg\""));
    }

    #[test]
    fn html_figure_missing_alt_renders_empty() {
        let doc = doc_with(vec![Block::Figure {
            src: "photo.jpg".into(),
            caption: Some("A photo".into()),
            alt: None,
            width: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("alt=\"\""), "Missing alt should render as empty string");
    }

    #[test]
    fn html_image_src_xss_escaped() {
        let doc = doc_with(vec![Block::HeroImage {
            src: "javascript:alert(1)".into(),
            alt: Some("test".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        // Should still render (browser won't execute in img src), but verify no unescaped injection
        assert!(!html.contains("<script>"), "No script injection");
    }

    #[test]
    fn html_utf8_content_renders_correctly() {
        let doc = doc_with(vec![Block::Markdown {
            content: "# Café ☕\n\nWillkommen in unserem Geschäft! 日本語テスト 🎉\n".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("Café"), "UTF-8 content should render");
        assert!(html.contains("☕"), "Emoji should render");
        assert!(html.contains("日本語"), "CJK should render");
    }

    #[test]
    fn html_style_accent_semicolon_escaped() {
        // Verify CSS accent values with semicolons are HTML-escaped
        let doc = doc_with(vec![Block::Style {
            properties: vec![StyleProperty {
                key: "accent".into(),
                value: "#ff0000; color: white; --x:".into(),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        // escape_html converts & < > " but not ; — documenting current behavior
        // The injected CSS is: --accent: #ff0000; color: white; --x:;
        // This IS a CSS injection but scope is limited to the :root selector
        assert!(html.contains("--accent:"), "Accent override should be present");
    }

    #[test]
    fn html_uploaded_image_relative_path() {
        // User-uploaded images use /uploads/ paths — verify they render as-is
        let doc = doc_with(vec![Block::Figure {
            src: "/uploads/abc123-photo.jpg".into(),
            caption: Some("My uploaded photo".into()),
            alt: Some("Photo".into()),
            width: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("src=\"/uploads/abc123-photo.jpg\""),
            "Uploaded image path should be preserved verbatim"
        );
    }

    #[test]
    fn html_gallery_images_have_alt() {
        use crate::types::GalleryItem;
        let doc = doc_with(vec![Block::Gallery {
            items: vec![
                GalleryItem { src: "a.jpg".into(), alt: Some("First".into()), caption: None, category: None },
                GalleryItem { src: "b.jpg".into(), alt: None, caption: None, category: None },
            ],
            columns: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("alt=\"First\""), "Gallery item with alt should render it");
        assert!(html.contains("alt=\"\""), "Gallery item without alt should render empty");
    }

    #[test]
    fn css_accent_sanitizes_semicolon_injection() {
        use super::sanitize_css_value;
        // Semicolons and braces are stripped — can't break out of CSS value
        let result = sanitize_css_value("red; } body { background: red");
        assert!(!result.contains(';'), "Semicolons should be stripped");
        assert!(!result.contains('{'), "Open braces should be stripped");
        assert!(!result.contains('}'), "Close braces should be stripped");
        assert!(!result.is_empty(), "Non-dangerous text should remain");
    }

    #[test]
    fn css_accent_sanitizes_url_injection() {
        let doc = doc_with(vec![Block::Style {
            properties: vec![StyleProperty {
                key: "accent".into(),
                value: "url(https://evil.com/track)".into(),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        // url() function calls are blocked entirely (returns empty → no accent set)
        assert!(!html.contains("--accent:"), "url() injection should prevent accent from being set");
    }

    #[test]
    fn css_accent_allows_valid_colors() {
        let doc = doc_with(vec![Block::Style {
            properties: vec![StyleProperty {
                key: "accent".into(),
                value: "#0052CC".into(),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("--accent: #0052CC"), "Valid hex color should pass through");
        assert!(html.contains("--accent-text:"), "accent-text should be computed for valid accent");
    }

    #[test]
    fn accent_text_color_wcag_compliance() {
        // Light accents → dark text (luminance > 0.25)
        assert_eq!(accent_text_color("#4CAF50"), "#1a1a2e"); // Green (L≈0.33)
        assert_eq!(accent_text_color("#f59e0b"), "#1a1a2e"); // Amber (L≈0.57)
        assert_eq!(accent_text_color("#ffffff"), "#1a1a2e"); // White (L=1.0)
        assert_eq!(accent_text_color("#eab308"), "#1a1a2e"); // Yellow (L≈0.55)
        // Dark accents → white text (luminance ≤ 0.25)
        assert_eq!(accent_text_color("#3b82f6"), "#fff");    // Blue (L≈0.24)
        assert_eq!(accent_text_color("#283593"), "#fff");    // Indigo (L≈0.04)
        assert_eq!(accent_text_color("#000000"), "#fff");    // Black (L=0)
        assert_eq!(accent_text_color("#ef4444"), "#fff");    // Red (L≈0.23)
        assert_eq!(accent_text_color("#ec4899"), "#fff");    // Pink (L≈0.25)
        assert_eq!(accent_text_color("#8b5cf6"), "#fff");    // Purple (L≈0.13)
        // Short hex
        assert_eq!(accent_text_color("#fff"), "#1a1a2e");
        assert_eq!(accent_text_color("#000"), "#fff");
        // Invalid → default white
        assert_eq!(accent_text_color("not-a-color"), "#fff");
    }

    // -- BeforeAfter -----------------------------------------------

    #[test]
    fn html_before_after_basic() {
        let doc = doc_with(vec![Block::BeforeAfter {
            before_items: vec![crate::types::BeforeAfterItem {
                label: "Manual".into(),
                detail: "Hand-written".into(),
            }],
            after_items: vec![crate::types::BeforeAfterItem {
                label: "Automated".into(),
                detail: "One-click".into(),
            }],
            transition: Some("SurfDoc".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-before-after"));
        assert!(html.contains("surfdoc-ba-dot-red"));
        assert!(html.contains("surfdoc-ba-dot-green"));
        assert!(html.contains("surfdoc-ba-transition"));
        assert!(html.contains("SurfDoc"));
        assert!(html.contains("Manual"));
        assert!(html.contains("Automated"));
    }

    #[test]
    fn html_before_after_no_transition() {
        let doc = doc_with(vec![Block::BeforeAfter {
            before_items: vec![crate::types::BeforeAfterItem {
                label: "Old".into(),
                detail: "Legacy".into(),
            }],
            after_items: vec![crate::types::BeforeAfterItem {
                label: "New".into(),
                detail: "Modern".into(),
            }],
            transition: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-before-after"));
        assert!(!html.contains("surfdoc-ba-transition"));
    }

    // -- Pipeline --------------------------------------------------

    #[test]
    fn html_pipeline_basic() {
        let doc = doc_with(vec![Block::Pipeline {
            steps: vec![
                crate::types::PipelineStep { label: "Phone".into(), description: Some("Input".into()) },
                crate::types::PipelineStep { label: "AI".into(), description: None },
                crate::types::PipelineStep { label: "App".into(), description: Some("Output".into()) },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-pipeline"));
        assert!(html.contains("surfdoc-pipeline-step"));
        assert!(html.contains("surfdoc-pipeline-arrow"));
        assert!(html.contains("Phone"));
        assert!(html.contains("Input"));
    }

    #[test]
    fn html_pipeline_no_arrows_single_step() {
        let doc = doc_with(vec![Block::Pipeline {
            steps: vec![
                crate::types::PipelineStep { label: "Solo".into(), description: None },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-pipeline"));
        assert!(!html.contains("surfdoc-pipeline-arrow"));
    }

    // -- Section ---------------------------------------------------

    #[test]
    fn html_section_muted() {
        let doc = doc_with(vec![Block::Section {
            bg: Some("muted".into()),
            headline: Some("Features".into()),
            subtitle: Some("What we offer".into()),
            content: String::new(),
            children: vec![Block::Markdown {
                content: "Some content".into(),
                span: span(),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-section section-muted"));
        assert!(html.contains("surfdoc-section-header"));
        assert!(html.contains("Features"));
        assert!(html.contains("What we offer"));
    }

    #[test]
    fn html_section_no_bg() {
        let doc = doc_with(vec![Block::Section {
            bg: None,
            headline: Some("Title".into()),
            subtitle: None,
            content: String::new(),
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-section\""));
        assert!(!html.contains("section-muted"));
    }

    // -- ProductCard -----------------------------------------------

    #[test]
    fn html_product_card_full() {
        let doc = doc_with(vec![Block::ProductCard {
            title: "Surf Browser".into(),
            subtitle: Some("Native viewer".into()),
            badge: Some("Available".into()),
            badge_color: Some("green".into()),
            body: "Render .surf files.".into(),
            features: vec!["Fast".into(), "Dark mode".into()],
            cta_label: Some("Download".into()),
            cta_href: Some("/download".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-product-card"));
        assert!(html.contains("surfdoc-badge-green"));
        assert!(html.contains("Available"));
        assert!(html.contains("Surf Browser"));
        assert!(html.contains("Native viewer"));
        assert!(html.contains("surfdoc-product-features"));
        assert!(html.contains("Fast"));
        assert!(html.contains("surfdoc-product-cta"));
        assert!(html.contains("/download"));
    }

    #[test]
    fn html_product_card_minimal() {
        let doc = doc_with(vec![Block::ProductCard {
            title: "Basic".into(),
            subtitle: None,
            badge: None,
            badge_color: None,
            body: String::new(),
            features: vec![],
            cta_label: None,
            cta_href: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-product-card"));
        assert!(html.contains("Basic"));
        assert!(!html.contains("surfdoc-badge"));
        assert!(!html.contains("surfdoc-product-features"));
        assert!(!html.contains("surfdoc-product-cta"));
    }

    // -- Fragment rendering tests -----------------------------------------

    #[test]
    fn fragment_no_page_chrome() {
        let doc = doc_with(vec![
            Block::Markdown {
                content: "# Hello".into(),
                span: span(),
            },
            Block::Callout {
                callout_type: CalloutType::Info,
                title: None,
                content: "A note.".into(),
                span: span(),
            },
        ]);
        let html = to_html_fragment(&doc.blocks);

        // Must contain rendered content
        assert!(html.contains("<h1 id=\"hello\">Hello</h1>"), "Should render heading");
        assert!(html.contains("surfdoc-callout"), "Should render callout");

        // Must NOT contain page chrome
        assert!(!html.contains("<!DOCTYPE"), "No DOCTYPE in fragment");
        assert!(!html.contains("<html"), "No <html> wrapper in fragment");
        assert!(!html.contains("<head"), "No <head> in fragment");
        assert!(!html.contains("<body"), "No <body> in fragment");
        assert!(!html.contains("<article"), "No <article> wrapper in fragment");
    }

    #[test]
    fn fragment_skips_style_scanning() {
        let doc = doc_with(vec![
            Block::Site {
                domain: Some("example.com".into()),
                properties: vec![StyleProperty {
                    key: "accent".into(),
                    value: "#ff0000".into(),
                }],
                span: span(),
            },
            Block::Markdown {
                content: "Hello".into(),
                span: span(),
            },
        ]);
        let fragment = to_html_fragment(&doc.blocks);

        // Fragment should NOT contain style override injection
        assert!(!fragment.contains("<style>"), "Fragment must not emit <style> tags");
        assert!(!fragment.contains("--accent"), "Fragment must not emit CSS variable overrides");
    }

    #[test]
    fn fragment_no_auto_section_wrap() {
        let doc = doc_with(vec![
            Block::Markdown {
                content: "# Section One".into(),
                span: span(),
            },
            Block::Markdown {
                content: "Content.".into(),
                span: span(),
            },
            Block::Markdown {
                content: "# Section Two".into(),
                span: span(),
            },
        ]);
        let fragment = to_html_fragment(&doc.blocks);

        // to_html() wraps h1/h2 in <section> tags; fragment must not
        assert!(
            !fragment.contains("surfdoc-section"),
            "Fragment must not add section wrapping"
        );
        assert!(
            !fragment.contains("<section"),
            "Fragment must not contain <section> elements from auto-wrapping"
        );
    }

    #[test]
    fn fragment_empty_input() {
        let html = to_html_fragment(&[]);
        assert_eq!(html, "", "Empty block slice should produce empty string");
    }

    #[test]
    fn fragment_single_block() {
        let blocks = vec![Block::Code {
            lang: Some("rust".into()),
            file: None,
            highlight: vec![],
            content: "let x = 1;".into(),
            span: span(),
        }];
        let html = to_html_fragment(&blocks);

        assert!(html.contains("surfdoc-code"), "Should render code block");
        assert!(html.contains("language-rust"), "Should have language class");
        assert!(html.contains("let x = 1;"), "Should contain code content");
        // Single block means no newline joiner to worry about, but verify no chrome
        assert!(!html.contains("<html"), "No page chrome");
    }

    #[test]
    fn fragment_escapes_html() {
        let blocks = vec![Block::Callout {
            callout_type: CalloutType::Info,
            title: None,
            content: "<script>alert('xss')</script>".into(),
            span: span(),
        }];
        let html = to_html_fragment(&blocks);

        assert!(!html.contains("<script>"), "Script tags must be escaped in fragments");
        assert!(html.contains("&lt;script&gt;"), "Should contain escaped script tag");
    }

    #[test]
    fn fragment_renders_nav_inline() {
        let blocks = vec![
            Block::Nav {
                items: vec![crate::types::NavItem {
                    label: "Home".into(),
                    href: "/".into(),
                    icon: None,
                    image: None, external: false,
                }],
                logo: Some("MySite".into()),
                groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
                span: span(),
            },
            Block::Markdown {
                content: "Content after nav.".into(),
                span: span(),
            },
        ];
        let html = to_html_fragment(&blocks);

        // Nav should render as a regular block in document order
        // (to_html() extracts nav blocks and renders them first)
        assert!(html.contains("surfdoc-nav"), "Should render nav block");
        assert!(html.contains("Content after nav."), "Should render content block");
    }

    #[test]
    fn site_nav_helper_basic() {
        let nav = build_site_nav_html(
            "My Site",
            &[
                ("/".into(), "Home".into()),
                ("/about".into(), "About".into()),
            ],
            "/about",
            "surfdoc-site-theme:/",
            false,
        );

        assert!(nav.contains("surfdoc-site-nav"), "Should have site nav class");
        assert!(nav.contains("My Site"), "Should contain site name");
        assert!(nav.contains("class=\"active\""), "Active route should be marked");
        assert!(
            nav.contains("class=\"active\" aria-current=\"page\""),
            "Active route should carry aria-current"
        );
        assert!(nav.contains("href=\"/about\""), "Should have about link");
        assert!(nav.contains("Home"), "Should have home link text");
    }

    #[test]
    fn site_nav_helper_active_home() {
        let nav = build_site_nav_html(
            "Site",
            &[
                ("/".into(), "Home".into()),
                ("/docs".into(), "Docs".into()),
            ],
            "/",
            "surfdoc-site-theme:/",
            false,
        );

        // The "/" route link should be active
        assert!(nav.contains("href=\"/\""), "Should have home link");
        // Verify active class is applied (check the pattern in the output)
        // The active link is: <a href="/" class="active" aria-current="page">Home</a>
        assert!(
            nav.contains("href=\"/\" class=\"active\""),
            "Home link should be active when current_route is /"
        );
    }

    // ── c1-drawer: the shell-style drawer ported into the site nav ────────

    #[test]
    fn site_nav_drawer_matches_shell_contract() {
        let items: Vec<(String, String)> = vec![
            ("/".into(), "Home".into()),
            ("/about".into(), "About".into()),
            ("/contact".into(), "Contact".into()),
        ];
        let nav = build_site_nav_html("Acme Studio", &items, "/", "surfdoc-site-theme:/", false);

        assert!(
            nav.contains("<div class=\"site-nav-drawer-head\">"),
            "drawer head present"
        );
        assert!(
            nav.contains("<span class=\"site-nav-drawer-brand\"><span class=\"site-nav-logo\" aria-hidden=\"true\">A</span>Acme Studio</span>"),
            "drawer brand is a span carrying the site-name monogram"
        );
        assert!(
            nav.contains("<label for=\"site-nav-toggle\" class=\"site-nav-close\""),
            "close affordance is a label for the toggle checkbox"
        );
        assert!(
            nav.contains("<span class=\"site-nav-group-label\">Pages</span>"),
            "labeled section group above the links"
        );
        assert_eq!(
            nav.matches("class=\"site-nav-link-icon\"").count(),
            items.len(),
            "exactly one line icon per nav item"
        );
        // Topbar anchor carries the same monogram.
        assert!(nav.contains(
            "class=\"site-name\"><span class=\"site-nav-logo\" aria-hidden=\"true\">A</span>Acme Studio</a>"
        ));
    }

    #[test]
    fn site_nav_needles_stable_for_surf_container() {
        // surf frontend/src/sites/container.rs post-processes this markup with
        // two byte-pinned needles (LOGO_NEEDLE / NAV_CLOSE_NEEDLE, mirrored
        // here verbatim). Any drift breaks the container's logo rewrite and
        // nav-CTA injection — keep these literals in lockstep.
        const LOGO_NEEDLE: &str = "<a href=\"/\" class=\"site-name\">";
        const NAV_CLOSE_NEEDLE: &str =
            "  </div>\n  <button type=\"button\" class=\"site-nav-theme-toggle\"";

        let nav = build_site_nav_html(
            "Acme",
            &[("/".into(), "Home".into()), ("/about".into(), "About".into())],
            "/",
            "surfdoc-site-theme:/",
            false,
        );
        assert!(nav.contains(LOGO_NEEDLE), "LOGO_NEEDLE bytes present");
        assert!(nav.contains(NAV_CLOSE_NEEDLE), "NAV_CLOSE_NEEDLE bytes present");
        // The topbar anchor is the FIRST (and only) site-name match, so the
        // container's first-occurrence rewrite hits the topbar, never the
        // drawer brand (a <span>).
        assert_eq!(nav.matches("class=\"site-name\"").count(), 1);
        assert!(
            nav.find(LOGO_NEEDLE).unwrap() < nav.find("site-nav-drawer-head").unwrap(),
            "topbar anchor precedes the drawer head"
        );
    }

    #[test]
    fn site_nav_icon_mapper_defaults() {
        // Route keyword hits.
        assert!(site_nav_link_icon("/", "Home").contains("m3 9 9-7 9 7"), "/ → house");
        assert!(
            site_nav_link_icon("/shop", "Shop").contains("M16 10a4 4 0 0 1-8 0"),
            "/shop → shopping bag"
        );
        // Unknown route AND title → generic document icon.
        assert!(
            site_nav_link_icon("/xyzzy", "Xyzzy").contains("M14 2H6a2 2 0 0 0-2 2"),
            "unknown → file-text default"
        );
        // Title fallback when the route says nothing.
        assert!(
            site_nav_link_icon("/p1", "Contact Us").contains("22,6 12,13 2,6"),
            "title keyword → mail"
        );
    }

    #[test]
    fn site_nav_spa_contract_kept() {
        let nav = build_site_nav_html(
            "Shop",
            &[
                ("/".into(), "Home".into()),
                ("/browse".into(), "Browse".into()),
                ("/faq".into(), "FAQ".into()),
            ],
            "/",
            "surfdoc-site-theme:/",
            true,
        );
        // The SPA router queries `.site-nav-links a[data-route]` — every page
        // link carries data-route (the head/close/scrim labels are not <a>s).
        for route in ["/", "/browse", "/faq"] {
            assert!(
                nav.contains(&format!(" data-route=\"{route}\">")),
                "page link for {route} carries data-route"
            );
        }
        assert_eq!(nav.matches(" data-route=\"").count(), 3, "one data-route per page link");
        assert_eq!(
            nav.matches("id=\"site-nav-toggle\"").count(),
            1,
            "exactly one toggle checkbox for getElementById"
        );
    }

    #[test]
    fn site_nav_css_theme_arms() {
        assert!(
            SITE_NAV_CSS.contains(
                "[data-theme=\"dark\"] .site-nav-links { box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); }"
            ),
            "explicit dark arm deepens the panel shadow"
        );
        assert!(
            SITE_NAV_CSS.contains(
                ":root:not([data-theme]) .site-nav-links { box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5); }"
            ),
            "device-dark (no data-theme) arm deepens the panel shadow"
        );
        assert!(
            SITE_NAV_CSS.contains("@media (prefers-reduced-motion: reduce)"),
            "reduced-motion arm present"
        );
        assert!(
            SITE_NAV_CSS.contains(".site-nav-links, .site-nav-scrim { transition: none; }"),
            "reduced-motion disables drawer + scrim transitions"
        );
    }

    #[test]
    fn single_file_site_has_all_routes_in_one_doc() {
        let pages = vec![
            PageEntry {
                route: "/".into(),
                layout: None,
                title: Some("Home".into()),
                sidebar: false,
                children: vec![Block::Markdown {
                    content: "Welcome home.".into(),
                    span: span(),
                }],
            },
            PageEntry {
                route: "/browse".into(),
                layout: None,
                title: Some("Browse".into()),
                sidebar: false,
                children: vec![Block::Markdown {
                    content: "Browse items.".into(),
                    span: span(),
                }],
            },
        ];
        let site = SiteConfig {
            name: Some("Shop".into()),
            ..Default::default()
        };
        let nav_items: Vec<(String, String)> = pages
            .iter()
            .map(|p| (p.route.clone(), p.display_title()))
            .collect();
        let html = render_site_single_file(&site, &pages, &nav_items, &PageConfig::default());

        // One document, both routes present as sections.
        assert_eq!(html.matches("<!DOCTYPE html>").count(), 1);
        assert_eq!(html.matches("<main").count(), 1, "single <main> landmark");
        assert!(html.contains("<section class=\"surfdoc-page\" data-route=\"/\">"));
        assert!(html.contains("<section class=\"surfdoc-page\" data-route=\"/browse\" hidden>"));
        assert!(html.contains("Welcome home.") && html.contains("Browse items."));

        // SPA nav: hash hrefs + data-route, home initially active.
        assert!(html.contains(
            "<a href=\"#/browse\" data-route=\"/browse\"><span class=\"site-nav-link-icon\""
        ));
        assert!(html.contains("</span>Browse</a>"));
        assert!(html.contains("data-route=\"/\""));

        // Router is inlined with the resolved route list.
        assert!(html.contains("var routes=[\"/\",\"/browse\"];"));
        assert!(!html.contains("__ROUTES__"), "router placeholder substituted");
    }

    #[test]
    fn site_page_nav_refactor_stable() {
        // This test captures that render_site_page() produces identical output
        // after the nav logic is extracted into build_site_nav_html().
        // The expected output is validated against known content patterns.
        let page = PageEntry {
            route: "/about".into(),
            layout: None,
            title: Some("About".into()),
            sidebar: false,
            children: vec![Block::Markdown {
                content: "About content.".into(),
                span: span(),
            }],
        };
        let site = SiteConfig {
            name: Some("Test Site".into()),
            ..Default::default()
        };
        let nav_items = vec![
            ("/".into(), "Home".into()),
            ("/about".into(), "About".into()),
        ];
        let config = PageConfig::default();

        let html = render_site_page(&page, &site, &nav_items, &config);

        // Verify nav structure is present and correct
        assert!(html.contains("surfdoc-site-nav"), "Should have site nav");
        assert!(html.contains("Test Site"), "Should have site name in nav");
        assert!(
            html.contains("href=\"/about\" class=\"active\""),
            "About should be active"
        );
        // Verify it is still a full page
        assert!(html.contains("<!DOCTYPE html>"), "Should be a full page");
        assert!(
            html.contains("<main id=\"surfdoc-main\" class=\"surfdoc\">"),
            "Should have the main landmark wrapper (BR-SITE-A11Y; was <article>)"
        );
    }

    // ── BR-SITE-A11Y / BR-SITE-THEME: structural accessibility layer ──────
    // "By design" means the suite enforces it: these pins make a regression
    // in the emitted site structure fail the build, not an audit.

    /// A representative site page: hero + markdown section + image + gallery,
    /// nav with several routes — the sweep-matrix page shape.
    fn a11y_fixture_page() -> (PageEntry, SiteConfig, Vec<(String, String)>) {
        use crate::types::GalleryItem;
        let page = PageEntry {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            children: vec![
                Block::Hero {
                    headline: Some("Welcome to Acme".into()),
                    subtitle: Some("We make things.".into()),
                    badge: None,
                    align: "center".into(),
                    image: Some("hero.jpg".into()),
                    image_alt: None,
                    layout: None,
                    transparent: false,
                    buttons: vec![],
                    content: String::new(),
                    span: span(),
                },
                Block::Markdown {
                    content: "## What we do\n\nThings, mostly.".into(),
                    span: span(),
                },
                Block::Gallery {
                    items: vec![GalleryItem {
                        src: "a.jpg".into(),
                        alt: Some("Workshop".into()),
                        caption: None,
                        category: None,
                    }],
                    columns: None,
                    span: span(),
                },
            ],
        };
        let site = SiteConfig {
            name: Some("Acme".into()),
            theme: Some("light".into()),
            accent: Some("#22c55e".into()),
            base_path: Some("/s/acme".into()),
            ..Default::default()
        };
        let nav = vec![
            ("/s/acme".into(), "Home".into()),
            ("/s/acme/about".into(), "About".into()),
        ];
        (page, site, nav)
    }

    #[test]
    fn a11y_exactly_one_main_landmark() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        assert_eq!(
            html.matches("<main").count(),
            1,
            "exactly one <main> landmark per page"
        );
        assert_eq!(html.matches("<nav class=\"surfdoc-site-nav\"").count(), 1);
        assert_eq!(html.matches("<footer class=\"surfdoc-site-footer\"").count(), 1);
    }

    #[test]
    fn a11y_skip_link_is_first_focusable() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        let body_at = html.find("<body>").expect("body present");
        let skip_at = html
            .find("<a class=\"surfdoc-skip-link\" href=\"#surfdoc-main\">")
            .expect("skip link present");
        let nav_at = html.find("<nav class=\"surfdoc-site-nav\"").expect("nav present");
        assert!(
            body_at < skip_at && skip_at < nav_at,
            "the skip link is the FIRST focusable in <body>, before the nav"
        );
        assert!(
            html.contains("id=\"surfdoc-main\""),
            "skip link target exists"
        );
        assert!(
            SITE_NAV_CSS.contains(".surfdoc-skip-link:focus"),
            "skip link becomes visible on focus"
        );
    }

    #[test]
    fn a11y_hamburger_is_keyboard_operable() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        // The checkbox is the keyboard surface: focusable + labeled, never
        // aria-hidden. The visual hamburger label is decorative.
        assert!(
            html.contains(
                "<input type=\"checkbox\" class=\"site-nav-toggle\" id=\"site-nav-toggle\" aria-label=\"Toggle menu\">"
            ),
            "menu checkbox is labeled and NOT aria-hidden"
        );
        assert!(
            html.contains("class=\"site-nav-hamburger\" aria-hidden=\"true\""),
            "the visual hamburger label is decorative"
        );
        // CSS keeps the checkbox focusable at all sizes (visually hidden, not
        // display:none) and shows its focus ring on the hamburger.
        assert!(
            SITE_NAV_CSS.contains(
                ".site-nav-toggle { position: absolute; width: 1px; height: 1px;"
            ),
            "checkbox is visually-hidden-but-focusable"
        );
        assert!(
            SITE_NAV_CSS.contains(".site-nav-toggle:focus-visible ~ .site-nav-hamburger"),
            "checkbox focus ring renders on the hamburger"
        );
    }

    #[test]
    fn a11y_focus_visible_rules_cover_site_chrome() {
        assert!(
            SITE_NAV_CSS.contains(
                ".surfdoc-site-nav a:focus-visible, .site-nav-theme-toggle:focus-visible, .surfdoc-skip-link:focus-visible, .site-nav-close:focus-visible"
            ),
            "nav links, theme toggle, skip link, and drawer close all carry :focus-visible rings"
        );
    }

    #[test]
    fn a11y_hero_page_has_exactly_one_h1() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        assert_eq!(
            html.matches("<h1").count(),
            1,
            "hero headline is the page's single h1; chrome adds none"
        );
    }

    #[test]
    fn a11y_every_img_carries_alt() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        let img_count = html.matches("<img").count();
        assert!(img_count >= 2, "fixture renders hero + gallery images");
        for (i, seg) in html.split("<img").skip(1).enumerate() {
            let tag = &seg[..seg.find('>').unwrap_or(seg.len())];
            assert!(
                tag.contains("alt="),
                "img #{} missing alt attribute: <img{}>",
                i + 1,
                tag
            );
        }
    }

    #[test]
    fn a11y_gallery_items_keyboard_activatable() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        assert!(
            html.contains("class=\"surfdoc-gallery-item\" data-index=\"0\" style=\"cursor:pointer\" tabindex=\"0\" role=\"button\""),
            "gallery items are focusable buttons"
        );
        assert!(
            html.contains("f.onkeydown=function(e){if(e.key==='Enter'||e.key===' ')"),
            "Enter/Space activate the lightbox"
        );
    }

    #[test]
    fn a11y_accent_ink_meets_aa_for_platform_palettes() {
        // The container's six palette accents (surf frontend
        // sites/container.rs PALETTES) — the toggle makes every one face
        // BOTH themes, so both inks must read as normal text at AA.
        for accent in ["#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#8b5cf6", "#2563eb"] {
            let light_ink = accent_ink_color(accent, false);
            let dark_ink = accent_ink_color(accent, true);
            // Strictest surface per theme; AA there implies AA on the page
            // + sheet surfaces too.
            assert!(
                contrast_ratio(&light_ink, "#eef1f7") >= 4.5,
                "{accent} light ink {light_ink} must be AA on light surfaces"
            );
            assert!(
                contrast_ratio(&dark_ink, "#161616") >= 4.5,
                "{accent} dark ink {dark_ink} must be AA on dark surfaces"
            );
        }
        // Unparseable input falls back to the theme-safe inks.
        assert_eq!(accent_ink_color("nope", false), "#1a1a2e");
        assert_eq!(accent_ink_color("nope", true), "#ffffff");
    }

    #[test]
    fn a11y_site_page_emits_theme_scoped_ink_overrides() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        let light_ink = accent_ink_color("#22c55e", false);
        let dark_ink = accent_ink_color("#22c55e", true);
        assert!(
            html.contains(&format!("--accent-ink: {light_ink};")),
            "light ink emitted in :root"
        );
        assert!(
            html.contains(&format!("[data-theme=\"dark\"] {{ --accent-ink: {dark_ink}; }}")),
            "dark ink emitted for the explicit dark arm"
        );
        assert!(
            html.contains(&format!(
                "@media (prefers-color-scheme: dark) {{ :root:not([data-theme]) {{ --accent-ink: {dark_ink}; }} }}"
            )),
            "dark ink emitted for the no-JS device-dark arm"
        );
    }

    #[test]
    fn a11y_muted_tokens_are_aa_pinned() {
        // Drift-pins the stylesheet's muted/faint text tokens to AA values
        // (the 2026-06-11 retune: light muted 4.19→4.77 on --surface-alt,
        // dark faint 2.82→4.52).
        assert!(SURFDOC_CSS.contains("--text-muted: #636a7e;"), "light muted retuned");
        assert!(SURFDOC_CSS.contains("--text-faint: #7f7f7f;"), "dark faint retuned");
        assert!(contrast_ratio("#636a7e", "#eef1f7") >= 4.5);
        assert!(contrast_ratio("#7f7f7f", "#161616") >= 4.5);
        // Accent-as-text reads through the ink (with the plain accent as the
        // doc-page fallback).
        assert!(
            SURFDOC_CSS.contains(".surfdoc a { color: var(--accent-ink, var(--accent)); text-decoration: none; }"),
            "prose links read through --accent-ink"
        );
    }

    #[test]
    fn theme_toggle_button_on_every_site_page() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        assert!(
            html.contains("<button type=\"button\" class=\"site-nav-theme-toggle\" aria-label=\"Switch between light and dark theme\""),
            "labeled toggle button present"
        );
        assert!(
            html.contains("localStorage.setItem('surfdoc-site-theme:/s/acme',t)"),
            "toggle persists to the per-site key"
        );
        assert!(html.contains("site-theme-icon-moon") && html.contains("site-theme-icon-sun"));
        // Touch target ≥24px (38px circular shell control) pinned in CSS.
        assert!(SITE_NAV_CSS.contains(".site-nav-theme-toggle { order: 2; display: inline-flex; align-items: center; justify-content: center; width: 38px; height: 38px;"));
    }

    #[test]
    fn theme_resolver_runs_before_first_paint() {
        let (page, site, nav) = a11y_fixture_page();
        let html = render_site_page(&page, &site, &nav, &PageConfig::default());
        let head_close = html.find("</head>").expect("head present");
        let resolver = html
            .find("localStorage.getItem('surfdoc-site-theme:/s/acme')")
            .expect("resolver present");
        assert!(resolver < head_close, "resolver script lives in <head> (pre-paint)");
        // Device preference outranks the palette hint; the hint is only the
        // no-preference fallback (the ratified palette-semantics decision).
        assert!(html.contains("(prefers-color-scheme: dark)"));
        assert!(html.contains("(prefers-color-scheme: light)"));
        assert!(html.contains(":'light');"), "fixture palette hint = light fallback");
    }

    #[test]
    fn sanitize_js_key_strips_breakouts() {
        assert_eq!(
            sanitize_js_key("surfdoc-site-theme:/s/acme"),
            "surfdoc-site-theme:/s/acme"
        );
        assert_eq!(sanitize_js_key("a'b\\c\"d<e>f"), "abcdef");
    }

    #[test]
    fn fragment_with_context() {
        let doc = doc_with(vec![Block::Markdown {
            content: "Hello {= name =}!".into(),
            span: span(),
        }]);
        let ctx = crate::TemplateContext::new();
        // Note: TemplateContext::new() starts empty.
        // Variables not found resolve to empty string or are left as-is
        // depending on the implementation. This test verifies the call chain works.
        let html = doc.to_html_fragment_with_context(&ctx);
        assert!(html.contains("Hello"), "Should render content");
    }

    // ── Interactive / application block HTML render tests ──────────

    #[test]
    fn html_app_shell() {
        let doc = doc_with(vec![Block::AppShell {
            layout: "sidebar-main-panel".into(),
            children: vec![Block::Markdown { content: "Inner".into(), span: span() }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-app-shell surfdoc-layout-sidebar-main-panel"));
        assert!(html.contains("Inner"));
    }

    #[test]
    fn html_sidebar() {
        let doc = doc_with(vec![Block::Sidebar {
            position: "left".into(),
            collapsible: true,
            width: Some(250),
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-sidebar surfdoc-sidebar-left"));
        assert!(html.contains("data-collapsible=\"true\""));
        assert!(html.contains("style=\"width:250px\""));
    }

    #[test]
    fn html_panel() {
        let doc = doc_with(vec![Block::Panel {
            position: "bottom".into(),
            resizable: true,
            height: Some(200),
            desktop_only: true,
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-panel surfdoc-panel-bottom"));
        assert!(html.contains("data-resizable=\"true\""));
        assert!(html.contains("data-desktop-only=\"true\""));
        assert!(html.contains("style=\"height:200px\""));
    }

    #[test]
    fn html_tab_bar() {
        let doc = doc_with(vec![Block::TabBar {
            active: Some("preview".into()),
            items: vec![
                TabBarItem { id: "preview".into(), label: "Preview".into(), icon: None, unread: false },
                TabBarItem { id: "edit".into(), label: "Edit".into(), icon: Some("pencil".into()), unread: true },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-tab-bar"));
        assert!(html.contains("role=\"tablist\""));
        assert!(html.contains("data-tab=\"preview\""));
        assert!(html.contains("class=\"active\""));
        assert!(html.contains("Preview"));
        assert!(html.contains("data-icon=\"pencil\""));
    }

    #[test]
    fn html_tab_content() {
        let doc = doc_with(vec![Block::TabContent {
            tab: "preview".into(),
            children: vec![Block::Markdown { content: "Tab body".into(), span: span() }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-tab-content"));
        assert!(html.contains("data-tab=\"preview\""));
        assert!(html.contains("Tab body"));
    }

    #[test]
    fn html_toolbar() {
        let doc = doc_with(vec![Block::Toolbar {
            items: vec![
                ToolbarItem::Button { label: Some("Deploy".into()), action: Some("deploy".into()), icon: None, style: Some("primary".into()), disabled: false },
                ToolbarItem::Separator,
                ToolbarItem::Spacer,
                ToolbarItem::Badge { value: "Live".into(), color: Some("green".into()) },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-toolbar"));
        assert!(html.contains("Deploy"));
        assert!(html.contains("surfdoc-toolbar-separator"));
        assert!(html.contains("surfdoc-toolbar-spacer"));
        assert!(html.contains("surfdoc-badge-green"));
        assert!(html.contains("Live"));
    }

    #[test]
    fn html_drawer() {
        let doc = doc_with(vec![Block::Drawer {
            name: "mako".into(),
            position: "right".into(),
            width: Some(320),
            trigger: Some("icon".into()),
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-drawer surfdoc-drawer-right"));
        assert!(html.contains("data-name=\"mako\""));
        assert!(html.contains("style=\"width:320px\""));
    }

    #[test]
    fn html_modal() {
        let doc = doc_with(vec![Block::Modal {
            name: "deploy".into(),
            title: Some("Deploy App".into()),
            width: None,
            placement: "centered".into(),
            dismissible: true,
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-modal"));
        assert!(html.contains("data-name=\"deploy\""));
        assert!(html.contains("data-placement=\"centered\""));
        assert!(html.contains("<header class=\"surfdoc-modal-header\">"));
        assert!(html.contains("<strong class=\"surfdoc-modal-title\">Deploy App</strong>"));
        assert!(html.contains("surfdoc-modal-close"));
        assert!(html.contains("aria-label=\"Close\""));
        assert!(!html.contains("data-dismissible"));
    }

    #[test]
    fn html_modal_non_dismissible_still_has_close() {
        let doc = doc_with(vec![Block::Modal {
            name: "confirm".into(),
            title: None,
            width: Some(480),
            placement: "top".into(),
            dismissible: false,
            children: vec![],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("data-dismissible=\"false\""));
        assert!(html.contains("style=\"width:480px\""));
        assert!(html.contains("data-placement=\"top\""));
        // Dismiss canon: close control renders even when dismissible=false.
        assert!(html.contains("surfdoc-modal-close"));
        // Title falls back to name when absent.
        assert!(html.contains("<strong class=\"surfdoc-modal-title\">confirm</strong>"));
    }

    #[test]
    fn html_row_unread_dot_right_side_no_left_border() {
        let doc = doc_with(vec![Block::Row {
            icon: "doc".into(),
            title: "Release notes".into(),
            description: "1.6.2".into(),
            href: None,
            state: RowState::Default,
            unread: true,
            span: span(),
        }]);
        let html = to_html(&doc);
        // Dot sits after the body, before the arrow — right side.
        let dot_idx = html.find("surfdoc-unread-dot").expect("unread dot");
        let body_idx = html.find("surfdoc-row-body").expect("row body");
        let arrow_idx = html.find("surfdoc-row-arrow").expect("row arrow");
        assert!(body_idx < dot_idx && dot_idx < arrow_idx);
        // Renderer invariant: accent-left-border BANNED — no left border in markup.
        assert!(!html.contains("border-left"));

        let doc_read = doc_with(vec![Block::Row {
            icon: "doc".into(),
            title: "Release notes".into(),
            description: "1.6.2".into(),
            href: None,
            state: RowState::Default,
            unread: false,
            span: span(),
        }]);
        assert!(!to_html(&doc_read).contains("surfdoc-unread-dot"));
    }

    #[test]
    fn html_tab_bar_unread_dot() {
        let doc = doc_with(vec![Block::TabBar {
            active: Some("inbox".into()),
            items: vec![
                TabBarItem { id: "inbox".into(), label: "Inbox".into(), icon: None, unread: true },
                TabBarItem { id: "sent".into(), label: "Sent".into(), icon: None, unread: false },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("Inbox<span class=\"surfdoc-unread-dot\" aria-label=\"Unread\"></span></button>"));
        assert_eq!(html.matches("surfdoc-unread-dot").count(), 1);
        assert!(!html.contains("border-left"));
    }

    #[test]
    fn html_segmented_control() {
        let doc = doc_with(vec![Block::SegmentedControl {
            active: Some("posts".into()),
            size: "compact".into(),
            action: Some("filter_posts".into()),
            segments: vec![
                SegmentItem { id: "all".into(), label: "All".into() },
                SegmentItem { id: "posts".into(), label: "Posts".into() },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-segmented-control"));
        assert!(html.contains("role=\"radiogroup\""));
        assert!(html.contains("data-size=\"compact\""));
        assert!(html.contains("data-action=\"filter_posts\""));
        // Exactly one active pill.
        assert_eq!(html.matches("is-active").count(), 1);
        assert!(html.contains("aria-checked=\"true\">Posts</button>"));
        assert!(html.contains("aria-checked=\"false\">All</button>"));
        // Not a tab bar: no tablist markup, no tab-switching script.
        assert!(!html.contains("role=\"tablist\""));
        assert!(!html.contains("surfdoc-tab-bar"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn html_dropdown_select() {
        let doc = doc_with(vec![Block::DropdownSelect {
            label: Some("Sort".into()),
            icon: Some("arrow".into()),
            selected: Some("Newest".into()),
            align: "right".into(),
            options: vec![
                DropdownOption {
                    label: "Newest".into(),
                    description: Some("Most recent first".into()),
                    icon: Some("clock".into()),
                    action: Some("sort_newest".into()),
                },
                DropdownOption {
                    label: "Oldest".into(),
                    description: None,
                    icon: None,
                    action: None,
                },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-dropdown-select"));
        assert!(html.contains("data-align=\"right\""));
        assert!(html.contains("surfdoc-dropdown-trigger"));
        assert!(html.contains("<span class=\"surfdoc-dropdown-label\">Sort</span>"));
        assert!(html.contains("<span class=\"surfdoc-dropdown-selected\">Newest</span>"));
        assert!(html.contains("surfdoc-dropdown-option is-selected"));
        assert!(html.contains("data-action=\"sort_newest\""));
        assert!(html.contains("<span class=\"surfdoc-dropdown-option-desc\">Most recent first</span>"));
        assert!(html.contains("<span class=\"surfdoc-dropdown-option-label\">Oldest</span>"));
    }

    #[test]
    fn html_command_palette() {
        let doc = doc_with(vec![Block::CommandPalette {
            trigger: Some("cmd+/".into()),
            items: vec![CommandItem {
                label: "Data Table".into(),
                description: Some("Define data".into()),
                action: Some("add_schema".into()),
                icon: Some("table".into()),
                group: Some("Data".into()),
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-command-palette"));
        assert!(html.contains("Data Table"));
        assert!(html.contains("Define data"));
        assert!(html.contains("data-group=\"Data\""));
    }

    #[test]
    fn html_code_editor() {
        let doc = doc_with(vec![Block::CodeEditor {
            lang: Some("surf".into()),
            source: Some("doc.source".into()),
            line_numbers: true,
            content: "::hero\n::".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-code-editor"));
        assert!(html.contains("data-lang=\"surf\""));
        assert!(html.contains("data-source=\"doc.source\""));
        assert!(html.contains("data-line-numbers=\"true\""));
    }

    #[test]
    fn html_block_editor() {
        let doc = doc_with(vec![Block::BlockEditor {
            source: Some("doc.blocks".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-block-editor"));
        assert!(html.contains("data-source=\"doc.blocks\""));
    }

    #[test]
    fn html_terminal() {
        let doc = doc_with(vec![Block::Terminal {
            shell: Some("default".into()),
            cwd: Some("workspace.path".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-terminal"));
        assert!(html.contains("data-shell=\"default\""));
        assert!(html.contains("data-cwd=\"workspace.path\""));
    }

    #[test]
    fn html_nav_tree() {
        let doc = doc_with(vec![Block::NavTree {
            source: Some("workspace.files".into()),
            on_select: Some("open_file".into()),
            on_rename: None,
            on_delete: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-nav-tree"));
        assert!(html.contains("data-source=\"workspace.files\""));
    }

    #[test]
    fn html_badge() {
        let doc = doc_with(vec![Block::Badge {
            value: "Live".into(),
            color: Some("green".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-badge surfdoc-badge-green"));
        assert!(html.contains("Live"));
    }

    #[test]
    fn html_suggestion_chips() {
        let doc = doc_with(vec![Block::SuggestionChips {
            source: Some("mako.suggestions".into()),
            max: Some(3),
            dismissible: true,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-suggestion-chips"));
        assert!(html.contains("data-source=\"mako.suggestions\""));
        assert!(html.contains("data-dismissible=\"true\""));
        assert!(html.contains("data-max=\"3\""));
    }

    #[test]
    fn html_chat_thread() {
        let doc = doc_with(vec![Block::ChatThread {
            source: Some("mako.conversation".into()),
            on_action: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-chat-thread"));
        assert!(html.contains("data-source=\"mako.conversation\""));
    }

    #[test]
    fn html_chat_input_simple() {
        let doc = doc_with(vec![Block::ChatInputSimple {
            placeholder: Some("Message Mako...".into()),
            action: Some("mako.send".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-chat-input"));
        assert!(html.contains("placeholder=\"Message Mako...\""));
        assert!(html.contains("data-action=\"mako.send\""));
        assert!(html.contains("Send"));
    }

    #[test]
    fn html_progress() {
        let doc = doc_with(vec![Block::Progress {
            source: Some("deploy.progress".into()),
            steps: vec![
                ProgressStep { label: "Parsing app".into(), status: "done".into() },
                ProgressStep { label: "Creating database".into(), status: "active".into() },
                ProgressStep { label: "Registering routes".into(), status: "pending".into() },
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-progress"));
        assert!(html.contains("surfdoc-step-done"));
        assert!(html.contains("surfdoc-step-active"));
        assert!(html.contains("surfdoc-step-pending"));
        assert!(html.contains("Parsing app"));
    }

    #[test]
    fn html_log_stream() {
        let doc = doc_with(vec![Block::LogStream {
            source: Some("app.logs".into()),
            tail: Some(100),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-log-stream"));
        assert!(html.contains("data-source=\"app.logs\""));
        assert!(html.contains("data-tail=\"100\""));
    }

    #[test]
    fn html_problem_list() {
        let doc = doc_with(vec![Block::ProblemList {
            source: Some("doc.diagnostics".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-problem-list"));
        assert!(html.contains("data-source=\"doc.diagnostics\""));
    }

    // ---- HTML sanitization (XSS prevention) ----

    #[test]
    fn script_tags_stripped_from_markdown() {
        let doc = doc_with(vec![Block::Markdown {
            content: "Hello <script>alert('xss')</script> world".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>"), "script tags must be stripped");
        assert!(!html.contains("</script>"), "script close tags must be stripped");
        assert!(html.contains("Hello"), "safe text before script preserved");
        assert!(html.contains("world"), "safe text after script preserved");
    }

    #[test]
    fn safe_html_preserved_in_markdown() {
        let doc = doc_with(vec![Block::Markdown {
            content: "**bold** and *italic* and [link](https://example.com)".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<strong>bold</strong>"), "bold preserved");
        assert!(html.contains("<em>italic</em>"), "italic preserved");
        assert!(html.contains("<a"), "link tag preserved");
        assert!(html.contains("https://example.com"), "link href preserved");
    }

    #[test]
    fn event_handlers_stripped_from_markdown() {
        let doc = doc_with(vec![Block::Markdown {
            content: "<img src=\"x\" onerror=\"alert(1)\">".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("onerror"), "event handler attributes must be stripped");
    }

    #[test]
    fn iframe_stripped_from_markdown() {
        let doc = doc_with(vec![Block::Markdown {
            content: "<iframe src=\"https://evil.com\"></iframe>".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<iframe"), "iframe tags must be stripped");
    }

    #[test]
    fn javascript_uri_stripped_from_markdown() {
        let doc = doc_with(vec![Block::Markdown {
            content: "<a href=\"javascript:alert(1)\">click</a>".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("javascript:"), "javascript: URIs must be stripped");
    }

    // -- Summary inline markdown emphasis tests ---------------------------
    // Regression: Block::Summary previously used escape_html() on its body,
    // leaking literal asterisks for **bold** / *italic*. It now uses
    // render_inline_markdown() to match the Callout sibling.

    #[test]
    fn summary_renders_bold_emphasis() {
        let doc = doc_with(vec![Block::Summary {
            content: "This is **bold** text.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<strong>bold</strong>"),
            "expected <strong>bold</strong> in: {html}"
        );
        assert!(!html.contains("**bold**"), "literal asterisks leaked: {html}");
    }

    #[test]
    fn summary_renders_italic_emphasis() {
        let doc = doc_with(vec![Block::Summary {
            content: "This is *italic* text.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<em>italic</em>"),
            "expected <em>italic</em> in: {html}"
        );
    }

    #[test]
    fn summary_renders_nested_emphasis() {
        let doc = doc_with(vec![Block::Summary {
            content: "Mixed ***x*** here.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<strong>"), "expected <strong> tag in: {html}");
        assert!(html.contains("<em>"), "expected <em> tag in: {html}");
        assert!(html.contains(">x<"), "expected literal x between tags in: {html}");
    }

    #[test]
    fn summary_with_no_emphasis_unchanged() {
        let doc = doc_with(vec![Block::Summary {
            content: "Plain text.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        // No spurious emphasis tags injected.
        assert!(!html.contains("<strong>"), "unexpected <strong> in plain summary: {html}");
        assert!(!html.contains("<em>"), "unexpected <em> in plain summary: {html}");
        assert!(html.contains("Plain text."), "plain text missing in: {html}");
    }

    #[test]
    fn summary_escapes_html_special_chars() {
        let doc = doc_with(vec![Block::Summary {
            content: "<script>alert(1)</script>".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            !html.contains("<script>"),
            "raw <script> tag must not appear: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "expected escaped script tag in: {html}"
        );
    }

    #[test]
    fn summary_with_link_renders_anchor() {
        let doc = doc_with(vec![Block::Summary {
            content: "See [CloudSurf](https://app.surf) for details.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<a href=\"https://app.surf\""),
            "expected anchor tag in: {html}"
        );
        assert!(html.contains(">CloudSurf</a>"), "expected anchor text in: {html}");
    }

    #[test]
    fn callout_inline_emphasis_still_works() {
        // Regression guard: the sibling Callout arm must continue to render
        // inline markdown (the source of the fix pattern).
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Info,
            title: None,
            content: "**bold**".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<strong>bold</strong>"),
            "callout regression: expected <strong>bold</strong> in: {html}"
        );
    }

    // -- Quote inline markdown emphasis tests -----------------------------
    // Regression: Block::Quote previously used escape_html() on its body,
    // leaking literal asterisks for **bold** / *italic*.

    #[test]
    fn quote_renders_bold_emphasis() {
        let doc = doc_with(vec![Block::Quote {
            content: "This is **bold** wisdom.".into(),
            attribution: None,
            cite: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<strong>bold</strong>"),
            "expected <strong>bold</strong> in: {html}"
        );
        assert!(!html.contains("**bold**"), "literal asterisks leaked: {html}");
    }

    #[test]
    fn quote_renders_italic_emphasis() {
        let doc = doc_with(vec![Block::Quote {
            content: "This is *italic* wisdom.".into(),
            attribution: None,
            cite: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<em>italic</em>"),
            "expected <em>italic</em> in: {html}"
        );
    }

    #[test]
    fn quote_escapes_html_special_chars() {
        let doc = doc_with(vec![Block::Quote {
            content: "<script>alert(1)</script>".into(),
            attribution: None,
            cite: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            !html.contains("<script>"),
            "raw <script> tag must not appear: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "expected escaped script tag in: {html}"
        );
    }

    // -- Testimonial inline markdown emphasis tests -----------------------

    #[test]
    fn testimonial_renders_bold_emphasis() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "Truly **amazing** product.".into(),
            author: None,
            role: None,
            company: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<strong>amazing</strong>"),
            "expected <strong>amazing</strong> in: {html}"
        );
        assert!(!html.contains("**amazing**"), "literal asterisks leaked: {html}");
    }

    #[test]
    fn testimonial_renders_italic_emphasis() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "Truly *amazing* product.".into(),
            author: None,
            role: None,
            company: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<em>amazing</em>"),
            "expected <em>amazing</em> in: {html}"
        );
    }

    #[test]
    fn testimonial_escapes_html_special_chars() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "<img src=x onerror=alert(1)>".into(),
            author: None,
            role: None,
            company: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            !html.contains("<img src=x onerror"),
            "raw <img> with onerror must not appear: {html}"
        );
        assert!(
            html.contains("&lt;img"),
            "expected escaped <img in: {html}"
        );
    }

    // -- Tasks item.text inline markdown emphasis tests -------------------

    #[test]
    fn tasks_item_text_renders_bold_emphasis() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![TaskItem {
                done: false,
                text: "Ship the **bold** fix".into(),
                assignee: None,
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<strong>bold</strong>"),
            "expected <strong>bold</strong> in tasks item: {html}"
        );
        assert!(!html.contains("**bold**"), "literal asterisks leaked: {html}");
    }

    #[test]
    fn tasks_item_text_renders_italic_emphasis() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![TaskItem {
                done: true,
                text: "Reviewed *carefully*".into(),
                assignee: None,
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("<em>carefully</em>"),
            "expected <em>carefully</em> in tasks item: {html}"
        );
    }

    #[test]
    fn tasks_item_text_escapes_html_special_chars() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![TaskItem {
                done: false,
                text: "<script>alert(1)</script>".into(),
                assignee: None,
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            !html.contains("<script>"),
            "raw <script> tag must not appear: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "expected escaped script tag in: {html}"
        );
    }

    // -- CalloutType::Context tests ---------------------------------------
    // Regression: ::callout [type=context] previously fell through to Info;
    // CalloutType::Context now wires through every renderer.

    #[test]
    fn callout_context_type_renders_with_context_class() {
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Context,
            title: None,
            content: "Background info.".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(
            html.contains("surfdoc-callout-context"),
            "expected surfdoc-callout-context class in: {html}"
        );
        assert!(
            !html.contains("surfdoc-callout-info"),
            "Context callout must not fall back to Info class: {html}"
        );
    }
}

#[cfg(test)]
mod proptest_summary {
    use super::*;
    use crate::types::*;
    use proptest::prelude::*;

    fn span() -> Span {
        Span {
            start_line: 1,
            end_line: 1,
            start_offset: 0,
            end_offset: 0,
        }
    }

    fn doc_with(blocks: Vec<Block>) -> SurfDoc {
        SurfDoc {
            front_matter: None,
            blocks,
            source: String::new(),
        }
    }

    proptest! {
        // For any printable ASCII string free of markdown-significant
        // characters, wrapping it in `**...**` inside a Summary block must
        // yield <strong>{s}</strong> in the rendered HTML.
        #[test]
        fn summary_bold_roundtrip(s in "[a-zA-Z0-9][a-zA-Z0-9 ,.!?]{0,78}[a-zA-Z0-9]") {
            // Anchored on alphanumerics at both ends to avoid pulldown-cmark
            // edge cases (e.g. `** **` becomes <hr>, leading/trailing spaces
            // break emphasis).
            let content = format!("**{}**", s);
            let doc = doc_with(vec![Block::Summary {
                content,
                span: span(),
            }]);
            let html = to_html(&doc);
            let expected = format!("<strong>{}</strong>", s);
            prop_assert!(
                html.contains(&expected),
                "expected {:?} in HTML: {}", expected, html
            );
        }

        // Rendering must never panic for any printable Unicode string.
        #[test]
        fn summary_never_panics(s in r"\PC*") {
            let doc = doc_with(vec![Block::Summary {
                content: s,
                span: span(),
            }]);
            let _ = to_html(&doc);
        }
    }
}

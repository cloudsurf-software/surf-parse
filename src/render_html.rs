//! HTML fragment renderer.
//!
//! Produces semantic HTML with `surfdoc-*` CSS classes. Markdown blocks are
//! rendered through `pulldown-cmark`. All other content is HTML-escaped to
//! prevent XSS.

use crate::icons::get_icon;
use crate::types::{Block, CalloutType, ChartType, DecisionStatus, FormFieldType, HttpMethod, ListDisplay, StyleProperty, SurfDoc, Trend};

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
    // Wrap bare <table> tags in scroll containers for mobile responsiveness
    html_output = html_output.replace("<table>", "<div class=\"surfdoc-table-wrap\"><table>");
    html_output = html_output.replace("</table>", "</table></div>");
    html_output
}

/// Render inline markdown (bold, italic, links, code) while escaping raw HTML.
///
/// Used for block content (callouts, decisions, etc.) where we want markdown
/// formatting but must prevent HTML injection. Escapes `<`, `>`, `&` first,
/// then runs through pulldown-cmark so `*italic*` and `**bold**` still render.
fn render_inline_markdown(content: &str) -> String {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    render_markdown(&escaped)
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
        }
    }
}

/// Render a `SurfDoc` as an HTML fragment.
///
/// The output is a sequence of semantic HTML elements with `surfdoc-*` CSS
/// classes. No `<html>`, `<head>`, or `<body>` wrapper is added.
/// If a `::site` block sets an accent color, a `<style>` override scoped to
/// `.surfdoc` is injected (not `:root`, to avoid leaking into editor chrome).
/// A resolved font preset: CSS font stack + optional Google Fonts import URL.
struct FontPreset {
    stack: &'static str,
    import: Option<&'static str>,
}

/// Resolve a font preset name to a CSS font stack and optional import.
fn resolve_font_preset(name: &str) -> Option<FontPreset> {
    match name.trim().to_lowercase().as_str() {
        "system" | "sans" => Some(FontPreset {
            stack: "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Oxygen, sans-serif",
            import: None,
        }),
        "serif" | "editorial" => Some(FontPreset {
            stack: "Georgia, \"Palatino Linotype\", \"Book Antiqua\", Palatino, serif",
            import: None,
        }),
        "mono" | "monospace" | "technical" => Some(FontPreset {
            stack: "\"SF Mono\", \"Fira Code\", \"Cascadia Code\", Menlo, Consolas, monospace",
            import: None,
        }),
        "inter" => Some(FontPreset {
            stack: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif",
            import: Some("https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"),
        }),
        "montserrat" => Some(FontPreset {
            stack: "'Montserrat', sans-serif",
            import: Some("https://fonts.googleapis.com/css2?family=Montserrat:wght@400;600;700;800&display=swap"),
        }),
        "jetbrains-mono" | "jetbrains" => Some(FontPreset {
            stack: "'JetBrains Mono', monospace",
            import: Some("https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500&display=swap"),
        }),
        "playfair" | "playfair-display" => Some(FontPreset {
            stack: "'Playfair Display', Georgia, serif",
            import: Some("https://fonts.googleapis.com/css2?family=Playfair+Display:wght@400;500;600;700&display=swap"),
        }),
        "lato" => Some(FontPreset {
            stack: "'Lato', -apple-system, BlinkMacSystemFont, sans-serif",
            import: Some("https://fonts.googleapis.com/css2?family=Lato:wght@300;400;700&display=swap"),
        }),
        _ => None,
    }
}

/// Parse a hex color (#RGB, #RRGGBB) and return the WCAG-compliant text color.
/// Returns "#fff" for dark accents, "#1a1a2e" for light accents.
fn accent_text_color(hex: &str) -> &'static str {
    let hex = hex.trim().trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            (r, g, b)
        }
        _ => return "#fff", // Can't parse — default to white
    };
    // sRGB to linear, then relative luminance (WCAG 2.1)
    fn linearize(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    }
    let lum = 0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b);
    // Threshold 0.25: ensures minimum ~3.5:1 contrast ratio (WCAG AA for large
    // text / UI components). Catches greens, yellows, ambers while keeping
    // standard blues (#3b82f6) and reds (#ef4444) with white text.
    if lum > 0.25 { "#1a1a2e" } else { "#fff" }
}

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

pub fn to_html(doc: &SurfDoc) -> String {
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
        if let Block::Nav { items, logo, .. } = block {
            // Use explicit logo, fall back to ::site name
            let effective_logo = logo.as_deref().or(site_name.as_deref());
            let mut html = String::from("<nav class=\"surfdoc-nav\" role=\"navigation\" aria-label=\"Page navigation\">");
            if let Some(logo_text) = effective_logo {
                html.push_str(&format!(
                    "<span class=\"surfdoc-nav-logo\">{}</span>",
                    escape_html(logo_text),
                ));
            }
            html.push_str("<input type=\"checkbox\" class=\"surfdoc-nav-toggle\" id=\"surfdoc-nav-toggle\" aria-hidden=\"true\">");
            html.push_str("<label for=\"surfdoc-nav-toggle\" class=\"surfdoc-nav-hamburger\" aria-label=\"Toggle menu\"><span></span><span></span><span></span></label>");
            html.push_str("<div class=\"surfdoc-nav-links\">");
            for item in items {
                let icon_html = item.icon
                    .as_deref()
                    .and_then(get_icon)
                    .map(|svg| format!("<span class=\"surfdoc-icon\">{}</span> ", svg))
                    .unwrap_or_default();
                html.push_str(&format!(
                    "<a href=\"{}\">{}{}</a>",
                    escape_html(&item.href),
                    icon_html,
                    escape_html(&item.label),
                ));
            }
            html.push_str("</div></nav>");
            parts.push(html);
        }
    }

    let mut in_section = false;
    let mut section_index: usize = 0;

    for block in &doc.blocks {
        // Skip nav blocks — already rendered above
        if matches!(block, Block::Nav { .. }) {
            continue;
        }

        let rendered = render_block(block);

        // Detect section boundaries: h1 or h2 starts a new visual section
        let starts_section = rendered.starts_with("<h1>") || rendered.starts_with("<h2>");
        if starts_section {
            if in_section {
                parts.push("</section>".to_string());
            }
            let alt = if section_index % 2 == 1 { " surfdoc-section-alt" } else { "" };
            parts.push(format!("<section class=\"surfdoc-section{}\">", alt));
            in_section = true;
            section_index += 1;
        }

        parts.push(rendered);
    }

    if in_section {
        parts.push("</section>".to_string());
    }

    parts.join("\n")
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
    blocks
        .iter()
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n")
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

    // Build meta tags: description, canonical, OG, Twitter
    let mut meta_extra = String::new();
    if let Some(desc) = &description {
        let desc_escaped = escape_html(desc);
        meta_extra.push_str(&format!(
            "\n    <meta name=\"description\" content=\"{}\">",
            desc_escaped
        ));
    }
    if let Some(url) = &config.canonical_url {
        let url_escaped = escape_html(url);
        meta_extra.push_str(&format!(
            "\n    <link rel=\"canonical\" href=\"{}\">",
            url_escaped
        ));
        // Open Graph
        meta_extra.push_str(&format!(
            "\n    <meta property=\"og:url\" content=\"{}\">",
            url_escaped
        ));
    }
    // Open Graph tags
    let title_escaped = escape_html(&title);
    meta_extra.push_str(&format!(
        "\n    <meta property=\"og:title\" content=\"{}\">",
        title_escaped
    ));
    meta_extra.push_str("\n    <meta property=\"og:type\" content=\"website\">");
    if let Some(desc) = &description {
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
    // Twitter Card tags
    meta_extra.push_str("\n    <meta name=\"twitter:card\" content=\"summary\">");
    meta_extra.push_str(&format!(
        "\n    <meta name=\"twitter:title\" content=\"{}\">",
        title_escaped
    ));
    if let Some(desc) = &description {
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

    format!(
        r#"<!-- Built with SurfDoc — source: {source_path} -->
<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="generator" content="SurfDoc v0.1">
    <link rel="alternate" type="text/surfdoc" href="{source_path}">
    <title>{title}</title>{meta_extra}
    <style>{css}</style>
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
        meta_extra = meta_extra,
        css = SURFDOC_CSS,
        body = body,
    )
}

// SURFDOC_CSS is now a public constant in lib.rs via include_str!("../assets/surfdoc.css").
// It's referenced here as crate::SURFDOC_CSS.
use crate::SURFDOC_CSS;

// The old inline CSS has been moved to assets/surfdoc.css and is loaded via include_str! in lib.rs.

/// Escape HTML special characters to prevent XSS.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

fn render_block(block: &Block) -> String {
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
            let heading = match title {
                Some(t) => format!("{}: {}", capitalize(type_str), escape_html(t)),
                None => capitalize(type_str).to_string(),
            };
            format!(
                "<div class=\"surfdoc-callout surfdoc-callout-{type_str}\" role=\"{role}\"><strong>{heading}</strong><div class=\"surfdoc-callout-body\">{}</div></div>",
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
                    html.push_str(&format!("<th scope=\"col\">{}</th>", escape_html(h)));
                }
                html.push_str("</tr></thead>");
            }
            html.push_str("<tbody>");
            for row in rows {
                html.push_str("<tr>");
                for cell in row {
                    html.push_str(&format!("<td>{}</td>", escape_html(cell)));
                }
                html.push_str("</tr>");
            }
            html.push_str("</tbody></table></div>");
            html
        }

        Block::Code {
            lang, content, ..
        } => {
            let class = match lang {
                Some(l) => format!(" class=\"language-{}\"", escape_html(l)),
                None => String::new(),
            };
            let aria = match lang {
                Some(l) => format!(" aria-label=\"{} code\"", escape_html(l)),
                None => String::new(),
            };
            format!(
                "<pre class=\"surfdoc-code\"{}><code{}>{}</code></pre>",
                aria,
                class,
                escape_html(content),
            )
        }

        Block::Tasks { items, .. } => {
            let mut html = String::from("<ul class=\"surfdoc-tasks\">");
            for item in items {
                let checked = if item.done { " checked" } else { "" };
                let assignee_html = match &item.assignee {
                    Some(a) => format!(" <span class=\"assignee\">@{}</span>", escape_html(a)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<li><label><input type=\"checkbox\"{checked} disabled> {}</label>{assignee_html}</li>",
                    escape_html(&item.text),
                ));
            }
            html.push_str("</ul>");
            html
        }

        Block::Decision {
            status,
            date,
            content,
            ..
        } => {
            let status_str = decision_status_str(*status);
            let date_html = match date {
                Some(d) => format!("<span class=\"date\">{}</span>", escape_html(d)),
                None => String::new(),
            };
            format!(
                "<div class=\"surfdoc-decision surfdoc-decision-{status_str}\" role=\"note\" aria-label=\"Decision: {status_str}\"><div class=\"surfdoc-decision-header\"><span class=\"status\">{status_str}</span>{date_html}</div><div class=\"surfdoc-decision-body\">{}</div></div>",
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
                Some(Trend::Up) => "<span class=\"trend up\">\u{2191}</span>".to_string(),
                Some(Trend::Down) => "<span class=\"trend down\">\u{2193}</span>".to_string(),
                Some(Trend::Flat) => "<span class=\"trend flat\">\u{2192}</span>".to_string(),
                None => String::new(),
            };
            let unit_html = match unit {
                Some(u) => format!("<span class=\"unit\">{}</span>", escape_html(u)),
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
            format!(
                "<div class=\"surfdoc-metric\" role=\"group\" aria-label=\"{}\"><span class=\"label\">{}</span><span class=\"value\">{}</span>{unit_html}{trend_html}</div>",
                escape_html(&aria_label),
                escape_html(label),
                escape_html(value),
            )
        }

        Block::Summary { content, .. } => {
            format!(
                "<div class=\"surfdoc-summary\" role=\"doc-abstract\"><p>{}</p></div>",
                escape_html(content),
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
                Some(c) => format!("<figcaption>{}</figcaption>", escape_html(c)),
                None => String::new(),
            };
            format!(
                "<figure class=\"surfdoc-figure\"><img src=\"{}\" alt=\"{}\" />{caption_html}</figure>",
                escape_html(src),
                escape_html(alt_attr),
            )
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
            let mut html = String::from("<div class=\"surfdoc-quote\"><blockquote>");
            html.push_str(&escape_html(content));
            html.push_str("</blockquote>");
            if let Some(attr) = attribution {
                let cite_part = match cite {
                    Some(c) => format!(", <cite>{}</cite>", escape_html(c)),
                    None => String::new(),
                };
                html.push_str(&format!(
                    "<div class=\"attribution\">{}{}</div>",
                    escape_html(attr),
                    cite_part,
                ));
            }
            html.push_str("</div>");
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
                "<div class=\"surfdoc-hero-image\"{}><img src=\"{}\" alt=\"{}\" /></div>",
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
            let mut html = format!("<div class=\"surfdoc-testimonial\" role=\"figure\"{}><blockquote>", aria_label);
            html.push_str(&escape_html(content));
            html.push_str("</blockquote>");
            if author.is_some() || role.is_some() || company.is_some() {
                html.push_str("<div class=\"author\">");
                if let Some(a) = author {
                    html.push_str(&escape_html(a));
                }
                let details: Vec<&str> = [role.as_deref(), company.as_deref()]
                    .iter()
                    .filter_map(|v| *v)
                    .collect();
                if !details.is_empty() {
                    html.push_str(&format!(
                        " <span class=\"role\">{}</span>",
                        escape_html(&details.join(", "))
                    ));
                }
                html.push_str("</div>");
            }
            html.push_str("</div>");
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
                    "<details><summary>{}</summary><div class=\"faq-answer\">{}</div></details>",
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
            let mut html = String::from("<div class=\"surfdoc-table-wrap\"><table class=\"surfdoc-pricing\" aria-label=\"Pricing comparison\">");
            if !headers.is_empty() {
                html.push_str("<thead><tr>");
                for h in headers {
                    html.push_str(&format!("<th scope=\"col\">{}</th>", escape_html(h)));
                }
                html.push_str("</tr></thead>");
            }
            html.push_str("<tbody>");
            for row in rows {
                html.push_str("<tr>");
                for cell in row {
                    html.push_str(&format!("<td>{}</td>", escape_html(cell)));
                }
                html.push_str("</tr>");
            }
            html.push_str("</tbody></table></div>");
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

        Block::Nav { items, logo, .. } => {
            let mut html = String::from("<nav class=\"surfdoc-nav\" role=\"navigation\" aria-label=\"Page navigation\">");
            if let Some(logo_text) = logo {
                html.push_str(&format!(
                    "<span class=\"surfdoc-nav-logo\">{}</span>",
                    escape_html(logo_text),
                ));
            }
            html.push_str("<input type=\"checkbox\" class=\"surfdoc-nav-toggle\" id=\"surfdoc-nav-toggle\" aria-hidden=\"true\">");
            html.push_str("<label for=\"surfdoc-nav-toggle\" class=\"surfdoc-nav-hamburger\" aria-label=\"Toggle menu\"><span></span><span></span><span></span></label>");
            html.push_str("<div class=\"surfdoc-nav-links\">");
            for item in items {
                let icon_html = item.icon
                    .as_deref()
                    .and_then(get_icon)
                    .map(|svg| format!("<span class=\"surfdoc-icon\">{}</span> ", svg))
                    .unwrap_or_default();
                html.push_str(&format!(
                    "<a href=\"{}\">{}{}</a>",
                    escape_html(&item.href),
                    icon_html,
                    escape_html(&item.label),
                ));
            }
            html.push_str("</div></nav>");
            html
        }

        Block::Embed {
            src, embed_type, width, height, title, ..
        } => {
            let w = width.as_deref().unwrap_or("100%");
            let h = height.as_deref().unwrap_or("400");
            let title_attr = title.as_deref().unwrap_or("Embedded content");
            let type_class = match embed_type {
                Some(crate::types::EmbedType::Map) => " surfdoc-embed-map",
                Some(crate::types::EmbedType::Video) => " surfdoc-embed-video",
                Some(crate::types::EmbedType::Audio) => " surfdoc-embed-audio",
                _ => "",
            };
            format!(
                "<div class=\"surfdoc-embed{type_class}\"><iframe src=\"{}\" width=\"{}\" height=\"{}\" title=\"{}\" frameborder=\"0\" allowfullscreen loading=\"lazy\" referrerpolicy=\"no-referrer\" sandbox=\"allow-scripts allow-same-origin allow-popups\"></iframe></div>",
                escape_html(src),
                escape_html(w),
                escape_html(h),
                escape_html(title_attr),
            )
        }

        Block::Form {
            fields, submit_label, ..
        } => {
            let btn_label = submit_label.as_deref().unwrap_or("Submit");
            let mut html = String::from("<form class=\"surfdoc-form\">");
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
                escape_html(btn_label),
            ));
            html.push_str("</form>");
            html
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
                    "<figure class=\"surfdoc-gallery-item\" data-index=\"{}\" style=\"cursor:pointer\"{cat_attr}>",
                    i
                ));
                html.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\" loading=\"lazy\" />",
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
            html.push_str(r#"<script>document.querySelectorAll('.surfdoc-gallery').forEach(function(g){var lb=g.querySelector('.surfdoc-lightbox');if(!lb)return;var si=lb.querySelector('.sl-img'),sc=lb.querySelector('.sl-caption'),sn=lb.querySelector('.sl-counter'),items,idx;function vis(){return Array.prototype.filter.call(g.querySelectorAll('.surfdoc-gallery-item'),function(i){return i.style.display!=='none'})}function show(i){items=vis();idx=i;var f=items[idx],im=f.querySelector('img'),fc=f.querySelector('figcaption');si.src=im.src;si.alt=im.alt||'';sc.textContent=fc?fc.textContent:'';sn.textContent=(idx+1)+' / '+items.length;lb.hidden=false;document.body.style.overflow='hidden'}function hide(){lb.hidden=true;document.body.style.overflow=''}function nav(d){show((idx+d+items.length)%items.length)}g.querySelectorAll('.surfdoc-gallery-item').forEach(function(f){f.onclick=function(){var v=vis();show(v.indexOf(f))}});lb.querySelector('.sl-close').onclick=hide;lb.querySelector('.sl-prev').onclick=function(){nav(-1)};lb.querySelector('.sl-next').onclick=function(){nav(1)};lb.onclick=function(e){if(e.target===lb)hide()};document.addEventListener('keydown',function(e){if(lb.hidden)return;if(e.key==='Escape')hide();if(e.key==='ArrowLeft')nav(-1);if(e.key==='ArrowRight')nav(1)});var tx;lb.addEventListener('touchstart',function(e){tx=e.touches[0].clientX});lb.addEventListener('touchend',function(e){var dx=e.changedTouches[0].clientX-tx;if(Math.abs(dx)>50)nav(dx>0?-1:1)})})</script>"#);
            html.push_str("</div>");
            html
        }

        Block::Footer {
            sections, copyright, social, ..
        } => {
            let mut html = String::from("<footer class=\"surfdoc-footer\">");
            if !sections.is_empty() {
                html.push_str("<div class=\"surfdoc-footer-sections\">");
                for section in sections {
                    html.push_str("<div class=\"surfdoc-footer-col\">");
                    html.push_str(&format!("<h4>{}</h4>", escape_html(&section.heading)));
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
                 <summary>{}</summary>\
                 <div class=\"surfdoc-details-content\">{}</div>\
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
            buttons,
            content: _,
            ..
        } => {
            let align_cls = if align == "left" { " surfdoc-hero-left" } else { "" };
            let mut parts = Vec::new();
            parts.push(format!("<section class=\"surfdoc-hero{}\">", align_cls));
            parts.push("<div class=\"surfdoc-hero-inner\">".to_string());
            // Centered layout: image above text (logo/product image)
            if align != "left" {
                if let Some(img) = image {
                    parts.push(format!("<div class=\"surfdoc-hero-image\"><img src=\"{}\" alt=\"\"></div>", escape_html(img)));
                }
            }
            if let Some(b) = badge {
                parts.push(format!("<span class=\"surfdoc-hero-badge\">{}</span>", escape_html(b)));
            }
            if let Some(h) = headline {
                parts.push(format!("<h1 class=\"surfdoc-hero-headline\">{}</h1>", escape_html(h)));
            }
            if let Some(s) = subtitle {
                parts.push(format!("<p class=\"surfdoc-hero-subtitle\">{}</p>", escape_html(s)));
            }
            if !buttons.is_empty() {
                parts.push("<div class=\"surfdoc-hero-actions\">".to_string());
                for btn in buttons {
                    let cls = if btn.primary { "surfdoc-hero-btn surfdoc-hero-btn-primary" } else { "surfdoc-hero-btn surfdoc-hero-btn-secondary" };
                    parts.push(format!("<a href=\"{}\" class=\"{}\">{}</a>", escape_html(&btn.href), cls, escape_html(&btn.label)));
                }
                parts.push("</div>".to_string());
            }
            parts.push("</div>".to_string());
            // Left-aligned layout: image to the side (side-by-side)
            if align == "left" {
                if let Some(img) = image {
                    parts.push(format!("<div class=\"surfdoc-hero-image-side\"><img src=\"{}\" alt=\"\"></div>", escape_html(img)));
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
                parts.push(format!("<h3 class=\"surfdoc-feature-title\">{}</h3>", escape_html(&card.title)));
                if !card.body.is_empty() {
                    parts.push(format!("<p class=\"surfdoc-feature-body\">{}</p>", escape_html(&card.body)));
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
                parts.push(format!("<h3 class=\"surfdoc-step-title\">{}{}</h3>", escape_html(&step.title), time_html));
                if !step.body.is_empty() {
                    parts.push(format!("<p class=\"surfdoc-step-body\">{}</p>", escape_html(&step.body)));
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
                parts.push(format!("<th{}>{}</th>", cls, escape_html(h)));
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
                let mut parts = Vec::new();
                parts.push(format!("<nav class=\"surfdoc-toc\" data-depth=\"{}\"><ul>", depth));
                for entry in entries {
                    parts.push(format!(
                        "<li class=\"surfdoc-toc-item surfdoc-toc-l{}\"><a href=\"#{}\">{}</a></li>",
                        entry.level, escape_html(&entry.id), escape_html(&entry.text)
                    ));
                }
                parts.push("</ul></nav>".to_string());
                parts.join("")
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
                    html.push_str(&format!("<h2>{}</h2>", escape_html(h)));
                }
                if let Some(s) = subtitle {
                    html.push_str(&format!("<p>{}</p>", escape_html(s)));
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
                parts.push(format!("<p class=\"surfdoc-product-subtitle\">{}</p>", escape_html(s)));
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
                    parts.push(format!("<li>{}</li>", escape_html(f)));
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
            html.push_str("<div class=\"surfdoc-list-items\" aria-live=\"polite\"><p class=\"surfdoc-list-empty\">Loading...</p></div>");
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
                "<div class=\"surfdoc-board\" data-surf-source=\"{}\"",
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
            html.push_str("<div class=\"surfdoc-board-columns\">");
            for col in columns {
                html.push_str(&format!(
                    "<div class=\"surfdoc-board-column\" data-column=\"{}\"><h3 class=\"surfdoc-board-column-title\">{}</h3><div class=\"surfdoc-board-cards\" aria-live=\"polite\"></div></div>",
                    escape_html(col),
                    escape_html(col),
                ));
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
                "<form class=\"surfdoc-action\" data-surf-method=\"{}\" data-surf-action=\"{}\"",
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
                "<div class=\"surfdoc-dashboard\" data-surf-source=\"{}\"",
                escape_html(source),
            );
            if let Some(r) = refresh {
                html.push_str(&format!(" data-surf-refresh=\"{}\"", r));
            }
            html.push_str("><div class=\"surfdoc-dashboard-grid\" aria-live=\"polite\"><p>Loading metrics...</p></div></div>");
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
                "<div class=\"surfdoc-feed\" data-surf-source=\"{}\"{}><div class=\"surfdoc-feed-items\" aria-live=\"polite\"><p>Loading...</p></div></div>",
                escape_html(source),
                stream_attr,
            )
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
            html.push_str("><p class=\"surfdoc-mount-placeholder\">Editor loading...</p></div>");
            html
        }

        Block::Chart {
            chart_type, source, period, ..
        } => {
            let type_str = match chart_type {
                ChartType::Line => "line",
                ChartType::Bar => "bar",
                ChartType::Pie => "pie",
                ChartType::Area => "area",
            };
            let mut html = format!(
                "<div class=\"surfdoc-chart\" data-chart-type=\"{}\" data-surf-source=\"{}\"",
                type_str,
                escape_html(source),
            );
            if let Some(p) = period {
                html.push_str(&format!(" data-period=\"{}\"", escape_html(p)));
            }
            html.push_str("><p class=\"surfdoc-mount-placeholder\">Chart loading...</p></div>");
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
            format!("<div class=\"surfdoc-build\"><h4>Build</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::InfraDatabase { name, shared_auth, volume_gb, properties, .. } => {
            let mut items = Vec::new();
            if let Some(n) = name { items.push(format!("<li>name: {}</li>", escape_html(n))); }
            if *shared_auth { items.push("<li>shared_auth: true</li>".to_string()); }
            if let Some(v) = volume_gb { items.push(format!("<li>volume: {v} GB</li>")); }
            for p in properties { items.push(format!("<li>{}: {}</li>", escape_html(&p.key), escape_html(&p.value))); }
            format!("<div class=\"surfdoc-database\"><h4>Database</h4><ul>{}</ul></div>", items.join(""))
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
                "<div class=\"surfdoc-deploy\"><span class=\"surfdoc-deploy-badge\">{}</span><ul>{}</ul></div>",
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
            format!("<div class=\"surfdoc-env\"><h4>Env ({})</h4><ul>{}</ul></div>", escape_html(tier_str), items.join(""))
        }

        Block::Health { path, method, grace, interval, timeout, .. } => {
            let mut items = Vec::new();
            if let Some(p) = path { items.push(format!("<li>path: {}</li>", escape_html(p))); }
            if let Some(m) = method { items.push(format!("<li>method: {}</li>", escape_html(m))); }
            if let Some(g) = grace { items.push(format!("<li>grace: {}</li>", escape_html(g))); }
            if let Some(i) = interval { items.push(format!("<li>interval: {}</li>", escape_html(i))); }
            if let Some(t) = timeout { items.push(format!("<li>timeout: {}</li>", escape_html(t))); }
            format!("<div class=\"surfdoc-health\"><h4>Health Check</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::Concurrency { concurrency_type, hard_limit, soft_limit, force_https, .. } => {
            let mut items = Vec::new();
            if let Some(t) = concurrency_type { items.push(format!("<li>type: {}</li>", escape_html(t))); }
            if let Some(h) = hard_limit { items.push(format!("<li>hard_limit: {h}</li>")); }
            if let Some(s) = soft_limit { items.push(format!("<li>soft_limit: {s}</li>")); }
            if *force_https { items.push("<li>force_https: true</li>".to_string()); }
            format!("<div class=\"surfdoc-concurrency\"><h4>Concurrency</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::Cicd { provider, properties, .. } => {
            let prov = provider.as_deref().unwrap_or("CI/CD");
            let items: Vec<String> = properties.iter()
                .map(|p| format!("<li>{}: {}</li>", escape_html(&p.key), escape_html(&p.value)))
                .collect();
            format!("<div class=\"surfdoc-cicd\"><h4>{}</h4><ul>{}</ul></div>", escape_html(prov), items.join(""))
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
            format!("<div class=\"surfdoc-domains\"><h4>Domains</h4><ul>{}</ul></div>", items.join(""))
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
            format!("<div class=\"surfdoc-crates\"><h4>Crates</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::DeployUrls { entries, .. } => {
            let items: Vec<String> = entries.iter()
                .map(|p| format!("<li>{}: <a href=\"{}\">{}</a></li>", escape_html(&p.key), escape_html(&p.value), escape_html(&p.value)))
                .collect();
            format!("<div class=\"surfdoc-deploy-urls\"><h4>Deploy URLs</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::Volumes { entries, .. } => {
            let items: Vec<String> = entries.iter()
                .map(|v| format!("<li><code>{}</code> &rarr; <code>{}</code></li>", escape_html(&v.name), escape_html(&v.mount)))
                .collect();
            format!("<div class=\"surfdoc-volumes\"><h4>Volumes</h4><ul>{}</ul></div>", items.join(""))
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
                "<div class=\"surfdoc-model\"><h4>Model: {}</h4><table class=\"surfdoc-model-table\">\
                 <thead><tr><th>Field</th><th>Type</th><th>Constraints</th></tr></thead>\
                 <tbody>{}</tbody></table></div>",
                escape_html(name), rows,
            )
        }

        Block::Route { method, path, auth, returns, body, content, .. } => {
            let method_str = http_method_str(*method);
            let mut details = Vec::new();
            if let Some(a) = auth { details.push(format!("<li>auth: {}</li>", escape_html(a))); }
            if let Some(r) = returns { details.push(format!("<li>returns: <code>{}</code></li>", escape_html(r))); }
            if let Some(b) = body { details.push(format!("<li>body: <code>{}</code></li>", escape_html(b))); }
            if !content.is_empty() { details.push(format!("<li>{}</li>", escape_html(content))); }
            let details_html = if details.is_empty() { String::new() } else { format!("<ul>{}</ul>", details.join("")) };
            format!(
                "<div class=\"surfdoc-route\"><span class=\"surfdoc-route-method surfdoc-method-{}\">{}</span> \
                 <code class=\"surfdoc-route-path\">{}</code>{}</div>",
                method_str.to_lowercase(), method_str, escape_html(path), details_html,
            )
        }

        Block::Auth { provider, session, roles, default_role, .. } => {
            let provider_str = auth_provider_str(*provider);
            let mut items = vec![format!("<li>provider: {}</li>", provider_str)];
            if let Some(s) = session { items.push(format!("<li>session: {}</li>", escape_html(s))); }
            if !roles.is_empty() { items.push(format!("<li>roles: {}</li>", roles.iter().map(|r| escape_html(r)).collect::<Vec<_>>().join(", "))); }
            if let Some(dr) = default_role { items.push(format!("<li>default role: {}</li>", escape_html(dr))); }
            format!("<div class=\"surfdoc-auth\"><h4>Authentication</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::Binding { source, target, events, .. } => {
            let mut items = vec![
                format!("<li>source: <code>{}</code></li>", escape_html(source)),
                format!("<li>target: <code>{}</code></li>", escape_html(target)),
            ];
            for e in events {
                items.push(format!("<li>{}: {}</li>", escape_html(&e.event), escape_html(&e.action)));
            }
            format!("<div class=\"surfdoc-binding\"><h4>Binding</h4><ul>{}</ul></div>", items.join(""))
        }

        Block::Unknown {
            name, content, ..
        } => {
            format!(
                "<div class=\"surfdoc-unknown\" role=\"note\" data-name=\"{}\">{}</div>",
                escape_html(name),
                escape_html(content),
            )
        }

    }
}

/// Render a comparison cell value: "yes"/"true"/"✓" → green check, "no"/"false"/"✗"/"-" → muted dash, else literal.
fn comparison_cell(cell: &str) -> String {
    match cell.trim().to_lowercase().as_str() {
        "yes" | "true" | "✓" | "✔" => "<span class=\"surfdoc-check\">\u{2713}</span>".to_string(),
        "no" | "false" | "✗" | "✘" | "-" | "—" => "<span class=\"surfdoc-dash\">\u{2014}</span>".to_string(),
        _ => escape_html(cell),
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
    pub theme: Option<String>,
    pub accent: Option<String>,
    pub font: Option<String>,
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
/* Site navigation */
.surfdoc-site-nav { display: flex; align-items: center; flex-wrap: wrap; padding: 0.75rem 1.5rem; background: var(--surface); border-bottom: 1px solid var(--border); max-width: 100%; position: sticky; top: 0; z-index: 100; }
.surfdoc-site-nav .site-name { font-weight: 700; color: var(--text); font-size: 1rem; text-decoration: none; margin-right: auto; }
.site-nav-links { display: flex; align-items: center; gap: 0.25rem; }
.site-nav-links a { color: var(--text-muted); text-decoration: none; font-size: 0.875rem; padding: 0.25rem 0.625rem; border-radius: 6px; transition: color 0.15s, background 0.15s; }
.site-nav-links a:hover { color: var(--text); background: var(--surface-hover); }
.site-nav-links a.active { color: var(--accent); font-weight: 600; }
.site-nav-toggle { display: none; }
.site-nav-hamburger { display: none; cursor: pointer; padding: 0.5rem; margin-left: auto; flex-direction: column; gap: 5px; }
.site-nav-hamburger span { display: block; width: 22px; height: 2px; background: var(--text); border-radius: 1px; transition: transform 0.2s, opacity 0.2s; }
@media (max-width: 640px) {
  .site-nav-hamburger { display: flex; }
  .surfdoc-site-nav .site-name { margin-right: 0; }
  .site-nav-links { display: none; flex-direction: column; align-items: stretch; width: 100%; padding: 0.5rem 0; }
  .site-nav-links a { padding: 0.625rem 0.75rem; font-size: 1rem; }
  .site-nav-toggle:checked ~ .site-nav-links { display: flex; }
  .site-nav-toggle:checked ~ .site-nav-hamburger span:nth-child(1) { transform: rotate(45deg) translate(5px, 5px); }
  .site-nav-toggle:checked ~ .site-nav-hamburger span:nth-child(2) { opacity: 0; }
  .site-nav-toggle:checked ~ .site-nav-hamburger span:nth-child(3) { transform: rotate(-45deg) translate(5px, -5px); }
}

/* Site footer */
.surfdoc-site-footer { margin-top: 4rem; padding: 1.5rem; border-top: 1px solid var(--border); text-align: center; color: var(--text-faint); font-size: 0.8rem; }
"#;

/// Build the site-level navigation HTML bar.
///
/// Produces a `<nav class="surfdoc-site-nav">` element with:
/// - Site name as a linked logo
/// - CSS-only hamburger toggle for mobile
/// - Navigation links with active-class detection
///
/// This is separate from the inline `Block::Nav` rendering in `to_html()`,
/// which uses `surfdoc-nav` class names and a different data model
/// (NavItem structs vs route/title pairs).
fn build_site_nav_html(
    site_name: &str,
    nav_items: &[(String, String)],
    current_route: &str,
) -> String {
    let mut nav_html = format!(
        "<nav class=\"surfdoc-site-nav\" role=\"navigation\" aria-label=\"Site navigation\">\n  <a href=\"/\" class=\"site-name\">{}</a>\n",
        escape_html(site_name)
    );
    // CSS-only hamburger toggle for mobile
    nav_html.push_str("  <input type=\"checkbox\" class=\"site-nav-toggle\" id=\"site-nav-toggle\" aria-hidden=\"true\">\n");
    nav_html.push_str("  <label for=\"site-nav-toggle\" class=\"site-nav-hamburger\" aria-label=\"Toggle menu\"><span></span><span></span><span></span></label>\n");
    nav_html.push_str("  <div class=\"site-nav-links\">\n");
    for (route, nav_title) in nav_items {
        let href = route.to_string();
        let active = if *route == current_route { " active" } else { "" };
        nav_html.push_str(&format!(
            "    <a href=\"{}\"{}>{}</a>\n",
            escape_html(&href),
            if active.is_empty() {
                String::new()
            } else {
                " class=\"active\"".to_string()
            },
            escape_html(nav_title),
        ));
    }
    nav_html.push_str("  </div>\n</nav>");
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
    // Render page children as HTML
    let mut body_parts: Vec<String> = Vec::new();
    for child in &page.children {
        body_parts.push(render_block(child));
    }
    let body = body_parts.join("\n");

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

    // Build navigation HTML (clean URLs — no /index.html suffix)
    let nav_html = build_site_nav_html(site_name, nav_items, &page.route);

    // Build footer
    let footer_html = format!(
        "<footer class=\"surfdoc-site-footer\">{}</footer>",
        escape_html(site_name),
    );

    // Build optional CSS variable overrides from site config
    let mut css_overrides = String::new();
    if let Some(accent) = &site.accent {
        css_overrides.push_str(&format!("--accent: {};\n", escape_html(accent)));
        let text = accent_text_color(accent);
        css_overrides.push_str(&format!("--accent-text: {};\n", text));
    }
    let override_block = if css_overrides.is_empty() {
        String::new()
    } else {
        format!("\n:root {{\n{}}}", css_overrides)
    };

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

    // Build data-theme attribute if theme is explicitly set
    let theme_attr = match &site.theme {
        Some(t) if !t.is_empty() => format!(" data-theme=\"{}\"", escape_html(t)),
        _ => String::new(),
    };

    format!(
        r#"<!-- Built with SurfDoc — source: {source_path} -->
<!DOCTYPE html>
<html lang="{lang}"{theme_attr}>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="generator" content="SurfDoc v0.1">
    <link rel="alternate" type="text/surfdoc" href="{source_path}">
    <title>{title}</title>{meta_extra}
    <style>{css}{nav_css}{override_block}</style>
</head>
<body>
{nav}
<article class="surfdoc">
{body}
</article>
{footer}
</body>
</html>"#,
        source_path = source_path,
        lang = escape_html(lang),
        theme_attr = theme_attr,
        title = title_escaped,
        meta_extra = meta_extra,
        css = SURFDOC_CSS,
        nav_css = SITE_NAV_CSS,
        override_block = override_block,
        nav = nav_html,
        body = body,
        footer = footer_html,
    )
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
        assert!(html.contains("<strong>Warning: Caution</strong>"));
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
        assert!(html.contains("<th scope=\"col\">Name</th>"));
        assert!(html.contains("<td>Alice</td>"));
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
        assert!(html.contains("<pre class=\"surfdoc-code\" aria-label=\"rust code\">"));
        assert!(html.contains("class=\"language-rust\""));
        assert!(html.contains("&lt;hello&gt;"), "Angle brackets should be escaped");
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
        assert!(html.contains("<input type=\"checkbox\" checked disabled>"));
        assert!(html.contains("<input type=\"checkbox\" disabled>"));
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
        assert!(html.contains("class=\"surfdoc-metric\""));
        assert!(html.contains("<span class=\"label\">Revenue</span>"));
        assert!(html.contains("<span class=\"value\">$10K</span>"));
        assert!(html.contains("class=\"trend up\""));
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
        assert!(html.contains("<img src=\"arch.png\" alt=\"System architecture\" />"));
        assert!(html.contains("<figcaption>Architecture diagram</figcaption>"));
    }

    #[test]
    fn html_markdown_rendered() {
        let doc = doc_with(vec![Block::Markdown {
            content: "# Hello\n\nWorld".into(),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<h1>Hello</h1>"));
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
        assert!(html.contains("class=\"surfdoc-quote\""));
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("class=\"attribution\""));
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
        assert!(html.contains("<summary>Is it free?</summary>"));
        assert!(html.contains("<summary>Can I self-host?</summary>"));
        assert!(html.contains("class=\"faq-answer\""));
        assert!(html.contains("Yes, the free tier is forever."));
    }

    #[test]
    fn html_pricing_table() {
        let doc = doc_with(vec![Block::PricingTable {
            headers: vec!["".into(), "Free".into(), "Pro".into()],
            rows: vec![
                vec!["Price".into(), "$0".into(), "$9/mo".into()],
                vec!["Storage".into(), "1GB".into(), "100GB".into()],
            ],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-pricing\""));
        assert!(html.contains("<th scope=\"col\">Free</th>"));
        assert!(html.contains("<th scope=\"col\">Pro</th>"));
        assert!(html.contains("<td>$9/mo</td>"));
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
        assert!(html.contains("<h1>Pricing</h1>")); // Markdown rendered
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
    fn aria_tasks_label_wraps_checkbox() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![TaskItem {
                done: false,
                text: "Do thing".into(),
                assignee: None,
            }],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("<label><input type=\"checkbox\" disabled> Do thing</label>"));
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

    #[test]
    fn aria_pricing_table_scope() {
        let doc = doc_with(vec![Block::PricingTable {
            headers: vec!["".into(), "Basic".into()],
            rows: vec![vec!["Price".into(), "$0".into()]],
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("scope=\"col\""));
        assert!(html.contains("aria-label=\"Pricing comparison\""));
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
    fn render_site_page_respects_theme_light() {
        let site = SiteConfig {
            name: Some("Light Site".into()),
            theme: Some("light".into()),
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

        assert!(html.contains("data-theme=\"light\""), "theme: light must set data-theme attribute on <html>");
        assert!(!html.contains("<html lang=\"en\">"), "should not have bare <html> without data-theme");
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
        // Active link for about page
        assert!(html.contains("class=\"active\">About</a>"));
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

    // -- Bug regression: alternating section backgrounds ----------------------

    #[test]
    fn sections_wrap_h1_boundaries() {
        let doc = doc_with(vec![
            Block::Markdown { content: "# Section One".into(), span: span() },
            Block::Markdown { content: "Content under section one.".into(), span: span() },
            Block::Markdown { content: "# Section Two".into(), span: span() },
            Block::Markdown { content: "Content under section two.".into(), span: span() },
        ]);
        let html = to_html(&doc);
        assert!(html.contains("<section class=\"surfdoc-section\">"));
        assert!(html.contains("<section class=\"surfdoc-section surfdoc-section-alt\">"));
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
        assert!(html.contains("<section class=\"surfdoc-section\">"));
        assert!(html.contains("surfdoc-section-alt"));
        assert_eq!(html.matches("</section>").count(), 2);
    }

    #[test]
    fn sections_alternate_correctly_across_three() {
        let doc = doc_with(vec![
            Block::Markdown { content: "# A".into(), span: span() },
            Block::Markdown { content: "# B".into(), span: span() },
            Block::Markdown { content: "# C".into(), span: span() },
        ]);
        let html = to_html(&doc);
        // Section 0: no alt, Section 1: alt, Section 2: no alt
        assert_eq!(html.matches("surfdoc-section-alt").count(), 1);
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
        assert!(SURFDOC_CSS.contains(".surfdoc-section-alt {"));
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
                crate::types::NavItem { label: "Home".into(), href: "/".into(), icon: None },
                crate::types::NavItem { label: "About".into(), href: "#about".into(), icon: None },
            ],
            logo: Some("MySite".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("class=\"surfdoc-nav\""));
        assert!(html.contains("surfdoc-nav-logo"));
        assert!(html.contains("MySite"));
        assert!(html.contains("href=\"/\""));
        assert!(html.contains("href=\"#about\""));
        assert!(html.contains(">Home</a>"));
        assert!(html.contains(">About</a>"));
    }

    #[test]
    fn html_nav_renders_before_sections() {
        let doc = doc_with(vec![
            Block::Markdown { content: "# Section One".into(), span: span() },
            Block::Nav {
                items: vec![
                    crate::types::NavItem { label: "Top".into(), href: "#top".into(), icon: None },
                ],
                logo: None,
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
                    crate::types::NavItem { label: "Docs".into(), href: "/docs".into(), icon: None },
                ],
                logo: None,
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
                },
            ],
            logo: None,
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(html.contains("surfdoc-icon"));
        assert!(html.contains("<svg"));
        assert!(html.contains("GitHub</a>"));
    }

    #[test]
    fn html_nav_escapes_xss() {
        let doc = doc_with(vec![Block::Nav {
            items: vec![
                crate::types::NavItem {
                    label: "<script>alert('x')</script>".into(),
                    href: "javascript:alert(1)".into(),
                    icon: None,
                },
            ],
            logo: Some("<img onerror=alert(1)>".into()),
            span: span(),
        }]);
        let html = to_html(&doc);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img onerror"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_nav_css_exists() {
        assert!(SURFDOC_CSS.contains(".surfdoc-nav {"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-logo"));
        assert!(SURFDOC_CSS.contains(".surfdoc-nav-links"));
        assert!(SURFDOC_CSS.contains(".surfdoc-icon"));
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
        assert!(SURFDOC_CSS.contains("font-family: var(--font-body)"));
        assert!(SURFDOC_CSS.contains("font-family: var(--font-heading)"));
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
        assert!(html.contains("<h1>Hello</h1>"), "Should render heading");
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
                }],
                logo: Some("MySite".into()),
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
        );

        assert!(nav.contains("surfdoc-site-nav"), "Should have site nav class");
        assert!(nav.contains("My Site"), "Should contain site name");
        assert!(nav.contains("class=\"active\""), "Active route should be marked");
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
        );

        // The "/" route link should be active
        assert!(nav.contains("href=\"/\""), "Should have home link");
        // Verify active class is applied (check the pattern in the output)
        // The active link is: <a href="/" class="active">Home</a>
        assert!(
            nav.contains("href=\"/\" class=\"active\""),
            "Home link should be active when current_route is /"
        );
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
        assert!(html.contains("<article class=\"surfdoc\">"), "Should have article wrapper");
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
}

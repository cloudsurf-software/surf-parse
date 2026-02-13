//! HTML fragment renderer.
//!
//! Produces semantic HTML with `surfdoc-*` CSS classes. Markdown blocks are
//! rendered through `pulldown-cmark`. All other content is HTML-escaped to
//! prevent XSS.

use crate::icons::get_icon;
use crate::types::{Block, CalloutType, DecisionStatus, FormFieldType, StyleProperty, SurfDoc, Trend};

/// Render a markdown string to HTML using pulldown-cmark with GFM extensions.
fn render_markdown(content: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    let parser = pulldown_cmark::Parser::new_ext(content, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output
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
        _ => None,
    }
}

/// Apply font/style properties from a `StyleProperty` list to CSS overrides.
/// Collects any required font imports into the `imports` set.
fn apply_style_overrides(properties: &[StyleProperty], css_overrides: &mut String, imports: &mut Vec<&'static str>) {
    for prop in properties {
        match prop.key.as_str() {
            "accent" => css_overrides.push_str(&format!(
                "--accent: {};", escape_html(&prop.value)
            )),
            "font" => {
                // Legacy: sets both heading and body
                if let Some(preset) = resolve_font_preset(&prop.value) {
                    css_overrides.push_str(&format!("--font-heading: {};", preset.stack));
                    css_overrides.push_str(&format!("--font-body: {};", preset.stack));
                    if let Some(url) = preset.import {
                        if !imports.contains(&url) { imports.push(url); }
                    }
                }
            }
            "heading-font" => {
                if let Some(preset) = resolve_font_preset(&prop.value) {
                    css_overrides.push_str(&format!("--font-heading: {};", preset.stack));
                    if let Some(url) = preset.import {
                        if !imports.contains(&url) { imports.push(url); }
                    }
                }
            }
            "body-font" => {
                if let Some(preset) = resolve_font_preset(&prop.value) {
                    css_overrides.push_str(&format!("--font-body: {};", preset.stack));
                    if let Some(url) = preset.import {
                        if !imports.contains(&url) { imports.push(url); }
                    }
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

/// Embedded CSS for standalone SurfDoc pages.
///
/// Dark theme ported from the CloudSurf strategy-web reference implementation.
/// Covers base typography, markdown elements, and all SurfDoc block types.
const SURFDOC_CSS: &str = r#"
:root {
    --bg: #0a0a0f;
    --bg-card: #12121a;
    --bg-hover: #1a1a26;
    --border: #2a2a3a;
    --border-subtle: #1e1e2e;
    --text: #e8e8f0;
    --text-dim: #8888a0;
    --text-muted: #5a5a72;
    --accent: #3b82f6;
    --bg-alt: #161B22;
    --font-heading: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, sans-serif;
    --font-body: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, sans-serif;
}

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body { background: var(--bg); color: var(--text); font-family: var(--font-body); -webkit-font-smoothing: antialiased; }
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: var(--border); border-radius: 3px; }

/* Layout */
.surfdoc { max-width: 48rem; margin: 0 auto; padding: 2rem 1.5rem 4rem; line-height: 1.7; }

/* Navigation bar */
.surfdoc-nav { display: flex; align-items: center; gap: 1rem; padding: 0.75rem 1.5rem; background: var(--bg-card); border-bottom: 1px solid var(--border-subtle); position: sticky; top: 0; z-index: 100; width: 100vw; margin-left: calc(-50vw + 50%); margin-top: -2rem; margin-bottom: 1rem; backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px); }
.surfdoc-nav-logo { font-weight: 700; color: #fff; font-size: 1rem; margin-right: auto; white-space: nowrap; }
.surfdoc-nav-links { display: flex; align-items: center; gap: 0.25rem; flex-wrap: wrap; }
.surfdoc-nav-links a { color: var(--text-dim); text-decoration: none; font-size: 0.875rem; padding: 0.25rem 0.625rem; border-radius: 6px; transition: color 0.15s, background 0.15s; display: inline-flex; align-items: center; gap: 0.375rem; }
.surfdoc-nav-links a:hover { color: var(--text); background: var(--bg-hover); text-decoration: none; }
.surfdoc-nav-toggle { display: none; }
.surfdoc-nav-hamburger { display: none; cursor: pointer; padding: 0.5rem; margin-left: auto; flex-direction: column; gap: 5px; }
.surfdoc-nav-hamburger span { display: block; width: 22px; height: 2px; background: var(--text); border-radius: 1px; transition: transform 0.2s, opacity 0.2s; }
@media (max-width: 640px) {
    .surfdoc-nav-hamburger { display: flex; }
    .surfdoc-nav-logo { margin-right: 0; }
    .surfdoc-nav-links { display: none; flex-direction: column; align-items: stretch; position: absolute; top: 100%; left: 0; right: 0; background: var(--bg-card); padding: 0.5rem 1rem; border-bottom: 1px solid var(--border-subtle); box-shadow: 0 4px 12px rgba(0,0,0,0.2); }
    .surfdoc-nav-links a { padding: 0.625rem 0.75rem; font-size: 1rem; }
    .surfdoc-nav-toggle:checked ~ .surfdoc-nav-links { display: flex; }
    .surfdoc-nav-toggle:checked ~ .surfdoc-nav-hamburger span:nth-child(1) { transform: rotate(45deg) translate(5px, 5px); }
    .surfdoc-nav-toggle:checked ~ .surfdoc-nav-hamburger span:nth-child(2) { opacity: 0; }
    .surfdoc-nav-toggle:checked ~ .surfdoc-nav-hamburger span:nth-child(3) { transform: rotate(-45deg) translate(5px, -5px); }
}

/* Inline icons */
.surfdoc-icon { display: inline-flex; align-items: center; vertical-align: middle; line-height: 0; }
.surfdoc-icon svg { width: 1em; height: 1em; }

/* Sections — alternating backgrounds */
.surfdoc-section { padding: 2rem 0; }
.surfdoc-section-alt { background: var(--bg-alt, #161B22); width: 100vw; margin-left: calc(-50vw + 50%); padding: 2rem calc(50vw - 50%); }
.surfdoc-section h2 { margin-top: 0; }

/* Typography */
.surfdoc h1, .surfdoc h2, .surfdoc h3, .surfdoc h4 { font-family: var(--font-heading); }
.surfdoc h1 { font-size: 1.875rem; font-weight: 700; margin: 2rem 0 1rem; letter-spacing: -0.025em; }
.surfdoc h2 { font-size: 1.5rem; font-weight: 600; margin: 1.75rem 0 0.75rem; padding-bottom: 0.5rem; border-bottom: 1px solid var(--border-subtle); }
.surfdoc h3 { font-size: 1.25rem; font-weight: 600; margin: 1.5rem 0 0.5rem; }
.surfdoc h4 { font-size: 1.1rem; font-weight: 600; margin: 1.25rem 0 0.5rem; color: var(--text-dim); }
.surfdoc p { margin: 0.75rem 0; }
.surfdoc a { color: var(--accent); text-decoration: none; }
.surfdoc a:hover { text-decoration: underline; }
.surfdoc strong { font-weight: 600; color: #fff; }
.surfdoc em { color: var(--text-dim); }
.surfdoc ul, .surfdoc ol { margin: 0.5rem 0; padding-left: 1.5rem; }
.surfdoc li { margin: 0.25rem 0; }
.surfdoc li::marker { color: var(--text-muted); }
.surfdoc blockquote { border-left: 3px solid var(--accent); padding: 0.5rem 1rem; margin: 1rem 0; background: rgba(59,130,246,0.05); border-radius: 0 6px 6px 0; }
.surfdoc blockquote p { margin: 0.25rem 0; }
.surfdoc code { font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace; font-size: 0.85em; background: rgba(255,255,255,0.06); padding: 0.15em 0.4em; border-radius: 4px; }
.surfdoc pre { background: #0d1117 !important; border: 1px solid var(--border-subtle); border-radius: 8px; padding: 1rem; overflow-x: auto; margin: 1rem 0; }
.surfdoc pre code { background: transparent; padding: 0; font-size: 0.8rem; line-height: 1.6; }
.surfdoc table { width: 100%; border-collapse: collapse; margin: 1rem 0; font-size: 0.875rem; }
.surfdoc th { text-align: left; padding: 0.5rem 0.75rem; border-bottom: 2px solid var(--border); font-weight: 600; color: var(--text-dim); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.03em; }
.surfdoc td { padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border-subtle); }
.surfdoc tr:hover td { background: rgba(255,255,255,0.02); }
.surfdoc hr { border: none; border-top: 1px solid var(--border-subtle); margin: 2rem 0; }
.surfdoc img { max-width: 100%; border-radius: 8px; }

/* Callout blocks */
.surfdoc-callout { border-left: 3px solid; padding: 0.75rem 1rem; margin: 1rem 0; border-radius: 0 8px 8px 0; background: var(--bg-card); }
.surfdoc-callout strong { display: block; margin-bottom: 0.25rem; font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.04em; }
.surfdoc-callout p { margin: 0; }
.surfdoc-callout-info { border-color: #3b82f6; }
.surfdoc-callout-info strong { color: #3b82f6; }
.surfdoc-callout-warning { border-color: #f59e0b; }
.surfdoc-callout-warning strong { color: #f59e0b; }
.surfdoc-callout-danger { border-color: #ef4444; }
.surfdoc-callout-danger strong { color: #ef4444; }
.surfdoc-callout-tip { border-color: #22c55e; }
.surfdoc-callout-tip strong { color: #22c55e; }
.surfdoc-callout-note { border-color: #06b6d4; }
.surfdoc-callout-note strong { color: #06b6d4; }
.surfdoc-callout-success { border-color: #22c55e; background: rgba(34,197,94,0.05); }
.surfdoc-callout-success strong { color: #22c55e; }

/* Data tables */
.surfdoc-data { width: 100%; border-collapse: collapse; margin: 1rem 0; font-size: 0.875rem; border: 1px solid var(--border-subtle); border-radius: 8px; overflow: hidden; }
.surfdoc-data thead { background: var(--bg-card); }
.surfdoc-data th { text-align: left; padding: 0.625rem 0.75rem; font-weight: 600; color: var(--text-dim); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.03em; border-bottom: 2px solid var(--border); }
.surfdoc-data td { padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border-subtle); }
.surfdoc-data tr:hover td { background: rgba(255,255,255,0.02); }
.surfdoc-data tr:last-child td { border-bottom: none; }

/* Code blocks */
.surfdoc-code { background: #0d1117; border: 1px solid var(--border-subtle); border-radius: 8px; padding: 1rem; overflow-x: auto; margin: 1rem 0; font-size: 0.8rem; line-height: 1.6; }
.surfdoc-code code { background: transparent; padding: 0; font-family: "SF Mono", "Fira Code", "Cascadia Code", monospace; }

/* Task lists */
.surfdoc-tasks { list-style: none; padding-left: 0; margin: 1rem 0; }
.surfdoc-tasks li { display: flex; align-items: center; gap: 0.5rem; padding: 0.375rem 0.75rem; margin: 0.125rem 0; border-radius: 6px; font-size: 0.9rem; }
.surfdoc-tasks li:hover { background: var(--bg-hover); }
.surfdoc-tasks input[type="checkbox"] { accent-color: var(--accent); width: 16px; height: 16px; }
.surfdoc-tasks .assignee { color: var(--accent); font-size: 0.8rem; margin-left: auto; }

/* Decision records */
.surfdoc-decision { border-left: 3px solid; padding: 0.75rem 1rem; margin: 1rem 0; border-radius: 0 8px 8px 0; background: var(--bg-card); }
.surfdoc-decision .status { display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px; font-size: 0.75rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; margin-right: 0.5rem; }
.surfdoc-decision .date { color: var(--text-muted); font-size: 0.8rem; }
.surfdoc-decision p { margin: 0.5rem 0 0; }
.surfdoc-decision-accepted { border-color: #22c55e; }
.surfdoc-decision-accepted .status { background: rgba(34,197,94,0.15); color: #22c55e; }
.surfdoc-decision-rejected { border-color: #ef4444; }
.surfdoc-decision-rejected .status { background: rgba(239,68,68,0.15); color: #ef4444; }
.surfdoc-decision-proposed { border-color: #f59e0b; }
.surfdoc-decision-proposed .status { background: rgba(245,158,11,0.15); color: #f59e0b; }
.surfdoc-decision-superseded { border-color: var(--text-muted); }
.surfdoc-decision-superseded .status { background: rgba(90,90,114,0.15); color: var(--text-muted); }

/* Metric displays */
.surfdoc-metric { display: inline-flex; align-items: baseline; gap: 0.5rem; padding: 0.625rem 1rem; margin: 0.5rem 0.5rem 0.5rem 0; background: var(--bg-card); border: 1px solid var(--border-subtle); border-radius: 8px; }
.surfdoc-metric .label { color: var(--text-dim); font-size: 0.8rem; font-weight: 500; }
.surfdoc-metric .value { font-size: 1.25rem; font-weight: 700; color: #fff; }
.surfdoc-metric .unit { color: var(--text-muted); font-size: 0.8rem; }
.surfdoc-metric .trend { font-size: 1rem; }
.surfdoc-metric .trend.up { color: #22c55e; }
.surfdoc-metric .trend.down { color: #ef4444; }
.surfdoc-metric .trend.flat { color: var(--text-muted); }

/* Summary blocks */
.surfdoc-summary { border-left: 3px solid var(--accent); padding: 0.75rem 1rem; margin: 1rem 0; background: rgba(59,130,246,0.04); border-radius: 0 8px 8px 0; font-style: italic; color: var(--text-dim); }
.surfdoc-summary p { margin: 0; }

/* Figure blocks */
.surfdoc-figure { margin: 1.5rem 0; text-align: center; }
.surfdoc-figure img { max-width: 100%; border-radius: 8px; border: 1px solid var(--border-subtle); }
.surfdoc-figure figcaption { margin-top: 0.5rem; font-size: 0.85rem; color: var(--text-muted); font-style: italic; }

/* Unknown blocks */
.surfdoc-unknown { padding: 0.75rem 1rem; margin: 1rem 0; background: var(--bg-card); border: 1px dashed var(--border); border-radius: 8px; color: var(--text-dim); font-size: 0.875rem; }

/* Tabs blocks */
.surfdoc-tabs { margin: 1rem 0; border: 1px solid var(--border-subtle); border-radius: 8px; overflow: hidden; }
.surfdoc-tabs nav { display: flex; background: var(--bg-card); border-bottom: 1px solid var(--border-subtle); }
.surfdoc-tabs nav button { padding: 0.5rem 1rem; background: none; border: none; color: var(--text-muted); font-size: 0.85rem; cursor: pointer; border-bottom: 2px solid transparent; transition: all 0.15s; }
.surfdoc-tabs nav button:hover { color: var(--text); background: var(--bg-hover); }
.surfdoc-tabs nav button.active { color: var(--accent); border-bottom-color: var(--accent); }
.surfdoc-tabs .tab-panel { padding: 1rem; display: none; }
.surfdoc-tabs .tab-panel.active { display: block; }

/* Columns layout */
.surfdoc-columns { display: grid; gap: 1.5rem; margin: 1rem 0; }
.surfdoc-columns[data-cols="2"] { grid-template-columns: repeat(2, 1fr); }
.surfdoc-columns[data-cols="3"] { grid-template-columns: repeat(3, 1fr); }
.surfdoc-columns[data-cols="4"] { grid-template-columns: repeat(4, 1fr); }
.surfdoc-column { min-width: 0; }
@media (max-width: 640px) {
    .surfdoc-columns { grid-template-columns: 1fr !important; }
}

/* Quote blocks */
.surfdoc-quote { border-left: 3px solid var(--text-muted); padding: 0.75rem 1.25rem; margin: 1.5rem 0; }
.surfdoc-quote blockquote { border: none; padding: 0; margin: 0; background: none; font-size: 1.1rem; font-style: italic; color: var(--text-dim); line-height: 1.6; }
.surfdoc-quote .attribution { margin-top: 0.5rem; font-size: 0.85rem; color: var(--text-muted); font-style: normal; }
.surfdoc-quote .attribution::before { content: "— "; }

/* CTA buttons — use a.surfdoc-cta to beat .surfdoc a specificity */
a.surfdoc-cta { display: inline-block; padding: 0.625rem 1.5rem; margin: 0.5rem 0.5rem 0.5rem 0; border-radius: 8px; font-weight: 600; font-size: 0.95rem; text-decoration: none; transition: all 0.15s; cursor: pointer; }
a.surfdoc-cta-primary { background: var(--accent); color: #fff; border: 1px solid var(--accent); }
a.surfdoc-cta-primary:hover { background: #2563eb; color: #fff; text-decoration: none; }
a.surfdoc-cta-secondary { background: transparent; color: var(--accent); border: 1px solid var(--border); }
a.surfdoc-cta-secondary:hover { background: var(--bg-hover); color: var(--accent); text-decoration: none; }

/* Hero image */
.surfdoc-hero-image { margin: 2rem 0; text-align: center; }
.surfdoc-hero-image img { max-width: 100%; border-radius: 12px; box-shadow: 0 8px 32px rgba(0,0,0,0.3); border: 1px solid var(--border-subtle); }

/* Testimonials */
.surfdoc-testimonial { padding: 1.25rem 1.5rem; margin: 1rem 0; background: var(--bg-card); border: 1px solid var(--border-subtle); border-radius: 12px; position: relative; }
.surfdoc-testimonial blockquote { border: none; background: none; padding: 0; margin: 0 0 0.75rem; font-size: 1rem; font-style: italic; color: var(--text-dim); line-height: 1.6; }
.surfdoc-testimonial .author { font-weight: 600; color: var(--text); font-size: 0.9rem; }
.surfdoc-testimonial .role { color: var(--text-muted); font-size: 0.8rem; }

/* Style blocks — invisible, metadata only */
.surfdoc-style { display: none; }

/* FAQ accordion */
.surfdoc-faq { margin: 1rem 0; }
.surfdoc-faq details { border: 1px solid var(--border-subtle); border-radius: 8px; margin: 0.5rem 0; overflow: hidden; }
.surfdoc-faq summary { padding: 0.75rem 1rem; font-weight: 600; cursor: pointer; background: var(--bg-card); color: var(--text); font-size: 0.95rem; }
.surfdoc-faq summary:hover { background: var(--bg-hover); }
.surfdoc-faq .faq-answer { padding: 0.75rem 1rem; color: var(--text-dim); line-height: 1.6; border-top: 1px solid var(--border-subtle); }

/* Pricing table */
.surfdoc-pricing { width: 100%; border-collapse: collapse; margin: 1rem 0; font-size: 0.875rem; border: 1px solid var(--border-subtle); border-radius: 8px; overflow: hidden; }
.surfdoc-pricing thead { background: var(--bg-card); }
.surfdoc-pricing th { text-align: center; padding: 0.75rem; font-weight: 600; color: var(--text); border-bottom: 2px solid var(--border); font-size: 0.95rem; }
.surfdoc-pricing th:first-child { text-align: left; color: var(--text-muted); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.03em; }
.surfdoc-pricing td { padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border-subtle); text-align: center; }
.surfdoc-pricing td:first-child { text-align: left; font-weight: 500; color: var(--text-dim); }
.surfdoc-pricing tr:hover td { background: rgba(255,255,255,0.02); }
.surfdoc-pricing tr:last-child td { border-bottom: none; }

/* Site config — invisible, metadata only */
.surfdoc-site { display: none; }

/* Page sections */
.surfdoc-page { margin: 2rem 0; padding: 2rem 0; border-top: 2px solid var(--border-subtle); }
.surfdoc-page:first-of-type { border-top: none; margin-top: 0; }
.surfdoc-page[data-layout="hero"] { text-align: center; padding: 4rem 0; }
.surfdoc-page[data-layout="hero"] h1 { font-size: 2.5rem; margin-bottom: 1rem; }
.surfdoc-page[data-layout="hero"] p { font-size: 1.15rem; color: var(--text-dim); max-width: 36rem; margin: 0 auto 1.5rem; }
.surfdoc-page[data-layout="hero"] .surfdoc-cta { margin: 0.5rem; }
.surfdoc-page[data-layout="cards"] { display: grid; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); gap: 1.5rem; }
.surfdoc-page[data-layout="split"] { display: grid; grid-template-columns: 1fr 1fr; gap: 2rem; align-items: center; }
@media (max-width: 640px) {
    .surfdoc-page[data-layout="split"] { grid-template-columns: 1fr; }
    .surfdoc-page[data-layout="hero"] h1 { font-size: 1.75rem; }
}

/* Embed blocks */
.surfdoc-embed { margin: 1.5rem 0; border-radius: 12px; overflow: hidden; border: 1px solid var(--border-subtle); }
.surfdoc-embed iframe { display: block; width: 100%; border: none; }
.surfdoc-embed-map { aspect-ratio: 16/9; }
.surfdoc-embed-map iframe { height: 100%; }
.surfdoc-embed-video { aspect-ratio: 16/9; }
.surfdoc-embed-video iframe { height: 100%; }

/* Form blocks */
.surfdoc-form { margin: 1.5rem 0; padding: 1.5rem; background: var(--bg-card); border: 1px solid var(--border-subtle); border-radius: 12px; }
.surfdoc-form-field { margin-bottom: 1rem; }
.surfdoc-form-field label { display: block; font-size: 0.875rem; font-weight: 500; color: var(--text-dim); margin-bottom: 0.375rem; }
.surfdoc-form-field .required { color: #ef4444; }
.surfdoc-form-field input, .surfdoc-form-field textarea, .surfdoc-form-field select { width: 100%; padding: 0.625rem 0.75rem; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; color: var(--text); font-size: 0.9375rem; font-family: inherit; outline: none; transition: border-color 0.15s; }
.surfdoc-form-field input:focus, .surfdoc-form-field textarea:focus, .surfdoc-form-field select:focus { border-color: var(--accent); }
.surfdoc-form-field select { appearance: auto; }

/* Gallery blocks */
.surfdoc-gallery { margin: 1.5rem 0; }
.surfdoc-gallery-filters { display: flex; gap: 0.5rem; margin-bottom: 1rem; flex-wrap: wrap; }
.surfdoc-gallery-filters .filter-btn { padding: 0.375rem 0.875rem; border: 1px solid var(--border); background: transparent; color: var(--text-dim); border-radius: 20px; font-size: 0.8rem; cursor: pointer; transition: all 0.15s; }
.surfdoc-gallery-filters .filter-btn:hover { color: var(--text); border-color: var(--text-muted); }
.surfdoc-gallery-filters .filter-btn.active { background: var(--accent); color: #fff; border-color: var(--accent); }
.surfdoc-gallery-grid { display: grid; gap: 1rem; }
.surfdoc-gallery[data-cols="2"] .surfdoc-gallery-grid { grid-template-columns: repeat(2, 1fr); }
.surfdoc-gallery[data-cols="3"] .surfdoc-gallery-grid { grid-template-columns: repeat(3, 1fr); }
.surfdoc-gallery[data-cols="4"] .surfdoc-gallery-grid { grid-template-columns: repeat(4, 1fr); }
.surfdoc-gallery-item { margin: 0; overflow: hidden; border-radius: 8px; border: 1px solid var(--border-subtle); }
.surfdoc-gallery-item img { width: 100%; aspect-ratio: 4/3; object-fit: cover; display: block; transition: transform 0.2s; }
.surfdoc-gallery-item:hover img { transform: scale(1.03); }
.surfdoc-gallery-item figcaption { padding: 0.5rem 0.75rem; font-size: 0.8rem; color: var(--text-dim); background: var(--bg-card); }
@media (max-width: 640px) {
    .surfdoc-gallery-grid { grid-template-columns: repeat(2, 1fr) !important; }
}

/* Footer blocks */
.surfdoc-footer { margin-top: 3rem; padding: 2rem 0; border-top: 1px solid var(--border-subtle); width: 100vw; margin-left: calc(-50vw + 50%); padding-left: calc(50vw - 50%); padding-right: calc(50vw - 50%); background: var(--bg-card); }
.surfdoc-footer-sections { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 2rem; margin-bottom: 1.5rem; }
.surfdoc-footer-col h4 { font-size: 0.8rem; font-weight: 600; color: var(--text); text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 0.75rem; }
.surfdoc-footer-col ul { list-style: none; padding: 0; }
.surfdoc-footer-col li { margin: 0.25rem 0; }
.surfdoc-footer-col a { color: var(--text-dim); text-decoration: none; font-size: 0.875rem; transition: color 0.15s; }
.surfdoc-footer-col a:hover { color: var(--text); text-decoration: none; }
.surfdoc-footer-social { display: flex; gap: 1rem; margin-bottom: 1rem; }
.surfdoc-footer-social .social-link { color: var(--text-dim); text-decoration: none; font-size: 0.875rem; text-transform: capitalize; transition: color 0.15s; }
.surfdoc-footer-social .social-link:hover { color: var(--accent); }
.surfdoc-footer-copyright { font-size: 0.8rem; color: var(--text-muted); }
@media (max-width: 640px) {
    .surfdoc-footer-sections { grid-template-columns: repeat(2, 1fr); }
}
"#;

/// Escape HTML special characters to prevent XSS.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
                "<div class=\"surfdoc-callout surfdoc-callout-{type_str}\" role=\"{role}\"><strong>{heading}</strong><p>{}</p></div>",
                escape_html(content),
            )
        }

        Block::Data {
            headers, rows, ..
        } => {
            let mut html = String::from("<table class=\"surfdoc-data\">");
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
            html.push_str("</tbody></table>");
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
                "<div class=\"surfdoc-decision surfdoc-decision-{status_str}\" role=\"note\" aria-label=\"Decision: {status_str}\"><span class=\"status\">{status_str}</span>{date_html}<p>{}</p></div>",
                escape_html(content),
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
            let mut html = String::from("<table class=\"surfdoc-pricing\" aria-label=\"Pricing comparison\">");
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
            html.push_str("</tbody></table>");
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
            for item in items {
                let alt = item.alt.as_deref().unwrap_or("");
                let cat_attr = match &item.category {
                    Some(c) => format!(" data-category=\"{}\"", escape_html(c)),
                    None => String::new(),
                };
                html.push_str(&format!("<figure class=\"surfdoc-gallery-item\"{cat_attr}>"));
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
            // Gallery filter JS
            if !categories.is_empty() {
                html.push_str(r#"<script>document.querySelectorAll('.surfdoc-gallery').forEach(g=>{g.querySelectorAll('.filter-btn').forEach(b=>{b.onclick=()=>{g.querySelectorAll('.filter-btn').forEach(e=>e.classList.remove('active'));b.classList.add('active');var f=b.dataset.filter;g.querySelectorAll('.surfdoc-gallery-item').forEach(i=>{i.style.display=f==='all'||i.dataset.category===f?'':'none'})}})})</script>"#);
            }
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

/// CSS for site-level navigation and footer.
const SITE_NAV_CSS: &str = r#"
/* Site navigation */
.surfdoc-site-nav { display: flex; align-items: center; gap: 1.5rem; padding: 0.75rem 1.5rem; background: var(--bg-card); border-bottom: 1px solid var(--border-subtle); max-width: 100%; position: sticky; top: 0; z-index: 100; }
.surfdoc-site-nav .site-name { font-weight: 700; color: #fff; font-size: 1rem; text-decoration: none; margin-right: auto; }
.surfdoc-site-nav a { color: var(--text-dim); text-decoration: none; font-size: 0.875rem; padding: 0.25rem 0.5rem; border-radius: 4px; transition: color 0.15s, background 0.15s; }
.surfdoc-site-nav a:hover { color: var(--text); background: var(--bg-hover); }
.surfdoc-site-nav a.active { color: var(--accent); font-weight: 600; }

/* Site footer */
.surfdoc-site-footer { margin-top: 4rem; padding: 1.5rem; border-top: 1px solid var(--border-subtle); text-align: center; color: var(--text-muted); font-size: 0.8rem; }
"#;

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

    // Title: page title > site name + route
    let title = match &page.title {
        Some(t) => format!("{} — {}", t, site_name),
        None if page.route == "/" => site_name.to_string(),
        None => format!("{} — {}", page.route.trim_start_matches('/'), site_name),
    };

    let source_path = escape_html(&config.source_path);

    // Build navigation HTML (clean URLs — no /index.html suffix)
    let mut nav_html = format!(
        "<nav class=\"surfdoc-site-nav\" role=\"navigation\" aria-label=\"Site navigation\">\n  <a href=\"/\" class=\"site-name\">{}</a>\n",
        escape_html(site_name)
    );
    for (route, nav_title) in nav_items {
        let href = route.to_string();
        let active = if *route == page.route { " active" } else { "" };
        nav_html.push_str(&format!(
            "  <a href=\"{}\"{}>{}</a>\n",
            escape_html(&href),
            if active.is_empty() {
                String::new()
            } else {
                format!(" class=\"active\"")
            },
            escape_html(nav_title),
        ));
    }
    nav_html.push_str("</nav>");

    // Build footer
    let footer_html = format!(
        "<footer class=\"surfdoc-site-footer\">{}</footer>",
        escape_html(site_name),
    );

    // Build optional CSS variable overrides from site config
    let mut css_overrides = String::new();
    if let Some(accent) = &site.accent {
        css_overrides.push_str(&format!("--accent: {};\n", escape_html(accent)));
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

    // -- Bug regression: CTA specificity fix (a.surfdoc-cta beats .surfdoc a) --

    #[test]
    fn css_cta_selectors_use_element_qualifier() {
        // The CSS must use `a.surfdoc-cta-primary` (specificity 0-1-1) to beat
        // `.surfdoc a` (also 0-1-1 but later in cascade). Without the `a` element
        // qualifier, link color var(--accent) overrides the white button text.
        assert!(SURFDOC_CSS.contains("a.surfdoc-cta-primary"));
        assert!(SURFDOC_CSS.contains("a.surfdoc-cta-secondary"));
        assert!(SURFDOC_CSS.contains("a.surfdoc-cta {"));
        // Every occurrence of .surfdoc-cta-primary must be preceded by `a`
        // (i.e. no bare `.surfdoc-cta-primary` without element qualifier)
        for (i, _) in SURFDOC_CSS.match_indices(".surfdoc-cta-primary") {
            if i == 0 || SURFDOC_CSS.as_bytes()[i - 1] != b'a' {
                panic!("Found bare .surfdoc-cta-primary without 'a' element qualifier at byte {}", i);
            }
        }
        for (i, _) in SURFDOC_CSS.match_indices(".surfdoc-cta-secondary") {
            if i == 0 || SURFDOC_CSS.as_bytes()[i - 1] != b'a' {
                panic!("Found bare .surfdoc-cta-secondary without 'a' element qualifier at byte {}", i);
            }
        }
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
        // Verify the CSS actually sets color: #fff on primary buttons
        assert!(SURFDOC_CSS.contains("a.surfdoc-cta-primary { background: var(--accent); color: #fff;"));
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
        assert!(html.contains("--bg:"));
        assert!(html.contains(".surfdoc {"));
        assert!(html.contains("a.surfdoc-cta-primary"));
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
        assert!(html.contains("<style>.surfdoc { --accent: #ec4899; }</style>"),
            "accent override should be scoped to .surfdoc, got: {}", html);
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
}

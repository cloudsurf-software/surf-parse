//! Presentation deck rendering — Slides as a first-class SurfDoc output type.
//!
//! This mirrors the `::site`/`::page` website path in [`crate::render_html`]:
//! a `::deck`/`::slide` block family is extracted into a [`DeckConfig`] plus a
//! list of [`SlideEntry`], then rendered to a single self-contained HTML file
//! using the canonical **`surf-dark`** reference theme (the shell ported from
//! `decks/surf-platform-pitch.html` — top progress bar, keyboard/click/touch
//! nav, fullscreen).
//!
//! Slide content reuses the existing block renderers ([`render_block`]) and the
//! bundled [`crate::SURFDOC_CSS`], so a slide is an *arrangement of blocks that
//! already exist* — no new content primitives. The deck chrome is layered on
//! top.
//!
//! When a document has neither a `::deck` nor any `::slide`, every `#`/`##`
//! heading boundary becomes a slide (Presentation Mode), so *any* SurfDoc is
//! already a deck with zero edits.

use crate::render_html::{escape_html, render_block};
use crate::types::{Block, SlideLayout, StyleProperty};
use crate::SurfDoc;

/// Deck-level configuration extracted from a `::deck` block (peer of
/// [`crate::SiteConfig`]).
#[derive(Debug, Clone, Default)]
pub struct DeckConfig {
    pub theme: Option<String>,
    pub aspect: Option<String>,
    pub transition: Option<String>,
    pub accent: Option<String>,
    pub font: Option<String>,
    pub title: Option<String>,
    /// Custom footer text (defaults to the deck title).
    pub footer: Option<String>,
    /// Slide-number / progress chrome toggle (default on). Set `numbers: off`
    /// (or `slide-numbers: false`) on `::deck` to hide the `n / N` counter.
    pub numbers: Option<String>,
    pub properties: Vec<StyleProperty>,
}

impl DeckConfig {
    /// Resolve the theme name, defaulting to the canonical `surf-dark`.
    pub fn theme_name(&self) -> &str {
        self.theme.as_deref().unwrap_or("surf-dark")
    }

    /// Aspect ratio, defaulting to the canonical `16:9`.
    pub fn aspect_ratio(&self) -> &str {
        self.aspect.as_deref().unwrap_or("16:9")
    }

    /// Transition name, defaulting to `fade`.
    pub fn transition_name(&self) -> &str {
        self.transition.as_deref().unwrap_or("fade")
    }

    /// Whether to show the slide-number counter (default true; off when the
    /// `numbers`/`slide-numbers` value is a falsy word).
    pub fn show_numbers(&self) -> bool {
        match self.numbers.as_deref() {
            Some(v) => !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "off" | "false" | "no" | "none" | "0" | "hide" | "hidden"
            ),
            None => true,
        }
    }
}

/// A single slide extracted from a `::slide` block (or an auto-split section).
#[derive(Debug, Clone)]
pub struct SlideEntry {
    pub layout: SlideLayout,
    pub kicker: Option<String>,
    pub notes: Option<String>,
    pub children: Vec<Block>,
}

/// Extract deck config and slide list from a parsed [`SurfDoc`].
///
/// Returns `(deck_config, slides)`. If the document contains explicit
/// `::slide` blocks they are used directly; otherwise the document body is
/// auto-split into slides on `#`/`##` heading boundaries.
pub fn extract_deck(doc: &SurfDoc) -> (DeckConfig, Vec<SlideEntry>) {
    let mut config = DeckConfig::default();
    let mut explicit: Vec<SlideEntry> = Vec::new();
    let mut loose: Vec<Block> = Vec::new();

    for block in &doc.blocks {
        match block {
            Block::Deck { properties, .. } => {
                config.properties = properties.clone();
                for prop in properties {
                    match prop.key.as_str() {
                        "theme" => config.theme = Some(prop.value.clone()),
                        "aspect" => config.aspect = Some(prop.value.clone()),
                        "transition" => config.transition = Some(prop.value.clone()),
                        "accent" => config.accent = Some(prop.value.clone()),
                        "font" => config.font = Some(prop.value.clone()),
                        "title" => config.title = Some(prop.value.clone()),
                        "footer" => config.footer = Some(prop.value.clone()),
                        "numbers" | "slide-numbers" | "slide_numbers" => {
                            config.numbers = Some(prop.value.clone())
                        }
                        _ => {}
                    }
                }
            }
            Block::Slide {
                layout,
                kicker,
                notes,
                children,
                ..
            } => {
                explicit.push(SlideEntry {
                    layout: layout.unwrap_or_default(),
                    kicker: kicker.clone(),
                    notes: notes.clone(),
                    children: children.clone(),
                });
            }
            other => loose.push(other.clone()),
        }
    }

    // Title falls back to front-matter title.
    if config.title.is_none()
        && let Some(fm) = &doc.front_matter
    {
        config.title = fm.title.clone();
    }

    let slides = if !explicit.is_empty() {
        explicit
    } else {
        auto_split(&loose)
    };

    (config, slides)
}

/// Auto-split loose top-level blocks into slides on heading boundaries.
///
/// A `# ` (H1) or `## ` (H2) line inside a [`Block::Markdown`] starts a new
/// slide. H1-led slides get the [`SlideLayout::Cover`] layout; everything else
/// is [`SlideLayout::Bullets`]. Non-markdown blocks attach to the current
/// slide (or open one).
fn auto_split(blocks: &[Block]) -> Vec<SlideEntry> {
    let mut slides: Vec<SlideEntry> = Vec::new();
    let mut current: Option<SlideEntry> = None;

    for block in blocks {
        match block {
            Block::Markdown { content, span } => {
                for (level, text) in split_md_sections(content) {
                    let text = text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    // A heading boundary opens a fresh slide.
                    if level.is_some() {
                        if let Some(done) = current.take() {
                            slides.push(done);
                        }
                        current = Some(SlideEntry {
                            layout: if level == Some(1) {
                                SlideLayout::Cover
                            } else {
                                SlideLayout::Bullets
                            },
                            kicker: None,
                            notes: None,
                            children: Vec::new(),
                        });
                    } else if current.is_none() {
                        current = Some(SlideEntry {
                            layout: SlideLayout::Bullets,
                            kicker: None,
                            notes: None,
                            children: Vec::new(),
                        });
                    }
                    current.as_mut().unwrap().children.push(Block::Markdown {
                        content: text.to_string(),
                        span: *span,
                    });
                }
            }
            other => {
                if current.is_none() {
                    current = Some(SlideEntry {
                        layout: SlideLayout::Bullets,
                        kicker: None,
                        notes: None,
                        children: Vec::new(),
                    });
                }
                current.as_mut().unwrap().children.push(other.clone());
            }
        }
    }

    if let Some(done) = current {
        slides.push(done);
    }
    slides
}

/// Split a markdown string on `#`/`##` heading boundaries.
///
/// Returns `(heading_level, section_markdown)` pairs. The level is `Some(1)`
/// for H1, `Some(2)` for H2, and `None` for a lead section before any heading.
fn split_md_sections(md: &str) -> Vec<(Option<u8>, String)> {
    let mut sections: Vec<(Option<u8>, String)> = Vec::new();
    let mut cur_level: Option<u8> = None;
    let mut cur = String::new();
    let mut started = false;

    for line in md.lines() {
        let t = line.trim_start();
        let level = if t.starts_with("# ") {
            Some(1u8)
        } else if t.starts_with("## ") {
            Some(2u8)
        } else {
            None
        };

        if level.is_some() {
            if started {
                sections.push((cur_level, std::mem::take(&mut cur)));
            }
            cur_level = level;
            started = true;
        } else if !started {
            started = true;
            cur_level = None;
        }
        cur.push_str(line);
        cur.push('\n');
    }

    if started {
        sections.push((cur_level, cur));
    }
    sections
}

/// Render a parsed [`SurfDoc`] as a complete, self-contained HTML deck.
pub fn to_slides_html(doc: &SurfDoc) -> String {
    let (config, slides) = extract_deck(doc);
    render_deck_html(&config, &slides)
}

/// Render a deck config + slide list into a single standalone HTML file.
pub fn render_deck_html(config: &DeckConfig, slides: &[SlideEntry]) -> String {
    let title = config.title.as_deref().unwrap_or("SurfDoc Deck");
    let title_esc = escape_html(title);
    let footer_text = config.footer.as_deref().unwrap_or(title);
    let footer_esc = escape_html(footer_text);

    let theme_css = theme_tokens(config.theme_name());

    // Per-deck root overrides (author wins over theme default): accent + font.
    let mut root_override = String::new();
    if let Some(a) = &config.accent {
        root_override.push_str(&format!("--accent:{};", escape_html(a)));
    }
    if let Some(f) = &config.font {
        root_override.push_str(&format!("--sans:{};", escape_html(f)));
    }
    let accent_override = if root_override.is_empty() {
        String::new()
    } else {
        format!(":root{{{root_override}}}")
    };

    let show_numbers = config.show_numbers();
    let aspect = config.aspect_ratio();
    let aspect_css = aspect_to_css(aspect);
    let transition = config.transition_name();
    let transition_esc = escape_html(transition);
    let aspect_esc = escape_html(aspect);

    let mut sections = String::new();
    let total = slides.len().max(1);
    for (i, slide) in slides.iter().enumerate() {
        let active = if i == 0 { " active" } else { "" };
        let layout = slide.layout.css_class();

        let kicker_html = slide
            .kicker
            .as_ref()
            .map(|k| format!("<div class=\"kicker\">{}</div>", escape_html(k)))
            .unwrap_or_default();

        let body = render_slide_body(slide);

        let notes_html = slide
            .notes
            .as_ref()
            .map(|n| format!("<aside class=\"notes\">{}</aside>", notes_to_html(n)))
            .unwrap_or_default();

        // Footer chrome: watermark + footer text + optional slide counter.
        let counter = if show_numbers {
            format!(
                "<span class=\"page\">{} / {}</span>",
                i + 1,
                total
            )
        } else {
            String::new()
        };

        sections.push_str(&format!(
            "<section class=\"slide {layout}{active}\" data-index=\"{idx}\">{kicker}<div class=\"surfdoc slide-inner\">{body}</div>{notes}<div class=\"footer\"><span class=\"wm\"><span class=\"accent\">surf</span>://</span><span class=\"footer-text\">{footer}</span>{counter}</div></section>\n",
            layout = layout,
            active = active,
            idx = i,
            kicker = kicker_html,
            body = body,
            notes = notes_html,
            footer = footer_esc,
            counter = counter,
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en" data-slides="{total}" data-aspect="{aspect}" data-transition="{transition}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{theme_css}
{accent_override}
:root{{{aspect_css}}}
{chrome}
{surfdoc}
</style>
</head>
<body class="transition-{transition}">
<div id="bar"></div>
<div id="deck">
{sections}</div>
<div id="notes-pane" aria-hidden="true"></div>
<div class="hint">← / → · space · N notes · F fullscreen</div>
<script>
{js}
</script>
</body>
</html>"#,
        total = total,
        title = title_esc,
        aspect = aspect_esc,
        transition = transition_esc,
        theme_css = theme_css,
        accent_override = accent_override,
        aspect_css = aspect_css,
        chrome = DECK_CHROME_CSS,
        surfdoc = crate::SURFDOC_CSS,
        sections = sections,
        js = DECK_JS,
    )
}

/// Render a slide's child blocks into its inner HTML, applying layout-specific
/// structure. Most layouts let CSS (keyed off the `.slide.<layout>` class) do
/// the styling; `two`/split wraps each top-level child as a column cell.
fn render_slide_body(slide: &SlideEntry) -> String {
    let parts: Vec<String> = slide.children.iter().map(render_block).collect();
    match slide.layout {
        SlideLayout::Two => {
            // Two-column split: each top-level child becomes a grid cell.
            let cells: String = parts
                .iter()
                .map(|p| format!("<div class=\"col\">{p}</div>"))
                .collect::<Vec<_>>()
                .join("");
            format!("<div class=\"slide-cols\">{cells}</div>")
        }
        _ => parts.join("\n"),
    }
}

/// Convert presenter-notes text to safe HTML, preserving line breaks.
fn notes_to_html(notes: &str) -> String {
    escape_html(notes.trim())
        .lines()
        .collect::<Vec<_>>()
        .join("<br>")
}

/// Map an `aspect` like `16:9` / `4:3` / `16:10` to a CSS `--aspect` ratio
/// custom property value. Unknown / malformed values fall back to `16 / 9`.
fn aspect_to_css(aspect: &str) -> String {
    let ratio = match aspect.split_once(':') {
        Some((w, h)) => {
            let w = w.trim();
            let h = h.trim();
            if w.chars().all(|c| c.is_ascii_digit())
                && h.chars().all(|c| c.is_ascii_digit())
                && !w.is_empty()
                && !h.is_empty()
                && h != "0"
            {
                format!("{w} / {h}")
            } else {
                "16 / 9".to_string()
            }
        }
        None => "16 / 9".to_string(),
    };
    format!("--aspect:{ratio};")
}

/// Theme token sets (CSS custom properties). `surf-dark` is canonical.
fn theme_tokens(theme: &str) -> &'static str {
    match theme {
        "surf-light" => THEME_SURF_LIGHT,
        "mono" => THEME_MONO,
        _ => THEME_SURF_DARK,
    }
}

const THEME_SURF_DARK: &str = r#":root{
  --bg:#000000; --bg2:#0a0a0a; --soft:#161616; --ink:#fafafa; --strong:#ffffff;
  --muted:#a3a3a3; --faint:#525252; --line:#262626; --line-subtle:#1a1a1a;
  --accent:#2563eb; --accent-soft:rgba(37,99,235,.18); --accent2:#8b5cf6;
  --good:#22c55e; --warn:#f59e0b; --bad:#ef4444; --radius:2px;
  --mono:"SF Mono","Fira Code","Fira Mono",ui-monospace,Menlo,Consolas,monospace;
  --sans:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;
  --slide-bg:var(--bg);
}"#;

const THEME_SURF_LIGHT: &str = r#":root{
  --bg:#fafbfd; --bg2:#ffffff; --soft:#f1f5f9; --ink:#0a0a0a; --strong:#000000;
  --muted:#525252; --faint:#a3a3a3; --line:#e5e7eb; --line-subtle:#f1f5f9;
  --accent:#2563eb; --accent-soft:rgba(37,99,235,.12); --accent2:#8b5cf6;
  --good:#16a34a; --warn:#d97706; --bad:#dc2626; --radius:2px;
  --mono:"SF Mono","Fira Code","Fira Mono",ui-monospace,Menlo,Consolas,monospace;
  --sans:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;
  --slide-bg:var(--bg);
}"#;

const THEME_MONO: &str = r#":root{
  --bg:#0a0a0a; --bg2:#111111; --soft:#1a1a1a; --ink:#e5e5e5; --strong:#ffffff;
  --muted:#888888; --faint:#555555; --line:#2a2a2a; --line-subtle:#1a1a1a;
  --accent:#e5e5e5; --accent-soft:rgba(229,229,229,.10); --accent2:#888888;
  --good:#22c55e; --warn:#f59e0b; --bad:#ef4444; --radius:0px;
  --mono:"SF Mono","Fira Code","Fira Mono",ui-monospace,Menlo,Consolas,monospace;
  --sans:var(--mono);
  --slide-bg:var(--bg);
}"#;

/// Deck chrome: slide positioning, progress bar, kicker, footer, per-layout
/// styling, presenter-notes pane, nav hint. Aspect ratio is honored via the
/// `--aspect` custom property (set per deck), centering a fixed-ratio stage.
const DECK_CHROME_CSS: &str = r#"
*{box-sizing:border-box}
html,body{height:100%;margin:0;padding:0}
body{background:var(--bg2);color:var(--ink);font-family:var(--sans);overflow:hidden;-webkit-font-smoothing:antialiased}
/* Center a fixed-aspect stage; the deck fills the largest box at --aspect. */
#deck{position:absolute;top:50%;left:50%;transform:translate(-50%,-50%);aspect-ratio:var(--aspect,16/9);width:min(100vw,calc(100vh*(var(--aspect,16/9))));height:min(100vh,calc(100vw/(var(--aspect,16/9))));background:var(--slide-bg);overflow:hidden;box-shadow:0 0 0 1px var(--line)}
.slide{position:absolute;inset:0;display:none;flex-direction:column;justify-content:center;padding:7% 9%;opacity:0;transition:opacity .35s ease;background:var(--slide-bg);overflow:auto}
.slide.active{display:flex;opacity:1}
/* Transition variants (deterministic; default fade). */
body.transition-none .slide{transition:none}
body.transition-slide .slide{transition:opacity .35s ease, transform .35s ease}
.slide .slide-inner{max-width:100%;width:100%}
.kicker{font-family:var(--mono);font-size:.82rem;letter-spacing:.22em;text-transform:uppercase;color:var(--accent);margin-bottom:1.4rem}
.footer{position:absolute;bottom:3.2%;left:9%;right:9%;display:flex;justify-content:space-between;align-items:center;gap:1.5rem;font-family:var(--mono);font-size:.72rem;color:var(--muted);letter-spacing:.08em;z-index:6}
.footer .wm{font-weight:700;color:var(--muted);white-space:nowrap}
.footer .footer-text{flex:1;text-align:center}
.footer .page{white-space:nowrap;color:var(--faint)}
.footer .accent{color:var(--accent)}
.notes{display:none}
#bar{position:absolute;top:0;left:0;height:2px;background:linear-gradient(90deg,var(--accent),var(--accent2));transition:width .35s ease;z-index:10;width:0}
.hint{position:absolute;bottom:1.6vh;left:50%;transform:translateX(-50%);font-family:var(--mono);font-size:.7rem;letter-spacing:.1em;color:var(--muted);pointer-events:none;white-space:nowrap;z-index:5}
/* Let the embedded SurfDoc content size to the slide instead of its own page chrome. */
.slide .surfdoc{background:transparent;max-width:100%;margin:0;padding:0}
.slide .surfdoc h1{font-size:clamp(2rem,4.6vw,4rem);line-height:1.1;letter-spacing:-.02em}
.slide .surfdoc h2{font-size:clamp(1.5rem,3vw,2.4rem);line-height:1.25;letter-spacing:-.02em}
/* ---- Per-layout styling ---- */
.slide.cover,.slide.title,.slide.section,.slide.quote,.slide.stat{justify-content:center;text-align:center;align-items:center}
.slide.cover .slide-inner,.slide.title .slide-inner,.slide.section .slide-inner,.slide.quote .slide-inner{text-align:center}
.slide.cover .surfdoc h1,.slide.title .surfdoc h1{font-weight:800;font-size:clamp(2.6rem,6vw,5rem)}
.slide.section{background:var(--soft)}
.slide.section .surfdoc h1,.slide.section .surfdoc h2{font-size:clamp(2rem,5vw,3.6rem);color:var(--strong)}
.slide.section::before{content:"";position:absolute;left:9%;top:50%;width:48px;height:3px;background:var(--accent);transform:translateY(-2.4em)}
.slide.quote .surfdoc blockquote,.slide.quote .surfdoc{font-size:clamp(1.6rem,3.4vw,2.8rem);line-height:1.3;font-weight:500;border:0;font-style:italic;color:var(--strong)}
.slide.quote .surfdoc blockquote{padding:0;margin:0}
.slide.image{padding:0;justify-content:center;align-items:center}
.slide.image .slide-inner{height:100%;display:flex;align-items:center;justify-content:center}
.slide.image img{max-width:100%;max-height:88%;object-fit:contain;border-radius:var(--radius)}
.slide.code .slide-inner{display:flex;flex-direction:column;justify-content:center}
.slide.code pre,.slide.code .surfdoc pre{font-size:clamp(.85rem,1.5vw,1.15rem);line-height:1.5;max-width:100%;overflow:auto}
/* Two-column / split */
.slide.two .slide-cols{display:grid;grid-template-columns:1fr 1fr;gap:3rem;align-items:start;width:100%}
.slide.two .slide-cols .col{min-width:0}
/* Charts & diagrams embedded in a slide: keep within the stage. */
.slide .surfdoc figure{margin:0 auto}
.slide .surfdoc svg{max-width:100%;height:auto;max-height:62vh}
/* ---- Presenter notes pane (toggle with N/S) ---- */
#notes-pane{display:none;position:absolute;left:0;right:0;bottom:0;max-height:38%;overflow:auto;padding:1.2rem 9% 1.4rem;background:rgba(0,0,0,.82);color:#fafafa;font-family:var(--sans);font-size:1rem;line-height:1.5;z-index:20;border-top:2px solid var(--accent)}
#notes-pane::before{content:"NOTES";display:block;font-family:var(--mono);font-size:.65rem;letter-spacing:.22em;color:var(--accent);margin-bottom:.5rem}
body.notes-on #notes-pane{display:block}
"#;

/// Deck navigation JS: keyboard/click/touch nav, progress bar, fullscreen, and
/// a presenter-notes toggle (N or S) that mirrors the active slide's notes into
/// `#notes-pane`. Deterministic, dependency-free.
const DECK_JS: &str = r#"const slides=[...document.querySelectorAll('.slide')];
let i=0;const bar=document.getElementById('bar');const pane=document.getElementById('notes-pane');
function syncNotes(){if(!pane)return;const n=slides[i]?slides[i].querySelector('.notes'):null;pane.innerHTML=n?n.innerHTML:'<em>No notes for this slide.</em>';}
function show(n){i=Math.max(0,Math.min(slides.length-1,n));slides.forEach((s,k)=>s.classList.toggle('active',k===i));bar.style.width=(slides.length>1?(i/(slides.length-1)*100):100)+'%';syncNotes();}
function next(){show(i+1)}function prev(){show(i-1)}
function toggleNotes(){document.body.classList.toggle('notes-on');}
document.addEventListener('keydown',e=>{
  if(['ArrowRight','ArrowDown',' ','PageDown'].includes(e.key)){e.preventDefault();next()}
  else if(['ArrowLeft','ArrowUp','PageUp'].includes(e.key)){e.preventDefault();prev()}
  else if(e.key==='Home'){show(0)}else if(e.key==='End'){show(slides.length-1)}
  else if(e.key.toLowerCase()==='n'||e.key.toLowerCase()==='s'){e.preventDefault();toggleNotes()}
  else if(e.key.toLowerCase()==='f'){if(!document.fullscreenElement)document.documentElement.requestFullscreen();else document.exitFullscreen()}
});
const deck=document.getElementById('deck');
if(deck)deck.addEventListener('click',e=>{if(e.clientX<window.innerWidth*0.28)prev();else next();});
show(0);"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn explicit_deck_and_slides_extract() {
        let src = "---\ntitle: Test Deck\ntype: deck\n---\n\n::deck\ntheme: surf-dark\naspect: 16:9\n::\n\n::slide[layout=cover kicker=\"Intro\"]\n# Hello\n::\n\n::slide[layout=bullets]\n## Points\n- one\n- two\n::\n";
        let doc = parse(src).doc;
        let (config, slides) = extract_deck(&doc);
        assert_eq!(config.theme.as_deref(), Some("surf-dark"));
        assert_eq!(config.aspect.as_deref(), Some("16:9"));
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].layout, SlideLayout::Cover);
        assert_eq!(slides[0].kicker.as_deref(), Some("Intro"));
        assert_eq!(slides[1].layout, SlideLayout::Bullets);
    }

    #[test]
    fn auto_split_on_headings() {
        // No ::deck / ::slide — every # / ## becomes a slide.
        let src = "---\ntitle: Auto\n---\n# Cover Title\nlead text\n\n## First\nbody one\n\n## Second\nbody two\n";
        let doc = parse(src).doc;
        let (_config, slides) = extract_deck(&doc);
        assert_eq!(slides.len(), 3, "H1 + two H2 => 3 slides");
        assert_eq!(slides[0].layout, SlideLayout::Cover);
        assert_eq!(slides[1].layout, SlideLayout::Bullets);
    }

    #[test]
    fn renders_self_contained_html() {
        let src = "---\ntitle: Render Me\ntype: deck\n---\n\n::slide[layout=cover]\n# Big\n::\n";
        let doc = parse(src).doc;
        let html = to_slides_html(&doc);
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("class=\"slide cover active\""));
        assert!(html.contains("Render Me")); // title in footer + <title>
        assert!(html.contains("id=\"bar\"")); // progress bar chrome
        assert!(html.contains(".surfdoc")); // bundled block CSS embedded
        assert!(html.contains("data-slides=\"1\""));
    }

    #[test]
    fn render_never_panics_on_empty() {
        let doc = parse("---\ntitle: Empty\n---\n").doc;
        let html = to_slides_html(&doc);
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn theme_defaults_to_surf_dark() {
        let cfg = DeckConfig::default();
        assert_eq!(cfg.theme_name(), "surf-dark");
        assert!(theme_tokens(cfg.theme_name()).contains("#2563eb"));
    }

    // ---- Chunk 5: layouts ----

    #[test]
    fn every_layout_parses_and_renders_its_class() {
        // One slide per supported layout keyword (incl. aliases).
        let src = "\
---
type: presentation
---
::slide[layout=title]
# T
::
::slide[layout=section]
# S
::
::slide[layout=two-column]
## A
## B
::
::slide[layout=image]
![alt](x.png)
::
::slide[layout=quote]
> q
::
::slide[layout=code]
```
x
```
::
::slide[layout=default]
- a
::
";
        let doc = parse(src).doc;
        let (_cfg, slides) = extract_deck(&doc);
        assert_eq!(slides.len(), 7);
        assert_eq!(slides[0].layout, SlideLayout::Title);
        assert_eq!(slides[1].layout, SlideLayout::Section);
        assert_eq!(slides[2].layout, SlideLayout::Two);
        assert_eq!(slides[3].layout, SlideLayout::Image);
        assert_eq!(slides[4].layout, SlideLayout::Quote);
        assert_eq!(slides[5].layout, SlideLayout::Code);
        assert_eq!(slides[6].layout, SlideLayout::Bullets); // "default" → Bullets

        let html = to_slides_html(&doc);
        for cls in ["slide title", "slide section", "slide two", "slide image", "slide quote", "slide code", "slide bullets"] {
            assert!(html.contains(cls), "missing layout class: {cls}");
        }
        // Two-column wraps children in column cells.
        assert!(html.contains("slide-cols"));
        assert!(html.contains("<div class=\"col\">"));
    }

    #[test]
    fn notes_attr_and_notes_block_both_populate_notes() {
        // Attribute form.
        let attr = parse("---\ntype: deck\n---\n::slide[layout=title notes=\"hi there\"]\n# T\n::\n").doc;
        let (_c, s) = extract_deck(&attr);
        assert_eq!(s[0].notes.as_deref(), Some("hi there"));

        // `:::notes` child form (deeper colon depth nests inside the slide).
        let blk = parse("---\ntype: deck\n---\n::slide[layout=title]\n# T\n:::notes\nspeaker note\n:::\n::\n").doc;
        let (_c2, s2) = extract_deck(&blk);
        assert_eq!(s2[0].notes.as_deref(), Some("speaker note"));
        // The notes block must NOT leak into the rendered slide children.
        let html = to_slides_html(&blk);
        assert!(html.contains("<aside class=\"notes\">speaker note</aside>"));
    }

    #[test]
    fn deck_options_parse_and_render() {
        let src = "\
---
type: presentation
---
::deck
theme: surf-light
accent: #ff0000
font: Georgia, serif
aspect: 4:3
transition: slide
footer: My Footer
numbers: off
::
::slide[layout=title]
# Hi
::
";
        let doc = parse(src).doc;
        let (cfg, _slides) = extract_deck(&doc);
        assert_eq!(cfg.theme.as_deref(), Some("surf-light"));
        assert_eq!(cfg.accent.as_deref(), Some("#ff0000"));
        assert_eq!(cfg.aspect_ratio(), "4:3");
        assert_eq!(cfg.transition_name(), "slide");
        assert_eq!(cfg.footer.as_deref(), Some("My Footer"));
        assert!(!cfg.show_numbers());

        let html = to_slides_html(&doc);
        assert!(html.contains("--accent:#ff0000;"));
        assert!(html.contains("--sans:Georgia, serif;"));
        assert!(html.contains("--aspect:4 / 3;"));
        assert!(html.contains("data-transition=\"slide\""));
        assert!(html.contains("My Footer"));
        // numbers off → no slide-number counter span.
        assert!(!html.contains("class=\"page\""));
    }

    #[test]
    fn slide_numbers_default_on() {
        let doc = parse("---\ntype: deck\n---\n::slide\n# A\n::\n::slide\n# B\n::\n").doc;
        let html = to_slides_html(&doc);
        assert!(html.contains("<span class=\"page\">1 / 2</span>"));
        assert!(html.contains("<span class=\"page\">2 / 2</span>"));
    }

    #[test]
    fn aspect_defaults_to_16_9_and_rejects_garbage() {
        assert_eq!(aspect_to_css("16:9"), "--aspect:16 / 9;");
        assert_eq!(aspect_to_css("4:3"), "--aspect:4 / 3;");
        assert_eq!(aspect_to_css("nonsense"), "--aspect:16 / 9;");
        assert_eq!(aspect_to_css("3:0"), "--aspect:16 / 9;");
        assert_eq!(DeckConfig::default().aspect_ratio(), "16:9");
    }

    #[test]
    fn embedded_chart_and_diagram_render_svg_in_slide() {
        let src = "\
---
type: presentation
---
::slide[layout=bullets]
## Data
:::chart[type=line title=\"Users\"]
Week | Users
W1 | 10
W2 | 20
:::
::
::slide[layout=bullets]
## Flow
:::diagram[type=flowchart title=\"Flow\"]
a: Start
b: End
a -> b
:::
::
";
        let doc = parse(src).doc;
        let html = to_slides_html(&doc);
        // Both the chart and the diagram produce inline SVG inside their slides.
        assert!(html.matches("<svg").count() >= 2, "expected >=2 inline SVGs");
        assert!(html.contains("Users")); // chart title
        assert!(html.contains("Flow")); // diagram title
    }

    #[test]
    fn deck_html_is_deterministic() {
        let src = "\
---
type: presentation
---
::deck
theme: surf-dark
accent: #2563eb
::
::slide[layout=title kicker=\"K\"]
# Title
:::notes
note
:::
::
::slide[layout=two-column]
## Left
## Right
:::chart[type=bar title=\"C\"]
X | Y
a | 1
b | 2
:::
::
";
        let doc = parse(src).doc;
        let a = to_slides_html(&doc);
        let b = to_slides_html(&doc);
        assert_eq!(a, b, "deck HTML must be byte-identical across renders");
    }
}

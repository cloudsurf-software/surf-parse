//! PDF renderer via Typst.
//!
//! Converts a SurfDoc to Typst markup using [`render_typst`], then compiles it
//! through the Typst engine to produce PDF bytes. Pure Rust, no external
//! dependencies (no Chrome, no system fonts required).

use std::collections::HashMap;

use crate::render_typst;
use crate::types::{Block, SurfDoc};

use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_as_lib::TypstEngine;

/// Liberation Sans — a libre (SIL OFL 1.1) sans-serif metrically compatible with
/// Arial/Helvetica. Bundled so the PDF body/headings match the on-screen surfdoc
/// viewer's system sans-serif look (the TypstKit embedded set ships only serif
/// faces — Libertinus Serif, New Computer Modern — plus DejaVu Sans Mono, so a
/// proportional sans must be supplied explicitly). See `assets/fonts/LICENSE`.
static LIBERATION_SANS_REGULAR: &[u8] =
    include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");
static LIBERATION_SANS_BOLD: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Bold.ttf");
/// Inter (SIL OFL 1.1, `assets/fonts/inter/OFL.txt`) — the resume profile's
/// face (surf-parse 0.31.0): the four weights the V7 resume uses. Static
/// instances, not the variable font (Typst selects by weight from static
/// faces). The generic layout keeps Liberation Sans.
static INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/inter/Inter-Regular.otf");
static INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/inter/Inter-Medium.otf");
static INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/inter/Inter-SemiBold.otf");
static INTER_BOLD: &[u8] = include_bytes!("../assets/fonts/inter/Inter-Bold.otf");
static LIBERATION_SANS_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/LiberationSans-Italic.ttf");
static LIBERATION_SANS_BOLD_ITALIC: &[u8] =
    include_bytes!("../assets/fonts/LiberationSans-BoldItalic.ttf");

/// Paper sizes for PDF output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperSize {
    /// 8.5 x 11 inches
    Letter,
    /// 8.27 x 11.69 inches (210 x 297 mm)
    A4,
    /// 8.5 x 14 inches
    Legal,
    /// Custom width x height in inches
    Custom { width: f64, height: f64 },
}

impl PaperSize {
    /// Width in inches.
    pub fn width(&self) -> f64 {
        match self {
            Self::Letter => 8.5,
            Self::A4 => 8.27,
            Self::Legal => 8.5,
            Self::Custom { width, .. } => *width,
        }
    }

    /// Height in inches.
    pub fn height(&self) -> f64 {
        match self {
            Self::Letter => 11.0,
            Self::A4 => 11.69,
            Self::Legal => 14.0,
            Self::Custom { height, .. } => *height,
        }
    }

    /// A front-matter `paper:` value: `letter` · `us-letter` · `a4` · `legal`
    /// · `us-legal`, any case; anything else is `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "letter" | "us-letter" | "usletter" => Some(Self::Letter),
            "a4" => Some(Self::A4),
            "legal" | "us-legal" | "uslegal" => Some(Self::Legal),
            _ => None,
        }
    }

    /// Typst paper name, if standard.
    fn typst_name(&self) -> Option<&'static str> {
        match self {
            Self::Letter => Some("us-letter"),
            Self::A4 => Some("a4"),
            Self::Legal => Some("us-legal"),
            Self::Custom { .. } => None,
        }
    }
}

/// Margins for PDF output, all values in inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 1.0,
            right: 1.0,
            bottom: 1.0,
            left: 1.0,
        }
    }
}

impl Margins {
    /// All four sides the same, in inches.
    pub const fn uniform(v: f64) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }

    /// The RESUME profile's margins (the measured V7 build): 0.42 / 0.65 /
    /// 0.3 / 0.65 in.
    pub const RESUME: Self = Self { top: 0.42, right: 0.65, bottom: 0.3, left: 0.65 };

    /// A front-matter `margins:` value in CSS order — 1, 2 or 4 lengths, each
    /// `<number>in` · `<number>cm` · `<number>mm` · `<number>pt` (a bare
    /// number is inches). `"0.42in 0.65in 0.3in 0.65in"`, `"2cm"`,
    /// `"1in 0.75in"`. Anything else (3 values, a bad unit, a negative) is
    /// `None` — never a guess.
    pub fn parse(s: &str) -> Option<Self> {
        fn inches(tok: &str) -> Option<f64> {
            let t = tok.trim().to_ascii_lowercase();
            let (num, scale) = if let Some(n) = t.strip_suffix("in") {
                (n, 1.0)
            } else if let Some(n) = t.strip_suffix("cm") {
                (n, 1.0 / 2.54)
            } else if let Some(n) = t.strip_suffix("mm") {
                (n, 1.0 / 25.4)
            } else if let Some(n) = t.strip_suffix("pt") {
                (n, 1.0 / 72.0)
            } else {
                (t.as_str(), 1.0)
            };
            let v: f64 = num.trim().parse().ok()?;
            (v.is_finite() && v >= 0.0).then_some(v * scale)
        }
        let parts: Vec<f64> = s
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .map(inches)
            .collect::<Option<Vec<f64>>>()?;
        match parts.as_slice() {
            [a] => Some(Self::uniform(*a)),
            [v, h] => Some(Self { top: *v, right: *h, bottom: *v, left: *h }),
            [t, r, b, l] => Some(Self { top: *t, right: *r, bottom: *b, left: *l }),
            _ => None,
        }
    }
}

/// Configuration for PDF rendering.
#[derive(Clone)]
pub struct PdfConfig {
    /// Paper size (default: A4).
    pub paper_size: PaperSize,
    /// Page margins in inches (default: 1 inch on all sides).
    pub margins: Margins,
    /// Landscape orientation (default: false).
    pub landscape: bool,
    /// Print background graphics (default: true).
    pub print_background: bool,
    /// Page title override. Falls back to front matter, then "SurfDoc".
    pub title: Option<String>,
    /// Source path for SurfDoc metadata (default: "source.surf").
    pub source_path: Option<String>,
    /// Pre-fetched image bytes, keyed by the doc-referenced src (e.g.
    /// `/images/{id}/file`). The Typst engine has no filesystem or network
    /// access, so this map is the ONLY way a figure/gallery/hero image renders
    /// for real: callers resolve each src from their own store (see
    /// [`collect_image_srcs`] for the srcs a doc references) and pass the raw
    /// bytes here. Srcs absent from the map — or whose bytes don't sniff as a
    /// supported format (PNG/JPEG/GIF/WebP/SVG) — degrade to a deterministic
    /// placeholder; they never fail the render (default: empty).
    pub images: HashMap<String, Vec<u8>>,
    /// A small line at the bottom-right of EVERY page's footer, including the
    /// first (surf-parse 0.34.0): the words, 8 pt in the page number's grey,
    /// as a link to <https://cloudsurf.com>. The caller decides the words and
    /// whether there are any — the engine never hears the word "plan"; a
    /// document cannot opt out by front matter ([`PdfConfig::for_doc`] passes
    /// it through untouched). `None` (the default) draws only the centred
    /// page counter.
    pub brand: Option<String>,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::A4,
            margins: Margins::default(),
            landscape: false,
            print_background: true,
            title: None,
            source_path: None,
            images: HashMap::new(),
            brand: None,
        }
    }
}

impl PdfConfig {
    /// The config a DOCUMENT asks for (surf-parse 0.31.0, the page profiles):
    /// start from `base` (the caller's route defaults — Letter, the title, the
    /// images), let the PRINT PROFILE set its paper and margins (the resume =
    /// Letter, [`Margins::RESUME`]), then let the doc's own front matter win
    /// (`paper:` through [`PaperSize::parse`], `margins:` through
    /// [`Margins::parse`]; an unparsable value is ignored, never an error).
    /// Every route that renders a doc should build its config here, so the
    /// preview and the export agree with the document.
    pub fn for_doc(doc: &SurfDoc, base: PdfConfig) -> PdfConfig {
        let mut cfg = base;
        let fm = doc.front_matter.as_ref();
        if render_typst::is_resume(doc) {
            cfg.paper_size = PaperSize::Letter;
            cfg.margins = Margins::RESUME;
        }
        if let Some(p) = fm.and_then(|f| f.paper()).and_then(|p| PaperSize::parse(&p)) {
            cfg.paper_size = p;
        }
        if let Some(m) = fm.and_then(|f| f.margins()).and_then(|m| Margins::parse(&m)) {
            cfg.margins = m;
        }
        cfg
    }
}

// Manual Debug: `images` prints as a src → byte-count summary, never the raw
// bytes (a resolved map can hold megabytes).
impl std::fmt::Debug for PdfConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PdfConfig")
            .field("paper_size", &self.paper_size)
            .field("margins", &self.margins)
            .field("landscape", &self.landscape)
            .field("print_background", &self.print_background)
            .field("title", &self.title)
            .field("source_path", &self.source_path)
            .field("brand", &self.brand)
            .field(
                "images",
                &self
                    .images
                    .iter()
                    .map(|(src, bytes)| format!("{src} ({} bytes)", bytes.len()))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Errors that can occur during PDF generation.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// Failed to compile Typst markup.
    #[error("Typst compilation failed: {0}")]
    Compilation(String),

    /// Failed to render PDF from compiled document.
    #[error("PDF rendering failed: {0}")]
    PdfRendering(String),
}

/// Render a `SurfDoc` to PDF bytes using the Typst engine.
///
/// This is a synchronous, pure-Rust operation. No Chrome, no external tools.
///
/// Images: srcs present in [`PdfConfig::images`] whose bytes sniff as a
/// supported format are registered as virtual files with the engine and render
/// for real; every other referenced src degrades to a placeholder. If a
/// registered image still fails compilation (e.g. corrupt bytes past the
/// magic-byte sniff), the render retries once with every image degraded to a
/// placeholder — a bad image never fails the whole export.
///
/// # Errors
///
/// Returns [`PdfError`] if Typst compilation or PDF rendering fails.
pub fn to_pdf(doc: &SurfDoc, config: &PdfConfig) -> Result<Vec<u8>, PdfError> {
    let compiled = compile(doc, config)?;
    let pdf_options = typst_pdf::PdfOptions::default();
    typst_pdf::pdf(&compiled, &pdf_options).map_err(|e| PdfError::PdfRendering(format!("{e:?}")))
}

/// Render a `SurfDoc` to one SVG per page — the SAME compile [`to_pdf`] runs
/// (surf-parse 0.31.0, the page stacks): a preview built from these pages is
/// byte-for-byte the document the PDF carries. Each string is a complete
/// `<svg …>` with the page's `viewBox` in points (Letter = `0 0 612 792`).
///
/// # Errors
///
/// Returns [`PdfError`] if Typst compilation fails.
pub fn to_pages(doc: &SurfDoc, config: &PdfConfig) -> Result<Vec<String>, PdfError> {
    let compiled = compile(doc, config)?;
    Ok(compiled.pages.iter().map(typst_svg::svg).collect())
}

/// The PDF bytes AND the page SVGs from ONE compile — what a server that
/// serves both (the download and the page stack) caches (surf-parse 0.31.0).
///
/// # Errors
///
/// Returns [`PdfError`] if Typst compilation or the PDF export fails.
pub fn to_pdf_and_pages(doc: &SurfDoc, config: &PdfConfig) -> Result<(Vec<u8>, Vec<String>), PdfError> {
    let compiled = compile(doc, config)?;
    let pdf_options = typst_pdf::PdfOptions::default();
    let pdf = typst_pdf::pdf(&compiled, &pdf_options).map_err(|e| PdfError::PdfRendering(format!("{e:?}")))?;
    let pages = compiled.pages.iter().map(typst_svg::svg).collect();
    Ok((pdf, pages))
}

/// The page count a document lays out to — one compile, no export.
///
/// # Errors
///
/// Returns [`PdfError`] if Typst compilation fails.
pub fn page_count(doc: &SurfDoc, config: &PdfConfig) -> Result<usize, PdfError> {
    Ok(compile(doc, config)?.pages.len())
}

/// The one compile behind [`to_pdf`], [`to_pages`] and [`page_count`]:
/// images mapped to virtual files, the markup rendered, the engine built, the
/// degrade-don't-die retry.
fn compile(doc: &SurfDoc, config: &PdfConfig) -> Result<typst::layout::PagedDocument, PdfError> {
    // Map resolvable images to virtual engine files. Deterministic: sorted by
    // src, so the same doc + map always yields the same virtual paths.
    let mut srcs: Vec<&String> = config.images.keys().collect();
    srcs.sort();
    let mut virtual_map: HashMap<String, String> = HashMap::new();
    let mut binaries: Vec<(String, Vec<u8>)> = Vec::new();
    for src in srcs {
        let bytes = &config.images[src];
        // Bytes that don't sniff as a Typst-supported format stay out of the
        // map — the renderer degrades that src to a placeholder.
        let Some(ext) = sniff_image_ext(bytes) else {
            continue;
        };
        let vpath = format!("/surf-image-{}.{ext}", virtual_map.len());
        virtual_map.insert(src.clone(), vpath.clone());
        binaries.push((vpath, bytes.clone()));
    }

    match compile_once(doc, config, &virtual_map, &binaries) {
        Ok(compiled) => Ok(compiled),
        // Degrade-don't-die: if compilation failed WITH images registered,
        // retry once with all images as placeholders before giving up.
        Err(PdfError::Compilation(first)) if !virtual_map.is_empty() => {
            compile_once(doc, config, &HashMap::new(), &[]).map_err(|_| PdfError::Compilation(first))
        }
        Err(e) => Err(e),
    }
}

/// One compile pass: render Typst markup under the given ambient image map,
/// register `binaries` with the engine, compile to a paged document.
fn compile_once(
    doc: &SurfDoc,
    config: &PdfConfig,
    virtual_map: &HashMap<String, String>,
    binaries: &[(String, Vec<u8>)],
) -> Result<typst::layout::PagedDocument, PdfError> {
    // Generate Typst markup from the SurfDoc block tree, with the src →
    // virtual-path map ambient so image emissions resolve (CiteScope pattern).
    let markup = {
        let _images = render_typst::install_image_context(virtual_map.clone());
        render_typst::to_typst(doc)
    };
    let typst_source = assemble_source(markup, config);

    // Build the Typst engine with our source and embedded fonts.
    // Without fonts, Typst renders boxes/lines but no text. The TypstKit embedded
    // set provides Libertinus Serif and DejaVu Sans Mono (used for code); we add
    // Liberation Sans explicitly so the body/headings render in a clean sans-serif
    // that mirrors the on-screen surfdoc viewer.
    let engine = TypstEngine::builder()
        .main_file(typst_source)
        .fonts([
            LIBERATION_SANS_REGULAR,
            LIBERATION_SANS_BOLD,
            LIBERATION_SANS_ITALIC,
            LIBERATION_SANS_BOLD_ITALIC,
            INTER_REGULAR,
            INTER_MEDIUM,
            INTER_SEMIBOLD,
            INTER_BOLD,
        ])
        .search_fonts_with(
            TypstKitFontOptions::new().include_system_fonts(false),
        )
        .with_static_file_resolver(
            binaries
                .iter()
                .map(|(path, bytes)| (path.as_str(), bytes.clone())),
        )
        .build();

    // Compile to a paged document
    let result = engine.compile::<typst::layout::PagedDocument>();

    result
        .output
        .map_err(|e| PdfError::Compilation(format!("{e:?}")))
}

/// Magic-byte sniff → the virtual-file extension Typst needs to pick a
/// decoder. `None` (unrecognized bytes) keeps the src unresolved so it
/// degrades to a placeholder instead of failing the compile.
fn sniff_image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(b"\xFF\xD8\xFF") {
        Some("jpg")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = bytes.get(..512).unwrap_or(bytes);
        let text = String::from_utf8_lossy(head);
        let trimmed = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
        (trimmed.starts_with("<svg") || trimmed.starts_with("<?xml")).then_some("svg")
    }
}

/// Every image src the doc references: `::figure`, `::gallery` items,
/// `::hero-image`, and `::logo`, recursing into container blocks. Deduplicated
/// in first-appearance order. This is the fetch list a caller resolves against
/// its own store to build [`PdfConfig::images`].
pub fn collect_image_srcs(doc: &SurfDoc) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    fn push(src: &str, out: &mut Vec<String>, seen: &mut std::collections::HashSet<String>) {
        if !src.is_empty() && seen.insert(src.to_string()) {
            out.push(src.to_string());
        }
    }
    fn walk(
        blocks: &[Block],
        out: &mut Vec<String>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        for b in blocks {
            match b {
                Block::Figure { src, .. }
                | Block::HeroImage { src, .. }
                | Block::Logo { src, .. } => push(src, out, seen),
                // A video prints as its poster: the stated one, or a
                // `media:` source's processed poster (`media:<id>/poster`).
                Block::Video { src, poster, .. } => {
                    if let Some(p) = crate::media::effective_poster(src, poster.as_deref()) {
                        push(&p, out, seen);
                    }
                }
                Block::Gallery { items, .. } => {
                    for item in items {
                        push(&item.src, out, seen);
                    }
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
                | Block::When { children, .. } => walk(children, out, seen),
                Block::Flow { steps, .. } => {
                    for step in steps {
                        walk(&step.children, out, seen);
                    }
                }
                _ => {}
            }
        }
    }
    walk(&doc.blocks, &mut out, &mut seen);
    out
}

/// The exact Typst source [`to_pdf`], [`to_pages`] and [`page_count`] compile
/// for `doc` under `config` (surf-parse 0.34.0): the page geometry, the page
/// furniture, then the profile template + the document's markup (images
/// unresolved — the same text the compile sees when no image bytes are
/// given). The seam the profile tests read the running footer through: the
/// page SVGs draw glyphs as paths, so the words are not greppable there.
pub fn typst_source(doc: &SurfDoc, config: &PdfConfig) -> String {
    let markup = {
        let _images = render_typst::install_image_context(HashMap::new());
        render_typst::to_typst(doc)
    };
    assemble_source(markup, config)
}

/// Geometry, furniture, then the markup. Both `#set page` rules come BEFORE
/// the template + markup: a page set rule that follows content starts a new
/// page in Typst, so "after the markup" is not a place a footer can live. The
/// profile templates set `margin:` only (surfdoc.typ stopped setting a header
/// and a footer in 0.34.0) and a set rule merges per property, so the
/// furniture survives them.
fn assemble_source(markup: String, config: &PdfConfig) -> String {
    let mut out = build_config_overrides(config);
    out.push_str(&build_page_furniture(config));
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&markup);
    out
}

/// The running footer every profile carries (surf-parse 0.34.0, the "SurfDoc"
/// running head retired): the page counter centred in 8 pt `luma(150)`, and
/// with [`PdfConfig::brand`] the words bottom-right on EVERY page as a link to
/// <https://cloudsurf.com>. The grid's outer columns are equal `1fr`, so the
/// counter stays centred whatever the words weigh.
fn build_page_furniture(config: &PdfConfig) -> String {
    let counter = "#counter(page).display(\"1 / 1\", both: true)";
    match &config.brand {
        Some(words) if !words.trim().is_empty() => format!(
            "#set page(footer: context [\n  #set text(size: 8pt, fill: luma(150))\n  #grid(columns: (1fr, auto, 1fr), align: (left, center, right), [], [{counter}], [#link(\"https://cloudsurf.com\")[#{}]])\n])\n",
            typst_string(words.trim())
        ),
        _ => format!(
            "#set page(footer: context [\n  #set text(size: 8pt, fill: luma(150))\n  #h(1fr) {counter} #h(1fr)\n])\n"
        ),
    }
}

/// A Typst string literal: backslashes and quotes escaped, nothing else
/// interpreted (the words are the caller's, shown verbatim).
fn typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Build Typst `#set page(...)` overrides from PdfConfig.
fn build_config_overrides(config: &PdfConfig) -> String {
    let mut parts = Vec::new();

    // Paper size
    if let Some(name) = config.paper_size.typst_name() {
        parts.push(format!("paper: \"{}\"", name));
    } else {
        let w = config.paper_size.width();
        let h = config.paper_size.height();
        parts.push(format!("width: {w}in, height: {h}in"));
    }

    // Margins
    let m = &config.margins;
    parts.push(format!(
        "margin: (top: {}in, right: {}in, bottom: {}in, left: {}in)",
        m.top, m.right, m.bottom, m.left
    ));

    // Landscape (swap width/height via flipped)
    if config.landscape {
        parts.push("flipped: true".to_string());
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("#set page({})\n", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete, decodable 1×1 RGB PNG (IHDR + IDAT + IEND) — the image
    /// tests compile it through the real Typst decoder, so a header-only
    /// fixture is not enough.
    const PNG_1X1_RED: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
        0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
        0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78,
        0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE, 0x92,
        0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn margins_parse_css_order_and_units() {
        let m = Margins::parse("0.42in 0.65in 0.3in 0.65in").expect("four");
        assert_eq!((m.top, m.right, m.bottom, m.left), (0.42, 0.65, 0.3, 0.65));
        let m = Margins::parse("1in 0.75in").expect("two");
        assert_eq!((m.top, m.right, m.bottom, m.left), (1.0, 0.75, 1.0, 0.75));
        let m = Margins::parse("2.54cm").expect("one, cm");
        assert!((m.top - 1.0).abs() < 1e-9 && (m.left - 1.0).abs() < 1e-9);
        let m = Margins::parse("72pt, 25.4mm").expect("pt + mm, comma");
        assert!((m.top - 1.0).abs() < 1e-9 && (m.right - 1.0).abs() < 1e-9);
        assert!(Margins::parse("1in 2in 3in").is_none(), "three values are not CSS");
        assert!(Margins::parse("-1in").is_none(), "negative");
        assert!(Margins::parse("wide").is_none(), "words");
        assert!(Margins::parse("").is_none(), "blank");
    }

    #[test]
    fn paper_parse_names() {
        assert_eq!(PaperSize::parse("Letter"), Some(PaperSize::Letter));
        assert_eq!(PaperSize::parse("us-letter"), Some(PaperSize::Letter));
        assert_eq!(PaperSize::parse("A4"), Some(PaperSize::A4));
        assert_eq!(PaperSize::parse("legal"), Some(PaperSize::Legal));
        assert_eq!(PaperSize::parse("tabloid"), None);
    }

    #[test]
    fn for_doc_profile_then_front_matter_over_the_route() {
        let route = PdfConfig { paper_size: PaperSize::Letter, title: Some("t".into()), ..Default::default() };
        // A plain doc keeps the route's paper and margins.
        let plain = crate::parse("---\ntitle: x\n---\n\n# x\n").doc;
        let c = PdfConfig::for_doc(&plain, route.clone());
        assert_eq!(c.paper_size, PaperSize::Letter);
        assert_eq!(c.margins, Margins::uniform(1.0));
        assert_eq!(c.title.as_deref(), Some("t"), "the route's title survives");
        // The resume template implies the resume profile: Letter + the V7 margins.
        let resume = crate::parse("---\ntitle: x\ntemplate: resume/v1-classic\n---\n\n# x\n").doc;
        let c = PdfConfig::for_doc(&resume, PdfConfig::default());
        assert_eq!(c.paper_size, PaperSize::Letter);
        assert_eq!(c.margins, Margins::RESUME);
        // The doc's own front matter wins over the profile.
        let a4 = crate::parse("---\ntitle: x\nprofile: resume\npaper: a4\nmargins: \"2cm\"\n---\n\n# x\n").doc;
        let c = PdfConfig::for_doc(&a4, route.clone());
        assert_eq!(c.paper_size, PaperSize::A4);
        assert!((c.margins.top - 2.0 / 2.54).abs() < 1e-9);
        // An unparsable value is ignored, never an error.
        let bad = crate::parse("---\ntitle: x\npaper: tabloid\nmargins: wide\n---\n\n# x\n").doc;
        let c = PdfConfig::for_doc(&bad, route);
        assert_eq!(c.paper_size, PaperSize::Letter);
        assert_eq!(c.margins, Margins::uniform(1.0));
    }

    #[test]
    fn pdf_config_defaults_are_sensible() {
        let config = PdfConfig::default();
        assert_eq!(config.paper_size, PaperSize::A4);
        assert!((config.margins.top - 1.0).abs() < f64::EPSILON);
        assert!((config.margins.right - 1.0).abs() < f64::EPSILON);
        assert!((config.margins.bottom - 1.0).abs() < f64::EPSILON);
        assert!((config.margins.left - 1.0).abs() < f64::EPSILON);
        assert!(!config.landscape);
        assert!(config.print_background);
        assert!(config.title.is_none());
        assert!(config.source_path.is_none());
        assert!(config.images.is_empty());
    }

    #[test]
    fn config_debug_summarizes_image_bytes() {
        let mut config = PdfConfig::default();
        config
            .images
            .insert("/images/abc/file".to_string(), vec![0u8; 1234]);
        let dbg = format!("{config:?}");
        assert!(dbg.contains("/images/abc/file (1234 bytes)"), "{dbg}");
        assert!(!dbg.contains("[0, 0"), "raw bytes must not be printed: {dbg}");
    }

    #[test]
    fn sniff_recognizes_supported_formats_and_rejects_garbage() {
        assert_eq!(sniff_image_ext(PNG_1X1_RED), Some("png"));
        assert_eq!(sniff_image_ext(b"\xFF\xD8\xFF\xE0..."), Some("jpg"));
        assert_eq!(sniff_image_ext(b"GIF89a..."), Some("gif"));
        assert_eq!(
            sniff_image_ext(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some("webp")
        );
        assert_eq!(sniff_image_ext(b"<svg xmlns=\"x\"></svg>"), Some("svg"));
        assert_eq!(sniff_image_ext(b"  <?xml version=\"1.0\"?><svg/>"), Some("svg"));
        assert_eq!(sniff_image_ext(b"not an image"), None);
        assert_eq!(sniff_image_ext(b""), None);
    }

    #[test]
    fn collect_image_srcs_walks_figures_galleries_and_containers() {
        let source = "\
::figure[src=\"/images/aaa/file\" alt=\"Page 1 image 0\"]\n\n\
::figure[src=\"/images/aaa/file\" alt=\"duplicate\"]\n\n\
::hero-image[src=\"/images/bbb/file\"]\n\n\
::logo[src=\"/images/ccc/file\"]\n";
        let result = crate::parse(source);
        let srcs = collect_image_srcs(&result.doc);
        assert_eq!(
            srcs,
            vec![
                "/images/aaa/file".to_string(),
                "/images/bbb/file".to_string(),
                "/images/ccc/file".to_string(),
            ],
            "dedup in first-appearance order"
        );
    }

    /// The bug shape behind the export 500: a doc whose body carries a
    /// PDF-import figure (`/images/{id}/file`) must render WITHOUT any
    /// resolver — the figure degrades to a placeholder, never a failed
    /// `image()` compile.
    #[test]
    fn pdf_renders_with_unresolvable_figure() {
        let source = "# Scanned doc\n\n::callout[type=note]\nPage 1 has no text layer (scanned or image-only); its text was not extracted.\n::\n\n::figure[src=\"/images/0b54ff0e-1cbd-4d5c-a2e5-fca9ad34fd52/file\" alt=\"Page 1 image 0\"]\n";
        let result = crate::parse(source);
        let pdf_bytes =
            to_pdf(&result.doc, &PdfConfig::default()).expect("unresolved figure must not fail");
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    /// Resolved bytes render for real: the same doc with the src supplied in
    /// `PdfConfig::images` also produces valid PDF bytes (and exercises the
    /// virtual-file + static-resolver path end to end).
    #[test]
    fn pdf_renders_with_resolved_figure_bytes() {
        let src = "/images/0b54ff0e-1cbd-4d5c-a2e5-fca9ad34fd52/file";
        let source = format!("# Scanned doc\n\n::figure[src=\"{src}\" alt=\"Page 1 image 0\"]\n");
        let result = crate::parse(&source);
        let mut config = PdfConfig::default();
        config.images.insert(src.to_string(), PNG_1X1_RED.to_vec());
        let pdf_bytes = to_pdf(&result.doc, &config).expect("resolved figure renders");
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    /// Corrupt bytes that pass the magic-byte sniff (a truncated PNG) trip the
    /// degrade-don't-die retry: the render still succeeds with a placeholder.
    #[test]
    fn pdf_retries_with_placeholders_when_image_bytes_are_corrupt() {
        let src = "/images/corrupt/file";
        let source = format!("# Doc\n\n::figure[src=\"{src}\" alt=\"broken\"]\n");
        let result = crate::parse(&source);
        let mut config = PdfConfig::default();
        // Valid PNG magic, garbage payload — sniffs as png, fails to decode.
        config
            .images
            .insert(src.to_string(), b"\x89PNG\r\n\x1a\ngarbage".to_vec());
        let pdf_bytes = to_pdf(&result.doc, &config).expect("corrupt image degrades, never fails");
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    /// The generated markup itself: no ambient context → no `image(` call for
    /// the figure src; installed context → the virtual path appears.
    #[test]
    fn typst_markup_never_references_an_unresolved_src() {
        let src = "/images/xyz/file";
        let source = format!("::figure[src=\"{src}\" alt=\"Page 1 image 0\"]\n");
        let result = crate::parse(&source);

        let markup = crate::render_typst::to_typst(&result.doc);
        assert!(
            !markup.contains("image("),
            "unresolved figure must not emit image(): {markup}"
        );

        let mut map = HashMap::new();
        map.insert(src.to_string(), "/surf-image-0.png".to_string());
        let markup = {
            let _scope = crate::render_typst::install_image_context(map);
            crate::render_typst::to_typst(&result.doc)
        };
        assert!(
            markup.contains("image(\"/surf-image-0.png\")"),
            "resolved figure must emit the virtual path: {markup}"
        );
        assert!(!markup.contains(src), "raw src must never leak: {markup}");
    }

    #[test]
    fn paper_size_dimensions() {
        assert!((PaperSize::Letter.width() - 8.5).abs() < f64::EPSILON);
        assert!((PaperSize::Letter.height() - 11.0).abs() < f64::EPSILON);

        assert!((PaperSize::A4.width() - 8.27).abs() < f64::EPSILON);
        assert!((PaperSize::A4.height() - 11.69).abs() < f64::EPSILON);

        assert!((PaperSize::Legal.width() - 8.5).abs() < f64::EPSILON);
        assert!((PaperSize::Legal.height() - 14.0).abs() < f64::EPSILON);

        let custom = PaperSize::Custom {
            width: 5.0,
            height: 7.0,
        };
        assert!((custom.width() - 5.0).abs() < f64::EPSILON);
        assert!((custom.height() - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn paper_size_typst_names() {
        assert_eq!(PaperSize::A4.typst_name(), Some("a4"));
        assert_eq!(PaperSize::Letter.typst_name(), Some("us-letter"));
        assert_eq!(PaperSize::Legal.typst_name(), Some("us-legal"));
        assert_eq!(
            PaperSize::Custom { width: 5.0, height: 7.0 }.typst_name(),
            None
        );
    }

    #[test]
    fn config_overrides_default() {
        let config = PdfConfig::default();
        let overrides = build_config_overrides(&config);
        assert!(overrides.contains("a4"));
        assert!(overrides.contains("1in"));
    }

    #[test]
    fn config_overrides_landscape() {
        let config = PdfConfig {
            landscape: true,
            ..PdfConfig::default()
        };
        let overrides = build_config_overrides(&config);
        assert!(overrides.contains("flipped: true"));
    }

    #[test]
    fn config_overrides_custom_size() {
        let config = PdfConfig {
            paper_size: PaperSize::Custom { width: 5.0, height: 7.0 },
            ..PdfConfig::default()
        };
        let overrides = build_config_overrides(&config);
        assert!(overrides.contains("width: 5in"));
        assert!(overrides.contains("height: 7in"));
    }

    #[test]
    fn pdf_error_display() {
        let err = PdfError::Compilation("syntax error".to_string());
        assert_eq!(err.to_string(), "Typst compilation failed: syntax error");

        let err = PdfError::PdfRendering("out of memory".to_string());
        assert_eq!(err.to_string(), "PDF rendering failed: out of memory");
    }

    /// Integration test — produces actual PDF bytes.
    /// Run with: cargo test --features pdf -- --ignored pdf_produces_valid_bytes
    #[test]
    #[ignore]
    fn pdf_produces_valid_bytes() {
        let source = "# Hello World\n\nThis is a test document.\n";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty());

        let config = PdfConfig::default();
        let pdf_bytes = to_pdf(&result.doc, &config).expect("PDF generation should succeed");

        // PDF files start with %PDF magic bytes
        assert!(
            pdf_bytes.len() > 4,
            "PDF should have content, got {} bytes",
            pdf_bytes.len()
        );
        assert_eq!(
            &pdf_bytes[..5],
            b"%PDF-",
            "PDF should start with %PDF- magic bytes"
        );
    }

    #[test]
    #[ignore]
    fn pdf_landscape_mode() {
        let source = "# Landscape Test\n\nWide content.\n";
        let result = crate::parse(source);

        let config = PdfConfig {
            paper_size: PaperSize::Letter,
            landscape: true,
            ..PdfConfig::default()
        };

        let pdf_bytes = to_pdf(&result.doc, &config).expect("PDF should generate");
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }

    #[test]
    #[ignore]
    fn pdf_with_callout_and_table() {
        let source = r#"---
title: Test Document
author: Brady
---

# Test Document

::callout[type=info]
This is an informational callout.
::

::data
| Name | Value |
|------|-------|
| Alpha | 100 |
| Beta  | 200 |
::
"#;
        let result = crate::parse(source);
        let config = PdfConfig::default();
        let pdf_bytes = to_pdf(&result.doc, &config).expect("PDF with blocks should generate");
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        assert!(pdf_bytes.len() > 1000, "PDF with content should be substantial");
    }
}

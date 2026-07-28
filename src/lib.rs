//! `surf-parse` — parser for the SurfDoc format.
//!
//! SurfDoc is a typed document format with block directives, embedded data,
//! and presentation hints. Backward-compatible with Markdown. This crate
//! provides the foundational parser that turns `.surf` source text into a
//! structured `SurfDoc` tree.
//!
//! # Quick start
//!
//! ```
//! let result = surf_parse::parse("# Hello\n\n::callout[type=info]\nHi!\n::\n");
//! assert!(result.diagnostics.is_empty());
//! assert_eq!(result.doc.blocks.len(), 2);
//! ```

pub mod attrs;
pub mod blocks;
pub mod builder;
pub(crate) mod chart;
pub mod citation;
pub(crate) mod diagram;
pub mod error;
pub mod icons;
pub mod inline;
pub mod lint;
pub mod parse;
pub mod render_html;
pub mod render_latex;
pub mod render_md;
pub mod resolve;
pub mod slots;
#[cfg(feature = "pdf")]
pub mod render_pdf;
pub mod render_typst;
#[cfg(feature = "slides")]
pub mod render_slides;
#[cfg(feature = "terminal")]
pub mod render_term;
#[cfg(feature = "native")]
pub mod render_native;
#[cfg(feature = "uniffi")]
pub mod ffi;
// UniFFI scaffolding must be generated at the crate root (it defines the
// crate-wide `UniFfiTag` used by every exported type and function).
#[cfg(feature = "uniffi")]
uniffi::setup_scaffolding!();
#[cfg(feature = "wasm")]
pub mod wasm;
pub mod template;
pub mod types;
pub mod validate;

/// Unified CSS for app chrome and SurfDoc content rendering.
///
/// Contains theme variables, reset, navigation, buttons, cards, forms,
/// and all 29 SurfDoc block type styles. Dark-first with light mode support
/// via `data-theme="light"` or `prefers-color-scheme`.
///
/// Override `:root` variables (`--accent`, `--background`, `--surface`,
/// `--font-heading`, `--font-body`) for site-level theming.
pub const SURFDOC_CSS: &str = include_str!("../assets/surfdoc.css");

pub use blocks::{parse_schema_field_type, parse_schema_constraint};
pub use builder::SurfDocBuilder;
pub use citation::{
    active_style, bibliography_heading, build_context, format_in_text, format_reference,
    parse_author, parse_authors, reference_list, reference_list_keyed, Author, CiteContext,
    CiteItem, CiteRef, Reference, RefType,
};
pub use error::*;
pub use lint::{
    AppliedFix, CheckReport, FixOutcome, LintConfig, LintRule, SkippedFix, apply_fixes,
    apply_fixes_once, check, check_with,
};
pub use parse::parse;
pub use template::TemplateContext;
pub use types::*;

pub use render_html::{
    PageConfig, SiteConfig, PageEntry, extract_site, humanize_route, render_site_page,
    render_site_single_file,
    accent_ink_color, contrast_ratio, to_shell_page, HeadIcon, HeadScript,
};
pub use slots::{resolve_slot_markers, IMG_SLOT_PLACEHOLDER_URI};
#[cfg(feature = "slides")]
pub use render_slides::{DeckConfig, SlideEntry, extract_deck, render_deck_html};

#[cfg(feature = "pdf")]
pub use render_pdf::{collect_image_srcs, PdfConfig, PdfError};

impl SurfDoc {
    /// Render this document as standard CommonMark markdown (no `::` markers).
    pub fn to_markdown(&self) -> String {
        render_md::to_markdown(self)
    }

    /// Render this document as an HTML fragment with `surfdoc-*` CSS classes.
    pub fn to_html(&self) -> String {
        render_html::to_html(self)
    }

    /// Render this document's blocks as bare HTML without page chrome.
    ///
    /// Unlike [`to_html()`], this does not scan for `::site`/`::style` overrides,
    /// extract nav blocks, or apply auto-section wrapping. Each block is rendered
    /// individually and joined with newlines.
    ///
    /// Use this for streaming, chat rendering, or embedding individual blocks
    /// where the caller controls the CSS context.
    pub fn to_html_fragment(&self) -> String {
        render_html::to_html_fragment(&self.blocks)
    }

    /// Render this document's blocks as bare HTML with template variable interpolation.
    pub fn to_html_fragment_with_context(&self, ctx: &TemplateContext) -> String {
        let html = self.to_html_fragment();
        ctx.resolve(&html)
    }

    /// Render this document as a complete HTML page with SurfDoc discovery metadata.
    pub fn to_html_page(&self, config: &PageConfig) -> String {
        render_html::to_html_page(self, config)
    }

    /// Render this document as an HTML fragment with template variable interpolation.
    pub fn to_html_with_context(&self, ctx: &TemplateContext) -> String {
        let html = self.to_html();
        ctx.resolve(&html)
    }

    /// Render this document as a complete HTML page with template variable interpolation.
    pub fn to_html_page_with_context(&self, config: &PageConfig, ctx: &TemplateContext) -> String {
        let html = self.to_html_page(config);
        ctx.resolve(&html)
    }

    /// Render this document to PDF bytes using the Typst engine.
    ///
    /// This is a synchronous, pure-Rust operation — no Chrome or external
    /// tools required. Requires the `pdf` feature.
    #[cfg(feature = "pdf")]
    pub fn to_pdf(
        &self,
        config: &render_pdf::PdfConfig,
    ) -> Result<Vec<u8>, render_pdf::PdfError> {
        render_pdf::to_pdf(self, config)
    }

    /// Render this document as Typst markup text.
    ///
    /// The output is a valid `.typ` file that can be compiled by Typst.
    /// `paper`/`report` documents use academic templates (IEEE/ACM/article and
    /// MLA/APA/Chicago); all other documents use the generic SurfDoc layout.
    pub fn to_typst(&self) -> String {
        render_typst::to_typst(self)
    }

    /// Render this document as a complete LaTeX (`.tex`) document string.
    ///
    /// The output target is selected by the document's [`RenderProfile`]:
    /// `paper` → `IEEEtran`/two-column `article`/`article`; `report` →
    /// `article` configured for MLA/APA/Chicago; everything else → a plain
    /// `article`. Citations and the reference list are produced by the citation
    /// engine in the active style. The `.tex` is deterministic and compilable.
    pub fn to_latex(&self) -> String {
        render_latex::to_latex(self)
    }

    /// Render this document as ANSI-colored terminal text.
    #[cfg(feature = "terminal")]
    pub fn to_terminal(&self) -> String {
        render_term::to_terminal(self)
    }

    /// Convert this document into a list of native blocks for mobile rendering.
    #[cfg(feature = "native")]
    pub fn to_native_blocks(&self) -> Vec<render_native::NativeBlock> {
        render_native::to_native_blocks(self)
    }

    /// The [`RenderProfile`] for this document, resolved from its front-matter
    /// `type:` and `format:` fields (see [`render_profile`]).
    pub fn render_profile(&self) -> RenderProfile {
        let (doc_type, format) = self
            .front_matter
            .as_ref()
            .map(|fm| (fm.doc_type, fm.format))
            .unwrap_or((None, None));
        render_profile(doc_type, format)
    }

    /// Whether this document should render as a presentation deck — true for
    /// `type: deck`, `type: slides`, and `type: presentation`. Callers that
    /// auto-dispatch output should route these documents to [`Self::to_slides_html`].
    pub fn is_presentation(&self) -> bool {
        matches!(self.render_profile(), RenderProfile::Presentation)
    }

    /// Render this document as a complete, self-contained HTML presentation deck.
    ///
    /// Slides are first-class the same way Website is: a `::deck`/`::slide`
    /// block family rendered via a dedicated renderer. If the document has no
    /// explicit deck/slides, every `#`/`##` heading becomes a slide
    /// (Presentation Mode). `type: presentation` (alongside `deck`/`slides`) is
    /// recognized as this slides path via [`Self::is_presentation`]. Requires
    /// the `slides` feature.
    #[cfg(feature = "slides")]
    pub fn to_slides_html(&self) -> String {
        render_slides::to_slides_html(self)
    }

    /// Serialize this document back to valid `.surf` format text.
    ///
    /// The output can be parsed again with [`parse`] to produce an equivalent
    /// document (round-trip).
    pub fn to_surf_source(&self) -> String {
        builder::to_surf_source(self)
    }

    /// Validate this document and return any diagnostics.
    pub fn validate(&self) -> Vec<crate::error::Diagnostic> {
        validate::validate(self)
    }
}

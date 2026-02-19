//! `surf-parse` — parser for the SurfDoc format.
//!
//! SurfDoc is a markdown superset with typed block directives, embedded data,
//! and presentation hints. This crate provides the foundational parser that
//! turns `.surf` (or `.md`) source text into a structured `SurfDoc` tree.
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
pub mod error;
pub mod icons;
pub mod inline;
pub mod parse;
pub mod render_html;
pub mod render_md;
#[cfg(feature = "pdf")]
pub mod render_pdf;
#[cfg(feature = "terminal")]
pub mod render_term;
pub mod types;
pub mod validate;

pub use builder::SurfDocBuilder;
pub use error::*;
pub use parse::parse;
pub use types::*;

pub use render_html::{PageConfig, SiteConfig, PageEntry, extract_site, humanize_route, render_site_page};

#[cfg(feature = "pdf")]
pub use render_pdf::{PdfConfig, PdfError};

impl SurfDoc {
    /// Render this document as standard CommonMark markdown (no `::` markers).
    pub fn to_markdown(&self) -> String {
        render_md::to_markdown(self)
    }

    /// Render this document as an HTML fragment with `surfdoc-*` CSS classes.
    pub fn to_html(&self) -> String {
        render_html::to_html(self)
    }

    /// Render this document as a complete HTML page with SurfDoc discovery metadata.
    pub fn to_html_page(&self, config: &PageConfig) -> String {
        render_html::to_html_page(self, config)
    }

    /// Render this document to PDF bytes using headless Chromium.
    ///
    /// Requires the `pdf` feature and a Chromium/Chrome installation.
    #[cfg(feature = "pdf")]
    pub async fn to_pdf(
        &self,
        config: &render_pdf::PdfConfig,
    ) -> Result<Vec<u8>, render_pdf::PdfError> {
        render_pdf::to_pdf(self, config).await
    }

    /// Render this document as ANSI-colored terminal text.
    #[cfg(feature = "terminal")]
    pub fn to_terminal(&self) -> String {
        render_term::to_terminal(self)
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

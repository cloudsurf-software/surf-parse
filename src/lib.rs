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
pub mod error;
pub mod extract;
pub mod icons;
pub mod inline;
pub mod parse;
pub mod render_html;
pub mod render_md;
#[cfg(feature = "pdf")]
pub mod render_pdf;
pub mod render_typst;
#[cfg(feature = "terminal")]
pub mod render_term;
#[cfg(feature = "native")]
pub mod render_native;
#[cfg(feature = "axum")]
pub mod serve;
pub mod template;
pub mod types;
pub mod validate;
pub mod manifest;

/// Unified CSS for app chrome and SurfDoc content rendering.
///
/// Contains theme variables, reset, navigation, buttons, cards, forms,
/// and all 29 SurfDoc block type styles. Dark-first with light mode support
/// via `data-theme="light"` or `prefers-color-scheme`.
///
/// Override `:root` variables (`--accent`, `--background`, `--surface`,
/// `--font-heading`, `--font-body`) for site-level theming.
pub const SURFDOC_CSS: &str = include_str!("../assets/surfdoc.css");

pub use builder::SurfDocBuilder;
pub use error::*;
pub use extract::ExtractedCode;
pub use parse::parse;
pub use template::TemplateContext;
pub use types::*;
pub use manifest::{
    AppManifest, AuthConfig, BindingConfig, ModelConfig, RouteConfig,
    PageConfig as ManifestPageConfig,
};

pub use render_html::{PageConfig, SiteConfig, PageEntry, extract_site, humanize_route, render_site_page};

/// Parse a `.surf` source string and extract the first app manifest.
///
/// Convenience wrapper: parses the source, extracts the manifest, and returns
/// a `Result` suitable for pipeline error handling.
pub fn parse_app_manifest(source: &str) -> Result<manifest::AppManifest, String> {
    let result = parse::parse(source);

    // Surface any fatal parse errors.
    let fatal: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == error::Severity::Error)
        .collect();
    if !fatal.is_empty() {
        let msgs: Vec<String> = fatal.iter().map(|d| d.message.clone()).collect();
        return Err(format!("Parse errors: {}", msgs.join("; ")));
    }

    result
        .doc
        .extract_manifest()
        .ok_or_else(|| "No ::app block found in spec source".to_string())
}

/// Validate an extracted app manifest for codegen readiness.
///
/// Checks structural rules required before code generation:
/// - Must have at least one model
/// - Must have a User model with id + email fields
/// - Must have an ::auth block
/// - ref() fields must reference existing models
/// - Route returns/accepts must reference existing models
///
/// Returns `Ok(())` if valid, or `Err(errors)` with a list of problems.
pub fn validate_app_manifest(manifest: &manifest::AppManifest) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Must have at least one model.
    if manifest.models.is_empty() {
        errors.push("App spec must define at least one ::model".to_string());
    }

    // Must have auth.
    if manifest.auth.is_none() {
        errors.push("App spec must include an ::auth block".to_string());
    }

    // Collect model names for reference checking.
    let model_names: std::collections::HashSet<&str> = manifest
        .models
        .iter()
        .map(|m| m.name.as_str())
        .collect();

    // Must have a User model.
    if !model_names.contains("User") {
        errors.push("App spec must include a ::model[name=User]".to_string());
    }

    // Check User model has id and email fields.
    if let Some(user_model) = manifest.models.iter().find(|m| m.name == "User") {
        let field_names: Vec<&str> = user_model.fields.iter().map(|f| f.name.as_str()).collect();
        if !field_names.contains(&"id") {
            errors.push("User model must have an 'id' field".to_string());
        }
        if !field_names.contains(&"email") {
            errors.push("User model must have an 'email' field".to_string());
        }
    }

    // Check ref() fields point to existing models.
    for model in &manifest.models {
        for field in &model.fields {
            if let types::ModelFieldType::Ref(ref target) = field.field_type {
                if !model_names.contains(target.as_str()) {
                    errors.push(format!(
                        "Field {}.{} references unknown model '{}'",
                        model.name, field.name, target
                    ));
                }
            }
        }
    }

    // Check route returns/accepts reference existing models.
    for route in &manifest.routes {
        if let Some(ref returns) = route.returns {
            let model_ref = returns
                .strip_prefix("list(")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or(returns);
            if model_ref != "none" && !model_names.contains(model_ref) {
                errors.push(format!(
                    "Route {:?} {} returns unknown model '{}'",
                    route.method, route.path, model_ref
                ));
            }
        }
        if let Some(ref body) = route.body {
            if !model_names.contains(body.as_str()) {
                errors.push(format!(
                    "Route {:?} {} accepts unknown model '{}'",
                    route.method, route.path, body
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

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
    pub fn to_typst(&self) -> String {
        render_typst::to_typst(self)
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

    /// Extract the first app manifest from the document.
    pub fn extract_manifest(&self) -> Option<manifest::AppManifest> {
        manifest::extract_manifest(self)
    }

    /// Extract all app manifests from the document.
    pub fn extract_all_manifests(&self) -> Vec<manifest::AppManifest> {
        manifest::extract_all_manifests(self)
    }

    /// Extract all code blocks from this document.
    ///
    /// Returns [`ExtractedCode`] items in document order. Blocks without a
    /// `[lang=...]` attribute have `language` set to `""`. Blocks without a
    /// `[file=...]` attribute have `file_path` set to `None`.
    ///
    /// # Example
    ///
    /// ```
    /// let doc = surf_parse::parse("::code[lang=rust]\nfn main() {}\n::\n").doc;
    /// let code = doc.extract_code();
    /// assert_eq!(code.len(), 1);
    /// assert_eq!(code[0].language, "rust");
    /// ```
    pub fn extract_code(&self) -> Vec<ExtractedCode> {
        extract::extract_code_blocks(&self.blocks)
    }

    /// Extract code blocks filtered by language.
    ///
    /// Language matching is case-insensitive with alias normalization:
    /// `"rs"` matches `"rust"`, `"ts"` matches `"typescript"`, etc.
    /// See [`extract::normalize_lang`] for the full alias table.
    ///
    /// # Example
    ///
    /// ```
    /// let doc = surf_parse::parse(
    ///     "::code[lang=rust]\nfn main() {}\n::\n::code[lang=python]\nx = 1\n::\n"
    /// ).doc;
    /// let rust_code = doc.extract_code_by_lang("rs");
    /// assert_eq!(rust_code.len(), 1);
    /// assert_eq!(rust_code[0].content, "fn main() {}");
    /// ```
    pub fn extract_code_by_lang(&self, language: &str) -> Vec<ExtractedCode> {
        extract::extract_code_blocks_by_lang(&self.blocks, language)
    }
}

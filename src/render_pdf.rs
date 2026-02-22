//! PDF renderer via Typst.
//!
//! Converts a SurfDoc to Typst markup using [`render_typst`], then compiles it
//! through the Typst engine to produce PDF bytes. Pure Rust, no external
//! dependencies (no Chrome, no system fonts required).

use crate::render_typst;
use crate::types::SurfDoc;

use typst_as_lib::TypstEngine;

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

/// Configuration for PDF rendering.
#[derive(Debug, Clone)]
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
        }
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
/// # Errors
///
/// Returns [`PdfError`] if Typst compilation or PDF rendering fails.
pub fn to_pdf(doc: &SurfDoc, config: &PdfConfig) -> Result<Vec<u8>, PdfError> {
    // Generate Typst markup from the SurfDoc block tree
    let mut typst_source = render_typst::to_typst(doc);

    // Apply config overrides (paper size, margins) at the top of the document
    let overrides = build_config_overrides(config);
    if !overrides.is_empty() {
        typst_source = format!("{overrides}\n{typst_source}");
    }

    // Build the Typst engine with our source
    let engine = TypstEngine::builder()
        .main_file(typst_source)
        .build();

    // Compile to a paged document
    let result = engine.compile::<typst::layout::PagedDocument>();

    let compiled = result
        .output
        .map_err(|e| PdfError::Compilation(format!("{e:?}")))?;

    // Render to PDF bytes
    let pdf_options = typst_pdf::PdfOptions::default();
    let pdf_bytes = typst_pdf::pdf(&compiled, &pdf_options)
        .map_err(|e| PdfError::PdfRendering(format!("{e:?}")))?;

    Ok(pdf_bytes)
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

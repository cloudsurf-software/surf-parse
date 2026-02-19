//! PDF renderer via headless Chromium.
//!
//! Reuses the HTML page renderer and pipes the output through headless Chrome's
//! built-in PDF printer using the Chrome DevTools Protocol.

use crate::render_html::PageConfig;
use crate::types::SurfDoc;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
use futures::StreamExt;

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
    fn width(&self) -> f64 {
        match self {
            Self::Letter => 8.5,
            Self::A4 => 8.27,
            Self::Legal => 8.5,
            Self::Custom { width, .. } => *width,
        }
    }

    /// Height in inches.
    fn height(&self) -> f64 {
        match self {
            Self::Letter => 11.0,
            Self::A4 => 11.69,
            Self::Legal => 14.0,
            Self::Custom { height, .. } => *height,
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
    /// Optional HTML header template. Supports `date`, `title`, `url`,
    /// `pageNumber`, and `totalPages` CSS classes.
    pub header_template: Option<String>,
    /// Optional HTML footer template. Same class support as header.
    pub footer_template: Option<String>,
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
            header_template: None,
            footer_template: None,
            print_background: true,
            title: None,
            source_path: None,
        }
    }
}

/// Errors that can occur during PDF generation.
#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    /// Failed to launch headless Chrome.
    #[error("Chrome launch failed: {0}")]
    ChromeLaunch(String),

    /// Failed to load page content.
    #[error("Page load failed: {0}")]
    PageLoad(String),

    /// Failed to generate PDF from page.
    #[error("PDF generation failed: {0}")]
    PdfGeneration(String),
}

/// Render a `SurfDoc` to PDF bytes using headless Chromium.
///
/// This launches a headless Chrome instance, renders the document's HTML page
/// output, and uses Chrome's built-in PDF printer to produce the output.
///
/// # Errors
///
/// Returns [`PdfError`] if Chrome cannot be launched, the page fails to load,
/// or PDF generation fails.
pub async fn to_pdf(doc: &SurfDoc, config: &PdfConfig) -> Result<Vec<u8>, PdfError> {
    let page_config = PageConfig {
        source_path: config
            .source_path
            .clone()
            .unwrap_or_else(|| "source.surf".to_string()),
        title: config.title.clone(),
        canonical_url: None,
        description: None,
        lang: None,
        og_image: None,
    };

    let html = doc.to_html_page(&page_config);
    let html = inject_print_css(&html, config);

    // Launch headless Chrome
    let browser_config = BrowserConfig::builder()
        .no_sandbox()
        .build()
        .map_err(|e| PdfError::ChromeLaunch(e.to_string()))?;

    let (mut browser, mut handler) = Browser::launch(browser_config)
        .await
        .map_err(|e| PdfError::ChromeLaunch(e.to_string()))?;

    // Drive the handler on a background task
    let handler_task = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    // Create a new page and set content
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| PdfError::PageLoad(e.to_string()))?;

    page.set_content(&html)
        .await
        .map_err(|e| PdfError::PageLoad(e.to_string()))?;

    // Build PDF params
    let mut pdf_params = PrintToPdfParams::builder()
        .paper_width(config.paper_size.width())
        .paper_height(config.paper_size.height())
        .margin_top(config.margins.top)
        .margin_right(config.margins.right)
        .margin_bottom(config.margins.bottom)
        .margin_left(config.margins.left)
        .landscape(config.landscape)
        .print_background(config.print_background);

    if config.header_template.is_some() || config.footer_template.is_some() {
        pdf_params = pdf_params.display_header_footer(true);
        if let Some(ref header) = config.header_template {
            pdf_params = pdf_params.header_template(header);
        }
        if let Some(ref footer) = config.footer_template {
            pdf_params = pdf_params.footer_template(footer);
        }
    }

    let pdf_bytes = page
        .pdf(pdf_params.build())
        .await
        .map_err(|e| PdfError::PdfGeneration(e.to_string()))?;

    // Clean up
    let _ = browser.close().await;
    let _ = handler_task.await;

    Ok(pdf_bytes)
}

/// Inject print-specific CSS into an HTML page before the closing `</head>` tag.
///
/// Adds `@page` rules for paper size and margins, plus `@media print` overrides
/// to ensure clean PDF output.
pub fn inject_print_css(html: &str, config: &PdfConfig) -> String {
    let width = config.paper_size.width();
    let height = config.paper_size.height();
    let top = config.margins.top;
    let right = config.margins.right;
    let bottom = config.margins.bottom;
    let left = config.margins.left;

    let print_css = format!(
        r#"<style>
    @page {{
        size: {width}in {height}in;
        margin: {top}in {right}in {bottom}in {left}in;
    }}
    @media print {{
        body {{
            -webkit-print-color-adjust: exact;
            print-color-adjust: exact;
        }}
        .surfdoc {{
            max-width: 100%;
            margin: 0;
            padding: 0;
        }}
    }}
    </style>"#
    );

    // Insert before </head>
    if let Some(pos) = html.find("</head>") {
        let mut result = String::with_capacity(html.len() + print_css.len() + 1);
        result.push_str(&html[..pos]);
        result.push('\n');
        result.push_str(&print_css);
        result.push('\n');
        result.push_str(&html[pos..]);
        result
    } else {
        // No </head> found — prepend the style block
        format!("{print_css}\n{html}")
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
        assert!(config.header_template.is_none());
        assert!(config.footer_template.is_none());
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
    fn inject_print_css_inserts_before_head_close() {
        let html = r#"<html>
<head>
    <title>Test</title>
</head>
<body>Hello</body>
</html>"#;

        let config = PdfConfig::default();
        let result = inject_print_css(html, &config);

        // The @page rule should appear before </head>
        let head_close_pos = result.find("</head>").expect("should have </head>");
        let page_rule_pos = result.find("@page").expect("should have @page rule");
        assert!(
            page_rule_pos < head_close_pos,
            "@page should appear before </head>"
        );

        // Should contain paper size
        assert!(result.contains("8.27in"));
        assert!(result.contains("11.69in"));

        // Should contain margin values
        assert!(result.contains("1in"));

        // Should contain print media query
        assert!(result.contains("@media print"));
        assert!(result.contains("print-color-adjust: exact"));
    }

    #[test]
    fn inject_print_css_custom_margins() {
        let html = "<html><head></head><body></body></html>";
        let config = PdfConfig {
            margins: Margins {
                top: 0.5,
                right: 0.75,
                bottom: 0.5,
                left: 0.75,
            },
            ..PdfConfig::default()
        };

        let result = inject_print_css(html, &config);
        assert!(result.contains("0.5in 0.75in 0.5in 0.75in"));
    }

    #[test]
    fn inject_print_css_letter_size() {
        let html = "<html><head></head><body></body></html>";
        let config = PdfConfig {
            paper_size: PaperSize::Letter,
            ..PdfConfig::default()
        };

        let result = inject_print_css(html, &config);
        assert!(result.contains("8.5in 11in"));
    }

    #[test]
    fn inject_print_css_no_head_tag() {
        let html = "<html><body>Hello</body></html>";
        let config = PdfConfig::default();
        let result = inject_print_css(html, &config);

        // Should prepend the style
        assert!(result.starts_with("<style>"));
        assert!(result.contains("@page"));
    }

    #[test]
    fn inject_print_css_preserves_original_content() {
        let html = r#"<html>
<head>
    <title>My Doc</title>
    <style>.surfdoc { color: red; }</style>
</head>
<body>
<article class="surfdoc">Content here</article>
</body>
</html>"#;

        let config = PdfConfig::default();
        let result = inject_print_css(html, &config);

        // Original content should be preserved
        assert!(result.contains("<title>My Doc</title>"));
        assert!(result.contains(".surfdoc { color: red; }"));
        assert!(result.contains("Content here"));
    }

    #[test]
    fn pdf_error_display() {
        let err = PdfError::ChromeLaunch("no chrome found".to_string());
        assert_eq!(err.to_string(), "Chrome launch failed: no chrome found");

        let err = PdfError::PageLoad("timeout".to_string());
        assert_eq!(err.to_string(), "Page load failed: timeout");

        let err = PdfError::PdfGeneration("out of memory".to_string());
        assert_eq!(err.to_string(), "PDF generation failed: out of memory");
    }

    /// Integration test that requires a working Chrome installation.
    /// Run with: cargo test --features pdf -- --ignored
    #[tokio::test]
    #[ignore]
    async fn to_pdf_produces_valid_pdf_bytes() {
        let source = "# Hello World\n\nThis is a test document.\n";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty());

        let config = PdfConfig::default();
        let pdf_bytes = to_pdf(&result.doc, &config).await.expect("PDF generation should succeed");

        // PDF files start with the %PDF magic bytes
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

    /// Integration test with landscape orientation.
    #[tokio::test]
    #[ignore]
    async fn to_pdf_landscape() {
        let source = "# Landscape Test\n\nWide content.\n";
        let result = crate::parse(source);

        let config = PdfConfig {
            paper_size: PaperSize::Letter,
            landscape: true,
            ..PdfConfig::default()
        };

        let pdf_bytes = to_pdf(&result.doc, &config).await.expect("PDF should generate");
        assert!(pdf_bytes.starts_with(b"%PDF-"));
    }
}

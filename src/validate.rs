//! Schema validation for SurfDoc documents.
//!
//! Checks required attributes, front matter rules, and block-level constraints.
//! Returns a list of `Diagnostic` items (non-fatal).

use crate::error::{Diagnostic, Severity};
use crate::types::{Block, SurfDoc};

/// Validate a parsed `SurfDoc` and return any diagnostics.
///
/// This function checks front matter completeness, required block attributes,
/// and block content constraints. It never modifies the document.
pub fn validate(doc: &SurfDoc) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // Front matter validation
    validate_front_matter(doc, &mut diagnostics);

    // Per-block validation
    for block in &doc.blocks {
        validate_block(block, &mut diagnostics);
    }

    diagnostics
}

fn validate_front_matter(doc: &SurfDoc, diagnostics: &mut Vec<Diagnostic>) {
    match &doc.front_matter {
        None => {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: "Missing front matter: no title specified".into(),
                span: None,
                code: Some("V001".into()),
            });
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: "Missing front matter: no doc_type specified".into(),
                span: None,
                code: Some("V002".into()),
            });
        }
        Some(fm) => {
            if fm.title.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Missing front matter field: title".into(),
                    span: None,
                    code: Some("V001".into()),
                });
            }
            if fm.doc_type.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Missing front matter field: doc_type".into(),
                    span: None,
                    code: Some("V002".into()),
                });
            }
        }
    }
}

fn validate_block(block: &Block, diagnostics: &mut Vec<Diagnostic>) {
    match block {
        Block::Metric {
            label,
            value,
            span,
            ..
        } => {
            if label.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Metric block is missing required attribute: label".into(),
                    span: Some(*span),
                    code: Some("V010".into()),
                });
            }
            if value.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Metric block is missing required attribute: value".into(),
                    span: Some(*span),
                    code: Some("V011".into()),
                });
            }
        }

        Block::Figure { src, span, .. } => {
            if src.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Figure block is missing required attribute: src".into(),
                    span: Some(*span),
                    code: Some("V020".into()),
                });
            }
        }

        Block::Data {
            headers,
            rows,
            span,
            ..
        } => {
            if !headers.is_empty() && rows.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Data block has headers but zero data rows".into(),
                    span: Some(*span),
                    code: Some("V030".into()),
                });
            }
        }

        Block::Callout {
            content, span, ..
        } => {
            if content.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Callout block has empty content".into(),
                    span: Some(*span),
                    code: Some("V040".into()),
                });
            }
        }

        Block::Code {
            content, span, ..
        } => {
            if content.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Code block has empty content".into(),
                    span: Some(*span),
                    code: Some("V050".into()),
                });
            }
        }

        Block::Decision {
            content, span, ..
        } => {
            if content.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Decision block has empty body".into(),
                    span: Some(*span),
                    code: Some("V060".into()),
                });
            }
        }

        Block::Tabs { tabs, span, .. } => {
            if tabs.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Tabs block has no tab panels".into(),
                    span: Some(*span),
                    code: Some("V070".into()),
                });
            }
        }

        Block::Quote {
            content, span, ..
        } => {
            if content.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Quote block has empty content".into(),
                    span: Some(*span),
                    code: Some("V080".into()),
                });
            }
        }

        Block::Cta {
            label,
            href,
            span,
            ..
        } => {
            if label.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Cta block is missing required attribute: label".into(),
                    span: Some(*span),
                    code: Some("V090".into()),
                });
            }
            if href.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Cta block is missing required attribute: href".into(),
                    span: Some(*span),
                    code: Some("V091".into()),
                });
            }
        }

        Block::HeroImage { src, span, .. } => {
            if src.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "HeroImage block is missing required attribute: src".into(),
                    span: Some(*span),
                    code: Some("V100".into()),
                });
            }
        }

        Block::Testimonial {
            content, span, ..
        } => {
            if content.trim().is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Testimonial block has empty content".into(),
                    span: Some(*span),
                    code: Some("V110".into()),
                });
            }
        }

        Block::Faq { items, span, .. } => {
            if items.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Faq block has no question/answer items".into(),
                    span: Some(*span),
                    code: Some("V120".into()),
                });
            }
        }

        Block::PricingTable {
            headers,
            rows,
            span,
            ..
        } => {
            if headers.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "PricingTable block has no headers (tier names)".into(),
                    span: Some(*span),
                    code: Some("V130".into()),
                });
            }
            if !headers.is_empty() && rows.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "PricingTable block has headers but zero feature rows".into(),
                    span: Some(*span),
                    code: Some("V131".into()),
                });
            }
        }

        Block::Page { route, span, .. } => {
            if route.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Page block is missing required attribute: route".into(),
                    span: Some(*span),
                    code: Some("V140".into()),
                });
            }
        }

        Block::Nav { items, span, .. } => {
            if items.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Nav block has no navigation items".into(),
                    span: Some(*span),
                    code: Some("V150".into()),
                });
            }
        }

        // Markdown, Tasks, Summary, Columns, Style, Site, Unknown — no required-field validation
        _ => {}
    }
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

    #[test]
    fn validate_empty_doc() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![],
            source: String::new(),
        };
        let diags = validate(&doc);
        // Should warn about missing title and doc_type
        assert!(
            diags.iter().any(|d| d.message.contains("title")),
            "Should warn about missing title"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("doc_type")),
            "Should warn about missing doc_type"
        );
    }

    #[test]
    fn validate_complete_doc() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Complete Doc".into()),
                doc_type: Some(DocType::Doc),
                ..FrontMatter::default()
            }),
            blocks: vec![Block::Markdown {
                content: "Hello".into(),
                span: span(),
            }],
            source: String::new(),
        };
        let diags = validate(&doc);
        assert!(
            diags.is_empty(),
            "Complete doc should have no diagnostics, got: {diags:?}"
        );
    }

    #[test]
    fn validate_missing_metric_label() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Test".into()),
                doc_type: Some(DocType::Report),
                ..FrontMatter::default()
            }),
            blocks: vec![Block::Metric {
                label: String::new(),
                value: "$2K".into(),
                trend: None,
                unit: None,
                span: span(),
            }],
            source: String::new(),
        };
        let diags = validate(&doc);
        let metric_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("label"))
            .collect();
        assert_eq!(metric_diags.len(), 1);
        assert_eq!(metric_diags[0].severity, Severity::Error);
    }

    #[test]
    fn validate_missing_figure_src() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Test".into()),
                doc_type: Some(DocType::Doc),
                ..FrontMatter::default()
            }),
            blocks: vec![Block::Figure {
                src: String::new(),
                caption: Some("Photo".into()),
                alt: None,
                width: None,
                span: span(),
            }],
            source: String::new(),
        };
        let diags = validate(&doc);
        let figure_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("src"))
            .collect();
        assert_eq!(figure_diags.len(), 1);
        assert_eq!(figure_diags[0].severity, Severity::Error);
    }

    #[test]
    fn validate_empty_code() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Test".into()),
                doc_type: Some(DocType::Doc),
                ..FrontMatter::default()
            }),
            blocks: vec![Block::Code {
                lang: Some("rust".into()),
                file: None,
                highlight: vec![],
                content: "   ".into(), // whitespace-only
                span: span(),
            }],
            source: String::new(),
        };
        let diags = validate(&doc);
        let code_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Code block"))
            .collect();
        assert_eq!(code_diags.len(), 1);
        assert_eq!(code_diags[0].severity, Severity::Warning);
    }
}

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

    // Validate ::app children
    for block in &doc.blocks {
        if let Block::App { children, .. } = block {
            for child in children {
                validate_block(child, &mut diagnostics);
            }
        }
    }

    // Cross-block validation: duplicate page routes
    validate_unique_page_routes(&doc.blocks, &mut diagnostics);

    // NOTE (0.10.0 open-core split): cross-model reference checking (V303)
    // and marketplace field-type semantics (V340-V343) moved to the private
    // surf-appcompile crate (validate_app_doc) — they are compile-to-app
    // rules, not document format rules.

    diagnostics
}

/// Check for duplicate `::page[route=...]` values within a document.
fn validate_unique_page_routes(blocks: &[Block], diagnostics: &mut Vec<Diagnostic>) {
    let mut seen: Vec<(&str, &crate::types::Span)> = Vec::new();
    for block in blocks {
        if let Block::Page { route, span, .. } = block {
            if let Some((_, first_span)) = seen.iter().find(|(r, _)| *r == route.as_str()) {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!(
                        "Duplicate page route \"{}\": first defined at line {}",
                        route, first_span.start_line
                    ),
                    span: Some(*span),
                    code: Some("V141".into()),
                    fix: None,
                });
            } else {
                seen.push((route.as_str(), span));
            }
        }
    }
}

fn validate_front_matter(doc: &SurfDoc, diagnostics: &mut Vec<Diagnostic>) {
    match &doc.front_matter {
        None => {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: "Missing front matter: no title specified".into(),
                span: None,
                code: Some("V001".into()),
                fix: None,
            });
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                message: "Missing front matter: no doc_type specified".into(),
                span: None,
                code: Some("V002".into()),
                fix: None,
            });
        }
        Some(fm) => {
            if fm.title.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Missing front matter field: title".into(),
                    span: None,
                    code: Some("V001".into()),
                    fix: None,
                });
            }
            if fm.doc_type.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Missing front matter field: doc_type".into(),
                    span: None,
                    code: Some("V002".into()),
                    fix: None,
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
                    fix: None,
                });
            }
            if value.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Metric block is missing required attribute: value".into(),
                    span: Some(*span),
                    code: Some("V011".into()),
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
                });
            }
            if href.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Cta block is missing required attribute: href".into(),
                    span: Some(*span),
                    code: Some("V091".into()),
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
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
                    fix: None,
                });
            }
            if !headers.is_empty() && rows.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "PricingTable block has headers but zero feature rows".into(),
                    span: Some(*span),
                    code: Some("V131".into()),
                    fix: None,
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
                    fix: None,
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
                    fix: None,
                });
            }
        }

        Block::App { name, span, .. } => {
            if name.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "App block is missing required attribute: name".into(),
                    span: Some(*span),
                    code: Some("V200".into()),
                    fix: None,
                });
            }
        }

        Block::Deploy { env, span, .. } => {
            if env.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Deploy block is missing required attribute: env".into(),
                    span: Some(*span),
                    code: Some("V201".into()),
                    fix: None,
                });
            } else if let Some(e) = env {
                if !["develop", "staging", "production"].contains(&e.as_str()) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Deploy env \"{}\" is not one of: develop, staging, production", e),
                        span: Some(*span),
                        code: Some("V202".into()),
                        fix: None,
                    });
                }
            }
        }

        Block::InfraEnv { tier, span, .. } => {
            if tier.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Env block is missing tier attribute".into(),
                    span: Some(*span),
                    code: Some("V203".into()),
                    fix: None,
                });
            } else if let Some(t) = tier {
                if !["required", "recommended", "optional", "defaults"].contains(&t.as_str()) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Env tier \"{}\" is not one of: required, recommended, optional, defaults", t),
                        span: Some(*span),
                        code: Some("V204".into()),
                        fix: None,
                    });
                }
            }
        }

        Block::Health { path, span, .. } => {
            if path.is_none() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Health block is missing required attribute: path".into(),
                    span: Some(*span),
                    code: Some("V205".into()),
                    fix: None,
                });
            }
        }

        Block::Smoke { checks, span, .. } => {
            for (i, check) in checks.iter().enumerate() {
                if !["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].contains(&check.method.as_str()) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Smoke check {} has unrecognized HTTP method: {}", i + 1, check.method),
                        span: Some(*span),
                        code: Some("V206".into()),
                        fix: None,
                    });
                }
            }
        }

        Block::Concurrency { hard_limit, soft_limit, span, .. } => {
            if let (Some(hard), Some(soft)) = (hard_limit, soft_limit) {
                if hard < soft {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Concurrency hard_limit ({}) should be >= soft_limit ({})", hard, soft),
                        span: Some(*span),
                        code: Some("V207".into()),
                        fix: None,
                    });
                }
            }
        }

        Block::Volumes { entries, span, .. } => {
            for entry in entries {
                if entry.name.is_empty() || entry.mount.is_empty() {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: "Volume entry must have both name and mount path".into(),
                        span: Some(*span),
                        code: Some("V208".into()),
                        fix: None,
                    });
                }
            }
        }

        Block::Model { name, fields, span, .. } => {
            if name.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Model block is missing required attribute: name".into(),
                    span: Some(*span),
                    code: Some("V300".into()),
                    fix: None,
                });
            }
            if fields.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Model \"{}\" has no fields defined", name),
                    span: Some(*span),
                    code: Some("V301".into()),
                    fix: None,
                });
            }
            // Check for duplicate field names
            let mut seen_fields: Vec<&str> = Vec::new();
            for field in fields {
                if seen_fields.contains(&field.name.as_str()) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("Model \"{}\" has duplicate field name: {}", name, field.name),
                        span: Some(*span),
                        code: Some("V302".into()),
                        fix: None,
                    });
                } else {
                    seen_fields.push(&field.name);
                }
            }

        }

        Block::Route { path, span, .. } => {
            if path.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Route block is missing required attribute: path".into(),
                    span: Some(*span),
                    code: Some("V310".into()),
                    fix: None,
                });
            } else if !path.starts_with('/') {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Route path \"{}\" should start with /", path),
                    span: Some(*span),
                    code: Some("V311".into()),
                    fix: None,
                });
            }
        }

        Block::Auth { roles, span, .. } => {
            if roles.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: "Auth block has no roles defined".into(),
                    span: Some(*span),
                    code: Some("V320".into()),
                    fix: None,
                });
            }
        }

        Block::Binding { source, target, span, .. } => {
            if source.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Binding block is missing required attribute: source".into(),
                    span: Some(*span),
                    code: Some("V330".into()),
                    fix: None,
                });
            }
            if target.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Binding block is missing required attribute: target".into(),
                    span: Some(*span),
                    code: Some("V331".into()),
                    fix: None,
                });
            }
        }

        Block::Details { .. } => {}
        Block::Divider { .. } => {}

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
    fn validate_duplicate_page_routes() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Test".into()),
                doc_type: Some(DocType::Doc),
                ..FrontMatter::default()
            }),
            blocks: vec![
                Block::Page {
                    route: "/".into(),
                    title: Some("Home v1".into()),
                    layout: None,
                    sidebar: false,
                    content: String::new(),
                    children: vec![],
                    span: Span { start_line: 1, end_line: 3, start_offset: 0, end_offset: 30 },
                },
                Block::Page {
                    route: "/about".into(),
                    title: Some("About".into()),
                    layout: None,
                    sidebar: false,
                    content: String::new(),
                    children: vec![],
                    span: Span { start_line: 4, end_line: 6, start_offset: 31, end_offset: 60 },
                },
                Block::Page {
                    route: "/".into(),
                    title: Some("Home v2".into()),
                    layout: None,
                    sidebar: false,
                    content: String::new(),
                    children: vec![],
                    span: Span { start_line: 7, end_line: 9, start_offset: 61, end_offset: 90 },
                },
            ],
            source: String::new(),
        };
        let diags = validate(&doc);
        let dup_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.code.as_deref() == Some("V141"))
            .collect();
        assert_eq!(dup_diags.len(), 1, "Expected exactly 1 duplicate route diagnostic");
        assert!(dup_diags[0].message.contains("/"), "Should mention the duplicate route");
        assert_eq!(dup_diags[0].severity, Severity::Error);
    }

    #[test]
    fn validate_unique_page_routes_no_false_positive() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Test".into()),
                doc_type: Some(DocType::Doc),
                ..FrontMatter::default()
            }),
            blocks: vec![
                Block::Page {
                    route: "/".into(),
                    title: Some("Home".into()),
                    layout: None,
                    sidebar: false,
                    content: String::new(),
                    children: vec![],
                    span: span(),
                },
                Block::Page {
                    route: "/about".into(),
                    title: Some("About".into()),
                    layout: None,
                    sidebar: false,
                    content: String::new(),
                    children: vec![],
                    span: span(),
                },
                Block::Page {
                    route: "/contact".into(),
                    title: Some("Contact".into()),
                    layout: None,
                    sidebar: false,
                    content: String::new(),
                    children: vec![],
                    span: span(),
                },
            ],
            source: String::new(),
        };
        let diags = validate(&doc);
        let dup_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.code.as_deref() == Some("V141"))
            .collect();
        assert!(dup_diags.is_empty(), "No duplicate route diagnostics expected");
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

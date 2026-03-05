//! Schema validation for SurfDoc documents.
//!
//! Checks required attributes, front matter rules, and block-level constraints.
//! Returns a list of `Diagnostic` items (non-fatal).

use crate::error::{Diagnostic, Severity};
use crate::types::{Block, FieldConstraint, ModelFieldType, SurfDoc};

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

    // Cross-block validation: model foreign key references
    validate_model_refs(&doc.blocks, &mut diagnostics);

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
                });
            } else {
                seen.push((route.as_str(), span));
            }
        }
    }
}

/// Check that `ref(ModelName)` fields point to existing `::model` blocks.
fn validate_model_refs(blocks: &[Block], diagnostics: &mut Vec<Diagnostic>) {
    use crate::types::ModelFieldType;

    // Collect all model names (including those nested inside ::app children)
    let mut model_names: Vec<String> = Vec::new();
    for block in blocks {
        if let Block::Model { name, .. } = block {
            model_names.push(name.clone());
        }
        if let Block::App { children, .. } = block {
            for child in children {
                if let Block::Model { name, .. } = child {
                    model_names.push(name.clone());
                }
            }
        }
    }

    // Check all ref() targets
    let check_fields = |fields: &[crate::types::ModelField], model_name: &str, span: &crate::types::Span, diagnostics: &mut Vec<Diagnostic>| {
        for field in fields {
            if let ModelFieldType::Ref(target) = &field.field_type {
                if !model_names.contains(target) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Model \"{}\" field \"{}\" references unknown model \"{}\"",
                            model_name, field.name, target
                        ),
                        span: Some(*span),
                        code: Some("V303".into()),
                    });
                }
            }
        }
    };

    for block in blocks {
        if let Block::Model { name, fields, span, .. } = block {
            check_fields(fields, name, span, diagnostics);
        }
        if let Block::App { children, .. } = block {
            for child in children {
                if let Block::Model { name, fields, span, .. } = child {
                    check_fields(fields, name, span, diagnostics);
                }
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

        Block::App { name, span, .. } => {
            if name.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "App block is missing required attribute: name".into(),
                    span: Some(*span),
                    code: Some("V200".into()),
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
                });
            } else if let Some(e) = env {
                if !["develop", "staging", "production"].contains(&e.as_str()) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Deploy env \"{}\" is not one of: develop, staging, production", e),
                        span: Some(*span),
                        code: Some("V202".into()),
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
                });
            } else if let Some(t) = tier {
                if !["required", "recommended", "optional", "defaults"].contains(&t.as_str()) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Env tier \"{}\" is not one of: required, recommended, optional, defaults", t),
                        span: Some(*span),
                        code: Some("V204".into()),
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
                });
            }
            if fields.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Model \"{}\" has no fields defined", name),
                    span: Some(*span),
                    code: Some("V301".into()),
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
                    });
                } else {
                    seen_fields.push(&field.name);
                }
            }

            // V340-V343: Semantic validation for marketplace field types
            for field in fields {
                match &field.field_type {
                    // V340: Money fields should have required or optional constraint
                    ModelFieldType::Money => {
                        let has_req_opt = field.constraints.iter().any(|c|
                            matches!(c, FieldConstraint::Required | FieldConstraint::Optional)
                        );
                        if !has_req_opt {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Warning,
                                message: format!(
                                    "Model \"{}\" field \"{}\" is type money but has no required/optional constraint — defaults to optional",
                                    name, field.name
                                ),
                                span: Some(*span),
                                code: Some("V340".into()),
                            });
                        }
                    }
                    // V341: Image fields default to optional (info if no required/optional)
                    ModelFieldType::Image => {
                        let has_required = field.constraints.iter().any(|c|
                            matches!(c, FieldConstraint::Required)
                        );
                        let has_optional = field.constraints.iter().any(|c|
                            matches!(c, FieldConstraint::Optional)
                        );
                        if !has_required && !has_optional {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Info,
                                message: format!(
                                    "Model \"{}\" field \"{}\" is type image with no required/optional — defaults to optional",
                                    name, field.name
                                ),
                                span: Some(*span),
                                code: Some("V341".into()),
                            });
                        }
                    }
                    // V342: Email fields auto-capped at 254 (RFC 5321) — warn if max > 254
                    ModelFieldType::Email => {
                        for c in &field.constraints {
                            if let FieldConstraint::Max(n) = c {
                                if *n > 254 {
                                    diagnostics.push(Diagnostic {
                                        severity: Severity::Warning,
                                        message: format!(
                                            "Model \"{}\" field \"{}\" has max={} but email addresses cannot exceed 254 characters (RFC 5321) — capping to 254",
                                            name, field.name, n
                                        ),
                                        span: Some(*span),
                                        code: Some("V342".into()),
                                    });
                                }
                            }
                        }
                    }
                    // V343: URL fields auto-capped at 2048 — warn if max > 2048
                    ModelFieldType::Url => {
                        for c in &field.constraints {
                            if let FieldConstraint::Max(n) = c {
                                if *n > 2048 {
                                    diagnostics.push(Diagnostic {
                                        severity: Severity::Warning,
                                        message: format!(
                                            "Model \"{}\" field \"{}\" has max={} but URLs should not exceed 2048 characters — capping to 2048",
                                            name, field.name, n
                                        ),
                                        span: Some(*span),
                                        code: Some("V343".into()),
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
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
                });
            } else if !path.starts_with('/') {
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Route path \"{}\" should start with /", path),
                    span: Some(*span),
                    code: Some("V311".into()),
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
                });
            }
            if target.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: "Binding block is missing required attribute: target".into(),
                    span: Some(*span),
                    code: Some("V331".into()),
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

    // --- V340-V343 unit tests ---

    fn model_doc(field: ModelField) -> SurfDoc {
        SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("Test".into()),
                doc_type: Some(DocType::App),
                ..FrontMatter::default()
            }),
            blocks: vec![Block::Model {
                name: "Test".into(),
                fields: vec![field],
                span: span(),
            }],
            source: String::new(),
        }
    }

    #[test]
    fn validate_v340_money_no_required_optional() {
        let doc = model_doc(ModelField {
            name: "price".into(),
            field_type: ModelFieldType::Money,
            constraints: vec![],
        });
        let diags = validate(&doc);
        let v340: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V340")).collect();
        assert_eq!(v340.len(), 1);
        assert_eq!(v340[0].severity, Severity::Warning);
    }

    #[test]
    fn validate_v340_money_with_required() {
        let doc = model_doc(ModelField {
            name: "price".into(),
            field_type: ModelFieldType::Money,
            constraints: vec![FieldConstraint::Required],
        });
        let diags = validate(&doc);
        let v340: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V340")).collect();
        assert!(v340.is_empty());
    }

    #[test]
    fn validate_v340_money_with_optional() {
        let doc = model_doc(ModelField {
            name: "price".into(),
            field_type: ModelFieldType::Money,
            constraints: vec![FieldConstraint::Optional],
        });
        let diags = validate(&doc);
        let v340: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V340")).collect();
        assert!(v340.is_empty());
    }

    #[test]
    fn validate_v341_image_no_constraints() {
        let doc = model_doc(ModelField {
            name: "photo".into(),
            field_type: ModelFieldType::Image,
            constraints: vec![],
        });
        let diags = validate(&doc);
        let v341: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V341")).collect();
        assert_eq!(v341.len(), 1);
        assert_eq!(v341[0].severity, Severity::Info);
    }

    #[test]
    fn validate_v341_image_with_optional() {
        let doc = model_doc(ModelField {
            name: "photo".into(),
            field_type: ModelFieldType::Image,
            constraints: vec![FieldConstraint::Optional],
        });
        let diags = validate(&doc);
        let v341: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V341")).collect();
        assert!(v341.is_empty());
    }

    #[test]
    fn validate_v342_email_max_over_254() {
        let doc = model_doc(ModelField {
            name: "email".into(),
            field_type: ModelFieldType::Email,
            constraints: vec![FieldConstraint::Required, FieldConstraint::Max(500)],
        });
        let diags = validate(&doc);
        let v342: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V342")).collect();
        assert_eq!(v342.len(), 1);
        assert_eq!(v342[0].severity, Severity::Warning);
    }

    #[test]
    fn validate_v342_email_max_under_254() {
        let doc = model_doc(ModelField {
            name: "email".into(),
            field_type: ModelFieldType::Email,
            constraints: vec![FieldConstraint::Required, FieldConstraint::Max(200)],
        });
        let diags = validate(&doc);
        let v342: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V342")).collect();
        assert!(v342.is_empty());
    }

    #[test]
    fn validate_v342_email_no_max() {
        let doc = model_doc(ModelField {
            name: "email".into(),
            field_type: ModelFieldType::Email,
            constraints: vec![FieldConstraint::Required],
        });
        let diags = validate(&doc);
        let v342: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V342")).collect();
        assert!(v342.is_empty());
    }

    #[test]
    fn validate_v343_url_max_over_2048() {
        let doc = model_doc(ModelField {
            name: "website".into(),
            field_type: ModelFieldType::Url,
            constraints: vec![FieldConstraint::Optional, FieldConstraint::Max(5000)],
        });
        let diags = validate(&doc);
        let v343: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V343")).collect();
        assert_eq!(v343.len(), 1);
        assert_eq!(v343[0].severity, Severity::Warning);
    }

    #[test]
    fn validate_v343_url_max_under_2048() {
        let doc = model_doc(ModelField {
            name: "website".into(),
            field_type: ModelFieldType::Url,
            constraints: vec![FieldConstraint::Optional, FieldConstraint::Max(1000)],
        });
        let diags = validate(&doc);
        let v343: Vec<_> = diags.iter().filter(|d| d.code.as_deref() == Some("V343")).collect();
        assert!(v343.is_empty());
    }
}

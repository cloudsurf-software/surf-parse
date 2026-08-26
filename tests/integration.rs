//! Integration tests that parse complete fixture files end-to-end.

use surf_parse::{Block, CalloutType, FieldConstraint, ModelFieldType, Severity, icons::get_icon};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read_fixture(name: &str) -> String {
    let path = fixtures_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture '{}': {}", path.display(), e))
}

#[test]
fn basic_fixture_parses() {
    let content = read_fixture("basic.surf");
    let result = surf_parse::parse(&content);

    // Should parse without errors (warnings from leaf metrics are acceptable)
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Unexpected errors: {errors:?}");

    // Should have front matter
    let fm = result.doc.front_matter.as_ref().expect("Should have front matter");
    assert_eq!(fm.title.as_deref(), Some("Basic SurfDoc Test"));

    // Should have multiple blocks including all 8 typed block types
    assert!(
        result.doc.blocks.len() >= 8,
        "Expected at least 8 blocks (8 typed + markdown), got {}",
        result.doc.blocks.len()
    );

    // Verify we have at least one of each expected type
    let has_markdown = result.doc.blocks.iter().any(|b| matches!(b, Block::Markdown { .. }));
    let has_callout = result.doc.blocks.iter().any(|b| matches!(b, Block::Callout { .. }));
    let has_data = result.doc.blocks.iter().any(|b| matches!(b, Block::Data { .. }));
    let has_code = result.doc.blocks.iter().any(|b| matches!(b, Block::Code { .. }));
    let has_tasks = result.doc.blocks.iter().any(|b| matches!(b, Block::Tasks { .. }));
    let has_decision = result.doc.blocks.iter().any(|b| matches!(b, Block::Decision { .. }));
    let has_metric = result.doc.blocks.iter().any(|b| matches!(b, Block::Metric { .. }));
    let has_summary = result.doc.blocks.iter().any(|b| matches!(b, Block::Summary { .. }));
    let has_figure = result.doc.blocks.iter().any(|b| matches!(b, Block::Figure { .. }));

    assert!(has_markdown, "Should contain Markdown block");
    assert!(has_callout, "Should contain Callout block");
    assert!(has_data, "Should contain Data block");
    assert!(has_code, "Should contain Code block");
    assert!(has_tasks, "Should contain Tasks block");
    assert!(has_decision, "Should contain Decision block");
    assert!(has_metric, "Should contain Metric block");
    assert!(has_summary, "Should contain Summary block");
    assert!(has_figure, "Should contain Figure block");
}

#[test]
fn strategy_sample_parses() {
    let content = read_fixture("strategy-sample.surf");
    let result = surf_parse::parse(&content);

    // Should not panic and should produce a document
    let fm = result.doc.front_matter.as_ref().expect("Should have front matter");
    assert_eq!(fm.title.as_deref(), Some("Q1 2026 Product Strategy Review"));
    assert!(
        fm.doc_type.is_some(),
        "Should have a doc_type"
    );
    assert!(
        fm.author.as_deref() == Some("Brady Davis"),
        "Author should be Brady Davis"
    );

    // Should have blocks
    assert!(
        !result.doc.blocks.is_empty(),
        "Strategy sample should have blocks"
    );

    // Should have data, decisions, metrics, callouts
    let has_data = result.doc.blocks.iter().any(|b| matches!(b, Block::Data { .. }));
    let has_decision = result.doc.blocks.iter().any(|b| matches!(b, Block::Decision { .. }));
    let has_metric = result.doc.blocks.iter().any(|b| matches!(b, Block::Metric { .. }));
    assert!(has_data, "Strategy sample should contain Data blocks");
    assert!(has_decision, "Strategy sample should contain Decision blocks");
    assert!(has_metric, "Strategy sample should contain Metric blocks");
}

#[test]
fn malformed_produces_diagnostics() {
    let content = read_fixture("malformed.surf");
    let result = surf_parse::parse(&content);

    // Should produce diagnostics (the unclosed front matter at minimum)
    assert!(
        !result.diagnostics.is_empty(),
        "Malformed input should produce parse diagnostics"
    );

    // Should still produce a document (graceful degradation)
    // The parser should not panic
    let _blocks = &result.doc.blocks;

    // Validation should also find issues
    let validation_diags = result.doc.validate();
    assert!(
        !validation_diags.is_empty(),
        "Malformed input should produce validation diagnostics"
    );
}

#[test]
fn nesting_fixture_parses() {
    let content = read_fixture("nesting.surf");
    let result = surf_parse::parse(&content);

    // Should have front matter
    let fm = result.doc.front_matter.as_ref().expect("Should have front matter");
    assert_eq!(fm.title.as_deref(), Some("Nesting Test"));

    // Should have blocks
    assert!(
        !result.doc.blocks.is_empty(),
        "Nesting fixture should produce blocks"
    );

    // The nested columns blocks should be resolved to Block::Columns
    let columns_blocks: Vec<_> = result
        .doc
        .blocks
        .iter()
        .filter(|b| matches!(b, Block::Columns { .. }))
        .collect();
    assert!(
        !columns_blocks.is_empty(),
        "Should contain at least one Columns block"
    );

    // Columns should have parsed the nested :::column directives into separate columns
    if let Block::Columns { columns, .. } = &columns_blocks[0] {
        assert!(
            !columns.is_empty(),
            "Columns block should have parsed column content"
        );
    }

    // Should also have a non-nested callout after the nesting
    let has_callout = result.doc.blocks.iter().any(|b| matches!(b, Block::Callout { .. }));
    assert!(has_callout, "Should have a callout block after nested structures");
}

#[test]
fn render_basic_markdown() {
    let content = read_fixture("basic.surf");
    let result = surf_parse::parse(&content);
    let md = result.doc.to_markdown();

    // Markdown output should not contain :: directive markers
    assert!(
        !md.contains("::callout"),
        "Markdown output should not contain ::callout"
    );
    assert!(
        !md.contains("::data"),
        "Markdown output should not contain ::data"
    );
    assert!(
        !md.contains("::code["),
        "Markdown output should not contain ::code"
    );
    assert!(
        !md.contains("::metric"),
        "Markdown output should not contain ::metric"
    );

    // Should contain degraded content
    assert!(md.contains("Hello, SurfDoc!"), "Should contain code content");
    assert!(
        md.contains("warning") || md.contains("Warning"),
        "Should contain callout type"
    );
}

#[test]
fn render_basic_html() {
    let content = read_fixture("basic.surf");
    let result = surf_parse::parse(&content);
    let html = result.doc.to_html();

    // HTML output should contain surfdoc- CSS classes
    assert!(
        html.contains("surfdoc-"),
        "HTML output should contain surfdoc- CSS classes"
    );
    assert!(
        html.contains("surfdoc-callout"),
        "HTML should contain surfdoc-callout class"
    );
    assert!(
        html.contains("surfdoc-code"),
        "HTML should contain surfdoc-code class"
    );
    assert!(
        html.contains("surfdoc-metric"),
        "HTML should contain surfdoc-metric class"
    );
}

#[test]
fn validate_basic() {
    let content = read_fixture("basic.surf");
    let result = surf_parse::parse(&content);
    let diags = result.doc.validate();

    // basic.surf should validate with zero errors
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "basic.surf should have no validation errors, got: {errors:?}"
    );
}

#[test]
fn validate_malformed() {
    let content = read_fixture("malformed.surf");
    let result = surf_parse::parse(&content);

    // Combine parse + validation diagnostics
    let mut all_diags = result.diagnostics;
    all_diags.extend(result.doc.validate());

    assert!(
        !all_diags.is_empty(),
        "malformed.surf should produce diagnostics"
    );

    // Should have at least one error-level diagnostic
    let has_error_or_warning = all_diags
        .iter()
        .any(|d| d.severity == Severity::Error || d.severity == Severity::Warning);
    assert!(
        has_error_or_warning,
        "malformed.surf should produce errors or warnings, got: {all_diags:?}"
    );
}

// -- E2E: Features with icons parse and render correctly ------------------

#[test]
fn e2e_features_icons_parse_and_render() {
    let input = r#"---
title: Icon Test
---

::features[cols=3]
### Speed {icon=zap}
Lightning fast performance.

### Security {icon=shield}
Enterprise-grade protection.

### Time {icon=clock}
Automatic time tracking.
::"#;

    let result = surf_parse::parse(input);
    let errors: Vec<_> = result.diagnostics.iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Should parse without errors: {:?}", errors);

    // Should produce a Features block
    let features = result.doc.blocks.iter().find(|b| matches!(b, Block::Features { .. }));
    assert!(features.is_some(), "Should contain a Features block");

    if let Some(Block::Features { cards, cols, .. }) = features {
        assert_eq!(*cols, Some(surf_parse::PerClass::uniform(3)));
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].icon.as_deref(), Some("zap"));
        assert_eq!(cards[1].icon.as_deref(), Some("shield"));
        assert_eq!(cards[2].icon.as_deref(), Some("clock"));
    }

    // Render to HTML and verify SVGs appear (not plain text)
    let html = result.doc.to_html();
    assert!(html.contains("<svg"), "HTML should contain inline SVGs");
    assert!(html.contains("surfdoc-feature-icon"), "Should have icon wrappers");
    assert!(!html.contains(">zap<"), "Should NOT render 'zap' as text");
    assert!(!html.contains(">shield<"), "Should NOT render 'shield' as text");
    assert!(!html.contains(">clock<"), "Should NOT render 'clock' as text");
    // Titles should still render
    assert!(html.contains("Speed"));
    assert!(html.contains("Security"));
    assert!(html.contains("Time"));
}

#[test]
fn e2e_features_unknown_icon_graceful() {
    let input = r#"---
title: Unknown Icon Test
---

::features[cols=2]
### Valid {icon=star}
Has a known icon.

### Invalid {icon=banana}
Has an unknown icon — should be silently omitted.
::"#;

    let result = surf_parse::parse(input);
    let html = result.doc.to_html();

    // star should render as SVG
    assert!(html.contains("<svg"), "Known icon should produce SVG");
    // banana should NOT appear as text
    assert!(!html.contains(">banana<"), "Unknown icon should not render as text");
    // Both titles should render
    assert!(html.contains("Valid"));
    assert!(html.contains("Invalid"));
}

#[test]
fn e2e_all_icons_resolvable() {
    // Every icon in the library should resolve to a valid SVG
    let all = surf_parse::icons::available_icons();
    assert!(all.len() >= 40, "Should have at least 40 icons, got {}", all.len());

    for name in all {
        let svg = get_icon(name);
        assert!(svg.is_some(), "Icon '{}' listed in available_icons() but get_icon() returns None", name);
        let svg = svg.unwrap();
        assert!(svg.starts_with("<svg"), "Icon '{}' SVG should start with <svg", name);
        assert!(svg.ends_with("</svg>"), "Icon '{}' SVG should end with </svg>", name);
        assert!(svg.contains("currentColor"), "Icon '{}' should use currentColor", name);
    }
}

// -- E2E: Plan mode app description parses and renders --------------------

#[test]
fn e2e_plan_app_description() {
    let content = read_fixture("plan-app.surf");
    let result = surf_parse::parse(&content);

    // Should parse without errors
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Unexpected errors: {errors:?}");

    // Should have app-type front matter
    let fm = result.doc.front_matter.as_ref().expect("Should have front matter");
    assert_eq!(fm.title.as_deref(), Some("Plan Mode"));

    // Count app blocks
    let filter_bars: Vec<_> = result.doc.blocks.iter()
        .filter(|b| matches!(b, Block::FilterBar { .. }))
        .collect();
    assert_eq!(filter_bars.len(), 1, "Should have 1 FilterBar block");

    let boards: Vec<_> = result.doc.blocks.iter()
        .filter(|b| matches!(b, Block::Board { .. }))
        .collect();
    assert_eq!(boards.len(), 1, "Should have 1 Board block");

    let actions: Vec<_> = result.doc.blocks.iter()
        .filter(|b| matches!(b, Block::Action { .. }))
        .collect();
    assert_eq!(actions.len(), 1, "Should have 1 Action block");

    let searches: Vec<_> = result.doc.blocks.iter()
        .filter(|b| matches!(b, Block::Search { .. }))
        .collect();
    assert_eq!(searches.len(), 1, "Should have 1 Search block");

    let dashboards: Vec<_> = result.doc.blocks.iter()
        .filter(|b| matches!(b, Block::Dashboard { .. }))
        .collect();
    assert_eq!(dashboards.len(), 1, "Should have 1 Dashboard block");

    let feeds: Vec<_> = result.doc.blocks.iter()
        .filter(|b| matches!(b, Block::Feed { .. }))
        .collect();
    assert_eq!(feeds.len(), 1, "Should have 1 Feed block");

    // Verify Board columns parsed correctly
    if let Block::Board { columns, source, .. } = &boards[0] {
        assert_eq!(columns, &["To Do", "In Progress", "Done"]);
        assert_eq!(source, "/api/tasks/board");
    }

    // Verify Action fields parsed correctly
    if let Block::Action { fields, method, target, label, .. } = &actions[0] {
        assert_eq!(*method, surf_parse::HttpMethod::Post);
        assert_eq!(target, "/api/tasks");
        assert_eq!(label, "Add Task");
        assert_eq!(fields.len(), 3);
        assert!(fields[0].required);
    }

    // Verify static HTML rendering works
    let html = result.doc.to_html();
    assert!(html.contains("surfdoc-filter-bar"), "HTML should contain filter-bar");
    assert!(html.contains("surfdoc-board"), "HTML should contain board");
    assert!(html.contains("surfdoc-action"), "HTML should contain action form");
    assert!(html.contains("surfdoc-search"), "HTML should contain search");
    assert!(html.contains("surfdoc-dashboard"), "HTML should contain dashboard");
    assert!(html.contains("surfdoc-feed"), "HTML should contain feed");
    assert!(html.contains("data-surf-source"), "HTML should contain data-surf-source attrs");
    assert!(html.contains("data-surf-stream"), "Feed should have stream flag");

    // Verify markdown degradation works
    let md = result.doc.to_markdown();
    assert!(md.contains("**Board**"), "Markdown should contain Board label");
    assert!(md.contains("To Do | In Progress | Done"), "Markdown should list columns");
    assert!(md.contains("**Add Task**"), "Markdown should contain action label");
    assert!(md.contains("**Search**"), "Markdown should contain search label");
    assert!(md.contains("**Dashboard**"), "Markdown should contain dashboard label");
    assert!(md.contains("**Feed**"), "Markdown should contain feed label");
}

#[test]
fn app_spec_html_rendering() {
    let content = read_fixture("app-spec.surf");
    let result = surf_parse::parse(&content);
    let html = result.doc.to_html();

    assert!(html.contains("surfdoc-model"), "HTML should contain model rendering");
    assert!(html.contains("surfdoc-route"), "HTML should contain route rendering");
    assert!(html.contains("surfdoc-auth"), "HTML should contain auth rendering");
    assert!(html.contains("surfdoc-binding"), "HTML should contain binding rendering");
    assert!(html.contains("Model: User"), "HTML should show User model");
    assert!(html.contains("Model: Task"), "HTML should show Task model");
    assert!(html.contains("GET"), "HTML should show GET method");
    assert!(html.contains("/api/tasks"), "HTML should show route path");
}

#[test]
fn app_spec_md_rendering() {
    let content = read_fixture("app-spec.surf");
    let result = surf_parse::parse(&content);
    let md = result.doc.to_markdown();

    assert!(md.contains("**Model: User**"), "Markdown should contain User model");
    assert!(md.contains("**Model: Task**"), "Markdown should contain Task model");
    assert!(md.contains("**GET `/api/tasks`**"), "Markdown should contain route");
    assert!(md.contains("**Authentication**"), "Markdown should contain auth");
    assert!(md.contains("**Binding**"), "Markdown should contain binding");
}

#[test]
fn app_spec_validation() {
    let content = read_fixture("app-spec.surf");
    let result = surf_parse::parse(&content);
    let diagnostics = result.doc.validate();

    // No errors expected in the well-formed fixture
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "Well-formed app spec should have no errors: {errors:?}");
}

#[test]
fn model_duplicate_fields_validation() {
    let src = r#"::model[name=BadModel]
- id: uuid [primary]
- name: string [required]
- name: string [optional]
::"#;
    let result = surf_parse::parse(src);
    let diagnostics = result.doc.validate();
    let dup_field: Vec<_> = diagnostics.iter().filter(|d| d.code.as_deref() == Some("V302")).collect();
    assert_eq!(dup_field.len(), 1, "Should detect duplicate field name");
}

#[test]
fn route_missing_path_validation() {
    let src = "::route[method=GET]\nauth: required\n::";
    let result = surf_parse::parse(src);
    let diagnostics = result.doc.validate();
    let path_errors: Vec<_> = diagnostics.iter().filter(|d| d.code.as_deref() == Some("V310")).collect();
    assert_eq!(path_errors.len(), 1, "Should error on missing route path");
}

#[test]
fn binding_missing_source_validation() {
    let src = "::binding[target=\"#list\"]\n::";
    let result = surf_parse::parse(src);
    let diagnostics = result.doc.validate();
    let source_errors: Vec<_> = diagnostics.iter().filter(|d| d.code.as_deref() == Some("V330")).collect();
    assert_eq!(source_errors.len(), 1, "Should error on missing binding source");
}

// =====================================================================
// Chunk A — Marketplace field types integration tests
// =====================================================================

/// Helper: parse a model block and extract the first model's fields.
fn parse_model_fields(src: &str) -> Vec<surf_parse::ModelField> {
    let result = surf_parse::parse(src);
    for block in &result.doc.blocks {
        if let Block::Model { fields, .. } = block {
            return fields.clone();
        }
    }
    // Check inside ::app children
    for block in &result.doc.blocks {
        if let Block::App { children, .. } = block {
            for child in children {
                if let Block::Model { fields, .. } = child {
                    return fields.clone();
                }
            }
        }
    }
    panic!("No Model block found in parsed source");
}

// --- Money type tests ---

#[test]
fn parse_money_field_type() {
    let fields = parse_model_fields("::model[name=Test]\n- price: money [required]\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Money);
    assert!(fields[0].constraints.contains(&FieldConstraint::Required));
}

#[test]
fn parse_money_alias_currency() {
    let fields = parse_model_fields("::model[name=Test]\n- price: currency [required]\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Money);
}

#[test]
fn parse_money_alias_price() {
    let fields = parse_model_fields("::model[name=Test]\n- cost: price [optional]\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Money);
}

// --- Image type tests ---

#[test]
fn parse_image_field_type() {
    let fields = parse_model_fields("::model[name=Test]\n- photo: image [optional]\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Image);
}

#[test]
fn parse_image_alias_photo() {
    let fields = parse_model_fields("::model[name=Test]\n- pic: photo\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Image);
}

#[test]
fn parse_image_alias_picture() {
    let fields = parse_model_fields("::model[name=Test]\n- pic: picture\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Image);
}

#[test]
fn parse_image_alias_img() {
    let fields = parse_model_fields("::model[name=Test]\n- pic: img\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Image);
}

// --- Email type tests ---

#[test]
fn parse_email_field_type() {
    let fields = parse_model_fields("::model[name=Test]\n- contact: email [required, unique]\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Email);
    assert!(fields[0].constraints.contains(&FieldConstraint::Required));
    assert!(fields[0].constraints.contains(&FieldConstraint::Unique));
}

#[test]
fn parse_email_alias() {
    let fields = parse_model_fields("::model[name=Test]\n- contact: email_address\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Email);
}

// --- URL type tests ---

#[test]
fn parse_url_field_type() {
    let fields = parse_model_fields("::model[name=Test]\n- website: url [optional]\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Url);
}

#[test]
fn parse_url_alias_uri() {
    let fields = parse_model_fields("::model[name=Test]\n- link: uri\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Url);
}

#[test]
fn parse_url_alias_link() {
    let fields = parse_model_fields("::model[name=Test]\n- link: link\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Url);
}

#[test]
fn parse_url_alias_href() {
    let fields = parse_model_fields("::model[name=Test]\n- link: href\n::");
    assert_eq!(fields[0].field_type, ModelFieldType::Url);
}

// --- Index constraint tests ---

#[test]
fn parse_index_constraint() {
    let fields = parse_model_fields("::model[name=Test]\n- title: string [required, index]\n::");
    assert!(fields[0].constraints.contains(&FieldConstraint::Index));
    assert!(fields[0].constraints.contains(&FieldConstraint::Required));
}

#[test]
fn parse_index_with_other_constraints() {
    let fields = parse_model_fields("::model[name=Test]\n- seller_id: ref(User) [required, index, unique]\n::");
    assert!(fields[0].constraints.contains(&FieldConstraint::Index));
    assert!(fields[0].constraints.contains(&FieldConstraint::Required));
    assert!(fields[0].constraints.contains(&FieldConstraint::Unique));
}

// --- Marketplace fixture tests ---

#[test]
fn marketplace_spec_parses() {
    let content = read_fixture("marketplace-spec.surf");
    let result = surf_parse::parse(&content);

    // Collect all model blocks (may be nested inside ::app)
    let mut models: Vec<(&str, &Vec<surf_parse::ModelField>)> = Vec::new();
    for block in &result.doc.blocks {
        if let Block::Model { name, fields, .. } = block {
            models.push((name.as_str(), fields));
        }
        if let Block::App { children, .. } = block {
            for child in children {
                if let Block::Model { name, fields, .. } = child {
                    models.push((name.as_str(), fields));
                }
            }
        }
    }

    assert_eq!(models.len(), 3, "Should have 3 models: User, Product, Order");

    let model_names: Vec<&str> = models.iter().map(|(n, _)| *n).collect();
    assert!(model_names.contains(&"User"), "Should have User model");
    assert!(model_names.contains(&"Product"), "Should have Product model");
    assert!(model_names.contains(&"Order"), "Should have Order model");

    // Check that new types are present
    let all_fields: Vec<&surf_parse::ModelField> = models.iter().flat_map(|(_, f)| f.iter()).collect();
    assert!(all_fields.iter().any(|f| f.field_type == ModelFieldType::Money), "Should have Money field");
    assert!(all_fields.iter().any(|f| f.field_type == ModelFieldType::Image), "Should have Image field");
    assert!(all_fields.iter().any(|f| f.field_type == ModelFieldType::Email), "Should have Email field");
    assert!(all_fields.iter().any(|f| f.field_type == ModelFieldType::Url), "Should have Url field");
    assert!(all_fields.iter().any(|f| f.constraints.contains(&FieldConstraint::Index)), "Should have Index constraint");
}

#[test]
fn marketplace_spec_roundtrip() {
    let content = read_fixture("marketplace-spec.surf");
    let result1 = surf_parse::parse(&content);
    let surf_source = result1.doc.to_surf_source();
    let result2 = surf_parse::parse(&surf_source);

    // Collect models from both parses
    fn collect_models(blocks: &[Block]) -> Vec<(String, Vec<surf_parse::ModelField>)> {
        let mut models = Vec::new();
        for block in blocks {
            if let Block::Model { name, fields, .. } = block {
                models.push((name.clone(), fields.clone()));
            }
            if let Block::App { children, .. } = block {
                for child in children {
                    if let Block::Model { name, fields, .. } = child {
                        models.push((name.clone(), fields.clone()));
                    }
                }
            }
        }
        models
    }

    let models1 = collect_models(&result1.doc.blocks);
    let models2 = collect_models(&result2.doc.blocks);

    assert_eq!(models1.len(), models2.len(), "Model count should match after roundtrip");
    for (m1, m2) in models1.iter().zip(models2.iter()) {
        assert_eq!(m1.0, m2.0, "Model names should match");
        assert_eq!(m1.1.len(), m2.1.len(), "Field counts should match for model {}", m1.0);
        for (f1, f2) in m1.1.iter().zip(m2.1.iter()) {
            assert_eq!(f1.name, f2.name, "Field names should match");
            assert_eq!(f1.field_type, f2.field_type, "Field types should match for {}.{}", m1.0, f1.name);
        }
    }
}

#[test]
fn marketplace_spec_html_rendering() {
    let content = read_fixture("marketplace-spec.surf");
    let result = surf_parse::parse(&content);
    let html = result.doc.to_html();

    assert!(html.contains("money"), "HTML should contain 'money' type");
    assert!(html.contains("image"), "HTML should contain 'image' type");
    assert!(html.contains("email"), "HTML should contain 'email' type");
    assert!(html.contains("url"), "HTML should contain 'url' type");
}

#[test]
fn marketplace_spec_md_rendering() {
    let content = read_fixture("marketplace-spec.surf");
    let result = surf_parse::parse(&content);
    let md = result.doc.to_markdown();

    assert!(md.contains("money"), "Markdown should contain 'money' type");
    assert!(md.contains("image"), "Markdown should contain 'image' type");
    assert!(md.contains("email"), "Markdown should contain 'email' type");
    assert!(md.contains("url"), "Markdown should contain 'url' type");
}

#[test]
fn marketplace_spec_validation() {
    let content = read_fixture("marketplace-spec.surf");
    let result = surf_parse::parse(&content);
    let diagnostics = result.doc.validate();

    // No errors should be present (some info/warnings are acceptable)
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "Marketplace spec should have no errors, got: {errors:?}");
}

#[test]
fn marketplace_index_constraint_rendering() {
    let src = "::model[name=Test]\n- title: string [required, index]\n::";
    let result = surf_parse::parse(src);
    let html = result.doc.to_html();
    assert!(html.contains("index"), "HTML should contain 'index' in constraint rendering");
}

#[test]
fn render_editor_surf_to_html() {
    // Use the editor.surf from surf-api pages (same pipeline as production).
    // Portable-dev-drive standard (2026-08-25): the canonical checkout lives
    // at /Volumes/Dev; the home-relative path is kept as a fallback for
    // machines that still carry ~/Projects.
    let home = std::env::var("HOME").expect("HOME should be set");
    let candidates = [
        "/Volumes/Dev/cloudsurf/repos/surf/pages/editor.surf".to_string(),
        format!("{home}/Projects/cloudsurf/repos/surf/pages/editor.surf"),
    ];
    let source = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("editor.surf should exist at a known checkout path");
    let result = surf_parse::parse(&source);
    assert!(!result.doc.blocks.is_empty(), "Should parse blocks");

    let html_fragment = result.doc.to_html();

    let app_css = std::fs::read_to_string(
        format!("{home}/Projects/cloudsurf/repos/surf/static/css/app.css")
    ).unwrap_or_default();

    // Wrap in surf-api shell pattern
    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Editor — Surf</title>
<style>{surfdoc_css}</style>
<style>{app_css}</style>
<style>
.editor-toolbar {{
  display: flex; align-items: center; gap: 4px; padding: 0 12px;
  height: 40px; background: var(--surface, #111118);
  border-bottom: 1px solid var(--border, #1e1e2a); flex-shrink: 0;
}}
.editor-toolbar-btn {{
  background: transparent; border: none; color: var(--text-muted, #71717a);
  cursor: pointer; padding: 4px 8px; border-radius: 4px; font-size: 13px;
  font-family: inherit;
}}
.editor-toolbar-btn:hover {{ background: rgba(255,255,255,0.05); color: var(--text); }}
.editor-toolbar-btn.primary {{
  background: var(--accent, #3b82f6); color: #fff; font-weight: 600; padding: 4px 14px;
}}
.editor-toolbar-text {{
  color: var(--text); font-weight: 500; font-size: 13px; padding: 0 8px;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}}
.editor-toolbar-spacer {{ flex: 1; }}
.editor-toolbar-sep {{ width: 1px; height: 20px; background: var(--border); margin: 0 4px; }}
.editor-badge {{
  display: inline-flex; padding: 2px 8px; border-radius: 10px;
  font-size: 11px; font-weight: 600; text-transform: uppercase;
  background: rgba(34,197,94,0.15); color: #22c55e;
}}
.surf-app {{ max-width: 100%; padding-top: 48px; }}
.surfdoc {{ max-width: 800px; margin: 0 auto; padding: 1.5rem; }}
</style>
</head>
<body>
<nav class="surf-nav">
  <a href="/" class="surf-nav-brand">Surf</a>
  <div class="surf-nav-links">
    <a href="/search" class="surf-nav-link">Search</a>
    <a href="/docs" class="surf-nav-link active">Docs</a>
    <a href="/code" class="surf-nav-link">Code</a>
    <a href="/apps" class="surf-nav-link">Apps</a>
    <a href="/chat" class="surf-nav-link">Chat</a>
  </div>
</nav>
<main class="surf-app">
  <div class="editor-toolbar">
    <button class="editor-toolbar-btn" title="Undo">&#x21A9;</button>
    <button class="editor-toolbar-btn" title="Redo">&#x21AA;</button>
    <span class="editor-toolbar-sep"></span>
    <span class="editor-toolbar-text">Bella's Dog Grooming</span>
    <span class="editor-toolbar-spacer"></span>
    <span class="editor-badge">Live</span>
    <button class="editor-toolbar-btn">Share</button>
    <button class="editor-toolbar-btn primary">Deploy</button>
  </div>
  <div class="surfdoc">{content}</div>
</main>
<script>
document.querySelectorAll('.surfdoc-tab-bar button').forEach(btn => {{
  btn.addEventListener('click', () => {{
    const tab = btn.dataset.tab;
    const parent = btn.closest('.surfdoc-tab-bar').parentElement || document;
    parent.querySelectorAll('.surfdoc-tab-bar button').forEach(b => {{
      b.classList.remove('active');
      b.setAttribute('aria-selected', 'false');
    }});
    btn.classList.add('active');
    btn.setAttribute('aria-selected', 'true');
    parent.querySelectorAll('.surfdoc-tab-content').forEach(tc => {{
      tc.style.display = tc.dataset.tab === tab ? 'block' : 'none';
    }});
  }});
}});
</script>
</body>
</html>"#,
        surfdoc_css = surf_parse::SURFDOC_CSS,
        app_css = app_css,
        content = html_fragment,
    );

    std::fs::write("/tmp/editor.html", &full_html).unwrap();
    eprintln!("\n\n  >>> Open: file:///tmp/editor.html\n");
}

// -- CalloutType::Context parser tests --------------------------------------
// Regression: ::callout[type=context] previously fell through to Info.

#[test]
fn callout_context_type_parses() {
    let src = "---\ntitle: T\ntype: doc\n---\n::callout[type=context]\nBackground.\n::";
    let result = surf_parse::parse(src);
    let callout = result
        .doc
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Callout { callout_type, .. } => Some(*callout_type),
            _ => None,
        })
        .expect("expected a Block::Callout");
    assert_eq!(callout, CalloutType::Context);
}

#[test]
fn callout_unknown_type_falls_back_to_info() {
    // Lock in existing behaviour: unrecognised type values default to Info.
    let src = "---\ntitle: T\ntype: doc\n---\n::callout[type=bogus]\nBody.\n::";
    let result = surf_parse::parse(src);
    let callout = result
        .doc
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Callout { callout_type, .. } => Some(*callout_type),
            _ => None,
        })
        .expect("expected a Block::Callout");
    assert_eq!(callout, CalloutType::Info);
}

// -- ::action-items alias tests ---------------------------------------------
// Regression: ::action-items previously fell through to Block::Unknown,
// rendering as a single collapsed paragraph of raw `- [ ]` text. It now
// aliases to the same parser as ::tasks.

#[test]
fn action_items_block_decomposes_into_task_items() {
    let src = "---\ntitle: T\ntype: doc\n---\n::action-items\n- [ ] **first**\n- [x] *second*\n::";
    let result = surf_parse::parse(src);
    let items = result
        .doc
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Tasks { items, .. } => Some(items.clone()),
            _ => None,
        })
        .expect("::action-items should produce Block::Tasks, not Unknown");
    assert_eq!(items.len(), 2, "expected 2 task items, got {}", items.len());
    assert_eq!(items[0].text, "**first**");
    assert!(!items[0].done);
    assert_eq!(items[1].text, "*second*");
    assert!(items[1].done);
}

#[test]
fn tasks_alias_still_works() {
    // Regression guard: original ::tasks dispatch must still produce Block::Tasks.
    let src = "---\ntitle: T\ntype: doc\n---\n::tasks\n- [ ] one\n::";
    let result = surf_parse::parse(src);
    let has_tasks = result
        .doc
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Tasks { .. }));
    assert!(has_tasks, "::tasks dispatch broke");
}

/// L5: `type: web` and `type: website` produce byte-identical rendered HTML
/// for the same `::site`/`::page` body — the multipage render path keys off
/// the blocks, not the doc type, so the alias must be transparent.
#[test]
fn web_and_website_render_identically() {
    let body = "\
::site
name: Acme
::

::page[route=/]
# Home

Welcome to Acme.
::

::page[route=/about]
# About

About Acme.
::
";
    let web = surf_parse::parse(&format!("---\ntitle: Acme\ntype: web\n---\n{body}"));
    let website = surf_parse::parse(&format!("---\ntitle: Acme\ntype: website\n---\n{body}"));

    assert_eq!(
        web.doc.front_matter.as_ref().unwrap().doc_type,
        Some(surf_parse::DocType::Web)
    );
    assert_eq!(
        website.doc.front_matter.as_ref().unwrap().doc_type,
        Some(surf_parse::DocType::Website)
    );

    // Multipage single-file render must be identical for both.
    let web_html = web.doc.to_html();
    let website_html = website.doc.to_html();
    assert_eq!(web_html, website_html, "web vs website HTML diverged");

    // Determinism: rendering twice yields the same bytes.
    assert_eq!(web_html, web.doc.to_html(), "render is non-deterministic");
    assert!(!web_html.is_empty());
}

/// L5: render_profile resolves the new types through the public API.
#[test]
fn render_profile_public_api() {
    use surf_parse::{DocType, Format, RenderProfile, render_profile};
    assert_eq!(render_profile(Some(DocType::Web), None), RenderProfile::Web);
    assert_eq!(
        render_profile(Some(DocType::Paper), Some(Format::Ieee)),
        RenderProfile::Paper(Format::Ieee)
    );
    assert_eq!(
        render_profile(Some(DocType::Report), Some(Format::Apa)),
        RenderProfile::Report(Format::Apa)
    );
}

/// L5: the diagrams showcase example renders every diagram kind to real SVG
/// (no prose fallback) and rendering is deterministic.
#[test]
fn diagrams_showcase_renders_all_kinds() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/showcase/diagrams-showcase.surf");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let result = surf_parse::parse(&content);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "showcase has parse errors: {errors:?}");

    let html = result.doc.to_html();
    for kind in [
        "architecture",
        "erd",
        "flowchart",
        "sequence",
        "gantt",
        "state",
        "mindmap",
        "class",
        "timeline",
        "journey",
        "quadrant",
        "kanban",
        "usecase",
        "gitgraph",
        "c4",
        "requirement",
        "sankey",
        // Chart aliases render as diagram figures wrapping chart SVG.
        "pie",
        "donut",
        "radar",
        "xychart",
    ] {
        assert!(
            html.contains(&format!("surfdoc-diagram-{kind}")),
            "missing rendered figure for diagram kind {kind}"
        );
    }
    // Every diagram produced real SVG — none degraded to the prose fallback.
    assert!(
        !html.contains("surfdoc-diagram-fallback"),
        "a diagram fell back to prose instead of rendering SVG"
    );
    // Each geometry kind produced a diagram <svg> (17 native types + the
    // mermaid-syntax block, which translates to a native flowchart); each
    // chart alias produced a chart <svg> through the ::chart pipeline.
    assert_eq!(html.matches("<svg class=\"surfdoc-diagram-svg\"").count(), 18);
    assert_eq!(html.matches("<svg class=\"surfdoc-chart-svg\"").count(), 4);

    // Determinism: byte-identical across two renders.
    assert_eq!(html, result.doc.to_html());
}

/// L5: the public diagram reference (docs/diagrams.surf) renders every one
/// of its example diagrams to real SVG — the reference never ships a prose
/// fallback.
#[test]
fn diagrams_reference_doc_renders_clean() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/diagrams.surf");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let result = surf_parse::parse(&content);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "reference doc has parse errors: {errors:?}");

    let html = result.doc.to_html();
    assert!(
        !html.contains("surfdoc-diagram-fallback"),
        "a reference diagram fell back to prose"
    );
    // 17 native examples + the mermaid example render as diagram SVG; the
    // pie chart-alias example renders as chart SVG.
    assert_eq!(html.matches("<svg class=\"surfdoc-diagram-svg\"").count(), 18);
    assert_eq!(html.matches("<svg class=\"surfdoc-chart-svg\"").count(), 1);
    assert_eq!(html, result.doc.to_html());
}

/// L5: the charts showcase example renders every chart type to real SVG from
/// inline data, the mount-point fallback still works, and output is byte-stable.
#[test]
fn charts_showcase_renders_all_types() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/showcase/charts-showcase.surf");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let result = surf_parse::parse(&content);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "showcase has parse errors: {errors:?}");

    let html = result.doc.to_html();
    for kind in [
        "line",
        "area",
        "bar",
        "stacked-bar",
        "scatter",
        "pie",
        "donut",
        "radar",
    ] {
        assert!(
            html.contains(&format!("surfdoc-chart-{kind}\"")),
            "missing rendered figure for chart type {kind}"
        );
    }
    // Eight inline-data charts each produced a real <svg> (role="img"); the
    // static preview placeholder has no role, so this counts only real charts.
    assert_eq!(
        html.matches("<svg class=\"surfdoc-chart-svg\" xmlns").count(),
        8
    );
    // The trailing data-less chart fell back to the static preview mount point.
    assert!(html.contains("surfdoc-chart-preview"));
    assert!(html.contains("data-surf-source=\"/api/metrics/wau\""));

    // Determinism: byte-identical across two renders.
    assert_eq!(html, result.doc.to_html());
}

/// L5: the citations showcase parses, resolves inline `[@key]` cites, and
/// renders an APA bibliography with anchors. Output is byte-stable.
#[test]
fn citations_showcase_renders_apa() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/showcase/citations-showcase.surf");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let result = surf_parse::parse(&content);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "showcase has parse errors: {errors:?}");

    // Four ::cite definitions + one ::bibliography parsed.
    let cites = result
        .doc
        .blocks
        .iter()
        .filter(|b| matches!(b, Block::Cite { .. }))
        .count();
    assert_eq!(cites, 4, "expected 4 ::cite blocks");
    assert!(result
        .doc
        .blocks
        .iter()
        .any(|b| matches!(b, Block::Bibliography { .. })));

    let html = result.doc.to_html();
    // Inline APA author-date citation resolved.
    assert!(html.contains("(Smith, 2020)"), "missing inline APA cite");
    assert!(html.contains("(Lee, 2021, p. 4)"), "missing inline cite w/ locator");
    // Bibliography heading + anchors.
    assert!(html.contains(">References<"), "missing References heading");
    assert!(html.contains("id=\"ref-smith2020\""), "missing bib anchor");
    assert!(html.contains("id=\"ref-garcia2022\""));
    // Anchored inline link target.
    assert!(html.contains("href=\"#ref-smith2020\""));

    // Determinism: byte-identical across two renders.
    assert_eq!(html, result.doc.to_html());
}

/// L5: the presentation showcase (`type: presentation`) renders a complete,
/// self-contained HTML deck exercising every slide layout, embedded chart +
/// diagram SVG, deck options, and speaker notes. Byte-stable across renders.
#[test]
fn presentation_showcase_renders_full_deck() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/showcase/presentation.surf");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let result = surf_parse::parse(&content);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "showcase has parse errors: {errors:?}");

    // `type: presentation` is recognized as the slides path.
    assert!(result.doc.is_presentation());

    let html = result.doc.to_slides_html();

    // Self-contained HTML deck.
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("data-slides=\"10\""));
    assert!(html.contains("id=\"bar\"")); // progress bar chrome

    // Every layout marker present.
    for cls in [
        "slide title",
        "slide section",
        "slide bullets",
        "slide two",
        "slide image",
        "slide quote",
        "slide code",
    ] {
        assert!(html.contains(cls), "missing layout marker: {cls}");
    }
    // Two-column layout produced column cells.
    assert!(html.contains("slide-cols"));

    // Deck options applied.
    assert!(html.contains("--accent:#2563eb;"));
    assert!(html.contains("data-aspect=\"16:9\""));
    assert!(html.contains("data-transition=\"fade\""));
    assert!(html.contains("Surfspace · Confidential")); // custom footer
    assert!(html.contains("<span class=\"page\">1 / 10</span>")); // slide numbers

    // Embedded chart + diagram each rendered as inline SVG within their slides.
    assert!(html.matches("<svg").count() >= 2, "expected chart + diagram SVG");
    assert!(html.contains("Weekly active users")); // chart title
    assert!(html.contains("Render pipeline")); // diagram title

    // Speaker notes rendered into the deck (hidden, fed to the notes pane by JS).
    assert!(html.contains("<aside class=\"notes\">"));
    assert!(html.contains("id=\"notes-pane\""));
    assert!(html.contains("toggleNotes")); // notes-toggle keyboard handler

    // Determinism: byte-identical across two renders.
    assert_eq!(html, result.doc.to_slides_html());
}

/// L5: IEEE numbering is assigned in citation order and used both in-text and in
/// the numbered reference list.
#[test]
fn citations_ieee_numbering_in_citation_order() {
    let src = "---\ntype: paper\nformat: ieee\n---\n\n\
::cite[key=alpha type=article]\nauthor = Zeta, Anna\ntitle = First Cited\ncontainer = J. A\nyear = 2020\n::\n\n\
::cite[key=beta type=article]\nauthor = Young, Ben\ntitle = Second Cited\ncontainer = J. B\nyear = 2019\n::\n\n\
Body text cites [@alpha] before [@beta], and again [@alpha].\n\n\
::bibliography\n::\n";
    let result = surf_parse::parse(src);
    let html = result.doc.to_html();
    // alpha first → [1], beta → [2]. Numbered styles wrap in <sup>.
    assert!(html.contains("<sup class=\"surfdoc-cite\"><a href=\"#ref-alpha\">[1]</a></sup>"));
    assert!(html.contains("<sup class=\"surfdoc-cite\"><a href=\"#ref-beta\">[2]</a></sup>"));
    // Numbered reference list, in citation order.
    assert!(html.contains("<ol class=\"surfdoc-bibliography-list\">"));
    let one = html.find("[1] A. Zeta").expect("entry 1 present");
    let two = html.find("[2] B. Young").expect("entry 2 present");
    assert!(one < two, "IEEE reference list must be in citation order");
}

/// L6 roundtrip: ::cite + ::bibliography survive serialize → parse.
#[test]
fn cite_block_roundtrips() {
    let src = "::cite[key=smith2020 type=article]\n\
author = Smith, John\ntitle = A Title\njournal = A Journal\nyear = 2020\npages = 1-9\n::\n\n\
::bibliography[style=mla]\n::\n";
    let doc = surf_parse::parse(src).doc;
    let serialized = surf_parse::builder::to_surf_source(&doc);
    let reparsed = surf_parse::parse(&serialized).doc;

    let cite = reparsed
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Cite { reference, .. } => Some(reference.clone()),
            _ => None,
        })
        .expect("cite survives roundtrip");
    assert_eq!(cite.key, "smith2020");
    assert_eq!(cite.title.as_deref(), Some("A Title"));
    assert_eq!(cite.authors.len(), 1);
    assert_eq!(cite.authors[0].family, "Smith");
    assert_eq!(cite.year.as_deref(), Some("2020"));

    let bib_style = reparsed.blocks.iter().find_map(|b| match b {
        Block::Bibliography { style, .. } => Some(*style),
        _ => None,
    });
    assert_eq!(bib_style, Some(Some(surf_parse::Format::Mla)));
}

// ── Chunk 6: paper/report rendering (Typst markup + LaTeX) ──────────────────

fn showcase(name: &str) -> surf_parse::SurfDoc {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/showcase")
        .join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let result = surf_parse::parse(&content);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{name} has parse errors: {errors:?}");
    result.doc
}

/// L5: the IEEE paper renders to a two-column Typst template and an IEEEtran
/// LaTeX document, with numbered citations and a reference list. Deterministic.
#[test]
fn paper_ieee_renders_typst_and_latex() {
    let doc = showcase("paper-ieee.surf");

    let ts = doc.to_typst();
    assert!(ts.contains("columns: 2"), "IEEE paper Typst must be two-column");
    assert!(ts.contains("Deterministic Server-Side Rendering"));
    assert!(ts.contains("Abstract."));
    assert!(ts.contains("References"));
    assert_eq!(ts, doc.to_typst(), "Typst must be byte-identical");

    let tex = doc.to_latex();
    assert!(tex.contains("{IEEEtran}"), "IEEE paper LaTeX uses IEEEtran");
    assert!(tex.contains("\\IEEEauthorblockN"));
    assert!(tex.contains("\\cite{"), "numbered in-text cite");
    assert!(tex.contains("\\begin{thebibliography}"));
    assert!(tex.contains("\\end{document}"));
    assert_eq!(tex, doc.to_latex(), "LaTeX must be byte-identical");
}

/// L5: each report style renders to Typst markup + LaTeX with the correct
/// bibliography heading.
#[test]
fn reports_render_typst_and_latex() {
    let cases = [
        ("report-mla.surf", "Works Cited"),
        ("report-apa.surf", "References"),
        ("report-chicago.surf", "Bibliography"),
    ];
    for (file, heading) in cases {
        let doc = showcase(file);

        let ts = doc.to_typst();
        assert!(ts.contains(heading), "{file} Typst missing heading {heading}");
        assert_eq!(ts, doc.to_typst(), "{file} Typst not deterministic");

        let tex = doc.to_latex();
        assert!(tex.contains("\\documentclass[12pt]{article}"), "{file} not article");
        assert!(tex.contains("\\usepackage{setspace}"), "{file} no setspace");
        assert!(tex.contains("\\doublespacing"), "{file} not double-spaced");
        assert!(
            tex.contains(&format!("\\section*{{{heading}}}")),
            "{file} LaTeX missing heading {heading}"
        );
        assert_eq!(tex, doc.to_latex(), "{file} LaTeX not deterministic");
    }
}

/// L5 (gated): the IEEE paper compiles to real PDF bytes via the Typst engine.
#[cfg(feature = "pdf")]
#[test]
fn paper_ieee_renders_pdf() {
    let doc = showcase("paper-ieee.surf");
    let config = surf_parse::PdfConfig::default();
    let pdf = doc.to_pdf(&config).expect("IEEE paper should compile to PDF");
    assert!(pdf.len() > 1000, "PDF should be substantial, got {}", pdf.len());
    assert_eq!(&pdf[..5], b"%PDF-", "PDF magic header");
}

// ── Slot markers: render-time «FILL:»/«IMG:» resolution on served sites ─────

fn parse_site(source: &str) -> (surf_parse::SiteConfig, Vec<surf_parse::PageEntry>) {
    let result = surf_parse::parse(source);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (site, pages, _loose) = surf_parse::extract_site(&result.doc);
    (site.expect("::site config"), pages)
}

/// A hero headline carrying a `«FILL: label | default»` marker renders as the
/// default text in the served page — the raw marker never reaches the visitor.
#[test]
fn site_page_hero_fill_resolved() {
    let source = "---\ntitle: \"Slot Site\"\ntype: doc\n---\n\n\
::site[domain=\"slots.test\"]\nname: Slot Site\n::\n\n\
::page[route=\"/\" title=\"Home\"]\n\n\
::hero\n\
# \u{ab}FILL: brand tag/headline | Skate or Die in One Piece\u{bb}\n\
\u{ab}FILL: subheadline | Boards built for the harbor wall\u{bb}\n\
::\n\n\
Visit us at \u{ab}FILL: street address | 12 Harbor Lane\u{bb}.\n\
::\n";
    let (site, pages) = parse_site(source);
    assert!(!pages.is_empty());
    let nav: Vec<(String, String)> = pages
        .iter()
        .map(|p| (p.route.clone(), p.title.clone().unwrap_or_default()))
        .collect();
    let config = surf_parse::PageConfig::default();

    let html = surf_parse::render_site_page(&pages[0], &site, &nav, &config);
    assert!(
        html.contains("<h1 class=\"surfdoc-hero-headline\">Skate or Die in One Piece</h1>"),
        "hero FILL default missing from the H1:\n{html}"
    );
    assert!(html.contains("Boards built for the harbor wall"));
    assert!(html.contains("12 Harbor Lane"));
    assert!(
        !html.contains('\u{ab}') && !html.contains('\u{bb}'),
        "raw slot markers leaked into served site HTML"
    );
}

/// A figure whose src is an `«IMG: description»` slot renders with the
/// self-contained placeholder image URI through the single-file site path.
#[test]
fn site_single_file_figure_img_resolved() {
    let source = "---\ntitle: \"Slot Shots\"\ntype: doc\n---\n\n\
::site[domain=\"slots.test\"]\nname: Slot Shots\n::\n\n\
::page[route=\"/\" title=\"Home\"]\n\n\
::figure[src=\"\u{ab}IMG: storefront at dusk\u{bb}\" caption=\"Our storefront\"]\n::\n\n\
\u{ab}IMG: skater mid-air over the harbor wall\u{bb}\n\
::\n";
    let (site, pages) = parse_site(source);
    assert!(!pages.is_empty());
    let nav: Vec<(String, String)> = pages
        .iter()
        .map(|p| (p.route.clone(), p.title.clone().unwrap_or_default()))
        .collect();
    let config = surf_parse::PageConfig::default();

    let html = surf_parse::render_site_single_file(&site, &pages, &nav, &config);
    assert!(
        html.contains(&format!("src=\"{}\"", surf_parse::IMG_SLOT_PLACEHOLDER_URI)),
        "figure IMG slot did not resolve to the placeholder URI"
    );
    assert!(html.contains("Our storefront"));
    // The loose IMG marker in text becomes a placeholder <img> with alt text.
    assert!(html.contains("surfdoc-img-slot"));
    assert!(html.contains("alt=\"skater mid-air over the harbor wall\""));
    assert!(
        !html.contains('\u{ab}') && !html.contains('\u{bb}'),
        "raw slot markers leaked into served site HTML"
    );
}

// ── p2-style-refresh: SURFDOC_CSS contract guards ───────────────────────────
// The generated-site block styles were retuned to the current Surfspace
// design language. These guards pin the non-negotiables: pricing tiers always
// carry CTAs, no accent left-stripe callouts, the layout=cover hero contract
// (incl. the [data-bg] darkening overlay) survives, and p1's image-slot hook
// stays styled.

#[test]
fn style_refresh_pricing_cta_rules_present() {
    assert!(
        surf_parse::SURFDOC_CSS.contains(".surfdoc .surfdoc-tier-cta-primary"),
        "pricing primary CTA rule missing from SURFDOC_CSS"
    );
    assert!(
        surf_parse::SURFDOC_CSS.contains(".surfdoc .surfdoc-tier-cta-secondary"),
        "pricing secondary CTA rule missing from SURFDOC_CSS"
    );
}

#[test]
fn style_refresh_no_accent_left_stripe() {
    // Every `border-left` in the stylesheet must be a structural hairline
    // (var(--border) rail) or an explicit `none` reset — never an accent
    // stripe. Callouts/summaries are full-border tinted cards by design.
    let css = surf_parse::SURFDOC_CSS;
    let mut checked = 0;
    for (idx, _) in css.match_indices("border-left:") {
        let rest = &css[idx + "border-left:".len()..];
        let value = rest.split(&[';', '}'][..]).next().unwrap_or("").trim();
        assert!(
            value.contains("var(--border") || value == "none",
            "border-left value `{value}` is not a structural var(--border) rail"
        );
        assert!(
            !value.contains("--accent"),
            "accent left-stripe reintroduced: `{value}`"
        );
        checked += 1;
    }
    assert!(checked > 0, "expected at least one border-left declaration");
}

#[test]
fn style_refresh_cover_hero_contract_intact() {
    assert!(
        surf_parse::SURFDOC_CSS.contains(".surfdoc-hero-cover"),
        "layout=cover hero rule missing from SURFDOC_CSS"
    );
    assert!(
        surf_parse::SURFDOC_CSS.contains(".surfdoc-hero[data-bg]::before"),
        "hero [data-bg] darkening overlay missing from SURFDOC_CSS"
    );
}

#[test]
fn style_row_anchor_underline_suppressed() {
    // G10: link rows are anchors; the prose `.surfdoc a:hover` underline must
    // never leak onto the row title/meta. The suppression rule covers the
    // anchor and its descendant spans for all states including hover.
    let css = surf_parse::SURFDOC_CSS;
    assert!(
        css.contains(".surfdoc a.surfdoc-row, .surfdoc a.surfdoc-row:hover"),
        "row anchor-decoration suppression selector missing from SURFDOC_CSS"
    );
    assert!(
        css.contains(".surfdoc a.surfdoc-row .surfdoc-row-title"),
        "row title must be covered by the anchor-decoration suppression rule"
    );
    assert!(
        css.contains(".surfdoc a.surfdoc-row .surfdoc-row-trailing"),
        "row trailing control must be covered by the anchor-decoration suppression rule"
    );
}

#[test]
fn style_refresh_img_slot_rule_present() {
    assert!(
        surf_parse::SURFDOC_CSS.contains(".surfdoc-img-slot"),
        "p1 image-slot placeholder rule missing from SURFDOC_CSS"
    );
}

#[test]
fn style_refresh_cards_differ_from_muted_and_dark_sections() {
    // Regression for the invisible-card defect: .section-muted resolves to
    // var(--surface) and .section-dark resolves to var(--surface-alt), so any
    // card background token that is literally EITHER of those collides with
    // one of the two ambient sections Features/Stats/Pricing/Product commonly
    // nest inside. The card token must be a blend of both, not equal to either.
    let css = surf_parse::SURFDOC_CSS;
    assert!(
        css.contains(".section-muted { background: var(--surface);"),
        "test assumption stale: .section-muted no longer resolves to var(--surface)"
    );
    assert!(
        css.contains(".section-dark { background: var(--surface-alt);"),
        "test assumption stale: .section-dark no longer resolves to var(--surface-alt)"
    );
    assert!(
        css.contains("--ws-feature-card-bg: color-mix(in srgb, var(--surface) 50%, var(--surface-alt) 50%)"),
        "card background token must blend --surface and --surface-alt so it differs from both .section-muted and .section-dark"
    );
    for (selector, rule) in [
        (".surfdoc-tier", ".surfdoc-tier {"),
        (".surfdoc-stat", ".surfdoc-stat {"),
        (".surfdoc-product-card", ".surfdoc-product-card {"),
    ] {
        let idx = css
            .find(rule)
            .unwrap_or_else(|| panic!("rule `{rule}` not found in SURFDOC_CSS"));
        let decl_end = css[idx..].find('}').unwrap() + idx;
        let decl = &css[idx..decl_end];
        assert!(
            decl.contains("background: var(--ws-feature-card-bg)"),
            "{selector} must use var(--ws-feature-card-bg) so it can't literally match a .section-muted/.section-dark background"
        );
        assert!(
            !decl.contains("background: var(--surface);")
                && !decl.contains("background: var(--surface-alt);"),
            "{selector} background must not be the bare --surface or --surface-alt token"
        );
    }
}

// ── p3-copy-sweep: no unresolved slot/markup leakage in served site HTML ────

/// Drop `<style>…</style>` and `<script>…</script>` spans before asserting on
/// visible copy — SURFDOC_CSS legitimately contains `::before`, `{`, etc.
fn strip_style_and_script(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let style = rest.find("<style");
        let script = rest.find("<script");
        let (start, close) = match (style, script) {
            (Some(a), Some(b)) if a < b => (a, "</style>"),
            (Some(a), None) => (a, "</style>"),
            (_, Some(b)) => (b, "</script>"),
            (None, None) => break,
        };
        out.push_str(&rest[..start]);
        match rest[start..].find(close) {
            Some(end_rel) => rest = &rest[start + end_rel + close.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Combined adversarial sweep: every reproduced leak class in one Mako-shaped
/// site doc, rendered through the exact serving path (parse → extract_site →
/// render_site_page). The body must be free of directive/attr/anchor/marker
/// artifacts while the real copy survives.
#[test]
fn copy_sweep_site_render_no_leaks() {
    let source = "---\ntitle: \"Nasty Site\"\ntype: doc\n---\n\n\
::site[domain=\"nasty.test\"]\nname: Nasty Site\n::\n\n\
::page[route=\"/\" title=\"Home\"]\n\n\
:: hero[title=\"About Us\" align=center]\n\
# \u{ab}FILL: brand tag \u{2014} Skate or Die\u{bb}\n\
Boards built for the harbor wall.\n\
::\n\n\
::features\n\
[cols=3 title=\"Why us\"]\n\
### Fast\nQuick turnaround.\n\
### Safe\nCertified gear.\n\
### Local\nHarbor-born.\n\
::\n\n\
::team\n\
## Crew\n\
- Ada\n- Lin\n\
::\n\n\
## Our Menu {#our-menu}\n\n\
After text.\n\
::\n\
::\n";
    let result = surf_parse::parse(source);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let (site, pages, _loose) = surf_parse::extract_site(&result.doc);
    let site = site.expect("::site config");
    assert!(!pages.is_empty(), "no pages extracted");
    let nav: Vec<(String, String)> = pages
        .iter()
        .map(|p| (p.route.clone(), p.title.clone().unwrap_or_default()))
        .collect();
    let config = surf_parse::PageConfig::default();

    let html = surf_parse::render_site_page(&pages[0], &site, &nav, &config);
    let body = strip_style_and_script(&html);

    // Class 1/2: no directive lines or orphan closers as visible text.
    assert!(
        !body.lines().any(|l| l.trim() == "::" || l.trim().starts_with(":: ")),
        "directive/closer leaked as text:\n{body}"
    );
    assert!(!body.contains("title=\"About Us\""), "attr title leaked: {body}");
    // Class 4: displaced attr line is adopted, not body copy.
    assert!(!body.contains("[cols="), "attrs-on-own-line leaked: {body}");
    // Class 5: FILL em-dash marker resolved to its default.
    assert!(
        !body.contains('\u{ab}') && !body.contains('\u{bb}'),
        "raw slot markers leaked: {body}"
    );
    assert!(body.contains("Skate or Die"), "FILL default missing: {body}");
    // Class 3: unknown block content rendered as markdown, not raw dump.
    assert!(!body.contains("## Crew"), "literal ## leaked: {body}");
    assert!(body.contains("Crew"), "unknown-block copy lost: {body}");
    // Class 6: explicit heading anchor becomes the id, never visible copy.
    assert!(!body.contains("{#"), "heading anchor braces leaked: {body}");
    assert!(html.contains("id=\"our-menu\""), "explicit anchor id missing: {html}");
    // Copy survives the sweep.
    assert!(body.contains("Boards built for the harbor wall"), "{body}");
    assert!(body.contains("Quick turnaround"), "{body}");
    assert!(body.contains("After text."), "{body}");
}

/// Class 7 pin: parse diagnostics (unclosed directive P001, orphan closer
/// P006) are a side channel — their message text must never surface in the
/// rendered site HTML, and the affected content still renders.
#[test]
fn copy_sweep_warnings_never_in_html() {
    let source = "---\ntitle: \"Warn Site\"\ntype: doc\n---\n\n\
::site[domain=\"warn.test\"]\nname: Warn Site\n::\n\n\
::page[route=\"/\" title=\"Home\"]\n\n\
::features\n\
### Fast\nQuick turnaround.\n";
    let result = surf_parse::parse(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("P001")),
        "expected an unclosed-directive warning: {:?}",
        result.diagnostics
    );
    let (site, pages, _loose) = surf_parse::extract_site(&result.doc);
    let site = site.expect("::site config");
    assert!(!pages.is_empty(), "no pages extracted");
    let nav: Vec<(String, String)> = pages
        .iter()
        .map(|p| (p.route.clone(), p.title.clone().unwrap_or_default()))
        .collect();
    let html =
        surf_parse::render_site_page(&pages[0], &site, &nav, &surf_parse::PageConfig::default());

    assert!(
        !html.contains("Unclosed block directive"),
        "P001 message text leaked into HTML: {html}"
    );
    assert!(!html.contains("P001") && !html.contains("P006"), "{html}");
    assert!(html.contains("Quick turnaround"), "content still renders: {html}");
}


/// 0.14 responsive shell (WP-R1/WP-R2, rulings R-A/R-D): a shell.surf-shaped
/// document — sidebar rows + divider + hub rows + topbar + tab-content +
/// right panel — renders the whole responsive kit together: the Surfy
/// drawer, the generated tab-bar (surface rows only), the FAB, and the
/// closed-by-default state attributes.
#[test]
fn responsive_shell_renders_drawer_tabbar_fab_and_state() {
    let source = "\
::app-shell[layout=sidebar-main-panel height=720]\n\
:::sidebar[position=left width=240]\n\
::::toolbar\n- text[value=\"Surfspace\" size=22]\n::::\n\
::::row[icon=doc href=#]\nDocs\n::::\n\
::::row[icon=task href=#]\nTasks\n::::\n\
::::row[icon=apps href=#]\nApps\n::::\n\
::::row[icon=knowledge href=# unread=true]\nMessages\n::::\n\
::::divider\n::::\n\
::::row[icon=posts href=#]\nPosts\n::::\n\
::::row[icon=settings href=#]\nSettings\n::::\n\
:::\n\
:::toolbar\n\
- button[label=\"Search\" icon=search action=openSearch]\n\
- separator\n\
- button[label=\"Surfy\" icon=surfy-fin action=toggleSurfy]\n\
:::\n\
:::tab-content[tab=main]\nBody\n:::\n\
:::panel[position=right]\nSurfy thread\n:::\n\
::\n";
    let result = surf_parse::parse(source);
    let html = result.doc.to_html();

    // Shell root: closed drawer state stamped (default CLOSED, ruling R-D).
    assert!(html.contains("data-panel-open=\"false\""), "{html}");

    // Drawer: right panel in drawer shape with fixed-width inner wrapper.
    assert!(html.contains("surfdoc-panel surfdoc-panel-right"));
    assert!(html.contains("aria-hidden=\"true\""));
    assert!(html.contains("<div class=\"surfdoc-panel-inner\">"));
    assert!(html.contains("Surfy thread"));

    // Drawer anatomy (0.14 WP-N0): ONE head row (fin glyph), the grounding
    // chip directly below it, then the flex-1 body region carrying the
    // panel's source children — in that order, even for a panel with no
    // dropdown/toolbar/composer children.
    assert_eq!(html.matches("surfdoc-panel-head").count(), 1, "one head row");
    let head = html.find("surfdoc-panel-head").unwrap();
    let ground = html.find("surfdoc-panel-grounding").expect("grounding chip");
    let body_region = html.find("surfdoc-panel-body").expect("body region");
    let thread = html.find("Surfy thread").unwrap();
    assert!(head < ground && ground < body_region && body_region < thread);
    assert!(html.contains("<span class=\"surfdoc-panel-fin\" aria-hidden=\"true\">"));

    // Generated tab-bar: the four surface rows, hub rows excluded.
    assert!(html.contains("<nav class=\"surfdoc-app-tabbar\""));
    for tab in ["docs", "tasks", "apps", "messages"] {
        assert!(html.contains(&format!("data-tab=\"{tab}\"")), "missing {tab}");
    }
    assert!(!html.contains("data-tab=\"posts\""));
    assert!(!html.contains("data-tab=\"settings\""));

    // FAB markup; the toggle script is GONE at 0.19.0 (runtime-owned —
    // spec web-runtime-v1 §2), so the shell is constructively coverable.
    assert!(html.contains("surfdoc-panel-fab"));
    assert!(html.contains("aria-controls=\"surfdoc-panel-right\""));
    assert!(!html.contains("__surfyWired"));
    assert!(!html.contains("localStorage"));
}

/// 0.14 drawer polish (WP-N0/WP-N1, cross-lane contract): a shell.surf-shaped
/// right panel — tier dropdown-select + toolbar + prose + chat-input-simple —
/// composes to the ruled drawer anatomy regardless of source block order:
/// one head row (fin + selected-value-only tier switcher + toolbar items),
/// the grounding chip with the pinned selectors, body, and the composer
/// LAST with the attach control before the input. Bottom panels keep the
/// generic chat-input markup byte-identical.
#[test]
fn surfy_drawer_head_grounding_and_composer_contract() {
    let source = "\
::panel[position=right width=360]\n\
:::dropdown-select[name=surfy-tier icon=surfy-fin label=\"Surfy Standard\" selected=\"Standard\" action=switchTier]\n\
- \"Standard\" value=standard action=switchTierStandard\n\
- \"Max\" value=max action=switchTierMax\n\
:::\n\
:::toolbar\n\
- spacer\n\
- button[label=\"Chats\" action=openSurfyChats]\n\
- button[label=\"\u{2715}\" action=dismissSurfy]\n\
:::\n\
Context inherits here.\n\
:::chat-input-simple[placeholder=\"Message Surfy...\" action=sendToSurfy]\n\
:::\n\
::\n";
    let result = surf_parse::parse(source);
    let html = result.doc.to_html();

    // B2: ONE head row — the toolbar's items are inlined into it, so no
    // nested `surfdoc-toolbar` wrapper renders inside the drawer.
    assert_eq!(html.matches("surfdoc-panel-head").count(), 1, "{html}");
    assert!(!html.contains("<div class=\"surfdoc-toolbar\""), "{html}");
    let head = html.find("<div class=\"surfdoc-panel-head\">").unwrap();
    let fin = html.find("surfdoc-panel-fin").unwrap();
    let dropdown = html.find("surfdoc-dropdown-select").unwrap();
    let spacer = html.find("surfdoc-toolbar-spacer").unwrap();
    let chats = html.find(">Chats</button>").unwrap();
    let close = html.find("data-action=\"dismissSurfy\"").unwrap();
    assert!(head < fin && fin < dropdown && dropdown < spacer && spacer < chats && chats < close);

    // B1: the tier switcher title is ONLY the selected value — no label
    // duplication — and the caret span is present for the is-open flip.
    assert!(html.contains("<span class=\"surfdoc-dropdown-selected\">Standard</span>"), "{html}");
    assert!(!html.contains("surfdoc-dropdown-label"), "{html}");
    assert!(!html.contains(">Surfy Standard<"), "{html}");
    assert!(html.contains("surfdoc-dropdown-caret"));
    assert!(surf_parse::SURFDOC_CSS
        .contains(".surfdoc-panel-head .surfdoc-dropdown-select.is-open .surfdoc-dropdown-caret"));

    // D1: grounding chip — exact cross-lane contract markup, hidden, in the
    // panel head area directly below the head row.
    let chip = "<div class=\"surfdoc-panel-grounding\" hidden>\
                <span class=\"surfdoc-grounding-label\"></span>\
                <button type=\"button\" class=\"surfdoc-grounding-clear\" \
                data-action=\"clearSurfyGrounding\" aria-label=\"Clear grounding\">"
        .replace("                ", "");
    assert!(html.contains(&chip), "{html}");

    // B5: body takes the remaining space; the composer renders LAST, after
    // the body region, whatever the source order.
    let body_region = html.find("<div class=\"surfdoc-panel-body\">").unwrap();
    let prose = html.find("Context inherits here.").unwrap();
    let composer = html.find("<div class=\"surfdoc-chat-input\">").unwrap();
    assert!(body_region < prose && prose < composer, "{html}");

    // D2: attach control before the input, contract selectors; Send keeps
    // its own button (no gray blanket class).
    let attach = html
        .find("<button type=\"button\" class=\"surfdoc-chat-attach\" data-action=\"attachToSurfy\" aria-label=\"Attach\">")
        .expect("attach control");
    let input = html.find("<input type=\"text\" placeholder=\"Message Surfy...\">").unwrap();
    let send = html.find("<button data-action=\"sendToSurfy\">Send</button>").unwrap();
    assert!(attach < input && input < send, "{html}");

    // Bottom panels: generic chat-input markup stays byte-identical — no
    // attach control, no drawer anatomy.
    let bottom = surf_parse::parse(
        "::panel[position=bottom height=200]\n\
         :::chat-input-simple[placeholder=\"Ask\" action=send]\n\
         :::\n\
         ::\n",
    )
    .doc
    .to_html();
    assert!(bottom.contains("<div class=\"surfdoc-chat-input\"><input type=\"text\" placeholder=\"Ask\"><button data-action=\"send\">Send</button></div>"), "{bottom}");
    assert!(!bottom.contains("surfdoc-chat-attach"));
    assert!(!bottom.contains("surfdoc-panel-head"));
    assert!(!bottom.contains("surfdoc-panel-grounding"));
}

/// Regression: `::summary` must never emit a `<p>` nested inside a `<p>`.
///
/// The pre-0.19.0 arm wrapped the body in a bare `<p>{render_inline_markdown(..)}</p>`,
/// which produced `<p><p>text</p>\n</p>` for the ordinary phrasing-only summary.
/// That is not merely ugly: an HTML parser auto-closes the outer `<p>` at the
/// inner one, so the SSR-parsed DOM disagreed with the literal element tree
/// `render_dom` constructs — the byte-identity backends could not both be right.
/// The golden corpus pinned the malformed bytes, but `tests/corpus.rs` is
/// `#![cfg(feature = "native")]`, so neither the default nor the dom-only run
/// could see it. This test is feature-free on purpose.
#[test]
fn summary_never_nests_paragraphs() {
    // Phrasing-only body: exactly one <p>, collapsed — not <p><p>.
    let html = surf_parse::parse("::summary\nOne source, every target.\n::\n")
        .doc
        .to_html();
    assert!(
        !html.contains("<p><p>"),
        "summary emitted a nested paragraph: {html}"
    );
    assert_eq!(
        html.matches("<p>").count(),
        1,
        "phrasing-only summary must collapse to a single <p>: {html}"
    );
    assert!(
        html.contains(
            "<div class=\"surfdoc-summary\" role=\"doc-abstract\">\
             <div class=\"surfdoc-summary-label\">Summary</div>\
             <p>One source, every target.</p></div>"
        ),
        "{html}"
    );

    // Block-level body (two paragraphs) must not be forced into a <p> at all;
    // it gets the <div> wrapper, still with no nesting.
    let multi =
        surf_parse::parse("::summary\nFirst paragraph.\n\nSecond paragraph.\n::\n")
            .doc
            .to_html();
    assert!(
        !multi.contains("<p><p>"),
        "multi-paragraph summary nested a paragraph: {multi}"
    );
    assert!(
        multi.contains("<div class=\"surfdoc-summary-label\">Summary</div><div>"),
        "block-level summary body must use the <div> wrapper: {multi}"
    );
    assert!(multi.contains("<p>First paragraph.</p>"), "{multi}");
    assert!(multi.contains("<p>Second paragraph.</p>"), "{multi}");
}

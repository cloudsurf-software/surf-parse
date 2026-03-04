//! Integration tests that parse complete fixture files end-to-end.

use surf_parse::{Block, Severity, icons::get_icon};

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
        assert_eq!(*cols, Some(3));
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
fn app_spec_fixture_parses() {
    let content = read_fixture("app-spec.surf");
    let result = surf_parse::parse(&content);

    // Should parse without errors
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Unexpected errors: {errors:?}");

    // Should have front matter
    let fm = result.doc.front_matter.as_ref().expect("Should have front matter");
    assert_eq!(fm.title.as_deref(), Some("Task Manager App"));

    // Should have App block with children
    let app_blocks: Vec<_> = result.doc.blocks.iter().filter(|b| matches!(b, Block::App { .. })).collect();
    assert_eq!(app_blocks.len(), 1, "Should have exactly 1 app block");

    // Extract manifest
    let manifest = result.doc.extract_manifest().expect("Should extract manifest");
    assert_eq!(manifest.name, "task-manager");
    assert_eq!(manifest.port, Some(8080));

    // Models
    assert_eq!(manifest.models.len(), 2, "Should have User and Task models");
    assert_eq!(manifest.models[0].name, "User");
    assert_eq!(manifest.models[1].name, "Task");

    // Check User model fields
    let user = &manifest.models[0];
    assert_eq!(user.fields.len(), 5, "User should have 5 fields");
    assert_eq!(user.fields[0].name, "id");
    assert_eq!(user.fields[1].name, "email");

    // Check Task model has ref to User
    let task = &manifest.models[1];
    assert_eq!(task.fields.len(), 7, "Task should have 7 fields");
    let assignee = &task.fields[4];
    assert_eq!(assignee.name, "assignee_id");
    assert!(matches!(&assignee.field_type, surf_parse::ModelFieldType::Ref(t) if t == "User"));

    // Routes
    assert_eq!(manifest.routes.len(), 4, "Should have 4 routes");
    assert_eq!(manifest.routes[0].path, "/api/tasks");
    assert_eq!(manifest.routes[0].method, surf_parse::HttpMethod::Get);
    assert_eq!(manifest.routes[1].method, surf_parse::HttpMethod::Post);
    assert_eq!(manifest.routes[2].method, surf_parse::HttpMethod::Put);
    assert_eq!(manifest.routes[3].method, surf_parse::HttpMethod::Delete);

    // Auth
    assert!(manifest.auth.is_some(), "Should have auth config");
    let auth = manifest.auth.as_ref().unwrap();
    assert_eq!(auth.provider, surf_parse::AuthProvider::Email);
    assert_eq!(auth.roles.len(), 2);
    assert_eq!(auth.default_role.as_deref(), Some("member"));

    // Bindings
    assert_eq!(manifest.bindings.len(), 1, "Should have 1 binding");
    assert_eq!(manifest.bindings[0].source, "/api/tasks");
    assert_eq!(manifest.bindings[0].target, "#task-list");
    assert_eq!(manifest.bindings[0].events.len(), 3);

    // Database & Deploy
    assert!(manifest.database.is_some());
    assert_eq!(manifest.deploys.len(), 1);
    assert!(manifest.health.is_some());
}

#[test]
fn app_spec_roundtrip() {
    let content = read_fixture("app-spec.surf");
    let result1 = surf_parse::parse(&content);
    let surf_source = result1.doc.to_surf_source();
    let result2 = surf_parse::parse(&surf_source);

    // Verify manifests match after round-trip
    let m1 = result1.doc.extract_manifest().expect("manifest 1");
    let m2 = result2.doc.extract_manifest().expect("manifest 2");

    assert_eq!(m1.name, m2.name);
    assert_eq!(m1.models.len(), m2.models.len(), "Model count should match");
    assert_eq!(m1.routes.len(), m2.routes.len(), "Route count should match");
    assert_eq!(m1.bindings.len(), m2.bindings.len(), "Binding count should match");
    assert!(m1.auth.is_some() && m2.auth.is_some(), "Auth should round-trip");

    for (a, b) in m1.models.iter().zip(m2.models.iter()) {
        assert_eq!(a.name, b.name, "Model names should match");
        assert_eq!(a.fields.len(), b.fields.len(), "Field count should match for model {}", a.name);
    }
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
fn model_foreign_key_unresolved_validation() {
    let src = r#"::model[name=Task]
- id: uuid [primary]
- owner_id: ref(NonExistentModel) [optional]
::"#;
    let result = surf_parse::parse(src);
    let diagnostics = result.doc.validate();
    let ref_warns: Vec<_> = diagnostics.iter().filter(|d| d.code.as_deref() == Some("V303")).collect();
    assert_eq!(ref_warns.len(), 1, "Should warn about unresolved foreign key ref");
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

//! Integration tests that parse complete fixture files end-to-end.

use surf_parse::{Block, Severity};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures")
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

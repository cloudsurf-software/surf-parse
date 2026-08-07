use std::collections::BTreeSet;
use serde::Deserialize;

/// Canonical list of Block enum variants (excluding Unknown, Markdown).
/// When you add a variant to types.rs, add it here too — or this test fails.
const ENUM_VARIANTS: &[&str] = &[
    // Core document blocks
    "Callout", "Code", "Data", "Decision", "Details", "Diagram", "Figure",
    "Metric", "Quote", "Summary", "Tasks",
    // Layout
    "Columns", "Divider", "Section", "Tabs",
    // Web / landing page
    "BeforeAfter", "Comparison", "Cta", "Embed", "Faq", "Features",
    "Footer", "Form", "Gallery", "Gate", "Hero", "HeroImage", "Logo",
    "Nav", "Pipeline", "PostGrid", "PricingTable", "ProductCard", "Site", "Page",
    "Stats", "Steps", "Style", "Testimonial", "Toc",
    // App description (data-bound UI)
    "Action", "Board", "ChatInput", "Dashboard", "Feed", "FilterBar",
    "List", "Search",
    // Compound widget mount points
    "Chart", "Editor", "SplitPane",
    // Infrastructure manifest
    "App", "Auth", "Binding", "Build", "Cicd", "Concurrency",
    "Crates", "Deploy", "DeployUrls", "Domains", "Health",
    "InfraDatabase", "InfraEnv", "Model", "Route", "Schema",
    "Smoke", "Use", "Volumes",
    // App format
    "AppDeploy", "AppEnv",
    // Compact display
    "Badge", "InfoCard", "Row",
    // App shell / interactive
    "AppShell", "BlockEditor", "ChatInputSimple", "ChatThread",
    "CodeEditor", "CommandPalette", "Drawer", "DropdownSelect", "LogStream", "Modal",
    "NavTree", "Panel", "ProblemList", "Progress", "Sidebar",
    "SuggestionChips", "TabBar", "TabContent", "Terminal", "Toolbar",
];

#[derive(Debug, Deserialize)]
struct Registry {
    meta: RegistryMeta,
    blocks: std::collections::BTreeMap<String, BlockEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryMeta {
    spec_version: String,
    total_blocks: usize,
    #[allow(dead_code)]
    registry_updated: String,
}

#[derive(Debug, Deserialize)]
struct BlockEntry {
    status: String,
    enum_variant: String,
    #[allow(dead_code)]
    purpose: String,
    #[allow(dead_code)]
    category: String,
    #[allow(dead_code)]
    attributes: Vec<String>,
    #[allow(dead_code)]
    degradation: String,
}

#[test]
fn spec_compliance() {
    let registry: Registry = toml::from_str(
        include_str!("../spec/blocks.toml")
    ).expect("spec/blocks.toml must parse as valid TOML");

    let enum_set: BTreeSet<&str> = ENUM_VARIANTS.iter().copied().collect();

    let registry_implemented: BTreeSet<&str> = registry.blocks.values()
        .filter(|b| b.status == "implemented")
        .map(|b| b.enum_variant.as_str())
        .collect();

    let registry_all: BTreeSet<&str> = registry.blocks.values()
        .map(|b| b.enum_variant.as_str())
        .collect();

    // CHECK 1: Every code variant must be in the spec (HARD FAIL)
    let code_without_spec: Vec<&&str> = enum_set.difference(&registry_all).collect();
    assert!(
        code_without_spec.is_empty(),
        "\n\nSPEC VIOLATION: Block variants exist in code but are NOT defined in the spec:\n  \
         {:?}\n\n\
         The spec is the authority. Either:\n  \
         1. Add these blocks to spec/blocks.toml (if the spec should define them)\n  \
         2. Remove these variants from types.rs (if they shouldn't exist)\n",
        code_without_spec
    );

    // CHECK 2: Every "implemented" registry entry must have a code variant (HARD FAIL)
    let registry_without_code: Vec<&&str> = registry_implemented.difference(&enum_set).collect();
    assert!(
        registry_without_code.is_empty(),
        "\n\nREGISTRY LIE: blocks.toml says these are implemented, but no Block variant exists:\n  \
         {:?}\n\n\
         Either implement the variant in types.rs or change status to \"planned\" in blocks.toml.\n",
        registry_without_code
    );

    // CHECK 3: Planned blocks not yet implemented (WARNING, not failure)
    let planned: Vec<(&String, &str)> = registry.blocks.iter()
        .filter(|(_, b)| b.status == "planned")
        .map(|(name, b)| (name, b.enum_variant.as_str()))
        .collect();

    if !planned.is_empty() {
        eprintln!("\n--- Spec backlog: {} blocks defined but not yet implemented ---", planned.len());
        for (name, variant) in &planned {
            eprintln!("  ::{}  (Block::{})", name, variant);
        }
        eprintln!("---\n");
    }

    // META CHECK: total_blocks matches actual count
    assert_eq!(
        registry.meta.total_blocks,
        registry.blocks.len(),
        "meta.total_blocks ({}) doesn't match actual block count ({})",
        registry.meta.total_blocks,
        registry.blocks.len()
    );
}

// ------------------------------------------------------------------
// Lint rule registry (spec/rules.toml) drift checks
// ------------------------------------------------------------------

#[test]
fn lint_rule_registry_matches_implementation() {
    let registry = surf_parse::lint::rule_registry();

    // CHECK 1: every style-layer registry entry has an implemented rule, and
    // every implemented rule is registered (bidirectional).
    let implemented: BTreeSet<&str> = surf_parse::lint::all_rules()
        .iter()
        .map(|r| r.id())
        .collect();
    let style_ids: BTreeSet<&str> = registry
        .iter()
        .filter(|(_, m)| m.layer == "style")
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        implemented, style_ids,
        "\n\nRULE DRIFT: style rules in spec/rules.toml and src/lint.rs::all_rules() \
         must match exactly.\n  registered: {style_ids:?}\n  implemented: {implemented:?}\n"
    );

    // CHECK 2: syntax-layer registry entries match the P-codes the parser emits.
    let syntax_ids: BTreeSet<&str> = registry
        .iter()
        .filter(|(_, m)| m.layer == "syntax")
        .map(|(id, _)| id.as_str())
        .collect();
    let parse_ids: BTreeSet<&str> = surf_parse::lint::PARSE_RULE_IDS.iter().copied().collect();
    assert_eq!(
        syntax_ids, parse_ids,
        "syntax rules in spec/rules.toml must match lint::PARSE_RULE_IDS"
    );

    // CHECK 3: id prefix must match layer; fix_safety only on fixable rules.
    for (id, meta) in registry {
        let expected_prefix = match meta.layer.as_str() {
            "syntax" => 'P',
            "style" => 'L',
            other => panic!(
                "rule {id}: unknown layer {other:?} (V-codes live in validate.rs, not the registry)"
            ),
        };
        assert!(
            id.starts_with(expected_prefix),
            "rule {id}: layer {:?} requires a {expected_prefix}-prefixed id",
            meta.layer
        );
        assert_eq!(
            meta.fix_safety.is_some(),
            meta.fixable,
            "rule {id}: fix_safety must be set exactly when fixable"
        );
        if let Some(safety) = &meta.fix_safety {
            assert!(
                safety == "safe" || safety == "suggested",
                "rule {id}: fix_safety must be \"safe\" or \"suggested\", got {safety:?}"
            );
        }
    }

    // META CHECK: total_rules matches actual count.
    assert_eq!(
        surf_parse::lint::rule_registry_total(),
        registry.len(),
        "meta.total_rules doesn't match actual rule count"
    );
}

#[test]
fn lint_known_block_names_come_from_blocks_toml() {
    let registry: Registry = toml::from_str(include_str!("../spec/blocks.toml"))
        .expect("spec/blocks.toml must parse as valid TOML");
    let spec_names: BTreeSet<&str> = registry.blocks.keys().map(String::as_str).collect();

    // L020's known-block set is exactly the spec registry (any status).
    let known: BTreeSet<&str> = surf_parse::lint::known_block_names()
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(
        known, spec_names,
        "lint::known_block_names() must be exactly the spec/blocks.toml names"
    );

    // The alias/sub-directive allowlist must NOT overlap the spec — once the
    // spec registers one of these names, remove it from the allowlist.
    for extra in surf_parse::lint::EXTRA_KNOWN_BLOCK_NAMES {
        assert!(
            !spec_names.contains(extra),
            "'{extra}' is now registered in spec/blocks.toml — remove it from \
             lint::EXTRA_KNOWN_BLOCK_NAMES"
        );
    }
}

// ------------------------------------------------------------------
// App-spec reference document (spec/app-spec-v1.surf)
// ------------------------------------------------------------------

/// The app-spec reference doc must itself be a valid SurfDoc: it parses with
/// zero errors, carries the expected front matter, and its live example app
/// passes structured validation.
#[test]
fn app_spec_v1_doc_is_valid_surfdoc() {
    let source = include_str!("../spec/app-spec-v1.surf");
    let result = surf_parse::parse(source);

    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == surf_parse::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "spec doc must parse without errors: {errors:?}");

    let fm = result
        .doc
        .front_matter
        .as_ref()
        .expect("spec doc must have front matter");
    assert_eq!(fm.title.as_deref(), Some("SurfDoc App Spec v1"));
    assert_eq!(fm.doc_type, Some(surf_parse::DocType::Doc));
    assert_eq!(fm.version, Some(1));
    assert_eq!(fm.created.as_deref(), Some("2026-07-15"));
    assert_eq!(fm.status, Some(surf_parse::DocStatus::Active));

    // Structural requirements: a ::summary block near the top and a warning
    // ::callout somewhere in the body.
    let summary_idx = result
        .doc
        .blocks
        .iter()
        .position(|b| matches!(b, surf_parse::Block::Summary { .. }))
        .expect("spec doc must contain a ::summary block");
    assert!(summary_idx <= 1, "::summary must be at the top of the document");
    assert!(
        result
            .doc
            .blocks
            .iter()
            .any(|b| matches!(b, surf_parse::Block::Callout { .. })),
        "spec doc must contain a ::callout"
    );

    // NOTE (0.10.0 open-core split): the "live Complete Example must pass
    // validate_app_spec" assertion moved to the private surf-appcompile
    // crate's test suite (spec_complete_example_app_validates), which
    // resolves this repo as a sibling checkout.
}

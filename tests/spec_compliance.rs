use std::collections::BTreeSet;
use serde::Deserialize;

/// Canonical list of Block enum variants (excluding Unknown, Markdown).
/// When you add a variant to types.rs, add it here too — or this test fails.
const ENUM_VARIANTS: &[&str] = &[
    "Callout", "Code", "Columns", "Cta", "Data", "Decision",
    "Details", "Divider",
    "Embed", "Faq", "Figure", "Footer", "Form", "Gallery",
    "HeroImage", "Metric", "Nav", "Page", "PricingTable",
    "Quote", "Site", "Style", "Summary", "Tabs", "Tasks",
    "Testimonial",
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

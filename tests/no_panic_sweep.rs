//! No-panic sweep (WP-1 family 4).
//!
//! Source-driven: every registry name in spec/blocks.toml (implemented
//! AND planned — planned names parse as tolerated-unknown blocks) is
//! pushed through every output format with adversarial, missing, and
//! duplicate attributes. Two guarantees, per source, per format:
//!
//!   1. No panic — parsing and rendering total garbage must degrade,
//!      never crash.
//!   2. Determinism — rendering the same parsed doc twice is
//!      byte-identical (no HashMap-iteration or RNG leakage).
//!
//! Formats: to_html, to_markdown, to_terminal (default features), and
//! to_native_blocks under `--features native`.

use std::collections::BTreeSet;

/// Registry names straight from the spec (all statuses).
fn registry_names() -> Vec<String> {
    let registry: toml::Value =
        toml::from_str(include_str!("../spec/blocks.toml")).expect("blocks.toml parses");
    registry["blocks"]
        .as_table()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

/// Adversarial source variants for one block name. Attr values include
/// duplicates, empty values, wrong types, negative/overflow numbers,
/// embedded quotes/brackets, and hook-style attrs the grammar never
/// defined; content includes template markers, half-tables, nested
/// fences, and attr-shaped lines.
fn adversarial_sources(name: &str) -> Vec<String> {
    vec![
        // Missing everything.
        format!("::{name}\n::"),
        // Empty attr list, empty content line.
        format!("::{name}[]\n\n::"),
        // Duplicate + wrong-typed + hostile attrs.
        format!(
            "::{name}[width=-1 width=99999999999999999999 title=\"a]b\" title= active=true \
             active=false unread=maybe foo=bar foo=baz on-select=\"invoke:x:y\" state=]\n\
             - \"Broken {{= template =}}\" description=\"x\n\
             | half | table\n\
             ::: nested\n\
             action: | \n\
             ::"
        ),
        // Unclosed block (EOF inside the body).
        format!("::{name}[label=\"unterminated"),
        // Attr-shaped content lines and stray closers.
        format!("::{name}[id=1 id=2 id=3]\nkey: value\n- item [attr=1\n::\n::\n:::"),
    ]
}

/// Render one parsed doc through every format, twice; assert equality.
fn render_all_twice(doc: &surf_parse::SurfDoc, label: &str) {
    let html1 = doc.to_html();
    let html2 = doc.to_html();
    assert_eq!(html1, html2, "{label}: to_html must be deterministic");

    let md1 = doc.to_markdown();
    let md2 = doc.to_markdown();
    assert_eq!(md1, md2, "{label}: to_markdown must be deterministic");

    let term1 = doc.to_terminal();
    let term2 = doc.to_terminal();
    assert_eq!(term1, term2, "{label}: to_terminal must be deterministic");

    #[cfg(feature = "native")]
    {
        let native1 = serde_json::to_string(&doc.to_native_blocks()).unwrap();
        let native2 = serde_json::to_string(&doc.to_native_blocks()).unwrap();
        assert_eq!(native1, native2, "{label}: to_native_blocks must be deterministic");
    }
}

#[test]
fn all_registry_kinds_survive_adversarial_attrs_in_every_format() {
    let names = registry_names();
    // The registry currently holds 111 kinds (98 implemented + 13
    // planned); a shrink here means the sweep silently lost coverage.
    assert!(
        names.len() >= 111,
        "registry shrank to {} kinds — update this floor only with the spec",
        names.len()
    );

    let unique: BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "registry names must be unique");

    for name in &names {
        for (i, src) in adversarial_sources(name).iter().enumerate() {
            // Parse must not panic; diagnostics are fine and expected.
            let result = surf_parse::parse(src);
            // Parse determinism, too: same source, same doc.
            let again = surf_parse::parse(src);
            assert_eq!(
                serde_json::to_string(&result.doc).unwrap(),
                serde_json::to_string(&again.doc).unwrap(),
                "::{name} variant {i}: parse must be deterministic"
            );
            render_all_twice(&result.doc, &format!("::{name} variant {i}"));
        }
    }
}

/// A whole-document stress: every registry kind concatenated into one
/// source, with duplicated attrs, then rendered through every format.
/// Catches cross-block interference (e.g. shell/tab activation state
/// bleeding between blocks) that per-kind docs cannot.
#[test]
fn union_of_all_kinds_renders_without_panic() {
    let mut src = String::from("---\ntitle: \"No-panic union\"\n---\n\n");
    for name in registry_names() {
        src.push_str(&format!(
            "::{name}[x=1 x=2 title=\"t\" active=zz]\ncontent {{= slot =}}\n::\n\n"
        ));
    }
    let doc = surf_parse::parse(&src).doc;
    render_all_twice(&doc, "union");
}

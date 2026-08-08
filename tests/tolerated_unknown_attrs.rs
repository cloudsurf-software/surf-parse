//! Tolerated-unknown attribute pin (test-hardening round, R3).
//!
//! The grammar does not define "hook" attributes — block-level `name` /
//! `placement` / `anchor` / `action` on blocks that never declared them,
//! and dropdown option `value` / `state` / `reason` / `danger`. Today the
//! parser TOLERATES them: they parse without diagnostics, are silently
//! dropped from the typed tree (dropdown option parsing keeps only
//! description/icon/action — blocks.rs; block parsers read only their
//! declared attrs), never reach any output format, and auto-fix leaves
//! the source form untouched.
//!
//! This suite pins TODAY'S behavior only (ruling R3: no first-class hook
//! attrs, no grammar debt paid here). If the grammar later adopts these
//! attrs, these tests should fail and be rewritten deliberately.

use surf_parse::FixSafety;

/// Hook attr names/values that must never leak into any output.
const HOOK_MARKERS: &[&str] = &[
    // dropdown option hooks
    "value=", "state=", "reason=", "danger",
    "hook-value-x", "hook-state-x", "hook-reason-x",
    // block-level hooks (values chosen to be greppable)
    "hook-name-x", "hook-placement-x", "hook-anchor-x", "hook-action-x",
];

fn hook_doc_source() -> &'static str {
    // Block-level hooks ride on blocks that do NOT declare them:
    // ::row declares none of name/placement/anchor; ::segmented-control
    // declares action (given a hook value on ::row instead).
    "::row[icon=doc name=hook-name-x placement=hook-placement-x anchor=hook-anchor-x action=hook-action-x]\n\
     Title\n\
     Description\n\
     ::\n\
     \n\
     ::dropdown-select[label=\"Sort\"]\n\
     - \"Newest\" description=\"Most recent\" action=sort_newest value=hook-value-x state=hook-state-x reason=hook-reason-x danger=false\n\
     - \"Oldest\" value=hook-value-x danger=true\n\
     ::\n"
}

#[test]
fn hook_attrs_parse_without_diagnostics() {
    let result = surf_parse::parse(hook_doc_source());
    assert!(
        result.diagnostics.is_empty(),
        "unknown hook attrs must be tolerated silently today: {:?}",
        result.diagnostics
    );
}

#[test]
fn hook_attrs_never_leak_into_any_output_format() {
    let doc = surf_parse::parse(hook_doc_source()).doc;
    let outputs: Vec<(&str, String)> = vec![
        ("html", doc.to_html()),
        ("markdown", doc.to_markdown()),
        ("terminal", doc.to_terminal()),
        #[cfg(feature = "native")]
        (
            "native",
            serde_json::to_string(&doc.to_native_blocks()).unwrap(),
        ),
    ];
    for (format, out) in &outputs {
        for marker in HOOK_MARKERS {
            assert!(
                !out.contains(marker),
                "hook attr {marker:?} leaked into {format} output:\n{out}"
            );
        }
        // The blocks themselves still render.
        assert!(!out.is_empty(), "{format} output must not be empty");
    }
    // Sanity: declared attrs DID make it through, so the drop is
    // attr-selective, not block-wide.
    let html = &outputs[0].1;
    assert!(html.contains("data-action=\"sort_newest\""));
    assert!(html.contains("Most recent"));
}

/// Auto-fix (both safety tiers) must leave a hook-attr document
/// byte-identical: tolerated means tolerated — no rule rewrites or strips
/// them, so the source form round-trips unchanged.
#[test]
fn hook_attrs_survive_autofix_round_trip() {
    let src = hook_doc_source();
    for safety in [FixSafety::Safe, FixSafety::Suggested] {
        let outcome = surf_parse::apply_fixes(src, safety);
        assert_eq!(
            outcome.source, src,
            "auto-fix ({safety:?}) must not rewrite tolerated hook attrs"
        );
    }
}

/// Re-parsing the (unchanged) source yields the same typed tree — the
/// drop is deterministic, not stateful.
#[test]
fn hook_attr_drop_is_deterministic() {
    let a = surf_parse::parse(hook_doc_source()).doc;
    let b = surf_parse::parse(hook_doc_source()).doc;
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap(),
    );
}

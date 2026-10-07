//! Every diagram type passes the constructive sink (0.41.0, TASK-1455).
//!
//! The web shell renders a whole doc through `render_dom`, and a diagram's
//! svg goes through `build_verified_markup`, which refuses any attribute
//! outside `attr_allowed` — so ONE unlisted attribute declined the WHOLE
//! document to its prose fallback. Measured 2026-10-07 on 0.40.0: a
//! sequence `-->` return, a usecase `^->` include, a c4 `boundary` (all
//! `stroke-dasharray`), a class `*->` / `o->` and an architecture `<->`
//! (`marker-start`), the xychart and radar aliases (`fill-opacity`).
//!
//! This test draws ONE block of every type (17 natives + 4 chart aliases,
//! `tests/fixtures/diagrams-every-type.surf`) through the native sink, names
//! the type that declines, and pins byte identity with `render_html` — the
//! renderer's own law. NEVER-WEAKEN: a new diagram type joins the fixture.

#![cfg(feature = "dom")]

use surf_parse::render_dom::{check_coverage, check_coverage_blocks, render_fragment_string};
use surf_parse::Block;

const FIXTURE: &str = include_str!("fixtures/diagrams-every-type.surf");

/// The types the fixture must carry, in its order — a type missing here or
/// there fails loudly instead of silently shrinking the coverage.
const EVERY_TYPE: [&str; 21] = [
    "architecture", "erd", "flowchart", "sequence", "gantt", "state", "mindmap", "class", "timeline",
    "journey", "quadrant", "kanban", "usecase", "gitgraph", "c4", "requirement", "sankey", "pie",
    "donut", "radar", "xychart",
];

fn diagram_blocks() -> Vec<Block> {
    surf_parse::parse(FIXTURE)
        .doc
        .blocks
        .into_iter()
        .filter(|b| matches!(b, Block::Diagram { .. }))
        .collect()
}

#[test]
fn the_fixture_carries_every_type_once_and_each_one_draws() {
    let blocks = diagram_blocks();
    let types: Vec<String> = blocks
        .iter()
        .map(|b| match b {
            Block::Diagram { diagram_type, .. } => diagram_type.clone(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(types, EVERY_TYPE, "the fixture's types, in order");
    // Every block DRAWS (no prose fallback): the html carries one svg per
    // block and no `surfdoc-diagram-src`.
    let html = surf_parse::parse(FIXTURE).doc.to_html_fragment();
    assert_eq!(html.matches("class=\"surfdoc-diagram-svg\"").count() + html.matches("class=\"surfdoc-chart-svg\"").count(), EVERY_TYPE.len(), "one svg per block: {html}");
    assert!(!html.contains("surfdoc-diagram-src"), "a block fell back to its source: {html}");
    assert!(!html.contains("surfdoc-diagram-fallback"), "a block fell back: {html}");
}

#[test]
fn every_diagram_type_passes_the_constructive_sink_alone() {
    let mut declined: Vec<String> = Vec::new();
    for b in diagram_blocks() {
        let Block::Diagram { diagram_type, .. } = &b else { unreachable!() };
        if let Err(e) = check_coverage_blocks(std::slice::from_ref(&b)) {
            declined.push(format!("{diagram_type}: {e}"));
        }
    }
    assert!(declined.is_empty(), "types the constructive sink declined:\n{}", declined.join("\n"));
}

#[test]
fn the_whole_fixture_is_covered_and_byte_identical_to_render_html() {
    let doc = surf_parse::parse(FIXTURE).doc;
    if let Err(e) = check_coverage(&doc) {
        panic!("expected full coverage of the every-type fixture, got decline: {e}");
    }
    let dom_html = render_fragment_string(&doc).expect("native sink renders");
    let string_html = doc.to_html_fragment();
    assert_eq!(dom_html, string_html, "constructive DOM serialization drifted from render_html");
}

/// The three attributes measured missing on 0.40.0 are admitted, and the
/// allowlist still refuses an event handler or a style sink.
#[test]
fn the_allowlist_admits_the_three_measured_paint_attributes_and_nothing_scripted() {
    use surf_parse::render_dom::attr_allowed;
    for name in ["stroke-dasharray", "marker-start", "fill-opacity"] {
        assert!(attr_allowed(name), "{name} is paint the diagram and chart renderers emit");
    }
    for name in ["onclick", "onload", "srcdoc", "xlink:href", "formaction"] {
        assert!(!attr_allowed(name), "{name} must stay refused");
    }
}

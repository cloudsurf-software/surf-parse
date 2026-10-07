//! A `::diagram` draws on the page (0.41.0, TASK-1455): the PDF pipeline
//! registers the SAME svg the web draws (`diagram::render_block_svg`) as a
//! virtual file, and the typst arm emits a sized, unnumbered figure with the
//! title as its caption — where it used to print the bold title and the raw
//! DSL in a fence. The CloudSurf web Doc tab's Preview mode is these pages
//! (`to_pages`), so this is what a person sees when a doc opens.
#![cfg(feature = "pdf")]

use surf_parse::{to_pages, typst_source, PdfConfig};

const FIXTURE: &str = include_str!("fixtures/diagrams-every-type.surf");

#[test]
fn every_diagram_type_is_a_figure_in_the_typst_source_not_a_fence() {
    let doc = surf_parse::parse(FIXTURE).doc;
    let typst = typst_source(&doc, &PdfConfig::default());
    // For the eyes: the markup as generated (never a gate).
    let _ = std::fs::write(std::env::temp_dir().join("surf-parse-diagrams.typ"), &typst);
    let figures = typst.matches("layout(size => image(\"/surf-diagram-").count();
    assert_eq!(figures, 21, "one sized figure per block: {typst}");
    assert_eq!(typst.matches("/surf-diagram-").count(), 21, "one virtual file per block, no duplicates");
    assert!(typst.matches("numbering: none").count() >= 21, "unnumbered, as the web's figcaption");
    assert!(typst.contains("caption: [C4 with a boundary]"), "the title is the caption: {typst}");
    assert!(!typst.contains("```"), "no raw-DSL fence remains: {typst}");
    assert!(!typst.contains("person user: Customer"), "the DSL text is not on the page: {typst}");
    assert!(typst.contains("calc.min(size.width, "), "the natural width, capped to the text width");
}

#[test]
fn a_bare_to_typst_keeps_the_fence_so_it_never_names_a_file_the_engine_lacks() {
    let doc = surf_parse::parse(FIXTURE).doc;
    let typst = doc.to_typst();
    assert!(!typst.contains("surf-diagram-"), "no virtual file outside the pipeline: {typst}");
    assert_eq!(typst.matches("```\n").count(), 42, "the title + fence fallback, 21 times");
}

#[test]
fn a_body_that_does_not_draw_keeps_the_fence_inside_the_pipeline() {
    let src = "# Broken\n\n::diagram[type=architecture title=\"Half\"]\nweb: Web\nweb ->\n::\n\n::diagram[type=nosuchtype]\nx -> y\n::\n";
    let doc = surf_parse::parse(src).doc;
    assert_eq!(doc.blocks.iter().filter(|b| matches!(b, surf_parse::Block::Diagram { .. })).count(), 2, "two diagram blocks parsed");
    let html = doc.to_html_fragment();
    assert_eq!(html.matches("surfdoc-diagram-fallback").count(), 2, "both fall back on the web too: {html}");
    let typst = typst_source(&doc, &PdfConfig::default());
    assert!(!typst.contains("surf-diagram-"), "{typst}");
    assert!(typst.contains("*Half* \\\n```\nweb: Web\nweb ->\n```"), "the author's source, under the title: {typst}");
}

#[test]
fn the_every_type_fixture_compiles_to_pages_that_carry_the_svgs() {
    let doc = surf_parse::parse(FIXTURE).doc;
    let pages = to_pages(&doc, &PdfConfig::default()).expect("the every-type fixture compiles");
    assert!(!pages.is_empty());
    for (i, p) in pages.iter().enumerate() {
        let _ = std::fs::write(std::env::temp_dir().join(format!("surf-parse-diagrams-page-{}.svg", i + 1)), p);
    }
    let all = pages.concat();
    // typst-svg draws an svg image as nested markup; the diagram's own
    // classes ride through, so the page carries the web's svg, not a fence.
    assert!(all.contains("surfdoc-diagram-node") || all.contains("surfdoc-diagram") || all.contains("<image"), "the pages carry the diagram svg: {}", &all[..all.len().min(2000)]);
}

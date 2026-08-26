//! Client DoS bounds — `max_depth`, `max_blocks`, `max_source_bytes`
//! (spec/web-runtime-v1.surf §4.4).
//!
//! Three properties are pinned here:
//! 1. Each bound, on its own, produces the typed decline — never a panic,
//!    never a silently truncated document.
//! 2. Every vendored Surfspace web-shell fixture passes under the shipped
//!    defaults, so the bounds cannot quietly start refusing real surfaces.
//! 3. The bound-free entry points behave exactly as they did before the
//!    bounds existed.

use surf_parse::limits::{LimitExceeded, ParseLimits, measure_blocks, measure_depth};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn web_shell_fixtures() -> Vec<(String, String)> {
    let root = format!("{}/tests/fixtures/web-shell", env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    collect(std::path::Path::new(&root), &root, &mut out);
    out.sort();
    assert!(
        out.len() >= 56,
        "web-shell corpus shrank to {} fixtures — the list is add-only",
        out.len()
    );
    out
}

fn collect(dir: &std::path::Path, root: &str, out: &mut Vec<(String, String)>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect(&path, root, out);
        } else if path.extension().map(|e| e == "surf").unwrap_or(false) {
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .display()
                .to_string();
            let src = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            out.push((rel, src));
        }
    }
}

/// A `::page` chain `depth` containers deep, holding one paragraph at the
/// bottom. Colon-fence width grows with depth so each container nests inside
/// the previous one rather than closing it.
fn nested_pages(depth: usize) -> String {
    let mut s = String::new();
    for i in 0..depth {
        s.push_str(&":".repeat(2 + i));
        s.push_str(&format!("page[title=P{i}]\n\n"));
    }
    s.push_str("leaf\n\n");
    for i in (0..depth).rev() {
        s.push_str(&":".repeat(2 + i));
        s.push_str("\n\n");
    }
    s
}

// ---------------------------------------------------------------------------
// Bound-by-bound declines
// ---------------------------------------------------------------------------

#[test]
fn depth_bound_declines_with_the_typed_error() {
    let doc = surf_parse::parse(&nested_pages(6)).doc;
    let measured = measure_depth(&doc.blocks);
    assert!(
        measured >= 6,
        "fixture nests {measured} deep, expected >= 6"
    );

    let limits = ParseLimits {
        max_depth: 3,
        ..ParseLimits::default()
    };
    match limits.check_doc(&doc) {
        Err(LimitExceeded::Depth { limit, reached }) => {
            assert_eq!(limit, 3);
            assert_eq!(reached, 4, "aborts at the first block past the bound");
        }
        other => panic!("expected a depth decline, got {other:?}"),
    }

    // Exactly at the bound is fine — the maximum is inclusive.
    let at_bound = ParseLimits {
        max_depth: measured,
        ..ParseLimits::default()
    };
    assert!(at_bound.check_doc(&doc).is_ok());
}

#[test]
fn block_count_bound_declines_with_the_typed_error() {
    // Dividers stay distinct blocks; consecutive paragraphs would coalesce.
    let src = "::divider\n\n".repeat(40);
    let doc = surf_parse::parse(&src).doc;
    let measured = measure_blocks(&doc.blocks);
    assert!(measured >= 40, "fixture holds {measured} blocks");

    let limits = ParseLimits {
        max_blocks: 10,
        ..ParseLimits::default()
    };
    match limits.check_doc(&doc) {
        Err(LimitExceeded::Blocks { limit, reached }) => {
            assert_eq!(limit, 10);
            assert_eq!(reached, 11, "the walk stops one past the bound");
        }
        other => panic!("expected a block-count decline, got {other:?}"),
    }

    let at_bound = ParseLimits {
        max_blocks: measured,
        ..ParseLimits::default()
    };
    assert!(at_bound.check_doc(&doc).is_ok());
}

#[test]
fn block_count_bound_counts_nested_blocks() {
    let doc = surf_parse::parse(&nested_pages(5)).doc;
    // Five containers plus the leaf paragraph, all nested — a walk that only
    // looked at top-level blocks would see 1.
    assert_eq!(measure_blocks(&doc.blocks), 6);
    let limits = ParseLimits {
        max_blocks: 4,
        ..ParseLimits::default()
    };
    assert!(matches!(
        limits.check_doc(&doc),
        Err(LimitExceeded::Blocks { limit: 4, .. })
    ));
}

#[test]
fn source_byte_bound_declines_before_parsing() {
    let src = "x".repeat(4096);
    let limits = ParseLimits {
        max_source_bytes: 1024,
        ..ParseLimits::default()
    };
    match limits.check_source_bytes(&src) {
        Err(LimitExceeded::SourceBytes { limit, reached }) => {
            assert_eq!(limit, 1024);
            assert_eq!(reached, 4096);
        }
        other => panic!("expected a source-bytes decline, got {other:?}"),
    }
    assert!(limits.check_source_bytes(&"x".repeat(1024)).is_ok());
}

#[test]
fn source_byte_bound_measures_bytes_not_characters() {
    // Four characters, twelve UTF-8 bytes.
    let src = "。。。。";
    assert_eq!(src.chars().count(), 4);
    let limits = ParseLimits {
        max_source_bytes: 8,
        ..ParseLimits::default()
    };
    assert!(matches!(
        limits.check_source_bytes(src),
        Err(LimitExceeded::SourceBytes { reached: 12, .. })
    ));
}

#[test]
fn unlimited_never_fires() {
    let doc = surf_parse::parse(&nested_pages(20)).doc;
    let limits = ParseLimits::unlimited();
    assert!(limits.check_doc(&doc).is_ok());
    assert!(limits.check_source_bytes(&"x".repeat(1 << 20)).is_ok());
}

// ---------------------------------------------------------------------------
// The corpus under the shipped defaults
// ---------------------------------------------------------------------------

#[test]
fn every_web_shell_fixture_passes_under_defaults() {
    let limits = ParseLimits::default();
    for (rel, src) in web_shell_fixtures() {
        limits
            .check_source_bytes(&src)
            .unwrap_or_else(|e| panic!("{rel}: source bound fired on a real surface: {e}"));
        let doc = surf_parse::parse(&src).doc;
        limits
            .check_doc(&doc)
            .unwrap_or_else(|e| panic!("{rel}: tree bound fired on a real surface: {e}"));
    }
}

#[test]
fn corpus_headroom_under_defaults_is_documented() {
    let limits = ParseLimits::default();
    let (mut depth, mut blocks, mut bytes) = (0usize, 0usize, 0usize);
    for (_, src) in web_shell_fixtures() {
        let doc = surf_parse::parse(&src).doc;
        depth = depth.max(measure_depth(&doc.blocks));
        blocks = blocks.max(measure_blocks(&doc.blocks));
        bytes = bytes.max(src.len());
    }
    // Real surfaces must stay far under the bounds; if a future surface
    // closes the gap, this fails and the defaults get re-ruled deliberately
    // rather than drifting.
    assert!(
        depth * 4 < limits.max_depth,
        "deepest surface nests {depth}"
    );
    assert!(
        blocks * 4 < limits.max_blocks,
        "largest surface holds {blocks} blocks"
    );
    assert!(
        bytes * 4 < limits.max_source_bytes,
        "largest source is {bytes} bytes"
    );
}

// ---------------------------------------------------------------------------
// The constructive-DOM gate
// ---------------------------------------------------------------------------

#[cfg(feature = "dom")]
mod dom_gate {
    use super::*;
    use surf_parse::render_dom::{
        RenderDomError, check_coverage, check_coverage_blocks, check_coverage_blocks_with_limits,
        check_coverage_with_limits, check_source_coverage,
    };

    /// A document whose only problem is its depth declines as a bound, not as
    /// an unimplemented construct.
    #[test]
    fn depth_bound_surfaces_as_a_typed_render_decline() {
        let doc = surf_parse::parse(&nested_pages(6)).doc;
        let limits = ParseLimits {
            max_depth: 3,
            ..ParseLimits::default()
        };
        match check_coverage_with_limits(&doc, &limits) {
            Err(RenderDomError::LimitExceeded(LimitExceeded::Depth {
                limit: 3,
                reached: 4,
            })) => {}
            other => panic!("expected a bounds decline, got {other:?}"),
        }
    }

    #[test]
    fn block_count_bound_surfaces_as_a_typed_render_decline() {
        let doc = surf_parse::parse(&"::divider\n\n".repeat(40)).doc;
        let limits = ParseLimits {
            max_blocks: 10,
            ..ParseLimits::default()
        };
        assert!(matches!(
            check_coverage_blocks_with_limits(&doc.blocks, &limits),
            Err(RenderDomError::LimitExceeded(LimitExceeded::Blocks {
                limit: 10,
                ..
            }))
        ));
    }

    #[test]
    fn source_bound_declines_without_parsing_or_rendering() {
        let limits = ParseLimits {
            max_source_bytes: 8,
            ..ParseLimits::default()
        };
        match check_source_coverage(&"::divider\n\n".repeat(40), &limits) {
            Err(RenderDomError::LimitExceeded(LimitExceeded::SourceBytes { limit: 8, .. })) => {}
            other => panic!("expected a source-bytes decline, got {other:?}"),
        }
    }

    /// The source entry point hands back the parsed document on success, so
    /// the server and the takeover never parse the same source twice.
    #[test]
    fn source_entry_point_returns_the_parsed_doc_on_success() {
        let src = "# Hi\n\nhello\n";
        let doc = check_source_coverage(src, &ParseLimits::default()).expect("covered");
        assert_eq!(doc.blocks.len(), surf_parse::parse(src).doc.blocks.len());
    }

    /// Bounds are checked BEFORE coverage: a document that is both too deep
    /// and full of uncovered kinds reports the bound.
    #[test]
    fn bounds_are_reported_ahead_of_coverage() {
        let mut src = nested_pages(6);
        src.push_str("\n::toc\n::\n");
        let doc = surf_parse::parse(&src).doc;
        assert!(
            matches!(check_coverage(&doc), Err(RenderDomError::Unimplemented(_))),
            "the fixture must be uncovered without bounds, or this proves nothing"
        );
        let limits = ParseLimits {
            max_depth: 3,
            ..ParseLimits::default()
        };
        assert!(matches!(
            check_coverage_with_limits(&doc, &limits),
            Err(RenderDomError::LimitExceeded(_))
        ));
    }

    /// Coverage verdicts are untouched when the bounds cannot fire: the
    /// limits-taking gate agrees with the bound-free one on every web-shell
    /// fixture, decline payload included.
    #[test]
    fn default_bounds_change_no_coverage_verdict() {
        let limits = ParseLimits::default();
        for (rel, src) in web_shell_fixtures() {
            let doc = surf_parse::parse(&src).doc;
            let bare = check_coverage(&doc);
            let bounded = check_coverage_with_limits(&doc, &limits);
            assert_eq!(bare, bounded, "{rel}: default bounds changed the verdict");
            let bare_blocks = check_coverage_blocks(&doc.blocks);
            let bounded_blocks = check_coverage_blocks_with_limits(&doc.blocks, &limits);
            assert_eq!(
                bare_blocks, bounded_blocks,
                "{rel}: default bounds changed the block-level verdict"
            );
        }
    }
}

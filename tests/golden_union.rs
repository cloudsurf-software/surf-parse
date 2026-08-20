//! Golden union render (test-hardening round, R2).
//!
//! One committed source — `tests/golden/union-0_12-0_13.surf` — exercises
//! every kind and attribute introduced in the 0.12 and 0.13 trains in a
//! single document: app-shell height, sidebar hairline divider, toolbar
//! title/title-source/text-size/toggled, tab-bar unread, ruled centered
//! tab-content (width/align), segmented-control, dropdown-select, row
//! unread/trailing/per-row actions/link-row demotion, list streaming
//! attrs, chat-thread seams, panel, modal (width/placement/dismissible),
//! recipient-picker, and both qr modes.
//!
//! Its HTML render is pinned byte-for-byte against
//! `tests/golden/union-0_12-0_13.html.snap`.
//!
//! UPDATING THE SNAPSHOT (same contract as tests/corpus.rs): when a
//! renderer change intentionally shifts output, regenerate with
//!
//!   UPDATE_SNAPSHOTS=1 cargo test --test golden_union
//!
//! then review the snapshot diff like any code change. A missing
//! snapshot without the env flag is a hard failure — CI can never
//! silently skip the pin.

use std::fs;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = golden_dir().join(name);
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&path, actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {} — run UPDATE_SNAPSHOTS=1 cargo test --test golden_union",
            path.display()
        )
    });
    assert_eq!(
        expected, actual,
        "golden union drift ({name}) — if intentional, regenerate with UPDATE_SNAPSHOTS=1 \
         and review the diff"
    );
}

#[test]
fn golden_union_html_pinned() {
    let src = fs::read_to_string(golden_dir().join("union-0_12-0_13.surf"))
        .expect("union source exists");
    let result = surf_parse::parse(&src);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == surf_parse::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "union source must parse without errors: {errors:?}");
    assert_snapshot("union-0_12-0_13.html.snap", &result.doc.to_html());
}

/// 0.17 union — the Messages mockup-fidelity round in one document:
/// chip-input (label/chips/filter input), row avatar (initials, group
/// glyph, auto-derivation), rtime meta, unread count pill, chat-thread
/// message children (sides, sender leads, in-bubble timestamps, read-only
/// reaction pills) plus the backward-compatible attrs-only preview.
#[test]
fn golden_union_0_17_html_pinned() {
    let src = fs::read_to_string(golden_dir().join("union-0_17.surf"))
        .expect("0.17 union source exists");
    let result = surf_parse::parse(&src);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == surf_parse::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "0.17 union source must parse without errors: {errors:?}");
    assert_snapshot("union-0_17.html.snap", &result.doc.to_html());
}

/// The 0.17 union source actually covers the round's vocabulary.
#[test]
fn golden_union_0_17_covers_the_new_kinds() {
    let src = fs::read_to_string(golden_dir().join("union-0_17.surf"))
        .expect("0.17 union source exists");
    let html = surf_parse::parse(&src).doc.to_html();
    for marker in [
        "surfdoc-chip-input",
        "surfdoc-chip-input-chip",
        "surfdoc-chip-input-remove",
        "surfdoc-chip-input-field",
        "surfdoc-row-avatar",
        "surfdoc-row-avatar-group",
        "surfdoc-row-time",
        "surfdoc-row-badge",
        "surfdoc-chat-bubble-them",
        "surfdoc-chat-bubble-own",
        "surfdoc-chat-time",
        "surfdoc-chat-sender-surfy",
        "surfdoc-chat-react-pill-mine",
        // attrs-only thread keeps the sample preview
        "surfdoc-chat-msg-user",
    ] {
        assert!(html.contains(marker), "0.17 union render must contain {marker}");
    }
    // avatar=auto derived initials from "sam rose".
    assert!(html.contains(">SR</span>"));
}

/// The union source actually covers the 0.12/0.13 vocabulary — guards the
/// fixture itself against decay when someone trims it.
#[test]
fn golden_union_covers_the_new_kinds() {
    let src = fs::read_to_string(golden_dir().join("union-0_12-0_13.surf"))
        .expect("union source exists");
    let html = surf_parse::parse(&src).doc.to_html();
    for marker in [
        // 0.13 round
        "surfdoc-segmented-control",
        "surfdoc-dropdown-select",
        "surfdoc-unread-dot",
        "surfdoc-toolbar-btn--toggled",
        "surfdoc-row-trailing",
        "surfdoc-modal-close",
        "data-dismissible=\"false\"",
        "min-height:720px",
        "max-width:880px",
        "surfdoc-divider-plain",
        "font-size:22px",
        // 0.12 train
        "surfdoc-recipient-picker",
        "surfdoc-qr-show",
        "surfdoc-qr-scan",
        "surfdoc-chat-thread",
        "data-title-source=\"thread.display_name\"",
        "surfdoc-row-action",
    ] {
        assert!(html.contains(marker), "union render must contain {marker}");
    }
}

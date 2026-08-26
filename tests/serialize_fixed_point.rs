//! `parse(serialize(parse(src)))` is a fixed point over the Surfspace
//! `/next` web-shell corpus (0.19.0).
//!
//! The surf lane composes docs and tasks pages by parsing a shell source,
//! editing the block tree and serializing it back with
//! [`surf_parse::builder::to_surf_source`]. That only works if serializing is
//! a fixed point: the source it emits must re-parse to a document that
//! renders byte-identically, and serializing THAT document must reproduce the
//! same source. Before 0.19.0 it was not — see the three regression tests at
//! the bottom of this file for the three distinct defects.
//!
//! Corpus: `tests/fixtures/web-shell/` (vendored by
//! `tests/fixtures/web-shell/README.md`; resync tool in the private app repo). The list is ADD-ONLY; the
//! floor below fails if fixtures disappear rather than silently passing on a
//! shrunken corpus.

use std::path::{Path, PathBuf};

/// Vendored web-shell sources, excluding the hostile sub-corpus.
const SHELL_FIXTURE_FLOOR: usize = 56;
/// Hostile (quote-breaking, URL-scheme, raw-text, half-open, max-depth) sources.
const HOSTILE_FIXTURE_FLOOR: usize = 5;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/web-shell")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().map(|e| e == "surf").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// `(relative name, source)` for every fixture under `web-shell/`, hostile
/// sources separated out.
fn fixtures(hostile: bool) -> Vec<(String, String)> {
    let root = fixture_root();
    let mut paths = Vec::new();
    collect(&root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .filter(|p| p.components().any(|c| c.as_os_str() == "hostile") == hostile)
        .map(|p| {
            let name = p
                .strip_prefix(&root)
                .expect("fixture under root")
                .to_string_lossy()
                .replace('\\', "/");
            let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {name}: {e}"));
            (name, src)
        })
        .collect()
}

/// One fixed-point pass: returns `(html1, html2, source1, source2)`.
fn round_trip(src: &str) -> (String, String, String, String) {
    let doc1 = surf_parse::parse(src).doc;
    let source1 = surf_parse::builder::to_surf_source(&doc1);
    let doc2 = surf_parse::parse(&source1).doc;
    let source2 = surf_parse::builder::to_surf_source(&doc2);
    let html1 = surf_parse::render_html::to_html(&doc1);
    let html2 = surf_parse::render_html::to_html(&doc2);
    (html1, html2, source1, source2)
}

/// Report the first byte where two strings diverge, with context.
fn first_divergence(a: &str, b: &str) -> String {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let mut i = 0;
    while i < ab.len().min(bb.len()) && ab[i] == bb[i] {
        i += 1;
    }
    let lo = a[..i].char_indices().rev().nth(60).map(|(x, _)| x).unwrap_or(0);
    let clip = |s: &str| -> String {
        let hi = s[i.min(s.len())..]
            .char_indices()
            .nth(160)
            .map(|(x, _)| i + x)
            .unwrap_or(s.len());
        s[lo.min(s.len())..hi].to_string()
    };
    format!("at byte {i}\n  first pass : {:?}\n  second pass: {:?}", clip(a), clip(b))
}

#[test]
fn web_shell_fixture_corpus_is_present() {
    let shell = fixtures(false);
    let hostile = fixtures(true);
    assert!(
        shell.len() >= SHELL_FIXTURE_FLOOR,
        "web-shell corpus shrank: {} sources, floor {SHELL_FIXTURE_FLOOR} (the list is add-only)",
        shell.len()
    );
    assert!(
        hostile.len() >= HOSTILE_FIXTURE_FLOOR,
        "hostile corpus shrank: {} sources, floor {HOSTILE_FIXTURE_FLOOR}",
        hostile.len()
    );
}

#[test]
fn serializing_every_web_shell_source_is_a_fixed_point() {
    let mut html_failures = Vec::new();
    let mut source_failures = Vec::new();
    let corpus = fixtures(false);
    for (name, src) in &corpus {
        let (html1, html2, source1, source2) = round_trip(src);
        if html1 != html2 {
            html_failures.push(format!("{name}: {}", first_divergence(&html1, &html2)));
        }
        if source1 != source2 {
            source_failures.push(format!("{name}: {}", first_divergence(&source1, &source2)));
        }
    }
    assert!(
        html_failures.is_empty(),
        "{} of {} web-shell sources re-render differently after a serialize round trip:\n{}",
        html_failures.len(),
        corpus.len(),
        html_failures.join("\n")
    );
    assert!(
        source_failures.is_empty(),
        "{} of {} web-shell sources do not re-serialize to themselves:\n{}",
        source_failures.len(),
        corpus.len(),
        source_failures.join("\n")
    );
}

#[test]
fn serializing_the_hostile_corpus_is_a_fixed_point() {
    let mut failures = Vec::new();
    for (name, src) in fixtures(true) {
        let (html1, html2, source1, source2) = round_trip(&src);
        if html1 != html2 {
            failures.push(format!("{name} html: {}", first_divergence(&html1, &html2)));
        }
        if source1 != source2 {
            failures.push(format!("{name} source: {}", first_divergence(&source1, &source2)));
        }
    }
    assert!(failures.is_empty(), "hostile round-trip drift:\n{}", failures.join("\n"));
}

/// The chrome family must survive the round trip as a TREE, not as a flat
/// sibling list — the failure mode the colon-depth defect produced.
#[test]
fn chrome_containers_keep_their_children_through_the_round_trip() {
    use surf_parse::types::Block;

    fn shape(blocks: &[Block]) -> Vec<(String, usize)> {
        blocks
            .iter()
            .map(|b| {
                let name = format!("{b:?}");
                let name = name.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_string();
                let kids = match b {
                    Block::AppShell { children, .. }
                    | Block::Sidebar { children, .. }
                    | Block::Panel { children, .. }
                    | Block::TabContent { children, .. }
                    | Block::Drawer { children, .. }
                    | Block::Modal { children, .. } => children.len(),
                    _ => 0,
                };
                (name, kids)
            })
            .collect()
    }

    let mut checked = 0usize;
    for (name, src) in fixtures(false) {
        let doc1 = surf_parse::parse(&src).doc;
        let doc2 = surf_parse::parse(&surf_parse::builder::to_surf_source(&doc1)).doc;
        assert_eq!(shape(&doc1.blocks), shape(&doc2.blocks), "{name}: top-level shape changed");
        for (a, b) in doc1.blocks.iter().zip(doc2.blocks.iter()) {
            if let (Block::AppShell { children: c1, .. }, Block::AppShell { children: c2, .. }) = (a, b) {
                assert!(!c1.is_empty(), "{name}: app-shell parsed with no children");
                assert_eq!(shape(c1), shape(c2), "{name}: app-shell children changed");
                checked += 1;
            }
        }
    }
    assert!(checked >= 25, "expected the app-shell family in the corpus, saw {checked}");
}

// ---------------------------------------------------------------------
// Regression tests — one per defect the corpus above caught.
// ---------------------------------------------------------------------

/// Nesting is expressed by the colon run on the fence line. Serializing every
/// level with `::` re-parsed as a flat sibling list.
#[test]
fn nested_containers_serialize_with_deeper_fences() {
    let src = "::app-shell[layout=sidebar-main-panel]\n\n:::sidebar[position=left]\n\n::::toolbar\n- text[value=\"Workspace\"]\n::::\n\n:::\n\n::\n";
    let doc = surf_parse::parse(src).doc;
    let out = surf_parse::builder::to_surf_source(&doc);
    assert!(out.contains("\n:::sidebar["), "sidebar must serialize at depth 3:\n{out}");
    assert!(out.contains("\n::::toolbar"), "toolbar must serialize at depth 4:\n{out}");
    let again = surf_parse::parse(&out).doc;
    assert_eq!(again.blocks.len(), 1, "re-parse must keep one app-shell:\n{out}");
}

/// A closer-less leaf (`::divider`) nested inside a container leaves the
/// parser's leaf/container look-ahead with an unmatched opener, so the
/// enclosing container reads as a leaf as soon as a same-depth sibling
/// directive appears later in the document.
#[test]
fn nested_closer_less_leaves_serialize_with_a_closer() {
    use surf_parse::types::Block;
    let src = "::app-shell[layout=sidebar-main-panel]\n\n:::sidebar[position=left]\n\n::::divider\n::::\n\n:::\n\n::\n\ntext\n\n::callout[type=note]\nnote\n::\n";
    let doc = surf_parse::parse(src).doc;
    let out = surf_parse::builder::to_surf_source(&doc);
    assert!(out.contains("::::divider\n::::"), "nested divider must carry a closer:\n{out}");
    let again = surf_parse::parse(&out).doc;
    let shell = again
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::AppShell { children, .. } => Some(children),
            _ => None,
        })
        .expect("app-shell survives");
    assert!(!shell.is_empty(), "app-shell must keep its sidebar:\n{out}");
    // Top level keeps the canonical closer-less form.
    let top = surf_parse::builder::to_surf_source(&surf_parse::parse("::divider\n").doc);
    assert_eq!(top.trim_end(), "::divider");
}

/// `::gallery[columns=N]` dropped its column count on serialize, so the
/// re-parsed gallery fell back to the 3-column default.
#[test]
fn gallery_columns_survive_serialization() {
    let src = "::gallery[columns=4]\n![a](a.png)\n![b](b.png)\n::\n";
    let out = surf_parse::builder::to_surf_source(&surf_parse::parse(src).doc);
    assert!(out.contains("::gallery[columns=4]"), "columns must round-trip:\n{out}");
    let html1 = surf_parse::render_html::to_html(&surf_parse::parse(src).doc);
    let html2 = surf_parse::render_html::to_html(&surf_parse::parse(&out).doc);
    assert_eq!(html1, html2);
}

/// `- id "Label"` item labels are read back with `trim_matches('"')`, which
/// does not honour backslash escapes: escaping them on the way out added one
/// `\` per round trip for any label carrying a quote.
#[test]
fn quoted_item_labels_do_not_grow_escapes() {
    for src in [
        "::segmented-control[active=find]\n- find \"Find \"it\"\"\n- ask \"Ask\"\n::\n",
        "::tab-bar[active=\"docs\"]\n- docs \"Docs \"beta\"\" {icon=doc unread}\n- tasks \"Tasks\"\n::\n",
    ] {
        let first = surf_parse::builder::to_surf_source(&surf_parse::parse(src).doc);
        let second = surf_parse::builder::to_surf_source(&surf_parse::parse(&first).doc);
        assert_eq!(first, second, "escape growth on:\n{src}first pass:\n{first}");
        assert!(!first.contains('\\'), "no backslash escape in the list form:\n{first}");
    }
}

/// 0.19.0 regression (TASK-267): an authored `state=active` row SURVIVES the
/// parse → serialize round trip and renders the active chrome (`is-active` +
/// `aria-current="page"`) server-side. Pre-fix code dropped `active` to
/// `RowState::Default` at parse — the round trip was "stable" only because
/// BOTH sides had already lost the state, and the live shell then needed a
/// client-side stamp that could never attest. This test fails there.
#[test]
fn active_row_state_round_trips_and_renders() {
    let src = "::::row[icon=doc action=openDocs href=/docs state=active]\nDocs\nAll your documents\n::::\n";
    let doc = surf_parse::parse(src).doc;
    let ser = surf_parse::builder::to_surf_source(&doc);
    assert!(ser.contains("state=active"), "state=active must survive serialization:\n{ser}");
    let html = surf_parse::render_html::to_html_fragment(&doc.blocks);
    assert!(html.contains("class=\"surfdoc-row is-active\""), "active row renders is-active:\n{html}");
    assert!(html.contains("aria-current=\"page\""), "active row carries aria-current:\n{html}");
    // And the round-tripped source renders byte-identically.
    let html2 = surf_parse::render_html::to_html_fragment(&surf_parse::parse(&ser).doc.blocks);
    assert_eq!(html, html2, "round-tripped active row must render identically");
}

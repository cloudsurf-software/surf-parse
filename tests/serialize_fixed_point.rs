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

/// 0.20.0: the four spreadsheet attributes on `::data` (`name=`, `source=`,
/// `rows=`, `cols=`) survive parse → serialize → parse, so a workbook edited
/// through the block tree keeps its sheet label and its out-of-line pointer.
#[test]
fn data_sheet_attributes_are_a_fixed_point() {
    use surf_parse::types::Block;

    let src = "::data[id=q3 name=\"Q3 revenue\" source=\"file:abc123\" rows=4200 cols=7]\n| Line | Amount |\n|---|---|\n| Coffee | 800 |\n::\n";
    let first = surf_parse::builder::to_surf_source(&surf_parse::parse(src).doc);
    let second = surf_parse::builder::to_surf_source(&surf_parse::parse(&first).doc);
    assert_eq!(first, second, "second pass drifted:\n{first}\n---\n{second}");
    let doc = surf_parse::parse(&first).doc;
    match &doc.blocks[0] {
        Block::Data {
            name,
            source,
            source_rows,
            source_cols,
            ..
        } => {
            assert_eq!(name.as_deref(), Some("Q3 revenue"));
            assert_eq!(source.as_deref(), Some("file:abc123"));
            assert_eq!(*source_rows, Some(4200));
            assert_eq!(*source_cols, Some(7));
        }
        other => panic!("expected Data, got {other:?}"),
    }
}

/// 0.21.0 site pair. `::hours` keeps each row's authored right-hand text, so
/// `label: text` re-parses to the same row (including an overnight range and
/// a closed day); `::marquee` normalises every item onto the dashed form.
/// Both must reach a fixed point on the FIRST pass, HTML included.
#[test]
fn site_blocks_hours_and_marquee_round_trip() {
    for src in [
        "::hours[title=\"Hours\" timezone=\"America/Los_Angeles\"]\nMonday: 11am - 9pm\nFriday: 5pm - 2am\nSunday: Closed\n::\n",
        "::marquee\n- Fresh daily\nOpen late\n::\n",
    ] {
        let first = surf_parse::builder::to_surf_source(&surf_parse::parse(src).doc);
        let second = surf_parse::builder::to_surf_source(&surf_parse::parse(&first).doc);
        assert_eq!(first, second, "not a fixed point for:\n{src}first pass:\n{first}");
        let html1 = surf_parse::render_html::to_html(&surf_parse::parse(&first).doc);
        let html2 = surf_parse::render_html::to_html(&surf_parse::parse(&second).doc);
        assert_eq!(html1, html2, "render drifted across the round trip:\n{src}");
    }
}

/// 0.25.0: the fourteen blocks that were planned until sessions 11 + 12.
/// Each source is the corpus's own authored shape; the serializer writes the
/// canonical spelling (`related` normalises a sentence relation onto the
/// dashed form, `timeline` its entries onto ` — `, `kernel` its packages
/// onto `[a, b]`), and every one reaches a fixed point on the FIRST pass —
/// HTML included — and re-parses to the same typed block.
#[test]
fn the_fourteen_planned_blocks_round_trip() {
    for src in [
        "::related\n- [Architecture Plan](plans/product/plan.md) \u{2014} produces\n- research/ards-v3/paper.md \u{2014} specific standard (cited for P2)\n- consumes: research/surfdoc/FINDINGS.md\n- plans/ideas/wiki.md\n::\n",
        "::turn[participant=\"claude\" time=\"2026-02-10T04:01Z\" role=ai model=\"opus\"]\nYes \u{2014} file before launch.\n\n1. **Prior art.**\n::\n",
        "::turn[participant=\"brady\"]\nShould we?\n::\n",
        "::timeline[title=\"Product Milestones\"]\n## Q1 2026\n- 2026-01: TaskSurf beta launch\n- 18:30 \u{2014} Deploy Build #38\n## Q2 2026\n- Wavesite launched\n::\n",
        "::output[for=\"analysis\" timestamp=\"2026-02-10T12:00:00Z\" exit=0 format=chart]\n{\"type\": \"bar\"}\n::\n",
        "::ai-generated[model=\"claude-opus-4\" date=\"2026-02-10\" reviewed=false]\nThis analysis suggests growth.\n::\n",
        "::alternatives\n| Option | Pros | Cons | Verdict |\n|---|---|---|---|\n| GTK4 | 5MB binary | Linux-first | **Selected** |\n| Electron | Cross-platform | 150MB | Rejected \u{2014} bloat |\n::\n",
        "::ai-context[model=\"opus\" tokens=2400 loaded=true]\nHow much context was available.\n::\n",
        "::countdown[date=\"2026-03-15\" label=\"Launch day\"]\n::\n",
        "::css\n.custom-thing { border: 2px dashed red; }\n::\n",
        "::footnote[id=\"1\"]\nGartner, \"Magic Quadrant,\" 2025. Tier 1 source.\n::\n",
        "::kernel[lang=python env=\"analysis\"]\n  runtime: python3.12\n  packages: [numpy, pandas, matplotlib]\n  sandbox: strict\n::\n",
        "::logo-cloud[title=\"Trusted by\"]\n- assets/logos/acme.svg\n- assets/logos/initech.svg | Initech\n::\n",
        "::subscribe[action=\"https://api.example.com/newsletter\" placeholder=\"you@email.com\"]\nGet notified when we launch.\n::\n",
        "::notes\nPause here, ask for questions.\n::\n",
    ] {
        let parsed = surf_parse::parse(src).doc;
        let first = surf_parse::builder::to_surf_source(&parsed);
        let second = surf_parse::builder::to_surf_source(&surf_parse::parse(&first).doc);
        assert_eq!(first, second, "not a fixed point for:\n{src}first pass:\n{first}");
        let html0 = surf_parse::render_html::to_html(&parsed);
        let html1 = surf_parse::render_html::to_html(&surf_parse::parse(&first).doc);
        assert_eq!(html0, html1, "render drifted across the round trip:\n{src}first pass:\n{first}");
        // The typed block itself survives (serde is the equality the enum has).
        let before = serde_json::to_value(&parsed.blocks[0]).unwrap();
        let after = serde_json::to_value(&surf_parse::parse(&first).doc.blocks[0]).unwrap();
        // Spans move with the canonical spelling; everything else must not.
        before.as_object().unwrap().keys().filter(|k| *k != "span").for_each(|k| {
            assert_eq!(before[k], after[k], "field {k} drifted for:\n{src}first pass:\n{first}");
        });
    }
}

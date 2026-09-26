//! The block-edit corpus (0.28.0, `surf_parse::edit`): one `(before, op,
//! after)` triple per verb under `tests/fixtures/edit/`, pinned byte for
//! byte, so a change to how an edit splices the source shows up as a
//! reviewable fixture diff. 0.29.0 adds the text-anchored cases over
//! `text.surf` (eight `replace_text` shapes) and the listing `text.blocks.json`
//! (`list_blocks` with its `text` + `count` columns). Update intentionally with:
//!   UPDATE_EDIT_FIXTURES=1 cargo test --test edit_fixtures

use std::fs;
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/edit")
}

fn read(name: &str) -> String {
    fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pin(verb: &str, before: &str) {
    let op = read(&format!("{verb}.op.json"));
    let actual = surf_parse::apply_edit_json(before, &op).unwrap_or_else(|e| panic!("{verb}: {e}"));
    let path = dir().join(format!("{verb}.after.surf"));
    if std::env::var("UPDATE_EDIT_FIXTURES").is_ok() {
        fs::write(&path, &actual).expect("write fixture");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing {} — run UPDATE_EDIT_FIXTURES=1 cargo test --test edit_fixtures",
            path.display()
        )
    });
    assert_eq!(actual, expected, "{verb}: the edit's result drifted from its pinned fixture");
    // Every edited document still parses clean, and every other block kept
    // its bytes: the parse-level shape is the same modulo the one verb.
    let parsed = surf_parse::parse(&actual);
    assert!(
        parsed.diagnostics.iter().all(|d| d.severity != surf_parse::Severity::Error),
        "{verb}: {:?}",
        parsed.diagnostics
    );
}

#[test]
fn every_verb_is_pinned_over_the_site_fixture() {
    let site = read("site.surf");
    for verb in ["replace", "insert_after", "remove", "move", "set_attr", "set_site_key", "set_text", "replace_text"] {
        pin(verb, &site);
    }
}

#[test]
fn stamp_ids_is_pinned_over_the_unstamped_fixture() {
    let src = read("unstamped.surf");
    pin("stamp_ids", &src);
    let stamped = surf_parse::stamp_ids(&src);
    assert_eq!(surf_parse::stamp_ids(&stamped), stamped, "idempotent");
    let ids: Vec<String> = surf_parse::list_blocks(&stamped).into_iter().filter_map(|b| b.id).collect();
    assert_eq!(ids, vec!["b-nav-1", "b-hero-1", "b-features-1", "second", "b-cta-1"]);
}

#[test]
fn the_site_fixture_lists_its_blocks_by_page() {
    let site = read("site.surf");
    let blocks = surf_parse::list_blocks(&site);
    let home: Vec<(String, Option<String>)> = blocks
        .iter()
        .filter(|b| b.route.as_deref() == Some("/"))
        .map(|b| (b.name.clone(), b.id.clone()))
        .collect();
    assert_eq!(
        home,
        vec![
            ("hero".into(), Some("hero".into())),
            ("features".into(), Some("features".into())),
            ("cta".into(), Some("cta".into())),
        ]
    );
    let top: Vec<String> = blocks.iter().filter(|b| b.depth == 0).map(|b| b.name.clone()).collect();
    assert_eq!(top, vec!["site", "nav", "footer", "page", "page"]);
    // Both pages have a `hero`: the id alone is ambiguous, the route decides.
    let err = surf_parse::apply_edit_json(&site, r#"{"op":"set_text","id":"hero","text":"x"}"#).unwrap_err();
    assert!(matches!(err, surf_parse::EditError::AmbiguousId { count: 2, .. }), "{err}");
}

/// The eight shapes a person quotes (TASK-1040 §P2): each replaces the
/// phrase INSIDE its line, the markup around it kept, and re-parses clean.
#[test]
fn every_replace_text_shape_is_pinned_over_the_text_fixture() {
    let text = read("text.surf");
    for case in [
        "nav_item",
        "card_title",
        "faq_answer",
        "button_label",
        "headline_anchor",
        "loose_paragraph",
        "attr_value",
        "table_cell",
    ] {
        pin(&format!("replace_text_{case}"), &text);
        let after = read(&format!("replace_text_{case}.after.surf"));
        assert_eq!(text.lines().count(), after.lines().count(), "{case}: a text edit never adds or removes a line");
        assert_eq!(text.lines().zip(after.lines()).filter(|(a, b)| a != b).count(), 1, "{case}: exactly one line changed");
        assert_eq!(surf_parse::list_blocks(&text).len(), surf_parse::list_blocks(&after).len(), "{case}: the block list is the same shape");
    }
}

/// `list_blocks` over the text fixture, pinned as JSON: the id, kind, route,
/// depth, lines, span, and the 0.29.0 `text` + `count` columns.
#[test]
fn the_text_fixture_listing_is_pinned() {
    let text = read("text.surf");
    let actual = serde_json::to_string_pretty(&surf_parse::list_blocks(&text)).expect("json") + "\n";
    let path = dir().join("text.blocks.json");
    if std::env::var("UPDATE_EDIT_FIXTURES").is_ok() {
        fs::write(&path, &actual).expect("write fixture");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("missing {} — run UPDATE_EDIT_FIXTURES=1 cargo test --test edit_fixtures", path.display())
    });
    assert_eq!(actual, expected, "the listing drifted from its pinned fixture");
    let listed = surf_parse::list_blocks(&text);
    let cta = listed.iter().find(|b| b.id.as_deref() == Some("cta")).expect("cta");
    assert_eq!(cta.text.as_deref(), Some("Get a quote"));
    let features = listed.iter().find(|b| b.id.as_deref() == Some("features")).expect("features");
    assert_eq!((features.text.as_deref(), features.count), (Some("Fast"), Some(3)));
    // The policy over the fixture: "Learn more" is in three places, never guessed.
    let hits = surf_parse::find_text(&text, "Learn more", None);
    assert_eq!(hits.len(), 3);
    assert!(matches!(surf_parse::resolve_text(&hits, Some("/")), surf_parse::Resolved::Ambiguous(_)));
    let v: serde_json::Value = serde_json::from_str(&surf_parse::find_text_json(&text, "Get a quote", Some("/about"))).unwrap();
    assert_eq!(v["policy"], "current_route");
    assert_eq!(v["pick"]["id"], "cta");
}

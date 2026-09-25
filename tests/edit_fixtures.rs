//! The block-edit corpus (0.28.0, `surf_parse::edit`): one `(before, op,
//! after)` triple per verb under `tests/fixtures/edit/`, pinned byte for
//! byte, so a change to how an edit splices the source shows up as a
//! reviewable fixture diff. Update intentionally with:
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
    for verb in ["replace", "insert_after", "remove", "move", "set_attr", "set_site_key", "set_text"] {
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

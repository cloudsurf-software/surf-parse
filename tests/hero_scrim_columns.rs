//! 0.42.1 — the scrim over a centred hero, and the button brace suffix inside
//! `:::columns` (TASK-2227).

use surf_parse::SURFDOC_CSS;

fn fixture() -> String {
    let path = format!("{}/tests/fixtures/hero-scrim-columns.surf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn html_of(src: &str) -> String {
    surf_parse::parse(src).doc.to_html()
}

fn columns_of(html: &str) -> &str {
    let at = html.find("<div class=\"surfdoc-columns\"").expect("a columns block");
    &html[at..]
}

/// The centred hero is the one stacking context: the scrim at 1, the inner no
/// longer a context, the copy at 2 through a zero-specificity selector.
#[test]
fn the_centred_hero_scrim_paints_over_its_picture() {
    for rule in [
        ".surfdoc-hero:not(.surfdoc-hero-left) { isolation: isolate; }",
        ".surfdoc-hero:not(.surfdoc-hero-left)::before { z-index: 1; }",
        ".surfdoc-hero:not(.surfdoc-hero-left) .surfdoc-hero-inner { z-index: auto; }",
        ":where(.surfdoc-hero:not(.surfdoc-hero-left) .surfdoc-hero-inner) > * { position: relative; z-index: 2; }",
    ] {
        assert!(SURFDOC_CSS.contains(rule), "surfdoc.css lacks {rule}");
    }
    // The scrim itself and the -left hero's inner context are unchanged.
    assert!(SURFDOC_CSS.contains(".surfdoc-hero[style*=\"background-image\"]::before,"));
    assert!(SURFDOC_CSS.contains(".surfdoc-hero-inner { position: relative; z-index: 1; }"));
    // The new rules come after the base ones, so they win the cascade.
    let base = SURFDOC_CSS.find(".surfdoc-hero-inner { position: relative; z-index: 1; }").unwrap();
    let fix = SURFDOC_CSS.find(".surfdoc-hero:not(.surfdoc-hero-left) { isolation: isolate; }").unwrap();
    assert!(fix > base);
}

#[test]
fn a_button_link_in_a_column_is_a_button() {
    let html = html_of(&fixture());
    let cols = columns_of(&html);
    assert!(cols.contains("<div class=\"surfdoc-cta-group\"><a href=\"/plans\" class=\"surfdoc-cta surfdoc-cta-primary\">Choose a plan</a></div>"), "{cols}");
    assert!(cols.contains("<a href=\"/guide\" class=\"surfdoc-cta surfdoc-cta-primary\">Read the guide</a>"), "{cols}");
    // {external} alone is a secondary button that opens a new tab; the blank
    // line between two buttons keeps one group.
    assert!(cols.contains("<a href=\"/guide\" class=\"surfdoc-cta surfdoc-cta-primary\">Read the guide</a><a href=\"https://example.com/ask\" class=\"surfdoc-cta surfdoc-cta-secondary\" target=\"_blank\" rel=\"noopener\">Ask us</a></div>"), "{cols}");
    assert!(!cols.contains("{primary}</"), "no literal flag after a link: {cols}");
    assert!(!cols.contains("{.primary}"), "{cols}");
    assert!(!cols.contains("{external}"), "{cols}");
}

#[test]
fn an_unknown_suffix_a_bare_link_and_code_stay_as_written() {
    let html = html_of(&fixture());
    let cols = columns_of(&html);
    // An unknown flag is not a button: the link stays a link, the braces text.
    assert!(cols.contains(">Odd</a>{shiny}"), "{cols}");
    assert!(!cols.contains("href=\"/odd\" class=\"surfdoc-cta"), "{cols}");
    // A bare link stays an inline link.
    assert!(cols.contains(">Compare</a>"), "{cols}");
    assert!(!cols.contains("href=\"/compare\" class=\"surfdoc-cta"), "{cols}");
    // A fenced code block is never read for buttons.
    assert!(cols.contains("[Not a button](/code){primary}"), "{cols}");
}

#[test]
fn a_column_without_a_button_renders_as_before() {
    let html = html_of("::columns\n:::column\nLeft with a [link](/a).\n:::\n:::column\nRight {not a flag}.\n:::\n::");
    let cols = columns_of(&html);
    assert!(cols.contains("<div class=\"surfdoc-column\"><p>Left with a <a href=\"/a\""), "{cols}");
    assert!(cols.contains("<p>Right {not a flag}.</p>"), "{cols}");
    assert!(!cols.contains("surfdoc-cta"), "{cols}");
}

#[test]
fn an_empty_or_mixed_brace_is_not_a_button() {
    let html = html_of("::columns\n:::column\n[A](/a){}\n[B](/b){primary shiny}\n[C](/c){primary} and more\n:::\n::");
    let cols = columns_of(&html);
    assert!(!cols.contains("surfdoc-cta"), "{cols}");
    assert!(cols.contains("{}"), "{cols}");
    assert!(cols.contains("{primary shiny}"), "{cols}");
    assert!(cols.contains("{primary} and more"), "{cols}");
}

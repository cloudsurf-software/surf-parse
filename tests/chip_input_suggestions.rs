//! 0.44.0 — the chip-input's suggestion list is in the stylesheet (CSS only: the host draws the rows), and the
//! renderer's chip-input markup is exactly what it was. The web's New message (TASK-1982) wears these rules; this
//! test is the seam a consumer's pin gate can trust.

/// Top-level rules only (a line that starts at column 0): the phone block's indented overrides are not counted.
fn rule_count(css: &str, selector: &str) -> usize {
    css.lines().filter(|l| l.starts_with(selector) && l.contains('{')).count()
}

#[test]
fn the_suggestion_list_rules_are_in_the_stylesheet() {
    let css = surf_parse::SURFDOC_CSS;
    for selector in [
        ".surfdoc-chip-input.is-chrome {",
        ".surfdoc-chip-input:focus-within {",
        ".surfdoc-chip-input-suggestions {",
        ".surfdoc-chip-input-suggestions:empty {",
        ".surfdoc-chip-input-option {",
        ".surfdoc-chip-input-option:hover {",
        ".surfdoc-chip-input-option[aria-selected=\"true\"] {",
        ".surfdoc-chip-input-option:focus-visible {",
        ".surfdoc-chip-input-avatar {",
        ".surfdoc-chip-input-avatar img {",
        ".surfdoc-chip-input-name {",
        ".surfdoc-chip-input-email {",
        ".surfdoc-chip-input-empty {",
    ] {
        assert_eq!(rule_count(css, selector), 1, "exactly one rule for `{selector}`");
    }
    // The 0.17 rules stay (one each).
    for selector in [".surfdoc-chip-input {", ".surfdoc-chip-input-label {", ".surfdoc-chip-input-chip {", ".surfdoc-chip-input-remove {", ".surfdoc-chip-input-field {"] {
        assert_eq!(rule_count(css, selector), 1, "the 0.17 rule `{selector}` stays, once");
    }
}

#[test]
fn the_highlight_is_a_wash_and_the_chrome_modifier_drops_the_margin() {
    let css = surf_parse::SURFDOC_CSS;
    let highlight = css.lines().find(|l| l.trim_start().starts_with(".surfdoc-chip-input-option[aria-selected=\"true\"] {")).expect("the highlight rule");
    assert!(highlight.contains("background: var(--accent-soft"), "the highlight is the accent-soft wash: {highlight}");
    assert!(!highlight.contains("border-left") && !highlight.contains("border-top") && !highlight.contains("border-bottom") && !highlight.contains("border-right"), "no one-sided accent border: {highlight}");
    let chrome = css.lines().find(|l| l.trim_start().starts_with(".surfdoc-chip-input.is-chrome {")).expect("the chrome modifier");
    assert!(chrome.contains("margin: 0;"), "`.is-chrome` drops the document margin: {chrome}");
    let option = css.lines().find(|l| l.trim_start().starts_with(".surfdoc-chip-input-option {")).expect("the option rule");
    assert!(option.contains("min-height: 44px") && option.contains("grid-template-columns: 28px minmax(0, 1fr) auto"), "a 44 px three-column row: {option}");
}

#[test]
fn under_640px_the_email_drops_under_the_name_and_the_field_keeps_16px_type() {
    let css = surf_parse::SURFDOC_CSS;
    let start = css.find(".surfdoc-chip-input-empty {").expect("the empty rule");
    let phone = &css[start..css[start..].find("}\n}\n").map(|i| start + i + 4).expect("the phone block closes")];
    assert!(phone.contains("@media (max-width: 640px)"), "the phone block follows the list rules: {phone}");
    assert!(phone.contains(".surfdoc-chip-input-option { grid-template-columns: 28px minmax(0, 1fr); }"), "{phone}");
    assert!(phone.contains(".surfdoc-chip-input-email { grid-column: 2; max-width: none; }"), "{phone}");
    assert!(phone.contains(".surfdoc-chip-input-field { font-size: 16px; }"), "{phone}");
}

#[test]
fn the_renderers_chip_input_markup_is_unchanged() {
    let src = "::chip-input[label=\"To:\" placeholder=\"Name or email\"]\n- Rae Member\n::\n";
    let html = surf_parse::parse(src).doc.to_html();
    assert!(html.contains("<div class=\"surfdoc-chip-input\""), "{html}");
    assert!(html.contains("<span class=\"surfdoc-chip-input-label\">To:</span>"), "{html}");
    assert!(html.contains("<span class=\"surfdoc-chip-input-chip\">Rae Member<button type=\"button\" class=\"surfdoc-chip-input-remove\" aria-label=\"Remove Rae Member\">"), "{html}");
    assert!(html.contains("<input type=\"text\" class=\"surfdoc-chip-input-field\" placeholder=\"Name or email\">"), "{html}");
    // The list is the host's: the renderer never emits it.
    assert!(!html.contains("surfdoc-chip-input-suggestions") && !html.contains("is-chrome"), "{html}");
}

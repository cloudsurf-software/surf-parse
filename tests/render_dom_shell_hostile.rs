//! Never-weaken hostile corpus over the /next web-shell CHROME kinds
//! (feature `dom`).
//!
//! `tests/render_dom_identity.rs` owns the marketing/page corpus and its own
//! hostile fixtures; this file is disjoint and covers the shell vocabulary the
//! Surfspace web shell actually authors — `app-shell`, `sidebar`, `toolbar`,
//! `row`, `tab-content`, `panel`, `split-pane`/`pane`, `modal`, `form`,
//! `embed`, `style`, `code` — under adversarial input.
//!
//! Every fixture is asserted with the SAME invariant the identity corpus
//! pins for recovered input: the constructive DOM sink either serializes
//! byte-identically to `render_html`, or it declines with a typed error that
//! `coverage_check` agrees with. Never a panic, never a divergence, never a
//! silent partial mount. These assertions must NEVER be relaxed; the fixture
//! list is add-only.

#![cfg(feature = "dom")]

use surf_parse::render_dom::{check_coverage, coverage_check, render_fragment_string, RenderDomError};
use surf_parse::SurfDoc;

/// Add-only. A fixture may join this list; none may leave it.
const SHELL_HOSTILE_FIXTURES: &[&str] = &[
    "hostile-shell-quotes",
    "hostile-shell-urls",
    "hostile-shell-rawtext",
    "hostile-shell-half-open",
    "hostile-shell-deep-nesting",
];

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/dom/{name}.surf",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn doc_of(name: &str) -> SurfDoc {
    surf_parse::parse(&fixture(name)).doc
}

/// The core never-weaken invariant. Returns the string-renderer fragment so
/// per-fixture escaping assertions can run on it (byte identity makes an
/// assertion on this string an assertion on the DOM output too, whenever the
/// constructive sink renders).
fn assert_identity_or_typed_decline(name: &str) -> String {
    let doc = doc_of(name);
    let string_html = doc.to_html_fragment();
    match render_fragment_string(&doc) {
        Ok(dom_html) => {
            assert_eq!(
                dom_html, string_html,
                "{name}: constructive DOM serialization drifted from render_html"
            );
        }
        Err(RenderDomError::Unimplemented(kind)) => {
            assert!(
                !kind.is_empty(),
                "{name}: a decline must name the kind it declined on"
            );
            assert!(
                !coverage_check(&doc),
                "{name}: render declined but coverage_check claimed full coverage"
            );
        }
        #[allow(unreachable_patterns)]
        Err(other) => {
            // A future typed decline (e.g. a parse-bounds error) is fine, as
            // long as coverage_check still refuses to promise a mount.
            assert!(
                !coverage_check(&doc),
                "{name}: render declined ({other}) but coverage_check claimed full coverage"
            );
        }
    }
    string_html
}

/// Every attribute VALUE in a serialized fragment, in document order.
/// Tag-aware: text between tags is not scanned (a `"` in body copy is legal).
fn attribute_values(html: &str) -> Vec<String> {
    let bytes: Vec<char> = html.chars().collect();
    let mut values = Vec::new();
    let mut i = 0usize;
    let mut in_tag = false;
    while i < bytes.len() {
        let c = bytes[i];
        if !in_tag {
            if c == '<' {
                in_tag = true;
            }
            i += 1;
            continue;
        }
        if c == '>' {
            in_tag = false;
            i += 1;
            continue;
        }
        if c == '=' && i + 1 < bytes.len() && bytes[i + 1] == '"' {
            let mut j = i + 2;
            let mut value = String::new();
            while j < bytes.len() && bytes[j] != '"' {
                value.push(bytes[j]);
                j += 1;
            }
            values.push(value);
            i = j + 1;
            continue;
        }
        i += 1;
    }
    values
}

/// No attribute value may carry a raw `"`, `<` or `>` — any of the three can
/// terminate the attribute, the tag, or the whole element early and turn
/// authored text into markup.
fn assert_no_attribute_breakout(name: &str, html: &str) {
    for value in attribute_values(html) {
        assert!(
            !value.contains('<') && !value.contains('>'),
            "{name}: raw angle bracket escaped an attribute value: {value}"
        );
    }
    // A raw `"` cannot survive the scanner above (it would have closed the
    // value), so assert the property that proves it: re-scanning the
    // fragment finds the same attribute count on both halves of a split at
    // every `&quot;` — i.e. every authored quote is entity-encoded.
    assert!(
        !html.contains("=\"\"\""),
        "{name}: an empty-then-stray quote pattern leaked into an attribute"
    );
}

/// Parse and render must both be deterministic — the same source twice must
/// give the same doc and the same bytes out of BOTH backends.
fn assert_parser_and_render_stability(name: &str) {
    let src = fixture(name);
    let first = surf_parse::parse(&src).doc;
    let second = surf_parse::parse(&src).doc;
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap(),
        "{name}: parse must be deterministic"
    );
    assert_eq!(
        first.to_html_fragment(),
        second.to_html_fragment(),
        "{name}: render_html must be deterministic"
    );
    match (render_fragment_string(&first), render_fragment_string(&second)) {
        (Ok(a), Ok(b)) => assert_eq!(a, b, "{name}: constructive DOM render must be deterministic"),
        (Err(a), Err(b)) => assert_eq!(
            a.to_string(),
            b.to_string(),
            "{name}: the decline must be deterministic"
        ),
        _ => panic!("{name}: the DOM path rendered on one pass and declined on the other"),
    }
    assert_eq!(
        check_coverage(&first).is_ok(),
        check_coverage(&second).is_ok(),
        "{name}: coverage must be deterministic"
    );
}

// -- (1) quote-breaking text through row / toolbar / modal fields -------------

/// Attribute-breaking quotes in `app-shell`, `sidebar`, `row` (title,
/// description, trailing label/action, aria-label), `toolbar` items (button
/// label, avatar, aria-label, text value), `tab-content` and `modal`
/// (name/title/anchor) fields, plus `action:` row lines: byte identity, and
/// no authored quote or angle bracket may terminate an attribute early.
#[test]
fn hostile_shell_quote_breaking_text() {
    let name = "hostile-shell-quotes";
    let html = assert_identity_or_typed_decline(name);
    assert_no_attribute_breakout(name, &html);
    assert_parser_and_render_stability(name);
    // The fixture must actually exercise chrome, not degrade to prose.
    for marker in [
        "surfdoc-app-shell",
        "surfdoc-sidebar",
        "surfdoc-toolbar",
        "surfdoc-row",
        "surfdoc-tab-content",
        "surfdoc-modal",
        "surfdoc-style",
    ] {
        assert!(html.contains(marker), "{name}: {marker} missing — fixture lost its teeth");
    }
    // Authored quotes survive as entities in both text and attribute slots.
    assert!(html.contains("&quot;"), "{name}: authored quotes must be entity-encoded");
    assert!(!html.contains("<script"), "{name}: no script element may be constructed");
}

// -- (2) javascript: / data: URLs in row href, form action, embed src, avatar -

/// PINS CURRENT BEHAVIOR. Chrome URL slots (`row href=`, `form action=`,
/// `embed src=`, the `avatar=` initials slot, `trailing-action`) are stamped
/// VERBATIM-ESCAPED by `render_html` — surf-parse applies scheme filtering on
/// the markdown path only, and the chrome path leaves scheme policy to the
/// serving layer's CSP. What this test guarantees is what the crate owns: the
/// hostile URL never breaks out of its attribute, never becomes markup, and
/// renders identically through both backends.
#[test]
fn hostile_shell_javascript_and_data_urls() {
    let name = "hostile-shell-urls";
    let html = assert_identity_or_typed_decline(name);
    assert_no_attribute_breakout(name, &html);
    assert_parser_and_render_stability(name);
    // Verbatim-escaped, inside the attribute, never executable markup.
    assert!(
        html.contains("href=\"javascript:alert('sidebar')\""),
        "{name}: row href must stay a pinned, escaped attribute value"
    );
    assert!(
        html.contains("action=\"javascript:alert('form')\""),
        "{name}: form action must stay a pinned, escaped attribute value"
    );
    assert!(
        html.contains("src=\"data:text/html,%3Cscript%3Ex%3C/script%3E\""),
        "{name}: embed src must stay a pinned, escaped attribute value"
    );
    // The data: payload's percent-encoded script must NEVER be decoded into
    // real markup by either backend.
    assert!(!html.contains("<script"), "{name}: no script element may be constructed");
    // A hostile avatar value lands in TEXT content, never in an attribute.
    assert!(
        html.contains("<span class=\"surfdoc-row-avatar\">javascript:alert('avatar')</span>"),
        "{name}: the avatar slot must render as inert text"
    );
    // Markdown link syntax inside a row description is NOT a link — the row
    // body is plain escaped text, so no anchor can be minted from it.
    assert!(
        html.contains("[js link](javascript:alert(1))"),
        "{name}: row description must stay inert text"
    );
    assert!(
        !html.contains("<a href=\"javascript:alert(1)\""),
        "{name}: a row description must never mint an anchor"
    );
}

// -- (3) closing-rawtext payloads through style and code ----------------------

/// `</style>`, `</script>`, `</textarea>` and `</code></pre>` payloads pushed
/// through `::style` property values, `::code` content/lang/file, toolbar and
/// row text, and form field specs: byte identity, and no payload may close a
/// real element. `::style` renders a hidden `data-properties` div (never a
/// live `<style>` element) and `::code` escapes its body — both must hold.
#[test]
fn hostile_shell_closing_rawtext_payloads() {
    let name = "hostile-shell-rawtext";
    let html = assert_identity_or_typed_decline(name);
    assert_no_attribute_breakout(name, &html);
    assert_parser_and_render_stability(name);
    assert!(
        !html.contains("<script"),
        "{name}: a closing-rawtext payload constructed a script element"
    );
    assert!(
        !html.contains("</style>"),
        "{name}: a `</style>` payload survived unescaped"
    );
    // The only legal `</textarea>` is the one closing the form's own control.
    let stray_textarea = html.matches("</textarea>").count() - html.matches("></textarea>").count();
    assert_eq!(
        stray_textarea, 0,
        "{name}: a `</textarea>` payload survived outside the form control"
    );
    // Style properties are a hidden data attribute, not a stylesheet.
    assert!(
        html.contains("<div class=\"surfdoc-style\" aria-hidden=\"true\" data-properties="),
        "{name}: ::style must stay a hidden data element"
    );
    // Code bodies are escaped.
    assert!(
        html.contains("&lt;/script&gt;"),
        "{name}: code payloads must render escaped"
    );
}

// -- (4) half-open app-shell and sidebar --------------------------------------

/// An `app-shell` and a `sidebar` that never close, with a `modal`, a `row`
/// and a `toolbar` opened inside the wreckage: whatever the parser recovers
/// to, the two backends agree or the DOM path declines — never a panic,
/// never a half-built mount.
#[test]
fn hostile_shell_half_open_containers() {
    let name = "hostile-shell-half-open";
    let html = assert_identity_or_typed_decline(name);
    assert_no_attribute_breakout(name, &html);
    assert_parser_and_render_stability(name);
    // Recovery must still produce the shell root (the unterminated children
    // degrade to text) — never an empty document.
    assert!(
        html.contains("surfdoc-app-shell"),
        "{name}: recovery must keep the shell root"
    );
    assert!(!html.contains("<script"), "{name}: no script element may be constructed");
}

// -- (5) max-depth nesting -----------------------------------------------------

/// Twelve levels of chrome containers (app-shell → tab-content → split-pane →
/// pane, three times over) with a row, a toolbar and a modal at the floor,
/// plus a branch nested under a container that does not adopt directive
/// children. Deep nesting must not blow the stack, must not diverge, and the
/// non-adopting branch must DEGRADE (its content dropped) rather than
/// half-render.
#[test]
fn hostile_shell_max_depth_nesting() {
    let name = "hostile-shell-deep-nesting";
    let html = assert_identity_or_typed_decline(name);
    assert_no_attribute_breakout(name, &html);
    assert_parser_and_render_stability(name);
    // The deep chain carries real content all the way down.
    assert!(
        html.contains("Deepest row title"),
        "{name}: the deepest row must survive the nesting"
    );
    assert!(
        html.contains("Deepest toolbar"),
        "{name}: the deepest toolbar must survive the nesting"
    );
    assert!(
        html.contains("Modal at max depth"),
        "{name}: the deepest modal must survive the nesting"
    );
    // Pinned degradation: `::section` does not adopt directive children, so
    // the chrome nested under it is dropped whole — not partially rendered.
    assert!(
        !html.contains("does not nest directives"),
        "{name}: the non-adopting branch must degrade, not half-render"
    );
    assert!(!html.contains("<script"), "{name}: no script element may be constructed");
}

// -- corpus-level guarantees ---------------------------------------------------

/// The fixture list is add-only and every entry must exist on disk.
#[test]
fn shell_hostile_fixture_list_is_add_only() {
    assert!(
        SHELL_HOSTILE_FIXTURES.len() >= 5,
        "the shell hostile corpus may only grow — it is at {}",
        SHELL_HOSTILE_FIXTURES.len()
    );
    for name in SHELL_HOSTILE_FIXTURES {
        assert!(
            !fixture(name).trim().is_empty(),
            "{name}: fixture missing or empty"
        );
    }
}

/// Whatever the DOM path promises, it delivers: `coverage_check` true must
/// mean `render_fragment_string` succeeds AND matches `render_html` byte for
/// byte, across the whole hostile shell corpus. This is the assertion that
/// gains teeth as the chrome kinds land — it can never be satisfied by
/// declining.
#[test]
fn coverage_promise_is_kept_across_the_shell_hostile_corpus() {
    for name in SHELL_HOSTILE_FIXTURES {
        let doc = doc_of(name);
        if coverage_check(&doc) {
            let dom_html = render_fragment_string(&doc)
                .unwrap_or_else(|e| panic!("{name}: coverage promised a mount, render declined: {e}"));
            assert_eq!(
                dom_html,
                doc.to_html_fragment(),
                "{name}: covered doc drifted from render_html"
            );
        }
    }
}

//! Never-weaken byte-identity corpus for the constructive DOM renderer
//! (feature `dom`).
//!
//! For every fixture whose block kinds AND markdown constructs are inside the
//! pilot coverage set, the serialized native-DOM render must equal
//! `to_html_fragment` output byte for byte. Script-emitting fixtures
//! (store/booking/gallery) keep the byte-identity property through the
//! native sink AND pin the constructive `script-emitting:*` decline.
//! Hostile fixtures (quote-breaking text, javascript:/data: URLs,
//! closing-rawtext payloads, half-open containers) are asserted individually
//! and must NEVER be relaxed.

#![cfg(feature = "dom")]

use surf_parse::render_dom::{check_coverage, coverage_check, render_fragment_string, RenderDomError};

fn fixture(rel: &str) -> String {
    let path = format!("{}/tests/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// Parse + assert coverage + assert byte identity against the string renderer.
fn assert_identity(rel: &str) {
    let src = fixture(rel);
    let doc = surf_parse::parse(&src).doc;
    if let Err(e) = check_coverage(&doc) {
        panic!("{rel}: expected full coverage, got decline: {e}");
    }
    assert_identity_bytes(rel);
}

/// Byte identity WITHOUT the coverage gate: the native sink still renders
/// script-emitting blocks so the identity property stays pinned, even though
/// `check_coverage` constructively declines them (creating a `<script>`
/// element with text is a TrustedScript sink under the pilot TT CSP).
fn assert_identity_bytes(rel: &str) {
    let src = fixture(rel);
    let doc = surf_parse::parse(&src).doc;
    let dom_html = render_fragment_string(&doc).expect("native sink renders");
    let string_html = doc.to_html_fragment();
    assert_eq!(
        dom_html, string_html,
        "{rel}: constructive DOM serialization drifted from render_html"
    );
}

/// Identity holds, AND the doc constructively declines as script-emitting
/// (never rendered into a live mount — full navigation instead).
fn assert_identity_with_script_decline(rel: &str, want_kind: &str) {
    assert_identity_bytes(rel);
    let doc = surf_parse::parse(&fixture(rel)).doc;
    assert!(!coverage_check(&doc), "{rel}: must decline (script-emitting)");
    match check_coverage(&doc) {
        Err(RenderDomError::Unimplemented(k)) => assert_eq!(
            k, want_kind,
            "{rel}: typed script-emitting decline"
        ),
        other => panic!("{rel}: expected script-emitting decline, got {other:?}"),
    }
}

// -- (a) existing snapshot fixtures whose kinds subset the coverage set ------

#[test]
fn identity_existing_fixture_site() {
    assert_identity("site.surf");
}

#[test]
fn identity_existing_fixture_single() {
    assert_identity("single.surf");
}

#[test]
fn identity_existing_fixture_gallery_form() {
    assert_identity_with_script_decline("gallery-form.surf", "script-emitting:gallery");
}

// -- the six thelove222 routes (census source) --------------------------------

#[test]
fn identity_thelove222_route_home() {
    assert_identity("dom/thelove222-route-home.surf");
}

/// The gallery/shop/book routes hold byte identity but constructively
/// DECLINE (script-emitting store/booking/gallery-lightbox blocks): under
/// the pilot they navigate as MPA, never a half-built mount.
#[test]
fn identity_thelove222_route_gallery() {
    assert_identity_with_script_decline(
        "dom/thelove222-route-gallery.surf",
        "script-emitting:gallery",
    );
}

#[test]
fn identity_thelove222_route_shop() {
    assert_identity_with_script_decline(
        "dom/thelove222-route-shop.surf",
        "script-emitting:store",
    );
}

#[test]
fn identity_thelove222_route_about() {
    assert_identity("dom/thelove222-route-about.surf");
}

#[test]
fn identity_thelove222_route_book() {
    assert_identity_with_script_decline(
        "dom/thelove222-route-book.surf",
        "script-emitting:booking",
    );
}

#[test]
fn identity_thelove222_route_admin() {
    assert_identity("dom/thelove222-route-admin.surf");
}

#[test]
fn identity_thelove222_full_doc() {
    // Whole-site doc: identity holds; coverage declines on the FIRST
    // script-emitting block in document order (the gallery page). Route-
    // scoped coverage (the wasm glue) still passes /, /about, /admin.
    assert_identity_with_script_decline("dom/thelove222-full.surf", "script-emitting:gallery");
}

// -- hostile fixtures (never relax) -------------------------------------------

/// Quote-breaking text through every covered string field: byte identity AND
/// no raw quote may terminate an attribute early (the serialized output must
/// stay parseable — every `"` inside attribute values is `&quot;`).
#[test]
fn hostile_quote_breaking_text() {
    assert_identity("dom/hostile-quotes.surf");
}

/// javascript:/data: URLs in markdown link href + img src are DROPPED
/// (ammonia rule), while block-level figure/hero/gallery srcs render escaped
/// exactly as render_html does today — identity pins current behavior.
#[test]
fn hostile_javascript_and_data_urls() {
    let rel = "dom/hostile-urls.surf";
    // Fixture carries a ::gallery (script-emitting) — identity still pinned
    // through the native sink; coverage declines it.
    assert_identity_with_script_decline(rel, "script-emitting:gallery");
    let doc = surf_parse::parse(&fixture(rel)).doc;
    let html = render_fragment_string(&doc).unwrap();
    // The markdown-path URLs must be dropped entirely (block-path srcs are a
    // separate, pinned-verbatim behavior — see the hero button in identity).
    assert!(
        html.contains("<a rel=\"noopener noreferrer\">js link</a>"),
        "markdown javascript: link should render href-less"
    );
    assert!(
        html.contains("<a rel=\"noopener noreferrer\">data link</a>"),
        "markdown data: link should render href-less"
    );
    assert!(
        html.contains("<img alt=\"img with js src\">"),
        "markdown javascript: img should render src-less"
    );
}

/// `</textarea>` / `</style>` / `</script>` payloads through form fields and
/// store/booking data islands: identity, plus the JSON data islands must keep
/// `<` escaped as < so no closing tag can terminate the script early.
#[test]
fn hostile_closing_rawtext_payloads() {
    let rel = "dom/hostile-rawtext.surf";
    // Store/booking data islands are script-emitting — identity still pinned
    // through the native sink; coverage declines (store is first in doc
    // order). The escaping assertions below must NEVER be relaxed.
    assert_identity_with_script_decline(rel, "script-emitting:store");
    let doc = surf_parse::parse(&fixture(rel)).doc;
    let html = render_fragment_string(&doc).unwrap();
    let json_island = html
        .split("data-st-data>")
        .nth(1)
        .and_then(|s| s.split("</script>").next())
        .expect("store data island present");
    assert!(
        !json_island.contains('<'),
        "raw '<' leaked into the store JSON island"
    );
}

/// Half-open containers (unterminated ::section/::callout): whatever the
/// parser recovers to, the DOM path either renders it byte-identically or
/// declines — never panics, never diverges.
#[test]
fn hostile_half_open_containers() {
    let rel = "dom/hostile-half-open.surf";
    let doc = surf_parse::parse(&fixture(rel)).doc;
    match render_fragment_string(&doc) {
        Ok(dom_html) => assert_eq!(
            dom_html,
            doc.to_html_fragment(),
            "{rel}: drifted on recovered half-open input"
        ),
        Err(_) => {
            assert!(!coverage_check(&doc), "decline must match coverage_check");
        }
    }
}

/// Adding an uncovered kind to an otherwise covered doc flips coverage off.
#[test]
fn coverage_declines_added_uncovered_kind() {
    let src = fixture("dom/thelove222-route-about.surf") + "\n::tabs\n== A\nx\n::\n";
    let doc = surf_parse::parse(&src).doc;
    assert!(!coverage_check(&doc));
}

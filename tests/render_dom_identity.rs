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

/// 0.19.2 `::data` preview contract: a 30-row block (capped tbody, kept
/// tfoot, `surfdoc-table-preview` + `data-rows`/`data-cols`, trailing
/// count line) must serialize identically through both web backends.
#[test]
fn identity_data_preview_thirty_rows() {
    assert_identity("data-preview.surf");
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

/// 0.18: the per-size-class `data-cols*` attribute set must be mirrored in
/// the constructive DOM path byte-for-byte — a varying triple AND the
/// uniform single value that every pre-0.18 document carries.
#[test]
fn per_class_cols_attrs_are_mirrored_in_the_dom_path() {
    assert_identity("dom/per-class-cols.surf");
    let src = fixture("dom/per-class-cols.surf");
    let doc = surf_parse::parse(&src).doc;
    let html = doc.to_html_fragment();
    assert!(
        html.contains("data-cols=\"1\" data-cols-tablet=\"2\" data-cols-desktop=\"3\""),
        "varying triple must widen: {html}"
    );
    assert!(
        html.contains("data-cols=\"3\">"),
        "a uniform value must stay the bare pre-0.18 attribute: {html}"
    );
}

/// 0.18.1 form vocabulary: the five new control types, the radio option
/// group, the label-less hidden field, and `group:` fieldsets must serialize
/// identically through the constructive DOM sink.
#[test]
fn form_controls_and_fieldsets_are_byte_identical() {
    assert_identity("dom/form-controls.surf");
}

/// 0.18.1 block addressing: `data-block-id` / `aria-label` are spliced into
/// the block root's opening tag by the string renderer and set on the root
/// element before any other attribute by the constructive renderer — the two
/// orders must agree, escaped values included.
#[test]
fn block_ids_and_labels_are_byte_identical() {
    assert_identity("dom/block-ids.surf");
}


// ===========================================================================
// Web-shell corpus (0.19.0) — the Surfspace /next shell sources, vendored
// [never-weaken]. The list below is ADD-ONLY: a fixture may be added, never
// removed, and `web_shell_fixture_list_is_complete` fails if a `.surf` lands
// in tests/fixtures/web-shell/ without being listed here.
//
// The copies are name-neutralized (prose/text runs only — no directive,
// attribute or nesting change); see tests/fixtures/web-shell/README.md (resync tool lives in the private app repo).
// ===========================================================================

/// Every vendored web-shell source. ADD-ONLY.
const WEB_SHELL_FIXTURES: &[&str] = &[
    "web-shell/modals/add-model-server.surf",
    "web-shell/modals/assign.surf",
    "web-shell/modals/attach-menu.surf",
    "web-shell/modals/connect-domain.surf",
    "web-shell/modals/create.surf",
    "web-shell/modals/delete-account.surf",
    "web-shell/modals/delete-workspace.surf",
    "web-shell/modals/device-detail.surf",
    "web-shell/modals/device-files.surf",
    "web-shell/modals/doc-menu.surf",
    "web-shell/modals/doc-task-picker.surf",
    "web-shell/modals/enroll-device.surf",
    "web-shell/modals/file-preview.surf",
    "web-shell/modals/filter.surf",
    "web-shell/modals/hub.surf",
    "web-shell/modals/link-task.surf",
    "web-shell/modals/media-picker.surf",
    "web-shell/modals/move-copy.surf",
    "web-shell/modals/participants.surf",
    "web-shell/modals/passkeys.surf",
    "web-shell/modals/photo-detail.surf",
    "web-shell/modals/plan-ladder.surf",
    "web-shell/modals/quiet-hours.surf",
    "web-shell/modals/react-report.surf",
    "web-shell/modals/rename.surf",
    "web-shell/modals/search.surf",
    "web-shell/modals/share.surf",
    "web-shell/modals/surfy-chats.surf",
    "web-shell/modals/top-up.surf",
    "web-shell/modals/version-history.surf",
    "web-shell/modals/workspace-switcher.surf",
    "web-shell/shell.surf",
    "web-shell/surfaces/account-settings.surf",
    "web-shell/surfaces/app-detail.surf",
    "web-shell/surfaces/apps.surf",
    "web-shell/surfaces/archive.surf",
    "web-shell/surfaces/deploy-lane.surf",
    "web-shell/surfaces/devices.surf",
    "web-shell/surfaces/doc-detail.surf",
    "web-shell/surfaces/doc-editor.surf",
    "web-shell/surfaces/docs.surf",
    "web-shell/surfaces/email.surf",
    "web-shell/surfaces/files.surf",
    "web-shell/surfaces/members.surf",
    "web-shell/surfaces/messages.surf",
    "web-shell/surfaces/notifications.surf",
    "web-shell/surfaces/post-compose.surf",
    "web-shell/surfaces/posts.surf",
    "web-shell/surfaces/report-bug.surf",
    "web-shell/surfaces/search.surf",
    "web-shell/surfaces/task-detail.surf",
    "web-shell/surfaces/tasks.surf",
    "web-shell/surfaces/thread.surf",
    "web-shell/surfaces/trash.surf",
    "web-shell/surfaces/tutorial.surf",
    "web-shell/surfaces/workspace-settings.surf",
];

/// Web-shell sources that carry a script-emitting block (`::gallery`) and so
/// constructively DECLINE: identity still holds through the native sink, but
/// the live mount refuses and the shell navigates instead. ADD-ONLY.
const WEB_SHELL_SCRIPT_DECLINE: &[(&str, &str)] = &[
    ("web-shell/modals/media-picker.surf", "script-emitting:gallery"),
    // 0.19.0: web-shell/shell.surf moved OUT of this list — the drawer
    // toggle is runtime-owned (no inline `<script>`), so the shell root's
    // direct right `::panel` no longer makes the doc script-emitting and
    // the composed /next chrome COVERS (the arming condition for
    // constructive navigation, TASK-267).
    ("web-shell/surfaces/files.surf", "script-emitting:gallery"),
];

fn web_shell_decline_kind(rel: &str) -> Option<&'static str> {
    WEB_SHELL_SCRIPT_DECLINE
        .iter()
        .find(|(f, _)| *f == rel)
        .map(|(_, k)| *k)
}

/// Parser-stability pin (NEVER weaken): fail if any block-level start tag
/// opens while a `<p>` is still open. The HTML parser auto-closes the open
/// `<p>` and hoists the block out as a sibling, so the SSR-parsed DOM would
/// diverge from the literal tree the constructive renderer builds — the
/// serialized bytes can match while the DOMs do not.
fn assert_parser_stable(rel: &str, html: &str) {
    const BLOCK_TAGS: &[&str] = &[
        "p", "ul", "ol", "div", "blockquote", "pre", "table", "section", "article", "aside",
        "nav", "header", "footer", "form", "fieldset", "figure", "hr", "h1", "h2", "h3", "h4",
        "h5", "h6", "dl", "details", "main", "address",
    ];
    let mut p_depth = 0usize;
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let closing = bytes.get(i + 1) == Some(&b'/');
        let name_start = if closing { i + 2 } else { i + 1 };
        let mut j = name_start;
        while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
            j += 1;
        }
        let name = html[name_start..j].to_ascii_lowercase();
        if name == "p" {
            if closing {
                p_depth = p_depth.saturating_sub(1);
            } else {
                assert!(p_depth == 0, "{rel}: parser-stability violation: <p> inside <p> at byte {i}");
                p_depth += 1;
            }
        } else if !closing && p_depth > 0 && BLOCK_TAGS.contains(&name.as_str()) {
            panic!("{rel}: parser-stability violation: <{name}> inside an open <p> at byte {i}");
        }
        i = j.max(i + 1);
    }
}

/// Count `<name` opens and `</name` closes so a rawtext payload cannot smuggle
/// an unbalanced closing tag through a covered string field.
fn tag_balance(html: &str, name: &str) -> (usize, usize) {
    // `</name` never matches `<name` (the `/` is in the way), so the open
    // count must NOT have the close count subtracted from it — with the
    // subtraction the pair could only ever balance at zero, i.e. the check
    // silently degraded to "this element must be absent" and fired on a form
    // that legitimately renders one `<textarea>…</textarea>`.
    let closes = html.matches(&format!("</{name}")).count();
    let opens = html.matches(&format!("<{name}")).count();
    (opens, closes)
}

fn walk_web_shell_fixtures() -> Vec<String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read fixture dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some("hostile") {
                    continue;
                }
                walk(&path, root, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("surf") {
                let rel = path.strip_prefix(root).expect("under root");
                out.push(format!("web-shell/{}", rel.display()));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/web-shell");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out.sort();
    out
}

/// The listed corpus must name every vendored source on disk — a fixture can
/// never be quietly dropped from the never-weaken set.
#[test]
fn web_shell_fixture_list_is_complete() {
    let on_disk = walk_web_shell_fixtures();
    let listed: Vec<String> = WEB_SHELL_FIXTURES.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        on_disk, listed,
        "tests/fixtures/web-shell/ and WEB_SHELL_FIXTURES disagree (the list is ADD-ONLY)"
    );
    assert_eq!(listed.len(), 56, "web-shell census measured 2026-08-26: 1 shell + 24 surfaces + 31 modals");
}

/// Byte identity over the whole vendored shell corpus: the constructive DOM
/// serialization must equal `to_html_fragment` for every source the /next
/// shell actually ships.
#[test]
fn web_shell_corpus_is_byte_identical() {
    for rel in WEB_SHELL_FIXTURES {
        assert_identity_bytes(rel);
    }
}

/// Coverage pin: every web-shell source that carries no script-emitting block
/// must pass `check_coverage` — the takeover gate is allowed to say yes.
#[test]
fn web_shell_corpus_coverage_is_ok() {
    let mut declined = Vec::new();
    for rel in WEB_SHELL_FIXTURES {
        if web_shell_decline_kind(rel).is_some() {
            continue;
        }
        let doc = surf_parse::parse(&fixture(rel)).doc;
        if let Err(e) = check_coverage(&doc) {
            declined.push(format!("{rel}: {e:?}"));
        }
    }
    assert!(declined.is_empty(), "web-shell sources must be covered: {declined:#?}");
}

/// The script-emitting shell surfaces keep byte identity AND the typed
/// decline (never a half-built live mount).
#[test]
fn web_shell_script_emitting_surfaces_decline() {
    for (rel, kind) in WEB_SHELL_SCRIPT_DECLINE {
        assert_identity_with_script_decline(rel, kind);
    }
}

/// Parser stability over the whole shell corpus, in BOTH backends.
#[test]
fn web_shell_corpus_is_parser_stable() {
    for rel in WEB_SHELL_FIXTURES {
        let doc = surf_parse::parse(&fixture(rel)).doc;
        assert_parser_stable(rel, &doc.to_html_fragment());
        if let Ok(dom_html) = render_fragment_string(&doc) {
            assert_parser_stable(rel, &dom_html);
        }
    }
}

/// Parsing is deterministic: two independent parses of the same source render
/// byte-identically through both backends.
#[test]
fn web_shell_corpus_parse_is_deterministic() {
    for rel in WEB_SHELL_FIXTURES {
        let src = fixture(rel);
        let a = surf_parse::parse(&src).doc;
        let b = surf_parse::parse(&src).doc;
        assert_eq!(a.to_html_fragment(), b.to_html_fragment(), "{rel}: string render not deterministic");
        assert_eq!(
            render_fragment_string(&a).ok(),
            render_fragment_string(&b).ok(),
            "{rel}: constructive render not deterministic"
        );
    }
}

// -- hostile corpus over the NEW web-shell kinds (never relax) ---------------

/// Quote-breaking text through every covered web-shell string field: byte
/// identity, and no raw `"` may terminate an attribute early.
#[test]
fn hostile_web_shell_quote_breaking() {
    let rel = "web-shell/hostile/quotes.surf";
    assert_identity_bytes(rel);
    assert_parser_stable(rel, &surf_parse::parse(&fixture(rel)).doc.to_html_fragment());
}

/// javascript:/data: URLs in row href, toolbar-button action/avatar, form and
/// search action, figure/embed src: byte identity plus the markdown-path drop.
#[test]
fn hostile_web_shell_urls() {
    let rel = "web-shell/hostile/urls.surf";
    assert_identity_bytes(rel);
    let doc = surf_parse::parse(&fixture(rel)).doc;
    let html = render_fragment_string(&doc).expect("native sink renders");
    assert!(
        html.contains("<a rel=\"noopener noreferrer\">js link</a>"),
        "markdown javascript: link should render href-less"
    );
    assert!(
        html.contains("<img alt=\"img with js src\">"),
        "markdown javascript: img should render src-less"
    );
}

/// `</textarea>` / `</style>` / `</script>` payloads through form fields, the
/// style block, code, chip/chat inputs, diagram and chart: byte identity, and
/// no smuggled closing tag may unbalance a rawtext element.
#[test]
fn hostile_web_shell_closing_rawtext() {
    let rel = "web-shell/hostile/rawtext.surf";
    assert_identity_bytes(rel);
    let doc = surf_parse::parse(&fixture(rel)).doc;
    let html = render_fragment_string(&doc).expect("native sink renders");
    for tag in ["textarea", "style", "script"] {
        let (opens, closes) = tag_balance(&html, tag);
        assert_eq!(
            opens, closes,
            "{rel}: <{tag}> opens/closes unbalanced — a rawtext payload escaped"
        );
    }
}

/// Half-open web-shell containers: whatever the parser recovers to, the DOM
/// path either renders byte-identically or declines — never panics, never
/// diverges.
#[test]
fn hostile_web_shell_half_open_containers() {
    let rel = "web-shell/hostile/half-open.surf";
    let doc = surf_parse::parse(&fixture(rel)).doc;
    match render_fragment_string(&doc) {
        Ok(dom_html) => assert_eq!(dom_html, doc.to_html_fragment(), "{rel}: drifted on recovery"),
        Err(_) => assert!(!coverage_check(&doc), "{rel}: decline must match coverage_check"),
    }
}

/// Deepest nesting the shell actually uses (app-shell → tab-content →
/// split-pane → pane → toolbar/chat-thread/embed/chat-input-simple): every
/// nested block must carry the SAME `data-block-id` in both backends.
#[test]
fn hostile_web_shell_max_depth_nesting() {
    let rel = "web-shell/hostile/max-depth.surf";
    assert_identity_bytes(rel);
    assert_parser_stable(rel, &surf_parse::parse(&fixture(rel)).doc.to_html_fragment());
}

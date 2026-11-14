//! The interaction grammar (0.47.0): `surface=`, `key=`, `focus=`,
//! `reveal=`, `native=`, `pane=` on any block (and `key=` / `focus=` /
//! `reveal=` on a toolbar button), `shortcuts=` / `panes=` / `panes-at=` on
//! `::app-shell` — HTML's `data-*`, the DOM twin, the native side table and
//! toolbar items, the serializer, and the lint rules L050–L055.

use surf_parse::{check, parse, Block, ToolbarItem};

const CORPUS: &str = include_str!("corpus/tier11-interaction.surf");

const SHELL: &str = "::app-shell[layout=adaptive panes=two shortcuts=\"Mod+1=#surfy mod+k=openSearch\" surface=chrome.shell]\n\n::: toolbar[title=\"App\"]\n- button[label=\"New\" action=openCreate key=\"shift+mod+n\" focus=2 reveal=hover]\n:::\n\n::: sidebar[position=left pane=start]\n::::row[icon=terminal href=#terminal focus=1 native=terminal surface=panel.terminal]\nTerminal\n::::\n:::\n\n::: panel-slot[role=work kind=desk pane=end]\n::::tab-bar\n::::\n:::\n\n::\n";

fn html(src: &str) -> String {
    parse(src).doc.to_html_fragment()
}

fn codes(src: &str, code: &str) -> Vec<String> {
    check(src)
        .diagnostics
        .into_iter()
        .filter(|d| d.code.as_deref() == Some(code))
        .map(|d| d.message)
        .collect()
}

// ── HTML ───────────────────────────────────────────────────────────────────

#[test]
fn the_shell_carries_its_chords_panes_and_surface() {
    let out = html(SHELL);
    assert!(
        out.contains(
            "data-surface=\"chrome.shell\" data-panes=\"two\" data-panes-at=\"tablet\" \
             data-shortcuts=\"mod+1=#surfy mod+k=openSearch\""
        ),
        "{out}"
    );
}

#[test]
fn a_block_carries_its_focus_native_pane_and_surface() {
    let out = html(SHELL);
    assert!(
        out.contains("data-surface=\"panel.terminal\" data-focus=\"1\" data-native=\"terminal\""),
        "{out}"
    );
    assert!(out.contains("data-pane=\"start\""), "{out}");
    assert!(out.contains("data-pane=\"end\""), "{out}");
}

#[test]
fn a_toolbar_button_carries_its_chord_focus_and_reveal_after_aria_pressed() {
    let out = html(SHELL);
    assert!(
        out.contains(
            "data-action=\"openCreate\" data-key=\"mod+shift+n\" data-focus=\"2\" data-reveal=\"hover\""
        ),
        "{out}"
    );
}

#[test]
fn values_that_do_not_read_reach_no_output() {
    let src = "::app-shell[layout=adaptive panes=three panes-at=wide shortcuts=\"cmd+1=#a mod+2\" surface=Shell]\n\n::: toolbar\n- button[label=\"X\" key=cmd+x focus=zero reveal=peek]\n:::\n\n::: sidebar[pane=middle native=printer focus=0]\n:::\n\n::\n";
    let out = html(src);
    for attr in ["data-surface", "data-key", "data-focus", "data-reveal", "data-native", "data-pane", "data-shortcuts"] {
        assert!(!out.contains(attr), "{attr} leaked: {out}");
    }
}

#[test]
fn a_cite_keeps_its_key_and_a_row_reads_a_chord() {
    let cite = html("::cite[key=smith2020 type=article]\ntitle: T\n::\n");
    assert!(!cite.contains("data-key"), "{cite}");
    let row = html("::row[icon=doc href=#a key=mod+o]\nOpen\n::\n");
    assert!(row.contains("data-key=\"mod+o\""), "{row}");
}

#[test]
fn shell_only_attributes_are_read_on_the_shell_alone() {
    let out = html("::callout[type=info shortcuts=\"mod+1=#a\" panes=two]\nHi\n::\n");
    assert!(!out.contains("data-shortcuts") && !out.contains("data-panes"), "{out}");
}

#[test]
fn a_document_without_the_grammar_renders_as_before() {
    let src = "::app-shell[layout=adaptive]\n\n::: toolbar[title=\"App\"]\n- button[label=\"New\" action=openCreate]\n:::\n\n::\n";
    let out = html(src);
    assert!(!out.contains("data-key") && !out.contains("data-surface") && !out.contains("data-focus"), "{out}");
}

#[test]
fn hostile_targets_are_escaped() {
    let out = html("::app-shell[layout=adaptive shortcuts=\"mod+1=\\\"><script>x</script>\"]\n::\n");
    assert!(!out.contains("<script>x"), "{out}");
}

// ── The DOM twin ───────────────────────────────────────────────────────────

#[cfg(feature = "dom")]
#[test]
fn the_dom_twin_is_byte_identical() {
    use surf_parse::render_dom::render_fragment_string;
    for src in [SHELL, CORPUS] {
        let doc = parse(src).doc;
        assert_eq!(render_fragment_string(&doc).expect("renders"), doc.to_html_fragment(), "{src}");
    }
}

// ── Native ─────────────────────────────────────────────────────────────────

#[cfg(feature = "native")]
#[test]
fn the_native_side_table_carries_the_grammar() {
    use surf_parse::render_native::{to_native_block_meta, NativeShortcut, NATIVE_DOC_SCHEMA_VERSION};
    assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 19);
    let doc = parse(SHELL).doc;
    let meta = to_native_block_meta(&doc);
    let shell = meta.iter().find(|m| m.surface_id.as_deref() == Some("chrome.shell")).expect("shell row");
    assert_eq!(shell.panes.as_deref(), Some("two"));
    assert_eq!(shell.panes_at.as_deref(), Some("tablet"));
    assert_eq!(
        shell.shortcuts,
        vec![
            NativeShortcut { chord: "mod+1".into(), target: "#surfy".into() },
            NativeShortcut { chord: "mod+k".into(), target: "openSearch".into() },
        ]
    );
    let terminal = meta.iter().find(|m| m.surface_id.as_deref() == Some("panel.terminal")).expect("row");
    assert_eq!(terminal.native_seat.as_deref(), Some("terminal"));
    assert_eq!(terminal.focus.as_deref(), Some("1"));
    assert!(meta.iter().any(|m| m.pane.as_deref() == Some("start")));
    assert!(meta.iter().any(|m| m.pane.as_deref() == Some("end")));
}

#[cfg(feature = "native")]
#[test]
fn a_native_toolbar_button_carries_its_chord() {
    use surf_parse::render_native::{to_native_blocks, NativeToolbarItem};
    fn find(v: &serde_json::Value) -> Option<serde_json::Value> {
        match v {
            serde_json::Value::Object(m) => {
                if m.get("kind").and_then(|k| k.as_str()) == Some("button")
                    && m.get("label").and_then(|l| l.as_str()) == Some("New")
                {
                    return Some(v.clone());
                }
                m.values().find_map(find)
            }
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    let doc = parse(SHELL).doc;
    let json = serde_json::to_value(to_native_blocks(&doc)).unwrap();
    let item: NativeToolbarItem = serde_json::from_value(find(&json).expect("the button crosses")).unwrap();
    assert_eq!(item.key.as_deref(), Some("mod+shift+n"));
    assert_eq!(item.focus.as_deref(), Some("2"));
    assert_eq!(item.reveal.as_deref(), Some("hover"));
}

#[cfg(feature = "native")]
#[test]
fn a_document_without_the_grammar_adds_no_side_table_rows() {
    let doc = parse("::app-shell[layout=adaptive]\n::: toolbar\n- button[label=\"A\"]\n:::\n::\n").doc;
    assert!(surf_parse::render_native::to_native_block_meta(&doc).is_empty());
}

// ── The serializer ─────────────────────────────────────────────────────────

#[test]
fn the_serializer_keeps_a_buttons_chord_focus_and_reveal() {
    let doc = parse(SHELL).doc;
    let out = doc.to_surf_source();
    assert!(out.contains("key=\"shift+mod+n\"") && out.contains("focus=\"2\"") && out.contains("reveal=\"hover\""), "{out}");
    let again = parse(&out).doc;
    assert_eq!(again.to_surf_source(), out, "the serializer is idempotent");
    let Some(Block::AppShell { children, .. }) = again.blocks.first() else { panic!("shell") };
    let Some(Block::Toolbar { items, .. }) = children.first() else { panic!("toolbar") };
    assert!(matches!(&items[0], ToolbarItem::Button { key: Some(k), .. } if k == "shift+mod+n"));
}

// ── Lint ───────────────────────────────────────────────────────────────────

#[test]
fn the_corpus_tier_is_lint_clean() {
    let report = check(CORPUS);
    let ours: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.as_deref().is_some_and(|c| ("L050"..="L055").contains(&c)))
        .collect();
    assert!(ours.is_empty(), "{ours:?}");
    assert_eq!(report.error_count, 0, "{:?}", report.diagnostics);
}

#[test]
fn the_new_rules_are_errors() {
    let report = check("::app-shell[layout=adaptive shortcuts=\"cmd+k=x\"]\n::\n");
    let d = report.diagnostics.iter().find(|d| d.code.as_deref() == Some("L050")).expect("L050");
    assert_eq!(d.severity, surf_parse::Severity::Error);
    assert!(d.message.contains("write 'mod'"), "{}", d.message);
}

#[test]
fn l051_scopes_duplicates_to_one_shell() {
    let one = "::app-shell[layout=adaptive shortcuts=\"mod+k=a\"]\n::: toolbar\n- button[label=\"B\" key=mod+k]\n:::\n::\n";
    assert_eq!(codes(one, "L051").len(), 1);
    let two = "::app-shell[layout=adaptive shortcuts=\"mod+k=a\"]\n::\n\n::app-shell[layout=adaptive shortcuts=\"mod+k=a\"]\n::\n";
    assert!(codes(two, "L051").is_empty());
}

#[test]
fn l052_needs_a_keyboard_path_for_a_hover_reveal() {
    assert_eq!(codes("::row[icon=doc href=#a reveal=hover]\nA\n::\n", "L052").len(), 1);
    assert!(codes("::row[icon=doc href=#a reveal=hover focus=3]\nA\n::\n", "L052").is_empty());
    assert!(codes("::row[icon=doc href=#a reveal=hover key=mod+a]\nA\n::\n", "L052").is_empty());
    let none = codes("::row[icon=doc href=#a reveal=hover focus=none]\nA\n::\n", "L052");
    assert!(none.len() == 1 && none[0].contains("focus=none"), "{none:?}");
    assert!(codes("::row[icon=doc href=#a reveal=always]\nA\n::\n", "L052").is_empty());
}

#[test]
fn l053_l054_l055_fire_on_their_shapes() {
    assert_eq!(codes("::panel[native=voice]\n::\n", "L053").len(), 1);
    assert!(codes("::panel[native=voice-mode]\n::\n", "L053").is_empty());
    assert_eq!(codes("::modal[name=a surface=sheet.a]\n::\n\n::modal[name=b surface=sheet.a]\n::\n", "L054").len(), 1);
    assert_eq!(codes("::modal[name=a surface=sheet]\n::\n", "L054").len(), 1);
    let half = "::app-shell[layout=adaptive panes=two]\n::: sidebar[pane=start]\n:::\n::\n";
    let l055 = codes(half, "L055");
    assert!(l055.len() == 1 && l055[0].contains("end pane"), "{l055:?}");
    assert!(codes(SHELL, "L055").is_empty());
}

#[test]
fn the_native_list_is_the_closed_d_app_11_list() {
    assert_eq!(
        surf_parse::interaction::NATIVE_SEATS,
        &["voice-mode", "camera", "terminal", "device-pairing", "share-sheet", "platform-sign-in", "store-billing", "menu-bar"]
    );
}

#[test]
fn the_crate_carries_the_interaction_grammar_0_47_0_or_later() {
    // 0.49.0 (public font presets) moved past 0.47.0; the grammar this file tests is in every later release.
    let v: Vec<u32> = surf_parse::spec_registry::CRATE_VERSION.split('.').map(|p| p.parse().unwrap()).collect();
    assert!((v[0], v[1]) >= (0, 47), "{}", surf_parse::spec_registry::CRATE_VERSION);
}

#[test]
fn the_grammar_survives_the_width_projection() {
    let doc = parse(CORPUS).doc;
    for width in [390, 834, 1280] {
        let out = doc.for_width(width).to_html();
        assert!(out.contains("data-shortcuts=\"mod+1=#surfy"), "{width}: {out}");
        assert!(out.contains("data-surface=\"chrome.topbar\""), "{width}");
    }
    let wide = doc.for_width(1280).to_html();
    assert!(wide.contains("data-surface=\"panel.desk\" data-pane=\"end\""), "{wide}");
}

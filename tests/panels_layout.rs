//! The 0.27 `panels` layout (CloudSurf on the web, lane G): grammar, the
//! per-class slot set, the HTML contract the player reads, the serializer
//! fixed point and lint L045.

use surf_parse::{AdaptiveMode, AppShellLayout, Block, PanelSlotRole, PresetSpan};

const SHELL: &str = r#"::app-shell[layout=adaptive desktop=panels tablet=rail mobile=tabs]
::: sidebar[position=left collapsible=false width="0 44 44"]
::::row[icon=doc href=#]
SurfDocs
::::
:::
::: panel-slot[role=navigator kind=surfdocs parks=true]
::::nav-tree[source=docs.recent on-select=open_doc]
::::
:::
::: panel-slot[role=navigator kind=surfy pinned=true]
::::chat-thread[source=surfy]
::::
:::
::: panel-slot[role=work kind=desk]
::::tab-bar
::::
::::tab-content[tab=desk]
::::
:::
::: panel-slot[role=work kind=desk2]
::::tab-bar
::::
::::tab-content[tab=desk2]
::::
:::
::: panel-slot[role=work kind=desk3]
::::tab-bar
::::
::::tab-content[tab=desk3]
::::
:::
::: preset[name=twoColumns title="Two Columns" icon=rectangle.split.2x1 columns="0.5 0.5" rows="1" spans="0,0 1,0" default=true]
:::
::: preset[name=mainPlusTwoStacked title="Main + Two Stacked" columns="0.6667 0.3333" rows="0.5 0.5" spans="0,0,1,2 1,0 1,1" slots="desk desk2 desk3"]
:::
::
"#;

fn shell_children(src: &str) -> Vec<Block> {
    let doc = surf_parse::parse(src).doc;
    match doc.blocks.into_iter().find(|b| matches!(b, Block::AppShell { .. })) {
        Some(Block::AppShell { children, .. }) => children,
        _ => panic!("no app-shell"),
    }
}

#[test]
fn panels_is_a_layout_token_and_an_adaptive_mode() {
    assert_eq!(AppShellLayout::parse("panels"), Some(AppShellLayout::Panels));
    assert_eq!(AppShellLayout::Panels.as_str(), "panels");
    assert!(AppShellLayout::TOKENS.contains(&"panels"));
    assert_eq!(AdaptiveMode::parse("Panels"), Some(AdaptiveMode::Panels));
    assert!(AdaptiveMode::TOKENS.contains(&"panels"));
    let doc = surf_parse::parse(SHELL);
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    match &doc.doc.blocks[0] {
        Block::AppShell { layout, adaptive, .. } => {
            assert_eq!(*layout, AppShellLayout::Adaptive);
            let a = adaptive.expect("adaptive triple");
            assert_eq!(a.desktop, AdaptiveMode::Panels);
            assert_eq!(a.tablet, AdaptiveMode::Rail);
            assert_eq!(a.mobile, AdaptiveMode::Tabs);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn panel_slot_and_preset_parse_typed() {
    let children = shell_children(SHELL);
    let slots: Vec<&Block> = children.iter().filter(|c| matches!(c, Block::PanelSlot { .. })).collect();
    assert_eq!(slots.len(), 5);
    match slots[1] {
        Block::PanelSlot { role, panel_kind: kind, pinned, parks, children, .. } => {
            assert_eq!(*role, PanelSlotRole::Navigator);
            assert_eq!(kind, "surfy");
            assert!(*pinned);
            assert!(!*parks);
            assert!(matches!(children[0], Block::ChatThread { .. }));
        }
        other => panic!("{other:?}"),
    }
    match slots[2] {
        Block::PanelSlot { role, panel_kind: kind, children, .. } => {
            assert_eq!(*role, PanelSlotRole::Work);
            assert_eq!(kind, "desk");
            assert!(matches!(children[0], Block::TabBar { .. }));
            assert!(matches!(children[1], Block::TabContent { .. }));
        }
        other => panic!("{other:?}"),
    }
    let presets: Vec<&Block> = children.iter().filter(|c| matches!(c, Block::Preset { .. })).collect();
    assert_eq!(presets.len(), 2);
    match presets[1] {
        Block::Preset { name, title, columns, rows, spans, slots, default, .. } => {
            assert_eq!(name, "mainPlusTwoStacked");
            assert_eq!(title.as_deref(), Some("Main + Two Stacked"));
            assert_eq!(columns, &[0.6667, 0.3333]);
            assert_eq!(rows, &[0.5, 0.5]);
            assert_eq!(spans[0], PresetSpan { col: 0, row: 0, col_span: 1, row_span: 2 });
            assert_eq!(spans[2], PresetSpan { col: 1, row: 1, col_span: 1, row_span: 1 });
            assert_eq!(slots, &["desk", "desk2", "desk3"]);
            assert!(!*default);
        }
        other => panic!("{other:?}"),
    }
    // A bare preset is a one-cell grid; a bad span token is dropped.
    let bare = surf_parse::parse("::preset[name=x spans=\"0,0 nope 1,0,0,1\"]\n::\n").doc;
    match &bare.blocks[0] {
        Block::Preset { columns, rows, spans, .. } => {
            assert_eq!(columns, &[1.0]);
            assert_eq!(rows, &[1.0]);
            assert_eq!(spans.len(), 1, "only 0,0 survives: {spans:?}");
        }
        other => panic!("{other:?}"),
    }
    // A work slot without kind= is `desk`; role defaults to work.
    let plain = surf_parse::parse("::panel-slot\n:::tab-bar\n:::\n::\n").doc;
    assert!(matches!(&plain.blocks[0], Block::PanelSlot { role: PanelSlotRole::Work, panel_kind, .. } if panel_kind == "desk"));
}

#[test]
fn the_slot_set_resolves_per_size_class() {
    let doc = surf_parse::parse(SHELL).doc;
    let kinds = |width: u32| -> Vec<String> {
        let projected = doc.for_width(width);
        match &projected.blocks[0] {
            Block::AppShell { children, .. } => children
                .iter()
                .filter_map(|c| match c {
                    Block::PanelSlot { panel_kind, .. } => Some(panel_kind.clone()),
                    _ => None,
                })
                .collect(),
            _ => panic!("no shell"),
        }
    };
    assert_eq!(kinds(1280), ["surfdocs", "surfy", "desk", "desk2", "desk3"], "desktop keeps every slot");
    assert_eq!(kinds(834), ["surfdocs", "desk"], "tablet: the first unpinned seat + ONE work slot");
    assert_eq!(kinds(390), ["desk"], "mobile: one work slot; the navigators are the tab bar's targets");
    // The sidebar and the presets survive every class (the tab bar is
    // generated from the sidebar rows; the presets are records).
    for w in [390, 834, 1280] {
        let projected = doc.for_width(w);
        let Block::AppShell { children, .. } = &projected.blocks[0] else { panic!() };
        assert!(children.iter().any(|c| matches!(c, Block::Sidebar { .. })), "{w}");
        assert_eq!(children.iter().filter(|c| matches!(c, Block::Preset { .. })).count(), 2, "{w}");
    }
}

#[test]
fn html_carries_the_grid_the_player_reads() {
    let doc = surf_parse::parse(SHELL).doc;
    let html = doc.to_html();
    assert!(html.contains("surfdoc-app-shell surfdoc-layout-adaptive surfdoc-layout-panels\""), "{html}");
    assert!(html.contains("data-adaptive-desktop=\"panels\""));
    // ONE work grid, the default preset's tracks as custom properties.
    assert_eq!(html.matches("class=\"surfdoc-panels-work\"").count(), 1);
    assert!(html.contains("style=\"--panels-preset:twoColumns;--panels-columns:0.5fr 0.5fr;--panels-rows:1fr\""), "{html}");
    // Seated slots carry their cell; the third work slot is unseated → hidden.
    assert!(html.contains("data-slot=\"desk\" data-role=\"work\" data-kind=\"desk\" data-pinned=\"false\" data-parks=\"false\" style=\"grid-column:1 / span 1;grid-row:1 / span 1\""), "{html}");
    assert!(html.contains("data-slot=\"desk2\"") && html.contains("style=\"grid-column:2 / span 1;grid-row:1 / span 1\""));
    assert!(html.contains("data-slot=\"desk3\" data-role=\"work\" data-kind=\"desk3\" data-pinned=\"false\" data-parks=\"false\" hidden>"), "{html}");
    // Navigator seats sit outside the grid, in authored order, before it.
    let seat = html.find("data-slot=\"surfdocs\"").unwrap();
    let pinned = html.find("data-slot=\"surfy\" data-role=\"navigator\" data-kind=\"surfy\" data-pinned=\"true\"").unwrap();
    let grid = html.find("surfdoc-panels-work").unwrap();
    assert!(seat < pinned && pinned < grid);
    // Presets are inert records.
    assert!(html.contains("<template class=\"surfdoc-preset\" data-preset=\"twoColumns\" data-slot-count=\"2\" data-columns=\"0.5fr 0.5fr\" data-rows=\"1fr\" data-spans=\"0,0 1,0\" data-title=\"Two Columns\" data-icon=\"rectangle.split.2x1\" data-default=\"true\"></template>"), "{html}");
    assert!(html.contains("data-spans=\"0,0,1,2 1,0 1,1\" data-title=\"Main + Two Stacked\" data-slots=\"desk desk2 desk3\"></template>"), "{html}");
    // The mobile projection still generates the tab bar from the rail rows.
    let mobile = doc.for_width(390).to_html();
    assert!(mobile.contains("surfdoc-app-tabbar"));
    assert_eq!(mobile.matches("surfdoc-panel-slot-work").count(), 1);
    // A shell with no preset: the work slots still form a grid, one column.
    let bare = surf_parse::parse("::app-shell[layout=panels]\n:::panel-slot[role=work kind=desk]\n::::tab-bar\n::::\n:::\n::\n").doc.to_html();
    assert!(bare.contains("surfdoc-layout-panels"));
    assert!(bare.contains("style=\"--panels-preset:none;--panels-columns:1fr;--panels-rows:1fr\""), "{bare}");
    assert!(bare.contains("data-slot=\"desk\"") && !bare.contains("hidden>"), "{bare}");
}

#[test]
fn serializer_round_trip_is_a_fixed_point() {
    let doc = surf_parse::parse(SHELL).doc;
    let out = surf_parse::builder::to_surf_source(&doc);
    let again = surf_parse::parse(&out).doc;
    assert_eq!(surf_parse::builder::to_surf_source(&again), out);
    assert_eq!(again.to_html(), doc.to_html(), "the re-parsed tree renders identically");
    assert!(out.contains("panel-slot[role=navigator kind=surfy pinned=true]"), "{out}");
    assert!(out.contains("preset[name=mainPlusTwoStacked title=\"Main + Two Stacked\" columns=\"0.6667 0.3333\" rows=\"0.5 0.5\" spans=\"0,0,1,2 1,0 1,1\" slots=\"desk desk2 desk3\"]"), "{out}");
}

#[test]
fn lint_l045_names_the_holes() {
    let clean = surf_parse::lint::check(SHELL);
    assert!(clean.diagnostics.iter().all(|d| d.code.as_deref() != Some("L045")), "{:?}", clean.diagnostics);
    let holes = "::app-shell[layout=panels]\n:::panel-slot[role=navigator]\n:::\n:::panel-slot[role=work kind=desk]\nno strip\n:::\n:::preset[name=x slots=\"desk ghost\"]\n:::\n::\n";
    let report = surf_parse::lint::check(holes);
    let l045: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.as_deref() == Some("L045"))
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(l045.len(), 3, "{l045:?}");
    assert!(l045.iter().any(|m| m.contains("names no kind=")));
    assert!(l045.iter().any(|m| m.contains("kind=desk]' has no '::tab-bar'")));
    assert!(l045.iter().any(|m| m.contains("seats slot 'ghost'")));
    // L041 accepts the new token and still refuses a stranger.
    let stranger = surf_parse::lint::check("::app-shell[layout=grid desktop=panels]\n::\n");
    let l041: Vec<&str> = stranger.diagnostics.iter().filter(|d| d.code.as_deref() == Some("L041")).map(|d| d.message.as_str()).collect();
    assert_eq!(l041.len(), 1, "{l041:?}");
    assert!(l041[0].contains("'grid'"));
}

#[cfg(feature = "native")]
#[test]
fn native_crosses_as_panel_slot_and_preset() {
    use surf_parse::render_native::{to_native_blocks, NativeBlock, NATIVE_DOC_SCHEMA_VERSION};
    assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 12);
    let doc = surf_parse::parse(SHELL).doc;
    let native = to_native_blocks(&doc);
    let NativeBlock::AppShell { layout, adaptive, children } = &native[0] else { panic!("{native:?}") };
    assert_eq!(layout, "adaptive");
    assert_eq!(adaptive.as_ref().unwrap().desktop, "panels");
    let slots = children.iter().filter(|c| matches!(c, NativeBlock::PanelSlot { .. })).count();
    assert_eq!(slots, 5);
    let preset = children.iter().find(|c| matches!(c, NativeBlock::Preset { is_default: true, .. })).expect("default preset");
    let NativeBlock::Preset { name, columns, spans, .. } = preset else { unreachable!() };
    assert_eq!(name, "twoColumns");
    assert_eq!(columns, &[0.5, 0.5]);
    assert_eq!(spans.len(), 2);
    let json = serde_json::to_string(&native).unwrap();
    assert!(json.contains("\"type\":\"panel_slot\"") && json.contains("\"type\":\"preset\""));
}

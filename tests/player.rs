//! `::player` (0.42.0): the grammar, the HTML (the list, the locked row,
//! the row layout, the runtime), the DOM twin's byte identity, the native
//! shape, every degrading renderer, the builder's fixed point, and hostile
//! input.

use surf_parse::{parse, Block, PlayerLayout};

const ALBUM: &str = "::player[title=\"Time Traveler\" artist=\"Scouter\" art=\"/img/cover.png\" buy=\"#buy\" buy-label=\"Full album\"]\n- Broken Hearted | 1:55 | | Chapter 1 · Ordinary World\n- Time Traveler | 2:54 | /audio/time-traveler.mp3 | Chapter 4\n- Without Rain | 3:16 | https://cdn.example.com/without-rain.mp3\n::";

const ROW: &str = "::player[layout=row title=\"Time Traveler\" artist=\"Scouter\" line=\"The title track streams free.\" buy=\"/music/tt#buy\" buy-label=\"Buy the album\"]\n- Time Traveler | 2:54 | /audio/time-traveler.mp3\n::";

fn html(src: &str) -> String {
    parse(src).doc.to_html_fragment()
}

fn only_player(src: &str) -> Block {
    parse(src)
        .doc
        .blocks
        .into_iter()
        .find(|b| matches!(b, Block::Player { .. }))
        .expect("a ::player block")
}

// ── Grammar ────────────────────────────────────────────────────────────────

#[test]
fn every_attribute_and_field_parses() {
    match only_player(ALBUM) {
        Block::Player { title, artist, art, buy, buy_label, line, layout, tracks, .. } => {
            assert_eq!(title.as_deref(), Some("Time Traveler"));
            assert_eq!(artist.as_deref(), Some("Scouter"));
            assert_eq!(art.as_deref(), Some("/img/cover.png"));
            assert_eq!(buy.as_deref(), Some("#buy"));
            assert_eq!(buy_label.as_deref(), Some("Full album"));
            assert_eq!(line, None);
            assert_eq!(layout, PlayerLayout::List);
            assert_eq!(tracks.len(), 3);
            assert_eq!(tracks[0].title, "Broken Hearted");
            assert_eq!(tracks[0].duration.as_deref(), Some("1:55"));
            assert!(tracks[0].locked(), "an empty source is a locked track");
            assert_eq!(tracks[0].line.as_deref(), Some("Chapter 1 · Ordinary World"));
            assert_eq!(tracks[0].duration_seconds(), Some(115));
            assert_eq!(tracks[1].src.as_deref(), Some("/audio/time-traveler.mp3"));
            assert!(!tracks[1].locked());
            assert_eq!(tracks[1].duration_seconds(), Some(174));
            assert_eq!(tracks[2].line, None);
        }
        other => panic!("expected Player, got {other:?}"),
    }
}

#[test]
fn the_word_locked_and_numbered_lines_and_the_row_layout() {
    match only_player("::player[layout=row]\n1. One | 1:00 | locked\n2. Two | 0:30 | /a.mp3\n* Three\n::") {
        Block::Player { layout, tracks, .. } => {
            assert_eq!(layout, PlayerLayout::Row);
            assert_eq!(tracks.len(), 3);
            assert!(tracks[0].locked() && tracks[0].src.is_none(), "`locked` is not a source");
            assert_eq!(tracks[1].src.as_deref(), Some("/a.mp3"));
            assert_eq!(tracks[2].title, "Three");
            assert_eq!(tracks[2].duration_seconds(), None);
        }
        other => panic!("expected Player, got {other:?}"),
    }
}

#[test]
fn a_bad_length_is_none() {
    match only_player("::player\n- One | long | /a.mp3\n- Two | 1:2:3:4 | /b.mp3\n::") {
        Block::Player { tracks, .. } => {
            assert_eq!(tracks[0].duration.as_deref(), Some("long"));
            assert_eq!(tracks[0].duration_seconds(), None);
            assert_eq!(tracks[1].duration_seconds(), None);
        }
        other => panic!("expected Player, got {other:?}"),
    }
}

// ── HTML ───────────────────────────────────────────────────────────────────

#[test]
fn the_list_draws_a_streaming_row_a_locked_row_and_the_runtime_once_per_block() {
    let h = html(ALBUM);
    assert!(h.contains("<section class=\"surfdoc-player\" data-player=\"\" data-layout=\"list\" data-album=\"Time Traveler\" data-artist=\"Scouter\" data-art=\"/img/cover.png\" data-buy=\"#buy\">"), "{h}");
    // The locked row links to the buy door and says so.
    assert!(h.contains("<a class=\"surfdoc-player-track surfdoc-player-locked\" href=\"#buy\" data-locked=\"\" aria-label=\"Broken Hearted, on the full album\"><span class=\"surfdoc-player-num\"><span class=\"surfdoc-player-index\">1</span></span><span class=\"surfdoc-player-main\"><span class=\"surfdoc-player-title\">Broken Hearted</span><span class=\"surfdoc-player-line\">Chapter 1 · Ordinary World</span></span><span class=\"surfdoc-player-buy\"><span class=\"surfdoc-player-lock\" aria-hidden=\"true\"></span><span>Full album</span></span><span class=\"surfdoc-player-time\">1:55</span></a>"), "{h}");
    // The streaming row is a button carrying its source, with the equaliser in the number's cell.
    assert!(h.contains("<button type=\"button\" class=\"surfdoc-player-track\" data-src=\"/audio/time-traveler.mp3\" data-title=\"Time Traveler\" data-duration=\"2:54\" aria-label=\"Play Time Traveler\"><span class=\"surfdoc-player-num\"><span class=\"surfdoc-player-index\">2</span><span class=\"surfdoc-player-eq\" aria-hidden=\"true\"><span></span><span></span><span></span></span></span>"), "{h}");
    assert!(h.contains("data-src=\"https://cdn.example.com/without-rain.mp3\""));
    assert_eq!(h.matches("<script>").count(), 1, "one runtime per block");
    assert!(h.contains("window.__surfdocPlayer"), "the runtime guards itself");
    assert!(h.trim_end().ends_with("</script>"));
}

#[test]
fn the_row_layout_is_the_now_playing_strip() {
    let h = html(ROW);
    assert!(h.contains("<section class=\"surfdoc-player surfdoc-player-row\" data-player=\"\" data-layout=\"row\" data-album=\"Time Traveler\" data-artist=\"Scouter\" data-buy=\"/music/tt#buy\">"), "{h}");
    assert!(h.contains("<span class=\"surfdoc-player-art\" data-art-missing=\"\"></span>"), "no art: the mark's chip");
    assert!(h.contains("<button type=\"button\" class=\"surfdoc-player-track surfdoc-player-now\" data-src=\"/audio/time-traveler.mp3\" data-title=\"Time Traveler\" data-duration=\"2:54\" aria-label=\"Play Time Traveler\"><span class=\"surfdoc-player-glyph\" aria-hidden=\"true\"></span></button>"), "{h}");
    assert!(h.contains("<span class=\"surfdoc-player-title\">Time Traveler · Scouter</span><span class=\"surfdoc-player-line\">The title track streams free.</span>"));
    assert!(h.contains("<span class=\"surfdoc-player-time\">2:54</span><a class=\"surfdoc-player-buy-btn\" href=\"/music/tt#buy\">Buy the album</a></section>"), "{h}");
}

#[test]
fn a_row_with_nothing_to_stream_is_disabled_and_a_locked_row_without_a_door_is_a_span() {
    let h = html("::player[layout=row title=\"Demos\"]\n- One | 1:00\n::");
    assert!(h.contains("surfdoc-player-now\" aria-disabled=\"true\" data-title=\"One\""), "{h}");
    assert!(!h.contains("surfdoc-player-buy-btn"));
    let h = html("::player\n- One | 1:00\n::");
    assert!(h.contains("<span class=\"surfdoc-player-track surfdoc-player-locked\" data-locked=\"\" aria-label=\"One, on the full album\">"), "{h}");
    assert!(h.contains("</span><span class=\"surfdoc-player-time\">1:00</span></span></li>"), "{h}");
}

#[test]
fn hostile_sources_and_text_never_reach_the_markup() {
    let h = html("::player[title=\"<b>x</b>\" art=\"javascript:alert(1)\" buy=\"javascript:alert(2)\"]\n- \"Quote\" & <i>tag</i> | 1:00 | javascript:alert(3)\n- Two | 1:00 | data:audio/mp3;base64,AAAA\n- Three | 1:00 | //evil.example/a.mp3\n::");
    assert!(!h.contains("javascript:"), "{h}");
    assert!(!h.contains("data:audio"), "{h}");
    assert!(!h.contains("//evil.example"), "{h}");
    assert!(!h.contains("<b>x</b>") && !h.contains("<i>tag</i>"), "{h}");
    assert!(h.contains("data-album=\"&lt;b&gt;x&lt;/b&gt;\""));
    assert!(h.contains("href=\"#\""), "a refused buy door is `#`");
    // Every row is locked: no source survived.
    assert_eq!(h.matches("data-locked=\"\"").count(), 3, "{h}");
    assert!(!h.contains("data-src="));
}

#[test]
fn a_media_source_needs_the_hosts_resolver() {
    let src = "::player\n- One | 1:00 | media:3f6c0a1e\n::";
    assert!(html(src).contains("data-locked=\"\""), "unresolved: the row is locked");
    let _media = surf_parse::install_media_resolver(surf_parse::media_templates("/media/{id}", "/media/{id}?r=loop", "/media/{id}/poster"));
    assert!(html(src).contains("data-src=\"/media/3f6c0a1e\""));
}

#[test]
fn the_stylesheet_carries_the_player() {
    for class in [
        ".surfdoc-player ", ".surfdoc-player-track", ".surfdoc-player-locked", ".surfdoc-player-eq", ".surfdoc-player-row",
        ".surfdoc-player-now", ".surfdoc-player-buy-btn", ".surfdoc-player-bar", ".surfdoc-player-sheet", ".surfdoc-player-scrubber",
        ".surfdoc-player-backdrop", ".surfdoc-player-glyph", ".surfdoc-player-lock",
    ] {
        assert!(surf_parse::SURFDOC_CSS.contains(class), "no rule for {class}");
    }
}

// ── The DOM twin ───────────────────────────────────────────────────────────

#[cfg(feature = "dom")]
#[test]
fn the_dom_twin_is_byte_identical_and_declines_as_script_emitting() {
    use surf_parse::render_dom::{check_coverage, render_fragment_string, RenderDomError};
    for src in [ALBUM, ROW, "::player\n- One | 1:00\n::", "::player[layout=row title=\"Demos\"]\n- One | 1:00\n::"] {
        let doc = parse(src).doc;
        assert_eq!(render_fragment_string(&doc).expect("renders"), doc.to_html_fragment(), "{src}");
        match check_coverage(&doc) {
            Err(RenderDomError::Unimplemented(k)) => assert_eq!(k, "script-emitting:player"),
            other => panic!("expected the script-emitting decline, got {other:?}"),
        }
    }
}

// ── Native ─────────────────────────────────────────────────────────────────

#[cfg(feature = "native")]
#[test]
fn the_native_block_carries_the_tracks() {
    use surf_parse::render_native::{to_native_blocks, NativeBlock, NATIVE_DOC_SCHEMA_VERSION};
    const _: () = assert!(NATIVE_DOC_SCHEMA_VERSION >= 17);
    let doc = parse(ALBUM).doc;
    let blocks = to_native_blocks(&doc);
    let player = blocks.iter().find(|b| matches!(b, NativeBlock::Player { .. })).expect("a native player");
    match player {
        NativeBlock::Player { title, artist, layout, tracks, buy, .. } => {
            assert_eq!(title.as_deref(), Some("Time Traveler"));
            assert_eq!(artist.as_deref(), Some("Scouter"));
            assert_eq!(layout, "list");
            assert_eq!(buy.as_deref(), Some("#buy"));
            assert_eq!(tracks.len(), 3);
            assert!(tracks[0].locked && tracks[0].src.is_none());
            assert_eq!(tracks[0].duration_seconds, Some(115));
            assert_eq!(tracks[1].src.as_deref(), Some("/audio/time-traveler.mp3"));
            assert!(!tracks[1].locked);
        }
        _ => unreachable!(),
    }
}

// ── Degrading renderers ────────────────────────────────────────────────────

#[test]
fn markdown_lists_the_tracks_and_links_what_streams() {
    let md = parse(ALBUM).doc.to_markdown();
    assert!(md.contains("**Time Traveler** · Scouter"), "{md}");
    assert!(md.contains("1. Broken Hearted · 1:55 · [Full album](#buy)"), "{md}");
    assert!(md.contains("2. [Time Traveler](/audio/time-traveler.mp3) · 2:54"), "{md}");
}

#[test]
fn latex_and_typst_degrade_to_a_numbered_list() {
    let doc = parse(ALBUM).doc;
    let tex = doc.to_latex();
    assert!(tex.contains("\\begin{enumerate}") && tex.contains("\\item Broken Hearted (1:55)"), "{tex}");
    assert!(tex.contains("\\url{https://cdn.example.com/without-rain.mp3}"), "{tex}");
    let typ = doc.to_typst();
    assert!(typ.contains("1. Broken Hearted (1:55)") && typ.contains("2. Time Traveler (2:54)"), "{typ}");
}

// ── The builder's fixed point ──────────────────────────────────────────────

#[test]
fn the_serializer_round_trips() {
    let doc = parse(ALBUM).doc;
    let out = doc.to_surf_source();
    assert!(out.contains("::player[title=\"Time Traveler\" artist=\"Scouter\" art=\"/img/cover.png\" buy=\"#buy\" buy-label=\"Full album\"]"), "{out}");
    assert!(out.contains("- Broken Hearted | 1:55 |  | Chapter 1 · Ordinary World"), "{out}");
    assert!(out.contains("- Time Traveler | 2:54 | /audio/time-traveler.mp3 | Chapter 4"), "{out}");
    assert!(out.contains("- Without Rain | 3:16 | https://cdn.example.com/without-rain.mp3\n"), "{out}");
    let again = parse(&out).doc;
    assert_eq!(again.to_html_fragment(), doc.to_html_fragment(), "fixed point");
    let row = parse(ROW).doc;
    let out = row.to_surf_source();
    assert!(out.contains("layout=row"), "{out}");
    assert_eq!(parse(&out).doc.to_html_fragment(), row.to_html_fragment());
}

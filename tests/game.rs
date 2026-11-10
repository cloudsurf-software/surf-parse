//! `::game` (0.46.0): a made game's spec — the grammar (Fox Run, a missing
//! engine, an unknown engine, stray lines), the inert card on HTML, the DOM
//! twin's byte identity, the native card (schema unchanged), every degrading
//! renderer, the serializer's fixed point, the lint and the registry row.

use surf_parse::{parse, Block, GameField, GameValue};

/// The spec 1M's Fox Run fixture carries (surf `apps/backends/spec.rs`).
const FOX: &str = "# Fox Run\n\n::game[engine=runner version=1]\ntitle: Fox Run\ncharacter: fox\ntrack: desert\nspeed: 1.2\nlanes: 3\nobstacles:\n  - cone (low, jump)\n  - bar (high, slide)\n  - cactus (tall, lane)\n::\n";

const FOX_BODY: &str = "title: Fox Run\ncharacter: fox\ntrack: desert\nspeed: 1.2\nlanes: 3\nobstacles:\n  - cone (low, jump)\n  - bar (high, slide)\n  - cactus (tall, lane)";

const FOX_CARD: &str = "<section class=\"surfdoc-game\" data-engine=\"runner\" data-version=\"1\"><header class=\"surfdoc-game-head\"><h3 class=\"surfdoc-game-title\">Fox Run</h3><span class=\"surfdoc-game-engine\">Runner</span></header><dl class=\"surfdoc-game-keys\"><dt>character</dt><dd>fox</dd><dt>track</dt><dd>desert</dd><dt>speed</dt><dd>1.2</dd><dt>lanes</dt><dd>3</dd></dl><div class=\"surfdoc-game-list\"><span class=\"surfdoc-game-list-key\">obstacles</span><ul class=\"surfdoc-game-items\"><li>cone (low, jump)</li><li>bar (high, slide)</li><li>cactus (tall, lane)</li></ul></div></section>";

const NO_ENGINE: &str = "::game\ntitle: Owl Hop\nspeed: 2\n::";
const UNKNOWN_ENGINE: &str = "::game[engine=platformer version=3]\ntitle: Owl Hop\n::";
const WILDS: &str = "::game[engine=wilds version=1]\ntitle: Tide Pools\nspecies:\n  - crab (scuttles, shy)\n::";
const STRAY: &str = "::game[engine=runner]\ntitle: Owl Hop\nthis line is not a key\n- an item with no list\nspeed: 2\n::";
const HOSTILE: &str = "::game[engine=\"<script>alert(1)</script>\" version=1]\ntitle: <script>alert(2)</script>\nnote: \" onmouseover=alert(3) style=\"color:red\nitems:\n  - <img src=x onerror=alert(4)>\n::";

fn html(src: &str) -> String {
    parse(src).doc.to_html_fragment()
}

fn only_game(src: &str) -> Block {
    parse(src)
        .doc
        .blocks
        .into_iter()
        .find(|b| matches!(b, Block::Game { .. }))
        .expect("a ::game block")
}

fn scalar(key: &str, value: &str) -> GameField {
    GameField { key: key.to_string(), value: GameValue::Scalar(value.to_string()) }
}

/// True when `markup` holds an `on…=` attribute or an inline style: the card
/// must carry neither.
fn has_handler_or_style(markup: &str) -> bool {
    let bytes = markup.as_bytes();
    let mut in_tag = false;
    let mut quote: Option<u8> = None;
    for i in 0..bytes.len() {
        let c = bytes[i];
        match (in_tag, quote) {
            (false, _) if c == b'<' => in_tag = true,
            (true, None) if c == b'>' => in_tag = false,
            (true, None) if c == b'"' || c == b'\'' => quote = Some(c),
            (true, Some(q)) if c == q => quote = None,
            (true, None) if c == b' ' => {
                let rest = &markup[i + 1..];
                let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
                if name == "style" || (name.len() > 2 && name.starts_with("on")) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

// ── Grammar ────────────────────────────────────────────────────────────────

#[test]
fn fox_run_parses_to_one_game_with_its_keys_and_its_verbatim_body() {
    let blocks: Vec<Block> = parse(FOX).doc.blocks;
    let games: Vec<&Block> = blocks.iter().filter(|b| matches!(b, Block::Game { .. })).collect();
    assert_eq!(games.len(), 1, "{blocks:?}");
    match games[0] {
        Block::Game { engine, version, fields, body, .. } => {
            assert_eq!(engine, "runner");
            assert_eq!(*version, Some(1));
            assert_eq!(body, FOX_BODY, "the body is kept verbatim");
            assert_eq!(
                fields[..5],
                [
                    scalar("title", "Fox Run"),
                    scalar("character", "fox"),
                    scalar("track", "desert"),
                    scalar("speed", "1.2"),
                    scalar("lanes", "3"),
                ]
            );
            assert_eq!(fields.len(), 6);
            assert_eq!(fields[5].key, "obstacles");
            assert_eq!(
                fields[5].value,
                GameValue::List(vec![
                    "cone (low, jump)".to_string(),
                    "bar (high, slide)".to_string(),
                    "cactus (tall, lane)".to_string(),
                ])
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn a_game_without_an_engine_parses_with_an_empty_engine_and_a_lint_warning() {
    match only_game(NO_ENGINE) {
        Block::Game { engine, version, fields, .. } => {
            assert_eq!(engine, "");
            assert_eq!(version, None);
            assert_eq!(fields, vec![scalar("title", "Owl Hop"), scalar("speed", "2")]);
        }
        _ => unreachable!(),
    }
    let parsed = parse(NO_ENGINE);
    assert!(
        parsed.diagnostics.iter().all(|d| d.severity != surf_parse::Severity::Error),
        "a missing engine is never an error: {:?}",
        parsed.diagnostics
    );
    let report = surf_parse::check(NO_ENGINE);
    let l049: Vec<_> = report.diagnostics.iter().filter(|d| d.code.as_deref() == Some("L049")).collect();
    assert_eq!(l049.len(), 1, "{:?}", report.diagnostics);
    assert_eq!(l049[0].severity, surf_parse::Severity::Warning);
    assert!(l049[0].message.contains("names no engine="), "{}", l049[0].message);
}

#[test]
fn an_unknown_engine_parses_and_is_never_judged_here() {
    match only_game(UNKNOWN_ENGINE) {
        Block::Game { engine, version, .. } => {
            assert_eq!(engine, "platformer");
            assert_eq!(version, Some(3));
        }
        _ => unreachable!(),
    }
    let report = surf_parse::check(UNKNOWN_ENGINE);
    assert!(
        report.diagnostics.iter().all(|d| d.code.as_deref() != Some("L049")),
        "an unknown engine is the platform's to refuse: {:?}",
        report.diagnostics
    );
    let out = html(UNKNOWN_ENGINE);
    assert!(out.contains("data-engine=\"platformer\" data-version=\"3\""), "{out}");
    assert!(out.contains("<span class=\"surfdoc-game-engine\">platformer</span>"), "{out}");
}

#[test]
fn stray_lines_stay_in_the_body_leave_the_card_and_are_named_by_the_lint() {
    match only_game(STRAY) {
        Block::Game { fields, body, .. } => {
            assert_eq!(fields, vec![scalar("title", "Owl Hop"), scalar("speed", "2")]);
            assert!(body.contains("this line is not a key") && body.contains("- an item with no list"));
        }
        _ => unreachable!(),
    }
    let out = html(STRAY);
    assert!(!out.contains("this line is not a key") && !out.contains("an item with no list"), "{out}");
    let report = surf_parse::check(STRAY);
    let lines: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.as_deref() == Some("L049"))
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("line 3"), "{lines:?}");
    assert!(lines[1].contains("line 4"), "{lines:?}");
    assert!(surf_parse::check(FOX).diagnostics.iter().all(|d| d.code.as_deref() != Some("L049")));
}

#[test]
fn an_empty_list_reads_as_an_empty_value_and_a_bad_version_as_none() {
    match only_game("::game[engine=runner version=1.5]\ntitle: A\nobstacles:\n::") {
        Block::Game { version, fields, .. } => {
            assert_eq!(version, None);
            assert_eq!(fields, vec![scalar("title", "A"), scalar("obstacles", "")]);
        }
        _ => unreachable!(),
    }
}

// ── HTML ───────────────────────────────────────────────────────────────────

#[test]
fn fox_run_draws_as_one_inert_card() {
    let out = html(FOX);
    assert!(out.contains(FOX_CARD), "{out}");
    assert_eq!(out.matches("<section class=\"surfdoc-game\"").count(), 1);
    assert!(!out.contains("<script"), "{out}");
    assert!(!has_handler_or_style(FOX_CARD));
    assert!(!has_handler_or_style(&out), "{out}");
}

#[test]
fn the_heading_falls_back_to_game_and_the_engine_words_are_the_readers() {
    let out = html(NO_ENGINE);
    assert!(out.contains("<section class=\"surfdoc-game\"><header"), "no data-engine without an engine: {out}");
    assert!(!out.contains("surfdoc-game-engine"), "{out}");
    let out = html("::game[engine=runner]\nspeed: 2\n::");
    assert!(out.contains("<h3 class=\"surfdoc-game-title\">Game</h3>"), "{out}");
    assert!(out.contains("data-engine=\"runner\">"), "{out}");
    let out = html(WILDS);
    assert!(out.contains("<span class=\"surfdoc-game-engine\">Surflings world</span>"), "{out}");
    assert!(out.contains("<span class=\"surfdoc-game-list-key\">species</span>"), "{out}");
}

#[test]
fn hostile_values_are_escaped_text_and_never_markup() {
    let out = html(HOSTILE);
    assert!(!out.contains("<script"), "{out}");
    assert!(!out.contains("<img"), "{out}");
    assert!(out.contains("&lt;script&gt;alert(2)&lt;/script&gt;"), "{out}");
    assert!(out.contains("data-engine=\"&lt;script&gt;alert(1)&lt;/script&gt;\""), "{out}");
    assert!(out.contains("&lt;img src=x onerror=alert(4)&gt;"), "{out}");
    assert!(!has_handler_or_style(&out), "{out}");
}

#[test]
fn the_stylesheet_carries_the_card() {
    let css = surf_parse::SURFDOC_CSS;
    for class in [
        ".surfdoc-game ",
        ".surfdoc-game-head",
        ".surfdoc-game-title",
        ".surfdoc-game-engine",
        ".surfdoc-game-keys",
        ".surfdoc-game-list ",
        ".surfdoc-game-list-key",
        ".surfdoc-game-items",
    ] {
        assert!(css.contains(class), "no rule for {class}");
    }
}

// ── The DOM twin ───────────────────────────────────────────────────────────

#[cfg(feature = "dom")]
#[test]
fn the_dom_twin_is_byte_identical_and_covered() {
    use surf_parse::render_dom::{check_coverage, render_fragment_string};
    let corpus = include_str!("corpus/tier10-game.surf");
    for src in [FOX, NO_ENGINE, UNKNOWN_ENGINE, WILDS, STRAY, HOSTILE, corpus, "::game[id=spec engine=runner]\ntitle: A\n::"] {
        let doc = parse(src).doc;
        check_coverage(&doc).unwrap_or_else(|e| panic!("expected coverage, got {e}\n{src}"));
        assert_eq!(render_fragment_string(&doc).expect("renders"), doc.to_html_fragment(), "{src}");
    }
}

// ── Native ─────────────────────────────────────────────────────────────────

#[cfg(feature = "native")]
#[test]
fn native_carries_an_open_details_card_and_the_schema_stays() {
    use surf_parse::render_native::{block_tier, to_native_blocks, BlockTier, NativeBlock, NATIVE_DOC_SCHEMA_VERSION};
    // 0.46.0 added no native variant (it kept v18); 0.47.0's v19 adds
    // record fields only, so the game card still crosses as `Details`.
    assert!(NATIVE_DOC_SCHEMA_VERSION >= 18, "0.46.0 adds no native variant");
    let doc = parse(FOX).doc;
    let game = doc.blocks.iter().find(|b| matches!(b, Block::Game { .. })).unwrap();
    assert_eq!(block_tier(game), BlockTier::Content);
    let blocks = to_native_blocks(&doc);
    let card = blocks.iter().find(|b| matches!(b, NativeBlock::Details { .. })).expect("a Details card");
    assert_eq!(
        card,
        &NativeBlock::Details {
            title: Some("Fox Run".to_string()),
            open: true,
            content: "Runner · version 1\n\n- character: fox\n- track: desert\n- speed: 1.2\n- lanes: 3\n- obstacles:\n  - cone (low, jump)\n  - bar (high, slide)\n  - cactus (tall, lane)".to_string(),
        }
    );
    match &to_native_blocks(&parse(NO_ENGINE).doc)[0] {
        NativeBlock::Details { title, content, .. } => {
            assert_eq!(title.as_deref(), Some("Owl Hop"));
            assert_eq!(content, "- speed: 2");
        }
        other => panic!("expected Details, got {other:?}"),
    }
}

// ── Degrading renderers ────────────────────────────────────────────────────

#[test]
fn markdown_degrades_to_the_title_the_engine_and_the_key_lines() {
    let md = parse(FOX).doc.to_markdown();
    assert!(
        md.contains("**Fox Run** · Runner · version 1\n\n- character: fox\n- track: desert\n- speed: 1.2\n- lanes: 3\n- obstacles:\n  - cone (low, jump)\n  - bar (high, slide)\n  - cactus (tall, lane)"),
        "{md}"
    );
    assert!(!md.contains("::game"), "{md}");
    let md = parse(NO_ENGINE).doc.to_markdown();
    assert!(md.contains("**Owl Hop**\n\n- speed: 2"), "{md}");
}

#[cfg(feature = "terminal")]
#[test]
fn the_terminal_reads_the_title_the_engine_and_the_keys() {
    let out = parse(FOX).doc.to_terminal();
    assert!(out.contains("Fox Run") && out.contains("Runner"), "{out}");
    assert!(out.contains("  - speed: 1.2"), "{out}");
    assert!(out.contains("  - obstacles:\n    - cone (low, jump)"), "{out}");
}

#[test]
fn latex_and_typst_degrade_to_the_title_and_a_key_list() {
    let doc = parse(FOX).doc;
    let tex = doc.to_latex();
    assert!(tex.contains("\\textbf{Fox Run}"), "{tex}");
    assert!(tex.contains("\\item speed: 1.2"), "{tex}");
    assert!(tex.contains("\\item obstacles:\n\\begin{itemize}\n\\item cone (low, jump)"), "{tex}");
    let typ = doc.to_typst();
    assert!(typ.contains("*Fox Run*"), "{typ}");
    assert!(typ.contains("- speed: 1.2\n"), "{typ}");
    assert!(typ.contains("- obstacles:\n  - cone (low, jump)\n"), "{typ}");
}

#[test]
fn an_email_degrades_the_game_through_its_markdown() {
    let doc = parse(FOX).doc;
    let game = doc.blocks.iter().find(|b| matches!(b, Block::Game { .. })).unwrap();
    assert_eq!(
        surf_parse::email_disposition(game, &surf_parse::EmailProfile::default()),
        surf_parse::EmailDisposition::Degrade
    );
}

// ── The serializer, the corpus and the registry ────────────────────────────

#[test]
fn the_serializer_writes_the_body_back_verbatim_to_a_fixed_point() {
    let doc = parse(FOX).doc;
    let out = doc.to_surf_source();
    assert!(out.contains(&format!("::game[engine=\"runner\" version=1]\n{FOX_BODY}\n::")), "{out}");
    let again = parse(&out).doc;
    assert_eq!(again.to_html_fragment(), doc.to_html_fragment(), "fixed point");
    assert_eq!(again.to_surf_source(), out, "the serializer is idempotent");
    let bare = parse(NO_ENGINE).doc.to_surf_source();
    assert!(bare.contains("::game\ntitle: Owl Hop\nspeed: 2\n::"), "{bare}");
}

#[test]
fn the_corpus_tier_round_trips() {
    let src = include_str!("corpus/tier10-game.surf");
    let doc = parse(src).doc;
    assert_eq!(doc.blocks.iter().filter(|b| matches!(b, Block::Game { .. })).count(), 3);
    let again = parse(&doc.to_surf_source()).doc;
    assert_eq!(again.to_html_fragment(), doc.to_html_fragment());
}

#[test]
fn the_registry_lists_the_block_from_0_46_0() {
    // 0.46.0 shipped the block; 0.47.0 (the interaction grammar) moved the
    // crate on, so the pin is a floor.
    let v: Vec<u32> = surf_parse::spec_registry::CRATE_VERSION.split('.').map(|p| p.parse().unwrap()).collect();
    assert!((v[0], v[1]) >= (0, 46), "{v:?}");
    let row = surf_parse::spec_registry::registry()
        .into_iter()
        .find(|r| r.name == "game")
        .expect("[blocks.game]");
    assert_eq!(row.status, "implemented");
    assert_eq!(row.category, "advanced");
    assert_eq!(row.enum_variant.as_deref(), Some("Game"));
    assert_eq!(row.attributes, vec!["engine".to_string(), "version".to_string()]);
    assert!(surf_parse::spec_registry::SNIPPETS.iter().any(|(k, _)| *k == "game"));
    assert!(surf_parse::lint::known_block_names().contains("game"));
}

#[test]
fn no_exported_word_carries_an_emoji() {
    for word in [
        surf_parse::game_engine_word("runner"),
        surf_parse::game_engine_word("wilds"),
        Some(surf_parse::game_title(&[])),
    ]
    .into_iter()
    .flatten()
    {
        assert!(word.chars().all(|c| (c as u32) < 0x2000), "{word}");
    }
}

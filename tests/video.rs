//! `::video`, `::hero[video=]` and the video-file `::embed` (0.37.0).
//!
//! One suite for the whole element: the grammar and its defaults, the flag
//! rules, the HTML structure and its reduced-motion mechanism, the hero
//! background, the embed split (a file is a player, a provider page is a
//! card), the unresolved-`media:` placeholder, the host's resolver seam,
//! `media_refs`, every degrading renderer, the lint rules, the model field,
//! the builder and the serializer's fixed point, and hostile input.

use surf_parse::media::{self, install_media_resolver, media_templates, MediaUse};
use surf_parse::{parse, Block, MediaRef, ModelFieldType, Severity, SurfDocBuilder};

fn html(src: &str) -> String {
    parse(src).doc.to_html_fragment()
}

/// The resolver a site installs: the id on the site's own origin.
fn site_media() -> surf_parse::MediaScope {
    install_media_resolver(media_templates(
        "/media/{id}",
        "/media/{id}?r=loop",
        "/media/{id}/poster",
    ))
}

fn only_video(src: &str) -> Block {
    let doc = parse(src).doc;
    doc.blocks
        .into_iter()
        .find(|b| matches!(b, Block::Video { .. }))
        .expect("a ::video block")
}

// ── Grammar ────────────────────────────────────────────────────────────────

#[test]
fn every_attribute_parses() {
    let block = only_video(
        "::video[src=\"media:3f6c\" poster=\"media:3f6c/poster\" autoplay loop muted controls caption=\"How a claim works\" alt=\"A phone\" width=640 aspect=16/9]\n::",
    );
    match block {
        Block::Video { src, poster, autoplay, loops, muted, controls, caption, alt, width, aspect, .. } => {
            assert_eq!(src, "media:3f6c");
            assert_eq!(poster.as_deref(), Some("media:3f6c/poster"));
            assert!(autoplay && loops && muted);
            assert_eq!(controls, Some(true));
            assert_eq!(caption.as_deref(), Some("How a claim works"));
            assert_eq!(alt.as_deref(), Some("A phone"));
            assert_eq!(width.as_deref(), Some("640"));
            assert_eq!(aspect.as_deref(), Some("16/9"));
        }
        other => panic!("expected Video, got {other:?}"),
    }
}

#[test]
fn defaults_are_all_off_and_controls_unstated() {
    match only_video("::video[src=/a.mp4]\n::") {
        Block::Video { src, poster, autoplay, loops, muted, controls, caption, alt, width, aspect, .. } => {
            assert_eq!(src, "/a.mp4");
            assert!(!autoplay && !loops && !muted);
            assert_eq!(controls, None, "unstated — the renderer decides");
            assert!(poster.is_none() && caption.is_none() && alt.is_none());
            assert!(width.is_none() && aspect.is_none());
        }
        other => panic!("expected Video, got {other:?}"),
    }
}

#[test]
fn boolean_flags_accept_bare_and_valued_spellings() {
    for (attrs, want_autoplay, want_controls) in [
        ("autoplay", true, None),
        ("autoplay=true", true, None),
        ("autoplay=false", false, None),
        ("autoplay=\"yes\"", true, None),
        ("autoplay=\"no\"", false, None),
        ("autoplay=1", true, None),
        ("autoplay=0", false, None),
        ("controls", false, Some(true)),
        ("controls=false", false, Some(false)),
        ("controls=\"off\"", false, Some(false)),
        ("autoplay controls", true, Some(true)),
        // A word that is not a boolean reads as not stated.
        ("autoplay=maybe controls=sometimes", false, None),
    ] {
        match only_video(&format!("::video[src=/a.mp4 {attrs}]\n::")) {
            Block::Video { autoplay, controls, .. } => {
                assert_eq!(autoplay, want_autoplay, "{attrs}");
                assert_eq!(controls, want_controls, "{attrs}");
            }
            other => panic!("expected Video, got {other:?}"),
        }
    }
}

#[test]
fn a_video_nests_in_a_section_and_is_a_known_block() {
    let doc = parse("::section\n## Watch\n:::video[src=/a.mp4 alt=\"A\"]\n:::\n::\n").doc;
    match &doc.blocks[0] {
        Block::Section { children, .. } => {
            assert!(children.iter().any(|b| matches!(b, Block::Video { .. })), "{children:?}");
        }
        other => panic!("expected Section, got {other:?}"),
    }
    assert!(surf_parse::lint::known_block_names().contains("video"));
}

// ── The flag rules ─────────────────────────────────────────────────────────

#[test]
fn flag_rules_are_one_public_function() {
    let f = surf_parse::video_flags(true, false, false, None);
    assert!(f.autoplay && f.muted && f.playsinline && !f.controls && !f.loops);
    let f = surf_parse::video_flags(false, true, false, None);
    assert!(!f.autoplay && !f.muted && f.playsinline && f.controls && f.loops);
    assert!(surf_parse::video_flags(true, false, false, Some(true)).controls);
    assert!(!surf_parse::video_flags(false, false, false, Some(false)).controls);
}

// ── HTML ───────────────────────────────────────────────────────────────────

#[test]
fn html_plain_media_video_exact() {
    let _m = site_media();
    assert_eq!(
        html("::video[src=\"media:abc\" caption=\"How a claim works\" alt=\"The claim screen\"]\n::"),
        "<figure class=\"surfdoc-video\"><video class=\"surfdoc-video-player\" controls playsinline preload=\"none\" poster=\"/media/abc/poster\" aria-label=\"The claim screen\"><source src=\"/media/abc\" type=\"video/mp4\"></video><figcaption class=\"surfdoc-video-cap\">How a claim works</figcaption></figure>"
    );
}

#[test]
fn html_autoplay_media_video_exact() {
    let _m = site_media();
    assert_eq!(
        html("::video[src=\"media:abc\" autoplay loop alt=\"Waves\"]\n::"),
        "<figure class=\"surfdoc-video surfdoc-video-autoplay\">\
<video class=\"surfdoc-video-player surfdoc-video-motion\" autoplay muted loop playsinline preload=\"metadata\" poster=\"/media/abc/poster\" aria-label=\"Waves\"><source src=\"/media/abc?r=loop\" type=\"video/mp4\" media=\"(prefers-reduced-motion: no-preference)\"></video>\
<video class=\"surfdoc-video-player surfdoc-video-still\" controls muted loop playsinline preload=\"none\" poster=\"/media/abc/poster\" aria-label=\"Waves\"><source src=\"/media/abc?r=loop\" type=\"video/mp4\"></video>\
</figure>"
    );
}

#[test]
fn html_unresolved_media_is_a_placeholder_never_a_broken_video() {
    // No resolver installed.
    let out = html("::video[src=\"media:abc\" caption=\"How a claim works\" alt=\"The claim screen\"]\n::");
    assert_eq!(
        out,
        "<figure class=\"surfdoc-video surfdoc-video-unavailable\"><div class=\"surfdoc-video-placeholder\" role=\"img\" aria-label=\"The claim screen\"></div><figcaption class=\"surfdoc-video-cap\">How a claim works</figcaption></figure>"
    );
    assert!(!out.contains("<video") && !out.contains("media:"));
    // A resolver that does not know the file gives the same placeholder.
    let _m = install_media_resolver(|_, _| None);
    assert_eq!(html("::video[src=\"media:abc\" caption=\"How a claim works\" alt=\"The claim screen\"]\n::"), out);
}

#[test]
fn html_preload_and_source_type_follow_the_source() {
    // A poster and no autoplay: nothing is fetched before play.
    let with_poster = html("::video[src=/a.webm poster=/p.jpg alt=A]\n::");
    assert!(with_poster.contains("preload=\"none\""), "{with_poster}");
    assert!(with_poster.contains("<source src=\"/a.webm\" type=\"video/webm\">"), "{with_poster}");
    // No poster: the metadata is fetched so the box has its size.
    let bare = html("::video[src=/a.mov alt=A]\n::");
    assert!(bare.contains("preload=\"metadata\""), "{bare}");
    assert!(bare.contains("type=\"video/quicktime\""), "{bare}");
    assert!(!bare.contains("poster="), "{bare}");
    // No recognisable extension: no `type` at all.
    let untyped = html("::video[src=/stream/live alt=A]\n::");
    assert!(untyped.contains("<source src=\"/stream/live\"></video>"), "{untyped}");
    // A query string does not hide the extension.
    let query = html("::video[src=\"https://cdn.example.com/a.mp4?token=1.2\" alt=A]\n::");
    assert!(query.contains("type=\"video/mp4\""), "{query}");
}

#[test]
fn html_controls_follow_the_stated_value() {
    let off = html("::video[src=/a.mp4 controls=false alt=A]\n::");
    assert!(!off.contains(" controls"), "{off}");
    let auto = html("::video[src=/a.mp4 poster=/p.jpg autoplay alt=A]\n::");
    let motion = &auto[..auto.find("surfdoc-video-still").unwrap()];
    assert!(!motion.contains(" controls"), "autoplay drops the default controls: {motion}");
    let stated = html("::video[src=/a.mp4 poster=/p.jpg autoplay controls alt=A]\n::");
    let motion = &stated[..stated.find("surfdoc-video-still").unwrap()];
    assert!(motion.contains(" controls autoplay muted"), "stated controls stay: {motion}");
}

#[test]
fn html_layout_box_is_checked_css() {
    let out = html("::video[src=/a.mp4 alt=A width=640 aspect=9/16]\n::");
    assert!(out.contains("<figure class=\"surfdoc-video\" style=\"max-width:640px\">"), "{out}");
    assert!(out.contains("style=\"aspect-ratio:9 / 16\""), "{out}");
    let hostile = html("::video[src=/a.mp4 alt=A width=\"100%;position:fixed\" aspect=\"1/1;color:red\"]\n::");
    assert!(!hostile.contains("style="), "unchecked values never reach a style: {hostile}");
}

#[test]
fn html_escapes_every_authored_value() {
    let _m = install_media_resolver(|id, _| Some(format!("/m/{id}?a=1&b=\"2\"")));
    let out = html(
        "::video[src=\"/a.mp4?x=1&y=<2>\" poster=\"/p.jpg?q=<s>\" caption=\"<b>bold</b> & \\\"quoted\\\"\" alt=\"<script>alert(1)</script>\"]\n::\n\n::video[src=\"media:abc\" alt=\"A\"]\n::",
    );
    assert!(!out.contains("<script>") && !out.contains("<b>"), "{out}");
    assert!(out.contains("src=\"/a.mp4?x=1&amp;y=&lt;2&gt;\""), "{out}");
    assert!(out.contains("poster=\"/p.jpg?q=&lt;s&gt;\""), "{out}");
    assert!(out.contains("&lt;b&gt;bold&lt;/b&gt; &amp; &quot;quoted&quot;"), "{out}");
    // The host's answer is escaped as well.
    assert!(out.contains("src=\"/m/abc?a=1&amp;b=&quot;2&quot;\""), "{out}");
}

// ── Reduced motion ─────────────────────────────────────────────────────────

#[test]
fn reduced_motion_is_markup_and_css_with_no_script() {
    let out = html("::video[src=/a.mp4 poster=/p.jpg autoplay loop alt=A]\n::");
    assert!(!out.contains("<script"), "no script: {out}");
    assert_eq!(out.matches("<video").count(), 2, "{out}");
    // Exactly one element autoplays, and only its source carries the query.
    assert_eq!(out.matches(" autoplay").count(), 1, "{out}");
    assert_eq!(out.matches("media=\"(prefers-reduced-motion: no-preference)\"").count(), 1, "{out}");
    let (motion, still) = out.split_at(out.find("surfdoc-video-still").unwrap());
    assert!(motion.contains("surfdoc-video-motion") && motion.contains(" autoplay"));
    assert!(motion.contains("media=\"(prefers-reduced-motion: no-preference)\""));
    // The stand-in: the poster with controls, never autoplaying, fetching nothing.
    assert!(still.contains(" controls") && !still.contains(" autoplay"), "{still}");
    assert!(still.contains("preload=\"none\"") && still.contains("poster=\"/p.jpg\""), "{still}");
    assert!(!still.contains("media="), "{still}");

    // The stylesheet half: the stand-in is hidden by default, and under
    // `reduce` the playing element is hidden and the stand-in shown.
    let css = surf_parse::SURFDOC_CSS;
    let default_rule = css.find(".surfdoc-video-still { display: none; }").expect("stand-in hidden by default");
    let guard = css[default_rule..]
        .find("@media (prefers-reduced-motion: reduce)")
        .map(|i| default_rule + i)
        .expect("a reduced-motion block follows");
    let block = &css[guard..guard + 220];
    assert!(block.contains(".surfdoc-video-motion { display: none; }"), "{block}");
    assert!(block.contains(".surfdoc-video-still { display: block; }"), "{block}");
    // A video that does not autoplay is one plain element.
    let plain = html("::video[src=/a.mp4 poster=/p.jpg alt=A]\n::");
    assert_eq!(plain.matches("<video").count(), 1);
    assert!(!plain.contains("surfdoc-video-motion") && !plain.contains("media="));
}

#[test]
fn reduced_motion_hides_the_hero_background_video() {
    let css = surf_parse::SURFDOC_CSS;
    let rule = css.find(".surfdoc-hero-bg { display: none; }").expect("hero video hidden under reduce");
    let guard = css[..rule].rfind("@media").expect("inside a media block");
    assert!(css[guard..rule].starts_with("@media (prefers-reduced-motion: reduce)"));
}

// ── Hero ───────────────────────────────────────────────────────────────────

#[test]
fn hero_video_exact_with_the_image_as_poster() {
    let _m = site_media();
    assert_eq!(
        html("::hero[video=\"media:bg\" image=\"/img/h.jpg\"]\n# Proof\n::"),
        "<section class=\"surfdoc-hero surfdoc-hero-cover surfdoc-hero-video\" style=\"background-image:url('/img/h.jpg')\">\
<video class=\"surfdoc-hero-bg\" autoplay muted loop playsinline preload=\"metadata\" poster=\"/img/h.jpg\" aria-hidden=\"true\" tabindex=\"-1\"><source src=\"/media/bg?r=loop\" type=\"video/mp4\" media=\"(prefers-reduced-motion: no-preference)\"></video>\
<div class=\"surfdoc-hero-inner\"><h1 class=\"surfdoc-hero-headline\">Proof</h1></div></section>"
    );
}

#[test]
fn hero_poster_order_is_stated_then_image_then_library() {
    let _m = site_media();
    let stated = html("::hero[video=/v.mp4 poster=/p.jpg image=/i.jpg]\n# H\n::");
    assert!(stated.contains("poster=\"/p.jpg\"") && stated.contains("url('/p.jpg')"), "{stated}");
    assert!(!stated.contains("/i.jpg"), "the image is not drawn beside the video: {stated}");
    let library = html("::hero[video=\"media:bg\"]\n# H\n::");
    assert!(library.contains("poster=\"/media/bg/poster\""), "{library}");
    let none = html("::hero[video=/v.mp4]\n# H\n::");
    assert!(!none.contains("poster=") && !none.contains("background-image"), "{none}");
    assert!(none.contains("<source src=\"/v.mp4\" type=\"video/mp4\""), "{none}");
}

#[test]
fn hero_without_a_playable_video_is_the_hero_it_was() {
    let plain = html("::hero[image=/i.jpg layout=cover]\n# H\n::");
    // Unresolved library file (no resolver), and an unusable scheme.
    assert_eq!(html("::hero[video=\"media:bg\" image=/i.jpg layout=cover]\n# H\n::"), plain);
    assert_eq!(html("::hero[video=\"http://x.example/v.mp4\" image=/i.jpg layout=cover]\n# H\n::"), plain);
    assert!(!plain.contains("<video"));
}

#[test]
fn hero_poster_cannot_break_out_of_the_css_url() {
    let out = html("::hero[video=/v.mp4 image=\"/img/a') ; x: url('b.jpg\"]\n# H\n::");
    assert!(out.contains("url('/img/a%27%29%20;%20x:%20url%28%27b.jpg')"), "{out}");
}

#[test]
fn figure_is_unchanged() {
    assert_eq!(
        html("::figure[src=/a.png alt=\"A\" caption=\"C\"]\n::"),
        "<figure class=\"surfdoc-figure\"><div class=\"surfdoc-figure-img\"><img src=\"/a.png\" alt=\"A\" data-img-fallback=\"hide\" /></div><figcaption class=\"surfdoc-figure-cap\">C</figcaption></figure>"
    );
}

// ── Embed ──────────────────────────────────────────────────────────────────

#[test]
fn embed_direct_file_is_a_player_with_controls() {
    for src in ["/media/tour.mp4", "https://cdn.example.com/a.webm?v=2", "/a.mov", "/a.m4v", "/a.ogv"] {
        for ty in [" type=video", ""] {
            let out = html(&format!("::embed[src=\"{src}\"{ty} title=\"A tour\"]\n::"));
            assert!(out.starts_with("<figure class=\"surfdoc-video\">"), "{src}{ty}: {out}");
            assert!(out.contains("<video class=\"surfdoc-video-player\" controls playsinline"), "{out}");
            assert!(out.contains("<figcaption class=\"surfdoc-video-cap\">A tour</figcaption>"), "{out}");
            assert!(!out.contains("surfdoc-embed"), "{out}");
        }
    }
    let _m = site_media();
    let out = html("::embed[src=\"media:abc\"]\n::");
    assert!(out.contains("<source src=\"/media/abc\" type=\"video/mp4\">"), "{out}");
}

#[test]
fn embed_provider_page_keeps_the_link_card() {
    for src in [
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtu.be/abc",
        "https://vimeo.com/12345",
        "https://example.com/watch",
    ] {
        let out = html(&format!("::embed[src=\"{src}\" type=video title=\"Watch\"]\n::"));
        assert!(out.contains("surfdoc-embed-title"), "{src}: {out}");
        assert!(!out.contains("<video") && !out.contains("<iframe"), "{src}: {out}");
    }
    // An http: file is not a usable video source; it stays a card too.
    let out = html("::embed[src=\"http://example.com/a.mp4\" type=video]\n::");
    assert!(out.contains("surfdoc-embed") && !out.contains("<video"), "{out}");
}

#[test]
fn embed_unresolved_media_is_the_placeholder() {
    let out = html("::embed[src=\"media:abc\" title=\"A tour\"]\n::");
    assert!(out.contains("surfdoc-video-unavailable"), "{out}");
    assert!(out.contains("A tour") && !out.contains("media:abc"), "{out}");
}

// ── The resolver seam and media_refs ───────────────────────────────────────

#[test]
fn resolver_is_told_the_use_and_guards_restore() {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    {
        let log = seen.clone();
        let _m = install_media_resolver(move |id, media_use| {
            log.borrow_mut().push((id.to_string(), media_use));
            Some(format!("/x/{id}"))
        });
        html("::video[src=\"media:one\" alt=A]\n::\n\n::video[src=\"media:two\" autoplay alt=B]\n::");
    }
    assert_eq!(
        *seen.borrow(),
        vec![
            ("one".to_string(), MediaUse::Video),
            ("one".to_string(), MediaUse::Poster),
            ("two".to_string(), MediaUse::VideoLoop),
            ("two".to_string(), MediaUse::Poster),
        ]
    );
    // Dropped: the same source is unresolved again.
    assert!(html("::video[src=\"media:one\" alt=A]\n::").contains("surfdoc-video-unavailable"));
}

#[test]
fn media_refs_lists_every_library_file_once() {
    let doc = parse(
        "::video[src=\"media:one\" alt=A]\n::\n\n::video[src=\"media:one\" alt=A]\n::\n\n::hero[video=\"media:bg\" poster=\"media:bg/poster\"]\n# H\n::\n\n::page[route=/ title=\"Home\"]\n:::embed[src=\"media:emb\"]\n:::\n:::video[src=/local.mp4 alt=A]\n:::\n::\n",
    )
    .doc;
    assert_eq!(
        surf_parse::media_refs(&doc),
        vec![
            MediaRef { id: "one".into(), media_use: MediaUse::Video },
            MediaRef { id: "one".into(), media_use: MediaUse::Poster },
            MediaRef { id: "bg".into(), media_use: MediaUse::VideoLoop },
            MediaRef { id: "bg".into(), media_use: MediaUse::Poster },
            MediaRef { id: "emb".into(), media_use: MediaUse::Video },
            MediaRef { id: "emb".into(), media_use: MediaUse::Poster },
        ]
    );
    assert_eq!(surf_parse::media_refs_in(&doc.blocks[..1]).len(), 2);
    // Every ref the document lists is one the renderers ask the host for.
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = asked.clone();
    let _m = install_media_resolver(move |id, media_use| {
        let r = MediaRef { id: id.to_string(), media_use };
        if !log.borrow().contains(&r) {
            log.borrow_mut().push(r);
        }
        Some(format!("/m/{id}"))
    });
    doc.to_html_fragment();
    assert_eq!(*asked.borrow(), surf_parse::media_refs(&doc));
}

// ── Degrading renderers ────────────────────────────────────────────────────

const DEGRADE_SRC: &str = "::video[src=\"https://cdn.example.com/a.mp4\" poster=\"/img/p.jpg\" caption=\"How a claim works\" alt=\"A phone\"]\n::";

#[test]
fn markdown_degrades_to_poster_caption_and_link() {
    assert_eq!(
        parse(DEGRADE_SRC).doc.to_markdown().trim(),
        "![A phone](/img/p.jpg)\n*How a claim works*\n[Video: How a claim works](https://cdn.example.com/a.mp4)"
    );
    // An unresolved library file: no picture, no dead link, the words stay.
    let md = parse("::video[src=\"media:abc\" caption=\"How a claim works\"]\n::").doc.to_markdown();
    assert_eq!(md.trim(), "*How a claim works*\nVideo: How a claim works");
    assert!(!md.contains("media:"));
    // Resolved by the host: poster and link are real URLs.
    let _m = site_media();
    let md = parse("::video[src=\"media:abc\" alt=\"A phone\"]\n::").doc.to_markdown();
    assert_eq!(md.trim(), "![A phone](/media/abc/poster)\n[Video: A phone](/media/abc)");
}

#[test]
fn terminal_degrades_to_a_labelled_line() {
    let out = parse(DEGRADE_SRC).doc.to_terminal();
    assert!(out.contains("[Video: How a claim works] (https://cdn.example.com/a.mp4)"), "{out}");
    let out = parse("::video[src=\"media:abc\"]\n::").doc.to_terminal();
    assert!(out.contains("[Video: Video]") && !out.contains("media:"), "{out}");
}

#[test]
fn typst_degrades_to_a_figure_caption_and_link() {
    let out = parse(DEGRADE_SRC).doc.to_typst();
    // No image bytes were handed over, so the placeholder box stands in.
    assert!(out.contains("#figure(\n  rect("), "{out}");
    assert!(out.contains("caption: [How a claim works]"), "{out}");
    assert!(out.contains("#link(\"https://cdn.example.com/a.mp4\")[How a claim works]"), "{out}");
    assert!(!out.contains("image(\"/img/p.jpg\")"), "an unresolved poster is never an image() call");
    // A site path is not a link a printed page can follow.
    let local = parse("::video[src=/a.mp4 alt=\"A phone\"]\n::").doc.to_typst();
    assert!(!local.contains("#link(\"/a.mp4\")"), "{local}");
}

#[test]
fn typst_draws_the_poster_when_the_caller_supplied_its_bytes() {
    let doc = parse("::video[src=\"media:abc\" caption=\"How a claim works\"]\n::").doc;
    // The key is the poster as the document names it — here the default one.
    let map = std::collections::HashMap::from([("media:abc/poster".to_string(), "/img/0.jpg".to_string())]);
    let _images = surf_parse::render_typst::install_image_context(map);
    let out = doc.to_typst();
    assert!(out.contains("#figure(\n  image(\"/img/0.jpg\"),\n  caption: [How a claim works]\n)"), "{out}");
}

#[test]
fn latex_degrades_to_a_figure_caption_and_url() {
    let out = parse(DEGRADE_SRC).doc.to_latex();
    assert!(out.contains("\\includegraphics[width=0.8\\linewidth]{/img/p.jpg}"), "{out}");
    assert!(out.contains("\\url{https://cdn.example.com/a.mp4}"), "{out}");
    assert!(out.contains("\\caption{How a claim works}"), "{out}");
    let out = parse("::video[src=\"media:abc\" alt=\"A phone\"]\n::").doc.to_latex();
    assert!(out.contains("\\textit{Video}") && !out.contains("includegraphics"), "{out}");
}

#[test]
fn slides_draw_the_same_element_as_html() {
    let src = "---\ntype: presentation\n---\n::slide\n:::video[src=/a.mp4 poster=/p.jpg alt=\"A\"]\n:::\n::\n";
    let doc = parse(src).doc;
    let (deck, slides) = surf_parse::extract_deck(&doc);
    let out = surf_parse::render_deck_html(&deck, &slides);
    assert!(
        out.contains("<figure class=\"surfdoc-video\"><video class=\"surfdoc-video-player\" controls playsinline preload=\"none\" poster=\"/p.jpg\" aria-label=\"A\"><source src=\"/a.mp4\" type=\"video/mp4\"></video></figure>"),
        "{out}"
    );
    assert!(out.contains(".surfdoc-video-player"), "the deck carries the video styles");
}

// ── Lint ───────────────────────────────────────────────────────────────────

fn video_diags(src: &str) -> Vec<(String, Severity, String)> {
    surf_parse::check(src)
        .diagnostics
        .into_iter()
        .filter(|d| matches!(d.code.as_deref(), Some("L047") | Some("L048")))
        .map(|d| (d.code.unwrap(), d.severity, d.message))
        .collect()
}

#[test]
fn lint_errors_only_for_a_source_that_cannot_play() {
    for src in [
        "::video[caption=\"C\"]\n::",
        "::video[src=\"\" caption=\"C\"]\n::",
        "::video[src=\"http://example.com/a.mp4\" caption=\"C\"]\n::",
        "::video[src=\"javascript:alert(1)\" caption=\"C\"]\n::",
        "::video[src=\"data:video/mp4;base64,AAAA\" caption=\"C\"]\n::",
        "::video[src=\"//cdn.example.com/a.mp4\" caption=\"C\"]\n::",
        "::section\n:::video[src=\"ftp://x/a.mp4\" caption=\"C\"]\n:::\n::",
    ] {
        let d = video_diags(src);
        assert_eq!(d.len(), 1, "{src}: {d:?}");
        assert_eq!((d[0].0.as_str(), d[0].1), ("L047", Severity::Error), "{src}");
    }
    for src in [
        "::video[src=\"media:abc\" caption=\"C\"]\n::",
        "::video[src=\"/a.mp4\" caption=\"C\"]\n::",
        "::video[src=\"clips/a.mp4\" alt=\"A\"]\n::",
        "::video[src=\"https://cdn.example.com/a.mp4\" poster=\"media:abc/poster\" autoplay alt=\"A\"]\n::",
        "::video[src=\"media:abc\" autoplay alt=\"A\"]\n::",
    ] {
        assert!(video_diags(src).is_empty(), "{src}: {:?}", video_diags(src));
    }
}

#[test]
fn lint_warns_and_never_errors_for_authoring() {
    for (src, needle) in [
        ("::video[src=/a.mp4 autoplay alt=\"A\"]\n::", "autoplays with no poster="),
        ("::video[src=/a.mp4 poster=/p.jpg]\n::", "neither alt= nor caption="),
        ("::video[src=\"media:not a file\" caption=\"C\"]\n::", "is not a media:<file-id> reference"),
        ("::video[src=\"media:abc/poster\" caption=\"C\"]\n::", "names a poster, not a video"),
        ("::video[src=/a.mp4 poster=\"media:abc\" caption=\"C\"]\n::", "names a file, not its poster"),
        ("::video[src=/a.mp4 poster=\"javascript:x\" caption=\"C\"]\n::", "is not a usable picture source"),
        ("::hero[video=\"http://x.example/v.mp4\"]\n# H\n::", "the hero keeps its picture"),
        ("::hero[video=/v.mp4 poster=\"media:abc\"]\n# H\n::", "'::hero' poster="),
    ] {
        let d = video_diags(src);
        assert_eq!(d.len(), 1, "{src}: {d:?}");
        assert_eq!((d[0].0.as_str(), d[0].1), ("L048", Severity::Warning), "{src}");
        assert!(d[0].2.contains(needle), "{src}: {}", d[0].2);
    }
    // Two at once: autoplay with no poster AND no words.
    assert_eq!(video_diags("::video[src=/a.mp4 autoplay]\n::").len(), 2);
}

#[test]
fn lint_rules_are_registered_with_their_severities() {
    let registry = surf_parse::lint::rule_registry();
    assert_eq!(registry["L047"].severity, Severity::Error);
    assert_eq!(registry["L048"].severity, Severity::Warning);
    assert!(!registry["L047"].fixable && !registry["L048"].fixable);
}

// ── Model field ────────────────────────────────────────────────────────────

#[test]
fn model_field_type_video_and_its_aliases() {
    let doc = parse("::model[name=Lesson]\n- id: uuid pk\n- intro: video\n- teaser: clip [optional]\n- feature: movie\n::").doc;
    match &doc.blocks[0] {
        Block::Model { fields, .. } => {
            let types: Vec<&ModelFieldType> = fields.iter().map(|f| &f.field_type).collect();
            assert_eq!(types[1..], [&ModelFieldType::Video, &ModelFieldType::Video, &ModelFieldType::Video]);
        }
        other => panic!("expected Model, got {other:?}"),
    }
    for word in ["video", "clip", "movie", "Video"] {
        assert_eq!(surf_parse::parse_schema_field_type(word).unwrap(), ModelFieldType::Video, "{word}");
    }
    // It reads back as `video` and serializes to a fixed point.
    let first = doc.to_surf_source();
    assert!(first.contains("intro: video"), "{first}");
    assert_eq!(parse(&first).doc.to_surf_source(), first);
    assert!(doc.to_markdown().contains("video"));
    assert!(doc.to_html().contains("video"));
}

// ── Builder and the serializer's fixed point ───────────────────────────────

#[test]
fn builder_emits_video_blocks() {
    let doc = SurfDocBuilder::new()
        .video("media:abc")
        .video_with_caption("/a.mp4", Some("/p.jpg"), "How a claim works", Some("A phone"))
        .video_loop("/loop.webm", Some("/loop.jpg"), Some("Waves"))
        .build();
    assert_eq!(doc.blocks.len(), 3);
    let source = doc.to_surf_source();
    assert!(source.contains("::video[src=\"media:abc\"]\n::"), "{source}");
    assert!(
        source.contains("::video[src=\"/a.mp4\" poster=\"/p.jpg\" caption=\"How a claim works\" alt=\"A phone\"]"),
        "{source}"
    );
    assert!(
        source.contains("::video[src=\"/loop.webm\" poster=\"/loop.jpg\" autoplay loop muted alt=\"Waves\"]"),
        "{source}"
    );
    let reparsed = parse(&source).doc;
    assert_eq!(reparsed.blocks.iter().filter(|b| matches!(b, Block::Video { .. })).count(), 3);
    assert_eq!(reparsed.to_html_fragment(), doc.to_html_fragment());
}

#[test]
fn serialize_is_a_fixed_point_on_the_first_pass() {
    let fixture = std::fs::read_to_string(format!("{}/tests/fixtures/video.surf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    for src in [
        fixture.as_str(),
        "::video[src=\"media:abc\" poster=\"media:abc/poster\" autoplay loop muted controls caption=\"A \\\"quoted\\\" caption\" alt=\"A\" width=\"640\" aspect=\"16/9\"]\n::\n",
        "::video[src=/a.mp4 controls=false]\n::\n",
        "::hero[video=\"media:bg\" poster=\"/p.jpg\" image=\"/i.jpg\" align=left]\n# H\n\nSub\n\n[Go](/go){primary}\n::\n",
        "::section\n## Watch\n:::video[src=/a.mp4 autoplay=true loop=false alt=\"A\"]\n:::\n::\n",
    ] {
        let _m = site_media();
        let first = parse(src).doc.to_surf_source();
        let second = parse(&first).doc.to_surf_source();
        assert_eq!(first, second, "not a fixed point for:\n{src}\nfirst pass:\n{first}");
        assert_eq!(
            parse(src).doc.to_html_fragment(),
            parse(&first).doc.to_html_fragment(),
            "the serialized source renders differently:\n{first}"
        );
    }
}

// ── Hostile input ──────────────────────────────────────────────────────────

#[test]
fn hostile_sources_never_reach_the_page_and_never_panic() {
    let _m = install_media_resolver(|id, _| Some(format!("/m/{id}")));
    for src in [
        "javascript:alert(1)",
        "JAVASCRIPT:alert(1)",
        "data:text/html,<script>alert(1)</script>",
        "vbscript:x",
        "http://example.com/a.mp4",
        "//evil.example/a.mp4",
        "\\\\\\\\evil.example\\\\a.mp4",
        "media:../../etc/passwd",
        "media:abc?x=1",
        "media:",
        "media:a/poster/poster",
        "blob:https://x/y",
        " ",
    ] {
        let doc = parse(&format!(
            "::video[src=\"{src}\" poster=\"{src}\" autoplay caption=\"C\"]\n::\n\n::hero[video=\"{src}\" poster=\"{src}\"]\n# H\n::\n\n::embed[src=\"{src}\" type=video]\n::\n"
        ))
        .doc;
        let out = doc.to_html_fragment();
        assert!(!out.contains("<video"), "{src}: {out}");
        assert!(!out.contains("<source"), "{src}: {out}");
        assert!(out.contains("surfdoc-video-unavailable"), "{src}: {out}");
        // Every other format degrades without a panic.
        let _ = (doc.to_markdown(), doc.to_terminal(), doc.to_typst(), doc.to_latex(), doc.to_html());
        assert!(surf_parse::media_refs(&doc).is_empty(), "{src}");
        let _ = surf_parse::check(&doc.to_surf_source());
    }
}

#[test]
fn unterminated_and_garbage_video_blocks_do_not_panic() {
    for src in [
        "::video",
        "::video[",
        "::video[src=",
        "::video[src=\"media:abc",
        "::video[src=media:abc autoplay=autoplay loop=loop aspect=0/0 width=-1 width=1e309]\n::",
        "::video[autoplay autoplay autoplay controls=]\n::",
        "::hero[video= poster=]\n::",
        ":::video[src=/a.mp4]\n::::\n::",
    ] {
        let doc = parse(src).doc;
        let _ = (doc.to_html(), doc.to_html_fragment(), doc.to_markdown(), doc.to_terminal(), doc.to_typst(), doc.to_latex());
        let _ = surf_parse::media_refs(&doc);
        let _ = surf_parse::check(src);
    }
}

#[test]
fn media_helpers_are_public() {
    assert!(media::is_direct_video_src("media:abc"));
    assert_eq!(media::video_mime("/a.webm"), Some("video/webm"));
    assert_eq!(media::video_aspect("16/9").as_deref(), Some("16 / 9"));
    assert_eq!(media::effective_poster("media:abc", None).as_deref(), Some("media:abc/poster"));
    assert_eq!(media::media_url("/a.mp4", MediaUse::Video).as_deref(), Some("/a.mp4"));
    assert_eq!(media::MEDIA_SCHEME, "media:");
}

// ── The DOM twin ───────────────────────────────────────────────────────────

#[cfg(feature = "dom")]
mod dom {
    use super::*;
    use surf_parse::render_dom::{attr_allowed, check_coverage, render_fragment_string};

    fn assert_twin(src: &str) {
        let doc = parse(src).doc;
        check_coverage(&doc).unwrap_or_else(|e| panic!("declined: {e}\n{src}"));
        assert_eq!(
            render_fragment_string(&doc).expect("native sink renders"),
            doc.to_html_fragment(),
            "the DOM twin drifted from render_html for:\n{src}"
        );
    }

    #[test]
    fn dom_draws_video_hero_video_and_the_file_embed_byte_identically() {
        let fixture = std::fs::read_to_string(format!("{}/tests/fixtures/video.surf", env!("CARGO_MANIFEST_DIR"))).unwrap();
        // Unresolved (placeholders and fallbacks) …
        assert_twin(&fixture);
        // … and resolved by a host.
        let _m = site_media();
        assert_twin(&fixture);
        assert_twin("::video[src=\"media:abc\" autoplay loop alt=\"Waves\"]\n::");
        assert_twin("::hero[video=\"media:bg\" image=\"/img/h.jpg\"]\n# Proof\n::");
        assert_twin("::embed[src=\"media:abc\" title=\"A tour\"]\n::");
        let doc = parse(&fixture).doc;
        let out = render_fragment_string(&doc).unwrap();
        assert!(out.contains("<video class=\"surfdoc-video-player surfdoc-video-motion\" autoplay muted loop playsinline"), "{out}");
        assert!(out.contains("<video class=\"surfdoc-hero-bg\" autoplay muted loop playsinline"), "{out}");
    }

    #[test]
    fn dom_allowlist_carries_the_video_attributes_and_no_script_sink() {
        for name in ["poster", "controls", "autoplay", "muted", "loop", "playsinline", "preload", "media"] {
            assert!(attr_allowed(name), "{name}");
        }
        for name in ["onerror", "onplay", "srcdoc", "onloadeddata"] {
            assert!(!attr_allowed(name), "{name}");
        }
    }

    #[test]
    fn dom_hostile_video_sources_stay_identical_and_inert() {
        for src in ["javascript:alert(1)", "data:text/html,x", "\"><script>alert(1)</script>", "media:<img>"] {
            let doc = parse(&format!("::video[src='{src}' caption=\"C\"]\n::\n\n::video[src=\"{}\" caption=\"C\"]\n::", src.replace('"', "\\\""))).doc;
            let out = render_fragment_string(&doc).expect("renders");
            assert_eq!(out, doc.to_html_fragment(), "{src}");
            assert!(!out.contains("<script") && !out.contains("src=\"javascript:"), "{src}: {out}");
        }
    }
}

// ── The native node ────────────────────────────────────────────────────────

#[cfg(feature = "native")]
mod native {
    use super::*;
    use surf_parse::render_native::{NativeBlock, NATIVE_DOC_SCHEMA_VERSION};

    #[test]
    fn native_schema_is_sixteen() {
        // 15 was 0.37.0's (this lane); 0.40.0 moved it to 16.
        assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 16);
    }

    #[test]
    fn native_video_carries_the_authored_source_and_effective_flags() {
        let blocks = parse(
            "::video[src=\"media:abc\" autoplay loop caption=\"How a claim works\" alt=\"A phone\" aspect=16/9]\n::",
        )
        .doc
        .to_native_blocks();
        assert_eq!(
            blocks,
            vec![NativeBlock::Video {
                src: "media:abc".into(),
                poster: Some("media:abc/poster".into()),
                autoplay: true,
                loops: true,
                muted: true,
                controls: false,
                playsinline: true,
                caption: Some("How a claim works".into()),
                alt: Some("A phone".into()),
                aspect: Some("16/9".into()),
            }]
        );
        let json = serde_json::to_value(&blocks[0]).unwrap();
        assert_eq!(json["type"], "video");
        assert_eq!(json["src"], "media:abc");
        assert_eq!(json["muted"], true);
        let back: NativeBlock = serde_json::from_value(json).unwrap();
        assert_eq!(back, blocks[0]);
    }

    #[test]
    fn native_plain_video_has_controls_and_no_invented_poster() {
        let blocks = parse("::video[src=/a.mp4 aspect=\"wide\"]\n::").doc.to_native_blocks();
        match &blocks[0] {
            NativeBlock::Video { src, poster, autoplay, muted, controls, playsinline, aspect, .. } => {
                assert_eq!(src, "/a.mp4");
                assert!(poster.is_none() && !autoplay && !muted && *controls && *playsinline);
                assert!(aspect.is_none(), "an aspect that is not w/h does not cross");
            }
            other => panic!("expected Video, got {other:?}"),
        }
    }

    #[test]
    fn native_resolver_does_not_rewrite_the_source() {
        let _m = site_media();
        match &parse("::video[src=\"media:abc\"]\n::").doc.to_native_blocks()[0] {
            NativeBlock::Video { src, poster, .. } => {
                assert_eq!(src, "media:abc");
                assert_eq!(poster.as_deref(), Some("media:abc/poster"));
            }
            other => panic!("expected Video, got {other:?}"),
        }
    }

    #[test]
    fn native_file_embed_is_a_video_and_a_provider_page_stays_an_embed() {
        let blocks = parse(
            "::embed[src=/tour.mp4 title=\"A tour\"]\n::\n\n::embed[src=\"https://youtu.be/abc\" title=\"Watch\"]\n::",
        )
        .doc
        .to_native_blocks();
        match &blocks[0] {
            NativeBlock::Video { src, controls, autoplay, caption, .. } => {
                assert_eq!(src, "/tour.mp4");
                assert!(*controls && !autoplay);
                assert_eq!(caption.as_deref(), Some("A tour"));
            }
            other => panic!("expected Video, got {other:?}"),
        }
        assert!(matches!(&blocks[1], NativeBlock::Embed { embed_type, .. } if embed_type == "video"));
    }

    #[test]
    fn native_hero_carries_video_and_poster() {
        let blocks = parse(
            "::hero[video=\"media:bg\" image=/i.jpg]\n# H\n::\n\n::hero[video=\"media:bg\"]\n# H\n::\n\n::hero[video=\"http://x.example/v.mp4\" image=/i.jpg]\n# H\n::\n\n::hero[image=/i.jpg]\n# H\n::",
        )
        .doc
        .to_native_blocks();
        let pairs: Vec<(Option<&str>, Option<&str>)> = blocks
            .iter()
            .map(|b| match b {
                NativeBlock::Hero { video, poster, .. } => (video.as_deref(), poster.as_deref()),
                other => panic!("expected Hero, got {other:?}"),
            })
            .collect();
        assert_eq!(
            pairs,
            vec![
                (Some("media:bg"), Some("/i.jpg")),
                (Some("media:bg"), Some("media:bg/poster")),
                // An unusable source does not cross; the hero is its picture.
                (None, None),
                (None, None),
            ]
        );
        // A hero without a video serializes exactly as before.
        let json = serde_json::to_string(&blocks[3]).unwrap();
        assert!(!json.contains("video") && !json.contains("poster"), "{json}");
    }
}

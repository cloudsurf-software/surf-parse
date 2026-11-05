//! The site base and the theme line (0.45.0 — D-LOOK-1 · D-LOOK-7).
//!
//! 1. A DOC renders byte for byte as 0.44.2 did: the HTML of three doc
//!    renders is pinned in `tests/golden/site-base-doc-*.html.snap`, written
//!    by the 0.44.2 crate (origin/main 5334274) and compared here; the five
//!    rhythm variables default to the literal values they replaced; the doc's
//!    48rem column rule is untouched; a doc page carries no site mark.
//! 2. A SITE page (main#surfdoc-main) reads the five: its column is
//!    `--ws-wrap`, its gutter `--ws-side` once, its page-level blocks one
//!    `--ws-block-gap`.
//! 3. The author's `theme:` line is the default: `light` / `dark` put
//!    `data-theme` on `<html>`; `auto` and no line follow the device; a
//!    visitor's stored toggle still wins; a hand-built `SiteConfig` that
//!    leaves `theme_default` unset renders exactly as 0.44.2.
//!
//! UPDATING THE SNAPSHOTS (same contract as tests/golden_union.rs):
//!   UPDATE_SNAPSHOTS=1 cargo test --test site_base_layers
//! A doc snapshot may change only with a deliberate doc render change.

use std::fs;
use std::path::{Path, PathBuf};

use surf_parse::{PageConfig, SURFDOC_CSS, Severity, SiteConfig};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = golden_dir().join(name);
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&path, actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {} — run UPDATE_SNAPSHOTS=1 cargo test --test site_base_layers",
            path.display()
        )
    });
    assert!(
        expected == actual,
        "doc render drifted from 0.44.2 ({name}) — the site base must leave a doc byte-identical"
    );
}

fn fixture(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(rel))
        .unwrap_or_else(|_| panic!("fixture {rel}"))
}

fn parse_clean(source: &str) -> surf_parse::SurfDoc {
    let result = surf_parse::parse(source);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    result.doc
}

/// The value of a custom property declared in the sheet's first `:root`
/// block (the `--ws-*` contract).
fn root_var(name: &str) -> Option<String> {
    let start = SURFDOC_CSS.find(":root {")?;
    let end = start + SURFDOC_CSS[start..].find("\n}")?;
    let block = &SURFDOC_CSS[start..end];
    let needle = format!("{name}:");
    let at = block.find(&needle)?;
    let rest = &block[at + needle.len()..];
    Some(rest[..rest.find(';')?].trim().to_string())
}

// ── 1. a doc renders byte-identical ──────────────────────────────────────

#[test]
fn doc_fragment_strategy_sample_is_byte_identical_to_0_44_2() {
    let doc = parse_clean(&fixture("fixtures/strategy-sample.surf"));
    assert_snapshot("site-base-doc-strategy-sample.html.snap", &doc.to_html());
}

#[test]
fn doc_fragment_marketing_blocks_is_byte_identical_to_0_44_2() {
    let doc = parse_clean(&fixture("corpus/tier2-marketing.surf"));
    assert_snapshot("site-base-doc-tier2-marketing.html.snap", &doc.to_html());
}

#[test]
fn doc_page_without_the_sheet_is_byte_identical_to_0_44_2() {
    let doc = parse_clean(&fixture("corpus/tier2-marketing.surf"));
    let config = PageConfig { embed_css: false, ..PageConfig::default() };
    assert_snapshot(
        "site-base-doc-page-tier2-marketing.html.snap",
        &doc.to_html_page(&config),
    );
}

#[test]
fn doc_page_carries_no_site_mark_and_no_rhythm_override() {
    let doc = parse_clean(&fixture("corpus/tier2-marketing.surf"));
    let config = PageConfig { embed_css: false, ..PageConfig::default() };
    let html = doc.to_html_page(&config);
    assert!(!html.contains("surfdoc-main"), "a doc page is not marked as a site page");
    assert!(!html.contains("--ws-"), "a doc page sets none of the rhythm variables");
    assert!(!html.contains("data-theme=\""), "a doc page is not pinned to a theme");
}

#[test]
fn the_five_variables_are_declared_with_the_values_they_replaced() {
    assert_eq!(root_var("--ws-wrap").as_deref(), Some("72rem"));
    assert_eq!(root_var("--ws-side").as_deref(), Some("1.5rem"));
    assert_eq!(root_var("--ws-section-pad").as_deref(), Some("clamp(64px, 7vw, 120px)"));
    assert_eq!(root_var("--ws-block-gap").as_deref(), Some("2rem"));
    assert_eq!(root_var("--ws-hero-min-h").as_deref(), Some("min(74vh, 44rem)"));
    // The doc's column is untouched.
    assert!(SURFDOC_CSS.contains(".surfdoc { max-width: 48rem; margin: 0 auto; padding: 2rem 1.5rem 4rem;"));
    // The page-level blocks read the gap; the old literal is gone from them.
    for block in [".surfdoc-features {", ".surfdoc-stats {", ".surfdoc-steps {", ".surfdoc-logo {", ".surfdoc-pipeline {"] {
        let at = SURFDOC_CSS.find(block).unwrap_or_else(|| panic!("{block} rule"));
        let rule = &SURFDOC_CSS[at..at + SURFDOC_CSS[at..].find('}').unwrap()];
        assert!(rule.contains("margin-block: var(--ws-block-gap); margin-inline: 0;"), "{block} reads the gap: {rule}");
        assert!(!rule.contains("margin: 2rem 0"), "{block} keeps no literal margin");
    }
    assert!(SURFDOC_CSS.contains(".surfdoc-hero-cover { min-height: var(--ws-hero-min-h);"));
}

// ── 2. a site page reads the five ───────────────────────────────────────

const SITE: &str = "---\ntitle: \"Rhythm\"\ntype: doc\n---\n\n\
::site[domain=\"rhythm.test\"]\nname: Rhythm\n::\n\n\
::page[route=\"/\" title=\"Home\"]\n\n\
::stats\n- value=\"12K+\" label=\"Makers\"\n- value=\"98%\" label=\"Happy\"\n- value=\"40\" label=\"Cities\"\n::\n\n\
::cta[label=\"Start\" href=\"/go\" primary]\n::\n\
::\n";

#[test]
fn site_page_is_marked_by_main_surfdoc_main_and_the_sheet_reads_the_five_there() {
    let doc = parse_clean(SITE);
    let (site, pages, _) = surf_parse::extract_site(&doc);
    let site = site.expect("::site");
    let html = surf_parse::render_site_page(&pages[0], &site, &[], &PageConfig::default());
    // The mark the site rules key on — the markup is unchanged from 0.44.2.
    assert!(html.contains("<main id=\"surfdoc-main\" class=\"surfdoc\">"));
    // The column reads --ws-wrap and the one gutter.
    assert!(SURFDOC_CSS.contains(
        ".surfdoc:where(#surfdoc-main) { max-width: var(--ws-wrap); padding-inline: var(--ws-side); padding-bottom: 0; }"
    ));
    // One gap between page-level blocks, the cta group included.
    assert!(SURFDOC_CSS.contains(":where(.surfdoc#surfdoc-main, .surfdoc#surfdoc-main > .surfdoc-page) > :is(.surfdoc-features, .surfdoc-stats,"));
    assert!(SURFDOC_CSS.contains(".surfdoc:where(#surfdoc-main) > .surfdoc-cta-group,"));
    // The section inner no longer doubles the gutter.
    assert!(SURFDOC_CSS.contains(":where(.surfdoc#surfdoc-main) .surfdoc-section-inner { max-width: var(--ws-wrap); padding-inline: 0; }"));
    // The footer meets the last block at the one gap.
    assert!(html.contains(".surfdoc-site-footer { margin-top: var(--ws-block-gap, 4rem);"));
    // The blocks themselves rendered.
    assert!(html.contains("class=\"surfdoc-stats\""));
    assert!(html.contains("surfdoc-cta-group"));
}

#[test]
fn site_rules_sit_last_so_they_follow_the_block_rules_they_replace() {
    let site_rule = SURFDOC_CSS.find(".surfdoc:where(#surfdoc-main) { max-width").expect("site column rule");
    for block in [".surfdoc { max-width: 48rem", ".surfdoc-stats {", ".surfdoc .surfdoc-cta-group {", ".surfdoc-section-inner {"] {
        let at = SURFDOC_CSS.find(block).unwrap_or_else(|| panic!("{block}"));
        assert!(at < site_rule, "{block} comes before the site base");
    }
}

// ── 3. the theme line is the default ────────────────────────────────────

fn site_with_theme(line: Option<&str>) -> String {
    let theme = line.map(|t| format!("theme: {t}\n")).unwrap_or_default();
    format!(
        "---\ntitle: \"T\"\ntype: doc\n---\n\n::site[domain=\"t.test\"]\nname: T\n{theme}::\n\n\
         ::page[route=\"/\" title=\"Home\"]\nHello.\n::\n"
    )
}

fn render_with_theme(line: Option<&str>) -> String {
    let doc = parse_clean(&site_with_theme(line));
    let (site, pages, _) = surf_parse::extract_site(&doc);
    surf_parse::render_site_page(&pages[0], &site.expect("::site"), &[], &PageConfig::default())
}

#[test]
fn theme_light_line_pins_light_on_html() {
    let html = render_with_theme(Some("light"));
    assert!(html.contains("<html lang=\"en\" data-theme=\"light\">"), "{html}");
    // No device lookup: the author's line wins over the scheme.
    assert!(!html.contains("prefers-color-scheme: dark)').matches"));
    // A visitor's stored toggle still overrides.
    assert!(html.contains("localStorage.getItem('surfdoc-site-theme:/')"));
    assert!(html.contains("if(s==='light'||s==='dark'){d.setAttribute('data-theme',s);}"));
}

#[test]
fn theme_dark_line_pins_dark_on_html() {
    let html = render_with_theme(Some("dark"));
    assert!(html.contains("<html lang=\"en\" data-theme=\"dark\">"), "{html}");
}

#[test]
fn theme_auto_line_and_no_line_follow_the_device() {
    for line in [Some("auto"), None] {
        let html = render_with_theme(line);
        assert!(html.contains("<html lang=\"en\">"), "{line:?}: no attribute");
        assert!(!html.contains("<html lang=\"en\" data-theme"), "{line:?}");
        assert!(
            html.contains("var t=s||((mm&&mm('(prefers-color-scheme: dark)').matches)?'dark':"),
            "{line:?}: the resolver follows the device"
        );
    }
}

#[test]
fn single_file_site_honours_the_line_too() {
    let doc = parse_clean(&site_with_theme(Some("light")));
    let (site, pages, _) = surf_parse::extract_site(&doc);
    let html = surf_parse::render_site_single_file(&site.expect("::site"), &pages, &[], &PageConfig::default());
    assert!(html.contains("<html lang=\"en\" data-theme=\"light\">"));
}

#[test]
fn a_hand_built_config_without_the_line_renders_as_0_44_2() {
    // The surf container builds SiteConfig by hand with the palette's hint in
    // `theme` — until it passes the author's line, nothing changes for it.
    for hint in ["light", "dark"] {
        let site = SiteConfig {
            name: Some("Palette".into()),
            theme: Some(hint.into()),
            base_path: Some("/s/palette".into()),
            ..Default::default()
        };
        let doc = parse_clean(&site_with_theme(None));
        let (_, pages, _) = surf_parse::extract_site(&doc);
        let html = surf_parse::render_site_page(&pages[0], &site, &[], &PageConfig::default());
        assert!(html.contains("<html lang=\"en\">"), "{hint}: not pinned");
        let expected = format!(
            "<script>(function(){{var d=document.documentElement;var s=null;\
             try{{s=localStorage.getItem('surfdoc-site-theme:/s/palette')}}catch(e){{}}\
             var mm=window.matchMedia;\
             var t=s||((mm&&mm('(prefers-color-scheme: dark)').matches)?'dark':\
             (mm&&mm('(prefers-color-scheme: light)').matches)?'light':'{hint}');\
             d.setAttribute('data-theme',t);}})();</script>"
        );
        assert!(html.contains(&expected), "{hint}: the 0.44.2 resolver, byte for byte");
    }
}

#[test]
fn extract_site_keeps_the_hint_and_records_the_line() {
    let doc = parse_clean(&site_with_theme(Some("dark")));
    let (site, _, _) = surf_parse::extract_site(&doc);
    let site = site.expect("::site");
    assert_eq!(site.theme.as_deref(), Some("dark"));
    assert_eq!(site.theme_default.as_deref(), Some("dark"));
}

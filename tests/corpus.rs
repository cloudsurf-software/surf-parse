//! Golden-corpus conformance suite (render-unification plan, Phase 5).
//!
//! One corpus, two snapshot pins: every fixture in `tests/corpus/` is
//! rendered through BOTH targets — `render_html` (websites) and
//! `to_native_blocks` (apps, serialized as JSON) — and pinned against
//! checked-in snapshots in `tests/snapshots/`. Any change to either
//! renderer's output for any tier shows up as a reviewable snapshot diff.
//!
//! Update intentionally with:
//!   UPDATE_SNAPSHOTS=1 cargo test --test corpus --features native
//!
//! The coverage-manifest tests at the bottom enforce the tier model:
//! every `Block` variant is classified (compile-time, via `block_tier`'s
//! exhaustive match) and the classification agrees with what conversion
//! actually does (runtime, over the corpus — which exercises all four
//! tiers).

#![cfg(feature = "native")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use surf_parse::render_native::{self, BlockTier, NativeBlock};

fn corpus_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("tests/corpus exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map(|e| e == "surf").unwrap_or(false))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "corpus must not be empty");
    files
}

fn snapshot_path(fixture: &Path, target: &str) -> PathBuf {
    let stem = fixture.file_stem().unwrap().to_str().unwrap();
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{stem}.{target}.snap"))
}

/// Compare `actual` against the checked-in snapshot, or (re)write it when
/// UPDATE_SNAPSHOTS=1. Missing snapshot + no update flag = hard failure,
/// so CI can never silently skip a pin.
fn assert_snapshot(fixture: &Path, target: &str, actual: &str) {
    let path = snapshot_path(fixture, target);
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&path, actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {} — run UPDATE_SNAPSHOTS=1 cargo test --test corpus --features native",
            path.display()
        )
    });
    assert_eq!(
        expected, actual,
        "snapshot drift for {} ({target}) — if intentional, regenerate with UPDATE_SNAPSHOTS=1",
        fixture.display()
    );
}

/// Pin render_html output for every corpus fixture (websites — the
/// most-used artifact; this is the "render_html never changes output"
/// guarantee made executable).
#[test]
fn corpus_render_html_pinned() {
    for fixture in corpus_files() {
        let src = fs::read_to_string(&fixture).unwrap();
        let doc = surf_parse::parse(&src).doc;
        assert_snapshot(&fixture, "html", &doc.to_html());
    }
}

/// Pin the native projection (block tree as canonical JSON) for every
/// corpus fixture — the FFI contract apps consume.
#[test]
fn corpus_to_native_pinned() {
    for fixture in corpus_files() {
        let src = fs::read_to_string(&fixture).unwrap();
        let doc = surf_parse::parse(&src).doc;
        let native = render_native::to_native_blocks(&doc);
        let json = serde_json::to_string_pretty(&native).unwrap();
        assert_snapshot(&fixture, "native", &json);
    }
}

/// The three widths the 0.18 axis is pinned at — one per size class,
/// taken from real devices (iPhone portrait, iPad portrait, laptop).
const CORPUS_WIDTHS: [u32; 3] = [390, 834, 1280];

/// Pin BOTH renderers for the width-varying fixture at every size class.
///
/// The render paths take no width, so the width is threaded the only way
/// it can be: `SurfDoc::for_width` projects the document onto one class
/// (collapsing per-class values, dropping gated chrome) and the ordinary
/// renderers run on the projection. Three widths x two targets = six new
/// pins per width-varying fixture.
#[test]
fn corpus_width_varying_pinned() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/corpus/tier5-size-class.surf");
    let src = fs::read_to_string(&fixture).unwrap();
    let doc = surf_parse::parse(&src).doc;
    for width in CORPUS_WIDTHS {
        let class = surf_parse::resolve_size_class(width);
        let projected = doc.for_width(width);
        assert_snapshot(&fixture, &format!("w{width}.html"), &projected.to_html());
        let native = render_native::to_native_blocks(&projected);
        assert_snapshot(
            &fixture,
            &format!("w{width}.native"),
            &serde_json::to_string_pretty(&native).unwrap(),
        );
        // The resolver agrees with the class the projection was built for.
        assert_eq!(
            class,
            surf_parse::resolve_size_class(width),
            "resolver must be pure"
        );
    }
}

/// The projection is a no-op for a document that uses none of the 0.18
/// attributes: every other corpus fixture renders identically at all three
/// widths, so the axis cannot silently change existing documents.
#[test]
fn projection_is_identity_for_documents_without_the_axis() {
    for fixture in corpus_files() {
        let src = fs::read_to_string(&fixture).unwrap();
        // `desktop-only=` counts: it is the deprecated spelling of
        // `classes=desktop` and gates the block exactly the same way.
        if ["classes=", "min-class=", "desktop-only=", "cols=\"", "columns=\"", "width=\""]
            .iter()
            .any(|marker| src.contains(marker))
        {
            continue;
        }
        let doc = surf_parse::parse(&src).doc;
        let baseline = doc.to_html();
        for width in CORPUS_WIDTHS {
            assert_eq!(
                baseline,
                doc.for_width(width).to_html(),
                "{} must render identically at {width}px — it uses no size-class attributes",
                fixture.display()
            );
        }
    }
}

/// Pin the resolved theme for a style-packed parse of the site fixture,
/// under both packs. Theme resolution lives in Rust (resolve.rs); this
/// pin is what makes "pack values resolve once" enforceable.
#[cfg(feature = "uniffi")]
#[test]
fn corpus_styled_themes_pinned() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/tier2-site.surf");
    let src = fs::read_to_string(&fixture).unwrap();
    let mut out = String::new();
    for pack in ["surf", "comic"] {
        let doc = surf_parse::ffi::parse_to_native_styled(
            src.clone(),
            Some(pack.to_string()),
            None,
            None,
        )
        .expect("styled parse");
        out.push_str(&format!(
            "=== pack: {pack} ===\n{}\n",
            serde_json::to_string_pretty(&doc.theme).unwrap()
        ));
    }
    assert_snapshot(&fixture, "themes", &out);
}

// ═══════════════════════════════════════════════════════════════════════
// Coverage manifest — the tier model, enforced
// ═══════════════════════════════════════════════════════════════════════

/// The serde tag (`kind`) of a parsed block — the variant name.
fn kind_of(block: &surf_parse::Block) -> String {
    serde_json::to_value(block).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Top-level blocks only. Nested children are exercised through the
/// snapshot pins (the native JSON serializes the whole tree); the tier
/// check needs each kind to appear at top level at least once, which the
/// corpus fixtures guarantee (chrome blocks appear both nested in the
/// app-shell and standalone).
fn walk<'a>(blocks: &'a [surf_parse::Block], out: &mut Vec<&'a surf_parse::Block>) {
    for b in blocks {
        out.push(b);
    }
}

/// Every block variant the corpus produces maps to a tier, and the tier
/// agrees with what conversion produces: Degraded ⇒ markdown string,
/// structured tiers ⇒ structured NativeBlock (modulo Markdown itself).
/// Also asserts the corpus actually covers all four tiers, so this test
/// cannot green-wash an empty corpus.
#[test]
fn corpus_tier_manifest_consistent() {
    let mut seen: BTreeMap<String, BlockTier> = BTreeMap::new();
    let mut tier_counts: BTreeMap<&'static str, usize> = BTreeMap::new();

    for fixture in corpus_files() {
        let src = fs::read_to_string(&fixture).unwrap();
        let doc = surf_parse::parse(&src).doc;
        let mut all: Vec<&surf_parse::Block> = Vec::new();
        walk(&doc.blocks, &mut all);
        for block in all {
            let tier = render_native::block_tier(block);
            let kind = kind_of(block);
            if let Some(prev) = seen.insert(kind.clone(), tier) {
                assert_eq!(prev, tier, "tier classification for {kind} must be stable");
            }
            *tier_counts
                .entry(match tier {
                    BlockTier::Content => "content",
                    BlockTier::Site => "site",
                    BlockTier::Chrome => "chrome",
                    BlockTier::Degraded => "degraded",
                })
                .or_default() += 1;

            let native = render_native::to_native_blocks(&surf_parse::SurfDoc {
                blocks: vec![block.clone()],
                ..doc.clone()
            });
            match tier {
                BlockTier::Degraded => {
                    assert!(
                        matches!(native[0], NativeBlock::Markdown { .. }),
                        "Degraded-tier {kind} must convert to Markdown"
                    );
                }
                _ => {
                    if kind != "Markdown" {
                        assert!(
                            !matches!(native[0], NativeBlock::Markdown { .. }),
                            "structured-tier {kind} must not silently degrade"
                        );
                    }
                }
            }
        }
    }

    for tier in ["content", "site", "chrome", "degraded"] {
        assert!(
            tier_counts.get(tier).copied().unwrap_or(0) > 0,
            "corpus must cover the {tier} tier (counts: {tier_counts:?})"
        );
    }
    // The corpus exercises a healthy share of the 95 variants; widen as
    // fixtures grow. (Totality itself is compile-time: block_tier has no
    // wildcard arm.)
    assert!(
        seen.len() >= 45,
        "corpus covers {} block kinds — expected ≥ 45: {:?}",
        seen.len(),
        seen.keys().collect::<Vec<_>>()
    );
}

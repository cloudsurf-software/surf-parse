//! Integration tests for the lint engine (`surf_parse::check`).
//!
//! Covers the `tests/fixtures/lint/` corpus (one fixture per rule + a clean
//! document) and the parse-layer P-code regression contract: diagnostic
//! COUNT and SEVERITY on the 13 pre-existing fixtures must be byte-identical
//! to the pre-lint baseline — only the `code` values were remapped
//! (E001/E002 → P002, W001 → P001, W002 → P003).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use surf_parse::Severity;

const LINT_FIXTURE_DIR: &str = "tests/fixtures/lint";
const PARSE_FIXTURE_DIR: &str = "tests/fixtures";

fn read_fixture(dir: &str, name: &str) -> String {
    let path = Path::new(dir).join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn surf_files(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {dir}: {e}"))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "surf"))
        .collect();
    files.sort();
    files
}

/// L-codes fired by `check()` on the given source, in diagnostic order.
fn l_codes(source: &str) -> Vec<String> {
    surf_parse::check(source)
        .diagnostics
        .iter()
        .filter_map(|d| d.code.clone())
        .filter(|c| c.starts_with('L'))
        .collect()
}

// ------------------------------------------------------------------
// Lint fixture corpus
// ------------------------------------------------------------------

/// Expected L-codes per lint fixture. Every file in `tests/fixtures/lint/`
/// must have an entry here (asserted below).
const LINT_CORPUS: &[(&str, &[&str])] = &[
    ("clean.surf", &[]),
    ("l001-section.surf", &["L001"]),
    ("l002-table.surf", &["L002"]),
    ("l003-curly-attrs.surf", &["L003"]),
    ("l004-same-line.surf", &["L004"]),
    ("l005-nesting.surf", &["L005"]),
    ("l010-missing-summary.surf", &["L010"]),
    ("l011-summary-late.surf", &["L011"]),
    ("l020-unknown-block.surf", &["L020"]),
    ("l030-derivable-updated.surf", &["L030"]),
    ("l030-missing-fields.surf", &["L030", "L030"]),
    ("l031-enum-case.surf", &["L031"]),
    ("l041-unknown-layout.surf", &["L041", "L041"]),
    ("l042-desktop-only.surf", &["L042"]),
    ("l043-duplicate-block-id.surf", &["L043"]),
    ("registered-blocks.surf", &[]),
    ("p001-unclosed.surf", &[]),
    ("p002-unclosed-frontmatter.surf", &[]),
];

#[test]
fn lint_corpus_fires_exact_rule_ids() {
    for (file, expected) in LINT_CORPUS {
        let source = read_fixture(LINT_FIXTURE_DIR, file);
        let fired = l_codes(&source);
        assert_eq!(
            fired, *expected,
            "{file}: expected L-codes {expected:?}, got {fired:?}"
        );
    }
}

#[test]
fn lint_corpus_covers_every_fixture_file() {
    // `.fixed.surf` companions are expected outputs of the fix engine, not
    // lint inputs — they are covered by FIX_CORPUS below.
    let on_disk: BTreeSet<String> = surf_files(LINT_FIXTURE_DIR)
        .iter()
        .map(|p| {
            p.file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| !name.ends_with(".fixed.surf"))
        .collect();
    let in_corpus: BTreeSet<String> = LINT_CORPUS.iter().map(|(f, _)| f.to_string()).collect();
    assert_eq!(
        on_disk, in_corpus,
        "tests/fixtures/lint/ and LINT_CORPUS must list the same files"
    );
}

#[test]
fn clean_fixture_has_zero_diagnostics_in_all_layers() {
    let source = read_fixture(LINT_FIXTURE_DIR, "clean.surf");
    let report = surf_parse::check(&source);
    assert!(
        report.diagnostics.is_empty(),
        "clean.surf must be fully clean, got: {:?}",
        report.diagnostics
    );
    assert_eq!(report.error_count, 0);
    assert_eq!(report.warning_count, 0);
    assert_eq!(report.info_count, 0);
    assert_eq!(report.fixable_count, 0);
}

#[test]
fn l020_suggestion_comes_from_blocks_toml() {
    let source = read_fixture(LINT_FIXTURE_DIR, "l020-unknown-block.surf");
    let report = surf_parse::check(&source);
    let l020 = report
        .diagnostics
        .iter()
        .find(|d| d.code.as_deref() == Some("L020"))
        .expect("L020 fires");
    assert!(
        l020.message.contains("did you mean '::callout'?"),
        "did-you-mean should suggest the spec block name: {}",
        l020.message
    );
}

#[test]
fn registered_0_18_1_directives_never_fire_l020() {
    // 0.18.1 closed the registry drift: these five directives were
    // parser-implemented long before `spec/blocks.toml` listed them, so L020
    // reported every one of them as an unknown block.
    let source = read_fixture(LINT_FIXTURE_DIR, "registered-blocks.surf");
    for name in ["banner", "booking", "store", "cite", "bibliography"] {
        assert!(
            surf_parse::lint::known_block_names().contains(name),
            "::{name} must be a registered block name"
        );
    }
    // The parser-only aliases resolve to the same renderers and must also
    // stay quiet, without earning a second registry row.
    let aliases = "::reference-def[key=k]\ntitle: T\n::\n\n::references\n::\n";
    for src in [source.as_str(), aliases] {
        let report = surf_parse::check(src);
        let l020: Vec<&str> = report
            .diagnostics
            .iter()
            .filter_map(|d| d.code.as_deref())
            .filter(|c| *c == "L020")
            .collect();
        assert!(l020.is_empty(), "unexpected L020: {l020:?}");
    }
}

// ------------------------------------------------------------------
// Parse-layer regression: P-codes must not change count/severity
// ------------------------------------------------------------------

/// Baseline recorded 2026-06-06 on the pre-change tree (surf-parse 0.7.1,
/// commit a91bbe0): per-fixture parse-diagnostic severities. The P-code remap
/// may only change `code` values — never count or severity.
const PARSE_BASELINE: &[(&str, &[Severity])] = &[
    ("app-spec.surf", &[]),
    (
        "basic.surf",
        &[Severity::Warning, Severity::Warning, Severity::Warning],
    ),
    // 0.19.2: the `::data` preview-contract fixture (30 body rows + a
    // `total:` summary) — a well-formed document, so it parses clean.
    ("data-preview.surf", &[]),
    ("fragment-site.surf", &[]),
    // 0.11 nesting fix: `::section` containing a balanced `::callout` is a
    // real container (surplus-closer rule), so the file parses clean — the
    // former P006 orphan-closer warning was the flat-emission bug.
    ("gallery-form.surf", &[]),
    ("literate.surf", &[]),
    ("malformed.surf", &[Severity::Error]),
    // 0.11 nesting fix: the trailing `::` at EOF is `::app`'s own closer
    // (surplus-closer rule) — the manifest's build/deploy/decision blocks are
    // App children, and the former P006 "stray closer" warning is gone.
    ("manifest.surf", &[]),
    ("marketplace-spec.surf", &[]),
    ("nesting.surf", &[]),
    ("plan-app.surf", &[]),
    ("single.surf", &[]),
    ("site.surf", &[]),
    ("strategy-sample.surf", &[]),
];

#[test]
fn parse_fixture_diagnostics_match_pre_lint_baseline() {
    let files = surf_files(PARSE_FIXTURE_DIR);
    assert_eq!(
        files.len(),
        PARSE_BASELINE.len(),
        "fixture set changed — update PARSE_BASELINE deliberately"
    );
    for path in files {
        let name = path
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .into_owned();
        let (_, expected) = PARSE_BASELINE
            .iter()
            .find(|(f, _)| *f == name)
            .unwrap_or_else(|| panic!("no baseline entry for fixture {name}"));
        let source = fs::read_to_string(&path).expect("read fixture");
        let result = surf_parse::parse(&source);
        let severities: Vec<Severity> = result.diagnostics.iter().map(|d| d.severity).collect();
        assert_eq!(
            severities, *expected,
            "{name}: parse diagnostic count/severity regressed"
        );
        for d in &result.diagnostics {
            let code = d.code.as_deref().unwrap_or("");
            assert!(
                code.starts_with('P'),
                "{name}: parse-layer diagnostic must carry a P-code, got {code:?}"
            );
        }
    }
}

#[test]
fn parse_p_code_values_are_mapped_correctly() {
    // basic.surf: 3× unclosed-block warnings → P001 (was W001).
    let source = read_fixture(PARSE_FIXTURE_DIR, "basic.surf");
    let codes: Vec<String> = surf_parse::parse(&source)
        .diagnostics
        .iter()
        .filter_map(|d| d.code.clone())
        .collect();
    assert_eq!(codes, vec!["P001", "P001", "P001"]);

    // malformed.surf: unclosed front matter error → P002 (was E001).
    let source = read_fixture(PARSE_FIXTURE_DIR, "malformed.surf");
    let codes: Vec<String> = surf_parse::parse(&source)
        .diagnostics
        .iter()
        .filter_map(|d| d.code.clone())
        .collect();
    assert_eq!(codes, vec!["P002"]);
}

// ------------------------------------------------------------------
// Fix engine — fixture companions (chunk 2)
// ------------------------------------------------------------------

use surf_parse::error::FixSafety;

/// Every fixable-rule fixture, its expected-fixed companion, the safety tier
/// needed to repair it, and the rule code that must be gone after fixing.
const FIX_CORPUS: &[(&str, FixSafety, &str)] = &[
    ("p001-unclosed", FixSafety::Safe, "P001"),
    ("p002-unclosed-frontmatter", FixSafety::Safe, "P002"),
    ("l001-section", FixSafety::Safe, "L001"),
    ("l002-table", FixSafety::Safe, "L002"),
    ("l003-curly-attrs", FixSafety::Safe, "L003"),
    ("l004-same-line", FixSafety::Safe, "L004"),
    ("l005-nesting", FixSafety::Safe, "L005"),
    ("l030-derivable-updated", FixSafety::Safe, "L030"),
    ("l031-enum-case", FixSafety::Safe, "L031"),
    ("l042-desktop-only", FixSafety::Safe, "L042"),
    ("l010-missing-summary", FixSafety::Suggested, "L010"),
    ("l020-unknown-block", FixSafety::Suggested, "L020"),
];

#[test]
fn fix_corpus_matches_expected_fixed_companions_exactly() {
    for (name, tier, rule) in FIX_CORPUS {
        let input = read_fixture(LINT_FIXTURE_DIR, &format!("{name}.surf"));
        let expected = read_fixture(LINT_FIXTURE_DIR, &format!("{name}.fixed.surf"));
        let outcome = surf_parse::apply_fixes(&input, *tier);
        assert_eq!(
            outcome.source, expected,
            "{name}: apply_fixes output must match {name}.fixed.surf byte-for-byte"
        );
        assert!(
            outcome.applied.iter().any(|a| a.code == *rule),
            "{name}: expected an applied {rule} fix, got {:?}",
            outcome.applied
        );
        // Re-check: the fixed rule no longer fires on the fixed output.
        let recheck = surf_parse::check(&outcome.source);
        assert!(
            recheck
                .diagnostics
                .iter()
                .all(|d| d.code.as_deref() != Some(rule)),
            "{name}: {rule} still fires on the fixed output: {:?}",
            recheck.diagnostics
        );
    }
}

#[test]
fn fix_corpus_covers_every_fixed_companion_on_disk() {
    let on_disk: BTreeSet<String> = surf_files(LINT_FIXTURE_DIR)
        .iter()
        .filter_map(|p| {
            p.file_name()
                .expect("file name")
                .to_string_lossy()
                .strip_suffix(".fixed.surf")
                .map(str::to_owned)
        })
        .collect();
    let in_corpus: BTreeSet<String> = FIX_CORPUS.iter().map(|(f, _, _)| f.to_string()).collect();
    assert_eq!(
        on_disk, in_corpus,
        "every .fixed.surf companion must have a FIX_CORPUS entry and vice versa"
    );
}

#[test]
fn fix_corpus_is_idempotent_and_safe_tier_is_a_subset() {
    for (name, tier, _) in FIX_CORPUS {
        let fixed = read_fixture(LINT_FIXTURE_DIR, &format!("{name}.fixed.surf"));
        let again = surf_parse::apply_fixes(&fixed, *tier);
        assert_eq!(
            again.source, fixed,
            "{name}: fixing the fixed output must be a no-op"
        );
        assert_eq!(again.iterations, 0, "{name}: fixed output needs no passes");
    }
    // Suggested-tier fixtures must be untouched by a Safe-tier run.
    for (name, tier, _) in FIX_CORPUS {
        if *tier == FixSafety::Suggested {
            let input = read_fixture(LINT_FIXTURE_DIR, &format!("{name}.surf"));
            let safe_only = surf_parse::apply_fixes(&input, FixSafety::Safe);
            assert_eq!(
                safe_only.source, input,
                "{name}: Safe tier must not apply suggested fixes"
            );
        }
    }
}

#[test]
fn malformed_fixture_auto_repairs_to_zero_p_code_errors() {
    let input = read_fixture(PARSE_FIXTURE_DIR, "malformed.surf");
    let before = surf_parse::check(&input);
    assert!(
        before
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("P002")),
        "precondition: malformed.surf carries a P002 error"
    );

    let outcome = surf_parse::apply_fixes(&input, FixSafety::Safe);
    assert!(
        outcome.iterations < surf_parse::lint::MAX_FIX_ITERATIONS,
        "repair must converge, not hit the iteration cap"
    );
    let after = surf_parse::check(&outcome.source);
    let p_codes: Vec<&surf_parse::Diagnostic> = after
        .diagnostics
        .iter()
        .filter(|d| d.code.as_deref().is_some_and(|c| c.starts_with('P')))
        .collect();
    assert!(
        p_codes.is_empty(),
        "malformed.surf must repair to zero P-code diagnostics, got {p_codes:?}"
    );
}

#[test]
fn attached_fixes_match_registry_fixability_and_safety() {
    // Drift check: a diagnostic may carry a fix only when its rule is
    // registered fixable, and the fix's tier must match the registry's
    // fix_safety. Run across every lint fixture + parse fixture.
    let registry = surf_parse::lint::rule_registry();
    let mut fixture_files = surf_files(LINT_FIXTURE_DIR);
    fixture_files.extend(surf_files(PARSE_FIXTURE_DIR));
    for path in fixture_files {
        let source = fs::read_to_string(&path).expect("read fixture");
        for d in surf_parse::check(&source).diagnostics {
            let Some(fix) = &d.fix else { continue };
            let code = d.code.as_deref().unwrap_or("");
            let Some(meta) = registry.get(code) else {
                // V-codes are not in the registry and never carry fixes.
                panic!(
                    "{}: fix attached to unregistered rule {code}",
                    path.display()
                );
            };
            assert!(
                meta.fixable,
                "{}: rule {code} carries a fix but is not fixable in rules.toml",
                path.display()
            );
            let expected = match meta.fix_safety.as_deref() {
                Some("suggested") => FixSafety::Suggested,
                _ => FixSafety::Safe,
            };
            assert_eq!(
                fix.safety,
                expected,
                "{}: rule {code} fix safety must match rules.toml",
                path.display()
            );
        }
    }
}

#[test]
fn fixable_count_is_real_on_fixtures() {
    let source = read_fixture(LINT_FIXTURE_DIR, "l003-curly-attrs.surf");
    let report = surf_parse::check(&source);
    assert_eq!(
        report.fixable_count, 1,
        "L003 fixture carries exactly one fix"
    );
    assert_eq!(
        report.fixable_count,
        report
            .diagnostics
            .iter()
            .filter(|d| d.fix.is_some())
            .count()
    );
}

#[test]
fn contract_type_and_ratified_status_parse_with_front_matter_intact() {
    // 0.18: `type: contract` / `status: ratified` are the header values the
    // clients/spec contract documents carry (LAYOUT-CONTRACT.surf). Before
    // 0.18 serde rejected both, which discarded the ENTIRE front matter and
    // cascaded V001/V002 from that one cause.
    let source = "---\ntitle: \"Surfspace spec layout contract\"\ntype: contract\nversion: 1\ncreated: 2026-08-24\nstatus: ratified\n---\n\n::summary\nThe frozen layout contract.\n::\n\nBody.\n";
    let doc = surf_parse::parse(source);
    assert_eq!(
        doc.doc.front_matter.as_ref().and_then(|f| f.title.as_deref()),
        Some("Surfspace spec layout contract"),
        "front matter must survive a contract/ratified header"
    );
    let codes: Vec<_> = doc
        .diagnostics
        .iter()
        .filter_map(|d| d.code.as_deref())
        .collect();
    assert!(
        !codes.contains(&"P005") && !codes.contains(&"P002"),
        "contract/ratified must be in-schema, got {codes:?}"
    );
}

#[test]
fn specification_type_lints_clean_with_front_matter_intact() {
    // 0.19.1: `type: specification` is the header the SurfContext standard
    // documents carry (SPEC.surf). Additive vocabulary entry — front matter
    // must survive intact and no schema diagnostic may fire.
    let source = "---\ntitle: \"SurfContext specification\"\ntype: specification\nversion: 1\ncreated: 2026-08-26\nstatus: ratified\n---\n\n::summary\nThe normative standard.\n::\n\nBody.\n";
    let doc = surf_parse::parse(source);
    assert_eq!(
        doc.doc.front_matter.as_ref().and_then(|f| f.title.as_deref()),
        Some("SurfContext specification"),
        "front matter must survive a specification header"
    );
    assert_eq!(
        doc.doc.front_matter.as_ref().and_then(|f| f.doc_type),
        Some(surf_parse::DocType::Specification)
    );
    let parse_codes: Vec<_> = doc
        .diagnostics
        .iter()
        .filter_map(|d| d.code.as_deref())
        .collect();
    assert!(
        !parse_codes.contains(&"P005") && !parse_codes.contains(&"P002"),
        "specification must be in-schema, got {parse_codes:?}"
    );
    // And the lint engine agrees: no enum-vocabulary L-code on the header.
    let lint_codes = l_codes(source);
    assert!(
        !lint_codes.iter().any(|c| c == "L031"),
        "specification must not read as a mis-cased enum value, got {lint_codes:?}"
    );
}

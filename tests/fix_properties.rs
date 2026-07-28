//! Property-based tests for the fix engine (`surf_parse::apply_fixes`).
//!
//! The four guarantees from the surf-lint plan's fix model:
//!
//! 1. **Idempotent** — `apply_fixes(apply_fixes(x)) == apply_fixes(x)`.
//! 2. **Converging** — after a converged Safe run, no diagnostic on the fixed
//!    output still carries a Safe fix (every applied fix's diagnostic is gone
//!    on re-check; new fixable diagnostics would have been fixed in a later
//!    pass).
//! 3. **Non-destructive** — lines outside the fixed spans are byte-identical.
//! 4. **Never panics** — arbitrary input never crashes the engine.

use proptest::prelude::*;

use surf_parse::error::FixSafety;
use surf_parse::lint::MAX_FIX_ITERATIONS;

// ------------------------------------------------------------------
// Generators: fixture-style documents with injected defects
// ------------------------------------------------------------------

/// A defect segment that one of the fixable rules detects, plus clean filler.
/// Mirrors the lint fixture corpus so generated inputs stay realistic.
fn arb_segment() -> impl Strategy<Value = String> {
    prop_oneof![
        // Clean filler.
        Just("Plain paragraph text.\n".to_string()),
        Just("## Heading\n\nMore text here.\n".to_string()),
        Just("::callout[type=info]\nAll good.\n::\n".to_string()),
        Just("| a | b |\n|---|---|\n| 1 | 2 |\n".to_string()),
        // L001 — ::section instead of a heading.
        "[A-Za-z ]{1,12}".prop_map(|t| format!("::section[title=\"{t}\"]\nSection body.\n::\n")),
        // L002 — ::table wrapper.
        Just("::table\n| x | y |\n|---|---|\n| 1 | 2 |\n::\n".to_string()),
        // L003 — curly attribute braces.
        Just("::callout{type=tip}\nCurly.\n::\n".to_string()),
        // L004 — opener with trailing content.
        "[A-Za-z !]{1,16}".prop_map(|t| format!("::callout[type=note] {t}\n::\n")),
        // L005 — over-deep opener.
        Just(":::column\nToo deep.\n:::\n".to_string()),
        // L005 — orphan closer.
        Just("::callout\nHi\n::\n::\n".to_string()),
        // L020 — unknown block (unique did-you-mean: suggested tier).
        Just("::callouts[type=info]\nTypo.\n::\n".to_string()),
        // Multi-byte content around defects.
        Just("# Café ☕ 日本語 🚀\n\n::callout{type=info}\n東京 emoji 🎉 body.\n::\n".to_string()),
    ]
}

/// Optional front matter: clean, unclosed (P002), or case-broken (L031).
fn arb_front_matter() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("---\ntitle: T\ntype: report\nstatus: active\n---\n\n".to_string()),
        Just("---\ntitle: T\ntype: report\nstatus: Active\n---\n\n".to_string()),
        Just("---\ntitle: T\ntype: report\nstatus: active\n\nBody starts here.\n".to_string()),
        Just(
            "---\ntitle: \"日本語 🚀\"\ntype: guide\nconfidence: high\ncreated: \"2026-06-01\"\n---\n\n"
                .to_string()
        ),
    ]
}

/// Optional trailing defect: an unclosed block at EOF (P001).
fn arb_tail() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just("::callout[type=warning]\nNever closed.\n".to_string()),
        Just("::columns\n:::column\nNested, never closed.\n".to_string()),
    ]
}

/// A fixture-mutated document: front matter + defect/filler segments + tail.
fn arb_document() -> impl Strategy<Value = String> {
    (
        arb_front_matter(),
        proptest::collection::vec(arb_segment(), 0..6),
        arb_tail(),
    )
        .prop_map(|(fm, segments, tail)| format!("{fm}{}{tail}", segments.join("\n")))
}

// ------------------------------------------------------------------
// Property 1: idempotency
// ------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// apply_fixes(apply_fixes(x)) == apply_fixes(x), both tiers.
    #[test]
    fn apply_fixes_is_idempotent(doc in arb_document()) {
        for tier in [FixSafety::Safe, FixSafety::Suggested] {
            let once = surf_parse::apply_fixes(&doc, tier);
            // Idempotency is only promised at a reached fixpoint.
            prop_assume!(once.iterations < MAX_FIX_ITERATIONS);
            let twice = surf_parse::apply_fixes(&once.source, tier);
            prop_assert_eq!(
                &twice.source,
                &once.source,
                "second run must be a no-op (tier {:?})",
                tier
            );
            prop_assert_eq!(twice.iterations, 0);
        }
    }

    // ------------------------------------------------------------------
    // Property 2: convergence
    // ------------------------------------------------------------------

    /// After a converged Safe run, re-check finds no diagnostic that still
    /// carries a Safe fix — i.e. every applied fix removed its diagnostic
    /// and no applicable fix is left behind.
    #[test]
    fn applied_safe_fixes_converge(doc in arb_document()) {
        let out = surf_parse::apply_fixes(&doc, FixSafety::Safe);
        prop_assume!(out.iterations < MAX_FIX_ITERATIONS);
        let recheck = surf_parse::check(&out.source);
        let remaining: Vec<_> = recheck
            .diagnostics
            .iter()
            .filter(|d| d.fix.as_ref().is_some_and(|f| f.safety == FixSafety::Safe))
            .collect();
        prop_assert!(
            remaining.is_empty(),
            "Safe-fixable diagnostics remain after convergence: {:?}",
            remaining
        );
    }

    // ------------------------------------------------------------------
    // Property 3: non-destructiveness
    // ------------------------------------------------------------------

    /// Lines outside the fixed region are byte-identical: with a single
    /// line-local defect between clean prefix/suffix lines, the prefix and
    /// suffix come through untouched.
    #[test]
    fn untouched_lines_are_byte_identical(
        prefix_n in 0usize..5,
        suffix_n in 0usize..5,
        defect_idx in 0usize..4,
    ) {
        // Line-local defects only (fix edits stay within the segment).
        let defects = [
            "::callout{type=tip}\nCurly.\n::",
            "::callout[type=note] Tail content!\n::",
            "::section[title=\"S ☕\"]\nBody 日本語.\n::",
            "::table\n| x |\n|---|\n| 1 |\n::",
        ];
        let prefix: Vec<String> =
            (0..prefix_n).map(|i| format!("Prefix line {i} — café ☕.")).collect();
        let suffix: Vec<String> =
            (0..suffix_n).map(|i| format!("Suffix line {i} — 東京.")).collect();
        let doc = format!(
            "{}\n\n{}\n\n{}\n",
            prefix.join("\n"),
            defects[defect_idx],
            suffix.join("\n")
        );

        let out = surf_parse::apply_fixes(&doc, FixSafety::Safe);
        prop_assert!(!out.applied.is_empty(), "defect must be fixed");
        let out_lines: Vec<&str> = out.source.lines().collect();
        for (i, line) in prefix.iter().enumerate() {
            prop_assert_eq!(
                out_lines.get(i).copied(),
                Some(line.as_str()),
                "prefix line {} must be untouched",
                i
            );
        }
        for (i, line) in suffix.iter().enumerate() {
            let from_end = suffix.len() - i;
            prop_assert_eq!(
                out_lines.get(out_lines.len() - from_end).copied(),
                Some(line.as_str()),
                "suffix line {} must be untouched",
                i
            );
        }
    }

    // ------------------------------------------------------------------
    // Property 4: never panics
    // ------------------------------------------------------------------

    /// Arbitrary printable strings never panic the fix engine.
    #[test]
    fn apply_fixes_never_panics(input in "\\PC{0,400}") {
        for tier in [FixSafety::Safe, FixSafety::Suggested] {
            let out = surf_parse::apply_fixes(&input, tier);
            let _ = surf_parse::check(&out.source);
        }
    }

    /// Arbitrary multi-line strings (printable lines joined by newlines,
    /// including directive-ish fragments) never panic the fix engine, and
    /// the iteration cap always holds.
    #[test]
    fn apply_fixes_never_panics_multiline(
        lines in proptest::collection::vec(
            prop_oneof![
                "\\PC{0,40}",
                Just("::".to_string()),
                Just(":::".to_string()),
                Just("---".to_string()),
                Just("::callout{".to_string()),
                Just("::section[title=\"x".to_string()),
            ],
            0..30,
        )
    ) {
        let input = lines.join("\n");
        let out = surf_parse::apply_fixes(&input, FixSafety::Suggested);
        prop_assert!(out.iterations <= MAX_FIX_ITERATIONS);
        let _ = surf_parse::apply_fixes_once(&input, FixSafety::Safe);
    }

    /// Fully arbitrary unicode strings (any chars, any length up to ~1 KiB)
    /// never panic.
    #[test]
    fn apply_fixes_never_panics_any_string(input in any::<String>()) {
        let _ = surf_parse::apply_fixes(&input, FixSafety::Suggested);
    }
}

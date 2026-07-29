//! Style lint rules (L-codes) and the unified `check` entry point.
//!
//! Three diagnostic layers feed [`check`]:
//!
//! 1. **syntax** — [`crate::parse::parse`] P-codes (parse-layer diagnostics)
//! 2. **schema** — [`crate::validate::validate`] V-codes (`SurfDoc::validate()`)
//! 3. **style** — the [`LintRule`] implementations in this module (L-codes)
//!
//! Rule metadata (default severity, fixability, message templates) is loaded
//! at compile time from `spec/rules.toml` via `include_str!`. The module is
//! wasm-clean: no filesystem access, no clock, no randomness. The
//! `spec_compliance` integration test asserts that the implemented rule set
//! and the registry match exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::attrs::parse_attrs;
use crate::error::{Diagnostic, Fix, FixSafety, Severity, TextEdit};
use crate::parse::{closing_directive_depth, directive_name_start, opening_directive};
use crate::types::{AttrValue, Block, DocType, FrontMatter, Span, SurfDoc};

// ------------------------------------------------------------------
// Rule registry (spec/rules.toml)
// ------------------------------------------------------------------

/// Metadata for a single lint rule, loaded from `spec/rules.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleMeta {
    /// Rule layer: `"syntax"` (P-codes) or `"style"` (L-codes).
    pub layer: String,
    /// Default severity; overridable per rule id via [`LintConfig`].
    pub severity: Severity,
    /// Whether Phase 2 ships a deterministic fix for this rule.
    pub fixable: bool,
    /// Fix tier: `"safe"` (auto-apply) or `"suggested"`; only set when fixable.
    #[serde(default)]
    pub fix_safety: Option<String>,
    /// Human-readable message template with `{placeholder}` slots.
    pub message: String,
    /// One-line rule description.
    pub description: String,
}

#[derive(Debug, Deserialize)]
struct RegistryMeta {
    #[allow(dead_code)]
    spec_version: String,
    total_rules: usize,
    #[allow(dead_code)]
    registry_updated: String,
}

#[derive(Debug, Deserialize)]
struct RegistryFile {
    meta: RegistryMeta,
    rules: BTreeMap<String, RuleMeta>,
}

static RULE_REGISTRY: LazyLock<RegistryFile> = LazyLock::new(|| {
    toml::from_str(include_str!("../spec/rules.toml"))
        .expect("spec/rules.toml is embedded at compile time and must parse as valid TOML — covered by spec_compliance tests")
});

/// The lint rule registry, keyed by rule id (`P001`, `L001`, …).
pub fn rule_registry() -> &'static BTreeMap<String, RuleMeta> {
    &RULE_REGISTRY.rules
}

/// `meta.total_rules` declared in `spec/rules.toml`.
pub fn rule_registry_total() -> usize {
    RULE_REGISTRY.meta.total_rules
}

/// Rule ids emitted by the parse layer (`src/parse.rs`), pinned for the
/// registry drift test in `tests/spec_compliance.rs`.
pub const PARSE_RULE_IDS: &[&str] = &["P001", "P002", "P003", "P005", "P006"];

/// Default severity for a rule id, falling back to `Warning` for ids that are
/// not in the registry (defensive; the drift test makes this unreachable).
fn registry_severity(id: &str) -> Severity {
    rule_registry()
        .get(id)
        .map_or(Severity::Warning, |m| m.severity)
}

/// Build a diagnostic for a rule id with its registry default severity.
fn diag(id: &str, message: String, span: Option<Span>) -> Diagnostic {
    diag_fix(id, message, span, None)
}

/// [`diag`] with an attached auto-fix.
fn diag_fix(id: &str, message: String, span: Option<Span>, fix: Option<Fix>) -> Diagnostic {
    Diagnostic {
        severity: registry_severity(id),
        message,
        span,
        code: Some(id.to_string()),
        fix,
    }
}

/// Build a [`Fix`] whose safety tier comes from the rule registry
/// (`fix_safety` in `spec/rules.toml`; defaults to `Safe`).
fn rule_fix(id: &str, description: String, edits: Vec<TextEdit>) -> Fix {
    let safety = match rule_registry()
        .get(id)
        .and_then(|m| m.fix_safety.as_deref())
    {
        Some("suggested") => FixSafety::Suggested,
        _ => FixSafety::Safe,
    };
    Fix {
        edits,
        safety,
        description,
    }
}

// ------------------------------------------------------------------
// Known block names (spec/blocks.toml)
// ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BlocksSpecFile {
    blocks: BTreeMap<String, toml::Value>,
}

static KNOWN_BLOCK_NAMES: LazyLock<BTreeSet<String>> = LazyLock::new(|| {
    let spec: BlocksSpecFile = toml::from_str(include_str!("../spec/blocks.toml"))
        .expect("spec/blocks.toml is embedded at compile time and must parse as valid TOML — covered by spec_compliance tests");
    spec.blocks.into_keys().collect()
});

/// All block names registered in `spec/blocks.toml` (any status).
///
/// L020 keys off this set — the spec, not the `Block` enum, is the law.
pub fn known_block_names() -> &'static BTreeSet<String> {
    &KNOWN_BLOCK_NAMES
}

/// Parser-accepted names that are deliberately NOT in `spec/blocks.toml`:
/// sub-directives (`column`) and aliases/renderer families the parser resolves
/// (`action-items` → tasks, `deck`/`slide`, `deploy_urls`, `info-card`).
/// L020 must not flag these. The spec_compliance test asserts none of them is
/// in blocks.toml — once the spec registers one, remove it from this list.
pub const EXTRA_KNOWN_BLOCK_NAMES: &[&str] = &[
    "action-items",
    "column",
    "deck",
    "deploy_urls",
    "info-card",
    "slide",
];

// ------------------------------------------------------------------
// LintRule trait + registry of implementations
// ------------------------------------------------------------------

/// A single style-layer lint rule.
pub trait LintRule {
    /// Stable rule id, e.g. `"L001"`. Must have an entry in `spec/rules.toml`.
    fn id(&self) -> &'static str;
    /// Check a parsed document against this rule.
    ///
    /// `source` is the CRLF-normalised source text the spans refer to
    /// (i.e. `doc.source`).
    fn check(&self, doc: &SurfDoc, source: &str) -> Vec<Diagnostic>;
}

/// All implemented style-layer rules, in rule-id order.
pub fn all_rules() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(SectionBlockUsed),
        Box::new(TableBlockUsed),
        Box::new(CurlyAttrBraces),
        Box::new(DirectiveNotOnOwnLine),
        Box::new(NestingColonMismatch),
        Box::new(MissingSummary),
        Box::new(SummaryNotNearTop),
        Box::new(UnknownBlockName),
        Box::new(MissingRequiredFrontMatter),
        Box::new(FrontMatterEnumCase),
        Box::new(MermaidConstructSkipped),
    ]
}

// ------------------------------------------------------------------
// Source-line scanner shared by the source-scan rules
// ------------------------------------------------------------------

/// A classified source line.
struct ScanLine<'a> {
    /// 0-based line index.
    idx: usize,
    /// Byte offset of the line start within the source.
    offset: usize,
    /// Raw line text (without trailing newline).
    raw: &'a str,
    /// `raw.trim()`, precomputed.
    trimmed: &'a str,
    /// Inside front matter, a `::code`/`::output` block body, or a fenced
    /// markdown code block — source-scan rules must skip these lines.
    literal: bool,
}

impl ScanLine<'_> {
    fn span(&self) -> Span {
        Span {
            start_line: self.idx + 1,
            end_line: self.idx + 1,
            start_offset: self.offset,
            end_offset: self.offset + self.raw.len(),
        }
    }

    /// Byte length of the leading whitespace on this line.
    fn indent(&self) -> usize {
        self.raw.len() - self.raw.trim_start().len()
    }

    /// A sub-line span: `start..end` are byte offsets WITHIN this line's raw
    /// text (relative to the line start).
    fn sub_span(&self, start: usize, end: usize) -> Span {
        Span {
            start_line: self.idx + 1,
            end_line: self.idx + 1,
            start_offset: self.offset + start,
            end_offset: self.offset + end,
        }
    }

    /// Edit replacing this line's content (newline untouched).
    fn replace_edit(&self, replacement: impl Into<String>) -> TextEdit {
        TextEdit {
            span: self.span(),
            replacement: replacement.into(),
        }
    }

    /// Edit deleting this whole line including its trailing newline (if any).
    fn delete_edit(&self, source_len: usize) -> TextEdit {
        let end = self.offset + self.raw.len();
        let end = if end < source_len { end + 1 } else { end };
        TextEdit {
            span: Span {
                start_line: self.idx + 1,
                end_line: self.idx + 1,
                start_offset: self.offset,
                end_offset: end,
            },
            replacement: String::new(),
        }
    }
}

/// 0-based index of the closing `---` line, if the document opens with front
/// matter that is properly closed. Mirrors the parser: an unclosed `---` means
/// the whole document is body text.
fn front_matter_close_line(lines: &[&str]) -> Option<usize> {
    if lines.first().map(|l| l.trim()) != Some("---") {
        return None;
    }
    lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim() == "---")
        .map(|(i, _)| i)
}

/// Block names whose body content is literal text (directive-looking lines
/// inside them must not trigger source-scan rules).
const LITERAL_BLOCK_NAMES: &[&str] = &["code", "output"];

/// Classify every source line for the source-scan rules.
fn scan_lines(source: &str) -> Vec<ScanLine<'_>> {
    let lines: Vec<&str> = source.split('\n').collect();
    let fm_end = front_matter_close_line(&lines);
    let mut out = Vec::with_capacity(lines.len());
    let mut offset = 0usize;
    let mut in_fence = false;
    // Depth of the innermost open literal block (::code / ::output), if any.
    let mut literal_depth: Option<usize> = None;

    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        let in_front_matter = fm_end.is_some_and(|end| idx <= end);
        let mut literal = in_front_matter;

        if !in_front_matter {
            if let Some(depth) = literal_depth {
                if closing_directive_depth(trimmed) == Some(depth) {
                    // The closer line itself is structural.
                    literal_depth = None;
                } else {
                    literal = true;
                }
            } else if in_fence {
                literal = true;
                if trimmed.starts_with("```") {
                    in_fence = false;
                }
            } else if trimmed.starts_with("```") {
                in_fence = true;
                literal = true;
            } else if let Some((depth, name, _)) = opening_directive(trimmed)
                && LITERAL_BLOCK_NAMES.contains(&name.as_str())
            {
                literal_depth = Some(depth);
            }
        }

        out.push(ScanLine {
            idx,
            offset,
            raw,
            trimmed,
            literal,
        });
        offset += raw.len() + 1;
    }
    out
}

/// Index of the closer line matching the opener at `scan[opener_idx]` (with
/// `depth` colons), if any. Mirrors the parser's depth matching: a closer
/// closes the innermost open block with the same colon count.
///
/// Limit: closer-less leaf directives between the opener and its closer can
/// steal the closer via depth matching (same divergence the parser resolves
/// with sibling lookahead). The fixpoint loop in [`apply_fixes`] cleans up
/// any leftover colon-only line via L005 on the next pass.
fn matching_closer_idx(scan: &[ScanLine<'_>], opener_idx: usize, depth: usize) -> Option<usize> {
    let mut stack = vec![depth];
    for (i, line) in scan.iter().enumerate().skip(opener_idx + 1) {
        if line.literal {
            continue;
        }
        if let Some(d) = closing_directive_depth(line.trimmed) {
            if let Some(pos) = stack.iter().rposition(|&s| s == d) {
                stack.truncate(pos);
                if stack.is_empty() {
                    return Some(i);
                }
            }
            continue;
        }
        if let Some((d, _, _)) = opening_directive(line.trimmed) {
            stack.push(d);
        }
    }
    None
}

// ------------------------------------------------------------------
// L001 / L002 — forbidden blocks with markdown equivalents
// ------------------------------------------------------------------

/// Heading text for a `::section` opener: the `title` (or `headline`) attr,
/// with the heading level from a `level` attr clamped to 1..=6 (default 2).
fn section_heading(attrs_str: &str) -> Option<String> {
    let attrs = parse_attrs(attrs_str).ok()?;
    let title = match attrs.get("title").or_else(|| attrs.get("headline"))? {
        AttrValue::String(s) => s.clone(),
        AttrValue::Number(n) => n.to_string(),
        _ => return None,
    };
    if title.trim().is_empty() {
        return None;
    }
    let level = match attrs.get("level") {
        Some(AttrValue::Number(n)) if n.fract() == 0.0 && (1.0..=6.0).contains(n) => *n as usize,
        _ => 2,
    };
    Some(format!("{} {}", "#".repeat(level), title))
}

/// L001: `::section` block used — prefer a markdown `##` heading.
///
/// Fix (safe): replace the opener with `## <title>` (from the `title` /
/// `headline` attr; `level` attr sets the heading depth) and delete the closer
/// line. Same-line trailing content composes too (L004 defers section openers
/// here): `::section Executive Summary` becomes `## Executive Summary`, and a
/// displaced `::section [title=X]` bracket block is read as attributes.
/// Without any usable title, opener and closer are simply stripped.
struct SectionBlockUsed;

/// Heading for a `::section` opener, considering both the attached
/// `[attrs]` and any same-line trailing content. Trailing content that is
/// itself a `[...]` block is parsed as displaced attributes; a `{...}` tail
/// is left for L003's brace rewrite; other text is the literal title.
fn section_heading_with_tail(attrs_str: &str, tail: &str) -> Option<String> {
    if !attrs_str.is_empty() {
        return match (section_heading(attrs_str), tail.is_empty()) {
            (Some(heading), true) => Some(heading),
            // Attrs gave the heading; trailing content survives below it.
            (Some(heading), false) => Some(format!("{heading}\n{tail}")),
            (None, true) => None,
            // No usable title attr — the trailing text is the title.
            (None, false) => Some(format!("## {tail}")),
        };
    }
    if tail.starts_with('[') && crate::parse::find_attr_close(tail) == Some(tail.len() - 1) {
        return section_heading(tail);
    }
    if tail.is_empty() || tail.starts_with('{') {
        return None;
    }
    Some(format!("## {tail}"))
}

impl LintRule for SectionBlockUsed {
    fn id(&self) -> &'static str {
        "L001"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let scan = scan_lines(source);
        let mut out = Vec::new();
        for (i, line) in scan.iter().enumerate() {
            if line.literal {
                continue;
            }
            let Some((depth, name, attrs_str)) = opening_directive(line.trimmed) else {
                continue;
            };
            if name != "section" {
                continue;
            }
            let tail = line.trimmed
                [directive_name_start(line.trimmed, depth) + name.len() + attrs_str.len()..]
                .trim();
            let mut edits = Vec::new();
            let description = match section_heading_with_tail(&attrs_str, tail) {
                Some(heading) => {
                    let d = format!(
                        "replace '::section' with '{}'",
                        heading.split('\n').next().unwrap_or(&heading)
                    );
                    edits.push(line.replace_edit(heading));
                    d
                }
                None => {
                    edits.push(line.delete_edit(source.len()));
                    "remove '::section' wrapper without a title".to_string()
                }
            };
            if let Some(ci) = matching_closer_idx(&scan, i, depth) {
                edits.push(scan[ci].delete_edit(source.len()));
            }
            out.push(diag_fix(
                "L001",
                "'::section' block used — prefer a markdown '##' heading".to_string(),
                Some(line.span()),
                Some(rule_fix("L001", description, edits)),
            ));
        }
        out
    }
}

/// L002: `::table` block used — prefer a markdown pipe table.
///
/// Fix (safe): strip the opener and closer lines, keeping the content.
struct TableBlockUsed;

impl LintRule for TableBlockUsed {
    fn id(&self) -> &'static str {
        "L002"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let scan = scan_lines(source);
        let mut out = Vec::new();
        for (i, line) in scan.iter().enumerate() {
            if line.literal {
                continue;
            }
            let Some((depth, name, _)) = opening_directive(line.trimmed) else {
                continue;
            };
            if name != "table" {
                continue;
            }
            let mut edits = vec![line.delete_edit(source.len())];
            if let Some(ci) = matching_closer_idx(&scan, i, depth) {
                edits.push(scan[ci].delete_edit(source.len()));
            }
            out.push(diag_fix(
                "L002",
                "'::table' block used — prefer a markdown pipe table".to_string(),
                Some(line.span()),
                Some(rule_fix(
                    "L002",
                    "remove the '::table' wrapper, keeping the table content".to_string(),
                    edits,
                )),
            ));
        }
        out
    }
}

// ------------------------------------------------------------------
// L003 — {curly} attribute braces
// ------------------------------------------------------------------

/// Byte index of the `}` closing the brace block opened by the leading `{`
/// of `s`. Quote-aware (honours `\"` escapes) and depth-aware, mirroring
/// `parse::find_attr_close` for brackets.
fn find_brace_close(s: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut in_quote = false;
    let mut escaped = false;
    for (i, b) in s.bytes().enumerate() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_quote = false;
            }
            continue;
        }
        match b {
            b'"' => in_quote = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// L003: `{curly}` attribute braces on a block opener — use `[square]`.
///
/// Fix (safe): rewrite the opening `{` and its matching `}` to `[` / `]`,
/// preserving the inner text byte-for-byte. No fix when the `{` is unmatched.
struct CurlyAttrBraces;

impl LintRule for CurlyAttrBraces {
    fn id(&self) -> &'static str {
        "L003"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for line in scan_lines(source) {
            if line.literal {
                continue;
            }
            if let Some((depth, name, _)) = opening_directive(line.trimmed) {
                let name_at = directive_name_start(line.trimmed, depth);
                let rest = &line.trimmed[name_at + name.len()..];
                if rest.starts_with('{') {
                    // Brace positions relative to the raw line.
                    let open = line.indent() + name_at + name.len();
                    let fix = find_brace_close(rest).map(|close_rel| {
                        let close = open + close_rel;
                        rule_fix(
                            "L003",
                            "rewrite {curly} attribute braces to [square] brackets".to_string(),
                            vec![
                                TextEdit {
                                    span: line.sub_span(open, open + 1),
                                    replacement: "[".to_string(),
                                },
                                TextEdit {
                                    span: line.sub_span(close, close + 1),
                                    replacement: "]".to_string(),
                                },
                            ],
                        )
                    });
                    out.push(diag_fix(
                        "L003",
                        format!(
                            "Block attributes use {{curly}} braces — use [square] brackets: ::{name}[...]"
                        ),
                        Some(line.span()),
                        fix,
                    ));
                }
            }
        }
        out
    }
}

// ------------------------------------------------------------------
// L004 — directive not on its own line
// ------------------------------------------------------------------

/// L004: block opener or closer carries content on the same line.
///
/// Fix (safe): replace the whitespace gap between the directive and the
/// trailing content with a newline, moving the content to its own line.
///
/// Limits: a closer at the END of a content line (`text ::`) is not
/// detected — only lines that START with a directive are classified.
struct DirectiveNotOnOwnLine;

/// Edit replacing the whitespace run starting at byte `from` (relative to the
/// raw line) with a newline, so the rest of the line moves to its own line.
fn break_line_edit(line: &ScanLine<'_>, from: usize) -> TextEdit {
    let rest = &line.raw[from..];
    let ws = rest.len() - rest.trim_start().len();
    TextEdit {
        span: line.sub_span(from, from + ws),
        replacement: "\n".to_string(),
    }
}

/// L004 fix for an opener with trailing content.
///
/// Author intent matters: `::callout [type=info]` is a displaced attribute
/// block (space before the bracket), so the fix REATTACHES it —
/// `::callout[type=info]` — instead of demoting it to literal body text.
/// Anything after a reattached `[...]` block, and every non-bracket tail,
/// still moves to its own line.
///
/// `tail_at` / `tail` are relative to `line.trimmed` (tail starts right after
/// the directive's own `[attrs]`, if any).
fn l004_opener_fix(
    line: &ScanLine<'_>,
    name: &str,
    attrs_str: &str,
    tail_at: usize,
    tail: &str,
) -> Fix {
    let gap_start = line.indent() + tail_at;
    let ws = tail.len() - tail.trim_start().len();
    let bracket = tail.trim_start();
    if attrs_str.is_empty()
        && ws > 0
        && bracket.starts_with('[')
        && let Some(close) = crate::parse::find_attr_close(bracket)
    {
        // Delete the whitespace gap so the bracket attaches to the name.
        let mut edits = vec![TextEdit {
            span: line.sub_span(gap_start, gap_start + ws),
            replacement: String::new(),
        }];
        // Content after the bracket block still moves to its own line.
        if !bracket[close + 1..].trim().is_empty() {
            edits.push(break_line_edit(line, gap_start + ws + close + 1));
        }
        return rule_fix(
            "L004",
            format!("attach the '[...]' attributes to '::{name}'"),
            edits,
        );
    }
    rule_fix(
        "L004",
        format!("move the content after '::{name}' to its own line"),
        vec![break_line_edit(line, gap_start)],
    )
}

impl LintRule for DirectiveNotOnOwnLine {
    fn id(&self) -> &'static str {
        "L004"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for line in scan_lines(source) {
            if line.literal {
                continue;
            }
            if let Some((depth, name, attrs_str)) = opening_directive(line.trimmed) {
                let name_at = directive_name_start(line.trimmed, depth);
                let rest = &line.trimmed[name_at + name.len()..];
                if rest.starts_with('{') {
                    // L003 owns curly-brace openers.
                    continue;
                }
                if name == "section" {
                    // L001 owns ::section openers (it fires on every one and
                    // its fix composes same-line content into a markdown
                    // heading); a competing L004 fix would demote the title
                    // to a bare paragraph.
                    continue;
                }
                let tail_at = name_at + name.len() + attrs_str.len();
                let tail = &line.trimmed[tail_at..];
                if !tail.trim().is_empty() {
                    let fix = l004_opener_fix(&line, &name, &attrs_str, tail_at, tail);
                    out.push(diag_fix(
                        "L004",
                        format!(
                            "Block directive '::{name}' has content on the same line — openers and closers must be on their own line"
                        ),
                        Some(line.span()),
                        Some(fix),
                    ));
                }
            } else {
                // Not an opener: a leading colon run followed by whitespace and
                // content is an attempted closer with content on the same line.
                let colons = line.trimmed.chars().take_while(|&c| c == ':').count();
                if colons >= 2 {
                    let rest = &line.trimmed[colons..];
                    if rest.starts_with(char::is_whitespace) && !rest.trim().is_empty() {
                        let fix = rule_fix(
                            "L004",
                            "move the content after '::' to its own line".to_string(),
                            vec![break_line_edit(&line, line.indent() + colons)],
                        );
                        out.push(diag_fix(
                            "L004",
                            "Block directive '::' has content on the same line — openers and closers must be on their own line".to_string(),
                            Some(line.span()),
                            Some(fix),
                        ));
                    }
                }
            }
        }
        out
    }
}

// ------------------------------------------------------------------
// L005 — nesting colon mismatch
// ------------------------------------------------------------------

/// L005: nesting colon count inconsistent with the actual nesting depth.
///
/// Conservative by design — it flags only the two unambiguous cases:
/// 1. an opener with more colons than the current depth allows
///    (e.g. `:::column` with no open parent), and
/// 2. a closer whose colon count matches no open block.
///
/// Closer-less leaf directives sit at the same depth as their parent
/// (`::cta` inside `::page`), so under-deep openers are NOT flagged.
struct NestingColonMismatch;

impl LintRule for NestingColonMismatch {
    fn id(&self) -> &'static str {
        "L005"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for line in scan_lines(source) {
            if line.literal {
                continue;
            }
            if let Some(close_depth) = closing_directive_depth(line.trimmed) {
                if let Some(pos) = stack.iter().rposition(|&d| d == close_depth) {
                    stack.truncate(pos);
                } else {
                    // Fix: close the innermost open block instead; with no
                    // open block at all, remove the orphan closer line.
                    let colon_start = line.indent();
                    let fix = match stack.last() {
                        Some(&top) => rule_fix(
                            "L005",
                            format!(
                                "rewrite closer to {top} colons to match the innermost open block"
                            ),
                            vec![TextEdit {
                                span: line.sub_span(colon_start, colon_start + close_depth),
                                replacement: ":".repeat(top),
                            }],
                        ),
                        None => rule_fix(
                            "L005",
                            "remove closing directive that matches no open block".to_string(),
                            vec![line.delete_edit(source.len())],
                        ),
                    };
                    out.push(diag_fix(
                        "L005",
                        format!(
                            "Nesting colon count inconsistent with actual depth: closing directive with {close_depth} colons does not match any open block"
                        ),
                        Some(line.span()),
                        Some(fix),
                    ));
                }
                continue;
            }
            if let Some((depth, _, _)) = opening_directive(line.trimmed) {
                let allowed = stack.last().map_or(2, |&d| d + 1);
                if depth > allowed {
                    let colon_start = line.indent();
                    let fix = rule_fix(
                        "L005",
                        format!("reduce opener colons from {depth} to {allowed}"),
                        vec![TextEdit {
                            span: line.sub_span(colon_start, colon_start + depth),
                            replacement: ":".repeat(allowed),
                        }],
                    );
                    out.push(diag_fix(
                        "L005",
                        format!(
                            "Nesting colon count inconsistent with actual depth: block opened with {depth} colons but nesting depth here allows at most {allowed}"
                        ),
                        Some(line.span()),
                        Some(fix),
                    ));
                }
                stack.push(depth);
            }
        }
        out
    }
}

// ------------------------------------------------------------------
// L010 / L011 — ::summary presence and placement
// ------------------------------------------------------------------

/// Doc types that should carry a `::summary` block.
const SUMMARY_DOC_TYPES: &[DocType] = &[DocType::Doc, DocType::Plan, DocType::Guide];

/// L011 placement window: the summary must start within this many lines
/// after the front matter.
const SUMMARY_TOP_LINES: usize = 30;

fn doc_type_name(doc_type: DocType) -> &'static str {
    match doc_type {
        DocType::Doc => "doc",
        DocType::Plan => "plan",
        DocType::Guide => "guide",
        DocType::Conversation => "conversation",
        DocType::Agent => "agent",
        DocType::Preference => "preference",
        DocType::Report => "report",
        DocType::Proposal => "proposal",
        DocType::Incident => "incident",
        DocType::Review => "review",
        DocType::App => "app",
        DocType::Manifest => "manifest",
        DocType::Website => "website",
        DocType::Web => "web",
        DocType::Deck => "deck",
        DocType::Slides => "slides",
        DocType::Presentation => "presentation",
        DocType::Paper => "paper",
    }
}

/// First top-level `::summary` block in the document, if any.
fn first_summary(doc: &SurfDoc) -> Option<&Span> {
    doc.blocks.iter().find_map(|b| match b {
        Block::Summary { span, .. } => Some(span),
        _ => None,
    })
}

/// Stub body inserted by the L010 fix.
const SUMMARY_STUB_BODY: &str = "TODO: add a one-paragraph summary.";

/// Build the L010 insertion fix: a `::summary` stub after the front matter,
/// or after the leading `#` title heading when one immediately follows it.
fn summary_stub_fix(source: &str) -> Option<Fix> {
    let lines: Vec<&str> = source.split('\n').collect();
    // L010 only fires when the front matter parsed, so the closer exists.
    let fm_end = front_matter_close_line(&lines)?;
    let mut anchor = fm_end;
    for (i, raw) in lines.iter().enumerate().skip(fm_end + 1) {
        if raw.trim().is_empty() {
            continue;
        }
        if raw.trim_start().starts_with("# ") {
            anchor = i;
        }
        break;
    }
    // Insertion point: start of the line after the anchor line.
    let after_anchor: usize = lines[..=anchor].iter().map(|l| l.len() + 1).sum();
    let (offset, text) = if after_anchor > source.len() {
        // Anchor is the last line and has no trailing newline.
        (
            source.len(),
            format!("\n\n::summary\n{SUMMARY_STUB_BODY}\n::"),
        )
    } else {
        (
            after_anchor,
            format!("\n::summary\n{SUMMARY_STUB_BODY}\n::\n"),
        )
    };
    Some(rule_fix(
        "L010",
        "insert a '::summary' stub block near the top of the document".to_string(),
        vec![TextEdit {
            span: Span {
                start_line: anchor + 2,
                end_line: anchor + 2,
                start_offset: offset,
                end_offset: offset,
            },
            replacement: text,
        }],
    ))
}

/// L010: missing `::summary` block on doc/plan/guide documents.
///
/// Fix (suggested): insert a `::summary` stub after the front matter (or the
/// leading title heading).
struct MissingSummary;

impl LintRule for MissingSummary {
    fn id(&self) -> &'static str {
        "L010"
    }

    fn check(&self, doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let Some(doc_type) = doc.front_matter.as_ref().and_then(|fm| fm.doc_type) else {
            return Vec::new();
        };
        if !SUMMARY_DOC_TYPES.contains(&doc_type) || first_summary(doc).is_some() {
            return Vec::new();
        }
        vec![diag_fix(
            "L010",
            format!(
                "Document of type '{}' has no '::summary' block",
                doc_type_name(doc_type)
            ),
            None,
            summary_stub_fix(source),
        )]
    }
}

/// L011: `::summary` present but not within the first ~30 lines after front
/// matter.
struct SummaryNotNearTop;

impl LintRule for SummaryNotNearTop {
    fn id(&self) -> &'static str {
        "L011"
    }

    fn check(&self, doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let Some(span) = first_summary(doc) else {
            return Vec::new();
        };
        let lines: Vec<&str> = source.split('\n').collect();
        // 1-based line number of the first body line.
        let body_first_line = front_matter_close_line(&lines).map_or(1, |i| i + 2);
        // 1-based position of the summary among non-front-matter lines.
        let body_position = span.start_line.saturating_sub(body_first_line) + 1;
        if body_position <= SUMMARY_TOP_LINES {
            return Vec::new();
        }
        vec![diag(
            "L011",
            format!(
                "'::summary' block starts at line {} — should appear within the first {} lines after front matter",
                span.start_line, SUMMARY_TOP_LINES
            ),
            Some(*span),
        )]
    }
}

// ------------------------------------------------------------------
// L020 — unknown block name with did-you-mean
// ------------------------------------------------------------------

/// Levenshtein edit distance between two strings (by chars).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Unique spec block name within edit distance 2 of `name`, if any.
/// Returns `None` on a tie at the minimum distance.
fn did_you_mean<'a>(name: &str, known: &'a BTreeSet<String>) -> Option<&'a str> {
    let mut best: Option<(&str, usize)> = None;
    let mut tie = false;
    for cand in known {
        if cand.len().abs_diff(name.len()) > 2 {
            continue;
        }
        let d = levenshtein(name, cand);
        if d > 2 {
            continue;
        }
        match best {
            None => best = Some((cand, d)),
            Some((_, bd)) if d < bd => {
                best = Some((cand, d));
                tie = false;
            }
            Some((_, bd)) if d == bd => tie = true,
            _ => {}
        }
    }
    if tie { None } else { best.map(|(c, _)| c) }
}

/// L020: block name not registered in `spec/blocks.toml`.
struct UnknownBlockName;

impl LintRule for UnknownBlockName {
    fn id(&self) -> &'static str {
        "L020"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let known = known_block_names();
        let mut out = Vec::new();
        for line in scan_lines(source) {
            if line.literal {
                continue;
            }
            if let Some((depth, name, _)) = opening_directive(line.trimmed) {
                // L002 owns ::table; aliases/sub-directives are parser-known.
                if name == "table"
                    || known.contains(&name)
                    || EXTRA_KNOWN_BLOCK_NAMES.contains(&name.as_str())
                {
                    continue;
                }
                // Fix (suggested): rename to the unique did-you-mean match.
                let (suggestion, fix) = match did_you_mean(&name, known) {
                    Some(s) => {
                        let name_start = line.indent() + depth;
                        let fix = rule_fix(
                            "L020",
                            format!("rename '::{name}' to '::{s}'"),
                            vec![TextEdit {
                                span: line.sub_span(name_start, name_start + name.len()),
                                replacement: s.to_string(),
                            }],
                        );
                        (format!(" — did you mean '::{s}'?"), Some(fix))
                    }
                    None => (String::new(), None),
                };
                out.push(diag_fix(
                    "L020",
                    format!("Unknown block '::{name}'{suggestion}"),
                    Some(line.span()),
                    fix,
                ));
            }
        }
        out
    }
}

// ------------------------------------------------------------------
// L030 — per-type required front matter fields
// ------------------------------------------------------------------

/// Build the L030 fix for a missing `updated` field whose value is derivable
/// from an existing top-level `created:` line in the raw front matter.
///
/// Lint core has no clock, so `created`/`updated` can never be filled with
/// "today"; the only derivable value is `updated` mirroring `created`.
fn derived_updated_fix(source: &str) -> Option<Fix> {
    let lines: Vec<&str> = source.split('\n').collect();
    let fm_end = front_matter_close_line(&lines)?;
    let mut offset = lines[0].len() + 1; // skip the opening `---`
    for raw in lines.iter().take(fm_end).skip(1) {
        let line_offset = offset;
        offset += raw.len() + 1;
        if raw.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((key, value)) = raw.split_once(':') else {
            continue;
        };
        if key.trim() != "created" || value.trim().is_empty() {
            continue;
        }
        // Insert `updated: <same raw value>` on the line after `created:`,
        // preserving the original quoting style.
        let insert_at = line_offset + raw.len() + 1;
        let line_no = source[..insert_at].matches('\n').count() + 1;
        return Some(rule_fix(
            "L030",
            format!("insert 'updated:{value}' derived from the created field"),
            vec![TextEdit {
                span: Span {
                    start_line: line_no,
                    end_line: line_no,
                    start_offset: insert_at,
                    end_offset: insert_at,
                },
                replacement: format!("updated:{value}\n"),
            }],
        ));
    }
    None
}

/// L030: missing required front matter field per doc type.
///
/// The global checks (title → V001, type → V002) live in validate.rs and are
/// NOT duplicated here; L030 covers only the per-type extras:
/// plan → status + created, guide → confidence + updated. `type: doc` has no
/// extras beyond V001/V002.
///
/// Fix (safe, partial): only `updated` is derivable without a clock (mirrors
/// an existing `created` value). All other missing fields are detection-only.
struct MissingRequiredFrontMatter;

impl LintRule for MissingRequiredFrontMatter {
    fn id(&self) -> &'static str {
        "L030"
    }

    fn check(&self, doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let Some(fm) = doc.front_matter.as_ref() else {
            return Vec::new();
        };
        let Some(doc_type) = fm.doc_type else {
            return Vec::new();
        };
        let missing: Vec<&str> = match doc_type {
            DocType::Plan => {
                let mut m = Vec::new();
                if fm.status.is_none() {
                    m.push("status");
                }
                if fm.created.is_none() {
                    m.push("created");
                }
                m
            }
            DocType::Guide => {
                let mut m = Vec::new();
                if fm.confidence.is_none() {
                    m.push("confidence");
                }
                if fm.updated.is_none() {
                    m.push("updated");
                }
                m
            }
            _ => Vec::new(),
        };
        missing
            .into_iter()
            .map(|field| {
                let fix = if field == "updated" && fm.created.is_some() {
                    derived_updated_fix(source)
                } else {
                    None
                };
                diag_fix(
                    "L030",
                    format!(
                        "Document of type '{}' is missing required front matter field: {field}",
                        doc_type_name(doc_type)
                    ),
                    None,
                    fix,
                )
            })
            .collect()
    }
}

// ------------------------------------------------------------------
// L031 — enum value invalid only by case/whitespace
// ------------------------------------------------------------------

/// Valid values for enum-typed front matter fields, as serde accepts them.
const FM_ENUM_FIELDS: &[(&str, &[&str])] = &[
    (
        "type",
        &[
            "doc",
            "guide",
            "conversation",
            "plan",
            "agent",
            "preference",
            "report",
            "proposal",
            "incident",
            "review",
            "app",
            "manifest",
            "website",
            "deck",
            "slides",
        ],
    ),
    ("status", &["draft", "active", "closed", "archived"]),
    (
        "scope",
        &[
            "personal",
            "workspace-private",
            "workspace",
            "repo",
            "public",
        ],
    ),
    ("confidence", &["low", "medium", "high"]),
];

/// L031: enum-valued front matter field invalid only by letter case or
/// surrounding whitespace (e.g. `status: Active`). Works on the raw front
/// matter text, so it fires even when the YAML enum parse failed (P002).
struct FrontMatterEnumCase;

impl LintRule for FrontMatterEnumCase {
    fn id(&self) -> &'static str {
        "L031"
    }

    fn check(&self, _doc: &SurfDoc, source: &str) -> Vec<Diagnostic> {
        let lines: Vec<&str> = source.split('\n').collect();
        let Some(fm_end) = front_matter_close_line(&lines) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut offset = lines[0].len() + 1; // skip the opening `---`
        for (idx, raw) in lines.iter().enumerate().take(fm_end).skip(1) {
            let line_offset = offset;
            offset += raw.len() + 1;
            // Top-level `key: value` lines only (no indented/nested keys).
            if raw.starts_with(char::is_whitespace) {
                continue;
            }
            let Some((key, value)) = raw.split_once(':') else {
                continue;
            };
            let key = key.trim();
            let Some((_, valid)) = FM_ENUM_FIELDS.iter().find(|(k, _)| *k == key) else {
                continue;
            };
            let raw_value = value;
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if value.is_empty() || valid.contains(&value) {
                continue;
            }
            let normalized = value.trim().to_lowercase();
            if let Some(expected) = valid.iter().find(|v| **v == normalized) {
                // Fix: line-level text edit replacing the raw value bytes
                // (including any quotes) with the normalized enum value.
                // Never YAML re-serialization — the rest of the front matter
                // is untouched.
                let colon_at = raw.len() - raw_value.len();
                let ws = raw_value.len() - raw_value.trim_start().len();
                let val_start = line_offset + colon_at + ws;
                let val_end = line_offset + colon_at + raw_value.trim_end().len();
                let fix = rule_fix(
                    "L031",
                    format!("normalize '{key}' value to '{expected}'"),
                    vec![TextEdit {
                        span: Span {
                            start_line: idx + 1,
                            end_line: idx + 1,
                            start_offset: val_start,
                            end_offset: val_end,
                        },
                        replacement: (*expected).to_string(),
                    }],
                );
                out.push(diag_fix(
                    "L031",
                    format!(
                        "Front matter field '{key}' has value '{value}' — should be '{expected}'"
                    ),
                    Some(Span {
                        start_line: idx + 1,
                        end_line: idx + 1,
                        start_offset: line_offset,
                        end_offset: line_offset + raw.len(),
                    }),
                    Some(fix),
                ));
            }
        }
        out
    }
}

// ------------------------------------------------------------------
// L040 — mermaid construct skipped during translation
// ------------------------------------------------------------------

/// L040: a `::diagram` body in mermaid syntax used a construct outside the
/// supported translation subset — the translator skipped that line (info;
/// the rest of the diagram still renders).
struct MermaidConstructSkipped;

impl MermaidConstructSkipped {
    fn walk(blocks: &[Block], out: &mut Vec<Diagnostic>) {
        for block in blocks {
            if let Block::Diagram { diagram_type, content, span, .. } = block {
                if let Some(t) = crate::mermaid_compat::translate(diagram_type, content) {
                    for note in &t.notes {
                        out.push(diag(
                            "L040",
                            format!(
                                "Mermaid construct not translated (diagram line {}): {}",
                                note.line, note.construct
                            ),
                            Some(*span),
                        ));
                    }
                }
            }
            if let Some(children) = container_children(block) {
                Self::walk(children, out);
            }
        }
    }
}

/// Child-block accessor for the container variants a `::diagram` can nest
/// inside (mirrors the citation walker's container set).
fn container_children(b: &Block) -> Option<&[Block]> {
    match b {
        Block::Page { children, .. }
        | Block::Section { children, .. }
        | Block::Slide { children, .. }
        | Block::App { children, .. }
        | Block::AppShell { children, .. }
        | Block::Sidebar { children, .. }
        | Block::Panel { children, .. }
        | Block::TabContent { children, .. }
        | Block::Drawer { children, .. }
        | Block::Modal { children, .. } => Some(children),
        _ => None,
    }
}

impl LintRule for MermaidConstructSkipped {
    fn id(&self) -> &'static str {
        "L040"
    }

    fn check(&self, doc: &SurfDoc, _source: &str) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        Self::walk(&doc.blocks, &mut out);
        out
    }
}

// ------------------------------------------------------------------
// Unified check entry point
// ------------------------------------------------------------------

/// Result of running all three diagnostic layers over a source string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    /// All diagnostics, sorted by span start offset then rule code.
    pub diagnostics: Vec<Diagnostic>,
    /// Number of `Severity::Error` diagnostics.
    pub error_count: usize,
    /// Number of `Severity::Warning` diagnostics.
    pub warning_count: usize,
    /// Number of `Severity::Info` diagnostics.
    pub info_count: usize,
    /// Number of diagnostics carrying an auto-fix (`fix.is_some()`).
    pub fixable_count: usize,
}

/// Configuration for [`check_with`]: per-rule severity overrides and
/// disabled rule ids. Applies to all three layers (P/V/L codes).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LintConfig {
    /// Override the default severity of a rule id.
    pub severity_overrides: BTreeMap<String, Severity>,
    /// Rule ids whose diagnostics are dropped entirely.
    pub disabled_rules: BTreeSet<String>,
    /// Extra allowed front matter values per field (e.g. `"type"` →
    /// `{"checkpoint"}`): suppresses the P005 unknown-value diagnostic for
    /// the listed values. The front matter itself still parses to `None`
    /// (the typed schema is unchanged) — this only silences the diagnostic.
    #[serde(default)]
    pub extra_frontmatter_values: BTreeMap<String, BTreeSet<String>>,
}

/// Run all three diagnostic layers (parse, validate, lint) over `input`.
pub fn check(input: &str) -> CheckReport {
    check_with(input, &LintConfig::default())
}

/// [`check`] with severity overrides and disabled rules applied.
pub fn check_with(input: &str, cfg: &LintConfig) -> CheckReport {
    let result = crate::parse::parse(input);
    let mut diagnostics = result.diagnostics;
    attach_parse_fixes(&mut diagnostics, &result.doc.source);
    // The front matter failed to parse (P002 broken YAML / P005 unknown
    // value) — V001/V002 would fire only because `front_matter` is `None`,
    // duplicating the parse diagnostic even when title/type are literally
    // present in the raw YAML. Suppress them; the P-code is the signal.
    // (Keyed on the raw parse diagnostics, BEFORE config-level suppression,
    // so a P005 silenced via extra_frontmatter_values still suppresses the
    // cascade.)
    let front_matter_unparseable = diagnostics
        .iter()
        .any(|d| matches!(d.code.as_deref(), Some("P002") | Some("P005")));
    let mut schema = result.doc.validate();
    if front_matter_unparseable {
        schema.retain(|d| !matches!(d.code.as_deref(), Some("V001") | Some("V002")));
    }
    diagnostics.extend(schema);
    // P005 values explicitly allowed by the caller's config are silenced
    // (exact reconstruction of the parse-layer message).
    if !cfg.extra_frontmatter_values.is_empty() {
        diagnostics.retain(|d| {
            d.code.as_deref() != Some("P005")
                || !cfg.extra_frontmatter_values.iter().any(|(field, values)| {
                    values
                        .iter()
                        .any(|v| d.message == crate::parse::p005_message(field, v))
                })
        });
    }
    for rule in all_rules() {
        if cfg.disabled_rules.contains(rule.id()) {
            continue;
        }
        diagnostics.extend(rule.check(&result.doc, &result.doc.source));
    }

    diagnostics.retain(|d| {
        d.code
            .as_deref()
            .is_none_or(|code| !cfg.disabled_rules.contains(code))
    });
    for d in &mut diagnostics {
        if let Some(severity) = d
            .code
            .as_deref()
            .and_then(|c| cfg.severity_overrides.get(c))
        {
            d.severity = *severity;
        }
    }

    diagnostics.sort_by(|a, b| {
        let ka = a.span.map_or(0, |s| s.start_offset);
        let kb = b.span.map_or(0, |s| s.start_offset);
        ka.cmp(&kb).then_with(|| a.code.cmp(&b.code))
    });

    let count = |sev: Severity| diagnostics.iter().filter(|d| d.severity == sev).count();
    CheckReport {
        error_count: count(Severity::Error),
        warning_count: count(Severity::Warning),
        info_count: count(Severity::Info),
        fixable_count: diagnostics.iter().filter(|d| d.fix.is_some()).count(),
        diagnostics,
    }
}

// ------------------------------------------------------------------
// Parse-layer (P-code) fix attachment
// ------------------------------------------------------------------

/// Attach deterministic fixes to parse-layer diagnostics where one exists:
///
/// - **P001** (unclosed block) — only the force-closed-at-EOF case: append a
///   closer with the opener's colon count at end of file. Orphans force-closed
///   mid-document by an outer closer get no fix (L005 covers the closer side).
/// - **P002** (front matter) — only the unclosed-`---` case, and only when the
///   lines between the opener and the first blank line already parse as valid
///   front matter YAML: insert the closing `---` before that blank line. The
///   invalid-YAML case is detection-only.
fn attach_parse_fixes(diagnostics: &mut [Diagnostic], source: &str) {
    let lines: Vec<&str> = source.split('\n').collect();
    // Depth stack still open at EOF in the LINT layer's fence-aware view.
    // The parser is not fence-aware, so it can report a directive inside a
    // markdown ``` fence as unclosed; appending a closer for it would create
    // exactly the orphan closer L005's fix removes (P001↔L005 oscillation).
    // EOF-closer fixes are granted innermost-first only while they pop a
    // matching depth off this stack, so every closer P001 appends is one the
    // style layer agrees is needed.
    let mut lint_stack = lint_depth_stack_at_eof(source);
    for d in diagnostics {
        match d.code.as_deref() {
            Some("P001") => d.fix = p001_eof_closer_fix(d, source, &lines, &mut lint_stack),
            Some("P002") => d.fix = p002_close_front_matter_fix(d, source, &lines),
            _ => {}
        }
    }
}

/// Nesting depth stack left open at EOF, computed over the lint layer's
/// fence-aware [`scan_lines`] with L005's exact push/pop algorithm.
fn lint_depth_stack_at_eof(source: &str) -> Vec<usize> {
    let mut stack: Vec<usize> = Vec::new();
    for line in scan_lines(source) {
        if line.literal {
            continue;
        }
        if let Some(d) = closing_directive_depth(line.trimmed) {
            if let Some(pos) = stack.iter().rposition(|&s| s == d) {
                stack.truncate(pos);
            }
            continue;
        }
        if let Some((d, _, _)) = opening_directive(line.trimmed) {
            stack.push(d);
        }
    }
    stack
}

/// P001 fix: append the missing closer at EOF (force-closed-at-EOF case only,
/// and only when the lint-layer depth stack agrees a closer is needed —
/// `lint_stack` is popped per granted fix; see [`attach_parse_fixes`]).
fn p001_eof_closer_fix(
    d: &Diagnostic,
    source: &str,
    lines: &[&str],
    lint_stack: &mut Vec<usize>,
) -> Option<Fix> {
    let span = d.span?;
    // The EOF force-close span always ends at the last byte AND its final
    // line is not a closing directive (otherwise this is the mid-document
    // orphan case, where the closer at span end belongs to an outer block).
    if span.end_offset != source.len() {
        return None;
    }
    let last = lines.get(span.end_line.saturating_sub(1))?;
    if closing_directive_depth(last.trim()).is_some() {
        return None;
    }
    let opener = lines.get(span.start_line.saturating_sub(1))?;
    let (depth, name, _) = opening_directive(opener.trim())?;
    // Style-layer agreement gate: the appended closer must pop the innermost
    // open block of the fence-aware view too, or L005 would flag it as an
    // orphan and remove it again.
    if lint_stack.last() != Some(&depth) {
        return None;
    }
    lint_stack.pop();
    let closer = ":".repeat(depth);
    let text = if source.is_empty() || source.ends_with('\n') {
        format!("{closer}\n")
    } else {
        format!("\n{closer}\n")
    };
    let line_no = lines.len();
    Some(Fix {
        edits: vec![TextEdit {
            span: Span {
                start_line: line_no,
                end_line: line_no,
                start_offset: source.len(),
                end_offset: source.len(),
            },
            replacement: text,
        }],
        safety: FixSafety::Safe,
        description: format!("append the missing '{closer}' closer for '::{name}' at end of file"),
    })
}

/// P002 fix: close an unclosed front matter block before the first blank line,
/// when the captured lines already form valid front matter YAML.
fn p002_close_front_matter_fix(d: &Diagnostic, source: &str, lines: &[&str]) -> Option<Fix> {
    // Only the unclosed-delimiter case (span pinned to line 1 by the parser);
    // the invalid-YAML case spans the whole front matter block.
    if !d.message.contains("never closed") {
        return None;
    }
    let blank = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| l.trim().is_empty())
        .map(|(i, _)| i)?;
    if blank < 2 {
        return None; // nothing between the `---` and the blank line
    }
    let yaml = lines[1..blank].join("\n");
    if serde_yaml::from_str::<FrontMatter>(&yaml).is_err() {
        return None;
    }
    let offset: usize = lines[..blank].iter().map(|l| l.len() + 1).sum();
    if offset > source.len() {
        return None;
    }
    Some(Fix {
        edits: vec![TextEdit {
            span: Span {
                start_line: blank + 1,
                end_line: blank + 1,
                start_offset: offset,
                end_offset: offset,
            },
            replacement: "---\n".to_string(),
        }],
        safety: FixSafety::Safe,
        description: "close the front matter with '---' before the first blank line".to_string(),
    })
}

// ------------------------------------------------------------------
// Fix application engine
// ------------------------------------------------------------------

/// Maximum number of fix → re-check passes in [`apply_fixes`].
pub const MAX_FIX_ITERATIONS: usize = 10;

/// A fix that was applied by [`apply_fixes`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedFix {
    /// Rule code of the fixed diagnostic (`P001`, `L003`, …).
    pub code: String,
    /// The fix's human-readable description.
    pub description: String,
}

/// A fix that was skipped by [`apply_fixes`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFix {
    /// Rule code of the diagnostic whose fix was skipped.
    pub code: String,
    /// The fix's human-readable description.
    pub description: String,
    /// Why the fix was not applied.
    pub reason: String,
}

/// Result of [`apply_fixes`] / [`apply_fixes_once`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixOutcome {
    /// The fixed source. Bytes outside the applied edits are preserved
    /// exactly (CRLF input is normalised to LF first, matching `parse`).
    pub source: String,
    /// Fixes applied, across all passes, in application order.
    pub applied: Vec<AppliedFix>,
    /// Fixes skipped in the final pass (earlier-pass skips are retried and
    /// either applied or re-skipped).
    pub skipped: Vec<SkippedFix>,
    /// Number of passes that changed the source (0 = nothing to fix;
    /// [`MAX_FIX_ITERATIONS`] = fixpoint not reached).
    pub iterations: usize,
}

/// True when two edits cannot be applied in the same pass: their byte ranges
/// overlap, or they start at the same offset (ambiguous ordering for
/// insertions).
fn edits_conflict(a: &TextEdit, b: &TextEdit) -> bool {
    let (a, b) = (&a.span, &b.span);
    a.start_offset == b.start_offset
        || (a.start_offset < b.end_offset && b.start_offset < a.end_offset)
}

/// True when the edit's byte range is applicable to `source` (in bounds,
/// ordered, and on UTF-8 character boundaries).
fn edit_span_is_valid(e: &TextEdit, source: &str) -> bool {
    let s = &e.span;
    s.start_offset <= s.end_offset
        && s.end_offset <= source.len()
        && source.is_char_boundary(s.start_offset)
        && source.is_char_boundary(s.end_offset)
}

/// One fix pass: run [`check`], take every fix at or below `safety`, drop
/// conflicting edits (first fix wins; ties prefer the innermost/latest
/// diagnostic so nested EOF closers land inside-out), and splice the accepted
/// edits into `source` in descending start-offset order.
///
/// Returns `None` as the new source when no edit was applied.
fn run_fix_pass(
    source: &str,
    safety: FixSafety,
    cfg: &LintConfig,
) -> (Option<String>, Vec<AppliedFix>, Vec<SkippedFix>) {
    let report = check_with(source, cfg);
    let mut candidates: Vec<(&Diagnostic, &Fix)> = report
        .diagnostics
        .iter()
        .filter_map(|d| d.fix.as_ref().map(|f| (d, f)))
        .filter(|(_, f)| f.safety <= safety)
        .collect();
    // Precedence order = application order: highest first-edit offset first;
    // ties broken by the innermost diagnostic (largest span start).
    candidates.sort_by(|(da, fa), (db, fb)| {
        let key = |f: &Fix| {
            f.edits
                .iter()
                .map(|e| e.span.start_offset)
                .min()
                .unwrap_or(0)
        };
        key(fb).cmp(&key(fa)).then_with(|| {
            let span_start = |d: &Diagnostic| d.span.map_or(0, |s| s.start_offset);
            span_start(db).cmp(&span_start(da))
        })
    });

    let mut accepted: Vec<TextEdit> = Vec::new();
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    'candidate: for (d, f) in candidates {
        let code = d.code.clone().unwrap_or_default();
        let skip = |reason: &str, skipped: &mut Vec<SkippedFix>| {
            skipped.push(SkippedFix {
                code: code.clone(),
                description: f.description.clone(),
                reason: reason.to_string(),
            });
        };
        if f.edits.is_empty() {
            skip("fix carries no edits", &mut skipped);
            continue;
        }
        for e in &f.edits {
            if !edit_span_is_valid(e, source) {
                skip("edit span is out of bounds for this source", &mut skipped);
                continue 'candidate;
            }
        }
        for (i, e) in f.edits.iter().enumerate() {
            if accepted.iter().any(|a| edits_conflict(a, e))
                || f.edits[..i].iter().any(|a| edits_conflict(a, e))
            {
                skip(
                    "overlaps an edit from an already-accepted fix",
                    &mut skipped,
                );
                continue 'candidate;
            }
        }
        accepted.extend(f.edits.iter().cloned());
        applied.push(AppliedFix {
            code,
            description: f.description.clone(),
        });
    }

    if accepted.is_empty() {
        return (None, applied, skipped);
    }
    // Splice descending so earlier offsets stay valid.
    accepted.sort_by(|a, b| {
        b.span
            .start_offset
            .cmp(&a.span.start_offset)
            .then(b.span.end_offset.cmp(&a.span.end_offset))
    });
    let mut out = source.to_string();
    for e in &accepted {
        out.replace_range(e.span.start_offset..e.span.end_offset, &e.replacement);
    }
    (Some(out), applied, skipped)
}

/// Apply every auto-fix at or below the requested `safety` tier, re-running
/// [`check`] after each pass until no applicable fix remains (fixpoint) or
/// [`MAX_FIX_ITERATIONS`] passes were applied.
///
/// CRLF input is normalised to LF first (the same normalisation `parse`
/// applies, so edit offsets line up); all other untouched bytes are preserved
/// exactly. Guarantees enforced by property tests: idempotent, converging
/// (an applied fix's diagnostic is gone on re-check), non-destructive, and
/// deterministic.
pub fn apply_fixes(input: &str, safety: FixSafety) -> FixOutcome {
    apply_fixes_with(input, safety, &LintConfig::default())
}

/// [`apply_fixes`] with a [`LintConfig`]: disabled rules contribute no fixes,
/// and config-suppressed diagnostics (e.g. P005 extra values) are likewise
/// never "fixed". Severity overrides do not change fix behaviour.
pub fn apply_fixes_with(input: &str, safety: FixSafety, cfg: &LintConfig) -> FixOutcome {
    let mut source = input.replace("\r\n", "\n");
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    let mut iterations = 0usize;
    while iterations < MAX_FIX_ITERATIONS {
        let (next, mut pass_applied, pass_skipped) = run_fix_pass(&source, safety, cfg);
        skipped = pass_skipped;
        match next {
            Some(s) => {
                iterations += 1;
                source = s;
                applied.append(&mut pass_applied);
            }
            None => break,
        }
    }
    FixOutcome {
        source,
        applied,
        skipped,
        iterations,
    }
}

/// Single-pass variant of [`apply_fixes`]: applies at most one round of
/// non-conflicting fixes without iterating to a fixpoint. Useful for callers
/// that want to show intermediate states (e.g. a CLI `--diff` mode).
pub fn apply_fixes_once(input: &str, safety: FixSafety) -> FixOutcome {
    let source = input.replace("\r\n", "\n");
    let (next, applied, skipped) = run_fix_pass(&source, safety, &LintConfig::default());
    let iterations = usize::from(next.is_some());
    FixOutcome {
        source: next.unwrap_or(source),
        applied,
        skipped,
        iterations,
    }
}

// ------------------------------------------------------------------
// JSON envelope (shared by the surf-lint CLI and the wasm bindings)
// ------------------------------------------------------------------

/// Version of the JSON envelope schema emitted by [`report_to_json`] /
/// [`reports_to_json`] — and therefore by `surf-lint check --format json`
/// and the wasm `check_json` export, which both serialize these values.
pub const JSON_SCHEMA_VERSION: u32 = 1;

/// Per-file object of the surf-lint JSON envelope.
///
/// `path` is the file label; pass `None` for in-memory input (e.g. the wasm
/// `check_json` export) and the `path` key is omitted entirely. Diagnostics
/// are the [`Diagnostic`] serde serialization verbatim: spans carry line
/// numbers and byte offsets, fix payloads (description, safety, edits) ride
/// along when present, and `span`/`code`/`fix` keys are omitted when `None`.
pub fn report_to_json(path: Option<&str>, report: &CheckReport) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    if let Some(path) = path {
        obj.insert("path".into(), serde_json::Value::String(path.to_owned()));
    }
    obj.insert(
        "diagnostics".into(),
        serde_json::to_value(&report.diagnostics).expect("Diagnostic always serializes"),
    );
    obj.insert("error_count".into(), report.error_count.into());
    obj.insert("warning_count".into(), report.warning_count.into());
    obj.insert("info_count".into(), report.info_count.into());
    obj.insert("fixable_count".into(), report.fixable_count.into());
    serde_json::Value::Object(obj)
}

/// Full surf-lint JSON envelope over `(path, report)` pairs:
///
/// ```json
/// { "schema_version": 1, "files": [ ... ], "summary": { ... } }
/// ```
///
/// Each pair becomes a [`report_to_json`] object in `files`; `summary`
/// aggregates the per-file counts (`files` = number of pairs). This is the
/// single construction point for the envelope, so the CLI (`--format json`)
/// and the wasm `check_json` export emit identical structures.
pub fn reports_to_json(files: &[(Option<&str>, &CheckReport)]) -> serde_json::Value {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    let mut fixable = 0usize;
    let json_files: Vec<serde_json::Value> = files
        .iter()
        .map(|(path, report)| {
            errors += report.error_count;
            warnings += report.warning_count;
            infos += report.info_count;
            fixable += report.fixable_count;
            report_to_json(*path, report)
        })
        .collect();
    serde_json::json!({
        "schema_version": JSON_SCHEMA_VERSION,
        "files": json_files,
        "summary": {
            "files": files.len(),
            "error_count": errors,
            "warning_count": warnings,
            "info_count": infos,
            "fixable_count": fixable,
        },
    })
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Run a single rule over parsed input.
    fn run_rule(rule: &dyn LintRule, input: &str) -> Vec<Diagnostic> {
        let result = crate::parse::parse(input);
        rule.check(&result.doc, &result.doc.source)
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().filter_map(|d| d.code.as_deref()).collect()
    }

    // --- registry ---

    #[test]
    fn registry_loads_with_expected_rules() {
        let reg = rule_registry();
        for id in ["P001", "P002", "P003", "L001", "L005", "L020", "L031"] {
            assert!(reg.contains_key(id), "registry missing {id}");
        }
        assert_eq!(rule_registry_total(), reg.len());
    }

    #[test]
    fn registry_severity_matches_defaults() {
        assert_eq!(registry_severity("P002"), Severity::Error);
        assert_eq!(registry_severity("P001"), Severity::Warning);
        assert_eq!(registry_severity("L011"), Severity::Info);
    }

    #[test]
    fn known_block_names_loaded_from_spec() {
        let known = known_block_names();
        assert!(known.contains("callout"));
        assert!(known.contains("section"));
        assert!(!known.contains("table"));
    }

    // --- L001 / L002 ---

    #[test]
    fn l001_fires_on_section_block() {
        let diags = run_rule(&SectionBlockUsed, "::section[title=Intro]\nText.\n::\n");
        assert_eq!(codes(&diags), vec!["L001"]);
        assert_eq!(diags[0].span.unwrap().start_line, 1);
    }

    #[test]
    fn l001_silent_on_headings() {
        let diags = run_rule(&SectionBlockUsed, "## Intro\n\nText.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn l002_fires_on_table_block() {
        let diags = run_rule(&TableBlockUsed, "::table\n| a | b |\n::\n");
        assert_eq!(codes(&diags), vec!["L002"]);
    }

    // --- L003 ---

    #[test]
    fn l003_fires_on_curly_attrs() {
        let diags = run_rule(&CurlyAttrBraces, "::callout{type=info}\nHi\n::\n");
        assert_eq!(codes(&diags), vec!["L003"]);
        assert!(diags[0].message.contains("[square]"));
    }

    #[test]
    fn l003_silent_on_square_attrs() {
        let diags = run_rule(&CurlyAttrBraces, "::callout[type=info]\nHi\n::\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn l003_silent_inside_code_block() {
        let input = "::code[lang=surf]\n::callout{type=info}\n::\n";
        let diags = run_rule(&CurlyAttrBraces, input);
        assert!(diags.is_empty(), "code block content is literal: {diags:?}");
    }

    #[test]
    fn l003_silent_inside_markdown_fence() {
        let input = "```\n::callout{type=info}\n```\n";
        let diags = run_rule(&CurlyAttrBraces, input);
        assert!(diags.is_empty(), "fenced content is literal: {diags:?}");
    }

    // --- L004 ---

    #[test]
    fn l004_fires_on_opener_with_trailing_content() {
        let diags = run_rule(
            &DirectiveNotOnOwnLine,
            "::callout[type=info] Watch out!\n::\n",
        );
        assert_eq!(codes(&diags), vec!["L004"]);
    }

    #[test]
    fn l004_fires_on_closer_with_trailing_content() {
        // Non-name tail: still a closer with content on the same line.
        let diags = run_rule(
            &DirectiveNotOnOwnLine,
            "::callout[type=info]\nHi\n:: 42 done\n",
        );
        assert_eq!(codes(&diags), vec!["L004"]);
        // A name-shaped tail (`:: done`) is a tolerant opener since
        // p3-copy-sweep — owned by the parser (P001 when unclosed), not L004.
        let none = run_rule(
            &DirectiveNotOnOwnLine,
            "::callout[type=info]\nHi\n:: done\n::\n",
        );
        assert!(none.is_empty(), "tolerant opener misread as closer: {none:?}");
    }

    #[test]
    fn l004_silent_on_clean_directives() {
        let diags = run_rule(&DirectiveNotOnOwnLine, "::callout[type=info]\nHi\n::\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn l004_defers_curly_openers_to_l003() {
        let diags = run_rule(&DirectiveNotOnOwnLine, "::callout{type=info}\nHi\n::\n");
        assert!(diags.is_empty());
    }

    // --- L005 ---

    #[test]
    fn l005_fires_on_overdeep_opener() {
        let diags = run_rule(&NestingColonMismatch, ":::column\nLeft\n:::\n");
        assert_eq!(codes(&diags), vec!["L005"]);
        assert!(diags[0].message.contains("at most 2"));
    }

    #[test]
    fn l005_fires_on_orphan_closer() {
        let diags = run_rule(&NestingColonMismatch, "::callout\nHi\n::\n::\n");
        assert_eq!(codes(&diags), vec!["L005"]);
        assert!(diags[0].message.contains("does not match any open block"));
    }

    #[test]
    fn l005_silent_on_correct_nesting() {
        let input = "::columns\n:::column\nLeft\n:::\n:::column\nRight\n:::\n::\n";
        let diags = run_rule(&NestingColonMismatch, input);
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn l005_silent_on_same_depth_leaf_children() {
        // Leaf directives inside ::page legally sit at the parent's depth.
        let input = "::page[route=\"/\"]\n::cta[label=\"Go\" href=\"/x\"]\n::hero-image[src=\"a.jpg\"]\n::\n";
        let diags = run_rule(&NestingColonMismatch, input);
        assert!(diags.is_empty(), "{diags:?}");
    }

    // --- L010 / L011 ---

    #[test]
    fn l010_fires_on_plan_without_summary() {
        let input = "---\ntitle: T\ntype: plan\n---\n# Heading\n";
        let diags = run_rule(&MissingSummary, input);
        assert_eq!(codes(&diags), vec!["L010"]);
        assert!(diags[0].message.contains("'plan'"));
    }

    #[test]
    fn l010_silent_when_summary_present() {
        let input = "---\ntitle: T\ntype: doc\n---\n::summary\nShort.\n::\n";
        let diags = run_rule(&MissingSummary, input);
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_silent_on_other_doc_types() {
        let input = "---\ntitle: T\ntype: report\n---\n# Heading\n";
        let diags = run_rule(&MissingSummary, input);
        assert!(diags.is_empty());
    }

    #[test]
    fn l011_fires_on_late_summary() {
        let filler = "Filler line.\n".repeat(35);
        let input = format!("---\ntitle: T\ntype: doc\n---\n{filler}::summary\nShort.\n::\n");
        let diags = run_rule(&SummaryNotNearTop, &input);
        assert_eq!(codes(&diags), vec!["L011"]);
        assert_eq!(diags[0].severity, Severity::Info);
    }

    #[test]
    fn l011_silent_on_early_summary() {
        let input = "---\ntitle: T\ntype: doc\n---\n\n::summary\nShort.\n::\n";
        let diags = run_rule(&SummaryNotNearTop, input);
        assert!(diags.is_empty());
    }

    // --- L020 ---

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("callout", "callout"), 0);
        assert_eq!(levenshtein("callouts", "callout"), 1);
        assert_eq!(levenshtein("colout", "callout"), 2);
        assert_eq!(levenshtein("table", "tabs"), 2);
    }

    #[test]
    fn l020_fires_with_suggestion() {
        let diags = run_rule(&UnknownBlockName, "::callouts[type=info]\nHi\n::\n");
        assert_eq!(codes(&diags), vec!["L020"]);
        assert!(
            diags[0].message.contains("did you mean '::callout'?"),
            "{}",
            diags[0].message
        );
    }

    #[test]
    fn l020_fires_without_suggestion_when_no_close_match() {
        let diags = run_rule(&UnknownBlockName, "::zzqqxx\nHi\n::\n");
        assert_eq!(codes(&diags), vec!["L020"]);
        assert!(!diags[0].message.contains("did you mean"));
    }

    #[test]
    fn l020_silent_on_known_planned_and_alias_names() {
        // `countdown` is planned-only in the spec; `column` and
        // `action-items` are parser-accepted extras.
        let input = "::countdown[date=2026-12-31]\n::\n::columns\n:::column\nx\n:::\n::\n::action-items\n- do it\n::\n";
        let diags = run_rule(&UnknownBlockName, input);
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn l020_defers_table_to_l002() {
        let diags = run_rule(&UnknownBlockName, "::table\n| a |\n::\n");
        assert!(diags.is_empty());
    }

    // --- L030 ---

    #[test]
    fn l030_fires_on_plan_missing_status_and_created() {
        let input = "---\ntitle: T\ntype: plan\n---\nBody.\n";
        let diags = run_rule(&MissingRequiredFrontMatter, input);
        assert_eq!(codes(&diags), vec!["L030", "L030"]);
        assert!(diags.iter().any(|d| d.message.contains("status")));
        assert!(diags.iter().any(|d| d.message.contains("created")));
    }

    #[test]
    fn l030_fires_on_guide_missing_confidence_and_updated() {
        let input = "---\ntitle: T\ntype: guide\n---\nBody.\n";
        let diags = run_rule(&MissingRequiredFrontMatter, input);
        assert_eq!(codes(&diags), vec!["L030", "L030"]);
        assert!(diags.iter().any(|d| d.message.contains("confidence")));
        assert!(diags.iter().any(|d| d.message.contains("updated")));
    }

    #[test]
    fn l030_silent_on_complete_plan_and_plain_doc() {
        let plan =
            "---\ntitle: T\ntype: plan\nstatus: active\ncreated: \"2026-06-06\"\n---\nBody.\n";
        assert!(run_rule(&MissingRequiredFrontMatter, plan).is_empty());
        // `type: doc` requires only title+type, already covered by V001/V002.
        let doc = "---\ntitle: T\ntype: doc\n---\nBody.\n";
        assert!(run_rule(&MissingRequiredFrontMatter, doc).is_empty());
    }

    // --- L031 ---

    #[test]
    fn l031_fires_on_case_mismatch() {
        let input = "---\ntitle: T\ntype: doc\nstatus: Active\n---\nBody.\n";
        let diags = run_rule(&FrontMatterEnumCase, input);
        assert_eq!(codes(&diags), vec!["L031"]);
        assert!(diags[0].message.contains("'Active'"));
        assert!(diags[0].message.contains("'active'"));
        assert_eq!(diags[0].span.unwrap().start_line, 4);
    }

    #[test]
    fn l031_fires_on_quoted_case_mismatch_in_type() {
        let input = "---\ntitle: T\ntype: \"Plan\"\n---\nBody.\n";
        let diags = run_rule(&FrontMatterEnumCase, input);
        assert_eq!(codes(&diags), vec!["L031"]);
        assert!(diags[0].message.contains("'plan'"));
    }

    #[test]
    fn l031_silent_on_valid_and_unfixable_values() {
        // Valid value: no diagnostic.
        let valid = "---\ntitle: T\ntype: doc\nstatus: active\n---\n";
        assert!(run_rule(&FrontMatterEnumCase, valid).is_empty());
        // Genuinely invalid value (not a case issue): V/parse layers own it.
        let invalid = "---\ntitle: T\ntype: doc\nstatus: bogus\n---\n";
        assert!(run_rule(&FrontMatterEnumCase, invalid).is_empty());
    }

    // --- check() / check_with() ---

    #[test]
    fn check_merges_all_three_layers_sorted() {
        // Parse layer: unclosed callout (P001). Schema layer: missing front
        // matter (V001/V002). Style layer: unknown block (L020).
        let input = "::wibble\nHi\n::\n\n::callout[type=info]\nUnclosed";
        let report = check(input);
        let fired = codes(&report.diagnostics);
        assert!(fired.contains(&"P001"), "{fired:?}");
        assert!(fired.contains(&"V001"), "{fired:?}");
        assert!(fired.contains(&"L020"), "{fired:?}");
        // Sorted by span start offset (None-span schema diags first).
        let offsets: Vec<usize> = report
            .diagnostics
            .iter()
            .map(|d| d.span.map_or(0, |s| s.start_offset))
            .collect();
        let mut sorted = offsets.clone();
        sorted.sort_unstable();
        assert_eq!(offsets, sorted);
        assert_eq!(
            report.error_count + report.warning_count + report.info_count,
            report.diagnostics.len()
        );
        // The unclosed callout (P001 at EOF) carries a fix.
        assert_eq!(
            report.fixable_count,
            report
                .diagnostics
                .iter()
                .filter(|d| d.fix.is_some())
                .count()
        );
        assert!(report.fixable_count >= 1, "P001 EOF fix expected");
    }

    #[test]
    fn check_clean_doc_is_empty() {
        let input =
            "---\ntitle: T\ntype: doc\n---\n\n::summary\nShort.\n::\n\n# Heading\n\nBody text.\n";
        let report = check(input);
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);
        assert_eq!(report.info_count, 0);
    }

    #[test]
    fn check_with_disables_rules() {
        let input =
            "---\ntitle: T\ntype: plan\nstatus: active\ncreated: \"2026-06-06\"\n---\nBody.\n";
        assert_eq!(codes(&check(input).diagnostics), vec!["L010"]);
        let cfg = LintConfig {
            disabled_rules: ["L010".to_string()].into_iter().collect(),
            ..LintConfig::default()
        };
        let report = check_with(input, &cfg);
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    }

    #[test]
    fn check_with_overrides_severity() {
        let input = "---\ntitle: T\ntype: doc\n---\nBody.\n";
        let cfg = LintConfig {
            severity_overrides: [("L010".to_string(), Severity::Error)]
                .into_iter()
                .collect(),
            ..LintConfig::default()
        };
        let report = check_with(input, &cfg);
        let l010 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("L010"))
            .expect("L010 fires");
        assert_eq!(l010.severity, Severity::Error);
        assert_eq!(report.error_count, 1);
    }

    // --- V001/V002 cascade suppression (dogfood bug 1) ---

    #[test]
    fn check_suppresses_v001_v002_cascade_on_p005() {
        // Dogfood repro: title and type are literally present; the closed
        // `type` enum rejects the value. The user gets ONE front-matter
        // diagnostic, not three.
        let input = "---\ntitle: T\ntype: checkpoint\n---\nBody.\n";
        let report = check(input);
        assert_eq!(codes(&report.diagnostics), vec!["P005"], "{report:?}");
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 1);
    }

    #[test]
    fn check_suppresses_v001_v002_cascade_on_p002() {
        // Unclosed front matter: exactly one front-matter diagnostic (P002).
        let input = "---\ntitle: T\ntype: doc\n\nBody.\n";
        let report = check(input);
        assert_eq!(codes(&report.diagnostics), vec!["P002"], "{report:?}");
    }

    #[test]
    fn check_keeps_v001_v002_when_front_matter_truly_absent() {
        // No front matter at all: V001/V002 are NOT a cascade — keep them.
        let report = check("# Just a heading\n\nBody.\n");
        let fired = codes(&report.diagnostics);
        assert!(fired.contains(&"V001"), "{fired:?}");
        assert!(fired.contains(&"V002"), "{fired:?}");
    }

    // --- P005 vs P002 split (dogfood bug 3 / message accuracy) ---

    #[test]
    fn p005_fires_for_unknown_enum_value_as_warning() {
        let report = check("---\ntitle: T\ntype: checkpoint\n---\nBody.\n");
        let p005 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("P005"))
            .expect("P005 fires");
        assert_eq!(p005.severity, Severity::Warning);
        assert_eq!(
            p005.message,
            "Front matter field 'type' has unrecognized value 'checkpoint'"
        );
        assert!(
            report
                .diagnostics
                .iter()
                .all(|d| d.code.as_deref() != Some("P002")),
            "P002 must not fire for valid YAML with out-of-enum values"
        );
    }

    #[test]
    fn p002_stays_error_for_genuinely_broken_yaml() {
        let report = check("---\ntitle: [unclosed\n---\nBody.\n");
        let p002 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("P002"))
            .expect("P002 fires on broken YAML");
        assert_eq!(p002.severity, Severity::Error);
        assert!(
            report
                .diagnostics
                .iter()
                .all(|d| d.code.as_deref() != Some("P005"))
        );
    }

    #[test]
    fn check_with_extra_frontmatter_values_suppresses_p005() {
        let input = "---\ntitle: T\ntype: checkpoint\n---\nBody.\n";
        let cfg = LintConfig {
            extra_frontmatter_values: [(
                "type".to_string(),
                ["checkpoint".to_string()].into_iter().collect(),
            )]
            .into_iter()
            .collect(),
            ..LintConfig::default()
        };
        let report = check_with(input, &cfg);
        // P005 silenced AND the V001/V002 cascade stays suppressed.
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        // A different unrecognized value still fires.
        let other = check_with("---\ntitle: T\ntype: spec\n---\nBody.\n", &cfg);
        assert_eq!(codes(&other.diagnostics), vec!["P005"]);
    }

    #[test]
    fn check_report_serializes_to_json() {
        let report = check("---\ntitle: T\ntype: doc\n---\nBody.\n");
        let json = serde_json::to_string(&report).expect("CheckReport serializes");
        assert!(json.contains("\"diagnostics\""));
        assert!(json.contains("\"fixable_count\""));
    }

    #[test]
    fn all_rules_have_unique_ids() {
        let rules = all_rules();
        let ids: BTreeSet<&str> = rules.iter().map(|r| r.id()).collect();
        assert_eq!(ids.len(), rules.len());
    }

    // ------------------------------------------------------------------
    // Fix engine — per-rule exact-output tests
    // ------------------------------------------------------------------

    fn fixed_safe(input: &str) -> String {
        apply_fixes(input, FixSafety::Safe).source
    }

    fn fixed_suggested(input: &str) -> String {
        apply_fixes(input, FixSafety::Suggested).source
    }

    #[test]
    fn fix_p001_appends_closer_at_eof() {
        assert_eq!(
            fixed_safe("::callout[type=info]\nUnclosed"),
            "::callout[type=info]\nUnclosed\n::\n"
        );
        assert_eq!(
            fixed_safe("::callout[type=info]\nUnclosed\n"),
            "::callout[type=info]\nUnclosed\n::\n"
        );
    }

    #[test]
    fn fix_p001_nested_closers_land_inside_out() {
        let out = apply_fixes("::columns\n:::column\nLeft", FixSafety::Safe);
        assert_eq!(out.source, "::columns\n:::column\nLeft\n:::\n::\n");
        assert_eq!(
            out.iterations, 2,
            "one closer per pass (same-offset edits conflict)"
        );
        assert!(
            check(&out.source)
                .diagnostics
                .iter()
                .all(|d| d.code.as_deref() != Some("P001"))
        );
    }

    #[test]
    fn fix_p001_skips_mid_document_orphans() {
        // The orphan ':::inner' is force-closed by the OUTER closer — no fix
        // (the closer line belongs to ::a). L005 has no complaint either:
        // the closer legitimately matches ::a.
        let input = "::a\n:::inner\nx\n::\n";
        let report = check(input);
        let p001 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("P001"))
            .expect("orphan fires P001");
        assert!(
            p001.fix.is_none(),
            "mid-document orphan must not get the EOF fix"
        );
    }

    #[test]
    fn fix_p002_closes_front_matter_before_first_blank_line() {
        let input = "---\ntitle: T\ntype: doc\n\nBody text.\n";
        assert_eq!(
            fixed_safe(input),
            "---\ntitle: T\ntype: doc\n---\n\nBody text.\n"
        );
    }

    #[test]
    fn fix_p002_no_fix_when_captured_lines_are_not_yaml() {
        let input = "---\ntitle: T\nnot yaml at all ::: {\n\nBody.\n";
        let report = check(input);
        let p002 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("P002"))
            .expect("P002 fires");
        assert!(p002.fix.is_none());
        assert_eq!(fixed_safe(input), input, "nothing applicable to fix");
    }

    #[test]
    fn fix_l001_replaces_section_with_heading() {
        assert_eq!(
            fixed_safe("::section[title=\"Background\"]\nText body.\n::\n"),
            "## Background\nText body.\n"
        );
    }

    #[test]
    fn fix_l001_honours_level_attr() {
        assert_eq!(
            fixed_safe("::section[title=\"Deep\" level=3]\nText.\n::\n"),
            "### Deep\nText.\n"
        );
    }

    #[test]
    fn fix_l001_strips_untitled_section() {
        assert_eq!(fixed_safe("::section\nText.\n::\n"), "Text.\n");
    }

    #[test]
    fn fix_l002_strips_table_wrapper() {
        assert_eq!(
            fixed_safe("::table\n| a | b |\n|---|---|\n| 1 | 2 |\n::\n"),
            "| a | b |\n|---|---|\n| 1 | 2 |\n"
        );
    }

    #[test]
    fn fix_l003_rewrites_curly_braces_to_square() {
        assert_eq!(
            fixed_safe("::callout{type=info}\nHi\n::\n"),
            "::callout[type=info]\nHi\n::\n"
        );
    }

    #[test]
    fn fix_l003_no_fix_for_unmatched_brace() {
        let input = "::callout{type=info\nHi\n::\n";
        let out = apply_fixes(input, FixSafety::Safe);
        assert_eq!(out.source, input);
        assert!(out.applied.is_empty());
    }

    #[test]
    fn fix_l004_moves_opener_tail_to_own_line() {
        assert_eq!(
            fixed_safe("::callout[type=info] Watch out!\n::\n"),
            "::callout[type=info]\nWatch out!\n::\n"
        );
    }

    #[test]
    fn fix_l004_moves_closer_tail_to_own_line() {
        // Non-name tail (a name-shaped tail like `:: done` is a tolerant
        // opener since p3-copy-sweep and no longer L004's to fix).
        assert_eq!(
            fixed_safe("::callout[type=info]\nHi\n:: 42 done\n"),
            "::callout[type=info]\nHi\n::\n42 done\n"
        );
    }

    #[test]
    fn fix_l004_attaches_displaced_attr_block() {
        // Dogfood bug 4 (plans/client/2026-02-23-rainbow-gardens-…:476):
        // `::callout [type=info]` is author-intended attributes — normalize
        // to the attribute form, never demote to literal body text.
        assert_eq!(
            fixed_safe("::callout [type=info]\nHi\n::\n"),
            "::callout[type=info]\nHi\n::\n"
        );
    }

    #[test]
    fn fix_l004_attaches_attrs_and_breaks_trailing_content() {
        // Bracket block reattaches; the content after it still moves down.
        assert_eq!(
            fixed_safe("::callout [type=info] Watch out!\n::\n"),
            "::callout[type=info]\nWatch out!\n::\n"
        );
    }

    #[test]
    fn fix_l004_plain_tail_still_breaks_line() {
        // Non-bracket tails keep the original break-to-next-line fix.
        assert_eq!(
            fixed_safe("::callout[type=info] note [1] here\n::\n"),
            "::callout[type=info]\nnote [1] here\n::\n"
        );
    }

    #[test]
    fn fix_l001_l004_composition_yields_heading() {
        // Dogfood bug 5: `::section <Title>` on one line must compose to a
        // markdown heading through apply_fixes, not a bare paragraph.
        assert_eq!(
            fixed_safe("::section Executive Summary\nBody.\n::\n"),
            "## Executive Summary\nBody.\n"
        );
    }

    #[test]
    fn fix_l001_composes_displaced_title_attrs() {
        // `::section [title=…]` — the displaced bracket block is read as
        // attributes, not as a literal title.
        assert_eq!(
            fixed_safe("::section [title=\"Background\"]\nBody.\n::\n"),
            "## Background\nBody.\n"
        );
    }

    #[test]
    fn fix_p001_l005_fence_oscillation_converges() {
        // Dogfood bug 2 (minimal repro of
        // plans/develop/2026-02-20-visitor-chat-implementation-plan.surf):
        // the opener lives inside a markdown ``` fence, so the parser (not
        // fence-aware) wants a closer at EOF while L005 (fence-aware) calls
        // that closer an orphan. Pre-fix this looped to the 10-pass cap.
        let input = "```\n::cta[label=x]\n```\n::\n";
        let out = apply_fixes(input, FixSafety::Safe);
        assert!(
            out.iterations <= 2,
            "fixpoint within 2 passes, got {} ({:?})",
            out.iterations,
            out.applied
        );
        // L005 removed the stray closer; P001 stays detection-only.
        assert_eq!(out.source, "```\n::cta[label=x]\n```\n");
        // Stable: fixing the output is a no-op.
        let again = apply_fixes(&out.source, FixSafety::Safe);
        assert_eq!(again.source, out.source);
        assert_eq!(again.iterations, 0);
    }

    #[test]
    fn fix_p001_no_eof_closer_for_opener_inside_fence() {
        // The same disagreement without a pre-existing closer: P001 fires
        // for the fenced opener but must NOT carry the EOF-closer fix.
        let input = "```\n::cta[label=x]\n```\n";
        let report = check(input);
        let p001 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("P001"))
            .expect("P001 fires (parser is not fence-aware)");
        assert!(p001.fix.is_none(), "no closer fix for a fenced opener");
        let out = apply_fixes(input, FixSafety::Safe);
        assert_eq!(out.source, input);
        assert_eq!(out.iterations, 0);
    }

    #[test]
    fn fix_l005_recomputes_overdeep_opener_then_closer() {
        // Pass 1 fixes the opener; pass 2 the now-orphaned ':::' closer (plus
        // a P001 closer for the then-unclosed block); pass 3 removes the
        // redundant closer. The fixpoint is correct and stable.
        let out = apply_fixes(":::column\nLeft\n:::\n", FixSafety::Safe);
        assert_eq!(out.source, "::column\nLeft\n::\n");
        assert_eq!(out.iterations, 3);
    }

    #[test]
    fn fix_l005_removes_orphan_closer_with_nothing_open() {
        assert_eq!(fixed_safe("::callout\nHi\n::\n::\n"), "::callout\nHi\n::\n");
    }

    #[test]
    fn fix_l030_derives_updated_from_created() {
        let input =
            "---\ntitle: T\ntype: guide\nconfidence: high\ncreated: \"2026-06-01\"\n---\nBody.\n";
        let fixed = fixed_safe(input);
        assert_eq!(
            fixed,
            "---\ntitle: T\ntype: guide\nconfidence: high\ncreated: \"2026-06-01\"\nupdated: \"2026-06-01\"\n---\nBody.\n"
        );
    }

    #[test]
    fn fix_l030_detection_only_when_not_derivable() {
        // Plan missing status+created: neither is derivable without a clock.
        let input = "---\ntitle: T\ntype: plan\n---\nBody.\n";
        let out = apply_fixes(input, FixSafety::Safe);
        assert_eq!(out.source, input);
        assert!(out.applied.is_empty());
        let report = check(input);
        assert!(
            report
                .diagnostics
                .iter()
                .filter(|d| d.code.as_deref() == Some("L030"))
                .all(|d| d.fix.is_none())
        );
    }

    #[test]
    fn fix_l031_normalizes_enum_case() {
        assert_eq!(
            fixed_safe("---\ntitle: T\ntype: doc\nstatus: Active\n---\nBody.\n"),
            "---\ntitle: T\ntype: doc\nstatus: active\n---\nBody.\n"
        );
    }

    #[test]
    fn fix_l031_strips_quotes_when_normalizing() {
        assert_eq!(
            fixed_safe(
                "---\ntitle: T\ntype: \"Plan\"\nstatus: active\ncreated: \"2026-06-06\"\n---\n::summary\nS.\n::\n"
            ),
            "---\ntitle: T\ntype: plan\nstatus: active\ncreated: \"2026-06-06\"\n---\n::summary\nS.\n::\n"
        );
    }

    #[test]
    fn fix_l010_suggested_inserts_summary_stub_after_heading() {
        let input = "---\ntitle: T\ntype: doc\n---\n\n# Doc\n\nBody.\n";
        assert_eq!(
            fixed_safe(input),
            input,
            "L010 is suggested-tier — Safe must not touch it"
        );
        let fixed = fixed_suggested(input);
        assert_eq!(
            fixed,
            "---\ntitle: T\ntype: doc\n---\n\n# Doc\n\n::summary\nTODO: add a one-paragraph summary.\n::\n\nBody.\n"
        );
        assert!(
            check(&fixed)
                .diagnostics
                .iter()
                .all(|d| d.code.as_deref() != Some("L010"))
        );
    }

    #[test]
    fn fix_l020_suggested_renames_to_unique_match() {
        let input = "::callouts[type=info]\nHi\n::\n";
        assert_eq!(fixed_safe(input), input, "L020 is suggested-tier");
        assert_eq!(fixed_suggested(input), "::callout[type=info]\nHi\n::\n");
    }

    #[test]
    fn fix_l020_no_fix_without_unique_suggestion() {
        let report = check("::zzqqxx\nHi\n::\n");
        let l020 = report
            .diagnostics
            .iter()
            .find(|d| d.code.as_deref() == Some("L020"))
            .expect("L020 fires");
        assert!(l020.fix.is_none());
    }

    // ------------------------------------------------------------------
    // Fix engine — UTF-8 multi-byte splicing
    // ------------------------------------------------------------------

    #[test]
    fn fix_l003_splices_correctly_after_multibyte_lines() {
        let input = "# Café ☕ 日本語\n\n::callout{type=info}\n🎉 Bienvenue à 東京!\n::\n";
        assert_eq!(
            fixed_safe(input),
            "# Café ☕ 日本語\n\n::callout[type=info]\n🎉 Bienvenue à 東京!\n::\n"
        );
    }

    #[test]
    fn fix_l031_with_multibyte_title_above() {
        let input = "---\ntitle: \"日本語のタイトル 🚀\"\ntype: doc\nstatus: Active\n---\nBody.\n";
        assert_eq!(
            fixed_safe(input),
            "---\ntitle: \"日本語のタイトル 🚀\"\ntype: doc\nstatus: Active\n---\nBody.\n"
                .replace("status: Active", "status: active")
        );
    }

    #[test]
    fn fix_l001_with_emoji_title() {
        assert_eq!(
            fixed_safe("::section[title=\"🚀 Launch — 発射\"]\nText.\n::\n"),
            "## 🚀 Launch — 発射\nText.\n"
        );
    }

    #[test]
    fn fix_l004_with_multibyte_tail() {
        assert_eq!(
            fixed_safe("::callout[type=info] ☕ café first\n::\n"),
            "::callout[type=info]\n☕ café first\n::\n"
        );
    }

    // ------------------------------------------------------------------
    // Fix engine — mechanics
    // ------------------------------------------------------------------

    #[test]
    fn fixable_count_counts_attached_fixes() {
        let report = check("::callout{type=info}\nHi\n::\n");
        assert_eq!(report.fixable_count, 1);
        let clean = check("---\ntitle: T\ntype: doc\n---\n\n::summary\nS.\n::\n");
        assert_eq!(clean.fixable_count, 0);
    }

    #[test]
    fn apply_fixes_clean_input_is_untouched() {
        let input = "---\ntitle: T\ntype: doc\n---\n\n::summary\nShort.\n::\n\n# H\n\nBody.\n";
        let out = apply_fixes(input, FixSafety::Suggested);
        assert_eq!(out.source, input);
        assert_eq!(out.iterations, 0);
        assert!(out.applied.is_empty());
        assert!(out.skipped.is_empty());
    }

    #[test]
    fn apply_fixes_normalises_crlf_like_parse() {
        let out = apply_fixes("::callout{type=info}\r\nHi\r\n::\r\n", FixSafety::Safe);
        assert_eq!(out.source, "::callout[type=info]\nHi\n::\n");
    }

    #[test]
    fn apply_fixes_once_applies_a_single_pass() {
        let input = ":::column\nLeft\n:::\n";
        let once = apply_fixes_once(input, FixSafety::Safe);
        assert_eq!(once.source, "::column\nLeft\n:::\n");
        assert_eq!(once.iterations, 1);
        // Iterating apply_fixes_once by hand reaches the same fixpoint.
        let mut cur = once.source;
        loop {
            let next = apply_fixes_once(&cur, FixSafety::Safe);
            if next.iterations == 0 {
                break;
            }
            cur = next.source;
        }
        assert_eq!(cur, apply_fixes(input, FixSafety::Safe).source);
        assert_eq!(cur, "::column\nLeft\n::\n");
    }

    #[test]
    fn apply_fixes_reports_applied_codes() {
        let out = apply_fixes(
            "::callout{type=info}\nHi\n::\n\n::section[title=\"S\"]\nT.\n::\n",
            FixSafety::Safe,
        );
        let codes: BTreeSet<&str> = out.applied.iter().map(|a| a.code.as_str()).collect();
        assert_eq!(codes, ["L001", "L003"].into_iter().collect());
    }

    #[test]
    fn edits_conflict_rules() {
        let e = |s: usize, t: usize| TextEdit {
            span: Span {
                start_line: 1,
                end_line: 1,
                start_offset: s,
                end_offset: t,
            },
            replacement: String::new(),
        };
        assert!(edits_conflict(&e(0, 5), &e(3, 8)), "overlapping ranges");
        assert!(edits_conflict(&e(4, 4), &e(4, 4)), "same-point insertions");
        assert!(
            edits_conflict(&e(2, 6), &e(2, 2)),
            "insertion at replace start"
        );
        assert!(
            !edits_conflict(&e(0, 4), &e(4, 8)),
            "adjacent ranges are fine"
        );
        assert!(
            !edits_conflict(&e(4, 4), &e(0, 4)),
            "insertion at range end is fine"
        );
    }

    // --- JSON envelope (shared CLI/wasm serialization) ---

    /// Fixture with one fixable L001 warning (`::section` block).
    const JSON_FIXTURE: &str = "---\ntitle: \"T\"\ntype: report\nstatus: active\n---\n\n# Doc\n\n::section[title=\"Background\"]\nBody text.\n::\n";

    #[test]
    fn report_to_json_per_file_shape() {
        let report = check(JSON_FIXTURE);
        assert!(report.fixable_count > 0, "fixture must have a fixable diag");
        let file = report_to_json(Some("a.surf"), &report);
        assert_eq!(file["path"], "a.surf");
        assert_eq!(file["error_count"], report.error_count);
        assert_eq!(file["warning_count"], report.warning_count);
        assert_eq!(file["info_count"], report.info_count);
        assert_eq!(file["fixable_count"], report.fixable_count);
        let diags = file["diagnostics"].as_array().expect("diagnostics array");
        assert_eq!(diags.len(), report.diagnostics.len());
    }

    #[test]
    fn report_to_json_omits_path_when_none() {
        let report = check("# Plain doc\n");
        let file = report_to_json(None, &report);
        assert!(
            file.get("path").is_none(),
            "path key must be omitted for in-memory input"
        );
        assert!(file.get("diagnostics").is_some());
        assert!(file.get("error_count").is_some());
    }

    #[test]
    fn report_to_json_carries_fix_payload_and_span() {
        let report = check(JSON_FIXTURE);
        let file = report_to_json(Some("a.surf"), &report);
        let diags = file["diagnostics"].as_array().unwrap();
        let l001 = diags
            .iter()
            .find(|d| d["code"] == "L001")
            .expect("L001 diagnostic present");
        assert_eq!(l001["severity"], "warning");
        let span = &l001["span"];
        assert!(span["start_line"].as_u64().unwrap() > 0);
        assert!(span.get("start_offset").is_some());
        assert!(span.get("end_offset").is_some());
        let fix = &l001["fix"];
        assert_eq!(fix["safety"], "safe");
        assert!(fix.get("description").is_some());
        let edits = fix["edits"].as_array().expect("fix edits array");
        assert!(!edits.is_empty());
        assert!(edits[0].get("span").is_some());
        assert!(edits[0].get("replacement").is_some());
    }

    #[test]
    fn reports_to_json_envelope_shape_and_summary() {
        let with_warning = check(JSON_FIXTURE);
        let clean =
            check("---\ntitle: \"T\"\ntype: report\nstatus: active\n---\n\n# Doc\n\nClean body.\n");
        let envelope =
            reports_to_json(&[(Some("a.surf"), &with_warning), (Some("b.surf"), &clean)]);
        assert_eq!(envelope["schema_version"], JSON_SCHEMA_VERSION);
        let files = envelope["files"].as_array().expect("files array");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["path"], "a.surf");
        assert_eq!(files[1]["path"], "b.surf");
        let summary = &envelope["summary"];
        assert_eq!(summary["files"], 2);
        assert_eq!(
            summary["error_count"],
            with_warning.error_count + clean.error_count
        );
        assert_eq!(
            summary["warning_count"],
            with_warning.warning_count + clean.warning_count
        );
        assert_eq!(
            summary["info_count"],
            with_warning.info_count + clean.info_count
        );
        assert_eq!(
            summary["fixable_count"],
            with_warning.fixable_count + clean.fixable_count
        );
    }

    #[test]
    fn reports_to_json_empty_input() {
        let envelope = reports_to_json(&[]);
        assert_eq!(envelope["schema_version"], JSON_SCHEMA_VERSION);
        assert_eq!(envelope["files"].as_array().unwrap().len(), 0);
        assert_eq!(envelope["summary"]["files"], 0);
        assert_eq!(envelope["summary"]["error_count"], 0);
    }

    // --- L040 ---

    #[test]
    fn l040_flags_skipped_mermaid_constructs() {
        let input = "---\ntitle: T\ntype: doc\n---\n\n::diagram\nflowchart LR\nsubgraph One\nA --> B\nend\n::\n";
        let diags = run_rule(&MermaidConstructSkipped, input);
        assert_eq!(codes(&diags), vec!["L040", "L040"]); // subgraph + end
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("subgraph"));
        assert!(diags[0].fix.is_none());
    }

    #[test]
    fn l040_silent_for_native_dsl_and_clean_mermaid() {
        // Native DSL bodies are never translated, so never flagged.
        let native = "---\ntitle: T\ntype: doc\n---\n\n::diagram[type=architecture]\nweb: Web\nweb -> api\n::\n";
        assert!(run_rule(&MermaidConstructSkipped, native).is_empty());
        // A fully-translatable mermaid body has no notes.
        let clean = "---\ntitle: T\ntype: doc\n---\n\n::diagram\nsequenceDiagram\nA->>B: hi\n::\n";
        assert!(run_rule(&MermaidConstructSkipped, clean).is_empty());
    }
}

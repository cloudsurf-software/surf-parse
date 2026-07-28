use serde::{Deserialize, Serialize};

use crate::types::Span;

/// Errors that can occur during parsing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid front matter: {message}")]
    FrontMatter { message: String, span: Span },

    #[error("Unclosed block directive '{name}' opened at line {line}")]
    UnclosedBlock { name: String, line: usize },

    #[error("Invalid attribute syntax: {message}")]
    InvalidAttrs { message: String, span: Span },
}

/// A diagnostic message produced during parsing.
///
/// Diagnostics are non-fatal: the parser continues and produces a best-effort
/// result even when diagnostics are emitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Deterministic auto-fix for this diagnostic, when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

/// A single span-based text edit against the original source.
///
/// `span` is BYTE-offset based (`start_offset`/`end_offset`) against the
/// CRLF-normalised source the diagnostics were produced from. Offsets always
/// sit on UTF-8 character boundaries. An empty span (`start_offset ==
/// end_offset`) is an insertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    /// Byte range in the original source to replace.
    pub span: Span,
    /// Text spliced in place of the spanned bytes.
    pub replacement: String,
}

/// Safety tier of a [`Fix`]. Ordered: `Safe < Suggested`, so a requested tier
/// of `Suggested` also applies every `Safe` fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FixSafety {
    /// Mechanical and behaviour-preserving; always safe to auto-apply.
    Safe,
    /// Probably right but involves a judgement call (e.g. a rename based on a
    /// did-you-mean match); applied only when explicitly requested.
    Suggested,
}

/// A deterministic auto-fix: one or more text edits applied to the ORIGINAL
/// source bytes (never an AST round-trip), so untouched bytes are preserved
/// exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fix {
    /// Edits to apply, all anchored to the same original source.
    pub edits: Vec<TextEdit>,
    /// Safety tier controlling when the fix is auto-applied.
    pub safety: FixSafety,
    /// Human-readable description, e.g. `"replace '::section' with '## X'"`.
    pub description: String,
}

/// Severity level for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

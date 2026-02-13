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
}

/// Severity level for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

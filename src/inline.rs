//! Inline extension scanner.
//!
//! Detects `:evidence[...]` and `:status[...]` patterns in text content and
//! returns their byte ranges and parsed `InlineExt` values.

use crate::attrs::parse_attrs;
use crate::types::{AttrValue, InlineExt};

/// Scan `text` for inline extensions (single-colon prefix).
///
/// Returns a vec of `(start_byte, end_byte, InlineExt)` tuples.
/// Double-colon prefixes (`::evidence[...]`) are block directives and are
/// intentionally skipped.
pub fn scan_inline_extensions(text: &str) -> Vec<(usize, usize, InlineExt)> {
    let mut results = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Look for ':' that is NOT preceded by another ':' and NOT followed by another ':'.
        if bytes[pos] == b':' {
            // Skip if preceded by ':'  (double-colon = block directive).
            if pos > 0 && bytes[pos - 1] == b':' {
                pos += 1;
                continue;
            }
            // Skip if followed by ':'  (double-colon = block directive).
            if pos + 1 < len && bytes[pos + 1] == b':' {
                pos += 2;
                continue;
            }

            // Try to match a known extension name starting right after the colon.
            if let Some(ext) = try_parse_extension(text, pos) {
                let end = ext.1;
                results.push(ext);
                // Advance past this extension.
                pos = end;
                continue;
            }
        }
        pos += 1;
    }

    results
}

/// Try to parse an inline extension at position `colon_pos` (the `:` character).
///
/// Returns `Some((start, end, InlineExt))` if a valid extension is found.
fn try_parse_extension(text: &str, colon_pos: usize) -> Option<(usize, usize, InlineExt)> {
    let rest = &text[colon_pos + 1..];

    let (name, after_name) = if let Some(stripped) = rest.strip_prefix("evidence[") {
        ("evidence", stripped)
    } else if let Some(stripped) = rest.strip_prefix("status[") {
        ("status", stripped)
    } else {
        return None;
    };

    // Find the closing bracket.
    let bracket_close = after_name.find(']')?;
    let attr_str = &after_name[..bracket_close];

    // The full extent: from colon through closing bracket.
    // colon_pos + 1 (colon) + name.len() + 1 ([) + bracket_close + 1 (])
    let end_pos = colon_pos + 1 + name.len() + 1 + bracket_close + 1;

    // Parse the bracketed content as attributes.
    let attrs = parse_attrs(attr_str).ok()?;

    match name {
        "evidence" => {
            let tier = attrs.get("tier").and_then(|v| match v {
                AttrValue::Number(n) => Some(*n as u8),
                AttrValue::String(s) => s.parse::<u8>().ok(),
                _ => None,
            });
            let source = attrs.get("source").and_then(|v| match v {
                AttrValue::String(s) => Some(s.clone()),
                _ => None,
            });
            // Use the whole attr string as the text representation.
            Some((
                colon_pos,
                end_pos,
                InlineExt::Evidence {
                    tier,
                    source,
                    text: attr_str.trim().to_string(),
                },
            ))
        }
        "status" => {
            let value = attrs
                .get("value")
                .and_then(|v| match v {
                    AttrValue::String(s) => Some(s.clone()),
                    AttrValue::Bool(b) => Some(b.to_string()),
                    AttrValue::Number(n) => Some(n.to_string()),
                    AttrValue::Null => None,
                })
                .unwrap_or_default();
            Some((colon_pos, end_pos, InlineExt::Status { value }))
        }
        _ => None,
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn scan_evidence_basic() {
        let text = r#"Some text :evidence[tier=1 source="Gartner"] more text"#;
        let results = scan_inline_extensions(text);
        assert_eq!(results.len(), 1);
        match &results[0].2 {
            InlineExt::Evidence { tier, source, .. } => {
                assert_eq!(*tier, Some(1));
                assert_eq!(source.as_deref(), Some("Gartner"));
            }
            other => panic!("Expected Evidence, got {other:?}"),
        }
    }

    #[test]
    fn scan_status_basic() {
        let text = ":status[value=shipped] and done";
        let results = scan_inline_extensions(text);
        assert_eq!(results.len(), 1);
        match &results[0].2 {
            InlineExt::Status { value } => {
                assert_eq!(value, "shipped");
            }
            other => panic!("Expected Status, got {other:?}"),
        }
    }

    #[test]
    fn scan_multiple_inline() {
        let text = r#":status[value=done] and :evidence[tier=2 source="IEEE"] end"#;
        let results = scan_inline_extensions(text);
        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0].2, InlineExt::Status { .. }));
        assert!(matches!(&results[1].2, InlineExt::Evidence { .. }));
    }

    #[test]
    fn scan_no_extensions() {
        let text = "Just plain text with no extensions.";
        let results = scan_inline_extensions(text);
        assert!(results.is_empty());
    }

    #[test]
    fn scan_double_colon_ignored() {
        let text = "::evidence[tier=1] should not match as inline";
        let results = scan_inline_extensions(text);
        assert!(results.is_empty(), "Double-colon should not be matched: {results:?}");
    }
}

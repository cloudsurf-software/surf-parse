use crate::attrs::parse_attrs;
use crate::error::{Diagnostic, Severity};
use crate::types::{Attrs, Block, FrontMatter, Span, SurfDoc};

/// Result of parsing a SurfDoc.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// The parsed document.
    pub doc: SurfDoc,
    /// Non-fatal diagnostics collected during parsing.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a SurfDoc string into a `ParseResult`.
///
/// This function never panics. Malformed input produces diagnostics and a
/// best-effort `SurfDoc`.
pub fn parse(input: &str) -> ParseResult {
    let mut diagnostics = Vec::new();

    // Normalise CRLF → LF.
    let normalised = input.replace("\r\n", "\n");
    let lines: Vec<&str> = normalised.split('\n').collect();

    // ---------------------------------------------------------------
    // Pass 1a: Front matter extraction.
    // ---------------------------------------------------------------
    let (front_matter, body_start_line) = extract_front_matter(&lines, &normalised, &mut diagnostics);

    // ---------------------------------------------------------------
    // Pass 1b: Line-by-line block directive scan.
    // ---------------------------------------------------------------
    let blocks = scan_blocks(&lines, body_start_line, &normalised, &mut diagnostics);

    // ---------------------------------------------------------------
    // Pass 2: Type resolution — convert Unknown blocks to typed variants.
    // ---------------------------------------------------------------
    let blocks = blocks
        .into_iter()
        .map(|block| match block {
            Block::Unknown { .. } => crate::blocks::resolve_block(block),
            other => other,
        })
        .collect();

    ParseResult {
        doc: SurfDoc {
            front_matter,
            blocks,
            source: normalised,
        },
        diagnostics,
    }
}

// ------------------------------------------------------------------
// Front matter
// ------------------------------------------------------------------

/// Try to extract YAML front matter from the beginning of the document.
///
/// Returns `(Option<FrontMatter>, first_body_line_index)`.
fn extract_front_matter(
    lines: &[&str],
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Option<FrontMatter>, usize) {
    if lines.is_empty() || lines[0].trim() != "---" {
        return (None, 0);
    }

    // Find the closing `---`.
    let mut end_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            end_idx = Some(i);
            break;
        }
    }

    let end_idx = match end_idx {
        Some(i) => i,
        None => {
            // No closing `---` — treat the whole thing as body text.
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: "Front matter opened with `---` but never closed".into(),
                span: Some(line_span(0, 0, source)),
                code: Some("E001".into()),
            });
            return (None, 0);
        }
    };

    let yaml_str: String = lines[1..end_idx].join("\n");
    let fm_span = Span {
        start_line: 1,
        end_line: end_idx + 1,
        start_offset: 0,
        end_offset: byte_offset_end_of_line(end_idx, source),
    };

    match serde_yaml::from_str::<FrontMatter>(&yaml_str) {
        Ok(fm) => (Some(fm), end_idx + 1),
        Err(e) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: format!("Failed to parse front matter YAML: {e}"),
                span: Some(fm_span),
                code: Some("E002".into()),
            });
            (None, end_idx + 1)
        }
    }
}

// ------------------------------------------------------------------
// Block scanning
// ------------------------------------------------------------------

/// State for an in-progress block directive on the nesting stack.
struct OpenBlock {
    name: String,
    attrs: Attrs,
    depth: usize, // number of leading colons (2 = top-level, 3 = nested, …)
    start_line: usize, // 1-based
    start_offset: usize,
    content_start_offset: usize, // byte offset right after the opening line
}

/// Scan the body lines for block directives, producing `Block` items.
///
/// Design: only **top-level** blocks (those opened when the stack is empty) are
/// emitted as `Block::Unknown`. Nested directives are tracked for depth
/// matching but their content stays inside the parent block's raw content string.
fn scan_blocks(
    lines: &[&str],
    body_start: usize,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut stack: Vec<OpenBlock> = Vec::new();

    // Track the start of the current "gap" of plain markdown between directives.
    // We only collect markdown gaps at the top nesting level (stack is empty).
    let mut md_start_line: Option<usize> = None; // 0-based index into `lines`
    let mut md_start_offset: Option<usize> = None;

    for (idx, &line) in lines.iter().enumerate().skip(body_start) {
        let trimmed = line.trim();
        let line_offset = byte_offset_start_of_line(idx, source);

        // Check for closing directive: a line that is *only* colons.
        if let Some(close_depth) = closing_directive_depth(trimmed) {
            // Find the innermost matching open block.
            if let Some(pos) = stack.iter().rposition(|b| b.depth == close_depth) {
                // If there are unclosed blocks deeper than `pos`, warn about them.
                while stack.len() > pos + 1 {
                    let orphan = stack.pop().unwrap();
                    diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!(
                            "Unclosed block directive '{}' opened at line {}",
                            orphan.name, orphan.start_line
                        ),
                        span: Some(Span {
                            start_line: orphan.start_line,
                            end_line: idx + 1,
                            start_offset: orphan.start_offset,
                            end_offset: line_offset + line.len(),
                        }),
                        code: Some("W001".into()),
                    });
                }

                let open = stack.pop().unwrap(); // this is the one at `pos`

                // Only emit a block if the stack is now empty (this was top-level).
                if stack.is_empty() {
                    let content = &source[open.content_start_offset..line_offset];
                    let content = content.strip_suffix('\n').unwrap_or(content);

                    blocks.push(Block::Unknown {
                        name: open.name,
                        attrs: open.attrs,
                        content: content.to_string(),
                        span: Span {
                            start_line: open.start_line,
                            end_line: idx + 1,
                            start_offset: open.start_offset,
                            end_offset: line_offset + line.len(),
                        },
                    });

                    md_start_line = None;
                    md_start_offset = None;
                }
                // If stack is not empty, this was a nested close — just pop, no emit.
                continue;
            }
            // No matching open block — fall through and treat as markdown.
        }

        // Check for opening directive: `::name[attrs]`
        if let Some((depth, name, attrs_str)) = opening_directive(trimmed) {
            // If we're at top level, flush any accumulated markdown.
            if stack.is_empty() {
                flush_markdown(
                    &mut blocks,
                    &mut md_start_line,
                    &mut md_start_offset,
                    idx,
                    source,
                );

                let attrs = match parse_attrs(&attrs_str) {
                    Ok(a) => a,
                    Err(e) => {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Warning,
                            message: format!("Invalid attributes on '::{}': {}", name, e),
                            span: Some(line_span(idx, idx, source)),
                            code: Some("W002".into()),
                        });
                        Attrs::new()
                    }
                };

                let content_start = line_offset + line.len() + 1; // +1 for the newline
                let content_start = content_start.min(source.len());

                stack.push(OpenBlock {
                    name,
                    attrs,
                    depth,
                    start_line: idx + 1,
                    start_offset: line_offset,
                    content_start_offset: content_start,
                });
            } else {
                // Inside an existing block — push a nesting tracker.
                // We don't parse attrs for nested blocks in Chunk 1; they stay
                // as raw content of the parent.
                stack.push(OpenBlock {
                    name,
                    attrs: Attrs::new(),
                    depth,
                    start_line: idx + 1,
                    start_offset: line_offset,
                    content_start_offset: 0, // unused for nested
                });
            }
            continue;
        }

        // Regular line — track markdown gap if at top level.
        if stack.is_empty() && md_start_line.is_none() {
            md_start_line = Some(idx);
            md_start_offset = Some(line_offset);
        }
    }

    // Flush any remaining markdown.
    flush_markdown(
        &mut blocks,
        &mut md_start_line,
        &mut md_start_offset,
        lines.len(),
        source,
    );

    // Force-close any remaining open blocks (unclosed at EOF).
    // Only the outermost (bottom of stack) gets emitted; inner ones just get diagnostics.
    while let Some(open) = stack.pop() {
        let eof_offset = source.len();
        let eof_line = lines.len();

        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: format!(
                "Unclosed block directive '{}' opened at line {}",
                open.name, open.start_line
            ),
            span: Some(Span {
                start_line: open.start_line,
                end_line: eof_line,
                start_offset: open.start_offset,
                end_offset: eof_offset,
            }),
            code: Some("W001".into()),
        });

        // Only emit for the outermost block (stack now empty).
        if stack.is_empty() {
            let content = if open.content_start_offset <= eof_offset {
                &source[open.content_start_offset..eof_offset]
            } else {
                ""
            };
            let content = content.strip_suffix('\n').unwrap_or(content);

            blocks.push(Block::Unknown {
                name: open.name,
                attrs: open.attrs,
                content: content.to_string(),
                span: Span {
                    start_line: open.start_line,
                    end_line: eof_line,
                    start_offset: open.start_offset,
                    end_offset: eof_offset,
                },
            });
        }
    }

    blocks
}

/// Flush accumulated markdown lines into a `Block::Markdown`.
fn flush_markdown(
    blocks: &mut Vec<Block>,
    md_start_line: &mut Option<usize>,
    md_start_offset: &mut Option<usize>,
    current_idx: usize,
    source: &str,
) {
    if let (Some(start_idx), Some(start_off)) = (*md_start_line, *md_start_offset) {
        let mut end_idx = current_idx.saturating_sub(1);

        // Walk backwards past trailing empty lines so spans are tight.
        let source_lines: Vec<&str> = source.split('\n').collect();
        while end_idx > start_idx && source_lines.get(end_idx).is_some_and(|l| l.trim().is_empty())
        {
            end_idx -= 1;
        }

        let end_offset = byte_offset_end_of_line(end_idx, source);
        let content = &source[start_off..end_offset];

        // Only emit if there's actual content (not just whitespace).
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            blocks.push(Block::Markdown {
                content: content.to_string(),
                span: Span {
                    start_line: start_idx + 1,
                    end_line: end_idx + 1,
                    start_offset: start_off,
                    end_offset,
                },
            });
        }

        *md_start_line = None;
        *md_start_offset = None;
    }
}

// ------------------------------------------------------------------
// Line classification helpers
// ------------------------------------------------------------------

/// If the line is a closing directive (`::`, `:::`, …), return the depth (colon count).
pub(crate) fn closing_directive_depth(trimmed: &str) -> Option<usize> {
    if trimmed.is_empty() {
        return None;
    }
    // Must be only colons.
    if trimmed.chars().all(|c| c == ':') && trimmed.len() >= 2 {
        Some(trimmed.len())
    } else {
        None
    }
}

/// If the line is an opening directive (`::name[attrs]`), return `(depth, name, attrs_str)`.
pub(crate) fn opening_directive(trimmed: &str) -> Option<(usize, String, String)> {
    if !trimmed.starts_with("::") {
        return None;
    }

    // Count leading colons.
    let depth = trimmed.chars().take_while(|&c| c == ':').count();
    if depth < 2 {
        return None;
    }

    let rest = &trimmed[depth..];
    if rest.is_empty() {
        // This is a closing directive, not an opening one.
        return None;
    }

    // The next character must be alphabetic (block name start).
    let first_char = rest.chars().next()?;
    if !first_char.is_alphabetic() {
        return None;
    }

    // Scan block name.
    let name_end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .unwrap_or(rest.len());
    let name = rest[..name_end].to_string();
    let remainder = &rest[name_end..];

    // Extract attrs if present.
    let attrs_str = if remainder.starts_with('[') {
        if let Some(close) = remainder.find(']') {
            remainder[..=close].to_string()
        } else {
            // Unclosed bracket — take everything.
            remainder.to_string()
        }
    } else {
        String::new()
    };

    Some((depth, name, attrs_str))
}

// ------------------------------------------------------------------
// Byte offset helpers
// ------------------------------------------------------------------

/// Byte offset of the start of line `idx` (0-based) within `source`.
fn byte_offset_start_of_line(idx: usize, source: &str) -> usize {
    let mut offset = 0;
    for (i, line) in source.split('\n').enumerate() {
        if i == idx {
            return offset;
        }
        offset += line.len() + 1; // +1 for '\n'
    }
    source.len()
}

/// Byte offset of the end (exclusive) of line `idx` (0-based) within `source`.
fn byte_offset_end_of_line(idx: usize, source: &str) -> usize {
    let mut offset = 0;
    for (i, line) in source.split('\n').enumerate() {
        offset += line.len();
        if i == idx {
            return offset;
        }
        offset += 1; // '\n'
    }
    source.len()
}

/// Build a `Span` covering lines `start_idx..=end_idx` (0-based).
fn line_span(start_idx: usize, end_idx: usize, source: &str) -> Span {
    Span {
        start_line: start_idx + 1,
        end_line: end_idx + 1,
        start_offset: byte_offset_start_of_line(start_idx, source),
        end_offset: byte_offset_end_of_line(end_idx, source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_empty_input() {
        let result = parse("");
        assert!(result.doc.front_matter.is_none());
        assert!(result.doc.blocks.is_empty());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn parse_plain_markdown() {
        let input = "# Hello\n\nSome text here.\n";
        let result = parse(input);
        assert!(result.doc.front_matter.is_none());
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Markdown { content, .. } => {
                assert!(content.contains("# Hello"));
                assert!(content.contains("Some text here."));
            }
            _ => panic!("Expected Markdown block"),
        }
    }

    #[test]
    fn parse_front_matter() {
        let input = "---\ntitle: Test\n---\n# Hello\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        let fm = result.doc.front_matter.as_ref().unwrap();
        assert_eq!(fm.title.as_deref(), Some("Test"));
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Markdown { content, .. } => {
                assert!(content.contains("# Hello"));
            }
            _ => panic!("Expected Markdown block"),
        }
    }

    #[test]
    fn parse_single_block() {
        let input = "::callout[type=warning]\nDanger!\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Callout {
                callout_type,
                content,
                span,
                ..
            } => {
                assert_eq!(*callout_type, crate::types::CalloutType::Warning);
                assert_eq!(content, "Danger!");
                assert_eq!(span.start_line, 1);
                assert_eq!(span.end_line, 3);
            }
            other => panic!("Expected Callout block, got {other:?}"),
        }
    }

    #[test]
    fn parse_two_blocks() {
        let input = "::callout[type=info]\nFirst\n::\n\nSome markdown.\n\n::data[format=json]\n{}\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 3);

        assert!(matches!(&result.doc.blocks[0], Block::Callout { .. }));
        assert!(matches!(&result.doc.blocks[1], Block::Markdown { .. }));
        assert!(matches!(&result.doc.blocks[2], Block::Data { .. }));
    }

    #[test]
    fn parse_nested_blocks() {
        let input = "::columns\n:::column\nLeft text.\n:::\n:::column\nRight text.\n:::\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Columns { columns, .. } => {
                assert_eq!(columns.len(), 2);
                assert!(columns[0].content.contains("Left text."));
                assert!(columns[1].content.contains("Right text."));
            }
            other => panic!("Expected Columns block, got {other:?}"),
        }
    }

    #[test]
    fn parse_unclosed_block() {
        let input = "::callout[type=warning]\nNo closing marker";
        let result = parse(input);
        assert!(!result.diagnostics.is_empty(), "Expected a diagnostic for unclosed block");
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Callout { content, .. } => {
                assert!(content.contains("No closing marker"));
            }
            other => panic!("Expected Callout block, got {other:?}"),
        }
    }

    #[test]
    fn parse_leaf_directive() {
        let input = "# Title\n\n::metric[label=\"MRR\" value=\"$2K\"]\n\n## More\n";
        let result = parse(input);
        // A leaf directive is a single-line directive with no explicit closing.
        // It should be treated as an unclosed block that captures no content.
        // But actually the parser should detect it as a block that's implicitly
        // closed by the next block or end-of-gap.
        //
        // With our design, the `::metric` opens a block on the stack.
        // The next lines are not closers, so at EOF the block is force-closed.
        // The diagnostic is expected.
        assert_eq!(
            result.doc.blocks.len(),
            2, // Markdown "# Title\n", then Metric (force-closed)
            "blocks: {:#?}", result.doc.blocks
        );
        let has_metric = result.doc.blocks.iter().any(|b| matches!(b, Block::Metric { .. }));
        assert!(has_metric, "Should contain a metric block");
    }

    #[test]
    fn parse_block_spans() {
        let input = "# Title\n::callout\nInside\n::\n# After\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);

        // First block: markdown "# Title\n" → line 1
        match &result.doc.blocks[0] {
            Block::Markdown { span, .. } => {
                assert_eq!(span.start_line, 1);
                assert_eq!(span.end_line, 1);
            }
            _ => panic!("Expected Markdown"),
        }

        // Second block: callout → lines 2-4
        match &result.doc.blocks[1] {
            Block::Callout { span, .. } => {
                assert_eq!(span.start_line, 2);
                assert_eq!(span.end_line, 4);
            }
            other => panic!("Expected Callout, got {other:?}"),
        }

        // Third block: markdown "# After\n" → line 5
        match &result.doc.blocks[2] {
            Block::Markdown { span, .. } => {
                assert_eq!(span.start_line, 5);
                assert_eq!(span.end_line, 5);
            }
            _ => panic!("Expected Markdown"),
        }
    }

    #[test]
    fn parse_front_matter_all_fields() {
        let input = r#"---
title: "Full Document"
type: plan
status: active
scope: workspace
tags: [rust, parser]
created: "2026-02-10"
updated: "2026-02-10"
author: "Brady Davis"
confidence: high
version: 2
workspace: cloudsurf
contributors: ["Claude"]
decision: "Use Rust"
related:
  - path: plans/example.md
    relationship: references
---
Body.
"#;
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        let fm = result.doc.front_matter.as_ref().unwrap();
        assert_eq!(fm.title.as_deref(), Some("Full Document"));
        assert_eq!(fm.doc_type, Some(crate::types::DocType::Plan));
        assert_eq!(fm.status, Some(crate::types::DocStatus::Active));
        assert_eq!(fm.scope, Some(crate::types::Scope::Workspace));
        assert_eq!(fm.tags.as_deref(), Some(&["rust".to_string(), "parser".to_string()][..]));
        assert_eq!(fm.created.as_deref(), Some("2026-02-10"));
        assert_eq!(fm.updated.as_deref(), Some("2026-02-10"));
        assert_eq!(fm.author.as_deref(), Some("Brady Davis"));
        assert_eq!(fm.confidence, Some(crate::types::Confidence::High));
        assert_eq!(fm.version, Some(2));
        assert_eq!(fm.workspace.as_deref(), Some("cloudsurf"));
        assert_eq!(fm.decision.as_deref(), Some("Use Rust"));
        let related = fm.related.as_ref().unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].path, "plans/example.md");
    }

    #[test]
    fn parse_unknown_front_matter_fields() {
        let input = "---\ntitle: Test\ncustom_field: hello\nanother: 42\n---\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        let fm = result.doc.front_matter.as_ref().unwrap();
        assert_eq!(fm.title.as_deref(), Some("Test"));
        assert!(fm.extra.contains_key("custom_field"), "extra should contain custom_field");
        assert!(fm.extra.contains_key("another"), "extra should contain another");
    }

    // ------------------------------------------------------------------
    // Chunk 2 integration tests — end-to-end through Pass 2 resolution.
    // ------------------------------------------------------------------

    #[test]
    fn parse_callout_end_to_end() {
        let input = "::callout[type=warning]\nWatch out for sharp edges.\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Callout {
                callout_type,
                content,
                span,
                ..
            } => {
                assert_eq!(*callout_type, crate::types::CalloutType::Warning);
                assert_eq!(content, "Watch out for sharp edges.");
                assert_eq!(span.start_line, 1);
                assert_eq!(span.end_line, 3);
            }
            other => panic!("Expected Callout block, got {other:?}"),
        }
    }

    #[test]
    fn parse_metric_end_to_end() {
        let input = "::metric[label=\"MRR\" value=\"$2K\"]\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Metric {
                label,
                value,
                trend,
                ..
            } => {
                assert_eq!(label, "MRR");
                assert_eq!(value, "$2K");
                assert!(trend.is_none());
            }
            other => panic!("Expected Metric block, got {other:?}"),
        }
    }

    #[test]
    fn parse_mixed_typed_blocks() {
        let input = concat!(
            "::callout[type=info]\nFYI\n::\n",
            "\n# Some Markdown\n\n",
            "::data[format=csv]\nA, B\n1, 2\n::\n",
        );
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 3, "blocks: {:#?}", result.doc.blocks);

        assert!(matches!(&result.doc.blocks[0], Block::Callout { .. }));
        assert!(matches!(&result.doc.blocks[1], Block::Markdown { .. }));
        match &result.doc.blocks[2] {
            Block::Data {
                format,
                headers,
                rows,
                ..
            } => {
                assert_eq!(*format, crate::types::DataFormat::Csv);
                assert_eq!(headers, &["A", "B"]);
                assert_eq!(rows.len(), 1);
            }
            other => panic!("Expected Data block, got {other:?}"),
        }
    }
}

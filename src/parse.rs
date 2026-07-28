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
                code: Some("P002".into()),
                fix: None,
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
            // Distinguish genuinely broken YAML (P002, error) from valid YAML
            // whose values fail the closed schema enums, e.g. `type:
            // checkpoint` (P005, warning). Recovery is identical either way:
            // no front matter, body starts after the closing `---` — the
            // SurfDoc produced does not depend on which code fired.
            let p005 = serde_yaml::from_str::<serde_yaml::Value>(&yaml_str)
                .ok()
                .as_ref()
                .and_then(offending_front_matter_field);
            let diagnostic = match p005 {
                Some((field, value)) => Diagnostic {
                    severity: Severity::Warning,
                    message: p005_message(&field, &value),
                    span: Some(fm_span),
                    code: Some("P005".into()),
                    fix: None,
                },
                None => Diagnostic {
                    severity: Severity::Error,
                    message: format!("Failed to parse front matter YAML: {e}"),
                    span: Some(fm_span),
                    code: Some("P002".into()),
                    fix: None,
                },
            };
            diagnostics.push(diagnostic);
            (None, end_idx + 1)
        }
    }
}

/// Message for a P005 diagnostic (front matter value outside the closed
/// schema enums). Shared with the lint layer, which reconstructs it verbatim
/// to suppress values allowed via `LintConfig::extra_frontmatter_values`.
pub(crate) fn p005_message(field: &str, value: &str) -> String {
    format!("Front matter field '{field}' has unrecognized value '{value}'")
}

/// First front matter field whose value fails the typed [`FrontMatter`]
/// schema, found by probing one `key: value` pair at a time. `None` when the
/// failure cannot be pinned to a single field (caller falls back to P002).
fn offending_front_matter_field(value: &serde_yaml::Value) -> Option<(String, String)> {
    let map = value.as_mapping()?;
    for (k, v) in map {
        let Some(key) = k.as_str() else { continue };
        let mut single = serde_yaml::Mapping::new();
        single.insert(k.clone(), v.clone());
        if serde_yaml::from_value::<FrontMatter>(serde_yaml::Value::Mapping(single)).is_err() {
            return Some((key.to_string(), render_yaml_value(v)));
        }
    }
    None
}

/// Compact single-line rendering of a YAML value for diagnostics.
fn render_yaml_value(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        other => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim_end()
            .replace('\n', " "),
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

    // Markdown code-fence state (``` / ~~~), mirroring lint.rs::scan_lines
    // literal semantics. Guards ONLY the newer tolerant behaviors below
    // (space-separated openers, orphan-closer consumption) so fenced SurfDoc
    // examples in prose stay literal; the pre-existing strict-opener path is
    // deliberately left non-fence-aware to avoid behavior churn.
    let mut in_fence = false;

    for (idx, &line) in lines.iter().enumerate().skip(body_start) {
        let trimmed = line.trim();
        let line_offset = byte_offset_start_of_line(idx, source);

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }

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
                        code: Some("P001".into()),
                        fix: None,
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
            // No matching open block. Outside fences, at top level, consume
            // the orphan closer with a P006 warning instead of leaking a
            // literal `::` paragraph into rendered output (mirrors the
            // orphan-closer skip in blocks.rs::parse_page_children). Inside a
            // fence it stays literal markdown; inside an open block it stays
            // part of the parent's raw content.
            if !in_fence && stack.is_empty() {
                // Close out any pending markdown gap first so the consumed
                // line can't be swept into a later gap's span.
                flush_markdown(
                    &mut blocks,
                    &mut md_start_line,
                    &mut md_start_offset,
                    idx,
                    source,
                );
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    message: format!("Closing directive with no open block at line {}", idx + 1),
                    span: Some(line_span(idx, idx, source)),
                    code: Some("P006".into()),
                    fix: None,
                });
                continue;
            }
            // Fall through and treat as markdown.
        }

        // Check for opening directive: `::name[attrs]`. The tolerant
        // space-separated form (`:: name`) is ignored inside fences so code
        // samples keep their literal lines; the strict form keeps its
        // pre-existing non-fence-aware behavior.
        let opener = opening_directive(trimmed).filter(|(depth, _, _)| {
            !in_fence || directive_name_start(trimmed, *depth) == *depth
        });
        if let Some((depth, name, attrs_str)) = opener {
            // Greedy page boundary: when a ::page or ::footer arrives and the
            // stack bottom is a ::page at the same depth, force-close the
            // current page.  Leaf directives inside pages (::hero-image, ::cta,
            // ::embed, etc.) don't have closers and steal the page's own `::`
            // markers via depth matching.  This ensures each ::page is emitted
            // as a top-level block regardless of inner leaf nesting.
            if !stack.is_empty()
                && (name == "page" || name == "footer")
                && stack[0].depth == depth
                && stack[0].name == "page"
            {
                // Discard unclosed nested blocks — they are raw content.
                while stack.len() > 1 {
                    stack.pop();
                }
                let open = stack.pop().unwrap();

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
                        end_offset: line_offset,
                    },
                });

                md_start_line = None;
                md_start_offset = None;
            }

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
                            code: Some("P003".into()),
                            fix: None,
                        });
                        Attrs::new()
                    }
                };

                // Leaf-aware: a closer-less top-level directive (e.g. ::metric,
                // ::badge, ::cta) has no matching `::`. If we pushed it onto the
                // stack it would steal the closer meant for a following sibling
                // block and swallow the rest of the document. Detect this by
                // looking ahead: if a same-depth sibling opening directive
                // appears before any matching closer, emit immediately as a leaf
                // and do NOT push. (At EOF with neither closer nor sibling we
                // keep the old behavior: push + force-close as an unclosed
                // container with a P001 diagnostic.)
                //
                // ::page / ::footer are excluded: they are containers whose
                // child leaf directives (::hero-image, ::cta, …) sit at the SAME
                // depth as the page itself, so generic sibling detection would
                // misclassify a page as a leaf. The dedicated greedy page-boundary
                // logic above handles their closing instead.
                let is_page_family = name == "page" || name == "footer";
                if !is_page_family && is_leaf_before_sibling(lines, idx + 1, depth) {
                    blocks.push(Block::Unknown {
                        name,
                        attrs,
                        content: String::new(),
                        span: line_span(idx, idx, source),
                    });

                    md_start_line = None;
                    md_start_offset = None;
                    continue;
                }

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
            code: Some("P001".into()),
            fix: None,
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

/// Returns true if a top-level opening directive at `depth` (whose line is at
/// `after_idx - 1`) is a LEAF: a same-depth sibling opening directive appears
/// before any matching closer. Returns false if its own closer comes first
/// (container) OR if neither is found before EOF (preserve existing unclosed-
/// container force-close behavior at EOF).
///
/// Mirrors the sibling-detection semantics of
/// `crate::blocks::scan_container_close`.
fn is_leaf_before_sibling(lines: &[&str], after_idx: usize, depth: usize) -> bool {
    let mut nesting = 0usize;
    for line in lines.iter().skip(after_idx) {
        let trimmed = line.trim();
        if let Some(close_depth) = closing_directive_depth(trimmed) {
            if nesting == 0 && close_depth == depth {
                return false; // own closer first → container
            }
            if nesting > 0 {
                nesting -= 1;
            }
            continue;
        }
        if let Some((nd, _, _)) = opening_directive(trimmed) {
            if nd == depth && nesting == 0 {
                return true; // sibling first → leaf
            }
            if nd > depth {
                nesting += 1;
            }
        }
    }
    false // EOF, no closer, no sibling → keep old behavior (push; force-closed at EOF)
}

/// If the line is a closing directive (`::`, `:::`, …), return the depth (colon count).
///
/// Public since 0.10.0: source-scanning consumers (e.g. span re-scan passes
/// in downstream tooling) pair this with [`opening_directive`].
pub fn closing_directive_depth(trimmed: &str) -> Option<usize> {
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
///
/// Tolerates spaces/tabs between the colons and the block name (`:: hero[...]`)
/// — a common generated-markup slip that previously leaked the whole directive
/// line into rendered output as literal text. Callers that do offset math on
/// the line must use [`directive_name_start`] rather than assuming the name
/// begins at `depth`.
pub fn opening_directive(trimmed: &str) -> Option<(usize, String, String)> {
    if !trimmed.starts_with("::") {
        return None;
    }

    // Count leading colons.
    let depth = trimmed.chars().take_while(|&c| c == ':').count();
    if depth < 2 {
        return None;
    }

    let rest = trimmed[depth..].trim_start_matches([' ', '\t']);
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

    // Extract attrs if present. Find the closing bracket quote- and depth-aware
    // so a `]` inside a quoted attribute value (e.g. a leftover `[[wikilink]]`
    // in a figure caption) doesn't truncate the block — which previously made
    // `parse_attrs` fail and silently drop every attribute (notably `src`,
    // leaving images blank).
    let attrs_str = if remainder.starts_with('[') {
        if let Some(close) = find_attr_close(remainder) {
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

/// Byte offset (within `trimmed`) at which an opening directive's block name
/// starts: `depth` plus any tolerated spaces/tabs between the colons and the
/// name. Lint rules that slice the line relative to the directive must use
/// this instead of assuming the name begins at `depth`.
pub(crate) fn directive_name_start(trimmed: &str, depth: usize) -> usize {
    let after = &trimmed[depth..];
    depth + (after.len() - after.trim_start_matches([' ', '\t']).len())
}

/// Find the byte index of the `]` that closes the attribute block opened by the
/// leading `[` of `s`. Quote-aware (a `]` inside `"..."` is ignored, honoring
/// `\"` escapes) and depth-aware (balances nested `[...]`, e.g. an unquoted
/// `[[wikilink]]`). Returns `None` if no balanced close exists.
pub(crate) fn find_attr_close(s: &str) -> Option<usize> {
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
            b'[' => depth += 1,
            b']' => {
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
    fn find_attr_close_is_quote_and_depth_aware() {
        // Plain.
        assert_eq!(find_attr_close("[a=b]"), Some(4));
        // `]` inside a quoted value must be ignored.
        let s = r#"[src="u" caption="x ] y"]"#;
        assert_eq!(find_attr_close(s), Some(s.len() - 1));
        // Nested unquoted brackets (leftover [[wikilink]]) are balanced.
        let s2 = r#"[src="u" caption="HQ [[Mountain View]] 2009"]"#;
        assert_eq!(find_attr_close(s2), Some(s2.len() - 1));
        // Escaped quote inside a value doesn't end the quote early.
        let s3 = r#"[k="say \"hi\" ]now"]"#;
        assert_eq!(find_attr_close(s3), Some(s3.len() - 1));
        // Unterminated.
        assert_eq!(find_attr_close("[a=b"), None);
    }

    #[test]
    fn opening_directive_keeps_attrs_with_bracket_in_caption() {
        // Regression: a `]` inside the caption used to truncate the block, so
        // parse_attrs failed and dropped `src`, blanking the image.
        let line = r#"::figure[src="https://x/y.jpg" caption="HQ in [[Mountain View, California]], 2009"]"#;
        let (_, name, attrs_str) = opening_directive(line).expect("directive parses");
        assert_eq!(name, "figure");
        let attrs = parse_attrs(&attrs_str).expect("attrs parse");
        let str_attr = |k: &str| match attrs.get(k) {
            Some(crate::types::AttrValue::String(s)) => Some(s.clone()),
            _ => None,
        };
        assert_eq!(str_attr("src").as_deref(), Some("https://x/y.jpg"));
        assert!(
            str_attr("caption").unwrap_or_default().contains("Mountain View"),
            "caption preserved"
        );
    }

    #[test]
    fn figure_with_wikilink_caption_renders_img_src() {
        // End-to-end: the rendered fragment must carry the real image src.
        let src = "::figure[src=\"https://upload.wikimedia.org/x.jpg\" caption=\"HQ in [[Mountain View, California]], 2009\"]\n";
        let frag = parse(src).doc.to_html_fragment();
        assert!(
            frag.contains(r#"src="https://upload.wikimedia.org/x.jpg""#),
            "img src present in fragment: {frag}"
        );
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
    fn p005_recovery_is_identical_to_p002_recovery() {
        // The P002/P005 split changes ONLY the diagnostic. Both subcases must
        // recover exactly as before: no front matter, body starts after the
        // closing `---`.
        let p005_input = "---\ntitle: T\ntype: checkpoint\n---\n# Heading\n\nBody.\n";
        let p002_input = "---\ntitle: [broken\n---\n# Heading\n\nBody.\n";
        for (input, code) in [(p005_input, "P005"), (p002_input, "P002")] {
            let result = parse(input);
            assert!(result.doc.front_matter.is_none(), "{code}: fm stays None");
            assert_eq!(result.diagnostics.len(), 1, "{code}: one diagnostic");
            assert_eq!(result.diagnostics[0].code.as_deref(), Some(code));
            assert_eq!(result.doc.blocks.len(), 1, "{code}: body parsed");
            match &result.doc.blocks[0] {
                Block::Markdown { content, .. } => {
                    assert!(content.contains("# Heading"), "{code}");
                    assert!(content.contains("Body."), "{code}");
                }
                other => panic!("{code}: expected Markdown block, got {other:?}"),
            }
        }
        // Severity split: P005 warning, P002 error.
        assert_eq!(parse(p005_input).diagnostics[0].severity, Severity::Warning);
        assert_eq!(parse(p002_input).diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn p005_names_the_offending_field_and_value() {
        let result = parse("---\ntitle: T\nstatus: superseded\n---\nBody.\n");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code.as_deref(), Some("P005"));
        assert_eq!(
            result.diagnostics[0].message,
            "Front matter field 'status' has unrecognized value 'superseded'"
        );
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

    // ------------------------------------------------------------------
    // Multipage parsing tests — greedy page boundary.
    // ------------------------------------------------------------------

    #[test]
    fn parse_multipage_with_leaf_directives() {
        // Pages contain leaf directives (::hero-image, ::cta) that don't have
        // closers.  The parser must still emit each page as a separate block.
        let input = concat!(
            "::site\nname: Test\n::\n",
            "::page[route=\"/\" title=\"Home\"]\n",
            "# Welcome\n",
            "::hero-image[src=\"bg.jpg\"]\n",
            "::cta[label=\"Go\" href=\"/about\"]\n",
            "::\n",
            "::page[route=\"/about\" title=\"About\"]\n",
            "# About Us\n",
            "::\n",
            "::footer\n(c) 2026\n::\n",
        );
        let result = parse(input);
        let pages: Vec<_> = result
            .doc
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Page { .. }))
            .collect();
        assert_eq!(pages.len(), 2, "Expected 2 pages, got {} — blocks: {:#?}", pages.len(), result.doc.blocks);
        match &pages[0] {
            Block::Page { route, title, .. } => {
                assert_eq!(route, "/");
                assert_eq!(title.as_deref(), Some("Home"));
            }
            _ => unreachable!(),
        }
        match &pages[1] {
            Block::Page { route, title, .. } => {
                assert_eq!(route, "/about");
                assert_eq!(title.as_deref(), Some("About"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn parse_multipage_children_resolved() {
        // Verify that children inside pages are correctly resolved even when
        // leaf directives are present.
        let input = concat!(
            "::page[route=\"/\"]\n",
            "::hero-image[src=\"bg.jpg\"]\n",
            "::columns\n:::column\nLeft\n:::\n:::column\nRight\n:::\n::\n",
            "::cta[label=\"Click\"]\n",
            "::\n",
            "::page[route=\"/next\"]\n",
            "# Next Page\n",
            "::\n",
        );
        let result = parse(input);
        let pages: Vec<_> = result
            .doc
            .blocks
            .iter()
            .filter(|b| matches!(b, Block::Page { .. }))
            .collect();
        assert_eq!(pages.len(), 2, "blocks: {:#?}", result.doc.blocks);

        // First page should have children: hero-image, columns, cta
        if let Block::Page { children, .. } = &pages[0] {
            let has_columns = children.iter().any(|b| matches!(b, Block::Columns { .. }));
            assert!(has_columns, "First page should contain a Columns block, got: {:#?}", children);
        }
    }

    #[test]
    fn parse_multipage_footer_closes_last_page() {
        // A ::footer should force-close the preceding ::page.
        let input = concat!(
            "::page[route=\"/\"]\n",
            "# Home\n",
            "::hero-image[src=\"bg.jpg\"]\n",
            "::\n",
            "::footer\n(c) 2026\n::\n",
        );
        let result = parse(input);
        let page_count = result.doc.blocks.iter().filter(|b| matches!(b, Block::Page { .. })).count();
        let footer_count = result.doc.blocks.iter().filter(|b| matches!(b, Block::Footer { .. })).count();
        assert_eq!(page_count, 1, "blocks: {:#?}", result.doc.blocks);
        assert_eq!(footer_count, 1, "blocks: {:#?}", result.doc.blocks);
    }

    #[test]
    fn parse_five_page_site() {
        // Regression test modelled on the Bowties Tuxedo .surf file.
        let input = concat!(
            "::site\nname: Bowties\n::\n",
            "::nav[logo=\"Bowties\"]\n- [Home](/)\n- [Gallery](/gallery)\n::\n",
            "::page[route=\"/\" title=\"Home\"]\n",
            "::hero-image[src=\"hero.jpg\"]\n",
            "::cta[label=\"View\" href=\"/gallery\"]\n",
            "::columns\n:::column\nA\n:::\n:::column\nB\n:::\n::\n",
            "::testimonial[author=\"Review\"]\nGreat!\n::\n",
            "::cta[label=\"Go\" href=\"/contact\"]\n",
            "::\n",
            "::page[route=\"/gallery\" title=\"Gallery\"]\n",
            "# Gallery\n",
            "::gallery\n![A](a.jpg) Caption A\n::\n",
            "::cta[label=\"Book\" href=\"/contact\"]\n",
            "::\n",
            "::page[route=\"/services\" title=\"Services\"]\n",
            "# Services\n",
            "::\n",
            "::page[route=\"/measurements\" title=\"Measurements\"]\n",
            "::form[submit=\"Submit\"]\n- Name (text) *\n::\n",
            "::\n",
            "::page[route=\"/contact\" title=\"Contact\"]\n",
            "# Contact\n",
            "::embed[src=\"https://maps.google.com\"]\n",
            "::form[submit=\"Send\"]\n- Name (text) *\n::\n",
            "::\n",
            "::footer\n## Links\n- [Home](/)\n::\n",
        );
        let result = parse(input);
        let pages: Vec<_> = result
            .doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Page { route, .. } => Some(route.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            pages,
            vec!["/", "/gallery", "/services", "/measurements", "/contact"],
            "Expected 5 pages, got {:?} — full blocks: {:#?}",
            pages,
            result.doc.blocks
        );
    }

    #[test]
    fn parse_utf8_content_no_panic() {
        // Regression: verify byte offset calculation handles multibyte UTF-8
        let input = "# Café ☕\n\n::callout[type=info]\nBienvenue à notre café!\n::\n\n日本語テスト\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        // Verify the markdown block captured the UTF-8 heading
        let md = result.doc.blocks.iter().find(|b| matches!(b, Block::Markdown { .. }));
        assert!(md.is_some(), "Expected markdown block with UTF-8 heading");
        if let Some(Block::Markdown { content, .. }) = md {
            assert!(content.contains("Café ☕"), "UTF-8 heading not captured");
        }
    }

    #[test]
    fn parse_utf8_in_block_directives() {
        // Test non-ASCII in block attributes and content
        let input = "::callout[type=info]\nDas ist ein Prüfung mit Ümlauten: äöüß\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 1);
    }

    #[test]
    fn parse_utf8_in_page_routes_and_titles() {
        let input = concat!(
            "::site\nname = \"カフェ\"\n::\n",
            "::page[route=\"/\" title=\"ホーム\"]\n# ホーム\n::\n",
            "::page[route=\"/über-uns\"]\n# Über Uns\n::\n",
        );
        let result = parse(input);
        let pages: Vec<_> = result
            .doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Page { route, .. } => Some(route.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(pages, vec!["/", "/über-uns"]);
    }

    #[test]
    fn parse_emoji_heavy_content() {
        // Stress test: every line has multibyte chars
        let input = "# 🚀 Launch Day\n\n::callout[type=tip]\n💡 Remember: 大事なこと\n::\n\n🎉 We did it! 🎊\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert!(result.doc.blocks.len() >= 2, "Expected at least 2 blocks");
    }

    #[test]
    fn parse_duplicate_page_routes() {
        // Two pages with identical routes — second should still parse
        // (validation should catch this, not the parser)
        let input = concat!(
            "::site\nname = \"Test\"\n::\n",
            "::page[route=\"/\"]\n# Home v1\n::\n",
            "::page[route=\"/\"]\n# Home v2\n::\n",
        );
        let result = parse(input);
        let pages: Vec<_> = result
            .doc
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Page { route, .. } => Some(route.as_str()),
                _ => None,
            })
            .collect();
        // Parser emits both — validation should flag the duplicate
        assert_eq!(pages.len(), 2, "Parser should emit both duplicate pages");
    }

    // ----------------------------------------------------------------
    // Regression: leaf directives must not swallow following siblings.
    // ----------------------------------------------------------------

    #[test]
    fn leaf_then_container_then_leaf_all_parse() {
        // Bug repro: a closer-less leaf (::metric) followed by a real
        // container (::decision) and another leaf (::tasks) used to swallow
        // everything after the metric. All three must now appear in order.
        let input = concat!(
            "::metric[label=\"x\" value=\"42K\"]\n",
            "\n",
            "::decision[status=accepted]\n",
            "x\n",
            "::\n",
            "\n",
            "::tasks\n",
        );
        let result = parse(input);
        let names: Vec<&str> = result
            .doc
            .blocks
            .iter()
            .map(|b| match b {
                Block::Metric { .. } => "metric",
                Block::Decision { .. } => "decision",
                Block::Tasks { .. } => "tasks",
                _ => "other",
            })
            .collect();
        assert_eq!(
            names,
            vec!["metric", "decision", "tasks"],
            "blocks: {:?}",
            result.doc.blocks
        );
    }

    #[test]
    fn consecutive_leaves_then_container_all_parse() {
        // A run of consecutive closer-less leaf directives followed by a real
        // container — every block must be present.
        let input = concat!(
            "::badge[value=\"New\"]\n",
            "\n",
            "::divider[label=\"sec\"]\n",
            "\n",
            "::cta[label=\"y\"]\n",
            "\n",
            "::callout[type=info]\n",
            "body text\n",
            "::\n",
        );
        let result = parse(input);
        assert!(
            result.doc.blocks.iter().any(|b| matches!(b, Block::Badge { .. })),
            "missing Badge; blocks: {:?}",
            result.doc.blocks
        );
        assert!(
            result.doc.blocks.iter().any(|b| matches!(b, Block::Divider { .. })),
            "missing Divider; blocks: {:?}",
            result.doc.blocks
        );
        assert!(
            result.doc.blocks.iter().any(|b| matches!(b, Block::Cta { .. })),
            "missing Cta; blocks: {:?}",
            result.doc.blocks
        );
        match result.doc.blocks.iter().find(|b| matches!(b, Block::Callout { .. })) {
            Some(Block::Callout { content, .. }) => {
                assert!(content.contains("body text"), "callout body lost: {content:?}");
            }
            _ => panic!("missing Callout; blocks: {:?}", result.doc.blocks),
        }
    }

    #[test]
    fn normal_container_keeps_body_intact() {
        // Guard against over-eager leaf classification: a genuine container
        // with a body must still parse with its content intact.
        let input = "::callout[type=warning]\nbody\n::\n";
        let result = parse(input);
        assert!(result.diagnostics.is_empty(), "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.doc.blocks.len(), 1);
        match &result.doc.blocks[0] {
            Block::Callout { content, .. } => assert_eq!(content, "body"),
            other => panic!("Expected Callout, got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "native")]
    fn gfm_pipe_table_in_markdown_becomes_data_table() {
        // End-to-end: a GFM pipe table embedded in plain markdown parses as a
        // Markdown block, then expands to a native DataTable on conversion.
        let input = "Lead text.\n\n| Col A | Col B |\n|-------|-------|\n| 1 | 2 |\n";
        let doc = parse(input).doc;
        // The parser keeps it as a single Markdown block (tables are markdown).
        assert!(doc
            .blocks
            .iter()
            .any(|b| matches!(b, Block::Markdown { .. })));
        // The native conversion lifts the table out.
        let native = crate::render_native::to_native_blocks(&doc);
        let table = native.iter().find_map(|b| match b {
            crate::render_native::NativeBlock::DataTable { headers, rows, .. } => {
                Some((headers.clone(), rows.clone()))
            }
            _ => None,
        });
        let (headers, rows) = table.expect("expected a DataTable from the pipe table");
        assert_eq!(headers, vec!["Col A".to_string(), "Col B".to_string()]);
        assert_eq!(rows, vec![vec!["1".to_string(), "2".to_string()]]);
    }

    #[test]
    #[cfg(feature = "native")]
    fn pipes_without_delimiter_stay_paragraph() {
        // A paragraph containing pipes but no delimiter row must NOT become a
        // table — it stays plain markdown.
        let input = "Run `a | b | c` in the shell.\n";
        let doc = parse(input).doc;
        let native = crate::render_native::to_native_blocks(&doc);
        assert!(!native
            .iter()
            .any(|b| matches!(b, crate::render_native::NativeBlock::DataTable { .. })));
        assert!(native
            .iter()
            .any(|b| matches!(b, crate::render_native::NativeBlock::Markdown { .. })));
    }

    #[test]
    fn leaf_directive_as_final_block_at_eof_emits() {
        // A leaf with no closer and no following sibling at EOF still emits.
        let input = "# Heading\n\n::metric[label=\"x\" value=\"42K\"]\n";
        let result = parse(input);
        assert!(
            result.doc.blocks.iter().any(|b| matches!(b, Block::Metric { .. })),
            "missing Metric at EOF; blocks: {:?}",
            result.doc.blocks
        );
    }

    // ----------------------------------------------------------------
    // p3-copy-sweep: tolerant openers + orphan-closer consumption.
    // ----------------------------------------------------------------

    #[test]
    fn opening_directive_tolerates_space_after_colons() {
        // `:: hero[title="X"]` (space between colons and name) opens a hero
        // block instead of leaking the directive line as body text.
        let input = ":: hero[title=\"About Us\" align=center]\n# Welcome\n::\n";
        let result = parse(input);
        assert_eq!(result.doc.blocks.len(), 1, "blocks: {:#?}", result.doc.blocks);
        match &result.doc.blocks[0] {
            Block::Hero { headline, .. } => {
                assert!(
                    headline.as_deref().unwrap_or_default().contains("Welcome"),
                    "headline: {headline:?}"
                );
            }
            other => panic!("Expected Hero block, got {other:?}"),
        }
        // Unit-level: the helper itself.
        let (depth, name, attrs) = opening_directive(":: hero[title=\"X\"]").unwrap();
        assert_eq!((depth, name.as_str(), attrs.as_str()), (2, "hero", "[title=\"X\"]"));
        assert_eq!(directive_name_start(":: hero", 2), 3);
        assert_eq!(directive_name_start("::hero", 2), 2);

        // Inside a markdown fence the tolerant form stays literal.
        let fenced = "```\n:: hero\n```\n";
        let result = parse(fenced);
        assert!(
            !result.doc.blocks.iter().any(|b| matches!(b, Block::Hero { .. })),
            "fenced tolerant opener must not open a block: {:#?}",
            result.doc.blocks
        );
        match &result.doc.blocks[0] {
            Block::Markdown { content, .. } => assert!(content.contains(":: hero")),
            other => panic!("Expected Markdown block, got {other:?}"),
        }
    }

    #[test]
    fn orphan_top_level_closer_consumed_p006() {
        // A stray top-level `::` (double-close / trailing closer) is consumed
        // with a P006 warning instead of rendering as a literal `::` paragraph.
        let input = "::callout[type=info]\nHi\n::\n::\n\nAfter text.\n";
        let result = parse(input);
        let p006: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.as_deref() == Some("P006"))
            .collect();
        assert_eq!(p006.len(), 1, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(p006[0].severity, Severity::Warning);
        for block in &result.doc.blocks {
            if let Block::Markdown { content, .. } = block {
                assert!(
                    !content.lines().any(|l| l.trim() == "::"),
                    "orphan closer leaked into markdown: {content:?}"
                );
                assert!(content.contains("After text."), "copy survives");
            }
        }
        // Inside a markdown fence the `::` line survives verbatim (code sample).
        let fenced = "```\n::\n```\n";
        let result = parse(fenced);
        assert!(
            result.diagnostics.iter().all(|d| d.code.as_deref() != Some("P006")),
            "no P006 for a fenced closer: {:?}",
            result.diagnostics
        );
        match &result.doc.blocks[0] {
            Block::Markdown { content, .. } => assert!(content.contains("::")),
            other => panic!("Expected Markdown block, got {other:?}"),
        }
    }
}

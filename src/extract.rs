//! Code extraction API for SurfDoc-as-Code workflows.
//!
//! Extracts code blocks from a parsed `SurfDoc` with optional language
//! filtering and alias normalization. Used by CLI tools (`surf extract`)
//! and CI pipelines to pull compilable snippets from documentation.

use crate::types::Block;

/// A code block extracted from a SurfDoc, with source location metadata.
///
/// Produced by [`SurfDoc::extract_code()`] and [`SurfDoc::extract_code_by_lang()`].
/// Fields are owned strings suitable for serialization or file writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedCode {
    /// Language tag (e.g., "rust", "swift", "typescript").
    ///
    /// Set to `""` (empty string) when the source code block has no
    /// `[lang=...]` attribute.
    pub language: String,

    /// Optional file path hint from the `[file=...]` attribute.
    ///
    /// **Security note:** This value is user-authored and untrusted.
    /// Consumers MUST sanitize before using as a filesystem path —
    /// reject `..`, absolute paths, and path traversal sequences.
    pub file_path: Option<String>,

    /// The raw code content between the `::code` markers.
    ///
    /// Byte-identical to `Block::Code.content`. No trimming, no
    /// transformation. Trailing newline behavior matches the parser.
    pub content: String,

    /// 0-based index of this block among all blocks in the document.
    ///
    /// Use for diagnostics ("code block at index 5") or for correlating
    /// back to `SurfDoc.blocks[block_index]`.
    pub block_index: usize,
}

/// Normalize a language identifier to its canonical form.
///
/// Case-insensitive. Maps common aliases to their full names:
/// - `"rs"` -> `"rust"`
/// - `"ts"` -> `"typescript"`
/// - `"js"` -> `"javascript"`
/// - `"py"` -> `"python"`
/// - `"rb"` -> `"ruby"`
/// - `"sh"` / `"bash"` / `"zsh"` -> `"shell"`
/// - `"yml"` -> `"yaml"`
///
/// All other values are lowercased and trimmed.
///
/// This function is idempotent: `normalize_lang(normalize_lang(s)) == normalize_lang(s)`.
pub fn normalize_lang(lang: &str) -> String {
    match lang.to_lowercase().trim() {
        "rs" => "rust".to_string(),
        "ts" => "typescript".to_string(),
        "js" => "javascript".to_string(),
        "py" => "python".to_string(),
        "rb" => "ruby".to_string(),
        "sh" | "bash" | "zsh" => "shell".to_string(),
        "yml" => "yaml".to_string(),
        other => other.to_string(),
    }
}

/// Extract all `Block::Code` variants from a block slice.
///
/// Returns `ExtractedCode` items in document order. This is the
/// workhorse function; the `SurfDoc` methods delegate here.
pub fn extract_code_blocks(blocks: &[Block]) -> Vec<ExtractedCode> {
    blocks
        .iter()
        .enumerate()
        .filter_map(|(i, block)| {
            if let Block::Code {
                lang,
                file,
                content,
                ..
            } = block
            {
                Some(ExtractedCode {
                    language: lang.clone().unwrap_or_default(),
                    file_path: file.clone(),
                    content: content.clone(),
                    block_index: i,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Extract code blocks filtered by language with alias normalization.
///
/// Delegates to [`extract_code_blocks`] then filters by normalized
/// language. See [`normalize_lang`] for the alias table.
pub fn extract_code_blocks_by_lang(blocks: &[Block], language: &str) -> Vec<ExtractedCode> {
    let target = normalize_lang(language);
    extract_code_blocks(blocks)
        .into_iter()
        .filter(|c| normalize_lang(&c.language) == target)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Span;

    /// Helper: build a Block::Code with the given fields.
    fn code_block(lang: Option<&str>, file: Option<&str>, content: &str) -> Block {
        Block::Code {
            lang: lang.map(|s| s.to_string()),
            file: file.map(|s| s.to_string()),
            highlight: vec![],
            content: content.to_string(),
            span: Span::SYNTHETIC,
        }
    }

    /// Helper: build a Block::Markdown.
    fn markdown_block(content: &str) -> Block {
        Block::Markdown {
            content: content.to_string(),
            span: Span::SYNTHETIC,
        }
    }

    // -- normalize_lang tests --

    #[test]
    fn normalize_lang_aliases() {
        assert_eq!(normalize_lang("rs"), "rust");
        assert_eq!(normalize_lang("RS"), "rust");
        assert_eq!(normalize_lang("Rs"), "rust");
        assert_eq!(normalize_lang("ts"), "typescript");
        assert_eq!(normalize_lang("js"), "javascript");
        assert_eq!(normalize_lang("py"), "python");
        assert_eq!(normalize_lang("rb"), "ruby");
        assert_eq!(normalize_lang("sh"), "shell");
        assert_eq!(normalize_lang("bash"), "shell");
        assert_eq!(normalize_lang("zsh"), "shell");
        assert_eq!(normalize_lang("yml"), "yaml");
    }

    #[test]
    fn normalize_lang_passthrough() {
        assert_eq!(normalize_lang("rust"), "rust");
        assert_eq!(normalize_lang("typescript"), "typescript");
        assert_eq!(normalize_lang("swift"), "swift");
        assert_eq!(normalize_lang("go"), "go");
        assert_eq!(normalize_lang(""), "");
    }

    #[test]
    fn normalize_lang_case_insensitive() {
        assert_eq!(normalize_lang("RUST"), "rust");
        assert_eq!(normalize_lang("TypeScript"), "typescript");
        assert_eq!(normalize_lang("YAML"), "yaml");
    }

    #[test]
    fn normalize_lang_idempotent() {
        let cases = vec![
            "rs", "ts", "js", "py", "rb", "sh", "bash", "zsh", "yml",
            "rust", "typescript", "javascript", "python", "ruby", "shell", "yaml",
            "swift", "go", "c", "cpp", "", "unknown",
        ];
        for input in cases {
            let once = normalize_lang(input);
            let twice = normalize_lang(&once);
            assert_eq!(once, twice, "normalize_lang is not idempotent for '{input}'");
        }
    }

    // -- extract_code_blocks tests --

    #[test]
    fn extract_from_empty_doc() {
        let blocks: Vec<Block> = vec![];
        let result = extract_code_blocks(&blocks);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_skips_non_code_blocks() {
        let blocks = vec![
            markdown_block("# Hello"),
            markdown_block("Some text"),
        ];
        let result = extract_code_blocks(&blocks);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_single_code_block() {
        let blocks = vec![
            markdown_block("# Example"),
            code_block(Some("rust"), Some("main.rs"), "fn main() {}"),
        ];
        let result = extract_code_blocks(&blocks);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, "rust");
        assert_eq!(result[0].file_path.as_deref(), Some("main.rs"));
        assert_eq!(result[0].content, "fn main() {}");
        assert_eq!(result[0].block_index, 1);
    }

    #[test]
    fn extract_multiple_code_blocks_preserves_order() {
        let blocks = vec![
            code_block(Some("rust"), None, "let x = 1;"),
            markdown_block("---"),
            code_block(Some("python"), None, "x = 1"),
            code_block(Some("swift"), None, "let x = 1"),
        ];
        let result = extract_code_blocks(&blocks);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].language, "rust");
        assert_eq!(result[0].block_index, 0);
        assert_eq!(result[1].language, "python");
        assert_eq!(result[1].block_index, 2);
        assert_eq!(result[2].language, "swift");
        assert_eq!(result[2].block_index, 3);
    }

    #[test]
    fn extract_code_block_without_lang() {
        let blocks = vec![
            code_block(None, None, "some code"),
        ];
        let result = extract_code_blocks(&blocks);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, "");
    }

    #[test]
    fn extract_code_block_without_file() {
        let blocks = vec![
            code_block(Some("rust"), None, "fn main() {}"),
        ];
        let result = extract_code_blocks(&blocks);
        assert_eq!(result.len(), 1);
        assert!(result[0].file_path.is_none());
    }

    #[test]
    fn extract_preserves_content_exactly() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let blocks = vec![code_block(Some("rust"), None, content)];
        let result = extract_code_blocks(&blocks);
        assert_eq!(result[0].content, content);
    }

    // -- extract_code_blocks_by_lang tests --

    #[test]
    fn filter_by_lang_exact_match() {
        let blocks = vec![
            code_block(Some("rust"), None, "let x = 1;"),
            code_block(Some("python"), None, "x = 1"),
            code_block(Some("rust"), None, "let y = 2;"),
        ];
        let result = extract_code_blocks_by_lang(&blocks, "rust");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "let x = 1;");
        assert_eq!(result[1].content, "let y = 2;");
    }

    #[test]
    fn filter_by_lang_alias() {
        let blocks = vec![
            code_block(Some("rust"), None, "let x = 1;"),
            code_block(Some("python"), None, "x = 1"),
        ];
        // "rs" should match "rust"
        let result = extract_code_blocks_by_lang(&blocks, "rs");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, "rust");
    }

    #[test]
    fn filter_by_lang_case_insensitive() {
        let blocks = vec![
            code_block(Some("Rust"), None, "let x = 1;"),
        ];
        let result = extract_code_blocks_by_lang(&blocks, "RUST");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_by_lang_no_match_returns_empty() {
        let blocks = vec![
            code_block(Some("rust"), None, "let x = 1;"),
        ];
        let result = extract_code_blocks_by_lang(&blocks, "swift");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_by_lang_empty_string_matches_untagged() {
        let blocks = vec![
            code_block(None, None, "some code"),
            code_block(Some("rust"), None, "let x = 1;"),
        ];
        let result = extract_code_blocks_by_lang(&blocks, "");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "some code");
    }

    #[test]
    fn filter_shell_aliases() {
        let blocks = vec![
            code_block(Some("sh"), None, "echo hello"),
            code_block(Some("bash"), None, "echo world"),
            code_block(Some("zsh"), None, "echo foo"),
            code_block(Some("rust"), None, "let x = 1;"),
        ];
        // All shell variants should match when filtering by "sh"
        let result = extract_code_blocks_by_lang(&blocks, "sh");
        assert_eq!(result.len(), 3);

        // Also match by canonical name
        let result2 = extract_code_blocks_by_lang(&blocks, "shell");
        assert_eq!(result2.len(), 3);
    }
}

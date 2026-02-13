//! Property-based tests using proptest.
//!
//! These tests verify that the parser never panics on arbitrary input and that
//! round-trip operations preserve content.

use proptest::prelude::*;

proptest! {
    /// Any random string fed to the parser should never cause a panic.
    #[test]
    fn any_markdown_no_panic(input in "\\PC{0,500}") {
        let result = surf_parse::parse(&input);
        // Just verify it returns without panic — the result can have diagnostics
        let _ = result.doc.blocks.len();
        let _ = result.diagnostics.len();
    }

    /// Parse then to_markdown should preserve text content from blocks.
    /// We test with well-formed markdown that contains no :: directives,
    /// so the parser will create Markdown blocks and round-trip them.
    #[test]
    fn roundtrip_preserves_content(
        heading in "[A-Za-z ]{1,30}",
        body in "[A-Za-z0-9 .,!?]{1,100}"
    ) {
        let input = format!("# {heading}\n\n{body}\n");
        let result = surf_parse::parse(&input);
        let md = result.doc.to_markdown();

        // The heading and body text should appear in the round-tripped markdown
        assert!(
            md.contains(&heading),
            "Round-trip should preserve heading '{heading}', got: {md}"
        );
        assert!(
            md.contains(&body),
            "Round-trip should preserve body '{body}', got: {md}"
        );
    }

    /// Random attribute strings should either parse successfully or return an error,
    /// but never panic.
    #[test]
    fn attrs_parser_completeness(input in "[a-z0-9=\", ]{0,100}") {
        let bracketed = format!("[{input}]");
        let result = surf_parse::attrs::parse_attrs(&bracketed);
        // Either Ok or Err — never panic
        match result {
            Ok(attrs) => {
                // Attrs should be a valid BTreeMap
                let _ = attrs.len();
            }
            Err(_e) => {
                // Parse errors are acceptable for random input
            }
        }
    }
}

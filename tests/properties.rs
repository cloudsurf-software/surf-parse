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

// ═══════════════════════════════════════════════════════════════════════════════
// v0.6.0 Property-Based Tests (Layer 6)
// ═══════════════════════════════════════════════════════════════════════════════
//
// Five invariants from the master strategy:
//   1. Fragment never panics
//   2. NativeBlock conversion length <= input length
//   3. New NativeBlock variants preserve fields through conversion
//   4. Code extraction preserves content byte-identical
//   5. Language normalization is idempotent

use surf_parse::extract::normalize_lang;
use surf_parse::render_html::to_html_fragment;
use surf_parse::types::*;

/// Helper: build a `Block::Code` with SYNTHETIC span.
fn synth_code(lang: Option<&str>, file: Option<&str>, content: &str) -> Block {
    Block::Code {
        lang: lang.map(|s| s.to_string()),
        file: file.map(|s| s.to_string()),
        highlight: vec![],
        content: content.to_string(),
        span: Span::SYNTHETIC,
    }
}

/// Helper: build a `Block::Markdown` with SYNTHETIC span.
fn synth_markdown(content: &str) -> Block {
    Block::Markdown {
        content: content.to_string(),
        span: Span::SYNTHETIC,
    }
}

/// Helper: build a `Block::Callout` with SYNTHETIC span.
fn synth_callout(ct: CalloutType, title: Option<&str>, content: &str) -> Block {
    Block::Callout {
        callout_type: ct,
        title: title.map(|s| s.to_string()),
        content: content.to_string(),
        span: Span::SYNTHETIC,
    }
}

/// Helper: build a `Block::Form` with SYNTHETIC span.
fn synth_form(fields: Vec<FormField>, submit_label: Option<&str>) -> Block {
    Block::Form {
        fields,
        submit_label: submit_label.map(|s| s.to_string()),
        span: Span::SYNTHETIC,
    }
}

/// Helper: build a `Block::Gallery` with SYNTHETIC span.
fn synth_gallery(items: Vec<GalleryItem>, columns: Option<u32>) -> Block {
    Block::Gallery {
        items,
        columns,
        span: Span::SYNTHETIC,
    }
}

/// Helper: build a `Block::Section` with SYNTHETIC span.
fn synth_section(
    bg: Option<&str>,
    headline: Option<&str>,
    subtitle: Option<&str>,
    children: Vec<Block>,
) -> Block {
    Block::Section {
        bg: bg.map(|s| s.to_string()),
        headline: headline.map(|s| s.to_string()),
        subtitle: subtitle.map(|s| s.to_string()),
        content: String::new(),
        children,
        span: Span::SYNTHETIC,
    }
}

/// Proptest strategy for CalloutType.
fn arb_callout_type() -> impl Strategy<Value = CalloutType> {
    prop_oneof![
        Just(CalloutType::Info),
        Just(CalloutType::Warning),
        Just(CalloutType::Danger),
        Just(CalloutType::Tip),
        Just(CalloutType::Note),
        Just(CalloutType::Success),
    ]
}

/// Proptest strategy for FormFieldType.
fn arb_form_field_type() -> impl Strategy<Value = FormFieldType> {
    prop_oneof![
        Just(FormFieldType::Text),
        Just(FormFieldType::Email),
        Just(FormFieldType::Tel),
        Just(FormFieldType::Date),
        Just(FormFieldType::Number),
        Just(FormFieldType::Select),
        Just(FormFieldType::Textarea),
    ]
}

/// Proptest strategy for a FormField.
fn arb_form_field() -> impl Strategy<Value = FormField> {
    (
        "[a-zA-Z ]{1,20}",       // label
        "[a-z_]{1,15}",          // name
        arb_form_field_type(),
        any::<bool>(),           // required
        proptest::option::of("[a-zA-Z0-9 ]{0,20}"),  // placeholder
        proptest::collection::vec("[a-zA-Z]{1,10}", 0..4), // options
    )
        .prop_map(|(label, name, field_type, required, placeholder, options)| FormField {
            label,
            name,
            field_type,
            required,
            placeholder,
            options,
        })
}

/// Proptest strategy for a GalleryItem.
fn arb_gallery_item() -> impl Strategy<Value = GalleryItem> {
    (
        "[a-z/]{1,30}\\.jpg",    // src
        proptest::option::of("[a-zA-Z ]{1,30}"),  // caption
        proptest::option::of("[a-zA-Z ]{1,30}"),  // alt
        proptest::option::of("[a-zA-Z]{1,10}"),   // category
    )
        .prop_map(|(src, caption, alt, category)| GalleryItem {
            src,
            caption,
            alt,
            category,
        })
}

/// Proptest strategy for a diverse Block (subset covering key variant families).
/// We generate blocks that exercise fragment rendering without needing complex
/// nested structures like Page/Site which have multi-field dependencies.
fn arb_block() -> impl Strategy<Value = Block> {
    prop_oneof![
        // Markdown
        "\\PC{0,100}".prop_map(|content| synth_markdown(&content)),
        // Code
        (
            proptest::option::of("[a-z]{1,10}"),
            proptest::option::of("[a-z/.]{1,20}"),
            "\\PC{0,200}",
        )
            .prop_map(|(lang, file, content)| synth_code(
                lang.as_deref(),
                file.as_deref(),
                &content,
            )),
        // Callout
        (arb_callout_type(), proptest::option::of("[a-zA-Z ]{1,20}"), "\\PC{0,100}")
            .prop_map(|(ct, title, content)| synth_callout(ct, title.as_deref(), &content)),
        // Divider
        proptest::option::of("[a-zA-Z ]{1,20}")
            .prop_map(|label| Block::Divider {
                label,
                span: Span::SYNTHETIC,
            }),
        // Summary
        "\\PC{0,100}".prop_map(|content| Block::Summary {
            content,
            span: Span::SYNTHETIC,
        }),
        // Figure
        (
            "[a-z/]{1,20}\\.png",
            proptest::option::of("[a-zA-Z ]{1,30}"),
            proptest::option::of("[a-zA-Z ]{1,30}"),
        )
            .prop_map(|(src, caption, alt)| Block::Figure {
                src,
                caption,
                alt,
                width: None,
                span: Span::SYNTHETIC,
            }),
        // Quote
        ("\\PC{0,100}", proptest::option::of("[a-zA-Z ]{1,20}"))
            .prop_map(|(content, attribution)| Block::Quote {
                content,
                attribution,
                cite: None,
                span: Span::SYNTHETIC,
            }),
        // Details
        (proptest::option::of("[a-zA-Z ]{1,20}"), any::<bool>(), "\\PC{0,100}")
            .prop_map(|(title, open, content)| Block::Details {
                title,
                open,
                content,
                span: Span::SYNTHETIC,
            }),
        // Form
        (
            proptest::collection::vec(arb_form_field(), 0..4),
            proptest::option::of("[a-zA-Z ]{1,15}"),
        )
            .prop_map(|(fields, submit_label)| synth_form(fields, submit_label.as_deref())),
        // Gallery
        (
            proptest::collection::vec(arb_gallery_item(), 0..4),
            proptest::option::of(1u32..6),
        )
            .prop_map(|(items, columns)| synth_gallery(items, columns)),
        // Section (non-recursive: empty children to keep generation bounded)
        (
            proptest::option::of("[a-zA-Z0-9]{1,10}"),
            proptest::option::of("[a-zA-Z ]{1,20}"),
            proptest::option::of("[a-zA-Z ]{1,30}"),
        )
            .prop_map(|(bg, headline, subtitle)| synth_section(
                bg.as_deref(),
                headline.as_deref(),
                subtitle.as_deref(),
                vec![],
            )),
    ]
}

// ─── Invariant 1: Fragment never panics ──────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Any sequence of diverse Block variants passed to `to_html_fragment()`
    /// must return a String without panicking. The output type is always valid UTF-8.
    #[test]
    fn fragment_never_panics(blocks in proptest::collection::vec(arb_block(), 0..10)) {
        let html = to_html_fragment(&blocks);
        // Must return valid UTF-8 (guaranteed by String type) and not panic.
        let _ = html.len();
    }
}

// ─── Invariant 2: NativeBlock conversion length <= input ─────────────────────

#[cfg(feature = "native")]
mod native_props {
    use super::*;
    use surf_parse::render_native::to_native_blocks;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        /// For any SurfDoc, `to_native_blocks()` produces a Vec whose length
        /// is exactly equal to the number of input blocks (1:1 mapping).
        /// The master strategy says "<= input length" to allow for merging,
        /// but the current implementation does a 1:1 map.
        #[test]
        fn native_conversion_length_le_input(blocks in proptest::collection::vec(arb_block(), 0..10)) {
            let doc = SurfDoc {
                blocks: blocks.clone(),
                front_matter: None,
                source: String::new(),
            };
            let native = to_native_blocks(&doc);
            prop_assert!(
                native.len() <= blocks.len(),
                "NativeBlock count ({}) should be <= input block count ({})",
                native.len(),
                blocks.len(),
            );
        }
    }

    // ─── Invariant 3: New NativeBlock variants preserve fields ───────────────

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Form blocks converted to NativeBlock::Form preserve all field metadata.
        #[test]
        fn form_conversion_preserves_fields(
            fields in proptest::collection::vec(arb_form_field(), 1..5),
            submit_label in proptest::option::of("[a-zA-Z ]{1,15}"),
        ) {
            let block = synth_form(fields.clone(), submit_label.as_deref());
            let doc = SurfDoc {
                blocks: vec![block],
                front_matter: None,
                source: String::new(),
            };
            let native = to_native_blocks(&doc);
            prop_assert_eq!(native.len(), 1);

            if let surf_parse::render_native::NativeBlock::Form {
                fields: native_fields,
                submit_label: native_submit,
            } = &native[0]
            {
                prop_assert_eq!(native_fields.len(), fields.len());
                for (nf, f) in native_fields.iter().zip(fields.iter()) {
                    prop_assert_eq!(&nf.label, &f.label);
                    prop_assert_eq!(&nf.name, &f.name);
                    prop_assert_eq!(nf.required, f.required);
                    prop_assert_eq!(&nf.placeholder, &f.placeholder);
                    prop_assert_eq!(&nf.options, &f.options);
                }
                let expected_label = submit_label.unwrap_or_else(|| "Submit".to_string());
                prop_assert_eq!(native_submit, &expected_label);
            } else {
                prop_assert!(false, "Expected NativeBlock::Form, got {:?}", native[0]);
            }
        }

        /// Gallery blocks converted to NativeBlock::Gallery preserve item metadata.
        #[test]
        fn gallery_conversion_preserves_fields(
            items in proptest::collection::vec(arb_gallery_item(), 1..5),
            columns in proptest::option::of(1u32..6),
        ) {
            let block = synth_gallery(items.clone(), columns);
            let doc = SurfDoc {
                blocks: vec![block],
                front_matter: None,
                source: String::new(),
            };
            let native = to_native_blocks(&doc);
            prop_assert_eq!(native.len(), 1);

            if let surf_parse::render_native::NativeBlock::Gallery {
                items: native_items,
                columns: native_cols,
            } = &native[0]
            {
                prop_assert_eq!(native_items.len(), items.len());
                for (ni, i) in native_items.iter().zip(items.iter()) {
                    prop_assert_eq!(&ni.src, &i.src);
                    prop_assert_eq!(&ni.caption, &i.caption);
                    prop_assert_eq!(&ni.alt, &i.alt);
                    prop_assert_eq!(&ni.category, &i.category);
                }
                let expected_cols = columns.unwrap_or(3);
                prop_assert_eq!(*native_cols, expected_cols);
            } else {
                prop_assert!(false, "Expected NativeBlock::Gallery, got {:?}", native[0]);
            }
        }

        /// SectionContainer blocks converted to NativeBlock::SectionContainer
        /// preserve headline, subtitle, bg, and recursively convert children.
        #[test]
        fn section_container_conversion_preserves_fields(
            bg in proptest::option::of("[a-zA-Z0-9]{1,10}"),
            headline in proptest::option::of("[a-zA-Z ]{1,20}"),
            subtitle in proptest::option::of("[a-zA-Z ]{1,30}"),
            child_content in "[a-zA-Z0-9 ]{1,50}",
        ) {
            let child = synth_markdown(&child_content);
            let block = synth_section(
                bg.as_deref(),
                headline.as_deref(),
                subtitle.as_deref(),
                vec![child],
            );
            let doc = SurfDoc {
                blocks: vec![block],
                front_matter: None,
                source: String::new(),
            };
            let native = to_native_blocks(&doc);
            prop_assert_eq!(native.len(), 1);

            if let surf_parse::render_native::NativeBlock::SectionContainer {
                bg: native_bg,
                headline: native_headline,
                subtitle: native_subtitle,
                children,
            } = &native[0]
            {
                prop_assert_eq!(native_bg, &bg);
                prop_assert_eq!(native_headline, &headline);
                prop_assert_eq!(native_subtitle, &subtitle);
                prop_assert_eq!(children.len(), 1);
                // The child should be a Markdown NativeBlock
                if let surf_parse::render_native::NativeBlock::Markdown { content } = &children[0] {
                    prop_assert_eq!(content, &child_content);
                } else {
                    prop_assert!(false, "Expected NativeBlock::Markdown child, got {:?}", children[0]);
                }
            } else {
                prop_assert!(false, "Expected NativeBlock::SectionContainer, got {:?}", native[0]);
            }
        }
    }
}

// ─── Invariant 4: Code extraction preserves content ──────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// For any Block::Code with arbitrary lang, file, and content, `extract_code_blocks()`
    /// returns content that is byte-identical to the input.
    #[test]
    fn code_extraction_preserves_content(
        lang in proptest::option::of("[a-zA-Z0-9]{1,15}"),
        file in proptest::option::of("[a-zA-Z0-9_/.]{1,30}"),
        content in "\\PC{0,500}",
    ) {
        let block = synth_code(lang.as_deref(), file.as_deref(), &content);
        let extracted = surf_parse::extract::extract_code_blocks(&[block]);

        prop_assert_eq!(extracted.len(), 1, "Should extract exactly one code block");
        prop_assert_eq!(
            &extracted[0].content,
            &content,
            "Extracted content must be byte-identical to input"
        );
        prop_assert_eq!(
            &extracted[0].language,
            &lang.unwrap_or_default(),
            "Language should match input"
        );
        prop_assert_eq!(
            &extracted[0].file_path,
            &file,
            "File path should match input"
        );
        prop_assert_eq!(extracted[0].block_index, 0, "Block index should be 0");
    }

    /// Code extraction from a mixed document only returns code blocks and
    /// preserves their relative order.
    #[test]
    fn code_extraction_filters_non_code(
        code_contents in proptest::collection::vec("\\PC{1,100}", 1..5),
        markdown_count in 0usize..5,
    ) {
        let mut blocks: Vec<Block> = Vec::new();
        let mut code_idx = 0;
        for i in 0..(code_contents.len() + markdown_count) {
            if code_idx < code_contents.len() && (i % 2 == 0 || i >= markdown_count) {
                blocks.push(synth_code(Some("rust"), None, &code_contents[code_idx]));
                code_idx += 1;
            } else {
                blocks.push(synth_markdown("some text"));
            }
        }
        let extracted = surf_parse::extract::extract_code_blocks(&blocks);
        prop_assert_eq!(
            extracted.len(),
            code_contents.len(),
            "Should extract exactly {} code blocks from {} total blocks",
            code_contents.len(),
            blocks.len(),
        );
        // Verify content ordering
        for (i, ec) in extracted.iter().enumerate() {
            prop_assert_eq!(
                &ec.content,
                &code_contents[i],
                "Code block {} content mismatch",
                i,
            );
        }
    }
}

// ─── Invariant 5: Language normalization is idempotent ───────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// For any string s, normalize_lang(normalize_lang(s)) == normalize_lang(s).
    /// This ensures the normalization function is a proper projection.
    #[test]
    fn lang_normalization_idempotent(s in "\\PC{0,50}") {
        let once = normalize_lang(&s);
        let twice = normalize_lang(&once);
        prop_assert_eq!(
            &once,
            &twice,
            "normalize_lang is not idempotent for input '{}':\n  once:  '{}'\n  twice: '{}'",
            s,
            once,
            twice,
        );
    }

    /// Normalized output is always lowercase (unless empty).
    #[test]
    fn lang_normalization_lowercase(s in "[a-zA-Z]{1,20}") {
        let normalized = normalize_lang(&s);
        let lowered = normalized.to_lowercase();
        prop_assert_eq!(
            normalized,
            lowered,
            "normalize_lang output should be lowercase for input '{}'",
            s,
        );
    }
}

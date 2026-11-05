//! A doc whose prose carries bare URLs and in-word emphasis markers compiles
//! to a PDF (surf-parse 0.44.2). Measured 2026-11-02 on app.cloudsurf.com: a
//! Surfy-written "Sources" list of Wikipedia URLs failed the whole Typst
//! compile ("unclosed delimiter" ×2, a 500 on GET /api/docs/{id}/export) —
//! `Star_Trek:_The_Original_Series` opened emphasis after the `:` that an
//! in-word `_` could never close.
#![cfg(feature = "pdf")]

use surf_parse::{typst_source, PdfConfig};

const SOURCES: &str = "\
# A short history

The series launched in 1966.[1][2] It ran three seasons.[3]

## Sources

1. Wikipedia, \"The Cage (Star Trek: The Original Series)\". https://en.wikipedia.org/wiki/The_Cage_(Star_Trek:_The_Original_Series)
2. Wikipedia, \"Where No Man Has Gone Before\". https://en.wikipedia.org/wiki/Where_No_Man_Has_Gone_Before
3. Wikipedia, \"Star Trek: The Original Series\". https://en.wikipedia.org/wiki/Star_Trek:_The_Original_Series
4. A report. https://www.example.org/reports/trek_history.htm
- See (https://example.com/a_(b)), or http://example.com/x*y*z.
";

#[test]
fn a_sources_list_of_bare_urls_compiles_to_a_pdf() {
    let parsed = surf_parse::parse(SOURCES);
    let cfg = parsed.doc.pdf_config(PdfConfig::default());
    let pdf = parsed.doc.to_pdf(&cfg).expect("bare URLs must not fail the compile");
    assert!(pdf.starts_with(b"%PDF-"), "a PDF");
}

#[test]
fn a_bare_url_is_one_link_to_the_whole_address() {
    let parsed = surf_parse::parse(SOURCES);
    let src = typst_source(&parsed.doc, &PdfConfig::default());
    assert!(
        src.contains("#link(\"https://en.wikipedia.org/wiki/Star_Trek:_The_Original_Series\");"),
        "the URL is followed whole, underscores and all:\n{src}"
    );
    assert!(
        src.contains("#link(\"https://en.wikipedia.org/wiki/The_Cage_(Star_Trek:_The_Original_Series)\");"),
        "balanced parentheses stay in the URL:\n{src}"
    );
    // Trailing sentence punctuation and an unbalanced `)` stay text.
    assert!(src.contains("#link(\"https://example.com/a_(b)\");),"), "{src}");
    assert!(src.contains("#link(\"http://example.com/x*y*z\");."), "{src}");
}

#[test]
fn emphasis_touching_a_word_is_text_not_an_unclosed_delimiter() {
    for md in ["*foo*bar is fine", "x_foo_ is fine", "a**b**c is fine", "snake_case_name here"] {
        let parsed = surf_parse::parse(md);
        let cfg = parsed.doc.pdf_config(PdfConfig::default());
        parsed
            .doc
            .to_pdf(&cfg)
            .unwrap_or_else(|e| panic!("{md:?} must compile: {e}"));
    }
}

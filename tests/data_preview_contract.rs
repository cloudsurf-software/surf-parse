//! `::data` preview render contract (0.19.2).
//!
//! A `::data` block with more body rows than [`surf_parse::DATA_PREVIEW_ROWS`]
//! paints only its first rows in the two WEB backends, keeps its `total:`
//! summary row, and carries an honest `N rows · open as spreadsheet` line plus
//! `data-rows`/`data-cols` on the wrap. A block at or under the cap renders
//! exactly as 0.19.1 did — no extra class, no `data-` attributes, no line — so
//! golden and downstream template snapshots do not churn.
//!
//! Every assertion here fails against the pre-0.19.2 arms: they emitted the
//! bare `<div class="surfdoc-table-wrap">` wrap, rendered every row, and had
//! no `.surfdoc-table-more` / sticky-header / print rules in the stylesheet.

use surf_parse::{DATA_PREVIEW_ROWS, SURFDOC_CSS};

/// Middle dot U+00B7 — the separator in the count line.
const DOT: char = '\u{b7}';

fn render(src: &str) -> String {
    surf_parse::parse(src).doc.to_html_fragment()
}

/// A `::data` source with `rows` body rows, `cols` columns and an optional
/// `total:` summary row.
fn data_block(rows: usize, cols: usize, with_total: bool) -> String {
    let mut src = String::from("::data\n");
    let header: Vec<String> = (1..=cols).map(|c| format!("H{c}")).collect();
    src.push_str(&header.join(" | "));
    src.push('\n');
    for r in 1..=rows {
        let cells: Vec<String> = (1..=cols).map(|c| format!("r{r}c{c}")).collect();
        src.push_str(&cells.join(" | "));
        src.push('\n');
    }
    if with_total {
        let cells: Vec<String> = (1..=cols).map(|c| format!("t{c}")).collect();
        src.push_str(&format!("total: {}\n", cells.join(" | ")));
    }
    src.push_str("::\n");
    src
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

// -- (b) over the cap: capped tbody, count line, attributes ------------------

#[test]
fn twenty_five_rows_render_exactly_twenty_body_rows() {
    let html = render(&data_block(25, 2, false));
    let body = html
        .split_once("<tbody>")
        .expect("tbody opens")
        .1
        .split_once("</tbody>")
        .expect("tbody closes")
        .0;
    assert_eq!(
        count_occurrences(body, "<tr>"),
        DATA_PREVIEW_ROWS,
        "capped table paints exactly {DATA_PREVIEW_ROWS} body rows: {html}"
    );
    // The 20th row is painted, the 21st and the last are not.
    assert!(body.contains("r20c1"), "row 20 is inside the preview");
    assert!(!body.contains("r21c1"), "row 21 is dropped");
    assert!(!body.contains("r25c1"), "the last row is dropped");
}

#[test]
fn capped_table_carries_the_count_line_with_the_total_row_count() {
    let html = render(&data_block(25, 2, false));
    let want = format!("<p class=\"surfdoc-table-more\">25 rows {DOT} open as spreadsheet</p>");
    assert!(html.contains(&want), "missing count line {want}: {html}");
    // The line is the LAST child of the wrap: after the closing table tag,
    // before the closing wrap div.
    assert!(
        html.contains(&format!("</table>{want}</div>")),
        "count line must sit after </table> inside the wrap: {html}"
    );
}

#[test]
fn capped_wrap_carries_preview_class_and_dimension_attributes() {
    let html = render(&data_block(25, 3, false));
    assert!(
        html.contains(
            "<div class=\"surfdoc-table-wrap surfdoc-table-preview\" data-rows=\"25\" data-cols=\"3\">"
        ),
        "preview wrap markup drifted: {html}"
    );
}

#[test]
fn data_cols_counts_the_widest_row_not_just_the_header() {
    // Header has 2 columns; one body row has 4 — `data-cols` reports 4.
    let mut src = String::from("::data\nH1 | H2\n");
    for r in 1..=24 {
        src.push_str(&format!("r{r}a | r{r}b\n"));
    }
    src.push_str("w1 | w2 | w3 | w4\n::\n");
    let html = render(&src);
    assert!(html.contains("data-rows=\"25\""), "25 body rows: {html}");
    assert!(html.contains("data-cols=\"4\""), "widest row wins: {html}");
}

#[test]
fn tfoot_total_still_renders_under_a_capped_table() {
    let html = render(&data_block(25, 2, true));
    assert!(html.contains("<tfoot><tr>"), "tfoot survives the cap: {html}");
    assert!(html.contains("t1"), "total cells survive the cap: {html}");
    assert!(
        html.contains("</tfoot></table><p class=\"surfdoc-table-more\">"),
        "count line follows the closed table, tfoot included: {html}"
    );
}

// -- (c) at or under the cap: byte-identical to 0.19.1 -----------------------

#[test]
fn twenty_rows_render_unchanged() {
    let html = render(&data_block(20, 2, false));
    assert!(
        html.contains("<div class=\"surfdoc-table-wrap\"><table class=\"surfdoc-data\">"),
        "an uncapped table keeps the bare wrap: {html}"
    );
    assert!(!html.contains("surfdoc-table-preview"), "no preview class: {html}");
    assert!(!html.contains("data-rows="), "no data-rows: {html}");
    assert!(!html.contains("data-cols="), "no data-cols: {html}");
    assert!(!html.contains("surfdoc-table-more"), "no count line: {html}");
    let body = html
        .split_once("<tbody>")
        .expect("tbody opens")
        .1
        .split_once("</tbody>")
        .expect("tbody closes")
        .0;
    assert_eq!(count_occurrences(body, "<tr>"), 20, "all 20 rows paint: {html}");
    assert!(html.ends_with("</table></div>"), "wrap closes right after the table: {html}");
}

#[test]
fn twenty_one_rows_is_the_first_capped_size() {
    let html = render(&data_block(21, 2, false));
    assert!(html.contains("surfdoc-table-preview"), "21 rows is a preview: {html}");
    assert!(
        html.contains(&format!(">21 rows {DOT} open as spreadsheet<")),
        "the line reports the TOTAL row count: {html}"
    );
}

// -- (d) the wide-table class, independent of the row count ------------------

#[test]
fn eight_columns_take_the_wide_class_and_seven_do_not() {
    let wide = render(&data_block(2, 8, false));
    assert!(
        wide.contains("<div class=\"surfdoc-table-wrap surfdoc-table-wide\">"),
        "8 columns is wide, and a short table gets no preview class: {wide}"
    );
    let narrow = render(&data_block(2, 7, false));
    assert!(!narrow.contains("surfdoc-table-wide"), "7 columns is not wide: {narrow}");
    assert!(
        narrow.contains("<div class=\"surfdoc-table-wrap\">"),
        "7 columns keeps the bare wrap: {narrow}"
    );
}

#[test]
fn a_wide_preview_carries_both_classes_in_order() {
    let html = render(&data_block(25, 8, false));
    assert!(
        html.contains(
            "<div class=\"surfdoc-table-wrap surfdoc-table-preview surfdoc-table-wide\" data-rows=\"25\" data-cols=\"8\">"
        ),
        "wrap, preview, wide — in that order: {html}"
    );
}

// -- (e) untouched backends --------------------------------------------------

#[test]
fn markdown_and_native_backends_are_never_truncated() {
    let doc = surf_parse::parse(&data_block(25, 2, false)).doc;
    let md = doc.to_markdown();
    assert!(md.contains("r25c1"), "markdown keeps every row: {md}");
    assert!(!md.contains("open as spreadsheet"), "markdown has no count line: {md}");
    let round_trip = doc.to_surf_source();
    assert!(round_trip.contains("r25c1"), "the serializer keeps every row");
}

// -- CSS: the stylesheet the web shell serves --------------------------------

#[test]
fn stylesheet_freezes_the_header_against_the_scrolling_wrap() {
    assert!(
        SURFDOC_CSS.contains(
            ".surfdoc-table-wrap .surfdoc-data thead th { position: sticky; top: 0; z-index: 1; background: var(--surface-alt); box-shadow: inset 0 -1px 0 var(--border); }"
        ),
        "sticky header rule missing from surfdoc.css (it must paint its own hairline: a collapsed border does not travel with a sticky cell)"
    );
    assert!(
        SURFDOC_CSS.contains(".surfdoc-table-wrap { overflow: auto; max-height: 70vh;"),
        "the wrap must scroll on both axes with a capped height"
    );
    assert!(
        SURFDOC_CSS.contains(".surfdoc-table-more {"),
        "the count line has no rule of its own"
    );
}

#[test]
fn stylesheet_carries_the_print_block_and_the_named_landscape_page() {
    assert!(
        SURFDOC_CSS.contains(
            "@media print {\n  .surfdoc-table-wrap { max-height: none; overflow: visible; }"
        ),
        "the print block must lift the scroll cap off the wrap"
    );
    for needle in [
        ".surfdoc-data thead { display: table-header-group; }",
        ".surfdoc-data tfoot { display: table-footer-group; }",
        "break-inside: avoid; page-break-inside: avoid;",
        ".surfdoc-table-wide { page: surfdoc-wide; }",
        "@page surfdoc-wide { size: landscape; }",
    ] {
        assert!(SURFDOC_CSS.contains(needle), "surfdoc.css is missing: {needle}");
    }
}

/// A landscape page only helps if the table fits it. Measured on a printed
/// ten-column doc: without these two rules the natural table width overflows
/// the landscape sheet and the last column prints clipped.
#[test]
fn a_wide_table_is_locked_to_the_printed_page_box() {
    for needle in [
        ".surfdoc-table-wide .surfdoc-data { table-layout: fixed; width: 100%; }",
        ".surfdoc-table-wide .surfdoc-data thead th { white-space: normal; }",
        ".surfdoc-table-wide .surfdoc-data th, .surfdoc-table-wide .surfdoc-data td { padding: 6px 6px; }",
    ] {
        assert!(SURFDOC_CSS.contains(needle), "surfdoc.css is missing: {needle}");
    }
    // Both live INSIDE the print block — screen rendering keeps the natural
    // column widths and the nowrap header.
    let print_block = SURFDOC_CSS
        .split_once("@media print {\n  .surfdoc-table-wrap { max-height: none;")
        .expect("print block")
        .1
        .split_once("\n}")
        .expect("print block closes")
        .0;
    assert!(
        print_block.contains("table-layout: fixed"),
        "the wide-table fit rules must be print-only"
    );
}

// -- the const is on the public path -----------------------------------------

#[test]
fn preview_cap_is_public_and_twenty() {
    assert_eq!(DATA_PREVIEW_ROWS, 20);
}

// -- 0.20.0: `source=` makes the body a preview and links the count line -----

/// A `::data` block referencing out-of-line rows: `n` inline preview rows,
/// two columns, and the authored counts.
fn sourced_block(inline_rows: usize, source: &str, rows: usize, cols: usize) -> String {
    let mut src = format!("::data[source=\"{source}\" rows={rows} cols={cols}]\n");
    src.push_str("H1 | H2\n");
    for r in 1..=inline_rows {
        src.push_str(&format!("r{r}c1 | r{r}c2\n"));
    }
    src.push_str("::\n");
    src
}

#[test]
fn a_file_source_links_the_count_line_under_files() {
    let html = render(&sourced_block(3, "file:abc123", 4200, 7));
    assert!(
        html.contains(&format!(
            "<a class=\"surfdoc-table-more\" href=\"/files/abc123\">4200 rows {DOT} open as spreadsheet</a>"
        )),
        "file: source must link the count line under /files: {html}"
    );
    assert!(
        !html.contains("<p class=\"surfdoc-table-more\">"),
        "the paragraph form must not also be emitted: {html}"
    );
}

#[test]
fn a_doc_source_links_the_count_line_under_docs_and_drops_the_sheet_fragment() {
    let html = render(&sourced_block(2, "doc:d17#Q3", 88, 4));
    assert!(
        html.contains("href=\"/docs/d17\""),
        "doc: source maps to /docs/<id> without the sheet fragment: {html}"
    );
    assert!(
        html.contains(&format!("88 rows {DOT} open as spreadsheet")),
        "the count comes from rows=, not the inline preview: {html}"
    );
}

#[test]
fn a_sourced_block_counts_from_the_attributes_not_the_preview() {
    let html = render(&sourced_block(3, "file:abc123", 4200, 7));
    assert!(
        html.contains("data-rows=\"4200\" data-cols=\"7\""),
        "the wrap carries the source counts: {html}"
    );
    assert!(
        html.contains("surfdoc-table-preview"),
        "any sourced block is a preview, whatever its inline size: {html}"
    );
    // The three inline rows all render — the cap only bites above 20.
    assert_eq!(count_occurrences(&html, "<tr>"), 4, "header + 3 body rows: {html}");
}

#[test]
fn a_sourced_block_with_no_inline_rows_still_renders_header_and_count_line() {
    let src = "::data[source=\"file:abc123\" rows=4200 cols=2]\nH1 | H2\n::\n";
    let html = render(src);
    assert!(html.contains("<thead><tr>"), "the header row survives: {html}");
    assert!(html.contains("<tbody></tbody>"), "the body is empty: {html}");
    assert!(
        html.contains(&format!("4200 rows {DOT} open as spreadsheet")),
        "the count line is still drawn: {html}"
    );
}

#[test]
fn an_unresolvable_source_degrades_to_the_inert_paragraph() {
    let html = render("::data[source=\"s3://bucket/key\" rows=9 cols=2]\nH1 | H2\n::\n");
    assert!(
        html.contains(&format!(
            "<p class=\"surfdoc-table-more\">9 rows {DOT} open as spreadsheet</p>"
        )),
        "an unknown scheme never invents a URL: {html}"
    );
    assert!(!html.contains("href=\"/files/"), "no invented href: {html}");
}

#[test]
fn a_block_without_source_is_byte_identical_to_0_19_2() {
    // The 0.20.0 fields are absent, so both the small and the capped shapes
    // must render exactly as the preview contract above pins them.
    let small = render(&data_block(20, 2, false));
    assert!(!small.contains("surfdoc-table-preview"), "{small}");
    assert!(!small.contains("surfdoc-table-more"), "{small}");
    assert!(!small.contains("data-rows="), "{small}");
    let capped = render(&data_block(25, 2, false));
    assert!(
        capped.contains(&format!(
            "<p class=\"surfdoc-table-more\">25 rows {DOT} open as spreadsheet</p>"
        )),
        "{capped}"
    );
    assert!(!capped.contains("<a class=\"surfdoc-table-more\""), "{capped}");
}

// -- 0.20.0: the `type: spreadsheet` workbook layout -------------------------

fn workbook_source() -> String {
    String::from(
        "---\ntitle: \"Books\"\ntype: spreadsheet\n---\n\n\
         ::data[name=\"Revenue\"]\nH1 | H2\nr1c1 | r1c2\n::\n\n\
         ::data\nH1 | H2\nr1c1 | r1c2\n::\n",
    )
}

#[test]
fn a_spreadsheet_document_renders_the_workbook_shell() {
    let html = surf_parse::render_html::to_html(&surf_parse::parse(&workbook_source()).doc);
    assert!(
        html.contains("<section class=\"surfdoc-workbook\">"),
        "workbook wrapper missing: {html}"
    );
    assert!(
        html.contains("<nav class=\"surfdoc-sheet-strip\">"),
        "sheet strip missing: {html}"
    );
    assert_eq!(
        count_occurrences(&html, "<section class=\"surfdoc-sheet\""),
        2,
        "one section per top-level ::data block: {html}"
    );
}

#[test]
fn sheet_names_come_from_the_name_attribute_then_position() {
    let html = surf_parse::render_html::to_html(&surf_parse::parse(&workbook_source()).doc);
    assert!(html.contains("data-sheet=\"Revenue\""), "authored name: {html}");
    assert!(
        html.contains("data-sheet=\"Sheet2\""),
        "the unnamed second sheet takes its position: {html}"
    );
    assert!(html.contains("href=\"#surfdoc-sheet-1\">Revenue</a>"), "{html}");
    assert!(html.contains("href=\"#surfdoc-sheet-2\">Sheet2</a>"), "{html}");
}

#[test]
fn an_ordinary_document_never_takes_the_workbook_layout() {
    let src = "---\ntitle: \"Books\"\ntype: doc\n---\n\n::data\nH1 | H2\nr1c1 | r1c2\n::\n";
    let html = surf_parse::render_html::to_html(&surf_parse::parse(src).doc);
    assert!(!html.contains("surfdoc-workbook"), "{html}");
    assert!(!html.contains("surfdoc-sheet"), "{html}");
}

#[test]
fn stylesheet_carries_the_workbook_selectors() {
    for selector in [
        ".surfdoc-workbook",
        ".surfdoc-sheet-strip",
        ".surfdoc-sheet ",
        "a.surfdoc-table-more",
    ] {
        assert!(
            SURFDOC_CSS.contains(selector),
            "surfdoc.css must carry a rule for {selector}"
        );
    }
}

// -- 0.20.0 (D-SS-14): the markdown pipe-table cap ---------------------------

/// A bare markdown pipe table with `rows` body rows and two columns.
fn pipe_table(rows: usize) -> String {
    let mut src = String::from("| A | B |\n|---|---|\n");
    for r in 1..=rows {
        src.push_str(&format!("| r{r}a | r{r}b |\n"));
    }
    src
}

#[test]
fn a_twenty_five_row_pipe_table_is_capped_like_a_data_block() {
    let html = render(&pipe_table(25));
    // Header row + 20 body rows.
    assert_eq!(count_occurrences(&html, "<tr>"), 21, "capped body: {html}");
    assert!(html.contains("r20b"), "the last kept row is row 20: {html}");
    assert!(!html.contains("r21a"), "row 21 is dropped: {html}");
    assert!(
        html.contains("<div class=\"surfdoc-table-wrap surfdoc-table-preview\" data-rows=\"25\" data-cols=\"2\">"),
        "the wrap takes the data-block shape: {html}"
    );
    assert!(
        html.contains(&format!(
            "<p class=\"surfdoc-table-more\">25 rows {DOT} open as spreadsheet</p></div>"
        )),
        "the inert count line is the last child of the wrap: {html}"
    );
}

#[test]
fn a_twenty_row_pipe_table_is_byte_identical_to_0_19_2() {
    let html = render(&pipe_table(20));
    assert!(
        html.starts_with("<div class=\"surfdoc-table-wrap\"><table>"),
        "no preview class at or under the cap: {html}"
    );
    assert!(!html.contains("data-rows="), "no dimension attributes: {html}");
    assert!(!html.contains("surfdoc-table-more"), "no count line: {html}");
    assert!(html.contains("r20b"), "every row still renders: {html}");
    assert!(html.ends_with("</tbody></table></div>\n"), "unchanged tail: {html}");
}

#[test]
fn a_twenty_one_row_pipe_table_is_the_first_capped_size() {
    let html = render(&pipe_table(21));
    assert_eq!(count_occurrences(&html, "<tr>"), 21, "{html}");
    assert!(html.contains("data-rows=\"21\""), "{html}");
    assert!(!html.contains("r21a"), "{html}");
}

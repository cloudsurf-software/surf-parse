//! The workbook plan — what a `type: spreadsheet` document's SHEETS are, what
//! each one is called, and which blocks are prose behind them (0.38.0).
//!
//! One rule for every renderer (D-WS-1, the Mac's D-SHEET-1 made canon):
//!
//! - **Every top-level `::data` block is a sheet, and every GFM pipe table
//!   inside a top-level markdown block is a sheet**, in source order. A
//!   markdown block is split around its tables; the prose between them stays
//!   prose.
//! - **A sheet's label** is the block's `name=`, else the table's `caption=`,
//!   else the nearest preceding markdown heading, else `Sheet<n>` by
//!   position — and never twice the same (a second "Status" is "Status (2)",
//!   the kit's `WorkbookAdapter.uniqueLabel` form, so the web and the Mac
//!   name a doc's tabs alike).
//! - **Everything else is About**: the summary, the callouts, the headings
//!   that became labels, the action items — in source order, never
//!   interleaved with the grid (D-SHEET-3). A `::nav` is chrome and is
//!   neither.
//!
//! The plan is pure data over the parsed blocks; `render_html` and
//! `render_dom` both walk it, which is how their workbook markup stays
//! byte-identical. Nothing here parses a cell: a pipe table reaches the
//! renderers as the markdown source of its own lines, and the ordinary
//! markdown path draws it (uncapped inside a sheet).

use crate::types::Block;

/// Where one sheet's rows come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetBody {
    /// A top-level `::data` block, by index into `blocks`.
    Data(usize),
    /// A GFM pipe table lifted out of a markdown block: exactly its lines
    /// (the header, the delimiter, the rows), newline-joined, no trailing
    /// newline.
    Table(String),
}

/// One sheet of the workbook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The strip's word for the sheet — unique within the workbook.
    pub label: String,
    pub body: SheetBody,
    /// Body rows the sheet carries, or — for a `source=` block — the
    /// authored `rows=` count of what lives out of line.
    pub rows: usize,
    /// Columns, by the same rule.
    pub cols: usize,
    /// A `::data[source=]` reference, passed through for the face's status
    /// line ("20 of N rows — the rest live on the server").
    pub source: Option<String>,
}

/// One piece of the About aside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AboutPart {
    /// A whole non-sheet block, by index into `blocks` (rendered exactly as
    /// the document profile renders it, root attributes included).
    Block(usize),
    /// The prose of a markdown block between (or around) its tables —
    /// rendered through the bare markdown path, with no block root to carry
    /// addressing attributes (the split block's id belongs to no one piece).
    Prose(String),
}

/// A spreadsheet document, split.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkbookPlan {
    pub sheets: Vec<Sheet>,
    pub about: Vec<AboutPart>,
}

/// The maximum width of a sheet tab's label in the strip, in CSS pixels /
/// points, on EVERY platform (D-WS-9, Brady 2026-10-02): the label is drawn
/// whole up to this width and clipped with a tail ellipsis past it; the
/// full name is the tooltip and the About aside lists it.
pub const SHEET_TAB_MAX_WIDTH: u32 = 180;

/// Build the plan for `blocks` (a document's top-level blocks).
pub fn plan_workbook(blocks: &[Block]) -> WorkbookPlan {
    let mut plan = WorkbookPlan::default();
    let mut used: Vec<String> = Vec::new();
    let mut heading: Option<String> = None;
    for (index, block) in blocks.iter().enumerate() {
        match block {
            Block::Nav { .. } => {}
            Block::Data { name, caption, headers, rows, source, source_rows, source_cols, .. } => {
                let label = sheet_label(
                    name.as_deref(),
                    caption.as_deref(),
                    heading.as_deref(),
                    plan.sheets.len(),
                    &mut used,
                );
                let inline_rows = rows.len();
                let inline_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(headers.len());
                let linked = source.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
                let (total_rows, total_cols) = match linked {
                    Some(_) => (source_rows.unwrap_or(inline_rows), source_cols.unwrap_or(inline_cols)),
                    None => (inline_rows, inline_cols),
                };
                plan.sheets.push(Sheet {
                    label,
                    body: SheetBody::Data(index),
                    rows: total_rows,
                    cols: total_cols,
                    source: linked,
                });
            }
            Block::Markdown { content, .. } => {
                for segment in split_markdown_tables(content) {
                    match segment {
                        MdSegment::Prose(text) => {
                            if let Some(last) = headings_in(&text).last() {
                                heading = Some(last.clone());
                            }
                            plan.about.push(AboutPart::Prose(text));
                        }
                        MdSegment::Table { source, rows, cols } => {
                            let label = sheet_label(None, None, heading.as_deref(), plan.sheets.len(), &mut used);
                            plan.sheets.push(Sheet {
                                label,
                                body: SheetBody::Table(source),
                                rows,
                                cols,
                                source: None,
                            });
                        }
                    }
                }
            }
            _ => plan.about.push(AboutPart::Block(index)),
        }
    }
    plan
}

/// The label rule: `name=` → `caption=` → the nearest preceding heading →
/// `Sheet<n>` (`index` is 0-based; the word is 1-based), made unique against
/// `used` as "X", "X (2)", "X (3)" … and recorded there.
pub fn sheet_label(
    name: Option<&str>,
    caption: Option<&str>,
    heading: Option<&str>,
    index: usize,
    used: &mut Vec<String>,
) -> String {
    let base = [name, caption, heading]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("Sheet{}", index + 1));
    if !used.iter().any(|u| u == &base) {
        used.push(base.clone());
        return base;
    }
    let mut n = 2usize;
    loop {
        let tagged = format!("{base} ({n})");
        if !used.iter().any(|u| u == &tagged) {
            used.push(tagged.clone());
            return tagged;
        }
        n += 1;
    }
}

/// A markdown block, cut around its pipe tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MdSegment {
    /// Prose between tables — trimmed of the blank lines that separated it
    /// from a table, so each piece renders as a tight block.
    Prose(String),
    /// One GFM pipe table: its own lines, newline-joined; `rows` body rows,
    /// `cols` header cells.
    Table { source: String, rows: usize, cols: usize },
}

/// Split markdown `content` into prose and pipe-table segments, in order.
///
/// A table is a header row followed by a delimiter row with the SAME cell
/// count (the GFM rule pulldown-cmark applies — a mismatched delimiter is
/// prose, and must not become a sheet whose section holds no table), then
/// every following line that is a pipe row. A fenced code block is never
/// cut, whatever it holds. Content with no table comes back as one prose
/// segment, byte for byte.
pub fn split_markdown_tables(content: &str) -> Vec<MdSegment> {
    let lines: Vec<&str> = content.split('\n').collect();
    let mut out: Vec<MdSegment> = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let mut fence: Option<&str> = None;
    let mut i = 0usize;

    fn flush(prose: &mut Vec<&str>, out: &mut Vec<MdSegment>) {
        if prose.iter().any(|l| !l.trim().is_empty()) {
            let start = prose.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
            let end = prose.iter().rposition(|l| !l.trim().is_empty()).unwrap_or(0);
            out.push(MdSegment::Prose(prose[start..=end].join("\n")));
        }
        prose.clear();
    }

    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            prose.push(line);
            i += 1;
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            prose.push(line);
            i += 1;
            continue;
        }
        if i + 1 < lines.len() && is_table_row(line) && is_delimiter_row(lines[i + 1]) {
            let cols = split_table_cells(line).len();
            if cols > 0 && split_table_cells(lines[i + 1]).len() == cols {
                let mut j = i + 2;
                while j < lines.len() && is_table_row(lines[j]) {
                    j += 1;
                }
                flush(&mut prose, &mut out);
                out.push(MdSegment::Table {
                    source: lines[i..j].join("\n"),
                    rows: j - (i + 2),
                    cols,
                });
                i = j;
                continue;
            }
        }
        prose.push(line);
        i += 1;
    }
    flush(&mut prose, &mut out);
    // No table: the content comes back byte for byte (no prose re-trimming).
    if !out.iter().any(|s| matches!(s, MdSegment::Table { .. })) {
        return vec![MdSegment::Prose(content.to_string())];
    }
    out
}

/// The ATX heading lines of a markdown piece, in order, without their
/// hashes or a trailing `{#slug}` anchor — the nearest preceding one names
/// the sheet under it. Fenced code is skipped.
pub fn headings_in(markdown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for raw in markdown.split('\n') {
        let t = raw.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            continue;
        }
        let hashes = t.bytes().take_while(|b| *b == b'#').count();
        if !(1..=6).contains(&hashes) || !t[hashes..].starts_with(' ') {
            continue;
        }
        let mut text = t[hashes..].trim();
        if let Some((before, _)) = crate::render_html::split_explicit_anchor(text) {
            text = before.trim();
        }
        // A closing run of hashes (`## Title ##`) is not part of the title.
        let text = text.trim_end_matches('#').trim_end();
        if !text.is_empty() {
            out.push(text.to_string());
        }
    }
    out
}

/// True if a line looks like a pipe-table row: after trimming it contains at
/// least one unescaped `|`.
pub(crate) fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    let mut escaped = false;
    for c in t.chars() {
        match c {
            '\\' => escaped = !escaped,
            '|' if !escaped => return true,
            _ => escaped = false,
        }
    }
    false
}

/// True if a line is a GFM delimiter row: every cell dashes with optional
/// leading/trailing colons, at least one cell.
pub(crate) fn is_delimiter_row(line: &str) -> bool {
    let t = line.trim();
    if !is_table_row(t) {
        return false;
    }
    let cells = split_table_cells(t);
    if cells.is_empty() {
        return false;
    }
    cells.iter().all(|cell| {
        let c = cell.trim();
        if c.is_empty() {
            return false;
        }
        let inner = c.trim_start_matches(':').trim_end_matches(':');
        !inner.is_empty() && inner.chars().all(|ch| ch == '-')
    })
}

/// Split a pipe-table row into trimmed cells, honouring `\|` and dropping
/// the optional border pipes.
pub(crate) fn split_table_cells(line: &str) -> Vec<String> {
    let mut t = line.trim();
    if let Some(stripped) = t.strip_prefix('|') {
        t = stripped;
    }
    if let Some(stripped) = t.strip_suffix('|')
        && !t.ends_with("\\|")
    {
        t = stripped;
    }
    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for c in t.chars() {
        if escaped {
            if c == '|' {
                cur.push('|');
            } else {
                cur.push('\\');
                cur.push(c);
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    if escaped {
        cur.push('\\');
    }
    cells.push(cur.trim().to_string());
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(src: &str) -> WorkbookPlan {
        plan_workbook(&crate::parse(src).doc.blocks)
    }

    fn labels(p: &WorkbookPlan) -> Vec<&str> {
        p.sheets.iter().map(|s| s.label.as_str()).collect()
    }

    /// An invented contract register in the shape of the doc that set the
    /// bar: a summary, a Totals table, then numbered sections with one pipe
    /// table each. No client's data — the words are made up.
    pub(crate) const REGISTER: &str = include_str!("../tests/fixtures/workbook-register.surf");

    #[test]
    fn the_register_is_thirteen_sheets_named_by_their_headings() {
        let p = plan(REGISTER);
        assert_eq!(p.sheets.len(), 13, "Totals + twelve numbered sections");
        assert_eq!(p.sheets[0].label, "Totals");
        assert_eq!(p.sheets[1].label, "1 — Schedule A deliverables (fixed bid)");
        assert_eq!(p.sheets[4].label, "4 — Engagement phases and acceptance gates for the pilot programme");
        assert_eq!(p.sheets[12].label, "12 — Closing conditions");
        assert!(p.sheets.iter().all(|s| matches!(s.body, SheetBody::Table(_))));
        assert_eq!(p.sheets[1].cols, 7, "ID · Requirement · Section · Obligor · Type · Status · Notes");
        assert_eq!(p.sheets[1].rows, 9);
        assert_eq!(p.sheets.iter().map(|s| s.rows).sum::<usize>(), 12 + 9 * 12, "every body row of every table is counted");
        // The prose: the summary block, the title heading, every section
        // heading — none of it interleaves with a sheet.
        assert!(p.about.iter().any(|a| matches!(a, AboutPart::Block(_))), "the ::summary is a whole block");
        let prose: Vec<&String> = p.about.iter().filter_map(|a| if let AboutPart::Prose(t) = a { Some(t) } else { None }).collect();
        assert!(prose.iter().any(|t| t.contains("# Contract register")), "the title heading is prose (a block with no table comes back whole, its leading newline included)");
        assert!(prose.iter().any(|t| t.contains("## 12 — Closing conditions")));
        assert!(prose.iter().all(|t| !t.contains("|---|")), "no table line survives in the prose");
    }

    #[test]
    fn a_second_table_under_the_same_heading_takes_a_numbered_label() {
        let src = "## Status\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nmore words\n\n| C | D |\n|---|---|\n| 3 | 4 |\n";
        let p = plan(src);
        assert_eq!(labels(&p), vec!["Status", "Status (2)"]);
        let src3 = "## Status\n\n| A |\n|---|\n| 1 |\n\n| B |\n|---|\n| 2 |\n\n| C |\n|---|\n| 3 |\n";
        assert_eq!(labels(&plan(src3)), vec!["Status", "Status (2)", "Status (3)"]);
    }

    #[test]
    fn a_data_block_is_named_first_by_name_then_caption_then_heading_then_position() {
        let src = "## Heading\n\n::data[name=\"Revenue\" caption=\"Money\"]\nH1 | H2\n1 | 2\n::\n\n::data[caption=\"Costs\"]\nH1 | H2\n1 | 2\n::\n\n::data\nH1 | H2\n1 | 2\n::\n\n| P | Q |\n|---|---|\n| 1 | 2 |\n";
        let p = plan(src);
        assert_eq!(labels(&p), vec!["Revenue", "Costs", "Heading", "Heading (2)"]);
        assert!(matches!(p.sheets[0].body, SheetBody::Data(1)));
        assert!(matches!(p.sheets[3].body, SheetBody::Table(_)));
        let mut used = Vec::new();
        assert_eq!(sheet_label(None, None, None, 4, &mut used), "Sheet5");
        assert_eq!(sheet_label(Some("  "), Some(""), None, 0, &mut used), "Sheet1");
    }

    #[test]
    fn a_doc_with_no_table_is_an_empty_workbook_whose_prose_is_all_about() {
        let src = "# Notes\n\nNo table here.\n\n::callout[type=info]\nStill none.\n::\n";
        let p = plan(src);
        assert!(p.sheets.is_empty());
        assert_eq!(p.about.len(), 2);
        assert!(matches!(&p.about[0], AboutPart::Prose(t) if t == "# Notes\n\nNo table here."));
        assert!(matches!(p.about[1], AboutPart::Block(1)));
    }

    #[test]
    fn a_source_block_counts_its_out_of_line_rows_and_keeps_the_reference() {
        let src = "::data[name=\"GSA\" source=\"file:abc\" rows=4200 cols=9]\nA | B\n1 | 2\n::\n";
        let p = plan(src);
        assert_eq!(p.sheets[0].rows, 4200);
        assert_eq!(p.sheets[0].cols, 9);
        assert_eq!(p.sheets[0].source.as_deref(), Some("file:abc"));
    }

    #[test]
    fn a_nav_is_neither_a_sheet_nor_about() {
        let src = "::nav\n- Home: /\n::\n\n| A |\n|---|\n| 1 |\n";
        let p = plan(src);
        assert_eq!(p.sheets.len(), 1);
        assert!(p.about.is_empty());
    }

    #[test]
    fn the_splitter_cuts_around_tables_and_leaves_fences_and_bad_delimiters_alone() {
        let src = "Lead.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nTail.";
        let segs = split_markdown_tables(src);
        assert_eq!(segs, vec![
            MdSegment::Prose("Lead.".into()),
            MdSegment::Table { source: "| A | B |\n|---|---|\n| 1 | 2 |".into(), rows: 1, cols: 2 },
            MdSegment::Prose("Tail.".into()),
        ]);
        let fenced = "```\n| A | B |\n|---|---|\n```\n";
        assert_eq!(split_markdown_tables(fenced), vec![MdSegment::Prose(fenced.into())]);
        let mismatch = "| A | B |\n|---|\n| 1 | 2 |\n";
        assert_eq!(split_markdown_tables(mismatch), vec![MdSegment::Prose(mismatch.into())]);
        assert_eq!(split_markdown_tables("plain"), vec![MdSegment::Prose("plain".into())]);
        assert_eq!(split_markdown_tables(""), vec![MdSegment::Prose(String::new())]);
    }

    #[test]
    fn headings_lose_their_hashes_anchors_and_closing_runs() {
        assert_eq!(headings_in("## Title {#t}\n\n### Sub ##\n```\n# not one\n```\n#nospace"), vec!["Title", "Sub"]);
    }
}

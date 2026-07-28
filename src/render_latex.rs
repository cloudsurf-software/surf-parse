//! LaTeX renderer.
//!
//! Emits real, compilable `.tex` from a [`SurfDoc`] block tree. The output
//! target is selected by the document's render profile (Chunk 1):
//!
//! - **Papers** (`type: paper`): `IEEEtran` for `ieee`, two-column `article` for
//!   `acm`, single-column `article` for the generic `article` format.
//! - **Reports** (`type: report`): `article` configured per style — MLA
//!   (`setspace` double-spacing + heading block + Works Cited), APA (title page +
//!   References), Chicago (title page + Bibliography).
//! - **Everything else**: a plain `article` document.
//!
//! Citations are formatted through the Chunk 4 citation engine: numbered styles
//! (IEEE/ACM) emit `\cite{…}` + a `thebibliography` environment; author styles
//! (MLA/APA/Chicago/article) emit the formatted in-text string inline and a
//! hanging-indent reference list under a `\section*` heading.
//!
//! Pure / deterministic: same input → byte-identical `.tex`. No new heavy deps —
//! the `.tex` is built as strings.

use crate::citation::{
    bibliography_heading, format_in_text, is_numbered, ordered_references, reference_list,
    with_active, CiteContext, CiteRef,
};
use crate::render_typst::{parse_backtick_code, parse_delimited, parse_link, split_ordered_list};
use crate::types::*;

/// Render a [`SurfDoc`] to a complete LaTeX (`.tex`) document string.
pub fn to_latex(doc: &SurfDoc) -> String {
    let format = doc.front_matter.as_ref().and_then(|fm| fm.format);
    let doc_type = doc.front_matter.as_ref().and_then(|fm| fm.doc_type);
    let _cite_scope =
        crate::citation::install_context(crate::citation::build_context(&doc.blocks, format));

    match crate::types::render_profile(doc_type, format) {
        RenderProfile::Paper(f) => latex_paper(doc, f),
        RenderProfile::Report(f) => latex_report(doc, f),
        _ => latex_generic(doc),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Front-matter helpers (mirror the Typst academic templates)
// ───────────────────────────────────────────────────────────────────────────

fn fm_extra(fm: &FrontMatter, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = fm.extra.get(*k) {
            if let Some(s) = v.as_str() {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
            if let Some(seq) = v.as_sequence() {
                let parts: Vec<String> = seq
                    .iter()
                    .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .collect();
                if !parts.is_empty() {
                    return Some(parts.join("; "));
                }
            }
        }
    }
    None
}

fn doc_title(doc: &SurfDoc) -> String {
    doc.front_matter
        .as_ref()
        .and_then(|fm| fm.title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Untitled".to_string())
}

fn doc_authors(doc: &SurfDoc) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Some(fm) = doc.front_matter.as_ref() {
        let raw = fm_extra(fm, &["authors"]).or_else(|| fm.author.clone());
        if let Some(raw) = raw {
            for part in raw.split(';') {
                let t = part.trim();
                if !t.is_empty() && !names.iter().any(|n| n == t) {
                    names.push(t.to_string());
                }
            }
        }
        if let Some(contribs) = fm.contributors.as_ref() {
            for c in contribs {
                let t = c.trim();
                if !t.is_empty() && !names.iter().any(|n| n == t) {
                    names.push(t.to_string());
                }
            }
        }
    }
    names
}

fn doc_date(doc: &SurfDoc) -> Option<String> {
    let fm = doc.front_matter.as_ref()?;
    fm_extra(fm, &["date"])
        .or_else(|| fm.created.clone())
        .or_else(|| fm.updated.clone())
}

// ───────────────────────────────────────────────────────────────────────────
// Paper templates
// ───────────────────────────────────────────────────────────────────────────

fn latex_paper(doc: &SurfDoc, format: Format) -> String {
    let title = doc_title(doc);
    let authors = doc_authors(doc);
    let affiliation = doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm_extra(fm, &["affiliation", "affiliations", "institution"]));
    let abstract_text = doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm_extra(fm, &["abstract"]));

    let mut out = String::with_capacity(8192);

    match format {
        Format::Ieee => {
            out.push_str("\\documentclass[conference]{IEEEtran}\n");
        }
        Format::Acm => {
            out.push_str("\\documentclass[twocolumn]{article}\n");
        }
        _ => {
            out.push_str("\\documentclass[11pt]{article}\n");
        }
    }
    push_common_preamble(&mut out);

    // Title + authors.
    out.push_str(&format!("\\title{{{}}}\n", escape_latex(&title)));
    if matches!(format, Format::Ieee) {
        out.push_str("\\author{\\IEEEauthorblockN{");
        out.push_str(&escape_latex(&authors.join(", ")));
        out.push('}');
        if let Some(aff) = &affiliation {
            out.push_str(&format!("\\IEEEauthorblockA{{{}}}", escape_latex(aff)));
        }
        out.push_str("}\n");
    } else {
        let mut a = escape_latex(&authors.join(", "));
        if let Some(aff) = &affiliation {
            a.push_str(&format!("\\\\ {}", escape_latex(aff)));
        }
        out.push_str(&format!("\\author{{{}}}\n", a));
    }
    if let Some(d) = doc_date(doc) {
        out.push_str(&format!("\\date{{{}}}\n", escape_latex(&d)));
    }

    out.push_str("\n\\begin{document}\n\\maketitle\n");

    if let Some(abs) = &abstract_text {
        out.push_str("\\begin{abstract}\n");
        out.push_str(&render_prose_latex(abs));
        out.push_str("\n\\end{abstract}\n\n");
    }

    latex_body(&doc.blocks, format, &mut out);

    out.push_str("\n\\end{document}\n");
    out
}

// ───────────────────────────────────────────────────────────────────────────
// Report templates
// ───────────────────────────────────────────────────────────────────────────

fn latex_report(doc: &SurfDoc, format: Format) -> String {
    let title = doc_title(doc);
    let authors = doc_authors(doc);
    let date = doc_date(doc);
    let instructor = doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm_extra(fm, &["instructor", "professor"]));
    let course = doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm_extra(fm, &["course", "class"]));
    let institution = doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm_extra(fm, &["institution", "affiliation", "university"]));

    let mut out = String::with_capacity(8192);
    out.push_str("\\documentclass[12pt]{article}\n");
    out.push_str("\\usepackage[margin=1in]{geometry}\n");
    out.push_str("\\usepackage{setspace}\n");
    push_common_preamble(&mut out);
    // Reports do not number their headings.
    out.push_str("\\setcounter{secnumdepth}{0}\n");

    out.push_str("\n\\begin{document}\n\\doublespacing\n");

    match format {
        Format::Mla => {
            // Top-left double-spaced heading block, then a centered title.
            let mut lines: Vec<String> = Vec::new();
            if !authors.is_empty() {
                lines.push(authors.join(", "));
            }
            if let Some(i) = &instructor {
                lines.push(i.clone());
            }
            if let Some(c) = &course {
                lines.push(c.clone());
            }
            if let Some(d) = &date {
                lines.push(d.clone());
            }
            let block: Vec<String> = lines.iter().map(|l| escape_latex(l)).collect();
            out.push_str(&format!("\\noindent {}\n\n", block.join("\\\\\n")));
            out.push_str(&format!(
                "\\begin{{center}}{}\\end{{center}}\n\n",
                escape_latex(&title)
            ));
        }
        _ => {
            // APA / Chicago title page.
            out.push_str("\\begin{titlepage}\n\\centering\n\\vspace*{3in}\n");
            out.push_str(&format!("{{\\bfseries {}}}\\\\[2em]\n", escape_latex(&title)));
            if !authors.is_empty() {
                out.push_str(&format!("{}\\\\[1em]\n", escape_latex(&authors.join(", "))));
            }
            if let Some(inst) = &institution {
                out.push_str(&format!("{}\\\\[1em]\n", escape_latex(inst)));
            }
            if let Some(c) = &course {
                out.push_str(&format!("{}\\\\[1em]\n", escape_latex(c)));
            }
            if let Some(i) = &instructor {
                out.push_str(&format!("{}\\\\[1em]\n", escape_latex(i)));
            }
            if let Some(d) = &date {
                out.push_str(&format!("{}\\\\\n", escape_latex(d)));
            }
            out.push_str("\\end{titlepage}\n\n");
        }
    }

    latex_body(&doc.blocks, format, &mut out);

    out.push_str("\n\\end{document}\n");
    out
}

// ───────────────────────────────────────────────────────────────────────────
// Generic (non paper/report) document
// ───────────────────────────────────────────────────────────────────────────

fn latex_generic(doc: &SurfDoc) -> String {
    let title = doc_title(doc);
    let authors = doc_authors(doc);

    let mut out = String::with_capacity(8192);
    out.push_str("\\documentclass[11pt]{article}\n");
    push_common_preamble(&mut out);
    out.push_str(&format!("\\title{{{}}}\n", escape_latex(&title)));
    if !authors.is_empty() {
        out.push_str(&format!("\\author{{{}}}\n", escape_latex(&authors.join(", "))));
    }
    if let Some(d) = doc_date(doc) {
        out.push_str(&format!("\\date{{{}}}\n", escape_latex(&d)));
    }
    out.push_str("\n\\begin{document}\n\\maketitle\n");
    let style = crate::citation::active_style(doc.front_matter.as_ref().and_then(|fm| fm.format));
    latex_body(&doc.blocks, style, &mut out);
    out.push_str("\n\\end{document}\n");
    out
}

fn push_common_preamble(out: &mut String) {
    out.push_str("\\usepackage[utf8]{inputenc}\n");
    out.push_str("\\usepackage[T1]{fontenc}\n");
    out.push_str("\\usepackage{graphicx}\n");
    out.push_str("\\usepackage[normalem]{ulem}\n");
    out.push_str("\\usepackage{hyperref}\n");
    out.push_str("\\usepackage{cite}\n");
}

// ───────────────────────────────────────────────────────────────────────────
// Body block rendering
// ───────────────────────────────────────────────────────────────────────────

fn latex_body(blocks: &[Block], style: Format, out: &mut String) {
    for b in blocks {
        match b {
            Block::Cite { .. } | Block::Site { .. } | Block::Style { .. } => {}
            Block::Bibliography { style: bstyle, .. } => {
                latex_bibliography(bstyle.unwrap_or(style), out);
            }
            _ => latex_block(b, out),
        }
    }
}

fn latex_block(b: &Block, out: &mut String) {
    match b {
        Block::Markdown { content, .. } => {
            out.push_str(&render_prose_latex(content));
            out.push_str("\n\n");
        }
        Block::Code { lang, content, .. } => {
            let _ = lang;
            out.push_str("\\begin{verbatim}\n");
            out.push_str(content);
            out.push_str("\n\\end{verbatim}\n\n");
        }
        Block::Quote {
            content,
            attribution,
            ..
        } => {
            out.push_str("\\begin{quote}\n");
            out.push_str(&render_prose_latex(content));
            if let Some(a) = attribution {
                out.push_str(&format!("\n\n\\hfill --- {}", escape_latex(a)));
            }
            out.push_str("\n\\end{quote}\n\n");
        }
        Block::Callout {
            title, content, ..
        } => {
            out.push_str("\\begin{quote}\n");
            if let Some(t) = title {
                out.push_str(&format!("\\textbf{{{}}}\\\\\n", escape_latex(t)));
            }
            out.push_str(&render_prose_latex(content));
            out.push_str("\n\\end{quote}\n\n");
        }
        Block::Summary { content, .. } => {
            out.push_str("\\begin{quote}\n\\textbf{Summary}\\\\\n");
            out.push_str(&render_prose_latex(content));
            out.push_str("\n\\end{quote}\n\n");
        }
        Block::Data { headers, rows, .. } | Block::PricingTable { headers, rows, .. } => {
            latex_table(headers, rows, out);
        }
        Block::Comparison { headers, rows, .. } => {
            latex_table(headers, rows, out);
        }
        Block::Figure {
            src,
            caption,
            width,
            ..
        } => {
            out.push_str("\\begin{figure}[h]\n\\centering\n");
            let w = width
                .as_deref()
                .filter(|w| !w.is_empty())
                .unwrap_or("0.8\\linewidth");
            // Numeric width -> treat as a fraction of \linewidth.
            let wspec = if w.chars().all(|c| c.is_ascii_digit() || c == '.') {
                format!("{}\\linewidth", w)
            } else if w.ends_with("%") {
                let frac = w.trim_end_matches('%').trim();
                format!("{}\\linewidth", frac.parse::<f64>().unwrap_or(80.0) / 100.0)
            } else {
                w.to_string()
            };
            out.push_str(&format!(
                "\\includegraphics[width={}]{{{}}}\n",
                wspec,
                escape_latex_path(src)
            ));
            if let Some(c) = caption {
                out.push_str(&format!("\\caption{{{}}}\n", md_inline_to_latex(c)));
            }
            out.push_str("\\end{figure}\n\n");
        }
        Block::Section {
            headline,
            subtitle,
            children,
            content,
            ..
        } => {
            if let Some(h) = headline {
                out.push_str(&format!("\\section{{{}}}\n", escape_latex(h)));
            }
            if let Some(s) = subtitle {
                out.push_str(&format!("\\textit{{{}}}\n\n", escape_latex(s)));
            }
            if children.is_empty() {
                out.push_str(&render_prose_latex(content));
                out.push_str("\n\n");
            } else {
                for c in children {
                    latex_block(c, out);
                }
            }
        }
        Block::Chart {
            chart_type,
            title,
            data,
            ..
        } => {
            out.push_str("\\begin{figure}[h]\n\\centering\n");
            if let Some(d) = data {
                let mut headers = vec![String::from("")];
                headers.extend(d.series.iter().map(|s| s.name.clone()));
                let rows: Vec<Vec<String>> = d
                    .categories
                    .iter()
                    .enumerate()
                    .map(|(i, cat)| {
                        let mut row = vec![cat.clone()];
                        for s in &d.series {
                            row.push(
                                s.values
                                    .get(i)
                                    .map(|v| format!("{}", v))
                                    .unwrap_or_default(),
                            );
                        }
                        row
                    })
                    .collect();
                latex_table(&headers, &rows, out);
            } else {
                out.push_str(&format!(
                    "\\fbox{{[{} chart]}}\n",
                    escape_latex(crate::render_html::chart_type_str(*chart_type))
                ));
            }
            if let Some(t) = title {
                out.push_str(&format!("\\caption{{{}}}\n", escape_latex(t)));
            }
            out.push_str("\\end{figure}\n\n");
        }
        Block::Diagram { title, content, .. } => {
            out.push_str("\\begin{figure}[h]\n\\centering\n\\begin{verbatim}\n");
            out.push_str(content);
            out.push_str("\n\\end{verbatim}\n");
            if let Some(t) = title {
                out.push_str(&format!("\\caption{{{}}}\n", escape_latex(t)));
            }
            out.push_str("\\end{figure}\n\n");
        }
        Block::Divider { .. } => {
            out.push_str("\\par\\noindent\\rule{\\linewidth}{0.4pt}\\par\n\n");
        }
        Block::Details { title, content, .. } => {
            if let Some(t) = title {
                out.push_str(&format!("\\textbf{{{}}}\\\\\n", escape_latex(t)));
            }
            out.push_str(&render_prose_latex(content));
            out.push_str("\n\n");
        }
        // Metadata / interactive / web-only blocks: no academic representation.
        _ => {}
    }
}

fn latex_table(headers: &[String], rows: &[Vec<String>], out: &mut String) {
    let ncols = if !headers.is_empty() {
        headers.len()
    } else if let Some(r) = rows.first() {
        r.len()
    } else {
        return;
    };
    let colspec = "l".repeat(ncols);
    out.push_str(&format!("\\begin{{tabular}}{{{}}}\n\\hline\n", colspec));
    if !headers.is_empty() {
        let cells: Vec<String> = headers.iter().map(|h| format!("\\textbf{{{}}}", escape_latex(h))).collect();
        out.push_str(&format!("{} \\\\\n\\hline\n", cells.join(" & ")));
    }
    for row in rows {
        let cells: Vec<String> = row.iter().map(|c| md_inline_to_latex(c)).collect();
        out.push_str(&format!("{} \\\\\n", cells.join(" & ")));
    }
    out.push_str("\\hline\n\\end{tabular}\n\n");
}

// ───────────────────────────────────────────────────────────────────────────
// Bibliography
// ───────────────────────────────────────────────────────────────────────────

fn latex_bibliography(style: Format, out: &mut String) {
    with_active(|ctx| {
        let refs = match ctx {
            Some(c) if !c.references.is_empty() => {
                if style == c.style {
                    ordered_references(c)
                } else {
                    c.references.clone()
                }
            }
            _ => return,
        };
        if is_numbered(style) {
            out.push_str("\\begin{thebibliography}{99}\n");
            for (i, line) in reference_list(&refs, style).iter().enumerate() {
                let body = line.splitn(2, "] ").nth(1).unwrap_or(line.as_str());
                let key = refs.get(i).map(|r| r.key.as_str()).unwrap_or("ref");
                out.push_str(&format!(
                    "\\bibitem{{{}}} {}\n",
                    key,
                    md_inline_to_latex(body)
                ));
            }
            out.push_str("\\end{thebibliography}\n");
        } else {
            out.push_str(&format!(
                "\\section*{{{}}}\n",
                escape_latex(bibliography_heading(style))
            ));
            for line in reference_list(&refs, style) {
                out.push_str(&format!(
                    "\\noindent\\hangindent=0.5in\\hangafter=1 {}\\par\\medskip\n",
                    md_inline_to_latex(&line)
                ));
            }
        }
        out.push('\n');
    });
}

// ───────────────────────────────────────────────────────────────────────────
// Markdown → LaTeX conversion
// ───────────────────────────────────────────────────────────────────────────

/// Convert markdown prose to LaTeX, first substituting inline `[@key]` cites
/// (numbered styles → `\cite{…}`; author styles → the formatted in-text string).
fn render_prose_latex(text: &str) -> String {
    with_active(|ctx| {
        let cites = crate::inline::find_inline_cites(text);
        if cites.is_empty() {
            return md_block_to_latex(text);
        }
        // Splice citation placeholders that survive the markdown→LaTeX pass,
        // then substitute the rendered cite commands back in.
        let mut prepared = String::with_capacity(text.len());
        let mut replacements: Vec<(String, String)> = Vec::new();
        let mut last = 0;
        for (idx, (s, e, cr)) in cites.iter().enumerate() {
            prepared.push_str(&text[last..*s]);
            let token = format!("\u{1}CITE{idx}\u{2}");
            let rep = match ctx {
                Some(c) => latex_in_text(cr, c),
                None => String::new(),
            };
            prepared.push_str(&token);
            replacements.push((token, rep));
            last = *e;
        }
        prepared.push_str(&text[last..]);
        let mut converted = md_block_to_latex(&prepared);
        for (tok, rep) in replacements {
            converted = converted.replace(&tok, &rep);
        }
        converted
    })
}

fn latex_in_text(cr: &CiteRef, ctx: &CiteContext) -> String {
    if is_numbered(ctx.style) {
        if cr.items.len() == 1 {
            let it = &cr.items[0];
            match &it.locator {
                Some(loc) => format!("\\cite[{}]{{{}}}", escape_latex(loc), it.key),
                None => format!("\\cite{{{}}}", it.key),
            }
        } else {
            let keys: Vec<String> = cr.items.iter().map(|i| i.key.clone()).collect();
            format!("\\cite{{{}}}", keys.join(","))
        }
    } else {
        escape_latex(&format_in_text(&ctx.references, cr, ctx.style, &ctx.numbers))
    }
}

/// Block-level markdown → LaTeX (headings, lists, code fences, quotes, rules).
fn md_block_to_latex(md: &str) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = String::with_capacity(md.len());
    let mut i = 0;
    let mut in_code = false;
    let mut code_buf = String::new();

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("```") {
            if in_code {
                out.push_str("\\begin{verbatim}\n");
                out.push_str(code_buf.trim_end_matches('\n'));
                out.push_str("\n\\end{verbatim}\n");
                code_buf.clear();
                in_code = false;
            } else {
                in_code = true;
            }
            i += 1;
            continue;
        }
        if in_code {
            code_buf.push_str(line);
            code_buf.push('\n');
            i += 1;
            continue;
        }

        // Headings
        if let Some(rest) = line.strip_prefix("#### ") {
            out.push_str(&format!("\\paragraph{{{}}} ", md_inline_to_latex(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("### ") {
            out.push_str(&format!("\\subsubsection{{{}}}\n", md_inline_to_latex(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            out.push_str(&format!("\\subsection{{{}}}\n", md_inline_to_latex(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            out.push_str(&format!("\\section{{{}}}\n", md_inline_to_latex(rest)));
            i += 1;
            continue;
        }

        let trimmed = line.trim();

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push_str("\\par\\noindent\\rule{\\linewidth}{0.4pt}\\par\n");
            i += 1;
            continue;
        }

        // Unordered list group
        if is_ul_item(line) {
            out.push_str("\\begin{itemize}\n");
            while i < lines.len() && is_ul_item(lines[i]) {
                let item = ul_item_text(lines[i]);
                out.push_str(&format!("  \\item {}\n", md_inline_to_latex(item)));
                i += 1;
            }
            out.push_str("\\end{itemize}\n");
            continue;
        }

        // Ordered list group
        if split_ordered_list(line).is_some() {
            out.push_str("\\begin{enumerate}\n");
            while i < lines.len() {
                if let Some((_, rest)) = split_ordered_list(lines[i]) {
                    out.push_str(&format!("  \\item {}\n", md_inline_to_latex(rest)));
                    i += 1;
                } else {
                    break;
                }
            }
            out.push_str("\\end{enumerate}\n");
            continue;
        }

        // Blockquote group
        if trimmed.starts_with('>') {
            out.push_str("\\begin{quote}\n");
            while i < lines.len() && lines[i].trim().starts_with('>') {
                let q = lines[i].trim().trim_start_matches('>').trim_start();
                out.push_str(&format!("{}\n", md_inline_to_latex(q)));
                i += 1;
            }
            out.push_str("\\end{quote}\n");
            continue;
        }

        // GFM table group
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let mut tbl_rows: Vec<Vec<String>> = Vec::new();
            while i < lines.len() {
                let t = lines[i].trim();
                if !(t.starts_with('|') && t.ends_with('|')) {
                    break;
                }
                // Skip separator rows
                if !t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
                    let cells: Vec<String> = t
                        .trim_matches('|')
                        .split('|')
                        .map(|c| c.trim().to_string())
                        .collect();
                    tbl_rows.push(cells);
                }
                i += 1;
            }
            if !tbl_rows.is_empty() {
                let headers = tbl_rows.remove(0);
                latex_table(&headers, &tbl_rows, &mut out);
            }
            continue;
        }

        // Blank line → paragraph break
        if trimmed.is_empty() {
            out.push('\n');
            i += 1;
            continue;
        }

        // Regular paragraph line
        out.push_str(&md_inline_to_latex(line));
        out.push('\n');
        i += 1;
    }

    if in_code {
        out.push_str("\\begin{verbatim}\n");
        out.push_str(code_buf.trim_end_matches('\n'));
        out.push_str("\n\\end{verbatim}\n");
    }

    out
}

fn is_ul_item(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ") || t.starts_with("* ")
}

fn ul_item_text(line: &str) -> &str {
    let t = line.trim_start();
    t.strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .unwrap_or(t)
}

/// Inline markdown → LaTeX (bold/italic/code/links/strikethrough), escaping the
/// remainder.
fn md_inline_to_latex(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;

    while i < len {
        if chars[i] == '\\' && i + 1 < len {
            out.push_str(&escape_latex_char(chars[i + 1]));
            i += 2;
            continue;
        }
        // Image: ![alt](src)
        if chars[i] == '!' && i + 1 < len && chars[i + 1] == '[' {
            if let Some((_, src, end)) = parse_link(&chars, i + 1) {
                out.push_str(&format!(
                    "\\includegraphics[width=0.8\\linewidth]{{{}}}",
                    escape_latex_path(&src)
                ));
                i = end;
                continue;
            }
        }
        // Link: [text](url)
        if chars[i] == '[' {
            if let Some((t, href, end)) = parse_link(&chars, i) {
                out.push_str(&format!(
                    "\\href{{{}}}{{{}}}",
                    escape_url(&href),
                    md_inline_to_latex(&t)
                ));
                i = end;
                continue;
            }
        }
        // Inline code
        if chars[i] == '`' {
            if let Some((code, end)) = parse_backtick_code(&chars, i) {
                out.push_str(&format!("\\texttt{{{}}}", escape_latex(&code)));
                i = end;
                continue;
            }
        }
        // Bold
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some((c, end)) = parse_delimited(&chars, i, "**") {
                out.push_str(&format!("\\textbf{{{}}}", md_inline_to_latex(&c)));
                i = end;
                continue;
            }
        }
        if i + 1 < len && chars[i] == '_' && chars[i + 1] == '_' {
            if let Some((c, end)) = parse_delimited(&chars, i, "__") {
                out.push_str(&format!("\\textbf{{{}}}", md_inline_to_latex(&c)));
                i = end;
                continue;
            }
        }
        // Italic
        if chars[i] == '*' && i + 1 < len && chars[i + 1] != '*' && !chars[i + 1].is_whitespace() {
            if let Some((c, end)) = parse_delimited(&chars, i, "*") {
                out.push_str(&format!("\\textit{{{}}}", md_inline_to_latex(&c)));
                i = end;
                continue;
            }
        }
        if chars[i] == '_'
            && i + 1 < len
            && chars[i + 1] != '_'
            && (i == 0 || !chars[i - 1].is_alphanumeric())
        {
            if let Some((c, end)) = parse_delimited(&chars, i, "_") {
                out.push_str(&format!("\\textit{{{}}}", md_inline_to_latex(&c)));
                i = end;
                continue;
            }
        }
        // Strikethrough
        if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
            if let Some((c, end)) = parse_delimited(&chars, i, "~~") {
                out.push_str(&format!("\\sout{{{}}}", md_inline_to_latex(&c)));
                i = end;
                continue;
            }
        }

        out.push_str(&escape_latex_char(chars[i]));
        i += 1;
    }

    out
}

// ───────────────────────────────────────────────────────────────────────────
// Escaping
// ───────────────────────────────────────────────────────────────────────────

/// Escape LaTeX special characters in a run of text.
fn escape_latex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push_str(&escape_latex_char(c));
    }
    out
}

fn escape_latex_char(c: char) -> String {
    match c {
        '\\' => "\\textbackslash{}".to_string(),
        '&' => "\\&".to_string(),
        '%' => "\\%".to_string(),
        '$' => "\\$".to_string(),
        '#' => "\\#".to_string(),
        '_' => "\\_".to_string(),
        '{' => "\\{".to_string(),
        '}' => "\\}".to_string(),
        '~' => "\\textasciitilde{}".to_string(),
        '^' => "\\textasciicircum{}".to_string(),
        other => other.to_string(),
    }
}

/// Escape a URL for use inside `\href{...}{}` (hyperref tolerates most chars; we
/// guard the ones that would break the argument).
fn escape_url(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '#' => out.push_str("\\#"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a file path for `\includegraphics{...}` (paths shouldn't contain TeX
/// specials, but guard the common offenders).
fn escape_latex_path(s: &str) -> String {
    s.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> SurfDoc {
        crate::parse(src).doc
    }

    #[test]
    fn escapes_special_chars() {
        assert_eq!(escape_latex("a & b % c"), "a \\& b \\% c");
        assert_eq!(escape_latex("snake_case"), "snake\\_case");
        assert_eq!(escape_latex("100% #1"), "100\\% \\#1");
        assert_eq!(escape_latex("a$b"), "a\\$b");
    }

    #[test]
    fn ieee_paper_uses_ieeetran() {
        let src = "---\ntitle: A Study\ntype: paper\nformat: ieee\nauthor: Jane Doe\n---\n\n# Intro\n\nText.\n";
        let tex = to_latex(&parse(src));
        assert!(tex.contains("{IEEEtran}"), "missing IEEEtran class: {tex}");
        assert!(tex.contains("\\IEEEauthorblockN"));
        assert!(tex.contains("\\begin{document}"));
        assert!(tex.contains("\\maketitle"));
    }

    #[test]
    fn mla_report_double_spaced_works_cited() {
        let src = "---\ntitle: My Essay\ntype: report\nformat: mla\nauthor: Sam Student\ninstructor: Dr. Smith\ncourse: ENG 101\n---\n\n# Body\n\nClaim [@a].\n\n::cite[key=a type=book]\nauthor = Author, One\ntitle = A Book\npublisher = Press\nyear = 2020\n::\n\n::bibliography\n::\n";
        let tex = to_latex(&parse(src));
        assert!(tex.contains("\\usepackage{setspace}"));
        assert!(tex.contains("\\doublespacing"));
        assert!(tex.contains("Dr. Smith"));
        assert!(tex.contains("\\section*{Works Cited}"), "missing Works Cited: {tex}");
        // MLA in-text is author-only.
        assert!(tex.contains("(Author)"));
    }

    #[test]
    fn apa_report_references_heading() {
        let src = "---\ntitle: Report\ntype: report\nformat: apa\nauthor: A B\n---\n\n# Body\n\nText [@a].\n\n::cite[key=a type=article]\nauthor = Q, R\ntitle = T\nyear = 2020\n::\n\n::bibliography\n::\n";
        let tex = to_latex(&parse(src));
        assert!(tex.contains("\\section*{References}"), "missing References: {tex}");
        assert!(tex.contains("\\begin{titlepage}"));
    }

    #[test]
    fn chicago_report_bibliography_heading() {
        let src = "---\ntitle: Report\ntype: report\nformat: chicago\nauthor: A B\n---\n\n# Body\n\nText [@a].\n\n::cite[key=a type=book]\nauthor = Q, R\ntitle = T\npublisher = P\nyear = 2020\n::\n\n::bibliography\n::\n";
        let tex = to_latex(&parse(src));
        assert!(tex.contains("\\section*{Bibliography}"), "missing Bibliography: {tex}");
    }

    #[test]
    fn ieee_numbered_cite_and_thebibliography() {
        let src = "---\ntitle: P\ntype: paper\nformat: ieee\nauthor: X\n---\n\n# Body\n\nClaim [@a].\n\n::cite[key=a type=article]\nauthor = Q, R\ntitle = T\njournal = J\nyear = 2020\n::\n\n::bibliography\n::\n";
        let tex = to_latex(&parse(src));
        assert!(tex.contains("\\cite{a}"), "missing \\cite: {tex}");
        assert!(tex.contains("\\begin{thebibliography}"));
        assert!(tex.contains("\\bibitem{a}"));
    }

    #[test]
    fn to_latex_is_deterministic() {
        let src = "---\ntitle: P\ntype: paper\nformat: ieee\nauthor: Jane Doe; John Roe\nabstract: A short abstract.\n---\n\n# Intro\n\nWork [@a].\n\n::cite[key=a type=article]\nauthor = Q, R\ntitle = T\njournal = J\nyear = 2020\n::\n\n::bibliography\n::\n";
        let doc = parse(src);
        assert_eq!(to_latex(&doc), to_latex(&doc));
    }

    #[test]
    fn escapes_in_body_paragraph() {
        let src = "---\ntype: paper\nformat: article\n---\n\nCost is 50% of #1 in the_field.\n";
        let tex = to_latex(&parse(src));
        assert!(tex.contains("50\\%"));
        assert!(tex.contains("\\#1"));
        assert!(tex.contains("the\\_field"));
    }
}

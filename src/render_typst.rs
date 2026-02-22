//! Typst markup renderer.
//!
//! Converts a `SurfDoc` block tree into valid Typst markup text. The output can
//! be compiled by the Typst engine to produce PDF (or other formats).
//!
//! Markdown content within blocks is converted to Typst markup via the
//! [`md_to_typst`] helper. All user content is escaped to prevent Typst
//! injection.

use crate::types::*;

/// Base Typst template with page setup, colors, and reusable components.
pub(crate) const SURFDOC_TEMPLATE: &str = include_str!("../assets/surfdoc.typ");

/// Render a `SurfDoc` as a complete Typst document string.
///
/// The output includes the base template, front matter header, and all blocks
/// mapped to their Typst equivalents. This string can be compiled directly by
/// the Typst engine.
pub fn to_typst(doc: &SurfDoc) -> String {
    let mut out = String::with_capacity(8192);

    // Base template (page setup, colors, components)
    out.push_str(SURFDOC_TEMPLATE);
    out.push_str("\n\n");

    // Front matter header
    if let Some(ref fm) = doc.front_matter {
        render_front_matter(fm, &mut out);
    }

    // Render each block
    for block in &doc.blocks {
        render_block(block, &mut out);
        out.push('\n');
    }

    out
}

/// Render YAML front matter as a centered document header.
fn render_front_matter(fm: &FrontMatter, out: &mut String) {
    let has_title = fm.title.as_ref().is_some_and(|t| !t.is_empty());
    let has_meta = fm.author.is_some()
        || fm.created.is_some()
        || fm.status.is_some()
        || fm.tags.as_ref().is_some_and(|t| !t.is_empty());

    if !has_title && !has_meta {
        return;
    }

    out.push_str("#align(center)[\n");

    if let Some(ref title) = fm.title {
        out.push_str(&format!(
            "  #text(size: 2em, weight: \"bold\")[{}]\n",
            escape_typst(title)
        ));
    }

    // Metadata line: author · date · status
    let mut meta_parts: Vec<String> = Vec::new();
    if let Some(ref author) = fm.author {
        meta_parts.push(escape_typst(author));
    }
    if let Some(ref created) = fm.created {
        meta_parts.push(escape_typst(created));
    }
    if let Some(ref status) = fm.status {
        meta_parts.push(format!("{status:?}"));
    }
    if !meta_parts.is_empty() {
        out.push_str("  #v(0.5em)\n");
        out.push_str(&format!(
            "  #text(fill: luma(100))[{}]\n",
            meta_parts.join(" · ")
        ));
    }

    // Tags
    if let Some(ref tags) = fm.tags {
        if !tags.is_empty() {
            out.push_str("  #v(0.3em)\n");
            let tag_str: Vec<String> = tags.iter().map(|t| escape_typst(t)).collect();
            out.push_str(&format!(
                "  #text(size: 0.9em, fill: luma(120))[{}]\n",
                tag_str.join(" · ")
            ));
        }
    }

    out.push_str("]\n");
    out.push_str("#v(1em)\n");
    out.push_str("#line(length: 100%, stroke: 0.5pt + luma(200))\n");
    out.push_str("#v(1em)\n\n");
}

/// Render a single block to Typst markup, appending to `out`.
fn render_block(block: &Block, out: &mut String) {
    match block {
        Block::Markdown { content, .. } => {
            out.push_str(&md_to_typst(content));
            out.push('\n');
        }

        Block::Callout {
            callout_type,
            title,
            content,
            ..
        } => {
            let type_name = callout_type_str(*callout_type);
            let title_arg = match title {
                Some(t) => format!("\"{}\"", escape_typst(t)),
                None => "none".to_string(),
            };
            out.push_str(&format!(
                "#surfdoc-callout(\"{type_name}\", {title_arg})[\n{}\n]\n",
                md_to_typst(content)
            ));
        }

        Block::Data {
            headers, rows, ..
        } => {
            render_data_table(headers, rows, out);
        }

        Block::Code {
            lang, content, ..
        } => {
            let lang_attr = match lang {
                Some(l) if !l.is_empty() => format!(", lang: \"{}\"", escape_typst(l)),
                _ => String::new(),
            };
            // Use triple backticks for raw blocks
            out.push_str(&format!(
                "#raw(block: true{})\n```\n{}\n```\n",
                lang_attr, content
            ));
        }

        Block::Tasks { items, .. } => {
            for item in items {
                let marker = if item.done { "☑" } else { "☐" };
                let assignee = match &item.assignee {
                    Some(a) => format!(" #text(fill: luma(120))[\\@{}]", escape_typst(a)),
                    None => String::new(),
                };
                out.push_str(&format!(
                    "- {} {}{}\n",
                    marker,
                    md_to_typst_inline(&item.text),
                    assignee
                ));
            }
            out.push('\n');
        }

        Block::Decision {
            status,
            date,
            deciders,
            content,
            ..
        } => {
            let status_str = decision_status_str(*status);
            out.push_str(&format!("#surfdoc-decision-badge(\"{status_str}\")"));
            if let Some(d) = date {
                out.push_str(&format!(" #text(fill: luma(120))[{}]", escape_typst(d)));
            }
            if !deciders.is_empty() {
                let names: Vec<String> = deciders.iter().map(|d| escape_typst(d)).collect();
                out.push_str(&format!(
                    " #text(fill: luma(120))[— {}]",
                    names.join(", ")
                ));
            }
            out.push('\n');
            out.push_str(&md_to_typst(content));
            out.push('\n');
        }

        Block::Metric {
            label,
            value,
            trend,
            unit,
            ..
        } => {
            let trend_arg = match trend {
                Some(t) => format!("\"{}\"", trend_str(*t)),
                None => "none".to_string(),
            };
            let unit_arg = match unit {
                Some(u) => format!(", unit: \"{}\"", escape_typst(u)),
                None => String::new(),
            };
            out.push_str(&format!(
                "#surfdoc-metric(\"{}\", \"{}\"{}, trend: {})\n",
                escape_typst(label),
                escape_typst(value),
                unit_arg,
                trend_arg
            ));
        }

        Block::Summary { content, .. } => {
            out.push_str("#block(fill: luma(245), inset: 12pt, radius: 4pt, width: 100%)[\n");
            out.push_str("  *Summary* \\\n");
            out.push_str(&format!("  {}\n", md_to_typst(content)));
            out.push_str("]\n");
        }

        Block::Figure {
            src,
            caption,
            alt,
            width,
            ..
        } => {
            let width_attr = match width {
                Some(w) => format!(", width: {}", w),
                None => String::new(),
            };
            let alt_val = alt.as_deref().unwrap_or("");
            out.push_str(&format!(
                "#figure(\n  image(\"{}\"{}){}",
                escape_typst(src),
                width_attr,
                if !alt_val.is_empty() {
                    format!(",\n  supplement: none")
                } else {
                    String::new()
                }
            ));
            if let Some(cap) = caption {
                out.push_str(&format!(",\n  caption: [{}]", md_to_typst_inline(cap)));
            }
            out.push_str("\n)\n");
        }

        Block::Quote {
            content,
            attribution,
            ..
        } => {
            if let Some(attr) = attribution {
                out.push_str(&format!(
                    "#quote(attribution: [{}])[\n{}\n]\n",
                    escape_typst(attr),
                    md_to_typst(content)
                ));
            } else {
                out.push_str(&format!(
                    "#quote[\n{}\n]\n",
                    md_to_typst(content)
                ));
            }
        }

        Block::Tabs { tabs, .. } => {
            for tab in tabs {
                out.push_str(&format!("=== {}\n\n", escape_typst(&tab.label)));
                out.push_str(&md_to_typst(&tab.content));
                out.push('\n');
            }
        }

        Block::Columns { columns, .. } => {
            let n = columns.len();
            out.push_str(&format!("#grid(\n  columns: ({}),\n  gutter: 1em,\n", "1fr, ".repeat(n).trim_end_matches(", ")));
            for col in columns {
                out.push_str(&format!("  [\n    {}\n  ],\n", md_to_typst(&col.content)));
            }
            out.push_str(")\n");
        }

        Block::Section {
            headline,
            subtitle,
            children,
            content,
            ..
        } => {
            if let Some(h) = headline {
                out.push_str(&format!("== {}\n\n", escape_typst(h)));
            }
            if let Some(s) = subtitle {
                out.push_str(&format!(
                    "#text(fill: luma(100))[{}]\n\n",
                    escape_typst(s)
                ));
            }
            if children.is_empty() {
                out.push_str(&md_to_typst(content));
                out.push('\n');
            } else {
                for child in children {
                    render_block(child, out);
                    out.push('\n');
                }
            }
        }

        Block::Details {
            title, content, ..
        } => {
            if let Some(t) = title {
                out.push_str(&format!("*{}*\n\n", escape_typst(t)));
            }
            out.push_str(&md_to_typst(content));
            out.push('\n');
        }

        Block::Divider { label, .. } => {
            if let Some(l) = label {
                out.push_str(&format!(
                    "#v(0.5em)\n#align(center)[#text(fill: luma(150), size: 0.9em)[— {} —]]\n#v(0.5em)\n",
                    escape_typst(l)
                ));
            } else {
                out.push_str("#line(length: 100%, stroke: 0.5pt + luma(200))\n");
            }
        }

        Block::Hero {
            headline,
            subtitle,
            badge,
            buttons,
            content,
            ..
        } => {
            out.push_str("#align(center)[\n");
            if let Some(b) = badge {
                out.push_str(&format!(
                    "  #box(fill: luma(235), radius: 12pt, inset: (x: 10pt, y: 4pt))[#text(size: 0.85em)[{}]]\n  #v(0.5em)\n",
                    escape_typst(b)
                ));
            }
            if let Some(h) = headline {
                out.push_str(&format!(
                    "  #text(size: 2.5em, weight: \"bold\")[{}]\n",
                    escape_typst(h)
                ));
            }
            if let Some(s) = subtitle {
                out.push_str(&format!(
                    "  #v(0.3em)\n  #text(size: 1.2em, fill: luma(100))[{}]\n",
                    escape_typst(s)
                ));
            }
            if !buttons.is_empty() {
                out.push_str("  #v(0.5em)\n");
                for btn in buttons {
                    if btn.primary {
                        out.push_str(&format!(
                            "  #box(fill: surfdoc-blue, radius: 4pt, inset: (x: 12pt, y: 6pt))[#link(\"{}\")[#text(fill: white, weight: \"bold\")[{}]]]\n",
                            escape_typst(&btn.href),
                            escape_typst(&btn.label)
                        ));
                    } else {
                        out.push_str(&format!(
                            "  #box(stroke: 1pt + luma(200), radius: 4pt, inset: (x: 12pt, y: 6pt))[#link(\"{}\")[{}]]\n",
                            escape_typst(&btn.href),
                            escape_typst(&btn.label)
                        ));
                    }
                    out.push_str("  #h(0.5em)\n");
                }
            }
            if !content.is_empty() {
                out.push_str(&format!("  #v(0.5em)\n  {}\n", md_to_typst(content)));
            }
            out.push_str("]\n#v(1em)\n");
        }

        Block::Features { cards, cols, .. } => {
            let ncols = cols.unwrap_or(3);
            let cols_str = (0..ncols).map(|_| "1fr").collect::<Vec<_>>().join(", ");
            out.push_str(&format!(
                "#grid(\n  columns: ({}),\n  gutter: 1.5em,\n",
                cols_str
            ));
            for card in cards {
                out.push_str("  block(stroke: 0.5pt + luma(220), radius: 6pt, inset: 12pt, width: 100%)[\n");
                out.push_str(&format!(
                    "    *{}*\n",
                    escape_typst(&card.title)
                ));
                if !card.body.is_empty() {
                    out.push_str(&format!("    \\\n    {}\n", md_to_typst_inline(&card.body)));
                }
                if let (Some(label), Some(href)) = (&card.link_label, &card.link_href) {
                    out.push_str(&format!(
                        "    \\\n    #link(\"{}\")[{}]\n",
                        escape_typst(href),
                        escape_typst(label)
                    ));
                }
                out.push_str("  ],\n");
            }
            out.push_str(")\n");
        }

        Block::Steps { steps, .. } => {
            for (i, step) in steps.iter().enumerate() {
                out.push_str(&format!(
                    "+ *{}*",
                    escape_typst(&step.title)
                ));
                if let Some(ref time) = step.time {
                    out.push_str(&format!(
                        " #text(fill: luma(120), size: 0.9em)[{}]",
                        escape_typst(time)
                    ));
                }
                if !step.body.is_empty() {
                    out.push_str(&format!(" — {}", md_to_typst_inline(&step.body)));
                }
                out.push('\n');
                if i < steps.len() - 1 {
                    // spacing between steps
                }
            }
            out.push('\n');
        }

        Block::Stats { items, .. } => {
            let n = items.len();
            let cols_str = (0..n).map(|_| "1fr").collect::<Vec<_>>().join(", ");
            out.push_str(&format!(
                "#grid(\n  columns: ({}),\n  gutter: 1em,\n",
                cols_str
            ));
            for item in items {
                out.push_str("  align(center)[\n");
                out.push_str(&format!(
                    "    #text(size: 2em, weight: \"bold\")[{}]\n    \\\n    {}\n",
                    escape_typst(&item.value),
                    escape_typst(&item.label)
                ));
                out.push_str("  ],\n");
            }
            out.push_str(")\n");
        }

        Block::Comparison {
            headers,
            rows,
            highlight,
            ..
        } => {
            render_comparison_table(headers, rows, highlight.as_deref(), out);
        }

        Block::Cta {
            label, href, primary, ..
        } => {
            if *primary {
                out.push_str(&format!(
                    "#align(center)[#box(fill: surfdoc-blue, radius: 4pt, inset: (x: 14pt, y: 8pt))[#link(\"{}\")[#text(fill: white, weight: \"bold\")[{}]]]]\n",
                    escape_typst(href),
                    escape_typst(label)
                ));
            } else {
                out.push_str(&format!(
                    "#align(center)[#box(stroke: 1pt + surfdoc-blue, radius: 4pt, inset: (x: 14pt, y: 8pt))[#link(\"{}\")[#text(fill: surfdoc-blue)[{}]]]]\n",
                    escape_typst(href),
                    escape_typst(label)
                ));
            }
        }

        Block::Nav { items, .. } => {
            let links: Vec<String> = items
                .iter()
                .map(|item| {
                    format!(
                        "#link(\"{}\")[{}]",
                        escape_typst(&item.href),
                        escape_typst(&item.label)
                    )
                })
                .collect();
            out.push_str(&format!(
                "#text(size: 0.9em)[{}]\n",
                links.join(" #h(1em) ")
            ));
        }

        Block::Form { fields, .. } => {
            out.push_str("#block(stroke: 0.5pt + luma(200), radius: 4pt, inset: 12pt, width: 100%)[\n");
            out.push_str("  *Form Fields*\n");
            for field in fields {
                let required = if field.required { " \\*" } else { "" };
                out.push_str(&format!(
                    "  - *{}*{} #text(fill: luma(120))[({:?})]\n",
                    escape_typst(&field.label),
                    required,
                    field.field_type
                ));
            }
            out.push_str("]\n");
        }

        Block::Gallery { items, columns, .. } => {
            let ncols = columns.unwrap_or(3);
            let cols_str = (0..ncols).map(|_| "1fr").collect::<Vec<_>>().join(", ");
            out.push_str(&format!(
                "#grid(\n  columns: ({}),\n  gutter: 1em,\n",
                cols_str
            ));
            for item in items {
                out.push_str(&format!("  figure(\n    image(\"{}\"),\n", escape_typst(&item.src)));
                if let Some(ref cap) = item.caption {
                    out.push_str(&format!("    caption: [{}],\n", escape_typst(cap)));
                }
                out.push_str("  ),\n");
            }
            out.push_str(")\n");
        }

        Block::Footer {
            sections,
            copyright,
            social,
            ..
        } => {
            out.push_str("#line(length: 100%, stroke: 0.5pt + luma(200))\n#v(0.5em)\n");
            if !sections.is_empty() {
                let n = sections.len();
                let cols_str = (0..n).map(|_| "1fr").collect::<Vec<_>>().join(", ");
                out.push_str(&format!(
                    "#grid(\n  columns: ({}),\n  gutter: 1em,\n",
                    cols_str
                ));
                for section in sections {
                    out.push_str(&format!(
                        "  [\n    *{}*\n",
                        escape_typst(&section.heading)
                    ));
                    for link in &section.links {
                        out.push_str(&format!(
                            "    - #link(\"{}\")[{}]\n",
                            escape_typst(&link.href),
                            escape_typst(&link.label)
                        ));
                    }
                    out.push_str("  ],\n");
                }
                out.push_str(")\n");
            }
            if let Some(cr) = copyright {
                out.push_str(&format!(
                    "#v(0.5em)\n#align(center)[#text(size: 0.85em, fill: luma(120))[{}]]\n",
                    escape_typst(cr)
                ));
            }
            if !social.is_empty() {
                let links: Vec<String> = social
                    .iter()
                    .map(|s| {
                        format!(
                            "#link(\"{}\")[{}]",
                            escape_typst(&s.href),
                            escape_typst(&s.platform)
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "#align(center)[#text(size: 0.85em)[{}]]\n",
                    links.join(" #h(0.5em) ")
                ));
            }
        }

        Block::Testimonial {
            content,
            author,
            role,
            company,
            ..
        } => {
            let mut attr_parts: Vec<String> = Vec::new();
            if let Some(a) = author {
                attr_parts.push(escape_typst(a));
            }
            if let Some(r) = role {
                attr_parts.push(escape_typst(r));
            }
            if let Some(c) = company {
                attr_parts.push(escape_typst(c));
            }
            if attr_parts.is_empty() {
                out.push_str(&format!(
                    "#quote[\n{}\n]\n",
                    md_to_typst(content)
                ));
            } else {
                out.push_str(&format!(
                    "#quote(attribution: [{}])[\n{}\n]\n",
                    attr_parts.join(", "),
                    md_to_typst(content)
                ));
            }
        }

        Block::PricingTable {
            headers, rows, ..
        } => {
            render_data_table(headers, rows, out);
        }

        Block::Faq { items, .. } => {
            for item in items {
                out.push_str(&format!(
                    "*Q: {}*\n\n{}\n\n",
                    escape_typst(&item.question),
                    md_to_typst(&item.answer)
                ));
            }
        }

        Block::Embed {
            src, title, ..
        } => {
            let label = title
                .as_deref()
                .unwrap_or(src.as_str());
            out.push_str(&format!(
                "#text(fill: luma(120))[\\[Embedded: #link(\"{}\")[{}]\\]]\n",
                escape_typst(src),
                escape_typst(label)
            ));
        }

        Block::BeforeAfter {
            before_items,
            after_items,
            transition,
            ..
        } => {
            out.push_str("*Before:*\n");
            for item in before_items {
                out.push_str(&format!(
                    "- *{}* — {}\n",
                    escape_typst(&item.label),
                    escape_typst(&item.detail)
                ));
            }
            let arrow = transition.as_deref().unwrap_or("↓");
            out.push_str(&format!("\n#align(center)[#text(size: 1.5em)[{}]]\n\n", escape_typst(arrow)));
            out.push_str("*After:*\n");
            for item in after_items {
                out.push_str(&format!(
                    "- *{}* — {}\n",
                    escape_typst(&item.label),
                    escape_typst(&item.detail)
                ));
            }
            out.push('\n');
        }

        Block::Pipeline { steps, .. } => {
            let labels: Vec<String> = steps
                .iter()
                .map(|s| {
                    let desc = match &s.description {
                        Some(d) => format!(" #text(fill: luma(120), size: 0.9em)[{}]", escape_typst(d)),
                        None => String::new(),
                    };
                    format!("*{}*{}", escape_typst(&s.label), desc)
                })
                .collect();
            out.push_str(&format!(
                "#align(center)[{}]\n",
                labels.join(" #h(0.3em) → #h(0.3em) ")
            ));
        }

        Block::ProductCard {
            title,
            subtitle,
            badge,
            body,
            features,
            cta_label,
            cta_href,
            ..
        } => {
            out.push_str("#block(stroke: 0.5pt + luma(200), radius: 6pt, inset: 14pt, width: 100%)[\n");
            if let Some(b) = badge {
                out.push_str(&format!(
                    "  #box(fill: surfdoc-blue, radius: 2pt, inset: (x: 6pt, y: 2pt))[#text(fill: white, size: 0.8em)[{}]]\n  #v(0.3em)\n",
                    escape_typst(b)
                ));
            }
            out.push_str(&format!(
                "  #text(size: 1.3em, weight: \"bold\")[{}]\n",
                escape_typst(title)
            ));
            if let Some(s) = subtitle {
                out.push_str(&format!(
                    "  \\\n  #text(fill: luma(100))[{}]\n",
                    escape_typst(s)
                ));
            }
            if !body.is_empty() {
                out.push_str(&format!("  \\\n  {}\n", md_to_typst_inline(body)));
            }
            if !features.is_empty() {
                out.push_str("  \\\n");
                for f in features {
                    out.push_str(&format!("  - {}\n", escape_typst(f)));
                }
            }
            if let (Some(label), Some(href)) = (cta_label, cta_href) {
                out.push_str(&format!(
                    "  #v(0.5em)\n  #link(\"{}\")[#text(fill: surfdoc-blue, weight: \"bold\")[{}]]\n",
                    escape_typst(href),
                    escape_typst(label)
                ));
            }
            out.push_str("]\n");
        }

        Block::Logo { src, size, .. } => {
            let width = size.map_or("60pt".to_string(), |s| format!("{s}pt"));
            out.push_str(&format!(
                "#align(center)[#image(\"{}\", width: {})]\n",
                escape_typst(src),
                width
            ));
        }

        Block::Toc { depth, .. } => {
            out.push_str(&format!("#outline(depth: {})\n", depth));
        }

        Block::HeroImage { src, alt, .. } => {
            let _ = alt; // alt text not rendered visually in PDF
            out.push_str(&format!(
                "#image(\"{}\", width: 100%)\n",
                escape_typst(src)
            ));
        }

        // Metadata blocks — skip (no visual representation in PDF)
        Block::Site { .. } | Block::Style { .. } | Block::Page { .. } => {}

        Block::Unknown { name, content, .. } => {
            out.push_str(&format!(
                "#block(stroke: 0.5pt + luma(180), radius: 4pt, inset: 10pt, width: 100%)[\n  #text(fill: luma(120), size: 0.9em)[Unknown block: {}]\n  \\\n  {}\n]\n",
                escape_typst(name),
                md_to_typst(content)
            ));
        }
    }
}

/// Render a data table (used by Data, PricingTable).
fn render_data_table(headers: &[String], rows: &[Vec<String>], out: &mut String) {
    let ncols = if !headers.is_empty() {
        headers.len()
    } else if let Some(first) = rows.first() {
        first.len()
    } else {
        return;
    };

    out.push_str(&format!("#table(\n  columns: {},\n", ncols));

    // Headers
    if !headers.is_empty() {
        for h in headers {
            out.push_str(&format!("  [*{}*],\n", escape_typst(h)));
        }
    }

    // Rows
    for row in rows {
        for cell in row {
            out.push_str(&format!("  [{}],\n", md_to_typst_inline(cell)));
        }
    }

    out.push_str(")\n");
}

/// Render a comparison table with green/red markers for yes/no cells.
fn render_comparison_table(
    headers: &[String],
    rows: &[Vec<String>],
    highlight: Option<&str>,
    out: &mut String,
) {
    let ncols = if !headers.is_empty() {
        headers.len()
    } else if let Some(first) = rows.first() {
        first.len()
    } else {
        return;
    };

    out.push_str(&format!("#table(\n  columns: {},\n", ncols));

    // Headers (with optional highlight column)
    if !headers.is_empty() {
        for h in headers {
            let is_highlight = highlight.is_some_and(|hl| hl == h);
            if is_highlight {
                out.push_str(&format!(
                    "  [*#text(fill: surfdoc-blue)[{}]*],\n",
                    escape_typst(h)
                ));
            } else {
                out.push_str(&format!("  [*{}*],\n", escape_typst(h)));
            }
        }
    }

    // Rows with yes/no marker conversion
    for row in rows {
        for cell in row {
            let trimmed = cell.trim().to_lowercase();
            match trimmed.as_str() {
                "yes" | "true" | "✓" | "✔" | "check" => {
                    out.push_str("  [#text(fill: surfdoc-green)[●]],\n");
                }
                "no" | "false" | "✗" | "✘" | "cross" | "-" | "—" => {
                    out.push_str("  [#text(fill: luma(200))[●]],\n");
                }
                _ => {
                    out.push_str(&format!("  [{}],\n", md_to_typst_inline(cell)));
                }
            }
        }
    }

    out.push_str(")\n");
}

// --- Markdown to Typst conversion ---

/// Convert a markdown string to Typst markup (block-level).
///
/// Handles headings, bold, italic, links, images, code, lists, and tables.
pub fn md_to_typst(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_content = String::new();

    for line in md.lines() {
        // Handle fenced code blocks
        if line.starts_with("```") {
            if in_code_block {
                // End code block
                out.push_str(&format!(
                    "```{}\n{}\n```\n",
                    code_lang,
                    code_content.trim_end()
                ));
                in_code_block = false;
                code_lang.clear();
                code_content.clear();
                continue;
            } else {
                // Start code block
                in_code_block = true;
                code_lang = line[3..].trim().to_string();
                continue;
            }
        }

        if in_code_block {
            code_content.push_str(line);
            code_content.push('\n');
            continue;
        }

        // Headings
        if let Some(rest) = line.strip_prefix("# ") {
            out.push_str(&format!("= {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            out.push_str(&format!("== {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("### ") {
            out.push_str(&format!("=== {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("#### ") {
            out.push_str(&format!("==== {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("##### ") {
            out.push_str(&format!("===== {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("###### ") {
            out.push_str(&format!("====== {}\n", md_to_typst_inline(rest)));
            continue;
        }

        // Horizontal rule
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push_str("#line(length: 100%, stroke: 0.5pt + luma(200))\n");
            continue;
        }

        // Unordered list items
        if let Some(rest) = line.strip_prefix("- ") {
            out.push_str(&format!("- {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("* ") {
            out.push_str(&format!("- {}\n", md_to_typst_inline(rest)));
            continue;
        }
        // Nested list items (2-space or 4-space indent)
        if let Some(rest) = line.strip_prefix("  - ") {
            out.push_str(&format!("  - {}\n", md_to_typst_inline(rest)));
            continue;
        }
        if let Some(rest) = line.strip_prefix("    - ") {
            out.push_str(&format!("    - {}\n", md_to_typst_inline(rest)));
            continue;
        }

        // Ordered list items
        if let Some((num_str, rest)) = split_ordered_list(line) {
            out.push_str(&format!("{}. {}\n", num_str, md_to_typst_inline(rest)));
            continue;
        }

        // Block quotes
        if let Some(rest) = line.strip_prefix("> ") {
            out.push_str(&format!("#quote[{}]\n", md_to_typst_inline(rest)));
            continue;
        }
        if trimmed == ">" {
            // Empty blockquote line
            continue;
        }

        // GFM table rows — detect by pipe at start
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            // Separator row (|---|---|)
            if trimmed.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ') {
                continue; // Skip separator
            }
            // Not handling full table conversion in md_to_typst — pass through as text
            let cells: Vec<&str> = trimmed.trim_matches('|').split('|').map(|c| c.trim()).collect();
            let cell_strs: Vec<String> = cells.iter().map(|c| md_to_typst_inline(c)).collect();
            out.push_str(&cell_strs.join(" | "));
            out.push('\n');
            continue;
        }

        // Empty line
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        // Regular paragraph
        out.push_str(&md_to_typst_inline(line));
        out.push('\n');
    }

    // Close unclosed code block
    if in_code_block {
        out.push_str(&format!(
            "```{}\n{}\n```\n",
            code_lang,
            code_content.trim_end()
        ));
    }

    out
}

/// Convert inline markdown to Typst inline markup.
///
/// Handles: `**bold**` → `*bold*`, `*italic*` → `_italic_`,
/// `` `code` `` → `` `code` ``, `[text](url)` → `#link("url")[text]`,
/// `![alt](src)` → `#image("src")`.
pub fn md_to_typst_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Escape backslash sequences
        if chars[i] == '\\' && i + 1 < len {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }

        // Image: ![alt](src)
        if chars[i] == '!' && i + 1 < len && chars[i + 1] == '[' {
            if let Some((alt, src, end)) = parse_link(&chars, i + 1) {
                let _ = alt; // alt not visible in Typst inline
                out.push_str(&format!("#image(\"{}\")", escape_typst(&src)));
                i = end;
                continue;
            }
        }

        // Link: [text](url)
        if chars[i] == '[' {
            if let Some((text_content, href, end)) = parse_link(&chars, i) {
                out.push_str(&format!(
                    "#link(\"{}\")[{}]",
                    escape_typst(&href),
                    escape_typst(&text_content)
                ));
                i = end;
                continue;
            }
        }

        // Inline code: `code`
        if chars[i] == '`' {
            if let Some((code, end)) = parse_backtick_code(&chars, i) {
                out.push('`');
                out.push_str(&code);
                out.push('`');
                i = end;
                continue;
            }
        }

        // Bold: **text** or __text__
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some((content, end)) = parse_delimited(&chars, i, "**") {
                out.push('*');
                out.push_str(&md_to_typst_inline(&content));
                out.push('*');
                i = end;
                continue;
            }
        }
        if i + 1 < len && chars[i] == '_' && chars[i + 1] == '_' {
            if let Some((content, end)) = parse_delimited(&chars, i, "__") {
                out.push('*');
                out.push_str(&md_to_typst_inline(&content));
                out.push('*');
                i = end;
                continue;
            }
        }

        // Italic: *text* or _text_
        if chars[i] == '*' && (i + 1 < len && chars[i + 1] != '*') {
            if let Some((content, end)) = parse_delimited(&chars, i, "*") {
                out.push('_');
                out.push_str(&md_to_typst_inline(&content));
                out.push('_');
                i = end;
                continue;
            }
        }
        if chars[i] == '_' && (i + 1 < len && chars[i + 1] != '_') {
            // Only treat as italic if not in middle of word
            let prev_space = i == 0 || !chars[i - 1].is_alphanumeric();
            if prev_space {
                if let Some((content, end)) = parse_delimited(&chars, i, "_") {
                    out.push('_');
                    out.push_str(&md_to_typst_inline(&content));
                    out.push('_');
                    i = end;
                    continue;
                }
            }
        }

        // Strikethrough: ~~text~~
        if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
            if let Some((content, end)) = parse_delimited(&chars, i, "~~") {
                out.push_str(&format!("#strike[{}]", md_to_typst_inline(&content)));
                i = end;
                continue;
            }
        }

        // Escape Typst special characters that aren't part of markup
        match chars[i] {
            '#' => out.push_str("\\#"),
            '@' => out.push_str("\\@"),
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            '$' => out.push_str("\\$"),
            c => out.push(c),
        }

        i += 1;
    }

    out
}

/// Parse a markdown link starting at `[` position.
/// Returns (text, href, end_index) where end_index is past the closing `)`.
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if start >= chars.len() || chars[start] != '[' {
        return None;
    }

    // Find closing ]
    let mut depth = 0;
    let mut i = start;
    let mut text_end = None;
    while i < chars.len() {
        match chars[i] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    text_end = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let text_end = text_end?;
    let text: String = chars[start + 1..text_end].iter().collect();

    // Expect ( immediately after ]
    if text_end + 1 >= chars.len() || chars[text_end + 1] != '(' {
        return None;
    }

    // Find closing )
    let href_start = text_end + 2;
    let mut paren_depth = 1;
    let mut j = href_start;
    while j < chars.len() {
        match chars[j] {
            '(' => paren_depth += 1,
            ')' => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    let href: String = chars[href_start..j].iter().collect();
                    return Some((text, href, j + 1));
                }
            }
            _ => {}
        }
        j += 1;
    }

    None
}

/// Parse backtick-delimited inline code.
fn parse_backtick_code(chars: &[char], start: usize) -> Option<(String, usize)> {
    if start >= chars.len() || chars[start] != '`' {
        return None;
    }

    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '`' {
            let code: String = chars[start + 1..i].iter().collect();
            return Some((code, i + 1));
        }
        i += 1;
    }

    None
}

/// Parse content delimited by a marker (e.g., `**`, `*`, `~~`).
fn parse_delimited(chars: &[char], start: usize, delim: &str) -> Option<(String, usize)> {
    let delim_chars: Vec<char> = delim.chars().collect();
    let dlen = delim_chars.len();

    if start + dlen > chars.len() {
        return None;
    }

    // Verify opening delimiter
    for (k, dc) in delim_chars.iter().enumerate() {
        if chars[start + k] != *dc {
            return None;
        }
    }

    let content_start = start + dlen;
    let mut i = content_start;

    while i + dlen <= chars.len() {
        // Check for closing delimiter
        let mut found = true;
        for (k, dc) in delim_chars.iter().enumerate() {
            if chars[i + k] != *dc {
                found = false;
                break;
            }
        }
        if found && i > content_start {
            let content: String = chars[content_start..i].iter().collect();
            return Some((content, i + dlen));
        }
        i += 1;
    }

    None
}

/// Try to parse a line as an ordered list item. Returns (number, rest).
fn split_ordered_list(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Skip leading whitespace
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    let num_start = i;
    // Collect digits
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == num_start || i >= bytes.len() {
        return None;
    }
    // Expect `. ` after digits
    if bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
        let num = &line[num_start..i];
        let rest = &line[i + 2..];
        Some((num, rest))
    } else {
        None
    }
}

// --- Helpers ---

/// Escape Typst special characters in a string.
fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '#' => out.push_str("\\#"),
            '@' => out.push_str("\\@"),
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            '$' => out.push_str("\\$"),
            '\\' => out.push_str("\\\\"),
            '"' if false => out.push_str("\\\""), // Only inside string literals
            _ => out.push(c),
        }
    }
    out
}

fn callout_type_str(ct: CalloutType) -> &'static str {
    match ct {
        CalloutType::Info => "info",
        CalloutType::Warning => "warning",
        CalloutType::Danger => "danger",
        CalloutType::Tip => "tip",
        CalloutType::Note => "note",
        CalloutType::Success => "success",
    }
}

fn decision_status_str(ds: DecisionStatus) -> &'static str {
    match ds {
        DecisionStatus::Proposed => "proposed",
        DecisionStatus::Accepted => "accepted",
        DecisionStatus::Rejected => "rejected",
        DecisionStatus::Superseded => "superseded",
    }
}

fn trend_str(t: Trend) -> &'static str {
    match t {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Flat => "flat",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_typst_special_chars() {
        assert_eq!(escape_typst("hello #world"), "hello \\#world");
        assert_eq!(escape_typst("a@b"), "a\\@b");
        assert_eq!(escape_typst("x < y > z"), "x \\< y \\> z");
        assert_eq!(escape_typst("cost: $10"), "cost: \\$10");
    }

    #[test]
    fn md_to_typst_inline_bold() {
        assert_eq!(md_to_typst_inline("**hello**"), "*hello*");
        assert_eq!(md_to_typst_inline("a **bold** word"), "a *bold* word");
    }

    #[test]
    fn md_to_typst_inline_italic() {
        assert_eq!(md_to_typst_inline("*hello*"), "_hello_");
        assert_eq!(md_to_typst_inline("an *italic* word"), "an _italic_ word");
    }

    #[test]
    fn md_to_typst_inline_code() {
        assert_eq!(md_to_typst_inline("`code`"), "`code`");
    }

    #[test]
    fn md_to_typst_inline_link() {
        assert_eq!(
            md_to_typst_inline("[click](https://example.com)"),
            "#link(\"https://example.com\")[click]"
        );
    }

    #[test]
    fn md_to_typst_inline_strikethrough() {
        assert_eq!(md_to_typst_inline("~~deleted~~"), "#strike[deleted]");
    }

    #[test]
    fn md_to_typst_headings() {
        assert!(md_to_typst("# Title\n").contains("= Title"));
        assert!(md_to_typst("## Subtitle\n").contains("== Subtitle"));
        assert!(md_to_typst("### H3\n").contains("=== H3"));
    }

    #[test]
    fn md_to_typst_lists() {
        let md = "- item 1\n- item 2\n";
        let result = md_to_typst(md);
        assert!(result.contains("- item 1"));
        assert!(result.contains("- item 2"));
    }

    #[test]
    fn md_to_typst_ordered_lists() {
        let md = "1. first\n2. second\n";
        let result = md_to_typst(md);
        assert!(result.contains("1. first"));
        assert!(result.contains("2. second"));
    }

    #[test]
    fn md_to_typst_code_block() {
        let md = "```rust\nfn main() {}\n```\n";
        let result = md_to_typst(md);
        assert!(result.contains("```rust"));
        assert!(result.contains("fn main() {}"));
    }

    #[test]
    fn md_to_typst_horizontal_rule() {
        assert!(md_to_typst("---\n").contains("#line("));
    }

    #[test]
    fn render_markdown_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Markdown {
                content: "Hello **world**".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("Hello *world*"));
    }

    #[test]
    fn render_callout_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Callout {
                callout_type: CalloutType::Warning,
                title: Some("Heads up".to_string()),
                content: "Be careful".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("surfdoc-callout"));
        assert!(result.contains("warning"));
        assert!(result.contains("Heads up"));
        assert!(result.contains("Be careful"));
    }

    #[test]
    fn render_data_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Data {
                id: None,
                format: DataFormat::Table,
                sortable: false,
                headers: vec!["Name".into(), "Value".into()],
                rows: vec![vec!["A".into(), "1".into()], vec!["B".into(), "2".into()]],
                raw_content: String::new(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("#table("));
        assert!(result.contains("[*Name*]"));
        assert!(result.contains("[*Value*]"));
        assert!(result.contains("[A]"));
    }

    #[test]
    fn render_code_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Code {
                lang: Some("rust".to_string()),
                file: None,
                highlight: vec![],
                content: "fn main() {}".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("raw(block: true"));
        assert!(result.contains("rust"));
    }

    #[test]
    fn render_tasks_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Tasks {
                items: vec![
                    TaskItem { done: true, text: "Done task".to_string(), assignee: None },
                    TaskItem { done: false, text: "Open task".to_string(), assignee: Some("brady".to_string()) },
                ],
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("☑"));
        assert!(result.contains("☐"));
        assert!(result.contains("Done task"));
        assert!(result.contains("\\@brady"));
    }

    #[test]
    fn render_decision_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Decision {
                status: DecisionStatus::Accepted,
                date: Some("2026-02-22".to_string()),
                deciders: vec!["Brady".to_string()],
                content: "We chose Typst".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("surfdoc-decision-badge"));
        assert!(result.contains("accepted"));
        assert!(result.contains("2026-02-22"));
    }

    #[test]
    fn render_metric_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Metric {
                label: "Revenue".to_string(),
                value: "$12K".to_string(),
                trend: Some(Trend::Up),
                unit: Some("MRR".to_string()),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("surfdoc-metric"));
        assert!(result.contains("Revenue"));
        assert!(result.contains("\\$12K"));
    }

    #[test]
    fn render_summary_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Summary {
                content: "Key points here".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("*Summary*"));
        assert!(result.contains("Key points here"));
    }

    #[test]
    fn render_quote_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Quote {
                content: "The best code is no code.".to_string(),
                attribution: Some("Someone".to_string()),
                cite: None,
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("#quote(attribution: [Someone])"));
        assert!(result.contains("The best code is no code."));
    }

    #[test]
    fn render_divider_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Divider {
                label: None,
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("#line(length: 100%"));
    }

    #[test]
    fn render_front_matter() {
        let doc = SurfDoc {
            front_matter: Some(FrontMatter {
                title: Some("My Document".to_string()),
                author: Some("Brady".to_string()),
                created: Some("2026-02-22".to_string()),
                tags: Some(vec!["strategy".to_string(), "pdf".to_string()]),
                ..FrontMatter::default()
            }),
            blocks: vec![],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("My Document"));
        assert!(result.contains("Brady"));
        assert!(result.contains("2026-02-22"));
        assert!(result.contains("strategy"));
    }

    #[test]
    fn render_hero_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Hero {
                headline: Some("Welcome".to_string()),
                subtitle: Some("To the future".to_string()),
                badge: None,
                align: "center".to_string(),
                image: None,
                buttons: vec![HeroButton {
                    label: "Get Started".to_string(),
                    href: "/start".to_string(),
                    primary: true,
                }],
                content: String::new(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("Welcome"));
        assert!(result.contains("To the future"));
        assert!(result.contains("Get Started"));
    }

    #[test]
    fn render_steps_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Steps {
                steps: vec![
                    StepItem { title: "Step 1".into(), time: None, body: "Do this".into() },
                    StepItem { title: "Step 2".into(), time: Some("5 min".into()), body: "Then this".into() },
                ],
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("*Step 1*"));
        assert!(result.contains("*Step 2*"));
        assert!(result.contains("5 min"));
    }

    #[test]
    fn render_comparison_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Comparison {
                headers: vec!["Feature".into(), "Us".into(), "Them".into()],
                rows: vec![vec!["Speed".into(), "yes".into(), "no".into()]],
                highlight: Some("Us".into()),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("#table("));
        assert!(result.contains("surfdoc-blue"));
        assert!(result.contains("surfdoc-green")); // yes marker
    }

    #[test]
    fn render_pipeline_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Pipeline {
                steps: vec![
                    PipelineStep { label: "Parse".into(), description: None },
                    PipelineStep { label: "Compile".into(), description: Some("via Typst".into()) },
                    PipelineStep { label: "Output".into(), description: None },
                ],
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("*Parse*"));
        assert!(result.contains("→"));
        assert!(result.contains("*Compile*"));
        assert!(result.contains("via Typst"));
        assert!(result.contains("*Output*"));
    }

    #[test]
    fn render_unknown_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Unknown {
                name: "widget".to_string(),
                attrs: Default::default(),
                content: "some content".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("Unknown block: widget"));
        assert!(result.contains("some content"));
    }

    #[test]
    fn render_before_after_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::BeforeAfter {
                before_items: vec![BeforeAfterItem { label: "Old".into(), detail: "Slow".into() }],
                after_items: vec![BeforeAfterItem { label: "New".into(), detail: "Fast".into() }],
                transition: None,
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("*Before:*"));
        assert!(result.contains("*After:*"));
        assert!(result.contains("*Old*"));
        assert!(result.contains("*New*"));
    }

    #[test]
    fn render_product_card_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::ProductCard {
                title: "WaveSite".to_string(),
                subtitle: Some("Build your site".into()),
                badge: Some("NEW".into()),
                badge_color: None,
                body: "Description here".into(),
                features: vec!["Fast".into(), "Free".into()],
                cta_label: Some("Try it".into()),
                cta_href: Some("https://wave.site".into()),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("WaveSite"));
        assert!(result.contains("NEW"));
        assert!(result.contains("Build your site"));
        assert!(result.contains("Fast"));
        assert!(result.contains("Try it"));
    }

    #[test]
    fn render_faq_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Faq {
                items: vec![FaqItem {
                    question: "What is SurfDoc?".into(),
                    answer: "A typed document format.".into(),
                }],
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("*Q: What is SurfDoc?*"));
        assert!(result.contains("A typed document format."));
    }

    #[test]
    fn render_stats_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Stats {
                items: vec![
                    StatItem { value: "373".into(), label: "Tests".into(), color: None },
                    StatItem { value: "37".into(), label: "Block Types".into(), color: None },
                ],
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("#grid("));
        assert!(result.contains("373"));
        assert!(result.contains("Tests"));
    }

    #[test]
    fn render_cta_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Cta {
                label: "Sign Up".into(),
                href: "/signup".into(),
                primary: true,
                icon: None,
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("Sign Up"));
        assert!(result.contains("/signup"));
        assert!(result.contains("surfdoc-blue"));
    }

    #[test]
    fn render_nav_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Nav {
                items: vec![
                    NavItem { label: "Home".into(), href: "/".into(), icon: None },
                    NavItem { label: "About".into(), href: "/about".into(), icon: None },
                ],
                logo: None,
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(result.contains("Home"));
        assert!(result.contains("About"));
    }

    #[test]
    fn site_and_style_blocks_skipped() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![
                Block::Site {
                    domain: Some("example.com".into()),
                    properties: vec![],
                    span: Span::SYNTHETIC,
                },
                Block::Style {
                    properties: vec![StyleProperty { key: "accent".into(), value: "#f00".into() }],
                    span: Span::SYNTHETIC,
                },
                Block::Markdown {
                    content: "visible".to_string(),
                    span: Span::SYNTHETIC,
                },
            ],
            source: String::new(),
        };
        let result = to_typst(&doc);
        assert!(!result.contains("example.com"));
        assert!(result.contains("visible"));
    }
}

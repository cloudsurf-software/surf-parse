//! ANSI terminal renderer.
//!
//! Produces colored terminal output using the `colored` crate. Each block type
//! gets a distinctive visual treatment suitable for CLI display.

use colored::Colorize;

use crate::types::{Block, CalloutType, DecisionStatus, SurfDoc, Trend};

/// Render a `SurfDoc` as ANSI-colored terminal text.
pub fn to_terminal(doc: &SurfDoc) -> String {
    let mut parts: Vec<String> = Vec::new();

    for block in &doc.blocks {
        parts.push(render_block(block));
    }

    parts.join("\n\n")
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Markdown { content, .. } => content.clone(),

        Block::Callout {
            callout_type,
            title,
            content,
            ..
        } => {
            let (border_color, type_label) = callout_style(*callout_type);
            let border = apply_color("\u{2502}", border_color); // │
            let label = format!("{}", type_label.bold());
            let title_part = match title {
                Some(t) => format!(": {t}"),
                None => String::new(),
            };
            let mut lines = vec![format!("{border} {label}{title_part}")];
            for line in content.lines() {
                lines.push(format!("{border} {line}"));
            }
            lines.join("\n")
        }

        Block::Data {
            headers, rows, ..
        } => {
            if headers.is_empty() {
                return String::new();
            }

            // Calculate column widths
            let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
            for row in rows {
                for (i, cell) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(cell.len());
                    }
                }
            }

            let separator: String = widths
                .iter()
                .map(|&w| "\u{2500}".repeat(w + 2)) // ─
                .collect::<Vec<_>>()
                .join("\u{253C}"); // ┼

            // Header row (bold)
            let header_cells: Vec<String> = headers
                .iter()
                .enumerate()
                .map(|(i, h)| format!(" {:width$} ", h, width = widths[i]))
                .collect();
            let header_line = format!(
                "\u{2502}{}\u{2502}",
                header_cells.join("\u{2502}")
            );

            let mut lines = vec![
                format!("{}", header_line.bold()),
                format!("\u{2502}{separator}\u{2502}"),
            ];

            for row in rows {
                let cells: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let w = widths.get(i).copied().unwrap_or(c.len());
                        format!(" {:width$} ", c, width = w)
                    })
                    .collect();
                lines.push(format!(
                    "\u{2502}{}\u{2502}",
                    cells.join("\u{2502}")
                ));
            }
            lines.join("\n")
        }

        Block::Code {
            lang, content, ..
        } => {
            let lang_label = match lang {
                Some(l) => format!(" {}", l.dimmed()),
                None => String::new(),
            };
            let border = format!("{}", "\u{2500}\u{2500}\u{2500}".dimmed()); // ───
            let mut lines = vec![format!("{border}{lang_label}")];
            for line in content.lines() {
                lines.push(format!("  {line}"));
            }
            lines.push(border.clone());
            lines.join("\n")
        }

        Block::Tasks { items, .. } => {
            let lines: Vec<String> = items
                .iter()
                .map(|item| {
                    if item.done {
                        let check = format!("{}", "\u{2713}".green()); // ✓
                        let text = format!("{}", item.text.strikethrough().green());
                        let assignee = match &item.assignee {
                            Some(a) => format!(" {}", format!("@{a}").dimmed()),
                            None => String::new(),
                        };
                        format!("{check} {text}{assignee}")
                    } else {
                        let check = "\u{2610}"; // ☐
                        let assignee = match &item.assignee {
                            Some(a) => format!(" {}", format!("@{a}").dimmed()),
                            None => String::new(),
                        };
                        format!("{check} {}{assignee}", item.text)
                    }
                })
                .collect();
            lines.join("\n")
        }

        Block::Decision {
            status,
            date,
            content,
            ..
        } => {
            let badge = decision_badge(*status);
            let label = format!("{}", "Decision".bold());
            let date_part = match date {
                Some(d) => format!(" ({d})"),
                None => String::new(),
            };
            format!("{badge} {label}{date_part}\n{content}")
        }

        Block::Metric {
            label,
            value,
            trend,
            unit,
            ..
        } => {
            let label_str = format!("{}", label.bold());
            let value_str = format!("{}", value.bold());
            let unit_part = match unit {
                Some(u) => format!(" {u}"),
                None => String::new(),
            };
            let trend_part = match trend {
                Some(Trend::Up) => format!(" {}", "\u{2191}".green()),
                Some(Trend::Down) => format!(" {}", "\u{2193}".red()),
                Some(Trend::Flat) => format!(" {}", "\u{2192}".dimmed()),
                None => String::new(),
            };
            format!("{label_str}: {value_str}{unit_part}{trend_part}")
        }

        Block::Summary { content, .. } => {
            let border = format!("{}", "\u{2502}".cyan()); // │
            let lines: Vec<String> = content
                .lines()
                .map(|l| format!("{border} {}", l.italic()))
                .collect();
            lines.join("\n")
        }

        Block::Figure {
            src, caption, ..
        } => {
            let cap = caption.as_deref().unwrap_or("Image");
            format!("{}", format!("[Figure: {cap}] ({src})").dimmed())
        }

        Block::Tabs { tabs, .. } => {
            let mut parts = Vec::new();
            for (i, tab) in tabs.iter().enumerate() {
                let label = format!("{}", format!("[Tab {}] {}", i + 1, tab.label).bold());
                parts.push(format!("{label}\n{}", tab.content));
            }
            parts.join("\n\n")
        }

        Block::Columns { columns, .. } => {
            let parts: Vec<String> = columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let label = format!("{}", format!("[Col {}]", i + 1).dimmed());
                    format!("{label}\n{}", col.content)
                })
                .collect();
            parts.join("\n\n")
        }

        Block::Quote {
            content,
            attribution,
            ..
        } => {
            let border = format!("{}", "\u{2502}".dimmed()); // │
            let mut lines: Vec<String> = content
                .lines()
                .map(|l| format!("{border} {}", l.italic()))
                .collect();
            if let Some(attr) = attribution {
                lines.push(format!("{border} {}", format!("\u{2014} {attr}").dimmed()));
            }
            lines.join("\n")
        }

        Block::Cta {
            label, href, primary, ..
        } => {
            let badge = if *primary {
                format!("{}", "[CTA]".blue().bold())
            } else {
                format!("{}", "[CTA]".dimmed())
            };
            format!("{badge} {} ({href})", label.bold())
        }

        Block::HeroImage { src, alt, .. } => {
            let desc = alt.as_deref().unwrap_or("Hero image");
            format!("{}", format!("[Hero: {desc}] ({src})").dimmed())
        }

        Block::Testimonial {
            content,
            author,
            role,
            company,
            ..
        } => {
            let border = format!("{}", "\u{2502}".dimmed()); // │
            let mut lines: Vec<String> = content
                .lines()
                .map(|l| format!("{border} {}", l.italic()))
                .collect();
            let details: Vec<&str> = [author.as_deref(), role.as_deref(), company.as_deref()]
                .iter()
                .filter_map(|v| *v)
                .collect();
            if !details.is_empty() {
                lines.push(format!("{border} {}", format!("\u{2014} {}", details.join(", ")).dimmed()));
            }
            lines.join("\n")
        }

        Block::Style { properties, .. } => {
            if properties.is_empty() {
                format!("{}", "[Style: empty]".dimmed())
            } else {
                let pairs: Vec<String> = properties
                    .iter()
                    .map(|p| format!("  {}: {}", p.key.bold(), p.value))
                    .collect();
                format!("{}\n{}", "[Style]".dimmed(), pairs.join("\n"))
            }
        }

        Block::Faq { items, .. } => {
            let mut parts = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let q = format!("{}", format!("Q{}: {}", i + 1, item.question).bold());
                parts.push(format!("{q}\n  {}", item.answer));
            }
            parts.join("\n\n")
        }

        Block::PricingTable {
            headers, rows, ..
        } => {
            // Reuse the same table rendering as Data blocks
            if headers.is_empty() {
                return String::new();
            }

            let label = format!("{}", "[Pricing]".bold().cyan());
            let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
            for row in rows {
                for (i, cell) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(cell.len());
                    }
                }
            }

            let separator: String = widths
                .iter()
                .map(|&w| "\u{2500}".repeat(w + 2))
                .collect::<Vec<_>>()
                .join("\u{253C}");

            let header_cells: Vec<String> = headers
                .iter()
                .enumerate()
                .map(|(i, h)| format!(" {:width$} ", h, width = widths[i]))
                .collect();
            let header_line = format!(
                "\u{2502}{}\u{2502}",
                header_cells.join("\u{2502}")
            );

            let mut lines = vec![
                label,
                format!("{}", header_line.bold()),
                format!("\u{2502}{separator}\u{2502}"),
            ];

            for row in rows {
                let cells: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let w = widths.get(i).copied().unwrap_or(c.len());
                        format!(" {:width$} ", c, width = w)
                    })
                    .collect();
                lines.push(format!(
                    "\u{2502}{}\u{2502}",
                    cells.join("\u{2502}")
                ));
            }
            lines.join("\n")
        }

        Block::Site { domain, properties, .. } => {
            let label = format!("{}", "[Site Config]".bold().cyan());
            let mut lines = vec![label];
            if let Some(d) = domain {
                lines.push(format!("  {}: {}", "domain".bold(), d));
            }
            for p in properties {
                lines.push(format!("  {}: {}", p.key.bold(), p.value));
            }
            lines.join("\n")
        }

        Block::Page {
            route,
            layout,
            children,
            content,
            ..
        } => {
            let layout_part = match layout {
                Some(l) => format!(" layout={l}"),
                None => String::new(),
            };
            let label = format!("{}", format!("[Page {route}{layout_part}]").bold().cyan());
            if children.is_empty() {
                if content.is_empty() {
                    label
                } else {
                    format!("{label}\n{content}")
                }
            } else {
                let child_output: Vec<String> = children.iter().map(render_block).collect();
                format!("{label}\n{}", child_output.join("\n\n"))
            }
        }

        Block::Nav { items, logo, .. } => {
            let header = if let Some(l) = logo {
                format!("{} ", l.bold())
            } else {
                String::new()
            };
            let links: Vec<String> = items
                .iter()
                .map(|item| format!("{} ({})", item.label.blue(), item.href.dimmed()))
                .collect();
            format!("{header}{}", links.join(" | "))
        }

        Block::Unknown {
            name, content, ..
        } => {
            let label = format!("{}", format!("[{name}]").dimmed());
            if content.is_empty() {
                label
            } else {
                format!("{label}\n{content}")
            }
        }

        Block::Embed { src, title, .. } => {
            let label = title.as_deref().unwrap_or("Embed");
            format!("{} {}", format!("[{label}]").cyan(), src.dimmed())
        }

        Block::Form { fields, submit_label, .. } => {
            let mut lines = vec![format!("{}", "Form".bold())];
            for field in fields {
                let req = if field.required { " *".red().to_string() } else { String::new() };
                lines.push(format!("  {} {}{}", "•".dimmed(), field.label, req));
            }
            if let Some(label) = submit_label {
                lines.push(format!("  {}", format!("[{label}]").cyan()));
            }
            lines.join("\n")
        }

        Block::Gallery { items, .. } => {
            let count = items.len();
            format!("{} ({count} images)", "Gallery".bold())
        }

        Block::Footer { sections, copyright, .. } => {
            let mut lines = vec!["─".repeat(40).dimmed().to_string()];
            for section in sections {
                lines.push(format!("{}", section.heading.bold()));
                for link in &section.links {
                    lines.push(format!("  {}", link.label.dimmed()));
                }
            }
            if let Some(cr) = copyright {
                lines.push(cr.dimmed().to_string());
            }
            lines.join("\n")
        }
    }
}

fn callout_style(ct: CalloutType) -> (&'static str, &'static str) {
    match ct {
        CalloutType::Warning => ("yellow", "Warning"),
        CalloutType::Danger => ("red", "Danger"),
        CalloutType::Info => ("blue", "Info"),
        CalloutType::Tip => ("green", "Tip"),
        CalloutType::Note => ("cyan", "Note"),
        CalloutType::Success => ("green", "Success"),
    }
}

fn apply_color(text: &str, color: &str) -> String {
    match color {
        "yellow" => format!("{}", text.yellow()),
        "red" => format!("{}", text.red()),
        "blue" => format!("{}", text.blue()),
        "green" => format!("{}", text.green()),
        "cyan" => format!("{}", text.cyan()),
        _ => text.to_string(),
    }
}

fn decision_badge(status: DecisionStatus) -> String {
    match status {
        DecisionStatus::Accepted => format!("{}", "[ACCEPTED]".green()),
        DecisionStatus::Rejected => format!("{}", "[REJECTED]".red()),
        DecisionStatus::Proposed => format!("{}", "[PROPOSED]".yellow()),
        DecisionStatus::Superseded => format!("{}", "[SUPERSEDED]".dimmed()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn span() -> Span {
        Span {
            start_line: 1,
            end_line: 1,
            start_offset: 0,
            end_offset: 0,
        }
    }

    fn doc_with(blocks: Vec<Block>) -> SurfDoc {
        SurfDoc {
            front_matter: None,
            blocks,
            source: String::new(),
        }
    }

    #[test]
    fn term_callout_has_color() {
        // Force colors on — the colored crate disables them when stdout is not a tty.
        colored::control::set_override(true);

        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Warning,
            title: None,
            content: "Watch out!".into(),
            span: span(),
        }]);
        let output = to_terminal(&doc);
        // ANSI escape codes start with \x1b[
        assert!(
            output.contains("\x1b["),
            "Terminal output should contain ANSI escape codes, got: {output:?}"
        );
        assert!(output.contains("Watch out!"));

        colored::control::unset_override();
    }

    #[test]
    fn term_tasks_symbols() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![
                TaskItem {
                    done: true,
                    text: "Done".into(),
                    assignee: None,
                },
                TaskItem {
                    done: false,
                    text: "Pending".into(),
                    assignee: None,
                },
            ],
            span: span(),
        }]);
        let output = to_terminal(&doc);
        assert!(output.contains("\u{2713}"), "Should contain checkmark"); // ✓
        assert!(output.contains("\u{2610}"), "Should contain empty checkbox"); // ☐
    }

    #[test]
    fn term_metric_trend() {
        let doc = doc_with(vec![
            Block::Metric {
                label: "MRR".into(),
                value: "$2K".into(),
                trend: Some(Trend::Up),
                unit: None,
                span: span(),
            },
            Block::Metric {
                label: "Churn".into(),
                value: "5%".into(),
                trend: Some(Trend::Down),
                unit: None,
                span: span(),
            },
        ]);
        let output = to_terminal(&doc);
        assert!(output.contains("\u{2191}"), "Should contain up arrow"); // ↑
        assert!(output.contains("\u{2193}"), "Should contain down arrow"); // ↓
    }
}

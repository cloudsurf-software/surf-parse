//! Markdown degradation renderer.
//!
//! Converts a `SurfDoc` into standard CommonMark with no `::` directive markers.
//! Each block type is degraded to the nearest Markdown equivalent.

use crate::types::{Block, CalloutType, DecisionStatus, SurfDoc, Trend};

/// Render a `SurfDoc` as standard CommonMark markdown.
///
/// The output contains no `::` directive markers. Each SurfDoc block type is
/// degraded to its closest CommonMark equivalent.
pub fn to_markdown(doc: &SurfDoc) -> String {
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
            let type_label = callout_type_label(*callout_type);
            let prefix = match title {
                Some(t) => format!("**{type_label}**: {t}"),
                None => format!("**{type_label}**"),
            };
            let mut lines = vec![format!("> {prefix}")];
            for line in content.lines() {
                lines.push(format!("> {line}"));
            }
            lines.join("\n")
        }

        Block::Data {
            headers, rows, ..
        } => {
            if headers.is_empty() {
                return String::new();
            }
            let mut lines = Vec::new();
            // Header row
            lines.push(format!("| {} |", headers.join(" | ")));
            // Separator
            let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
            lines.push(format!("| {} |", sep.join(" | ")));
            // Data rows
            for row in rows {
                lines.push(format!("| {} |", row.join(" | ")));
            }
            lines.join("\n")
        }

        Block::Code {
            lang, content, ..
        } => {
            let lang_tag = lang.as_deref().unwrap_or("");
            format!("```{lang_tag}\n{content}\n```")
        }

        Block::Tasks { items, .. } => {
            let lines: Vec<String> = items
                .iter()
                .map(|item| {
                    let check = if item.done { "x" } else { " " };
                    match &item.assignee {
                        Some(a) => format!("- [{check}] {} @{a}", item.text),
                        None => format!("- [{check}] {}", item.text),
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
            let status_label = decision_status_label(*status);
            let date_part = match date {
                Some(d) => format!(" ({d})"),
                None => String::new(),
            };
            let mut lines = vec![format!("> **Decision** ({status_label}){date_part}")];
            for line in content.lines() {
                lines.push(format!("> {line}"));
            }
            lines.join("\n")
        }

        Block::Metric {
            label,
            value,
            trend,
            unit,
            ..
        } => {
            let trend_arrow = match trend {
                Some(Trend::Up) => " \u{2191}",
                Some(Trend::Down) => " \u{2193}",
                Some(Trend::Flat) => " \u{2192}",
                None => "",
            };
            let unit_part = match unit {
                Some(u) => format!(" {u}"),
                None => String::new(),
            };
            format!("**{label}**: {value}{unit_part}{trend_arrow}")
        }

        Block::Summary { content, .. } => {
            let lines: Vec<String> = content.lines().map(|l| format!("> *{l}*")).collect();
            lines.join("\n")
        }

        Block::Figure {
            src,
            caption,
            alt,
            ..
        } => {
            let alt_text = alt.as_deref().unwrap_or("");
            let img = format!("![{alt_text}]({src})");
            match caption {
                Some(c) => format!("{img}\n*{c}*"),
                None => img,
            }
        }

        Block::Tabs { tabs, .. } => {
            let parts: Vec<String> = tabs
                .iter()
                .map(|tab| format!("### {}\n\n{}", tab.label, tab.content))
                .collect();
            parts.join("\n\n")
        }

        Block::Columns { columns, .. } => {
            let parts: Vec<String> = columns
                .iter()
                .map(|col| col.content.clone())
                .collect();
            parts.join("\n\n---\n\n")
        }

        Block::Quote {
            content,
            attribution,
            ..
        } => {
            let mut lines: Vec<String> = content.lines().map(|l| format!("> {l}")).collect();
            if let Some(attr) = attribution {
                lines.push(format!(">\n> \u{2014} {attr}"));
            }
            lines.join("\n")
        }

        Block::Cta {
            label, href, ..
        } => {
            // Degrades to a markdown link
            format!("[{label}]({href})")
        }

        Block::HeroImage {
            src, alt, ..
        } => {
            let alt_text = alt.as_deref().unwrap_or("Hero image");
            format!("![{alt_text}]({src})")
        }

        Block::Testimonial {
            content,
            author,
            role,
            company,
            ..
        } => {
            let mut lines: Vec<String> = content.lines().map(|l| format!("> {l}")).collect();
            let details: Vec<&str> = [author.as_deref(), role.as_deref(), company.as_deref()]
                .iter()
                .filter_map(|v| *v)
                .collect();
            if !details.is_empty() {
                lines.push(format!(">\n> \u{2014} {}", details.join(", ")));
            }
            lines.join("\n")
        }

        Block::Style { .. } => {
            // Style blocks are invisible in markdown degradation
            String::new()
        }

        Block::Faq { items, .. } => {
            // Degrades to headings + paragraphs
            let parts: Vec<String> = items
                .iter()
                .map(|item| format!("### {}\n\n{}", item.question, item.answer))
                .collect();
            parts.join("\n\n")
        }

        Block::PricingTable {
            headers, rows, ..
        } => {
            // Degrades to a standard markdown table (same as Data)
            if headers.is_empty() {
                return String::new();
            }
            let mut lines = Vec::new();
            lines.push(format!("| {} |", headers.join(" | ")));
            let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
            lines.push(format!("| {} |", sep.join(" | ")));
            for row in rows {
                lines.push(format!("| {} |", row.join(" | ")));
            }
            lines.join("\n")
        }

        Block::Site { domain, properties, .. } => {
            // Degrades to a YAML-like config block
            let mut lines = vec!["**Site Configuration**".to_string()];
            if let Some(d) = domain {
                lines.push(format!("- domain: {d}"));
            }
            for p in properties {
                lines.push(format!("- {}: {}", p.key, p.value));
            }
            lines.join("\n")
        }

        Block::Page {
            title,
            content,
            ..
        } => {
            // Degrades to a heading + raw content
            if let Some(t) = title {
                format!("## {t}\n\n{content}")
            } else {
                content.clone()
            }
        }

        Block::Nav { items, .. } => {
            // Degrades to a markdown list of links
            items
                .iter()
                .map(|item| format!("- [{}]({})", item.label, item.href))
                .collect::<Vec<_>>()
                .join("\n")
        }

        Block::BeforeAfter {
            before_items,
            after_items,
            transition,
            ..
        } => {
            let mut lines = Vec::new();
            lines.push("**Before**".to_string());
            for item in before_items {
                lines.push(format!("- {} \u{2014} {}", item.label, item.detail));
            }
            lines.push(String::new());
            if let Some(t) = transition {
                lines.push(format!("\u{2193} *{t}* \u{2193}"));
                lines.push(String::new());
            }
            lines.push("**After**".to_string());
            for item in after_items {
                lines.push(format!("- {} \u{2014} {}", item.label, item.detail));
            }
            lines.join("\n")
        }

        Block::Pipeline { steps, .. } => {
            steps
                .iter()
                .map(|s| {
                    if let Some(d) = &s.description {
                        format!("{} ({})", s.label, d)
                    } else {
                        s.label.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" \u{2192} ")
        }

        Block::Section {
            headline,
            subtitle,
            children,
            ..
        } => {
            let mut lines = Vec::new();
            if let Some(h) = headline {
                lines.push(format!("## {h}"));
                lines.push(String::new());
            }
            if let Some(s) = subtitle {
                lines.push(s.clone());
                lines.push(String::new());
            }
            for child in children {
                lines.push(render_block(child));
                lines.push(String::new());
            }
            lines.join("\n").trim().to_string()
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
            let mut lines = Vec::new();
            let badge_str = badge.as_ref().map(|b| format!(" [{b}]")).unwrap_or_default();
            lines.push(format!("### {title}{badge_str}"));
            lines.push(String::new());
            if let Some(s) = subtitle {
                lines.push(format!("*{s}*"));
                lines.push(String::new());
            }
            if !body.is_empty() {
                lines.push(body.clone());
                lines.push(String::new());
            }
            for f in features {
                lines.push(format!("- {f}"));
            }
            if let (Some(label), Some(href)) = (cta_label, cta_href) {
                lines.push(String::new());
                lines.push(format!("[{label}]({href})"));
            }
            lines.join("\n").trim().to_string()
        }

        Block::Unknown {
            name,
            content,
            ..
        } => {
            let mut lines = Vec::new();
            lines.push(format!("<!-- ::{name} -->"));
            if !content.is_empty() {
                lines.push(content.clone());
            }
            lines.push("<!-- :: -->".to_string());
            lines.join("\n")
        }

        Block::Embed { src, title, .. } => {
            let label = title.as_deref().unwrap_or("Embedded content");
            format!("[{label}]({src})")
        }

        Block::Form { fields, submit_label, .. } => {
            let mut lines = Vec::new();
            lines.push("**Form**".to_string());
            for field in fields {
                let req = if field.required { " *" } else { "" };
                lines.push(format!("- {}{}", field.label, req));
            }
            if let Some(label) = submit_label {
                lines.push(format!("\n[{}]", label));
            }
            lines.join("\n")
        }

        Block::Gallery { items, .. } => {
            let mut lines = Vec::new();
            for item in items {
                let alt = item.alt.as_deref().unwrap_or("");
                let cap = item.caption.as_deref().map(|c| format!(" — {c}")).unwrap_or_default();
                lines.push(format!("![{alt}]({}){cap}", item.src));
            }
            lines.join("\n")
        }

        Block::Footer { sections, copyright, social, .. } => {
            let mut lines = Vec::new();
            lines.push("---".to_string());
            for section in sections {
                lines.push(format!("**{}**", section.heading));
                for link in &section.links {
                    if link.href.is_empty() {
                        lines.push(format!("- {}", link.label));
                    } else {
                        lines.push(format!("- [{}]({})", link.label, link.href));
                    }
                }
                lines.push(String::new());
            }
            for link in social {
                lines.push(format!("@{} {}", link.platform, link.href));
            }
            if let Some(cr) = copyright {
                lines.push(cr.clone());
            }
            lines.join("\n")
        }

        Block::Details {
            title, content, ..
        } => {
            let heading = title.as_deref().unwrap_or("Details");
            format!("**{}**\n\n{}", heading, content)
        }

        Block::Divider { label, .. } => match label {
            Some(text) => format!("--- {} ---", text),
            None => "---".to_string(),
        },

        Block::Hero {
            headline,
            subtitle,
            buttons,
            ..
        } => {
            let mut lines = Vec::new();
            if let Some(h) = headline {
                lines.push(format!("# {h}"));
                lines.push(String::new());
            }
            if let Some(s) = subtitle {
                lines.push(s.clone());
                lines.push(String::new());
            }
            for btn in buttons {
                lines.push(format!("[{}]({})", btn.label, btn.href));
            }
            lines.join("\n")
        }

        Block::Features { cards, .. } => {
            let mut lines = Vec::new();
            for card in cards {
                lines.push(format!("### {}", card.title));
                lines.push(String::new());
                if !card.body.is_empty() {
                    lines.push(card.body.clone());
                    lines.push(String::new());
                }
                if let (Some(label), Some(href)) = (&card.link_label, &card.link_href) {
                    lines.push(format!("[{label}]({href})"));
                    lines.push(String::new());
                }
            }
            lines.join("\n").trim().to_string()
        }

        Block::Steps { steps, .. } => {
            let mut lines = Vec::new();
            for (i, step) in steps.iter().enumerate() {
                lines.push(format!("{}. **{}**", i + 1, step.title));
                if !step.body.is_empty() {
                    lines.push(format!("   {}", step.body));
                }
            }
            lines.join("\n")
        }

        Block::Stats { items, .. } => {
            items
                .iter()
                .map(|item| format!("- **{}** {}", item.value, item.label))
                .collect::<Vec<_>>()
                .join("\n")
        }

        Block::Comparison {
            headers, rows, ..
        } => {
            let mut lines = Vec::new();
            lines.push(format!("| {} |", headers.join(" | ")));
            lines.push(format!("| {} |", headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
            for row in rows {
                lines.push(format!("| {} |", row.join(" | ")));
            }
            lines.join("\n")
        }

        Block::Logo { src, alt, .. } => {
            let alt_text = alt.as_deref().unwrap_or("Logo");
            format!("![{alt_text}]({src})")
        }

        Block::Toc { .. } => {
            "*Table of Contents*".to_string()
        }
    }
}

fn callout_type_label(ct: CalloutType) -> &'static str {
    match ct {
        CalloutType::Info => "Info",
        CalloutType::Warning => "Warning",
        CalloutType::Danger => "Danger",
        CalloutType::Tip => "Tip",
        CalloutType::Note => "Note",
        CalloutType::Success => "Success",
    }
}

fn decision_status_label(ds: DecisionStatus) -> &'static str {
    match ds {
        DecisionStatus::Proposed => "proposed",
        DecisionStatus::Accepted => "accepted",
        DecisionStatus::Rejected => "rejected",
        DecisionStatus::Superseded => "superseded",
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
    fn md_callout_warning() {
        let doc = doc_with(vec![Block::Callout {
            callout_type: CalloutType::Warning,
            title: Some("Watch out".into()),
            content: "Sharp edges ahead.".into(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("> **Warning**: Watch out"));
        assert!(md.contains("> Sharp edges ahead."));
    }

    #[test]
    fn md_data_table() {
        let doc = doc_with(vec![Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec!["Name".into(), "Age".into()],
            rows: vec![vec!["Alice".into(), "30".into()]],
            raw_content: String::new(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("| Name | Age |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Alice | 30 |"));
    }

    #[test]
    fn md_code_block() {
        let doc = doc_with(vec![Block::Code {
            lang: Some("rust".into()),
            file: None,
            highlight: vec![],
            content: "fn main() {}".into(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
        assert!(md.contains("```"));
    }

    #[test]
    fn md_tasks() {
        let doc = doc_with(vec![Block::Tasks {
            items: vec![
                TaskItem {
                    done: false,
                    text: "Write tests".into(),
                    assignee: None,
                },
                TaskItem {
                    done: true,
                    text: "Write parser".into(),
                    assignee: Some("brady".into()),
                },
            ],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("- [ ] Write tests"));
        assert!(md.contains("- [x] Write parser @brady"));
    }

    #[test]
    fn md_decision() {
        let doc = doc_with(vec![Block::Decision {
            status: DecisionStatus::Accepted,
            date: Some("2026-02-10".into()),
            deciders: vec![],
            content: "We chose Rust.".into(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("> **Decision** (accepted) (2026-02-10)"));
        assert!(md.contains("> We chose Rust."));
    }

    #[test]
    fn md_metric() {
        let doc = doc_with(vec![Block::Metric {
            label: "MRR".into(),
            value: "$2K".into(),
            trend: Some(Trend::Up),
            unit: Some("USD".into()),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("**MRR**: $2K USD"));
        assert!(md.contains("\u{2191}")); // up arrow
    }

    #[test]
    fn md_summary() {
        let doc = doc_with(vec![Block::Summary {
            content: "Executive overview.".into(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("> *Executive overview.*"));
    }

    #[test]
    fn md_figure() {
        let doc = doc_with(vec![Block::Figure {
            src: "diagram.png".into(),
            caption: Some("Architecture".into()),
            alt: Some("Diagram".into()),
            width: None,
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("![Diagram](diagram.png)"));
        assert!(md.contains("*Architecture*"));
    }

    // -- Web blocks ------------------------------------------------

    #[test]
    fn md_cta() {
        let doc = doc_with(vec![Block::Cta {
            label: "Sign Up".into(),
            href: "/signup".into(),
            primary: true,
            icon: None,
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert_eq!(md, "[Sign Up](/signup)");
    }

    #[test]
    fn md_hero_image() {
        let doc = doc_with(vec![Block::HeroImage {
            src: "hero.png".into(),
            alt: Some("Product shot".into()),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert_eq!(md, "![Product shot](hero.png)");
    }

    #[test]
    fn md_testimonial() {
        let doc = doc_with(vec![Block::Testimonial {
            content: "Great product!".into(),
            author: Some("Jane".into()),
            role: Some("Engineer".into()),
            company: None,
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("> Great product!"));
        assert!(md.contains("\u{2014} Jane, Engineer"));
    }

    #[test]
    fn md_style_invisible() {
        let doc = doc_with(vec![Block::Style {
            properties: vec![crate::types::StyleProperty {
                key: "accent".into(),
                value: "blue".into(),
            }],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.is_empty());
    }

    #[test]
    fn md_faq() {
        let doc = doc_with(vec![Block::Faq {
            items: vec![
                crate::types::FaqItem {
                    question: "Is it free?".into(),
                    answer: "Yes.".into(),
                },
                crate::types::FaqItem {
                    question: "Can I export?".into(),
                    answer: "PDF and HTML.".into(),
                },
            ],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("### Is it free?"));
        assert!(md.contains("Yes."));
        assert!(md.contains("### Can I export?"));
        assert!(md.contains("PDF and HTML."));
    }

    #[test]
    fn md_pricing_table() {
        let doc = doc_with(vec![Block::PricingTable {
            headers: vec!["".into(), "Free".into(), "Pro".into()],
            rows: vec![vec!["Price".into(), "$0".into(), "$9/mo".into()]],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("Free | Pro"));
        assert!(md.contains("| --- | --- | --- |"));
        assert!(md.contains("| Price | $0 | $9/mo |"));
    }

    #[test]
    fn md_site() {
        let doc = doc_with(vec![Block::Site {
            domain: Some("example.com".into()),
            properties: vec![
                crate::types::StyleProperty { key: "name".into(), value: "Test".into() },
            ],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("**Site Configuration**"));
        assert!(md.contains("domain: example.com"));
        assert!(md.contains("name: Test"));
    }

    #[test]
    fn md_page_with_title() {
        let doc = doc_with(vec![Block::Page {
            route: "/".into(),
            layout: None,
            title: Some("Home".into()),
            sidebar: false,
            content: "Welcome to our site.".into(),
            children: vec![],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("## Home"));
        assert!(md.contains("Welcome to our site."));
    }

    #[test]
    fn md_page_no_title() {
        let doc = doc_with(vec![Block::Page {
            route: "/about".into(),
            layout: None,
            title: None,
            sidebar: false,
            content: "# About Us\n\nWe build things.".into(),
            children: vec![],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("# About Us"));
        assert!(md.contains("We build things."));
    }

    #[test]
    fn md_no_surfdoc_markers() {
        let doc = doc_with(vec![
            Block::Callout {
                callout_type: CalloutType::Info,
                title: None,
                content: "Hello".into(),
                span: span(),
            },
            Block::Code {
                lang: Some("rust".into()),
                file: None,
                highlight: vec![],
                content: "let x = 1;".into(),
                span: span(),
            },
            Block::Metric {
                label: "A".into(),
                value: "1".into(),
                trend: None,
                unit: None,
                span: span(),
            },
        ]);
        let md = to_markdown(&doc);
        // Ensure no :: markers exist (they belong to SurfDoc directives, not Markdown)
        assert!(
            !md.contains("::callout"),
            "Output should not contain ::callout markers"
        );
        assert!(
            !md.contains("::code"),
            "Output should not contain ::code markers"
        );
        assert!(
            !md.contains("::metric"),
            "Output should not contain ::metric markers"
        );
    }

    #[test]
    fn md_before_after() {
        let doc = doc_with(vec![Block::BeforeAfter {
            before_items: vec![crate::types::BeforeAfterItem {
                label: "Manual".into(),
                detail: "Hand-written".into(),
            }],
            after_items: vec![crate::types::BeforeAfterItem {
                label: "Auto".into(),
                detail: "Generated".into(),
            }],
            transition: Some("SurfDoc".into()),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("**Before**"));
        assert!(md.contains("**After**"));
        assert!(md.contains("Manual"));
        assert!(md.contains("SurfDoc"));
    }

    #[test]
    fn md_pipeline() {
        let doc = doc_with(vec![Block::Pipeline {
            steps: vec![
                crate::types::PipelineStep { label: "A".into(), description: Some("first".into()) },
                crate::types::PipelineStep { label: "B".into(), description: None },
            ],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("A (first)"));
        assert!(md.contains("\u{2192}"));
        assert!(md.contains("B"));
    }

    #[test]
    fn md_section() {
        let doc = doc_with(vec![Block::Section {
            bg: Some("muted".into()),
            headline: Some("Features".into()),
            subtitle: Some("What we offer".into()),
            content: String::new(),
            children: vec![Block::Markdown {
                content: "Some content".into(),
                span: span(),
            }],
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("## Features"));
        assert!(md.contains("What we offer"));
        assert!(md.contains("Some content"));
    }

    #[test]
    fn md_product_card() {
        let doc = doc_with(vec![Block::ProductCard {
            title: "Surf".into(),
            subtitle: Some("Browser".into()),
            badge: Some("Available".into()),
            badge_color: None,
            body: "A great product.".into(),
            features: vec!["Fast".into(), "Secure".into()],
            cta_label: Some("Get it".into()),
            cta_href: Some("/download".into()),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("### Surf [Available]"));
        assert!(md.contains("*Browser*"));
        assert!(md.contains("A great product."));
        assert!(md.contains("- Fast"));
        assert!(md.contains("[Get it](/download)"));
    }
}

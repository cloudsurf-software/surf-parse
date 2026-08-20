//! Markdown degradation renderer.
//!
//! Converts a `SurfDoc` into standard CommonMark with no `::` directive markers.
//! Each block type is degraded to the nearest Markdown equivalent.

use crate::citation;
use crate::types::{Block, CalloutType, ChartType, DecisionStatus, Format, HttpMethod, ListDisplay, SurfDoc, Trend};

/// Render a `SurfDoc` as standard CommonMark markdown.
///
/// The output contains no `::` directive markers. Each SurfDoc block type is
/// degraded to its closest CommonMark equivalent.
pub fn to_markdown(doc: &SurfDoc) -> String {
    let _cite_scope = citation::install_context(citation::build_context(
        &doc.blocks,
        doc.front_matter.as_ref().and_then(|fm| fm.format),
    ));
    let mut parts: Vec<String> = Vec::new();

    for block in &doc.blocks {
        parts.push(render_block(block));
    }

    parts.join("\n\n")
}

/// Render a `::bibliography` as a markdown reference list (heading + entries).
fn render_bibliography_md(style_override: Option<Format>) -> String {
    citation::with_active(|ctx| {
        let Some(ctx) = ctx else {
            return String::new();
        };
        if ctx.references.is_empty() {
            return String::new();
        }
        let style = style_override.unwrap_or(ctx.style);
        let refs = if style_override.is_some() {
            ctx.references.clone()
        } else {
            citation::ordered_references(ctx)
        };
        let mut lines = vec![format!("## {}", citation::bibliography_heading(style))];
        if citation::is_numbered(style) {
            for line in citation::reference_list(&refs, style) {
                lines.push(line);
            }
        } else {
            for line in citation::reference_list(&refs, style) {
                lines.push(format!("- {line}"));
            }
        }
        lines.join("\n")
    })
}

pub(crate) fn render_block(block: &Block) -> String {
    match block {
        Block::Markdown { content, .. } => citation::substitute_text_cites(content),

        // `::cite` is a definition only — emits nothing in markdown output.
        Block::Cite { .. } => String::new(),

        Block::Bibliography { style, .. } => render_bibliography_md(*style),

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

        Block::Diagram {
            diagram_type,
            title,
            content,
            ..
        } => {
            // Degrade to a fenced code block carrying the raw DSL; the info
            // string keeps the diagram kind recoverable downstream.
            let info = if diagram_type.is_empty() {
                "diagram".to_string()
            } else {
                format!("diagram-{diagram_type}")
            };
            let fence = format!("```{info}\n{content}\n```");
            match title {
                Some(t) => format!("**{t}**\n\n{fence}"),
                None => fence,
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

        Block::Deck { properties, .. } => {
            // Degrades to a YAML-like config block.
            let mut lines = vec!["**Deck Configuration**".to_string()];
            for p in properties {
                lines.push(format!("- {}: {}", p.key, p.value));
            }
            lines.join("\n")
        }

        Block::Slide {
            kicker, children, ..
        } => {
            // Degrades to an optional kicker line + the slide's child blocks.
            let mut parts: Vec<String> = Vec::new();
            if let Some(k) = kicker {
                parts.push(format!("**{k}**"));
            }
            for child in children {
                parts.push(render_block(child));
            }
            parts.join("\n\n")
        }

        Block::Nav { items, groups, .. } => {
            // Degrades to a markdown list of links, with grouped sections as
            // `## Heading` subheads (mirrors the nav author syntax).
            let mut lines: Vec<String> = items
                .iter()
                .map(|item| format!("- [{}]({})", item.label, item.href))
                .collect();
            for g in groups {
                if let Some(label) = &g.label {
                    lines.push(format!("## {}", label));
                }
                for item in &g.items {
                    lines.push(format!("- [{}]({})", item.label, item.href));
                }
            }
            lines.join("\n")
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

        Block::Banner {
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
            lines.join("\n").trim().to_string()
        }

        Block::ProductGrid { groups, .. } => {
            let mut lines = Vec::new();
            for group in groups {
                if let Some(label) = &group.label {
                    lines.push(format!("### {label}"));
                    lines.push(String::new());
                }
                for item in &group.items {
                    let tagline = item
                        .tagline
                        .as_deref()
                        .map(|t| format!(" — {t}"))
                        .unwrap_or_default();
                    lines.push(format!("- [{}]({}){}", item.name, item.href, tagline));
                }
                lines.push(String::new());
            }
            lines.join("\n").trim().to_string()
        }

        Block::PostGrid { title, subtitle, items, .. } => {
            let mut lines = Vec::new();
            if let Some(t) = title {
                lines.push(format!("### {t}"));
                lines.push(String::new());
            }
            if let Some(s) = subtitle {
                lines.push(s.clone());
                lines.push(String::new());
            }
            for item in items {
                let excerpt = item
                    .excerpt
                    .as_deref()
                    .map(|e| format!(" — {e}"))
                    .unwrap_or_default();
                lines.push(format!("- [{}]({}){}", item.title, item.href, excerpt));
            }
            lines.join("\n").trim().to_string()
        }

        Block::Gate { title, subtitle, .. } => {
            let mut lines = Vec::new();
            if let Some(t) = title {
                lines.push(format!("### {t}"));
                lines.push(String::new());
            }
            if let Some(s) = subtitle {
                lines.push(s.clone());
            }
            lines.join("\n").trim().to_string()
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

        // ----- App description blocks -----

        Block::List {
            source, display, filters, sort, ..
        } => {
            let display_str = match display {
                ListDisplay::Card => "cards",
                ListDisplay::Table => "table",
                ListDisplay::Compact => "compact list",
            };
            let mut lines = vec![format!("**Data List** ({display_str})")];
            lines.push(format!("Source: `{source}`"));
            if !filters.is_empty() {
                let filter_names: Vec<&str> = filters.iter().map(|f| f.field.as_str()).collect();
                lines.push(format!("Filters: {}", filter_names.join(", ")));
            }
            if let Some(s) = sort {
                let dir = if s.descending { "descending" } else { "ascending" };
                lines.push(format!("Sort: {} {dir}", s.field));
            }
            lines.join("\n")
        }

        Block::Board {
            source, columns, ..
        } => {
            let mut lines = vec!["**Board**".to_string()];
            if !columns.is_empty() {
                lines.push(format!("Columns: {}", columns.join(" | ")));
            }
            lines.push(format!("Source: `{source}`"));
            lines.join("\n")
        }

        Block::Action {
            method, target, label, fields, ..
        } => {
            let method_str = match method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
                HttpMethod::Put => "PUT",
                HttpMethod::Patch => "PATCH",
                HttpMethod::Delete => "DELETE",
            };
            let mut lines = vec![format!("**{label}** ({method_str} `{target}`)")];
            for field in fields {
                let req = if field.required { " *" } else { "" };
                lines.push(format!("- {}{}", field.label, req));
            }
            lines.join("\n")
        }

        Block::FilterBar { fields, .. } => {
            let mut lines = vec!["**Filters**".to_string()];
            for field in fields {
                lines.push(format!("- {}: {}", field.label, field.options.join(" | ")));
            }
            lines.join("\n")
        }

        Block::Search { placeholder, .. } => {
            let ph = placeholder.as_deref().unwrap_or("Search...");
            format!("**Search**: {ph}")
        }

        Block::Dashboard { source, refresh, .. } => {
            let refresh_str = refresh.map(|r| format!(" (refresh every {r}s)")).unwrap_or_default();
            format!("**Dashboard**{refresh_str}\nSource: `{source}`")
        }

        Block::ChatInput { modes, placeholder, .. } => {
            let ph = placeholder.as_deref().unwrap_or("Type a message...");
            let mut lines = vec![format!("**Chat**: {ph}")];
            if !modes.is_empty() {
                lines.push(format!("Modes: {}", modes.join(" | ")));
            }
            lines.join("\n")
        }

        Block::Feed { source, stream, .. } => {
            let mode = if *stream { "streaming" } else { "polling" };
            format!("**Feed** ({mode})\nSource: `{source}`")
        }

        Block::Store {
            title, currency, items, ..
        } => {
            let cur = currency.as_deref().unwrap_or("$");
            let mut out = format!("**{}**\n", title.as_deref().unwrap_or("Store"));
            for it in items {
                match &it.blurb {
                    Some(b) => out.push_str(&format!("- {} — {}{} ({})\n", it.name, cur, it.price, b)),
                    None => out.push_str(&format!("- {} — {}{}\n", it.name, cur, it.price)),
                }
            }
            out
        }

        Block::Booking {
            title, services, days, ..
        } => {
            let mut out = format!("**{}**\n", title.as_deref().unwrap_or("Booking"));
            for s in services {
                let meta: Vec<&str> = [s.duration.as_deref(), s.price.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect();
                if meta.is_empty() {
                    out.push_str(&format!("- {}\n", s.name));
                } else {
                    out.push_str(&format!("- {} ({})\n", s.name, meta.join(", ")));
                }
            }
            let open = days.iter().filter(|d| !d.slots.is_empty()).count();
            out.push_str(&format!("\n_{open} day(s) with availability._"));
            out
        }

        Block::Editor { lang, .. } => {
            let lang_str = lang.as_deref().unwrap_or("text");
            format!("**Editor** ({lang_str})")
        }

        Block::Chart { chart_type, source, period, title, data, .. } => {
            let type_str = match chart_type {
                ChartType::Line => "Line",
                ChartType::Bar => "Bar",
                ChartType::Pie => "Pie",
                ChartType::Area => "Area",
                ChartType::Scatter => "Scatter",
                ChartType::Donut => "Donut",
                ChartType::StackedBar => "Stacked Bar",
                ChartType::Radar => "Radar",
            };
            let heading = match title {
                Some(t) => format!("**{type_str} Chart — {t}**"),
                None => format!("**{type_str} Chart**"),
            };
            // Inline data → render a compact markdown table of the values.
            if let Some(d) = data {
                let mut out = heading;
                out.push('\n');
                out.push_str("| | ");
                out.push_str(&d.series.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(" | "));
                out.push_str(" |\n|---|");
                out.push_str(&"---|".repeat(d.series.len()));
                for (i, cat) in d.categories.iter().enumerate() {
                    out.push_str(&format!("\n| {cat} | "));
                    let cells: Vec<String> = d
                        .series
                        .iter()
                        .map(|s| s.values.get(i).map(|v| format!("{v}")).unwrap_or_default())
                        .collect();
                    out.push_str(&cells.join(" | "));
                    out.push_str(" |");
                }
                return out;
            }
            let period_str = period.as_ref().map(|p| format!(" ({p})")).unwrap_or_default();
            format!("{heading}{period_str}\nSource: `{source}`")
        }

        Block::SplitPane { ratio, left, right, .. } => {
            if left.is_empty() && right.is_empty() {
                format!("**Split Pane** ({ratio})")
            } else {
                // Container idiom (degradation "sequential sections"): panes
                // flatten in order, left then right.
                let parts: Vec<String> = left
                    .iter()
                    .chain(right.iter())
                    .map(render_block)
                    .filter(|s| !s.is_empty())
                    .collect();
                parts.join("\n\n")
            }
        }

        // ----- Infrastructure manifest blocks -----

        Block::App { name, children, .. } => {
            let mut lines = vec![format!("## App: {name}")];
            for child in children {
                let rendered = render_block(child);
                if !rendered.is_empty() { lines.push(rendered); }
            }
            lines.join("\n\n")
        }

        Block::Build { base, runtime, edition, properties, .. } => {
            let mut lines = vec!["**Build**".to_string()];
            if let Some(b) = base { lines.push(format!("- base: {b}")); }
            if let Some(r) = runtime { lines.push(format!("- runtime: {r}")); }
            if let Some(e) = edition { lines.push(format!("- edition: {e}")); }
            for p in properties { lines.push(format!("- {}: {}", p.key, p.value)); }
            lines.join("\n")
        }

        Block::InfraDatabase { name, shared_auth, volume_gb, properties, .. } => {
            let mut lines = vec!["**Database**".to_string()];
            if let Some(n) = name { lines.push(format!("- name: {n}")); }
            if *shared_auth { lines.push("- shared_auth: true".to_string()); }
            if let Some(v) = volume_gb { lines.push(format!("- volume: {v} GB")); }
            for p in properties { lines.push(format!("- {}: {}", p.key, p.value)); }
            lines.join("\n")
        }

        Block::Deploy { env, app, machines, memory, auto_stop, min_machines, strategy, properties, .. } => {
            let env_str = env.as_deref().unwrap_or("unknown");
            let mut lines = vec![format!("**Deploy: {env_str}**")];
            if let Some(a) = app { lines.push(format!("- app: {a}")); }
            if let Some(m) = machines { lines.push(format!("- machines: {m}")); }
            if let Some(m) = memory { lines.push(format!("- memory: {m} MB")); }
            if let Some(a) = auto_stop { lines.push(format!("- auto_stop: {a}")); }
            if let Some(m) = min_machines { lines.push(format!("- min_machines: {m}")); }
            if let Some(s) = strategy { lines.push(format!("- strategy: {s}")); }
            for p in properties { lines.push(format!("- {}: {}", p.key, p.value)); }
            lines.join("\n")
        }

        Block::InfraEnv { tier, entries, .. } => {
            let tier_str = tier.as_deref().unwrap_or("env");
            let mut lines = vec![format!("**Env ({tier_str})**"), "```".to_string()];
            for e in entries {
                match &e.default_value {
                    Some(v) => lines.push(format!("{} = {}", e.name, v)),
                    None => lines.push(e.name.clone()),
                }
            }
            lines.push("```".to_string());
            lines.join("\n")
        }

        Block::Health { path, method, grace, interval, timeout, .. } => {
            let mut lines = vec!["**Health Check**".to_string()];
            if let Some(p) = path { lines.push(format!("- path: {p}")); }
            if let Some(m) = method { lines.push(format!("- method: {m}")); }
            if let Some(g) = grace { lines.push(format!("- grace: {g}")); }
            if let Some(i) = interval { lines.push(format!("- interval: {i}")); }
            if let Some(t) = timeout { lines.push(format!("- timeout: {t}")); }
            lines.join("\n")
        }

        Block::Concurrency { concurrency_type, hard_limit, soft_limit, force_https, .. } => {
            let mut lines = vec!["**Concurrency**".to_string()];
            if let Some(t) = concurrency_type { lines.push(format!("- type: {t}")); }
            if let Some(h) = hard_limit { lines.push(format!("- hard_limit: {h}")); }
            if let Some(s) = soft_limit { lines.push(format!("- soft_limit: {s}")); }
            if *force_https { lines.push("- force_https: true".to_string()); }
            lines.join("\n")
        }

        Block::Cicd { provider, properties, .. } => {
            let prov = provider.as_deref().unwrap_or("CI/CD");
            let mut lines = vec![format!("**{prov}**")];
            for p in properties { lines.push(format!("- {}: {}", p.key, p.value)); }
            lines.join("\n")
        }

        Block::Smoke { checks, .. } => {
            let mut lines = vec![
                "| Method | Path | Expected |".to_string(),
                "| --- | --- | --- |".to_string(),
            ];
            for c in checks {
                lines.push(format!("| {} | `{}` | {} |", c.method, c.path, c.expected));
            }
            lines.join("\n")
        }

        Block::Domains { entries, .. } => {
            let mut lines = vec!["**Domains**".to_string()];
            for e in entries {
                match &e.description {
                    Some(d) => lines.push(format!("- {} \u{2014} {}", e.domain, d)),
                    None => lines.push(format!("- {}", e.domain)),
                }
            }
            lines.join("\n")
        }

        Block::Crates { entries, .. } => {
            let mut lines = vec!["**Crates**".to_string()];
            for e in entries {
                let mut detail = Vec::new();
                if let Some(s) = &e.source { detail.push(format!("source: {s}")); }
                if let Some(f) = &e.features { detail.push(format!("features: {f}")); }
                if detail.is_empty() {
                    lines.push(format!("- `{}`", e.name));
                } else {
                    lines.push(format!("- `{}` ({})", e.name, detail.join(", ")));
                }
            }
            lines.join("\n")
        }

        Block::DeployUrls { entries, .. } => {
            let mut lines = vec!["**Deploy URLs**".to_string()];
            for p in entries { lines.push(format!("- {}: {}", p.key, p.value)); }
            lines.join("\n")
        }

        Block::Volumes { entries, .. } => {
            let mut lines = vec!["**Volumes**".to_string()];
            for v in entries { lines.push(format!("- {} \u{2192} {}", v.name, v.mount)); }
            lines.join("\n")
        }

        Block::Model { name, fields, .. } => {
            let mut lines = vec![format!("**Model: {name}**"), String::new()];
            lines.push("| Field | Type | Constraints |".to_string());
            lines.push("|-------|------|-------------|".to_string());
            for f in fields {
                let type_str = model_field_type_md(&f.field_type);
                let constraints: Vec<String> = f.constraints.iter().map(|c| constraint_md(c)).collect();
                lines.push(format!("| {} | {} | {} |", f.name, type_str, constraints.join(", ")));
            }
            lines.join("\n")
        }

        Block::Route { method, path, auth, returns, body, handler, content, .. } => {
            let method_str = http_method_md(*method);
            let mut lines = vec![format!("**{method_str} `{path}`**")];
            if let Some(a) = auth { lines.push(format!("- auth: {a}")); }
            if let Some(r) = returns { lines.push(format!("- returns: `{r}`")); }
            if let Some(b) = body { lines.push(format!("- body: `{b}`")); }
            if let Some(h) = handler {
                lines.push(String::new());
                lines.push("```rust".to_string());
                lines.push(h.clone());
                lines.push("```".to_string());
            }
            if !content.is_empty() { lines.push(content.clone()); }
            lines.join("\n")
        }

        Block::Auth { provider, session, roles, default_role, .. } => {
            let provider_str = auth_provider_md(*provider);
            let mut lines = vec!["**Authentication**".to_string()];
            lines.push(format!("- provider: {provider_str}"));
            if let Some(s) = session { lines.push(format!("- session: {s}")); }
            if !roles.is_empty() { lines.push(format!("- roles: {}", roles.join(", "))); }
            if let Some(dr) = default_role { lines.push(format!("- default role: {dr}")); }
            lines.join("\n")
        }

        Block::Binding { source, target, events, .. } => {
            let mut lines = vec!["**Binding**".to_string()];
            lines.push(format!("- source: `{source}`"));
            lines.push(format!("- target: `{target}`"));
            for e in events { lines.push(format!("- {}: {}", e.event, e.action)); }
            lines.join("\n")
        }

        Block::Schema { name, fields, .. } => {
            let mut lines = vec![format!("**Schema: {name}**"), String::new()];
            lines.push("| Column | Type | Constraints |".to_string());
            lines.push("|---|---|---|".to_string());
            for f in fields {
                let constraints = f.constraints.iter().map(|c| constraint_md(c)).collect::<Vec<_>>().join(", ");
                lines.push(format!("| {} | {} | {} |", f.name, model_field_type_md(&f.field_type), constraints));
            }
            lines.join("\n")
        }

        Block::Use { crates, .. } => {
            let mut lines = vec!["**Dependencies**".to_string()];
            for c in crates {
                let mut parts = vec![format!("`{}`", c.name)];
                if let Some(v) = &c.version { parts.push(v.clone()); }
                if !c.features.is_empty() { parts.push(format!("[{}]", c.features.join(", "))); }
                lines.push(format!("- {}", parts.join(" ")));
            }
            lines.join("\n")
        }

        Block::AppEnv { vars, .. } => {
            let mut lines = vec!["**Environment Variables**".to_string(), String::new()];
            lines.push("| Name | Required | Description |".to_string());
            lines.push("|---|---|---|".to_string());
            for v in vars {
                let req = if v.required { "yes" } else { "no" };
                let desc = v.description.as_deref().unwrap_or("");
                lines.push(format!("| `{}` | {} | {} |", v.name, req, desc));
            }
            lines.join("\n")
        }

        Block::AppDeploy { region, scale, domain, memory, properties, .. } => {
            let mut lines = vec!["**Deploy Configuration**".to_string()];
            if let Some(r) = region { lines.push(format!("- region: {r}")); }
            if let Some(s) = scale { lines.push(format!("- scale: {s}")); }
            if let Some(d) = domain { lines.push(format!("- domain: {d}")); }
            if let Some(m) = memory { lines.push(format!("- memory: {m}")); }
            for (k, v) in properties { lines.push(format!("- {k}: {v}")); }
            lines.join("\n")
        }

        Block::Row { title, description, actions, .. } => {
            let base = if description.is_empty() { format!("- {title}") } else { format!("- **{title}** — {description}") };
            if actions.is_empty() {
                base
            } else {
                let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
                format!("{base} [{}]", labels.join(" | "))
            }
        }

        Block::InfoCard { title, subtitle, summary, facts, steps, .. } => {
            let mut lines = vec![format!("## {title}")];
            if !subtitle.is_empty() { lines.push(format!("*{subtitle}*")); }
            if !summary.is_empty() { lines.push(String::new()); lines.push(summary.clone()); }
            for fact in facts { lines.push(format!("- **{}**: {}", fact[0], fact[1])); }
            for (i, step) in steps.iter().enumerate() { lines.push(format!("{}. {step}", i + 1)); }
            lines.join("\n")
        }

        // ── Interactive / application blocks (markdown fallback) ──

        Block::AppShell { children, .. } => {
            let parts: Vec<String> = children.iter().map(render_block).collect();
            parts.join("\n\n")
        }

        Block::Sidebar { children, .. } => {
            let parts: Vec<String> = children.iter().map(render_block).collect();
            parts.join("\n\n")
        }

        Block::Panel { children, .. } => {
            let parts: Vec<String> = children.iter().map(render_block).collect();
            parts.join("\n\n")
        }

        Block::TabBar { items, .. } => {
            let labels: Vec<String> = items.iter().map(|i| format!("- {}", i.label)).collect();
            labels.join("\n")
        }

        Block::TabContent { tab, children, .. } => {
            let parts: Vec<String> = children.iter().map(render_block).collect();
            format!("### {tab}\n\n{}", parts.join("\n\n"))
        }

        Block::Toolbar { title, items, .. } => {
            let mut labels: Vec<String> = Vec::new();
            if let Some(t) = title {
                labels.push(format!("**{t}**"));
            }
            labels.extend(items.iter().filter_map(|i| match i {
                crate::types::ToolbarItem::Button { label, .. } => Some(format!("- {}", label.as_deref().unwrap_or("Button"))),
                crate::types::ToolbarItem::Badge { value, .. } => Some(format!("- [{value}]")),
                crate::types::ToolbarItem::Text { value, .. } => Some(format!("- {value}")),
                _ => None,
            }));
            labels.join("\n")
        }

        Block::Drawer { children, .. } => {
            let parts: Vec<String> = children.iter().map(render_block).collect();
            parts.join("\n\n")
        }

        Block::Modal { name, title, children, .. } => {
            let heading = title.as_deref().unwrap_or(name);
            let parts: Vec<String> = children.iter().map(render_block).collect();
            format!("### {heading}\n\n{}", parts.join("\n\n"))
        }

        Block::SegmentedControl { active, segments, .. } => {
            let parts: Vec<String> = segments.iter().map(|s| {
                if active.as_ref().is_some_and(|a| a == &s.id) {
                    format!("**[{}]**", s.label)
                } else {
                    s.label.clone()
                }
            }).collect();
            parts.join(" | ")
        }

        Block::DropdownSelect { label, selected, options, .. } => {
            let heading = label.as_deref().or(selected.as_deref()).unwrap_or("Select");
            let lines: Vec<String> = options.iter().map(|o| {
                let marker = if selected.as_deref() == Some(o.label.as_str()) { " (selected)" } else { "" };
                match o.description.as_deref() {
                    Some(d) => format!("- **{}**{marker} — {d}", o.label),
                    None => format!("- **{}**{marker}", o.label),
                }
            }).collect();
            format!("{heading}:\n\n{}", lines.join("\n"))
        }

        Block::CommandPalette { items, .. } => {
            let lines: Vec<String> = items.iter().map(|i| {
                let desc = i.description.as_deref().unwrap_or("");
                format!("- **{}** — {desc}", i.label)
            }).collect();
            lines.join("\n")
        }

        Block::CodeEditor { lang, content, .. } => {
            let lang_tag = lang.as_deref().unwrap_or("");
            format!("```{lang_tag}\n{content}\n```")
        }

        Block::BlockEditor { .. } => "*Block editor*".to_string(),

        Block::Terminal { .. } => "*Terminal*".to_string(),

        Block::NavTree { .. } => "*File tree*".to_string(),

        Block::Badge { value, .. } => format!("[{value}]"),

        Block::SuggestionChips { .. } => "*Suggestion chips*".to_string(),

        Block::ChatThread { messages, .. } => {
            if messages.is_empty() {
                "*Chat thread*".to_string()
            } else {
                // Sequential message list (registry degradation string).
                messages
                    .iter()
                    .map(|m| match m.sender.as_deref() {
                        Some(s) => format!("- **{s}**: {}", m.text),
                        None => format!("- {}", m.text),
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }

        Block::ChipInput { label, chips, .. } => {
            // Labeled chip list (registry degradation string).
            let lead = label.as_deref().unwrap_or("Recipients:");
            if chips.is_empty() {
                format!("**{lead}**")
            } else {
                format!("**{lead}** {}", chips.join(", "))
            }
        }

        Block::RecipientPicker { source, mode, .. } => {
            let mode_str = if mode == "multi" { "multi-select" } else { "single-select" };
            format!("**Recipient picker** ({mode_str})\nSource: `{source}`")
        }

        Block::Qr { mode, .. } => {
            if mode == "scan" { "*QR scanner*".to_string() } else { "*QR code*".to_string() }
        }

        Block::ChatInputSimple { placeholder, .. } => {
            let ph = placeholder.as_deref().unwrap_or("Message...");
            format!("*[{ph}]*")
        }

        Block::Progress { steps, .. } => {
            let lines: Vec<String> = steps.iter().enumerate().map(|(i, s)| {
                let marker = match s.status.as_str() {
                    "done" => "\u{2713}",
                    "active" => "\u{25CF}",
                    _ => "\u{25CB}",
                };
                format!("{}. {} {marker}", i + 1, s.label)
            }).collect();
            lines.join("\n")
        }

        Block::LogStream { .. } => "*Log stream*".to_string(),

        Block::ProblemList { .. } => "*Problem list*".to_string(),

    }
}

fn model_field_type_md(ft: &crate::types::ModelFieldType) -> String {
    use crate::types::ModelFieldType;
    match ft {
        ModelFieldType::Uuid => "uuid".to_string(),
        ModelFieldType::String => "string".to_string(),
        ModelFieldType::Int => "int".to_string(),
        ModelFieldType::Float => "float".to_string(),
        ModelFieldType::Bool => "bool".to_string(),
        ModelFieldType::Datetime => "datetime".to_string(),
        ModelFieldType::Text => "text".to_string(),
        ModelFieldType::Json => "json".to_string(),
        ModelFieldType::Money => "money".to_string(),
        ModelFieldType::Image => "image".to_string(),
        ModelFieldType::Email => "email".to_string(),
        ModelFieldType::Url => "url".to_string(),
        ModelFieldType::Enum(variants) => format!("enum({})", variants.join(", ")),
        ModelFieldType::Ref(target) => format!("ref({target})"),
    }
}

fn constraint_md(c: &crate::types::FieldConstraint) -> String {
    use crate::types::FieldConstraint;
    match c {
        FieldConstraint::Primary => "primary".to_string(),
        FieldConstraint::Auto => "auto".to_string(),
        FieldConstraint::Required => "required".to_string(),
        FieldConstraint::Optional => "optional".to_string(),
        FieldConstraint::Unique => "unique".to_string(),
        FieldConstraint::Index => "index".to_string(),
        FieldConstraint::Max(n) => format!("max={n}"),
        FieldConstraint::Min(n) => format!("min={n}"),
        FieldConstraint::Default(v) => format!("default={v}"),
    }
}

fn http_method_md(m: crate::types::HttpMethod) -> &'static str {
    use crate::types::HttpMethod;
    match m {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn auth_provider_md(p: crate::types::AuthProvider) -> &'static str {
    use crate::types::AuthProvider;
    match p {
        AuthProvider::Email => "email",
        AuthProvider::OAuth => "oauth",
        AuthProvider::ApiKey => "api-key",
        AuthProvider::Token => "token",
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
        CalloutType::Context => "Context",
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
    fn md_diagram_degrades_to_fenced_block() {
        let doc = doc_with(vec![Block::Diagram {
            diagram_type: "architecture".into(),
            title: Some("System Map".into()),
            content: "web -> api".into(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert!(md.contains("**System Map**"));
        assert!(md.contains("```diagram-architecture\nweb -> api\n```"));

        // No title + empty type -> bare `diagram` info string, no bold line.
        let doc = doc_with(vec![Block::Diagram {
            diagram_type: String::new(),
            title: None,
            content: "a -> b".into(),
            span: span(),
        }]);
        let md = to_markdown(&doc);
        assert_eq!(md, "```diagram\na -> b\n```");
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

//! Per-block-type content parsers (Pass 2 resolution).
//!
//! `resolve_block` converts a `Block::Unknown` into a typed variant based on
//! the block name. Unknown block names pass through unchanged.

use crate::types::{
    AttrValue, Attrs, BeforeAfterItem, Block, CalloutType, ColumnContent, DataFormat,
    DecisionStatus, EmbedType, FaqItem, FeatureCard, FooterSection, FormField, FormFieldType,
    GalleryItem, HeroButton, NavItem, PipelineStep, SocialLink, Span, StatItem, StepItem,
    StyleProperty, TabPanel, TaskItem, Trend,
};

/// Resolve a `Block::Unknown` into a typed variant, if the name matches a known
/// block type. Unrecognised names are returned unchanged.
pub fn resolve_block(block: Block) -> Block {
    let Block::Unknown {
        name,
        attrs,
        content,
        span,
    } = &block
    else {
        return block;
    };

    match name.as_str() {
        "callout" => parse_callout(attrs, content, *span),
        "data" => parse_data(attrs, content, *span),
        "code" => parse_code(attrs, content, *span),
        "tasks" => parse_tasks(content, *span),
        "decision" => parse_decision(attrs, content, *span),
        "metric" => parse_metric(attrs, *span),
        "summary" => parse_summary(content, *span),
        "figure" => parse_figure(attrs, *span),
        "tabs" => parse_tabs(content, *span),
        "columns" => parse_columns(content, *span),
        "quote" => parse_quote(attrs, content, *span),
        "cta" => parse_cta(attrs, *span),
        "hero-image" => parse_hero_image(attrs, *span),
        "testimonial" => parse_testimonial(attrs, content, *span),
        "style" => parse_style(content, *span),
        "faq" => parse_faq(content, *span),
        "pricing-table" => parse_pricing_table(content, *span),
        "site" => parse_site(attrs, content, *span),
        "page" => parse_page(attrs, content, *span),
        "nav" => parse_nav(attrs, content, *span),
        "embed" => parse_embed(attrs, *span),
        "form" => parse_form(attrs, content, *span),
        "gallery" => parse_gallery(content, *span),
        "footer" => parse_footer(content, *span),
        "details" => parse_details(attrs, content, *span),
        "divider" => parse_divider(attrs, *span),
        "hero" => parse_hero(attrs, content, *span),
        "features" => parse_features(attrs, content, *span),
        "steps" => parse_steps(content, *span),
        "stats" => parse_stats(content, *span),
        "comparison" => parse_comparison(attrs, content, *span),
        "logo" => parse_logo(attrs, *span),
        "toc" => parse_toc(attrs, *span),
        "before-after" => parse_before_after(attrs, content, *span),
        "pipeline" => parse_pipeline(content, *span),
        "section" => parse_section(attrs, content, *span),
        "product-card" => parse_product_card(attrs, content, *span),
        _ => block,
    }
}

// ------------------------------------------------------------------
// Attribute extraction helpers
// ------------------------------------------------------------------

fn attr_string(attrs: &Attrs, key: &str) -> Option<String> {
    attrs.get(key).and_then(|v| match v {
        AttrValue::String(s) => Some(s.clone()),
        AttrValue::Number(n) => Some(n.to_string()),
        AttrValue::Bool(b) => Some(b.to_string()),
        AttrValue::Null => None,
    })
}

fn attr_bool(attrs: &Attrs, key: &str) -> bool {
    attrs
        .get(key)
        .is_some_and(|v| matches!(v, AttrValue::Bool(true)))
}

// ------------------------------------------------------------------
// Per-block parsers
// ------------------------------------------------------------------

fn parse_callout(attrs: &Attrs, content: &str, span: Span) -> Block {
    let callout_type = attr_string(attrs, "type")
        .and_then(|s| match s.as_str() {
            "info" => Some(CalloutType::Info),
            "warning" => Some(CalloutType::Warning),
            "danger" => Some(CalloutType::Danger),
            "tip" => Some(CalloutType::Tip),
            "note" => Some(CalloutType::Note),
            "success" => Some(CalloutType::Success),
            _ => None,
        })
        .unwrap_or(CalloutType::Info);

    let title = attr_string(attrs, "title");

    Block::Callout {
        callout_type,
        title,
        content: content.to_string(),
        span,
    }
}

fn parse_data(attrs: &Attrs, content: &str, span: Span) -> Block {
    let id = attr_string(attrs, "id");
    let sortable = attr_bool(attrs, "sortable");

    let format = attr_string(attrs, "format")
        .and_then(|s| match s.as_str() {
            "table" => Some(DataFormat::Table),
            "csv" => Some(DataFormat::Csv),
            "json" => Some(DataFormat::Json),
            _ => None,
        })
        .unwrap_or(DataFormat::Table);

    let (headers, rows) = match format {
        DataFormat::Table => parse_table_content(content),
        DataFormat::Csv => parse_csv_content(content),
        DataFormat::Json => (Vec::new(), Vec::new()),
    };

    Block::Data {
        id,
        format,
        sortable,
        headers,
        rows,
        raw_content: content.to_string(),
        span,
    }
}

/// Parse pipe-delimited table content.
///
/// First non-empty line is headers. Lines that look like `|---|---|` are
/// separator rows and get skipped. Remaining lines are data rows.
fn parse_table_content(content: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut header_done = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip separator lines like |---|---| or | --- | --- |
        if is_table_separator(trimmed) {
            continue;
        }

        let cells: Vec<String> = split_pipe_row(trimmed);

        if !header_done {
            headers = cells;
            header_done = true;
        } else {
            rows.push(cells);
        }
    }

    (headers, rows)
}

/// Check whether a line is a markdown table separator (e.g. `|---|---|`).
fn is_table_separator(line: &str) -> bool {
    let stripped = line.trim().trim_matches('|').trim();
    if stripped.is_empty() {
        return false;
    }
    stripped
        .split('|')
        .all(|cell| cell.trim().chars().all(|c| c == '-' || c == ':'))
}

/// Split a pipe-delimited row into trimmed cell strings, stripping leading and
/// trailing pipes.
fn split_pipe_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    // Remove leading/trailing pipes.
    let inner = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed);
    let inner = inner
        .strip_suffix('|')
        .unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// Parse CSV content: newline-delimited, comma-separated.
fn parse_csv_content(content: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut header_done = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let cells: Vec<String> = trimmed.split(',').map(|c| c.trim().to_string()).collect();

        if !header_done {
            headers = cells;
            header_done = true;
        } else {
            rows.push(cells);
        }
    }

    (headers, rows)
}

fn parse_code(attrs: &Attrs, content: &str, span: Span) -> Block {
    let lang = attr_string(attrs, "lang");
    let file = attr_string(attrs, "file");
    let highlight = attr_string(attrs, "highlight")
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    Block::Code {
        lang,
        file,
        highlight,
        content: content.to_string(),
        span,
    }
}

fn parse_tasks(content: &str, span: Span) -> Block {
    let mut items = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        let (done, rest) = if let Some(rest) = trimmed.strip_prefix("- [x] ") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [X] ") {
            (true, rest)
        } else if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            (false, rest)
        } else {
            continue;
        };

        // Extract optional @assignee from end of text.
        let (text, assignee) = extract_assignee(rest);

        items.push(TaskItem {
            done,
            text,
            assignee,
        });
    }

    Block::Tasks { items, span }
}

/// Extract a trailing `@username` from the end of a task text.
///
/// Returns `(text_without_assignee, Option<assignee>)`.
fn extract_assignee(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim_end();
    if let Some(at_pos) = trimmed.rfind(" @") {
        let candidate = &trimmed[at_pos + 2..];
        // Assignee must be a single word (no spaces).
        if !candidate.is_empty() && !candidate.contains(' ') {
            let main_text = trimmed[..at_pos].trim_end().to_string();
            return (main_text, Some(candidate.to_string()));
        }
    }
    (text.to_string(), None)
}

fn parse_decision(attrs: &Attrs, content: &str, span: Span) -> Block {
    let status = attr_string(attrs, "status")
        .and_then(|s| match s.as_str() {
            "proposed" => Some(DecisionStatus::Proposed),
            "accepted" => Some(DecisionStatus::Accepted),
            "rejected" => Some(DecisionStatus::Rejected),
            "superseded" => Some(DecisionStatus::Superseded),
            _ => None,
        })
        .unwrap_or(DecisionStatus::Proposed);

    let date = attr_string(attrs, "date");

    let deciders = attr_string(attrs, "deciders")
        .map(|s| s.split(',').map(|d| d.trim().to_string()).collect())
        .unwrap_or_default();

    Block::Decision {
        status,
        date,
        deciders,
        content: content.to_string(),
        span,
    }
}

fn parse_metric(attrs: &Attrs, span: Span) -> Block {
    let label = attr_string(attrs, "label").unwrap_or_default();
    let value = attr_string(attrs, "value").unwrap_or_default();

    let trend = attr_string(attrs, "trend").and_then(|s| match s.as_str() {
        "up" => Some(Trend::Up),
        "down" => Some(Trend::Down),
        "flat" => Some(Trend::Flat),
        _ => None,
    });

    let unit = attr_string(attrs, "unit");

    Block::Metric {
        label,
        value,
        trend,
        unit,
        span,
    }
}

fn parse_summary(content: &str, span: Span) -> Block {
    Block::Summary {
        content: content.to_string(),
        span,
    }
}

fn parse_tabs(content: &str, span: Span) -> Block {
    let mut tabs = Vec::new();
    let mut current_label: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Tab labels: `## Label` or `### Label` inside tabs block
        if let Some(rest) = trimmed.strip_prefix("## ") {
            // Flush previous tab
            if let Some(label) = current_label.take() {
                tabs.push(TabPanel {
                    label,
                    content: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
            current_label = Some(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("### ") {
            if let Some(label) = current_label.take() {
                tabs.push(TabPanel {
                    label,
                    content: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
            current_label = Some(rest.trim().to_string());
        } else {
            current_lines.push(line);
        }
    }

    // Flush final tab
    if let Some(label) = current_label {
        tabs.push(TabPanel {
            label,
            content: current_lines.join("\n").trim().to_string(),
        });
    } else if !current_lines.is_empty() {
        // No headers found — single unnamed tab
        let text = current_lines.join("\n").trim().to_string();
        if !text.is_empty() {
            tabs.push(TabPanel {
                label: "Tab 1".to_string(),
                content: text,
            });
        }
    }

    Block::Tabs { tabs, span }
}

fn parse_columns(content: &str, span: Span) -> Block {
    let mut columns = Vec::new();
    let mut current_lines: Vec<&str> = Vec::new();
    let mut found_separator = false;

    for line in content.lines() {
        let trimmed = line.trim();
        // Nested :::column directives serve as separators
        if trimmed.starts_with(":::column") {
            if !current_lines.is_empty() {
                columns.push(ColumnContent {
                    content: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
            found_separator = true;
        } else if trimmed == ":::" {
            // Close a :::column — flush content
            if found_separator {
                columns.push(ColumnContent {
                    content: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
        } else if trimmed == "---" && !found_separator {
            // Horizontal rule as column separator (simpler syntax)
            columns.push(ColumnContent {
                content: current_lines.join("\n").trim().to_string(),
            });
            current_lines.clear();
            found_separator = true;
        } else {
            current_lines.push(line);
        }
    }

    // Flush remaining content
    let remaining = current_lines.join("\n").trim().to_string();
    if !remaining.is_empty() {
        columns.push(ColumnContent {
            content: remaining,
        });
    }

    // If no separators were found, treat the whole thing as one column
    if columns.is_empty() {
        columns.push(ColumnContent {
            content: content.trim().to_string(),
        });
    }

    Block::Columns { columns, span }
}

fn parse_quote(attrs: &Attrs, content: &str, span: Span) -> Block {
    let attribution = attr_string(attrs, "by")
        .or_else(|| attr_string(attrs, "attribution"))
        .or_else(|| attr_string(attrs, "author"));
    let cite = attr_string(attrs, "cite")
        .or_else(|| attr_string(attrs, "source"));

    Block::Quote {
        content: content.to_string(),
        attribution,
        cite,
        span,
    }
}

fn parse_figure(attrs: &Attrs, span: Span) -> Block {
    let src = attr_string(attrs, "src").unwrap_or_default();
    let caption = attr_string(attrs, "caption");
    let alt = attr_string(attrs, "alt");
    let width = attr_string(attrs, "width");

    Block::Figure {
        src,
        caption,
        alt,
        width,
        span,
    }
}

fn parse_cta(attrs: &Attrs, span: Span) -> Block {
    let label = attr_string(attrs, "label").unwrap_or_default();
    let href = attr_string(attrs, "href").unwrap_or_default();
    let primary = attr_bool(attrs, "primary");
    let icon = attr_string(attrs, "icon");

    Block::Cta {
        label,
        href,
        primary,
        icon,
        span,
    }
}

fn parse_hero_image(attrs: &Attrs, span: Span) -> Block {
    let src = attr_string(attrs, "src").unwrap_or_default();
    let alt = attr_string(attrs, "alt");

    Block::HeroImage { src, alt, span }
}

fn parse_testimonial(attrs: &Attrs, content: &str, span: Span) -> Block {
    let author = attr_string(attrs, "author")
        .or_else(|| attr_string(attrs, "name"));
    let role = attr_string(attrs, "role")
        .or_else(|| attr_string(attrs, "title"));
    let company = attr_string(attrs, "company")
        .or_else(|| attr_string(attrs, "org"));

    Block::Testimonial {
        content: content.to_string(),
        author,
        role,
        company,
        span,
    }
}

fn parse_style(content: &str, span: Span) -> Block {
    let mut properties = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Parse "key: value" lines
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                properties.push(StyleProperty { key, value });
            }
        }
    }

    Block::Style { properties, span }
}

fn parse_faq(content: &str, span: Span) -> Block {
    let mut items = Vec::new();
    let mut current_question: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // FAQ questions: `### Question` inside faq block
        if let Some(rest) = trimmed.strip_prefix("### ") {
            // Flush previous item
            if let Some(question) = current_question.take() {
                items.push(FaqItem {
                    question,
                    answer: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
            current_question = Some(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            // Also accept ## headers
            if let Some(question) = current_question.take() {
                items.push(FaqItem {
                    question,
                    answer: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
            current_question = Some(rest.trim().to_string());
        } else {
            current_lines.push(line);
        }
    }

    // Flush final item
    if let Some(question) = current_question {
        items.push(FaqItem {
            question,
            answer: current_lines.join("\n").trim().to_string(),
        });
    }

    Block::Faq { items, span }
}

fn parse_pricing_table(content: &str, span: Span) -> Block {
    let (headers, rows) = parse_table_content(content);

    Block::PricingTable {
        headers,
        rows,
        span,
    }
}

fn parse_site(attrs: &Attrs, content: &str, span: Span) -> Block {
    let domain = attr_string(attrs, "domain");

    let mut properties = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                properties.push(StyleProperty { key, value });
            }
        }
    }

    Block::Site {
        domain,
        properties,
        span,
    }
}

fn parse_page(attrs: &Attrs, content: &str, span: Span) -> Block {
    let route = attr_string(attrs, "route").unwrap_or_default();
    let layout = attr_string(attrs, "layout");
    let title = attr_string(attrs, "title");
    let sidebar = attr_bool(attrs, "sidebar");

    // Scan content for leaf directives, interleaving with markdown.
    let children = parse_page_children(content);

    Block::Page {
        route,
        layout,
        title,
        sidebar,
        content: content.to_string(),
        children,
        span,
    }
}

// ------------------------------------------------------------------
// Page child block scanner
// ------------------------------------------------------------------

/// Scan page content for both leaf directives and container blocks.
///
/// Container blocks (`::name\ncontent\n::`) are collected and resolved via
/// `resolve_block()`. Leaf directives (`::name[attrs]` with no matching closer)
/// are handled as before. Consecutive non-directive lines are collected as
/// `Block::Markdown`.
fn parse_page_children(content: &str) -> Vec<Block> {
    let lines: Vec<&str> = content.lines().collect();
    let mut children = Vec::new();
    let mut md_lines: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Check for opening directive
        if let Some((depth, name, attrs_str)) = crate::parse::opening_directive(trimmed) {
            // Scan ahead for matching closing directive
            if let Some((content_str, end_idx)) =
                scan_container_close(&lines, i + 1, depth)
            {
                // Container block — flush markdown, resolve, advance past closer
                flush_md_lines(&mut md_lines, &mut children);

                let attrs = crate::attrs::parse_attrs(&attrs_str).unwrap_or_default();
                let dummy_span = Span {
                    start_line: 0,
                    end_line: 0,
                    start_offset: 0,
                    end_offset: 0,
                };

                let block = Block::Unknown {
                    name,
                    attrs,
                    content: content_str,
                    span: dummy_span,
                };
                children.push(resolve_block(block));

                i = end_idx + 1; // skip past closing ::
                continue;
            } else {
                // No matching closer — treat as leaf directive
                if let Some(block) = try_parse_leaf_directive(lines[i]) {
                    flush_md_lines(&mut md_lines, &mut children);
                    children.push(block);
                    i += 1;
                    continue;
                }
                // Not a valid leaf either — fall through to markdown
            }
        }

        // Skip bare closing markers that aren't matched to anything
        if crate::parse::closing_directive_depth(trimmed).is_some() {
            // Orphan closer — don't leak into markdown
            i += 1;
            continue;
        }

        md_lines.push(lines[i]);
        i += 1;
    }

    // Flush remaining markdown
    flush_md_lines(&mut md_lines, &mut children);

    children
}

fn parse_nav(attrs: &Attrs, content: &str, span: Span) -> Block {
    let logo = attr_string(attrs, "logo");
    let mut items = Vec::new();

    // Parse markdown-style link lines: `- [Label](href)` or `- [Label](href){icon=name}`
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("- [") {
            continue;
        }
        let rest = &trimmed[3..]; // skip "- ["
        if let Some(label_end) = rest.find("](") {
            let label = rest[..label_end].to_string();
            let after_bracket = &rest[label_end + 2..]; // skip "]("
            if let Some(href_end) = after_bracket.find(')') {
                let href = after_bracket[..href_end].to_string();
                // Check for optional {icon=name} after the link
                let after_link = &after_bracket[href_end + 1..];
                let icon = if after_link.trim_start().starts_with("{icon=") {
                    let icon_start = after_link.find("{icon=").unwrap() + 6;
                    let icon_part = &after_link[icon_start..];
                    icon_part.find('}').map(|end| icon_part[..end].to_string())
                } else {
                    None
                };
                items.push(NavItem { label, href, icon });
            }
        }
    }

    Block::Nav { items, logo, span }
}

fn parse_embed(attrs: &Attrs, span: Span) -> Block {
    let src = attr_string(attrs, "src").unwrap_or_default();
    let title = attr_string(attrs, "title");
    let width = attr_string(attrs, "width");
    let height = attr_string(attrs, "height");

    let embed_type = attr_string(attrs, "type")
        .and_then(|s| match s.as_str() {
            "map" => Some(EmbedType::Map),
            "video" => Some(EmbedType::Video),
            "audio" => Some(EmbedType::Audio),
            "generic" => Some(EmbedType::Generic),
            _ => None,
        })
        .or_else(|| {
            // Auto-detect from URL
            let s = src.to_lowercase();
            if s.contains("google.com/maps") || s.contains("maps.google") {
                Some(EmbedType::Map)
            } else if s.contains("youtube.com") || s.contains("youtu.be") || s.contains("vimeo.com") {
                Some(EmbedType::Video)
            } else if s.contains("soundcloud.com") || s.contains("spotify.com") {
                Some(EmbedType::Audio)
            } else {
                None
            }
        });

    Block::Embed {
        src,
        embed_type,
        width,
        height,
        title,
        span,
    }
}

fn parse_form(attrs: &Attrs, content: &str, span: Span) -> Block {
    let submit_label = attr_string(attrs, "submit");
    let mut fields = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse field lines: `- Label (type) *` or `- Label (type, "placeholder")`
        // or `- Label (select: Option A | Option B | Option C) *`
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            let (label_part, type_part) = if let Some(paren_start) = rest.find('(') {
                let label = rest[..paren_start].trim();
                let after_paren = &rest[paren_start + 1..];
                let paren_end = after_paren.find(')').unwrap_or(after_paren.len());
                let type_str = &after_paren[..paren_end];
                let remainder = &after_paren[paren_end..].trim_start_matches(')');
                (label, Some((type_str.to_string(), remainder.to_string())))
            } else {
                (rest.trim_end_matches(" *").trim_end_matches('*'), None)
            };

            let required = rest.ends_with('*') || rest.ends_with("* ");

            let (field_type, placeholder, options) = match &type_part {
                Some((type_str, _)) => {
                    let type_str = type_str.trim();
                    // Check for select with options: "select: A | B | C"
                    if let Some(opts_part) = type_str.strip_prefix("select:") {
                        let opts: Vec<String> = opts_part
                            .split('|')
                            .map(|o| o.trim().to_string())
                            .filter(|o| !o.is_empty())
                            .collect();
                        (FormFieldType::Select, None, opts)
                    } else {
                        // Check for placeholder after comma: "text, Enter your name"
                        let (type_name, placeholder) = if let Some((t, p)) = type_str.split_once(',') {
                            (t.trim(), Some(p.trim().trim_matches('"').to_string()))
                        } else {
                            (type_str, None)
                        };
                        let ft = match type_name {
                            "email" => FormFieldType::Email,
                            "tel" | "phone" => FormFieldType::Tel,
                            "date" => FormFieldType::Date,
                            "number" => FormFieldType::Number,
                            "select" => FormFieldType::Select,
                            "textarea" | "multiline" => FormFieldType::Textarea,
                            _ => FormFieldType::Text,
                        };
                        (ft, placeholder, Vec::new())
                    }
                }
                None => (FormFieldType::Text, None, Vec::new()),
            };

            // Generate a machine-safe name from the label
            let name = label_part
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric(), "_")
                .trim_matches('_')
                .to_string();

            fields.push(FormField {
                label: label_part.to_string(),
                name,
                field_type,
                required,
                placeholder,
                options,
            });
        }
    }

    Block::Form {
        fields,
        submit_label,
        span,
    }
}

fn parse_gallery(content: &str, span: Span) -> Block {
    let mut items = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse gallery items: `![alt](src) Category: caption`
        // or simpler: `![alt](src) caption`
        // or minimal: `![](src)`
        if let Some(after_bang) = trimmed.strip_prefix("![") {
            if let Some(alt_end) = after_bang.find("](") {
                let alt_text = &after_bang[..alt_end];
                let after_bracket = &after_bang[alt_end + 2..];
                if let Some(src_end) = after_bracket.find(')') {
                    let src = after_bracket[..src_end].to_string();
                    let remainder = after_bracket[src_end + 1..].trim();

                    let alt = if alt_text.is_empty() {
                        None
                    } else {
                        Some(alt_text.to_string())
                    };

                    // Check for "Category: caption" pattern
                    let (category, caption) = if let Some(colon_pos) = remainder.find(':') {
                        let cat = remainder[..colon_pos].trim();
                        let cap = remainder[colon_pos + 1..].trim();
                        (
                            if cat.is_empty() { None } else { Some(cat.to_string()) },
                            if cap.is_empty() { None } else { Some(cap.to_string()) },
                        )
                    } else if remainder.is_empty() {
                        (None, None)
                    } else {
                        (None, Some(remainder.to_string()))
                    };

                    items.push(GalleryItem {
                        src,
                        caption,
                        alt,
                        category,
                    });
                }
            }
        }
    }

    // Determine column count from number of items
    let columns = if items.len() <= 2 {
        Some(2)
    } else if items.len() <= 6 {
        Some(3)
    } else {
        Some(4)
    };

    Block::Gallery {
        items,
        columns,
        span,
    }
}

fn parse_footer(content: &str, span: Span) -> Block {
    let mut sections: Vec<FooterSection> = Vec::new();
    let mut copyright: Option<String> = None;
    let mut social: Vec<SocialLink> = Vec::new();
    let mut current_heading: Option<String> = None;
    let mut current_links: Vec<NavItem> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Copyright line: starts with (c), ©, or "Copyright"
        if trimmed.starts_with("(c)") || trimmed.starts_with('\u{00a9}') || trimmed.to_lowercase().starts_with("copyright") {
            copyright = Some(trimmed.to_string());
            continue;
        }

        // Social link: `@platform url`
        if trimmed.starts_with('@') {
            if let Some((platform, href)) = trimmed[1..].split_once(|c: char| c.is_whitespace()) {
                social.push(SocialLink {
                    platform: platform.trim().to_string(),
                    href: href.trim().to_string(),
                });
            }
            continue;
        }

        // Section heading: `## Heading` or `### Heading`
        if let Some(rest) = trimmed.strip_prefix("## ") {
            // Flush previous section
            if let Some(heading) = current_heading.take() {
                sections.push(FooterSection {
                    heading,
                    links: std::mem::take(&mut current_links),
                });
            }
            current_heading = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            if let Some(heading) = current_heading.take() {
                sections.push(FooterSection {
                    heading,
                    links: std::mem::take(&mut current_links),
                });
            }
            current_heading = Some(rest.trim().to_string());
            continue;
        }

        // Link: `- [Label](href)` — reuse nav link parsing
        if let Some(rest) = trimmed.strip_prefix("- [") {
            if let Some(label_end) = rest.find("](") {
                let label = rest[..label_end].to_string();
                let after_bracket = &rest[label_end + 2..];
                if let Some(href_end) = after_bracket.find(')') {
                    let href = after_bracket[..href_end].to_string();
                    current_links.push(NavItem {
                        label,
                        href,
                        icon: None,
                    });
                }
            }
            continue;
        }

        // Plain text link: `- text` (no href, displayed as text)
        if let Some(rest) = trimmed.strip_prefix("- ") {
            current_links.push(NavItem {
                label: rest.trim().to_string(),
                href: String::new(),
                icon: None,
            });
        }
    }

    // Flush final section
    if let Some(heading) = current_heading {
        sections.push(FooterSection {
            heading,
            links: current_links,
        });
    }

    Block::Footer {
        sections,
        copyright,
        social,
        span,
    }
}

fn parse_details(attrs: &Attrs, content: &str, span: Span) -> Block {
    let title = attr_string(attrs, "title");
    let open = attr_bool(attrs, "open");
    Block::Details {
        title,
        open,
        content: content.to_string(),
        span,
    }
}

fn parse_divider(attrs: &Attrs, span: Span) -> Block {
    let label = attr_string(attrs, "label");
    Block::Divider { label, span }
}

/// Scan forward from `start` looking for a closing directive matching `open_depth`.
///
/// Tracks nesting so that `:::column` / `:::` inside a `::columns` block are
/// skipped correctly. If a sibling opening directive at the same depth is
/// encountered (e.g. `::callout` while scanning for `::hero-image`'s closer),
/// we bail out — the original block has no closer and should be treated as a leaf.
///
/// Returns `(collected_content, closing_line_index)`.
fn scan_container_close(lines: &[&str], start: usize, open_depth: usize) -> Option<(String, usize)> {
    let mut nesting = 0usize;
    let mut content_lines: Vec<&str> = Vec::new();

    for i in start..lines.len() {
        let trimmed = lines[i].trim();

        // Check for a closing directive
        if let Some(close_depth) = crate::parse::closing_directive_depth(trimmed) {
            if close_depth == open_depth && nesting == 0 {
                // This closes our container
                return Some((content_lines.join("\n"), i));
            }
            // Might close a nested block
            if nesting > 0 {
                nesting -= 1;
                content_lines.push(lines[i]);
                continue;
            }
            // Unmatched closer at wrong depth — include as content
            content_lines.push(lines[i]);
            continue;
        }

        // Check for opening directives
        if let Some((nested_depth, _, _)) = crate::parse::opening_directive(trimmed) {
            if nested_depth == open_depth && nesting == 0 {
                // Sibling block at same depth — our block has no closer
                return None;
            }
            if nested_depth > open_depth {
                nesting += 1;
            }
        }

        content_lines.push(lines[i]);
    }

    // No matching closer found
    None
}

/// Try to parse a single line as a leaf directive (`::name[attrs]`).
///
/// Returns `Some(resolved_block)` if the line matches, `None` otherwise.
fn try_parse_leaf_directive(line: &str) -> Option<Block> {
    let trimmed = line.trim();
    if !trimmed.starts_with("::") {
        return None;
    }

    // Count leading colons — must be exactly 2 for a top-level directive.
    let depth = trimmed.chars().take_while(|&c| c == ':').count();
    if depth != 2 {
        return None;
    }

    let rest = &trimmed[2..];
    if rest.is_empty() {
        return None; // closing `::`, not an opening directive
    }

    // Must start with alphabetic character.
    let first = rest.chars().next()?;
    if !first.is_alphabetic() {
        return None;
    }

    // Scan block name.
    let name_end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    let remainder = &rest[name_end..];

    // Extract attrs if present.
    let attrs_str = if remainder.starts_with('[') {
        if let Some(close) = remainder.find(']') {
            &remainder[..=close]
        } else {
            remainder
        }
    } else {
        ""
    };

    let attrs = crate::attrs::parse_attrs(attrs_str).unwrap_or_default();
    let dummy_span = Span {
        start_line: 0,
        end_line: 0,
        start_offset: 0,
        end_offset: 0,
    };

    let block = Block::Unknown {
        name: name.to_string(),
        attrs,
        content: String::new(),
        span: dummy_span,
    };

    Some(resolve_block(block))
}

/// Flush accumulated markdown lines into a `Block::Markdown` if non-empty.
fn flush_md_lines(lines: &mut Vec<&str>, children: &mut Vec<Block>) {
    let text = lines.join("\n");
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        children.push(Block::Markdown {
            content: text.trim().to_string(),
            span: Span {
                start_line: 0,
                end_line: 0,
                start_offset: 0,
                end_offset: 0,
            },
        });
    }
    lines.clear();
}

// ------------------------------------------------------------------
// Landing page block parsers
// ------------------------------------------------------------------

fn parse_hero(attrs: &Attrs, content: &str, span: Span) -> Block {
    let badge = attr_string(attrs, "badge");
    let align = attr_string(attrs, "align").unwrap_or_else(|| "center".to_string());
    let image = attr_string(attrs, "image");

    let mut headline: Option<String> = None;
    let mut subtitle_lines: Vec<&str> = Vec::new();
    let mut buttons: Vec<HeroButton> = Vec::new();
    let mut in_subtitle = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if in_subtitle {
                in_subtitle = false;
            }
            continue;
        }

        // Heading -> headline
        if let Some(rest) = trimmed.strip_prefix("# ") {
            headline = Some(rest.trim().to_string());
            in_subtitle = false;
            continue;
        }

        // Markdown link -> button: [Label](href){primary} or [Label](href)
        if trimmed.starts_with('[')
            && let Some(label_end) = trimmed.find("](") {
                let label = trimmed[1..label_end].to_string();
                let after = &trimmed[label_end + 2..];
                // Find closing paren, possibly followed by {primary}
                if let Some(href_end) = after.find(')') {
                    let href = after[..href_end].to_string();
                    let suffix = after[href_end + 1..].trim();
                    let primary = suffix == "{primary}";
                    buttons.push(HeroButton {
                        label,
                        href,
                        primary,
                    });
                    continue;
                }
            }

        // Otherwise it's subtitle text
        if headline.is_some() && buttons.is_empty() {
            subtitle_lines.push(trimmed);
            in_subtitle = true;
        }
    }

    let subtitle = if subtitle_lines.is_empty() {
        None
    } else {
        Some(subtitle_lines.join(" "))
    };

    Block::Hero {
        headline,
        subtitle,
        badge,
        align,
        image,
        buttons,
        content: content.to_string(),
        span,
    }
}

fn parse_features(attrs: &Attrs, content: &str, span: Span) -> Block {
    let cols = attr_string(attrs, "cols").and_then(|s| s.parse::<u32>().ok());
    let mut cards: Vec<FeatureCard> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_icon: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    let flush = |title: String, icon: Option<String>, lines: &[&str]| -> FeatureCard {
        let mut body_lines: Vec<&str> = Vec::new();
        let mut link_label: Option<String> = None;
        let mut link_href: Option<String> = None;

        for &line in lines {
            body_lines.push(line);
        }

        // Check if the last non-empty line is a markdown link
        if let Some(last) = body_lines.iter().rev().find(|l| !l.trim().is_empty()) {
            let t = last.trim();
            if t.starts_with('[')
                && let Some(le) = t.find("](") {
                    let after = &t[le + 2..];
                    if let Some(he) = after.find(')') {
                        link_label = Some(t[1..le].to_string());
                        link_href = Some(after[..he].to_string());
                        // Remove last line from body
                        while body_lines.last().is_some_and(|l| l.trim().is_empty()) {
                            body_lines.pop();
                        }
                        body_lines.pop(); // remove the link line
                    }
                }
        }

        let body = body_lines.join("\n").trim().to_string();
        FeatureCard {
            title,
            icon,
            body,
            link_label,
            link_href,
        }
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            // Flush previous card
            if let Some(title) = current_title.take() {
                cards.push(flush(title, current_icon.take(), &current_lines));
                current_lines.clear();
            }
            // Extract {icon=name} from heading
            let heading = rest.trim();
            if let Some(brace_start) = heading.rfind("{icon=") {
                let before = heading[..brace_start].trim().to_string();
                let after = &heading[brace_start + 6..];
                let icon_name = after.trim_end_matches('}').to_string();
                current_title = Some(before);
                current_icon = Some(icon_name);
            } else {
                current_title = Some(heading.to_string());
                current_icon = None;
            }
        } else {
            current_lines.push(line);
        }
    }

    // Flush final card
    if let Some(title) = current_title {
        cards.push(flush(title, current_icon, &current_lines));
    }

    Block::Features { cards, cols, span }
}

fn parse_steps(content: &str, span: Span) -> Block {
    let mut steps: Vec<StepItem> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_time: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            // Flush previous step
            if let Some(title) = current_title.take() {
                steps.push(StepItem {
                    title,
                    time: current_time.take(),
                    body: current_lines.join("\n").trim().to_string(),
                });
                current_lines.clear();
            }
            // Extract {time="X"} from heading
            let heading = rest.trim();
            if let Some(brace_start) = heading.rfind("{time=") {
                let before = heading[..brace_start].trim().to_string();
                let after = &heading[brace_start + 6..];
                let time_val = after.trim_end_matches('}').trim_matches('"').to_string();
                current_title = Some(before);
                current_time = Some(time_val);
            } else {
                current_title = Some(heading.to_string());
                current_time = None;
            }
        } else {
            current_lines.push(line);
        }
    }

    // Flush final step
    if let Some(title) = current_title {
        steps.push(StepItem {
            title,
            time: current_time,
            body: current_lines.join("\n").trim().to_string(),
        });
    }

    Block::Steps { steps, span }
}

fn parse_stats(content: &str, span: Span) -> Block {
    let mut items: Vec<StatItem> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let text = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if text.is_empty() {
            continue;
        }

        // Extract {label="..." color="..."} suffix
        if let Some(brace_start) = text.rfind('{') {
            let value = text[..brace_start].trim().to_string();
            let attrs_str = &text[brace_start + 1..].trim_end_matches('}');
            let label = extract_quoted_attr(attrs_str, "label").unwrap_or_default();
            let color = extract_quoted_attr(attrs_str, "color");
            if !value.is_empty() && !label.is_empty() {
                items.push(StatItem {
                    value,
                    label,
                    color,
                });
            }
        }
    }

    Block::Stats { items, span }
}

/// Extract a quoted attribute value from an inline attr string like `label="Foo" color="#hex"`.
fn extract_quoted_attr(s: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=", key);
    if let Some(pos) = s.find(&pattern) {
        let after = &s[pos + pattern.len()..];
        if let Some(inner) = after.strip_prefix('"') {
            if let Some(end) = inner.find('"') {
                return Some(inner[..end].to_string());
            }
        } else {
            // Unquoted value -- take until space or end
            let end = after.find(' ').unwrap_or(after.len());
            return Some(after[..end].to_string());
        }
    }
    None
}

fn parse_comparison(attrs: &Attrs, content: &str, span: Span) -> Block {
    let highlight = attr_string(attrs, "highlight");

    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('|') {
            continue;
        }
        // Skip separator rows
        let inner = trimmed.trim_matches('|');
        if inner.chars().all(|c| c == '-' || c == '|' || c == ' ' || c == ':') {
            continue;
        }
        let cells: Vec<String> = trimmed
            .split('|')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect();
        if headers.is_empty() {
            headers = cells;
        } else {
            rows.push(cells);
        }
    }

    Block::Comparison {
        headers,
        rows,
        highlight,
        span,
    }
}

fn parse_logo(attrs: &Attrs, span: Span) -> Block {
    let src = attr_string(attrs, "src").unwrap_or_default();
    let alt = attr_string(attrs, "alt");
    let size = attr_string(attrs, "size").and_then(|s| s.parse::<u32>().ok());
    Block::Logo {
        src,
        alt,
        size,
        span,
    }
}

fn parse_toc(attrs: &Attrs, span: Span) -> Block {
    let depth = attr_string(attrs, "depth")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(3);
    Block::Toc {
        depth,
        entries: Vec::new(),
        span,
    }
}

fn parse_before_after(attrs: &Attrs, content: &str, span: Span) -> Block {
    let transition = attr_string(attrs, "transition");
    let mut before_items: Vec<BeforeAfterItem> = Vec::new();
    let mut after_items: Vec<BeforeAfterItem> = Vec::new();
    let mut in_after = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("### before") {
            in_after = false;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("### after") {
            in_after = true;
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let text = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if let Some(pipe_pos) = text.find(" | ") {
            let label = text[..pipe_pos].trim().to_string();
            let detail = text[pipe_pos + 3..].trim().to_string();
            let item = BeforeAfterItem { label, detail };
            if in_after {
                after_items.push(item);
            } else {
                before_items.push(item);
            }
        }
    }

    Block::BeforeAfter {
        before_items,
        after_items,
        transition,
        span,
    }
}

fn parse_pipeline(content: &str, span: Span) -> Block {
    let mut steps: Vec<PipelineStep> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let text = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if let Some(pipe_pos) = text.find(" | ") {
            let label = text[..pipe_pos].trim().to_string();
            let description = Some(text[pipe_pos + 3..].trim().to_string());
            steps.push(PipelineStep { label, description });
        } else {
            steps.push(PipelineStep {
                label: text.trim().to_string(),
                description: None,
            });
        }
    }

    Block::Pipeline { steps, span }
}

fn parse_section(attrs: &Attrs, content: &str, span: Span) -> Block {
    let bg = attr_string(attrs, "bg");
    let mut headline: Option<String> = None;
    let mut subtitle: Option<String> = None;
    let mut body_start = 0;
    let mut found_headline = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() && !found_headline {
            body_start = i + 1;
            continue;
        }
        if !found_headline {
            if let Some(rest) = trimmed.strip_prefix("## ") {
                headline = Some(rest.trim().to_string());
                found_headline = true;
                body_start = i + 1;
                continue;
            }
        }
        if found_headline && subtitle.is_none() && !trimmed.is_empty() {
            // Check if this line looks like a subtitle (plain text, not a directive or heading)
            if !trimmed.starts_with("::") && !trimmed.starts_with('#') && !trimmed.starts_with("- ") {
                subtitle = Some(trimmed.to_string());
                body_start = i + 1;
                continue;
            }
        }
        if found_headline {
            body_start = i;
            break;
        }
    }

    let remaining: String = content
        .lines()
        .skip(body_start)
        .collect::<Vec<_>>()
        .join("\n");
    let remaining = remaining.trim().to_string();

    let children = parse_page_children(&remaining);

    Block::Section {
        bg,
        headline,
        subtitle,
        content: content.to_string(),
        children,
        span,
    }
}

fn parse_product_card(attrs: &Attrs, content: &str, span: Span) -> Block {
    let badge = attr_string(attrs, "badge");
    let badge_color = attr_string(attrs, "badge-color");

    let mut title = String::new();
    let mut subtitle: Option<String> = None;
    let mut body_lines: Vec<&str> = Vec::new();
    let mut features: Vec<String> = Vec::new();
    let mut cta_label: Option<String> = None;
    let mut cta_href: Option<String> = None;

    // States: 0=before_title, 1=after_title(subtitle), 2=body, 3=features
    let mut state = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        match state {
            0 => {
                if let Some(rest) = trimmed.strip_prefix("## ") {
                    title = rest.trim().to_string();
                    state = 1;
                }
            }
            1 => {
                if trimmed.is_empty() {
                    if subtitle.is_some() {
                        state = 2;
                    }
                    continue;
                }
                if trimmed.starts_with("- ") {
                    features.push(trimmed[2..].to_string());
                    state = 3;
                } else if trimmed.starts_with('[') && trimmed.contains("](") {
                    parse_cta_link(trimmed, &mut cta_label, &mut cta_href);
                } else if subtitle.is_none() {
                    subtitle = Some(trimmed.to_string());
                } else {
                    body_lines.push(trimmed);
                    state = 2;
                }
            }
            2 => {
                if trimmed.starts_with("- ") {
                    features.push(trimmed[2..].to_string());
                    state = 3;
                } else if trimmed.starts_with('[') && trimmed.contains("](") {
                    parse_cta_link(trimmed, &mut cta_label, &mut cta_href);
                } else {
                    body_lines.push(trimmed);
                }
            }
            3 => {
                if trimmed.starts_with("- ") {
                    features.push(trimmed[2..].to_string());
                } else if trimmed.starts_with('[') && trimmed.contains("](") {
                    parse_cta_link(trimmed, &mut cta_label, &mut cta_href);
                }
            }
            _ => {}
        }
    }

    let body = body_lines.join("\n").trim().to_string();

    Block::ProductCard {
        title,
        subtitle,
        badge,
        badge_color,
        body,
        features,
        cta_label,
        cta_href,
        span,
    }
}

fn parse_cta_link(trimmed: &str, cta_label: &mut Option<String>, cta_href: &mut Option<String>) {
    if let Some(label_end) = trimmed.find("](") {
        let label = trimmed[1..label_end].to_string();
        let after = &trimmed[label_end + 2..];
        if let Some(href_end) = after.find(')') {
            *cta_label = Some(label);
            *cta_href = Some(after[..href_end].to_string());
        }
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttrValue;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    /// Helper: build a `Block::Unknown` for testing.
    fn unknown(name: &str, attrs: Attrs, content: &str) -> Block {
        Block::Unknown {
            name: name.to_string(),
            attrs,
            content: content.to_string(),
            span: Span {
                start_line: 1,
                end_line: 3,
                start_offset: 0,
                end_offset: 100,
            },
        }
    }

    /// Helper: quick attrs builder.
    fn attrs(pairs: &[(&str, AttrValue)]) -> Attrs {
        let mut map = BTreeMap::new();
        for (k, v) in pairs {
            map.insert(k.to_string(), v.clone());
        }
        map
    }

    // -- Callout ---------------------------------------------------

    #[test]
    fn resolve_callout_warning() {
        let block = unknown(
            "callout",
            attrs(&[("type", AttrValue::String("warning".into()))]),
            "Watch out!",
        );
        match resolve_block(block) {
            Block::Callout {
                callout_type,
                content,
                ..
            } => {
                assert_eq!(callout_type, CalloutType::Warning);
                assert_eq!(content, "Watch out!");
            }
            other => panic!("Expected Callout, got {other:?}"),
        }
    }

    #[test]
    fn resolve_callout_with_title() {
        let block = unknown(
            "callout",
            attrs(&[
                ("type", AttrValue::String("tip".into())),
                ("title", AttrValue::String("Pro Tip".into())),
            ]),
            "Use Rust.",
        );
        match resolve_block(block) {
            Block::Callout {
                callout_type,
                title,
                ..
            } => {
                assert_eq!(callout_type, CalloutType::Tip);
                assert_eq!(title, Some("Pro Tip".to_string()));
            }
            other => panic!("Expected Callout, got {other:?}"),
        }
    }

    #[test]
    fn resolve_callout_default_type() {
        let block = unknown("callout", Attrs::new(), "No type attr.");
        match resolve_block(block) {
            Block::Callout { callout_type, .. } => {
                assert_eq!(callout_type, CalloutType::Info);
            }
            other => panic!("Expected Callout, got {other:?}"),
        }
    }

    // -- Data ------------------------------------------------------

    #[test]
    fn resolve_data_table() {
        let content = "| Name | Age |\n|---|---|\n| Alice | 30 |\n| Bob | 25 |";
        let block = unknown("data", Attrs::new(), content);
        match resolve_block(block) {
            Block::Data {
                headers,
                rows,
                format,
                ..
            } => {
                assert_eq!(format, DataFormat::Table);
                assert_eq!(headers, vec!["Name", "Age"]);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0], vec!["Alice", "30"]);
                assert_eq!(rows[1], vec!["Bob", "25"]);
            }
            other => panic!("Expected Data, got {other:?}"),
        }
    }

    #[test]
    fn resolve_data_with_separator() {
        let content = "| H1 | H2 |\n| --- | --- |\n| v1 | v2 |";
        let block = unknown("data", Attrs::new(), content);
        match resolve_block(block) {
            Block::Data { headers, rows, .. } => {
                assert_eq!(headers, vec!["H1", "H2"]);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0], vec!["v1", "v2"]);
            }
            other => panic!("Expected Data, got {other:?}"),
        }
    }

    #[test]
    fn resolve_data_sortable() {
        let block = unknown(
            "data",
            attrs(&[("sortable", AttrValue::Bool(true))]),
            "| A |\n| 1 |",
        );
        match resolve_block(block) {
            Block::Data { sortable, .. } => {
                assert!(sortable);
            }
            other => panic!("Expected Data, got {other:?}"),
        }
    }

    #[test]
    fn resolve_data_csv() {
        let content = "Name, Age\nAlice, 30\nBob, 25";
        let block = unknown(
            "data",
            attrs(&[("format", AttrValue::String("csv".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Data {
                format,
                headers,
                rows,
                ..
            } => {
                assert_eq!(format, DataFormat::Csv);
                assert_eq!(headers, vec!["Name", "Age"]);
                assert_eq!(rows.len(), 2);
            }
            other => panic!("Expected Data, got {other:?}"),
        }
    }

    // -- Code ------------------------------------------------------

    #[test]
    fn resolve_code_with_lang() {
        let block = unknown(
            "code",
            attrs(&[("lang", AttrValue::String("rust".into()))]),
            "fn main() {}",
        );
        match resolve_block(block) {
            Block::Code { lang, content, .. } => {
                assert_eq!(lang, Some("rust".to_string()));
                assert_eq!(content, "fn main() {}");
            }
            other => panic!("Expected Code, got {other:?}"),
        }
    }

    #[test]
    fn resolve_code_with_file() {
        let block = unknown(
            "code",
            attrs(&[
                ("lang", AttrValue::String("rust".into())),
                ("file", AttrValue::String("main.rs".into())),
            ]),
            "fn main() {}",
        );
        match resolve_block(block) {
            Block::Code { lang, file, .. } => {
                assert_eq!(lang, Some("rust".to_string()));
                assert_eq!(file, Some("main.rs".to_string()));
            }
            other => panic!("Expected Code, got {other:?}"),
        }
    }

    // -- Tasks -----------------------------------------------------

    #[test]
    fn resolve_tasks_mixed() {
        let content = "- [ ] Write tests\n- [x] Write parser";
        let block = unknown("tasks", Attrs::new(), content);
        match resolve_block(block) {
            Block::Tasks { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(!items[0].done);
                assert_eq!(items[0].text, "Write tests");
                assert!(items[1].done);
                assert_eq!(items[1].text, "Write parser");
            }
            other => panic!("Expected Tasks, got {other:?}"),
        }
    }

    #[test]
    fn resolve_tasks_with_assignee() {
        let content = "- [ ] Fix bug @brady";
        let block = unknown("tasks", Attrs::new(), content);
        match resolve_block(block) {
            Block::Tasks { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].text, "Fix bug");
                assert_eq!(items[0].assignee, Some("brady".to_string()));
            }
            other => panic!("Expected Tasks, got {other:?}"),
        }
    }

    // -- Decision --------------------------------------------------

    #[test]
    fn resolve_decision_accepted() {
        let block = unknown(
            "decision",
            attrs(&[
                ("status", AttrValue::String("accepted".into())),
                ("date", AttrValue::String("2026-02-10".into())),
            ]),
            "We chose Rust.",
        );
        match resolve_block(block) {
            Block::Decision {
                status,
                date,
                content,
                ..
            } => {
                assert_eq!(status, DecisionStatus::Accepted);
                assert_eq!(date, Some("2026-02-10".to_string()));
                assert_eq!(content, "We chose Rust.");
            }
            other => panic!("Expected Decision, got {other:?}"),
        }
    }

    #[test]
    fn resolve_decision_with_deciders() {
        let block = unknown(
            "decision",
            attrs(&[
                ("status", AttrValue::String("proposed".into())),
                ("deciders", AttrValue::String("Brady, Claude".into())),
            ]),
            "Consider options.",
        );
        match resolve_block(block) {
            Block::Decision { deciders, .. } => {
                assert_eq!(deciders, vec!["Brady", "Claude"]);
            }
            other => panic!("Expected Decision, got {other:?}"),
        }
    }

    // -- Metric ----------------------------------------------------

    #[test]
    fn resolve_metric_basic() {
        let block = unknown(
            "metric",
            attrs(&[
                ("label", AttrValue::String("MRR".into())),
                ("value", AttrValue::String("$2K".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Metric { label, value, .. } => {
                assert_eq!(label, "MRR");
                assert_eq!(value, "$2K");
            }
            other => panic!("Expected Metric, got {other:?}"),
        }
    }

    #[test]
    fn resolve_metric_with_trend() {
        let block = unknown(
            "metric",
            attrs(&[
                ("label", AttrValue::String("Users".into())),
                ("value", AttrValue::String("500".into())),
                ("trend", AttrValue::String("up".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Metric { trend, .. } => {
                assert_eq!(trend, Some(Trend::Up));
            }
            other => panic!("Expected Metric, got {other:?}"),
        }
    }

    // -- Summary ---------------------------------------------------

    #[test]
    fn resolve_summary() {
        let block = unknown("summary", Attrs::new(), "This is the executive summary.");
        match resolve_block(block) {
            Block::Summary { content, .. } => {
                assert_eq!(content, "This is the executive summary.");
            }
            other => panic!("Expected Summary, got {other:?}"),
        }
    }

    // -- Figure ----------------------------------------------------

    #[test]
    fn resolve_figure_basic() {
        let block = unknown(
            "figure",
            attrs(&[
                ("src", AttrValue::String("img.png".into())),
                ("caption", AttrValue::String("Photo".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Figure {
                src,
                caption,
                alt,
                width,
                ..
            } => {
                assert_eq!(src, "img.png");
                assert_eq!(caption, Some("Photo".to_string()));
                assert!(alt.is_none());
                assert!(width.is_none());
            }
            other => panic!("Expected Figure, got {other:?}"),
        }
    }

    // -- Tabs ------------------------------------------------------

    #[test]
    fn resolve_tabs_with_headers() {
        let content = "## Overview\nIntro text.\n\n## Details\nTechnical info.\n\n## FAQ\nQ&A here.";
        let block = unknown("tabs", Attrs::new(), content);
        match resolve_block(block) {
            Block::Tabs { tabs, .. } => {
                assert_eq!(tabs.len(), 3);
                assert_eq!(tabs[0].label, "Overview");
                assert!(tabs[0].content.contains("Intro text."));
                assert_eq!(tabs[1].label, "Details");
                assert!(tabs[1].content.contains("Technical info."));
                assert_eq!(tabs[2].label, "FAQ");
                assert!(tabs[2].content.contains("Q&A here."));
            }
            other => panic!("Expected Tabs, got {other:?}"),
        }
    }

    #[test]
    fn resolve_tabs_single_no_header() {
        let content = "Just some text without any tab headers.";
        let block = unknown("tabs", Attrs::new(), content);
        match resolve_block(block) {
            Block::Tabs { tabs, .. } => {
                assert_eq!(tabs.len(), 1);
                assert_eq!(tabs[0].label, "Tab 1");
                assert!(tabs[0].content.contains("Just some text"));
            }
            other => panic!("Expected Tabs, got {other:?}"),
        }
    }

    // -- Columns ---------------------------------------------------

    #[test]
    fn resolve_columns_with_nested_directives() {
        let content = ":::column\nLeft content.\n:::\n:::column\nRight content.\n:::";
        let block = unknown("columns", Attrs::new(), content);
        match resolve_block(block) {
            Block::Columns { columns, .. } => {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].content, "Left content.");
                assert_eq!(columns[1].content, "Right content.");
            }
            other => panic!("Expected Columns, got {other:?}"),
        }
    }

    #[test]
    fn resolve_columns_with_hr_separator() {
        let content = "Left side.\n---\nRight side.";
        let block = unknown("columns", Attrs::new(), content);
        match resolve_block(block) {
            Block::Columns { columns, .. } => {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].content, "Left side.");
                assert_eq!(columns[1].content, "Right side.");
            }
            other => panic!("Expected Columns, got {other:?}"),
        }
    }

    #[test]
    fn resolve_columns_single() {
        let content = "All in one column.";
        let block = unknown("columns", Attrs::new(), content);
        match resolve_block(block) {
            Block::Columns { columns, .. } => {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].content, "All in one column.");
            }
            other => panic!("Expected Columns, got {other:?}"),
        }
    }

    // -- Quote -----------------------------------------------------

    #[test]
    fn resolve_quote_with_attribution() {
        let block = unknown(
            "quote",
            attrs(&[
                ("by", AttrValue::String("Alan Kay".into())),
                ("cite", AttrValue::String("ACM 1971".into())),
            ]),
            "The best way to predict the future is to invent it.",
        );
        match resolve_block(block) {
            Block::Quote {
                content,
                attribution,
                cite,
                ..
            } => {
                assert_eq!(content, "The best way to predict the future is to invent it.");
                assert_eq!(attribution, Some("Alan Kay".to_string()));
                assert_eq!(cite, Some("ACM 1971".to_string()));
            }
            other => panic!("Expected Quote, got {other:?}"),
        }
    }

    #[test]
    fn resolve_quote_no_attribution() {
        let block = unknown("quote", Attrs::new(), "Anonymous wisdom.");
        match resolve_block(block) {
            Block::Quote {
                content,
                attribution,
                ..
            } => {
                assert_eq!(content, "Anonymous wisdom.");
                assert!(attribution.is_none());
            }
            other => panic!("Expected Quote, got {other:?}"),
        }
    }

    #[test]
    fn resolve_quote_author_alias() {
        let block = unknown(
            "quote",
            attrs(&[("author", AttrValue::String("Knuth".into()))]),
            "Premature optimization.",
        );
        match resolve_block(block) {
            Block::Quote { attribution, .. } => {
                assert_eq!(attribution, Some("Knuth".to_string()));
            }
            other => panic!("Expected Quote, got {other:?}"),
        }
    }

    // -- Cta -------------------------------------------------------

    #[test]
    fn resolve_cta_primary() {
        let block = unknown(
            "cta",
            attrs(&[
                ("label", AttrValue::String("Get Started".into())),
                ("href", AttrValue::String("/signup".into())),
                ("primary", AttrValue::Bool(true)),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Cta {
                label,
                href,
                primary,
                ..
            } => {
                assert_eq!(label, "Get Started");
                assert_eq!(href, "/signup");
                assert!(primary);
            }
            other => panic!("Expected Cta, got {other:?}"),
        }
    }

    #[test]
    fn resolve_cta_secondary() {
        let block = unknown(
            "cta",
            attrs(&[
                ("label", AttrValue::String("Learn More".into())),
                ("href", AttrValue::String("https://example.com".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Cta {
                label,
                href,
                primary,
                ..
            } => {
                assert_eq!(label, "Learn More");
                assert_eq!(href, "https://example.com");
                assert!(!primary);
            }
            other => panic!("Expected Cta, got {other:?}"),
        }
    }

    // -- HeroImage -------------------------------------------------

    #[test]
    fn resolve_hero_image_with_alt() {
        let block = unknown(
            "hero-image",
            attrs(&[
                ("src", AttrValue::String("hero.png".into())),
                ("alt", AttrValue::String("Product screenshot".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::HeroImage { src, alt, .. } => {
                assert_eq!(src, "hero.png");
                assert_eq!(alt, Some("Product screenshot".to_string()));
            }
            other => panic!("Expected HeroImage, got {other:?}"),
        }
    }

    #[test]
    fn resolve_hero_image_no_alt() {
        let block = unknown(
            "hero-image",
            attrs(&[("src", AttrValue::String("banner.jpg".into()))]),
            "",
        );
        match resolve_block(block) {
            Block::HeroImage { src, alt, .. } => {
                assert_eq!(src, "banner.jpg");
                assert!(alt.is_none());
            }
            other => panic!("Expected HeroImage, got {other:?}"),
        }
    }

    // -- Testimonial -----------------------------------------------

    #[test]
    fn resolve_testimonial_full() {
        let block = unknown(
            "testimonial",
            attrs(&[
                ("author", AttrValue::String("Jane Dev".into())),
                ("role", AttrValue::String("Engineer".into())),
                ("company", AttrValue::String("Acme".into())),
            ]),
            "This tool replaced 3 others for me.",
        );
        match resolve_block(block) {
            Block::Testimonial {
                content,
                author,
                role,
                company,
                ..
            } => {
                assert_eq!(content, "This tool replaced 3 others for me.");
                assert_eq!(author, Some("Jane Dev".to_string()));
                assert_eq!(role, Some("Engineer".to_string()));
                assert_eq!(company, Some("Acme".to_string()));
            }
            other => panic!("Expected Testimonial, got {other:?}"),
        }
    }

    #[test]
    fn resolve_testimonial_name_alias() {
        let block = unknown(
            "testimonial",
            attrs(&[("name", AttrValue::String("Bob".into()))]),
            "Great product.",
        );
        match resolve_block(block) {
            Block::Testimonial { author, .. } => {
                assert_eq!(author, Some("Bob".to_string()));
            }
            other => panic!("Expected Testimonial, got {other:?}"),
        }
    }

    #[test]
    fn resolve_testimonial_anonymous() {
        let block = unknown("testimonial", Attrs::new(), "Anonymous feedback.");
        match resolve_block(block) {
            Block::Testimonial {
                content,
                author,
                role,
                company,
                ..
            } => {
                assert_eq!(content, "Anonymous feedback.");
                assert!(author.is_none());
                assert!(role.is_none());
                assert!(company.is_none());
            }
            other => panic!("Expected Testimonial, got {other:?}"),
        }
    }

    // -- Style -----------------------------------------------------

    #[test]
    fn resolve_style_properties() {
        let content = "hero-bg: gradient indigo\ncard-radius: lg\nmax-width: 1200px";
        let block = unknown("style", Attrs::new(), content);
        match resolve_block(block) {
            Block::Style { properties, .. } => {
                assert_eq!(properties.len(), 3);
                assert_eq!(properties[0].key, "hero-bg");
                assert_eq!(properties[0].value, "gradient indigo");
                assert_eq!(properties[1].key, "card-radius");
                assert_eq!(properties[1].value, "lg");
                assert_eq!(properties[2].key, "max-width");
                assert_eq!(properties[2].value, "1200px");
            }
            other => panic!("Expected Style, got {other:?}"),
        }
    }

    #[test]
    fn resolve_style_empty() {
        let block = unknown("style", Attrs::new(), "");
        match resolve_block(block) {
            Block::Style { properties, .. } => {
                assert!(properties.is_empty());
            }
            other => panic!("Expected Style, got {other:?}"),
        }
    }

    #[test]
    fn resolve_style_skips_blank_lines() {
        let content = "  \nfont: inter\n\naccent: #6366f1\n  ";
        let block = unknown("style", Attrs::new(), content);
        match resolve_block(block) {
            Block::Style { properties, .. } => {
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[0].key, "font");
                assert_eq!(properties[0].value, "inter");
                assert_eq!(properties[1].key, "accent");
                assert_eq!(properties[1].value, "#6366f1");
            }
            other => panic!("Expected Style, got {other:?}"),
        }
    }

    // -- Faq -------------------------------------------------------

    #[test]
    fn resolve_faq_multiple_items() {
        let content = "### Is my data encrypted?\nYes — AES-256 at rest, TLS in transit.\n\n### Can I self-host?\nYes. Docker image available.";
        let block = unknown("faq", Attrs::new(), content);
        match resolve_block(block) {
            Block::Faq { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].question, "Is my data encrypted?");
                assert!(items[0].answer.contains("AES-256"));
                assert_eq!(items[1].question, "Can I self-host?");
                assert!(items[1].answer.contains("Docker"));
            }
            other => panic!("Expected Faq, got {other:?}"),
        }
    }

    #[test]
    fn resolve_faq_h2_headers() {
        let content = "## Question one\nAnswer one.\n\n## Question two\nAnswer two.";
        let block = unknown("faq", Attrs::new(), content);
        match resolve_block(block) {
            Block::Faq { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].question, "Question one");
                assert_eq!(items[1].question, "Question two");
            }
            other => panic!("Expected Faq, got {other:?}"),
        }
    }

    #[test]
    fn resolve_faq_empty() {
        let block = unknown("faq", Attrs::new(), "");
        match resolve_block(block) {
            Block::Faq { items, .. } => {
                assert!(items.is_empty());
            }
            other => panic!("Expected Faq, got {other:?}"),
        }
    }

    #[test]
    fn resolve_faq_single_item() {
        let content = "### How does pricing work?\nWe charge per seat per month.";
        let block = unknown("faq", Attrs::new(), content);
        match resolve_block(block) {
            Block::Faq { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].question, "How does pricing work?");
                assert_eq!(items[0].answer, "We charge per seat per month.");
            }
            other => panic!("Expected Faq, got {other:?}"),
        }
    }

    // -- PricingTable ----------------------------------------------

    #[test]
    fn resolve_pricing_table() {
        let content = "| | Free | Pro | Team |\n|---|---|---|---|\n| Price | $0 | $4.99/mo | $8.99/seat/mo |\n| Notes | Unlimited | Unlimited | Unlimited |";
        let block = unknown("pricing-table", Attrs::new(), content);
        match resolve_block(block) {
            Block::PricingTable {
                headers, rows, ..
            } => {
                assert_eq!(headers, vec!["", "Free", "Pro", "Team"]);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0][0], "Price");
                assert_eq!(rows[0][2], "$4.99/mo");
                assert_eq!(rows[1][3], "Unlimited");
            }
            other => panic!("Expected PricingTable, got {other:?}"),
        }
    }

    #[test]
    fn resolve_pricing_table_empty() {
        let block = unknown("pricing-table", Attrs::new(), "");
        match resolve_block(block) {
            Block::PricingTable {
                headers, rows, ..
            } => {
                assert!(headers.is_empty());
                assert!(rows.is_empty());
            }
            other => panic!("Expected PricingTable, got {other:?}"),
        }
    }

    // -- Site ------------------------------------------------------

    #[test]
    fn resolve_site_with_domain() {
        let block = unknown(
            "site",
            attrs(&[("domain", AttrValue::String("notesurf.io".into()))]),
            "name: NoteSurf\ntagline: Notes that belong to you.\ntheme: dark\naccent: #6366f1",
        );
        match resolve_block(block) {
            Block::Site {
                domain,
                properties,
                ..
            } => {
                assert_eq!(domain, Some("notesurf.io".to_string()));
                assert_eq!(properties.len(), 4);
                assert_eq!(properties[0].key, "name");
                assert_eq!(properties[0].value, "NoteSurf");
                assert_eq!(properties[1].key, "tagline");
                assert_eq!(properties[1].value, "Notes that belong to you.");
                assert_eq!(properties[2].key, "theme");
                assert_eq!(properties[2].value, "dark");
            }
            other => panic!("Expected Site, got {other:?}"),
        }
    }

    #[test]
    fn resolve_site_no_domain() {
        let block = unknown("site", Attrs::new(), "name: Test Site");
        match resolve_block(block) {
            Block::Site {
                domain,
                properties,
                ..
            } => {
                assert!(domain.is_none());
                assert_eq!(properties.len(), 1);
            }
            other => panic!("Expected Site, got {other:?}"),
        }
    }

    // -- Page ------------------------------------------------------

    #[test]
    fn resolve_page_basic() {
        let block = unknown(
            "page",
            attrs(&[
                ("route", AttrValue::String("/".into())),
                ("layout", AttrValue::String("hero".into())),
            ]),
            "# Welcome\n\nSome intro text.",
        );
        match resolve_block(block) {
            Block::Page {
                route,
                layout,
                children,
                ..
            } => {
                assert_eq!(route, "/");
                assert_eq!(layout, Some("hero".to_string()));
                // All content is markdown (no leaf directives)
                assert_eq!(children.len(), 1);
                assert!(matches!(&children[0], Block::Markdown { .. }));
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn resolve_page_with_nested_cta() {
        let content = "# Take notes anywhere.\n\nIntro paragraph.\n\n::cta[label=\"Download\" href=\"/download\" primary]\n::cta[label=\"Try Web\" href=\"https://app.example.com\"]";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                // Should be: Markdown, Cta (primary), Cta (secondary)
                assert_eq!(children.len(), 3, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                match &children[1] {
                    Block::Cta {
                        label, primary, ..
                    } => {
                        assert_eq!(label, "Download");
                        assert!(*primary);
                    }
                    other => panic!("Expected Cta, got {other:?}"),
                }
                match &children[2] {
                    Block::Cta {
                        label, primary, ..
                    } => {
                        assert_eq!(label, "Try Web");
                        assert!(!*primary);
                    }
                    other => panic!("Expected Cta, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn resolve_page_with_mixed_children() {
        let content = "# Hero Title\n\n::hero-image[src=\"hero.png\" alt=\"Screenshot\"]\n\nMore text below.\n\n::cta[label=\"Sign Up\" href=\"/signup\" primary]";
        let block = unknown(
            "page",
            attrs(&[
                ("route", AttrValue::String("/".into())),
                ("layout", AttrValue::String("hero".into())),
            ]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                // Markdown, HeroImage, Markdown, Cta
                assert_eq!(children.len(), 4, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                assert!(matches!(&children[1], Block::HeroImage { .. }));
                assert!(matches!(&children[2], Block::Markdown { .. }));
                assert!(matches!(&children[3], Block::Cta { .. }));
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn resolve_page_empty() {
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/about".into()))]),
            "",
        );
        match resolve_block(block) {
            Block::Page {
                route, children, ..
            } => {
                assert_eq!(route, "/about");
                assert!(children.is_empty());
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    // -- Passthrough -----------------------------------------------

    #[test]
    fn resolve_unknown_passthrough() {
        let block = unknown("custom_block", Attrs::new(), "whatever");
        match resolve_block(block) {
            Block::Unknown { name, .. } => {
                assert_eq!(name, "custom_block");
            }
            other => panic!("Expected Unknown passthrough, got {other:?}"),
        }
    }

    // -- Container blocks inside Page ---------------------------------

    #[test]
    fn page_container_pricing_table() {
        let content = "# Menu\n\n::pricing-table\n| Item | Price |\n|------|-------|\n| Coffee | $4 |\n| Muffin | $3 |\n::\n\nVisit us today!";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 3, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                match &children[1] {
                    Block::PricingTable { headers, rows, .. } => {
                        assert_eq!(headers, &["Item", "Price"]);
                        assert_eq!(rows.len(), 2);
                        assert_eq!(rows[0], vec!["Coffee", "$4"]);
                        assert_eq!(rows[1], vec!["Muffin", "$3"]);
                    }
                    other => panic!("Expected PricingTable, got {other:?}"),
                }
                assert!(matches!(&children[2], Block::Markdown { .. }));
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_callout() {
        let content = "::callout[type=warning]\nWatch out for hot drinks!\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Callout { callout_type, content, .. } => {
                        assert_eq!(*callout_type, CalloutType::Warning);
                        assert_eq!(content, "Watch out for hot drinks!");
                    }
                    other => panic!("Expected Callout, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_faq() {
        let content = "::faq\n### What are your hours?\nMon-Fri 7am-6pm.\n### Do you deliver?\nYes, within 5 miles.\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Faq { items, .. } => {
                        assert_eq!(items.len(), 2);
                        assert_eq!(items[0].question, "What are your hours?");
                        assert!(items[0].answer.contains("7am-6pm"));
                        assert_eq!(items[1].question, "Do you deliver?");
                    }
                    other => panic!("Expected Faq, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_data() {
        let content = "::data\n| Name | Value |\n|------|-------|\n| Alpha | 100 |\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Data { headers, rows, .. } => {
                        assert_eq!(headers, &["Name", "Value"]);
                        assert_eq!(rows.len(), 1);
                        assert_eq!(rows[0], vec!["Alpha", "100"]);
                    }
                    other => panic!("Expected Data, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_testimonial() {
        let content = "::testimonial[author=\"Jane\" role=\"Regular\"]\nBest bakery in town!\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Testimonial { content, author, role, .. } => {
                        assert_eq!(content, "Best bakery in town!");
                        assert_eq!(author.as_deref(), Some("Jane"));
                        assert_eq!(role.as_deref(), Some("Regular"));
                    }
                    other => panic!("Expected Testimonial, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_columns_with_nesting() {
        let content = "::columns\n:::column\nLeft side.\n:::\n:::column\nRight side.\n:::\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Columns { columns, .. } => {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0].content, "Left side.");
                        assert_eq!(columns[1].content, "Right side.");
                    }
                    other => panic!("Expected Columns, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_mixed_leaf_and_container_preserves_order() {
        let content = "# Welcome\n\n::hero-image[src=\"hero.png\"]\n\n::callout[type=tip]\nPro tip: order early!\n::\n\n::cta[label=\"Order Now\" href=\"/order\" primary]\n\n::faq\n### How to order?\nOnline or in store.\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                // Markdown, HeroImage, Callout, Cta, Faq
                assert_eq!(children.len(), 5, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                assert!(matches!(&children[1], Block::HeroImage { .. }));
                assert!(matches!(&children[2], Block::Callout { .. }));
                assert!(matches!(&children[3], Block::Cta { .. }));
                assert!(matches!(&children[4], Block::Faq { .. }));
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_bakery_example_no_leaked_markers() {
        // Simulate a typical AI-generated bakery site
        let content = r#"# Fresh Baked Daily

Welcome to Sunrise Bakery! We bake fresh bread, pastries, and cakes every morning.

::hero-image[src="/images/bakery.jpg" alt="Fresh bread"]

::pricing-table
| Item | Price |
|------|-------|
| Sourdough Loaf | $6 |
| Croissant | $3.50 |
| Birthday Cake | $35 |
::

::testimonial[author="Sarah M." role="Regular Customer"]
The best sourdough in the city. I come here every weekend!
::

::faq
### Do you take custom orders?
Yes! Place custom cake orders at least 48 hours in advance.
### Are you open on weekends?
Saturday 7am-4pm, Sunday 8am-2pm.
::

::cta[label="Order Online" href="/order" primary]"#;

        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                // Markdown, HeroImage, PricingTable, Testimonial, Faq, Cta
                assert_eq!(children.len(), 6, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                assert!(matches!(&children[1], Block::HeroImage { .. }));
                assert!(matches!(&children[2], Block::PricingTable { .. }));
                assert!(matches!(&children[3], Block::Testimonial { .. }));
                assert!(matches!(&children[4], Block::Faq { .. }));
                assert!(matches!(&children[5], Block::Cta { .. }));

                // Verify no :: leaked into any markdown block
                for child in &children {
                    if let Block::Markdown { content, .. } = child {
                        assert!(
                            !content.contains("\n::") && !content.starts_with("::"),
                            "Leaked :: markers in markdown: {content}"
                        );
                    }
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    // -- Embed -------------------------------------------------

    #[test]
    fn resolve_embed_basic() {
        let block = unknown(
            "embed",
            attrs(&[("src", AttrValue::String("https://www.google.com/maps/embed?pb=123".into()))]),
            "",
        );
        match resolve_block(block) {
            Block::Embed {
                src, embed_type, width, height, ..
            } => {
                assert!(src.contains("google.com/maps"));
                assert_eq!(embed_type, Some(crate::types::EmbedType::Map));
                assert!(width.is_none());
                assert!(height.is_none());
            }
            other => panic!("Expected Embed, got {other:?}"),
        }
    }

    #[test]
    fn resolve_embed_youtube_auto_detect() {
        let block = unknown(
            "embed",
            attrs(&[("src", AttrValue::String("https://www.youtube.com/embed/abc123".into()))]),
            "",
        );
        match resolve_block(block) {
            Block::Embed { embed_type, .. } => {
                assert_eq!(embed_type, Some(crate::types::EmbedType::Video));
            }
            other => panic!("Expected Embed, got {other:?}"),
        }
    }

    #[test]
    fn resolve_embed_explicit_type() {
        let block = unknown(
            "embed",
            attrs(&[
                ("src", AttrValue::String("https://example.com/widget".into())),
                ("type", AttrValue::String("generic".into())),
                ("width", AttrValue::String("600".into())),
                ("height", AttrValue::String("300".into())),
                ("title", AttrValue::String("My Widget".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Embed {
                src, embed_type, width, height, title, ..
            } => {
                assert_eq!(src, "https://example.com/widget");
                assert_eq!(embed_type, Some(crate::types::EmbedType::Generic));
                assert_eq!(width, Some("600".to_string()));
                assert_eq!(height, Some("300".to_string()));
                assert_eq!(title, Some("My Widget".to_string()));
            }
            other => panic!("Expected Embed, got {other:?}"),
        }
    }

    // -- Form --------------------------------------------------

    #[test]
    fn resolve_form_basic_fields() {
        let content = "- Name (text) *\n- Email (email) *\n- Message (textarea)";
        let block = unknown("form", Attrs::new(), content);
        match resolve_block(block) {
            Block::Form { fields, submit_label, .. } => {
                assert_eq!(fields.len(), 3);
                assert_eq!(fields[0].label, "Name");
                assert_eq!(fields[0].field_type, crate::types::FormFieldType::Text);
                assert!(fields[0].required);
                assert_eq!(fields[1].label, "Email");
                assert_eq!(fields[1].field_type, crate::types::FormFieldType::Email);
                assert!(fields[1].required);
                assert_eq!(fields[2].label, "Message");
                assert_eq!(fields[2].field_type, crate::types::FormFieldType::Textarea);
                assert!(!fields[2].required);
                assert!(submit_label.is_none());
            }
            other => panic!("Expected Form, got {other:?}"),
        }
    }

    #[test]
    fn resolve_form_with_select() {
        let content = "- Size (select: Small | Medium | Large) *";
        let block = unknown("form", Attrs::new(), content);
        match resolve_block(block) {
            Block::Form { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].label, "Size");
                assert_eq!(fields[0].field_type, crate::types::FormFieldType::Select);
                assert_eq!(fields[0].options, vec!["Small", "Medium", "Large"]);
                assert!(fields[0].required);
            }
            other => panic!("Expected Form, got {other:?}"),
        }
    }

    #[test]
    fn resolve_form_with_submit_label() {
        let block = unknown(
            "form",
            attrs(&[("submit", AttrValue::String("Request Quote".into()))]),
            "- Name (text) *",
        );
        match resolve_block(block) {
            Block::Form { submit_label, .. } => {
                assert_eq!(submit_label, Some("Request Quote".to_string()));
            }
            other => panic!("Expected Form, got {other:?}"),
        }
    }

    #[test]
    fn resolve_form_measurement_fields() {
        let content = "- Full Name (text) *\n- Email (email) *\n- Phone (tel)\n- Event Date (date) *\n- Height (number)\n- Notes (textarea)";
        let block = unknown("form", Attrs::new(), content);
        match resolve_block(block) {
            Block::Form { fields, .. } => {
                assert_eq!(fields.len(), 6);
                assert_eq!(fields[0].field_type, crate::types::FormFieldType::Text);
                assert_eq!(fields[1].field_type, crate::types::FormFieldType::Email);
                assert_eq!(fields[2].field_type, crate::types::FormFieldType::Tel);
                assert_eq!(fields[3].field_type, crate::types::FormFieldType::Date);
                assert_eq!(fields[4].field_type, crate::types::FormFieldType::Number);
                assert_eq!(fields[5].field_type, crate::types::FormFieldType::Textarea);
                assert_eq!(fields[0].name, "full_name");
                assert_eq!(fields[3].name, "event_date");
            }
            other => panic!("Expected Form, got {other:?}"),
        }
    }

    // -- Gallery -----------------------------------------------

    #[test]
    fn resolve_gallery_basic() {
        let content = "![Suit](suit.jpg) Suits: Classic black tuxedo\n![Vest](vest.jpg) Accessories: Silk vest";
        let block = unknown("gallery", Attrs::new(), content);
        match resolve_block(block) {
            Block::Gallery { items, columns, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].src, "suit.jpg");
                assert_eq!(items[0].alt, Some("Suit".to_string()));
                assert_eq!(items[0].category, Some("Suits".to_string()));
                assert_eq!(items[0].caption, Some("Classic black tuxedo".to_string()));
                assert_eq!(items[1].src, "vest.jpg");
                assert_eq!(items[1].category, Some("Accessories".to_string()));
                assert_eq!(columns, Some(2));
            }
            other => panic!("Expected Gallery, got {other:?}"),
        }
    }

    #[test]
    fn resolve_gallery_no_categories() {
        let content = "![](a.jpg) Photo one\n![](b.jpg) Photo two\n![](c.jpg) Photo three";
        let block = unknown("gallery", Attrs::new(), content);
        match resolve_block(block) {
            Block::Gallery { items, columns, .. } => {
                assert_eq!(items.len(), 3);
                assert!(items[0].category.is_none());
                assert_eq!(items[0].caption, Some("Photo one".to_string()));
                assert_eq!(columns, Some(3));
            }
            other => panic!("Expected Gallery, got {other:?}"),
        }
    }

    #[test]
    fn resolve_gallery_empty() {
        let block = unknown("gallery", Attrs::new(), "");
        match resolve_block(block) {
            Block::Gallery { items, .. } => {
                assert!(items.is_empty());
            }
            other => panic!("Expected Gallery, got {other:?}"),
        }
    }

    #[test]
    fn resolve_gallery_many_items() {
        let content = "![](1.jpg)\n![](2.jpg)\n![](3.jpg)\n![](4.jpg)\n![](5.jpg)\n![](6.jpg)\n![](7.jpg)";
        let block = unknown("gallery", Attrs::new(), content);
        match resolve_block(block) {
            Block::Gallery { items, columns, .. } => {
                assert_eq!(items.len(), 7);
                assert_eq!(columns, Some(4));
            }
            other => panic!("Expected Gallery, got {other:?}"),
        }
    }

    // -- Footer ------------------------------------------------

    #[test]
    fn resolve_footer_full() {
        let content = "## Company\n- [About](/about)\n- [Contact](/contact)\n\n## Legal\n- [Privacy](/privacy)\n- [Terms](/terms)\n\n@instagram https://instagram.com/bowties\n@facebook https://facebook.com/bowties\n\n(c) 2026 Bowties Tuxedo. All rights reserved.";
        let block = unknown("footer", Attrs::new(), content);
        match resolve_block(block) {
            Block::Footer {
                sections, copyright, social, ..
            } => {
                assert_eq!(sections.len(), 2);
                assert_eq!(sections[0].heading, "Company");
                assert_eq!(sections[0].links.len(), 2);
                assert_eq!(sections[0].links[0].label, "About");
                assert_eq!(sections[0].links[0].href, "/about");
                assert_eq!(sections[1].heading, "Legal");
                assert_eq!(sections[1].links.len(), 2);
                assert_eq!(social.len(), 2);
                assert_eq!(social[0].platform, "instagram");
                assert_eq!(social[1].platform, "facebook");
                assert!(copyright.unwrap().contains("2026 Bowties Tuxedo"));
            }
            other => panic!("Expected Footer, got {other:?}"),
        }
    }

    #[test]
    fn resolve_footer_copyright_only() {
        let content = "\u{00a9} 2026 CloudSurf Software LLC";
        let block = unknown("footer", Attrs::new(), content);
        match resolve_block(block) {
            Block::Footer {
                sections, copyright, social, ..
            } => {
                assert!(sections.is_empty());
                assert!(social.is_empty());
                assert!(copyright.unwrap().contains("2026 CloudSurf"));
            }
            other => panic!("Expected Footer, got {other:?}"),
        }
    }

    #[test]
    fn resolve_footer_empty() {
        let block = unknown("footer", Attrs::new(), "");
        match resolve_block(block) {
            Block::Footer {
                sections, copyright, social, ..
            } => {
                assert!(sections.is_empty());
                assert!(copyright.is_none());
                assert!(social.is_empty());
            }
            other => panic!("Expected Footer, got {other:?}"),
        }
    }

    #[test]
    fn resolve_footer_plain_text_links() {
        let content = "## Hours\n- Mon-Fri 9am-5pm\n- Sat 10am-2pm";
        let block = unknown("footer", Attrs::new(), content);
        match resolve_block(block) {
            Block::Footer { sections, .. } => {
                assert_eq!(sections.len(), 1);
                assert_eq!(sections[0].heading, "Hours");
                assert_eq!(sections[0].links.len(), 2);
                assert_eq!(sections[0].links[0].label, "Mon-Fri 9am-5pm");
                assert!(sections[0].links[0].href.is_empty());
            }
            other => panic!("Expected Footer, got {other:?}"),
        }
    }

    // -- New blocks inside Page --------------------------------

    #[test]
    fn page_container_embed() {
        let content = "::embed[src=\"https://www.google.com/maps/embed?pb=123\"]\n\nVisit us!";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 2, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Embed { .. }));
                assert!(matches!(&children[1], Block::Markdown { .. }));
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_form() {
        let content = "# Request Measurement\n\n::form[submit=\"Book Appointment\"]\n- Name (text) *\n- Phone (tel) *\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 2, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                match &children[1] {
                    Block::Form { fields, submit_label, .. } => {
                        assert_eq!(fields.len(), 2);
                        assert_eq!(submit_label.as_deref(), Some("Book Appointment"));
                    }
                    other => panic!("Expected Form, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_gallery() {
        let content = "::gallery\n![Suit](suit.jpg) Suits: Classic\n![Vest](vest.jpg) Accessories: Silk\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Gallery { items, .. } => {
                        assert_eq!(items.len(), 2);
                    }
                    other => panic!("Expected Gallery, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_footer() {
        let content = "# Welcome\n\n::footer\n## Links\n- [Home](/)\n\n(c) 2026 Test\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 2, "children: {children:#?}");
                assert!(matches!(&children[0], Block::Markdown { .. }));
                match &children[1] {
                    Block::Footer { sections, copyright, .. } => {
                        assert_eq!(sections.len(), 1);
                        assert!(copyright.as_ref().unwrap().contains("2026"));
                    }
                    other => panic!("Expected Footer, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    // -- Hero --------------------------------------------------

    #[test]
    fn resolve_hero_basic() {
        let content = "# Build Something Great\n\nThe fastest way to ship your next project.\n\n[Get Started](/signup){primary}\n[Learn More](/docs)";
        let block = unknown("hero", Attrs::new(), content);
        match resolve_block(block) {
            Block::Hero {
                headline,
                subtitle,
                buttons,
                align,
                badge,
                image,
                ..
            } => {
                assert_eq!(headline, Some("Build Something Great".to_string()));
                assert_eq!(
                    subtitle,
                    Some("The fastest way to ship your next project.".to_string())
                );
                assert_eq!(buttons.len(), 2);
                assert_eq!(buttons[0].label, "Get Started");
                assert_eq!(buttons[0].href, "/signup");
                assert!(buttons[0].primary);
                assert_eq!(buttons[1].label, "Learn More");
                assert_eq!(buttons[1].href, "/docs");
                assert!(!buttons[1].primary);
                assert_eq!(align, "center");
                assert!(badge.is_none());
                assert!(image.is_none());
            }
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    #[test]
    fn resolve_hero_with_badge_and_image() {
        let content = "# Welcome Home\n\nYour new favorite tool.\n\n[Try Free](/free){primary}";
        let block = unknown(
            "hero",
            attrs(&[
                ("badge", AttrValue::String("New".into())),
                ("image", AttrValue::String("hero.png".into())),
                ("align", AttrValue::String("left".into())),
            ]),
            content,
        );
        match resolve_block(block) {
            Block::Hero {
                headline,
                badge,
                image,
                align,
                buttons,
                ..
            } => {
                assert_eq!(headline, Some("Welcome Home".to_string()));
                assert_eq!(badge, Some("New".to_string()));
                assert_eq!(image, Some("hero.png".to_string()));
                assert_eq!(align, "left");
                assert_eq!(buttons.len(), 1);
                assert!(buttons[0].primary);
            }
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    // -- Features ----------------------------------------------

    #[test]
    fn resolve_features_basic() {
        let content = "### Fast {icon=zap}\n\nBlazingly fast builds.\n\n### Secure {icon=lock}\n\nEnd-to-end encryption.\n\n### Simple {icon=star}\n\nNo config needed.";
        let block = unknown("features", Attrs::new(), content);
        match resolve_block(block) {
            Block::Features { cards, cols, .. } => {
                assert_eq!(cards.len(), 3);
                assert_eq!(cards[0].title, "Fast");
                assert_eq!(cards[0].icon, Some("zap".to_string()));
                assert!(cards[0].body.contains("Blazingly fast"));
                assert_eq!(cards[1].title, "Secure");
                assert_eq!(cards[1].icon, Some("lock".to_string()));
                assert_eq!(cards[2].title, "Simple");
                assert_eq!(cards[2].icon, Some("star".to_string()));
                assert!(cols.is_none());
            }
            other => panic!("Expected Features, got {other:?}"),
        }
    }

    #[test]
    fn resolve_features_with_link() {
        let content = "### Docs\n\nComprehensive documentation.\n\n[Read Docs](/docs)";
        let block = unknown(
            "features",
            attrs(&[("cols", AttrValue::String("2".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Features { cards, cols, .. } => {
                assert_eq!(cards.len(), 1);
                assert_eq!(cards[0].title, "Docs");
                assert_eq!(cards[0].link_label, Some("Read Docs".to_string()));
                assert_eq!(cards[0].link_href, Some("/docs".to_string()));
                assert!(!cards[0].body.contains("[Read Docs]"));
                assert_eq!(cols, Some(2));
            }
            other => panic!("Expected Features, got {other:?}"),
        }
    }

    // -- Steps -------------------------------------------------

    #[test]
    fn resolve_steps_basic() {
        let content = "### Sign Up {time=\"1 min\"}\n\nCreate your account.\n\n### Configure\n\nSet your preferences.\n\n### Deploy {time=\"5 min\"}\n\nShip to production.";
        let block = unknown("steps", Attrs::new(), content);
        match resolve_block(block) {
            Block::Steps { steps, .. } => {
                assert_eq!(steps.len(), 3);
                assert_eq!(steps[0].title, "Sign Up");
                assert_eq!(steps[0].time, Some("1 min".to_string()));
                assert!(steps[0].body.contains("Create your account"));
                assert_eq!(steps[1].title, "Configure");
                assert!(steps[1].time.is_none());
                assert_eq!(steps[2].title, "Deploy");
                assert_eq!(steps[2].time, Some("5 min".to_string()));
            }
            other => panic!("Expected Steps, got {other:?}"),
        }
    }

    // -- Stats -------------------------------------------------

    #[test]
    fn resolve_stats_basic() {
        let content = "- 99.9% {label=\"Uptime\" color=\"#22c55e\"}\n- 10K+ {label=\"Users\"}\n- <50ms {label=\"Latency\" color=\"#3b82f6\"}\n- 24/7 {label=\"Support\"}";
        let block = unknown("stats", Attrs::new(), content);
        match resolve_block(block) {
            Block::Stats { items, .. } => {
                assert_eq!(items.len(), 4);
                assert_eq!(items[0].value, "99.9%");
                assert_eq!(items[0].label, "Uptime");
                assert_eq!(items[0].color, Some("#22c55e".to_string()));
                assert_eq!(items[1].value, "10K+");
                assert_eq!(items[1].label, "Users");
                assert!(items[1].color.is_none());
                assert_eq!(items[2].value, "<50ms");
                assert_eq!(items[2].label, "Latency");
                assert_eq!(items[2].color, Some("#3b82f6".to_string()));
                assert_eq!(items[3].value, "24/7");
                assert_eq!(items[3].label, "Support");
            }
            other => panic!("Expected Stats, got {other:?}"),
        }
    }

    // -- Comparison --------------------------------------------

    #[test]
    fn resolve_comparison_basic() {
        let content = "| Feature | Free | Pro | Team |\n|---|---|---|---|\n| Projects | 3 | Unlimited | Unlimited |\n| Storage | 1GB | 10GB | 100GB |\n| Support | Community | Email | Priority |";
        let block = unknown("comparison", Attrs::new(), content);
        match resolve_block(block) {
            Block::Comparison {
                headers,
                rows,
                highlight,
                ..
            } => {
                assert_eq!(headers, vec!["Feature", "Free", "Pro", "Team"]);
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0], vec!["Projects", "3", "Unlimited", "Unlimited"]);
                assert_eq!(rows[1][0], "Storage");
                assert_eq!(rows[2][3], "Priority");
                assert!(highlight.is_none());
            }
            other => panic!("Expected Comparison, got {other:?}"),
        }
    }

    #[test]
    fn resolve_comparison_highlight() {
        let content = "| | Basic | Pro |\n|---|---|---|\n| Price | $0 | $12/mo |";
        let block = unknown(
            "comparison",
            attrs(&[("highlight", AttrValue::String("Pro".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Comparison {
                headers,
                rows,
                highlight,
                ..
            } => {
                assert_eq!(headers, vec!["", "Basic", "Pro"]);
                assert_eq!(rows.len(), 1);
                assert_eq!(highlight, Some("Pro".to_string()));
            }
            other => panic!("Expected Comparison, got {other:?}"),
        }
    }

    // -- Logo --------------------------------------------------

    #[test]
    fn resolve_logo_basic() {
        let block = unknown(
            "logo",
            attrs(&[
                ("src", AttrValue::String("/logo.svg".into())),
                ("alt", AttrValue::String("Acme Inc".into())),
                ("size", AttrValue::String("120".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Logo {
                src, alt, size, ..
            } => {
                assert_eq!(src, "/logo.svg");
                assert_eq!(alt, Some("Acme Inc".to_string()));
                assert_eq!(size, Some(120));
            }
            other => panic!("Expected Logo, got {other:?}"),
        }
    }

    #[test]
    fn resolve_logo_defaults() {
        let block = unknown(
            "logo",
            attrs(&[("src", AttrValue::String("brand.png".into()))]),
            "",
        );
        match resolve_block(block) {
            Block::Logo {
                src, alt, size, ..
            } => {
                assert_eq!(src, "brand.png");
                assert!(alt.is_none());
                assert!(size.is_none());
            }
            other => panic!("Expected Logo, got {other:?}"),
        }
    }

    // -- Toc ---------------------------------------------------

    #[test]
    fn resolve_toc_default_depth() {
        let block = unknown("toc", Attrs::new(), "");
        match resolve_block(block) {
            Block::Toc {
                depth, entries, ..
            } => {
                assert_eq!(depth, 3);
                assert!(entries.is_empty());
            }
            other => panic!("Expected Toc, got {other:?}"),
        }
    }

    #[test]
    fn resolve_toc_custom_depth() {
        let block = unknown(
            "toc",
            attrs(&[("depth", AttrValue::String("2".into()))]),
            "",
        );
        match resolve_block(block) {
            Block::Toc { depth, .. } => {
                assert_eq!(depth, 2);
            }
            other => panic!("Expected Toc, got {other:?}"),
        }
    }

    // -- BeforeAfter -----------------------------------------------

    #[test]
    fn resolve_before_after_basic() {
        let content = "### Before\n- Manual | Write everything by hand\n- Slow | Takes hours\n### After\n- Automated | One-click generation\n- Fast | Takes seconds";
        let block = unknown(
            "before-after",
            attrs(&[("transition", AttrValue::String("SurfDoc".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::BeforeAfter {
                before_items,
                after_items,
                transition,
                ..
            } => {
                assert_eq!(before_items.len(), 2);
                assert_eq!(before_items[0].label, "Manual");
                assert_eq!(before_items[0].detail, "Write everything by hand");
                assert_eq!(after_items.len(), 2);
                assert_eq!(after_items[1].label, "Fast");
                assert_eq!(transition, Some("SurfDoc".to_string()));
            }
            other => panic!("Expected BeforeAfter, got {other:?}"),
        }
    }

    #[test]
    fn resolve_before_after_no_transition() {
        let content = "### Before\n- Old | Legacy way\n### After\n- New | Modern way";
        let block = unknown("before-after", Attrs::new(), content);
        match resolve_block(block) {
            Block::BeforeAfter {
                before_items,
                after_items,
                transition,
                ..
            } => {
                assert_eq!(before_items.len(), 1);
                assert_eq!(after_items.len(), 1);
                assert_eq!(transition, None);
            }
            other => panic!("Expected BeforeAfter, got {other:?}"),
        }
    }

    // -- Pipeline --------------------------------------------------

    #[test]
    fn resolve_pipeline_basic() {
        let content = "Phone | User's device\nAI Chat | Natural language\nSurfDoc | Structured output";
        let block = unknown("pipeline", Attrs::new(), content);
        match resolve_block(block) {
            Block::Pipeline { steps, .. } => {
                assert_eq!(steps.len(), 3);
                assert_eq!(steps[0].label, "Phone");
                assert_eq!(steps[0].description, Some("User's device".to_string()));
                assert_eq!(steps[2].label, "SurfDoc");
            }
            other => panic!("Expected Pipeline, got {other:?}"),
        }
    }

    #[test]
    fn resolve_pipeline_no_desc() {
        let content = "Step 1\nStep 2\nStep 3";
        let block = unknown("pipeline", Attrs::new(), content);
        match resolve_block(block) {
            Block::Pipeline { steps, .. } => {
                assert_eq!(steps.len(), 3);
                assert_eq!(steps[0].label, "Step 1");
                assert_eq!(steps[0].description, None);
            }
            other => panic!("Expected Pipeline, got {other:?}"),
        }
    }

    // -- Section ---------------------------------------------------

    #[test]
    fn resolve_section_with_headline() {
        let content = "## Why SurfDoc?\nThe future of documents\n\nSome body text here.";
        let block = unknown(
            "section",
            attrs(&[("bg", AttrValue::String("muted".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Section {
                bg,
                headline,
                subtitle,
                children,
                ..
            } => {
                assert_eq!(bg, Some("muted".to_string()));
                assert_eq!(headline, Some("Why SurfDoc?".to_string()));
                assert_eq!(subtitle, Some("The future of documents".to_string()));
                assert!(!children.is_empty());
            }
            other => panic!("Expected Section, got {other:?}"),
        }
    }

    #[test]
    fn resolve_section_bg_attr() {
        let content = "## Features";
        let block = unknown(
            "section",
            attrs(&[("bg", AttrValue::String("dark".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Section { bg, headline, .. } => {
                assert_eq!(bg, Some("dark".to_string()));
                assert_eq!(headline, Some("Features".to_string()));
            }
            other => panic!("Expected Section, got {other:?}"),
        }
    }

    // -- ProductCard -----------------------------------------------

    #[test]
    fn resolve_product_card_full() {
        let content = "## Surf Browser\nNative SurfDoc viewer\n\nRender .surf files beautifully.\n\n- Fast rendering\n- Dark mode\n- Offline support\n\n[Download](/download)";
        let block = unknown(
            "product-card",
            attrs(&[
                ("badge", AttrValue::String("Available".into())),
                ("badge-color", AttrValue::String("green".into())),
            ]),
            content,
        );
        match resolve_block(block) {
            Block::ProductCard {
                title,
                subtitle,
                badge,
                badge_color,
                body,
                features,
                cta_label,
                cta_href,
                ..
            } => {
                assert_eq!(title, "Surf Browser");
                assert_eq!(subtitle, Some("Native SurfDoc viewer".to_string()));
                assert_eq!(badge, Some("Available".to_string()));
                assert_eq!(badge_color, Some("green".to_string()));
                assert!(body.contains("Render .surf files"));
                assert_eq!(features.len(), 3);
                assert_eq!(features[0], "Fast rendering");
                assert_eq!(cta_label, Some("Download".to_string()));
                assert_eq!(cta_href, Some("/download".to_string()));
            }
            other => panic!("Expected ProductCard, got {other:?}"),
        }
    }

    #[test]
    fn resolve_product_card_minimal() {
        let content = "## Simple Product\n\n- One feature";
        let block = unknown("product-card", Attrs::new(), content);
        match resolve_block(block) {
            Block::ProductCard {
                title,
                subtitle,
                badge,
                features,
                cta_label,
                ..
            } => {
                assert_eq!(title, "Simple Product");
                assert_eq!(subtitle, None);
                assert_eq!(badge, None);
                assert_eq!(features.len(), 1);
                assert_eq!(cta_label, None);
            }
            other => panic!("Expected ProductCard, got {other:?}"),
        }
    }
}

//! Per-block-type content parsers (Pass 2 resolution).
//!
//! `resolve_block` converts a `Block::Unknown` into a typed variant based on
//! the block name. Unknown block names pass through unchanged.

use crate::citation::{parse_authors, Reference, RefType};
use crate::types::{
    AttrValue, Attrs, AuthProvider, BeforeAfterItem, BindingEvent, Block, BookingDay,
    BookingService, CalloutType, ChartData, ChartSeries, ChartType, Format, StoreItem,
    ColumnContent, CommandItem, CrateDep, CrateEntry, DataFormat, DecisionStatus, DomainEntry,
    EmbedType, EnvEntry, EnvVar, FaqItem, FeatureCard, FieldConstraint, FilterField, FooterSection,
    FormField, FormFieldType, GalleryItem, HeroButton, HttpMethod, ListDisplay, ListFilter,
    ModelField, ModelFieldType, NavGroup, NavItem, PipelineStep, PostItem, ProductGroup, ProductItem, ProgressStep,
    RowAction, RowState, SchemaField,
    SlideLayout, SmokeCheck, SocialLink, SortSpec, Span, StatItem, StepItem,
    StyleProperty, TabBarItem, TabPanel, TaskItem, ToolbarItem, Trend, VolumeEntry,
};

/// Resolve a `Block::Unknown` into a typed variant, if the name matches a known
/// block type. Unrecognised names are returned unchanged.
pub fn resolve_block(block: Block) -> Block {
    let block = adopt_own_line_attrs(block);
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
        "tasks" | "action-items" => parse_tasks(content, *span),
        "decision" => parse_decision(attrs, content, *span),
        "metric" => parse_metric(attrs, *span),
        "summary" => parse_summary(content, *span),
        "cite" | "reference-def" => parse_cite(attrs, content, *span),
        "bibliography" | "references" => parse_bibliography(attrs, *span),
        "figure" => parse_figure(attrs, *span),
        "diagram" => parse_diagram(attrs, content, *span),
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
        "deck" => parse_deck(attrs, content, *span),
        "slide" => parse_slide(attrs, content, *span),
        "nav" => parse_nav(attrs, content, *span),
        "embed" => parse_embed(attrs, *span),
        "form" => parse_form(attrs, content, *span),
        "banner" => parse_banner(attrs, content, *span),
        "product-grid" => parse_product_grid(attrs, content, *span),
        "post-grid" => parse_post_grid(attrs, content, *span),
        "gate" => parse_gate(attrs, content, *span),
        "gallery" => parse_gallery(content, *span),
        "footer" => parse_footer(attrs, content, *span),
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
        // App description blocks
        "list" => parse_list(attrs, content, *span),
        "board" => parse_board(attrs, content, *span),
        "action" => parse_action(attrs, content, *span),
        "filter-bar" => parse_filter_bar(attrs, content, *span),
        "search" => parse_search(attrs, *span),
        "dashboard" => parse_dashboard(attrs, *span),
        "chat-input" => parse_chat_input(attrs, content, *span),
        "feed" => parse_feed(attrs, *span),
        "booking" => parse_booking(attrs, content, *span),
        "store" => parse_store(attrs, content, *span),
        // Compound widget mount points
        "editor" => parse_editor(attrs, *span),
        "chart" => parse_chart(attrs, content, *span),
        "split-pane" => parse_split_pane(attrs, *span),
        // Infrastructure manifest blocks
        "app" => parse_app(attrs, content, *span),
        "build" => parse_build(attrs, content, *span),
        "database" => parse_infra_database(attrs, content, *span),
        "deploy" => parse_deploy(attrs, content, *span),
        "env" => parse_infra_env(attrs, content, *span),
        "health" => parse_health(attrs, *span),
        "concurrency" => parse_concurrency(attrs, *span),
        "cicd" => parse_cicd(attrs, content, *span),
        "smoke" => parse_smoke(attrs, content, *span),
        "domains" => parse_domains(content, *span),
        "crates" => parse_crates(content, *span),
        "deploy_urls" | "deploy-urls" => parse_deploy_urls(content, *span),
        "volumes" => parse_volumes(content, *span),
        // App spec blocks (data layer + API)
        "model" => parse_model(attrs, content, *span),
        "route" => parse_route(attrs, content, *span),
        "auth" => parse_auth(attrs, content, *span),
        "binding" => parse_binding(attrs, content, *span),
        // App format blocks (schema, deps, config, deploy)
        "schema" => parse_schema(attrs, content, *span),
        "use" => parse_use(attrs, content, *span),
        "app-env" => parse_app_env(attrs, content, *span),
        "app-deploy" => parse_app_deploy(attrs, content, *span),
        "row" => parse_row(attrs, content, *span),
        "infocard" | "info-card" => parse_infocard(attrs, content, *span),
        // Interactive / application blocks
        "app-shell" => parse_app_shell(attrs, content, *span),
        "sidebar" => parse_sidebar(attrs, content, *span),
        "panel" => parse_panel(attrs, content, *span),
        "tab-bar" => parse_tab_bar(attrs, content, *span),
        "tab-content" => parse_tab_content(attrs, content, *span),
        "toolbar" => parse_toolbar(attrs, content, *span),
        "drawer" => parse_drawer(attrs, content, *span),
        "modal" => parse_modal(attrs, content, *span),
        "command-palette" => parse_command_palette(attrs, content, *span),
        "code-editor" => parse_code_editor(attrs, content, *span),
        "block-editor" => parse_block_editor(attrs, *span),
        "terminal" => parse_terminal(attrs, *span),
        "nav-tree" => parse_nav_tree(attrs, *span),
        "badge" => parse_badge(attrs, *span),
        "suggestion-chips" => parse_suggestion_chips(attrs, *span),
        "chat-thread" => parse_chat_thread(attrs, *span),
        "chat-input-simple" => parse_chat_input_simple(attrs, *span),
        "recipient-picker" => parse_recipient_picker(attrs, *span),
        "qr" => parse_qr(attrs, *span),
        "progress" => parse_progress(attrs, content, *span),
        "log-stream" => parse_log_stream(attrs, *span),
        "problem-list" => parse_problem_list(attrs, *span),
        _ => block,
    }
}

// ------------------------------------------------------------------
// Attribute extraction helpers
// ------------------------------------------------------------------

/// Adopt a whole-line `[attrs]` group displaced onto the block's first content
/// line (`::features` newline `[cols=3 title="Why us"]`) as the block's
/// attributes — a common generated-markup slip that otherwise renders the
/// bracket text as body copy. Fires only when the block has no attrs of its
/// own, the first non-blank content line is exactly a bracket group, and
/// `parse_attrs` accepts it with at least one attribute — which keeps markdown
/// link references (`[1]: url`), task boxes, and prose brackets out.
fn adopt_own_line_attrs(block: Block) -> Block {
    let Block::Unknown {
        name,
        attrs,
        content,
        span,
    } = block
    else {
        return block;
    };

    if attrs.is_empty() {
        if let Some((idx, line)) = content
            .lines()
            .enumerate()
            .find(|(_, l)| !l.trim().is_empty())
        {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Ok(adopted) = crate::attrs::parse_attrs(trimmed) {
                    if !adopted.is_empty() {
                        let remaining: String = content
                            .lines()
                            .enumerate()
                            .filter(|(i, _)| *i != idx)
                            .map(|(_, l)| l)
                            .collect::<Vec<_>>()
                            .join("\n");
                        return Block::Unknown {
                            name,
                            attrs: adopted,
                            content: remaining,
                            span,
                        };
                    }
                }
            }
        }
    }

    Block::Unknown {
        name,
        attrs,
        content,
        span,
    }
}

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
            "context" => Some(CalloutType::Context),
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

fn parse_diagram(attrs: &Attrs, content: &str, span: Span) -> Block {
    // Raw `type` value, lowercased — kept as a String (not an enum) so unknown
    // types round-trip losslessly and hit the prose fallback at render time.
    let diagram_type = attr_string(attrs, "type")
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let title = attr_string(attrs, "title");

    Block::Diagram {
        diagram_type,
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
    split_pipe_cells(inner)
}

/// Split `s` on `|` into trimmed cells, treating `«FILL: label | default»` /
/// `«IMG: …»` generation slot markers as atomic so their own `|` separator
/// (part of the FILL grammar, resolved later by
/// `crate::slots::resolve_slot_markers` on the final page HTML) is never
/// mistaken for a table column delimiter — that would split one marker into
/// two mangled cells and silently drop the default text.
fn split_pipe_cells(s: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_marker = false;
    for ch in s.chars() {
        match ch {
            '\u{ab}' => {
                in_marker = true;
                cur.push(ch);
            }
            '\u{bb}' => {
                in_marker = false;
                cur.push(ch);
            }
            '|' if !in_marker => {
                cells.push(cur.trim().to_string());
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    cells.push(cur.trim().to_string());
    cells
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

/// Parse a `::cite` block into a [`Block::Cite`] holding a [`Reference`].
///
/// The key comes from the `key=`/`id=` attr or a body `key:` line. The body is
/// `field: value` (or `field = value`) lines; recognised fields map onto the
/// reference (author/title/journal/year/volume/issue/pages/url/doi/…). Multiple
/// authors use `author = "Last, First; Last2, First2"`.
fn parse_cite(attrs: &Attrs, content: &str, span: Span) -> Block {
    let mut reference = Reference::default();

    // Key + type may be supplied as attributes.
    if let Some(k) = attr_string(attrs, "key").or_else(|| attr_string(attrs, "id")) {
        reference.key = k;
    }
    if let Some(t) = attr_string(attrs, "type") {
        reference.ref_type = RefType::from_str_lossy(&t);
    }

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // Split on the first ':' or '=' (whichever comes first).
        let sep = match (line.find(':'), line.find('=')) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => continue,
        };
        let field = line[..sep].trim().to_ascii_lowercase();
        let value = strip_quotes(line[sep + 1..].trim());
        if value.is_empty() {
            continue;
        }
        let v = || Some(value.to_string());
        match field.as_str() {
            "key" | "id" => reference.key = value.to_string(),
            "type" | "kind" => reference.ref_type = RefType::from_str_lossy(value),
            "author" | "authors" => reference.authors = parse_authors(value),
            "editor" | "editors" => reference.editors = parse_authors(value),
            "title" => reference.title = v(),
            "container" | "journal" | "booktitle" | "proceedings" | "site" => {
                reference.container = v()
            }
            "publisher" => reference.publisher = v(),
            "year" | "date" => reference.year = v(),
            "volume" | "vol" => reference.volume = v(),
            "issue" | "number" | "no" => reference.issue = v(),
            "pages" | "page" | "pp" => reference.pages = v(),
            "url" | "link" => reference.url = v(),
            "doi" => reference.doi = v(),
            "accessed" | "access" | "retrieved" => reference.accessed = v(),
            "edition" | "ed" => reference.edition = v(),
            _ => {}
        }
    }

    Block::Cite { reference, span }
}

/// Strip a single pair of surrounding double or single quotes.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Parse a `::bibliography` / `::references` block. Optional `style=` overrides
/// the document `format:` for this list only.
fn parse_bibliography(attrs: &Attrs, span: Span) -> Block {
    let style = attr_string(attrs, "style").and_then(|s| match s.to_ascii_lowercase().as_str() {
        "ieee" => Some(Format::Ieee),
        "acm" => Some(Format::Acm),
        "article" => Some(Format::Article),
        "mla" => Some(Format::Mla),
        "apa" => Some(Format::Apa),
        "chicago" => Some(Format::Chicago),
        _ => None,
    });
    Block::Bibliography { style, span }
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

/// Parse a `::deck` block — deck-level config (peer of `::site`).
///
/// Config may be supplied as attributes (`::deck { theme: surf-dark, aspect: 16:9 }`)
/// and/or as `key: value` lines in the block body. Both are merged into
/// `properties`; [`crate::render_slides::extract_deck`] reads the known keys.
fn parse_deck(attrs: &Attrs, content: &str, span: Span) -> Block {
    let mut properties = Vec::new();

    // Attributes first.
    for (key, value) in attrs.iter() {
        if let Some(v) = attr_value_string(value) {
            properties.push(StyleProperty {
                key: key.clone(),
                value: v,
            });
        }
    }

    // Then `key: value` body lines (mirrors parse_site).
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

    Block::Deck { properties, span }
}

/// Parse a `::slide` block — a single slide container (peer of `::page`).
///
/// Children reuse the existing content block families via
/// [`parse_page_children`], so a slide is an arrangement of existing blocks.
fn parse_slide(attrs: &Attrs, content: &str, span: Span) -> Block {
    let layout = attr_string(attrs, "layout")
        .as_deref()
        .and_then(SlideLayout::from_name);
    let kicker = attr_string(attrs, "kicker");
    let mut notes = attr_string(attrs, "notes");

    let mut children = parse_page_children(content);

    // Extract a `::notes` / `::speaker-notes` child block into the slide's
    // `notes` field (presenter notes), removing it from the rendered children.
    // The `notes=` attribute, if present, takes precedence.
    let mut extracted_notes: Option<String> = None;
    children.retain(|child| {
        if let Block::Unknown { name, content, .. } = child
            && (name == "notes" || name == "speaker-notes" || name == "presenter-notes")
        {
            if extracted_notes.is_none() {
                let text = content.trim();
                if !text.is_empty() {
                    extracted_notes = Some(text.to_string());
                }
            }
            return false;
        }
        true
    });
    if notes.is_none() {
        notes = extracted_notes;
    }

    Block::Slide {
        layout,
        kicker,
        notes,
        content: content.to_string(),
        children,
        span,
    }
}

/// Stringify an [`AttrValue`] for storage as a [`StyleProperty`] value.
fn attr_value_string(value: &AttrValue) -> Option<String> {
    match value {
        AttrValue::String(s) => Some(s.clone()),
        AttrValue::Number(n) => Some(n.to_string()),
        AttrValue::Bool(b) => Some(b.to_string()),
        AttrValue::Null => None,
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

    // Fence guard mirroring parse.rs::scan_blocks: the tolerant space-after-colons
    // opener (and the bare-`::` orphan-closer skip) are suppressed inside fenced
    // code blocks so a fenced SurfDoc example nested in a page's content stays
    // literal instead of being spliced apart.
    let mut in_fence = false;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }

        // Check for opening directive. The tolerant space-separated form
        // (`:: name`) is ignored inside fences so code samples keep their
        // literal lines; the strict form keeps its pre-existing
        // non-fence-aware behavior (matches parse.rs::scan_blocks).
        let opener = crate::parse::opening_directive(trimmed).filter(|(depth, _, _)| {
            !in_fence || crate::parse::directive_name_start(trimmed, *depth) == *depth
        });
        if let Some((depth, name, attrs_str)) = opener {
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

        // Skip bare closing markers that aren't matched to anything. Only
        // outside a fence — inside one, a lone `::` is literal code-sample
        // content and must not be dropped (matches parse.rs::scan_blocks).
        if !in_fence && crate::parse::closing_directive_depth(trimmed).is_some() {
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

/// Parse a `- [Label](href){icon=… image=… external}` nav row into a `NavItem`.
/// The trailing `{…}` is optional and may carry `icon=`, `image=`, and a bare
/// `external` token in any order.
fn parse_nav_item_line(trimmed: &str) -> Option<NavItem> {
    let rest = trimmed.strip_prefix("- [")?;
    let label_end = rest.find("](")?;
    let label = rest[..label_end].to_string();
    let after_bracket = &rest[label_end + 2..]; // skip "]("
    let href_end = after_bracket.find(')')?;
    let href = after_bracket[..href_end].to_string();

    // Optional trailing `{...}` attribute brace.
    let mut icon = None;
    let mut image = None;
    let mut external = false;
    let after_link = after_bracket[href_end + 1..].trim_start();
    if let Some(brace) = after_link.strip_prefix('{')
        && let Some(end) = brace.find('}')
    {
        for tok in brace[..end].split_whitespace() {
            if let Some(v) = tok.strip_prefix("icon=") {
                icon = Some(v.to_string());
            } else if let Some(v) = tok.strip_prefix("image=") {
                image = Some(v.to_string());
            } else if tok == "external" {
                external = true;
            }
        }
    }
    Some(NavItem { label, href, icon, image, external })
}

fn parse_nav(attrs: &Attrs, content: &str, span: Span) -> Block {
    let logo = attr_string(attrs, "logo");
    let brand = attr_string(attrs, "brand");
    let brand_reg = attr_bool(attrs, "reg");
    let drawer = attr_bool(attrs, "drawer");
    let minimal = attr_bool(attrs, "minimal");
    // Trailing primary CTA link (per-page "Get in touch" etc.).
    let cta = match (attr_string(attrs, "cta-label"), attr_string(attrs, "cta-href")) {
        (Some(label), Some(href)) => Some(NavItem {
            label,
            href,
            icon: attr_string(attrs, "cta-icon"),
            image: None,
            external: false,
        }),
        _ => None,
    };

    let mut items: Vec<NavItem> = Vec::new();
    let mut groups: Vec<NavGroup> = Vec::new();
    let mut current_label: Option<String> = None;
    let mut current_items: Vec<NavItem> = Vec::new();

    // Parse link lines, grouping by `## Heading` / `### Heading` markers. Items
    // before any heading stay in the flat `items` list (legacy behaviour).
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("## ").or_else(|| trimmed.strip_prefix("### ")) {
            // Flush the previous group (or lead items) before starting a new one.
            if let Some(label) = current_label.take() {
                groups.push(NavGroup {
                    label: Some(label),
                    items: std::mem::take(&mut current_items),
                });
            }
            current_label = Some(rest.trim().to_string());
            continue;
        }
        if !trimmed.starts_with("- [") {
            continue;
        }
        if let Some(item) = parse_nav_item_line(trimmed) {
            if current_label.is_some() {
                current_items.push(item);
            } else {
                items.push(item);
            }
        }
    }
    if let Some(label) = current_label {
        groups.push(NavGroup {
            label: Some(label),
            items: current_items,
        });
    }

    Block::Nav { items, logo, groups, brand, brand_reg, cta, drawer, minimal, span }
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
    let action = attr_string(attrs, "action");
    let method = attr_string(attrs, "method");
    let honeypot = attr_bool(attrs, "honeypot");
    let mut fields = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse field lines: `- Label (type) *` or `- Label (type, "placeholder")`
        // or `- Label (select: Option A | Option B | Option C) *`
        // or `- type: Label` (colon shorthand: field type before colon, label after)
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            // Colon shorthand: `- type: Label` (no parentheses, colon present)
            // Only treat as type:label if the left-hand side is a known type keyword.
            let (label_part, type_part) = if !rest.contains('(') {
                if let Some(colon_pos) = rest.find(':') {
                    let maybe_type = rest[..colon_pos].trim().to_lowercase();
                    let label_after = rest[colon_pos + 1..].trim();
                    let known = matches!(
                        maybe_type.as_str(),
                        "email" | "tel" | "phone" | "date" | "number" | "password"
                            | "select" | "textarea" | "multiline" | "text"
                    );
                    if known && !label_after.is_empty() {
                        (label_after, Some((maybe_type, String::new())))
                    } else {
                        (rest.trim_end_matches(" *").trim_end_matches('*'), None)
                    }
                } else {
                    (rest.trim_end_matches(" *").trim_end_matches('*'), None)
                }
            } else {
                let paren_start = rest.find('(').unwrap();
                let label = rest[..paren_start].trim();
                let after_paren = &rest[paren_start + 1..];
                let paren_end = after_paren.find(')').unwrap_or(after_paren.len());
                let type_str = &after_paren[..paren_end];
                let remainder = &after_paren[paren_end..].trim_start_matches(')');
                (label, Some((type_str.to_string(), remainder.to_string())))
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
                            "password" => FormFieldType::Password,
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
        action,
        method,
        honeypot,
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

fn parse_footer(attrs: &Attrs, content: &str, span: Span) -> Block {
    let brand = attr_string(attrs, "brand");
    let brand_reg = attr_bool(attrs, "reg");
    let brand_logo = attr_string(attrs, "logo");
    let tagline = attr_string(attrs, "tagline");
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
                        image: None,
                        external: false,
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
                image: None,
                external: false,
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
        brand,
        brand_reg,
        brand_logo,
        tagline,
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
/// Balance-aware since 0.11 (mirrors `parse.rs::is_leaf_before_sibling`):
/// same-depth openers after ours count as nested children owed one closer
/// each — chrome-in-chrome nesting is real nesting. Our closer is the first
/// SURPLUS closer at `open_depth`. If EOF arrives without it, the block has
/// no closer and is treated as a leaf (return None).
///
/// Returns `(collected_content, closing_line_index)`.
fn scan_container_close(lines: &[&str], start: usize, open_depth: usize) -> Option<(String, usize)> {
    let mut nesting = 0usize;
    let mut content_lines: Vec<&str> = Vec::new();
    // Fence guard mirroring parse_page_children/parse.rs::scan_blocks: suppress
    // the tolerant space-after-colons opener match inside fenced code blocks so
    // a nested SurfDoc example isn't parsed as a real sibling/nested directive.
    let mut in_fence = false;

    for i in start..lines.len() {
        let trimmed = lines[i].trim();

        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        }

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

        // Check for opening directives. Same tolerant-opener fence gating as
        // the top-level scan (see parse.rs::scan_blocks).
        let opener = crate::parse::opening_directive(trimmed).filter(|(depth, _, _)| {
            !in_fence || crate::parse::directive_name_start(trimmed, *depth) == *depth
        });
        if let Some((nested_depth, _, _)) = opener {
            // Same-or-deeper opener — a nested child owed one closer.
            if nested_depth >= open_depth {
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

    // Tolerate spaces/tabs between the colons and the name, matching
    // `crate::parse::opening_directive`.
    let rest = trimmed[2..].trim_start_matches([' ', '\t']);
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

/// Parse the brace suffix on a hero/banner button link — `{primary}`,
/// `{external}`, `{primary external}` (order-free, `.`-prefixed markdown
/// class forms accepted) → (primary, external).
fn parse_button_flags(suffix: &str) -> (bool, bool) {
    let Some(inner) = suffix.strip_prefix('{').and_then(|s| s.strip_suffix('}')) else {
        return (false, false);
    };
    let mut primary = false;
    let mut external = false;
    for flag in inner.split_whitespace() {
        match flag.trim_start_matches('.') {
            "primary" => primary = true,
            "external" => external = true,
            _ => {}
        }
    }
    (primary, external)
}

fn parse_hero(attrs: &Attrs, content: &str, span: Span) -> Block {
    let badge = attr_string(attrs, "badge");
    let align = attr_string(attrs, "align").unwrap_or_else(|| "center".to_string());
    let image = attr_string(attrs, "image");
    let image_alt = attr_string(attrs, "image-alt");
    let layout = attr_string(attrs, "layout");
    let transparent = attr_bool(attrs, "transparent");

    let mut headline: Option<String> = None;
    let mut subtitle_lines: Vec<&str> = Vec::new();
    let mut buttons: Vec<HeroButton> = Vec::new();
    let mut in_subtitle = false;
    // Lines NOT consumed as headline/subtitle/buttons. Only these survive as
    // `content` — native renderers draw `content` verbatim below the typed
    // fields, so keeping the full body there duplicates the hero on screen.
    let mut leftover: Vec<&str> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if in_subtitle {
                in_subtitle = false;
            }
            leftover.push("");
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
                // Find closing paren, possibly followed by {primary} / {.primary}
                if let Some(href_end) = after.find(')') {
                    let href = after[..href_end].to_string();
                    let suffix = after[href_end + 1..].trim();
                    let (primary, external) = parse_button_flags(suffix);
                    buttons.push(HeroButton {
                        label,
                        href,
                        primary,
                        external,
                    });
                    continue;
                }
            }

        // Otherwise it's subtitle text
        if headline.is_some() && buttons.is_empty() {
            subtitle_lines.push(trimmed);
            in_subtitle = true;
        } else {
            leftover.push(line);
        }
    }

    let subtitle = if subtitle_lines.is_empty() {
        None
    } else {
        Some(subtitle_lines.join(" "))
    };
    let content = leftover.join("\n").trim().to_string();

    Block::Hero {
        headline,
        subtitle,
        badge,
        align,
        image,
        image_alt,
        layout,
        transparent,
        buttons,
        content,
        span,
    }
}

/// Parse a `::banner` block: a centered CTA band.
///
/// Reuses the hero authoring grammar — `# Heading`, free-text subtitle lines,
/// and `[Label](href){primary}` button links — but renders a compact mid-page
/// band rather than a page hero.
fn parse_banner(attrs: &Attrs, content: &str, span: Span) -> Block {
    let id = attr_string(attrs, "id");
    let mut headline: Option<String> = None;
    let mut subtitle_lines: Vec<&str> = Vec::new();
    let mut buttons: Vec<HeroButton> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            headline = Some(rest.trim().to_string());
            continue;
        }
        if trimmed.starts_with('[')
            && let Some(label_end) = trimmed.find("](")
        {
            let label = trimmed[1..label_end].to_string();
            let after = &trimmed[label_end + 2..];
            if let Some(href_end) = after.find(')') {
                let href = after[..href_end].to_string();
                let suffix = after[href_end + 1..].trim();
                let (primary, external) = parse_button_flags(suffix);
                buttons.push(HeroButton {
                    label,
                    href,
                    primary,
                    external,
                });
                continue;
            }
        }
        subtitle_lines.push(trimmed);
    }

    let subtitle = if subtitle_lines.is_empty() {
        None
    } else {
        Some(subtitle_lines.join(" "))
    };

    Block::Banner {
        headline,
        subtitle,
        buttons,
        id,
        // Every non-blank line above is consumed into a typed field, so no
        // free-form content survives — mirroring `parse_hero`'s leftover rule
        // (native renderers draw `content` verbatim; a full-body copy here
        // renders the banner twice).
        content: String::new(),
        span,
    }
}

/// Parse a `::product-grid` block.
///
/// `### Group` headings split the grid into labelled groups. Each card is a
/// pipe-delimited list item: `- Name | emblem-src | href | tagline | bg`. The
/// emblem, tagline, and bg fields are optional (leave empty or omit trailing
/// pipes). The emblem field also accepts `icon:<lucide-name>` — the built-in
/// icon renders as an accent chip instead of an image (unknown → omitted).
/// An href of `-` (or empty) marks a **linkless card** — a product
/// with no public destination yet: classic cards render as a static `<div>`
/// (no arrow, no hover), tiles drop the default "Learn more" CTA.
/// `[tiles]` renders apple.com-style promo tiles; the bg field then
/// picks the tile background: `image:<src>` / `color:<css>` / `gradient:<css>`
/// / `surface` (opaque theme surface, theme-following text) / `transparent`,
/// with a trailing ` dark` token for light-on-dark text.
fn parse_product_grid(attrs: &Attrs, content: &str, span: Span) -> Block {
    let tiles = attr_bool(attrs, "tiles");
    let mut groups: Vec<ProductGroup> = Vec::new();
    let mut current = ProductGroup {
        label: None,
        items: Vec::new(),
        cols: None,
    };
    let mut started = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            // New group heading — flush the previous group if it has content.
            if started && (!current.items.is_empty() || current.label.is_some()) {
                groups.push(current);
            }
            // `{cols=N}` brace on the heading line sets this row's max columns
            // (tiles mode); a heading that is ONLY the brace starts an unlabelled row.
            let mut heading = rest.trim().to_string();
            let mut cols = None;
            if let Some(i) = heading.find("{cols=") {
                let tail = &heading[i + 6..];
                if let Some(j) = tail.find('}') {
                    cols = tail[..j].trim().parse::<u8>().ok().map(|n| n.clamp(1, 3));
                    heading = format!("{}{}", &heading[..i], &heading[i + 6 + j + 1..]).trim().to_string();
                }
            }
            current = ProductGroup {
                label: if heading.is_empty() { None } else { Some(heading) },
                items: Vec::new(),
                cols,
            };
            started = true;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let parts: Vec<&str> = rest.split('|').map(|p| p.trim()).collect();
            let name = parts.first().copied().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let emblem = parts
                .get(1)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let href = parts.get(2).copied().unwrap_or("").to_string();
            let tagline = parts
                .get(3)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let bg = parts
                .get(4)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            // Trailing `[Label](href)` fields (after bg): one link = the
            // secondary CTA; two links = primary override + secondary.
            let links: Vec<(String, String)> = parts
                .get(5..)
                .unwrap_or(&[])
                .iter()
                .filter_map(|s| {
                    let rest = s.trim().strip_prefix('[')?;
                    let (label, tail) = rest.split_once("](")?;
                    let href = tail.strip_suffix(')')?;
                    Some((label.to_string(), href.to_string()))
                })
                .collect();
            let (cta1, cta2) = match links.len() {
                0 => (None, None),
                1 => (None, Some(links[0].clone())),
                _ => (Some(links[0].clone()), Some(links[1].clone())),
            };
            current.items.push(ProductItem {
                name,
                href,
                emblem,
                tagline,
                cta1_label: cta1.as_ref().map(|l| l.0.clone()),
                cta1_href: cta1.as_ref().map(|l| l.1.clone()),
                cta2_label: cta2.as_ref().map(|l| l.0.clone()),
                cta2_href: cta2.as_ref().map(|l| l.1.clone()),
                bg,
            });
            started = true;
        }
    }
    if !current.items.is_empty() || current.label.is_some() {
        groups.push(current);
    }

    Block::ProductGrid { groups, tiles, span }
}

/// Parse a `::post-grid` block: a card grid for a blog/news/events index.
///
/// `title`/`subtitle` come from attrs. Each card is a markdown-link bullet with
/// optional `{external}` brace and pipe-delimited trailing fields:
///
/// ```text
/// - [Card Title](/href){external} | Category · Date | One-line excerpt | /assets/img.png
/// ```
///
/// Fields after the link are split on `|`, trimmed, and all optional:
/// `meta`, `excerpt`, `image` in that order.
fn parse_post_grid(attrs: &Attrs, content: &str, span: Span) -> Block {
    let title = attr_string(attrs, "title");
    let subtitle = attr_string(attrs, "subtitle");
    let mut items: Vec<PostItem> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let rest = rest.trim();
        if !rest.starts_with('[') {
            continue;
        }
        let Some(label_end) = rest.find("](") else {
            continue;
        };
        let post_title = rest[1..label_end].trim().to_string();
        let after = &rest[label_end + 2..];
        let Some(href_end) = after.find(')') else {
            continue;
        };
        let href = after[..href_end].trim().to_string();
        let mut tail = after[href_end + 1..].trim();

        // Optional `{external}` brace immediately after the link.
        let mut external = false;
        if let Some(stripped) = tail.strip_prefix("{external}") {
            external = true;
            tail = stripped.trim();
        }

        // Remaining pipe-delimited fields: meta | excerpt | image.
        let tail = tail.trim_start_matches('|').trim();
        let fields: Vec<&str> = if tail.is_empty() {
            Vec::new()
        } else {
            tail.split('|').map(|f| f.trim()).collect()
        };
        let field = |i: usize| {
            fields
                .get(i)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };

        if post_title.is_empty() {
            continue;
        }
        items.push(PostItem {
            title: post_title,
            href,
            meta: field(0),
            excerpt: field(1),
            image: field(2),
            external,
        });
    }

    Block::PostGrid {
        title,
        subtitle,
        items,
        span,
    }
}

/// Parse a `::gate` block: an access-code card (password + submit).
///
/// All fields come from attrs. `action` defaults to "". When the `subtitle`
/// attr is absent, any free-text body becomes the subtitle.
fn parse_gate(attrs: &Attrs, content: &str, span: Span) -> Block {
    let title = attr_string(attrs, "title");
    let action = attr_string(attrs, "action").unwrap_or_default();
    let field_label = attr_string(attrs, "field");
    let submit_label = attr_string(attrs, "submit");
    let error = attr_string(attrs, "error");

    let subtitle = attr_string(attrs, "subtitle").or_else(|| {
        let body: Vec<&str> = content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if body.is_empty() {
            None
        } else {
            Some(body.join(" "))
        }
    });

    Block::Gate {
        title,
        subtitle,
        action,
        field_label,
        submit_label,
        error,
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
        // Accept both `### ` and `## ` headings to start a card (mirrors parse_steps).
        if let Some(rest) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
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

/// Parse a `::booking` block.
///
/// Attrs: `title`, `service-label`. Content lines:
/// - `service: Name | Duration | Price`  (duration/price optional)
/// - `day: 2026-07-06 | 9:00 AM, 10:00 AM, 2:00 PM`  (slots comma-separated;
///   `full` or no slots → the day renders as unavailable)
fn parse_booking(attrs: &Attrs, content: &str, span: Span) -> Block {
    let mut services: Vec<BookingService> = Vec::new();
    let mut days: Vec<BookingDay> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if let Some(rest) = trimmed.strip_prefix("service:") {
            let mut parts = rest.split('|').map(|p| p.trim());
            let name = parts.next().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let duration = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
            let price = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
            services.push(BookingService {
                name,
                duration,
                price,
            });
        } else if let Some(rest) = trimmed.strip_prefix("day:") {
            let mut parts = rest.splitn(2, '|');
            let date = parts.next().unwrap_or("").trim().to_string();
            if date.is_empty() {
                continue;
            }
            let slots_raw = parts.next().unwrap_or("").trim();
            let slots: Vec<String> = if slots_raw.is_empty()
                || slots_raw.eq_ignore_ascii_case("full")
                || slots_raw.eq_ignore_ascii_case("none")
            {
                Vec::new()
            } else {
                slots_raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            days.push(BookingDay { date, slots });
        }
    }

    Block::Booking {
        title: attr_string(attrs, "title"),
        service_label: attr_string(attrs, "service-label"),
        services,
        days,
        span,
    }
}

/// Parse a `::store` block.
///
/// Attrs: `title`, `currency`. Content lines:
/// - `category: Home`  (sets the category for subsequent items)
/// - `item: Name | Price | Blurb | Badge`  (blurb/badge optional)
fn parse_store(attrs: &Attrs, content: &str, span: Span) -> Block {
    let mut items: Vec<StoreItem> = Vec::new();
    let mut current_category: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if let Some(rest) = trimmed.strip_prefix("category:") {
            let c = rest.trim();
            current_category = if c.is_empty() {
                None
            } else {
                Some(c.to_string())
            };
        } else if let Some(rest) = trimmed.strip_prefix("item:") {
            let mut parts = rest.split('|').map(|p| p.trim());
            let name = parts.next().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let price = parts.next().unwrap_or("0").to_string();
            let blurb = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
            let badge = parts.next().filter(|s| !s.is_empty()).map(str::to_string);
            items.push(StoreItem {
                name,
                price,
                blurb,
                badge,
                category: current_category.clone(),
            });
        }
    }

    Block::Store {
        title: attr_string(attrs, "title"),
        currency: attr_string(attrs, "currency"),
        items,
        span,
    }
}

fn parse_stats(content: &str, span: Span) -> Block {
    let mut items: Vec<StatItem> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let text = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if text.is_empty() {
            continue;
        }

        // Pipe syntax: `value | label` (no inline color).
        if let Some(pipe) = text.find('|') {
            let value = text[..pipe].trim().to_string();
            let label = text[pipe + 1..].trim().to_string();
            if !value.is_empty() && !label.is_empty() {
                items.push(StatItem {
                    value,
                    label,
                    color: None,
                });
            }
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
        let lower = trimmed.to_ascii_lowercase();
        // Accept both `## Before`/`## After` and `### Before`/`### After`.
        if lower == "## before" || lower == "### before" {
            in_after = false;
            continue;
        }
        if lower == "## after" || lower == "### after" {
            in_after = true;
            continue;
        }
        // Skip blanks and `---` section separators.
        if trimmed.is_empty() || trimmed.chars().all(|c| c == '-') {
            continue;
        }
        let text = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        // `label | detail` is optional — a plain bullet is just a label.
        let item = if let Some(pipe_pos) = text.find(" | ") {
            BeforeAfterItem {
                label: text[..pipe_pos].trim().to_string(),
                detail: text[pipe_pos + 3..].trim().to_string(),
            }
        } else {
            BeforeAfterItem {
                label: text.to_string(),
                detail: String::new(),
            }
        };
        if in_after {
            after_items.push(item);
        } else {
            before_items.push(item);
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
    // Title / subtitle / CTA may be supplied as attrs (consistent with `hero`,
    // `cta`, etc.) or in the content body. Attrs take precedence.
    let attr_title = attr_string(attrs, "title");
    let attr_subtitle = attr_string(attrs, "subtitle");
    let attr_cta_label = attr_string(attrs, "cta-label");
    let attr_cta_href = attr_string(attrs, "cta-href");

    let mut title = String::new();
    let mut subtitle: Option<String> = None;
    let mut body_lines: Vec<&str> = Vec::new();
    let mut features: Vec<String> = Vec::new();
    let mut cta_label: Option<String> = None;
    let mut cta_href: Option<String> = None;

    // States: 0=before_title, 1=after_title(subtitle), 2=body, 3=features.
    // When the title is given as an attr, the content carries no `## ` heading,
    // so start in the body state to capture body/features/cta.
    let mut state = if attr_title.is_some() { 2 } else { 0 };

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

    // Attrs override content-derived values.
    let title = attr_title.unwrap_or(title);
    let subtitle = attr_subtitle.or(subtitle);
    let cta_label = attr_cta_label.or(cta_label);
    let cta_href = attr_cta_href.or(cta_href);

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
// App description block parsers
// ------------------------------------------------------------------

fn attr_u32(attrs: &Attrs, key: &str) -> Option<u32> {
    attrs.get(key).and_then(|v| match v {
        AttrValue::Number(n) => Some(*n as u32),
        AttrValue::String(s) => s.parse().ok(),
        _ => None,
    })
}

/// Validate a URL-like source/action path.
///
/// Only relative paths starting with `/` are allowed. Rejects `javascript:`,
/// `data:`, external URLs, and path traversal to prevent injection attacks.
fn validate_source_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    // Block dangerous schemes
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("vbscript:")
    {
        return None;
    }
    // Block external URLs — only relative paths allowed
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("//") {
        return None;
    }
    // Block path traversal
    if trimmed.contains("..") {
        return None;
    }
    // Must start with /
    if !trimmed.starts_with('/') {
        return None;
    }
    Some(trimmed.to_string())
}

/// Validate a data-block `source=` value that may be either a `/`-rooted
/// path (delegated to [`validate_source_path`]) or a bare binding-registry
/// name: dotted lowerCamel segments (`conversations`, `chat.thread`,
/// `contactRequests`). The segment character allowlist (`[a-z]` start,
/// alphanumeric/`_` body, `.` separator) structurally rejects everything the
/// strict validator blocks — `javascript:`/`data:`/`vbscript:` schemes (no
/// `:`), `http(s)://` and protocol-relative URLs (no `/`), and `..`
/// traversal (empty segments). Wired to `::list`/`::search` only; every
/// other caller stays on the strict path-only validator.
fn validate_registry_source(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('/') {
        return validate_source_path(trimmed);
    }
    let is_registry_name = trimmed.split('.').all(|seg| {
        seg.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    if is_registry_name {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn parse_list(attrs: &Attrs, content: &str, span: Span) -> Block {
    let source = attr_string(attrs, "source")
        .and_then(|s| validate_registry_source(&s))
        .unwrap_or_default();

    let display = attr_string(attrs, "display")
        .and_then(|s| match s.as_str() {
            "card" => Some(ListDisplay::Card),
            "table" => Some(ListDisplay::Table),
            "compact" => Some(ListDisplay::Compact),
            _ => None,
        })
        .unwrap_or(ListDisplay::Card);

    let preload = attr_bool(attrs, "preload");
    let stream = attr_string(attrs, "stream");
    let on_select = attr_string(attrs, "on-select");

    let mut item_template = String::new();
    let mut filters = Vec::new();
    let mut sort = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(field) = trimmed.strip_prefix("filter:") {
            filters.push(ListFilter {
                field: field.trim().to_string(),
            });
        } else if let Some(sort_spec) = trimmed.strip_prefix("sort:") {
            let parts: Vec<&str> = sort_spec.trim().split_whitespace().collect();
            if let Some(field) = parts.first() {
                let descending = parts.get(1).is_some_and(|d| d.eq_ignore_ascii_case("desc"));
                sort = Some(SortSpec {
                    field: field.to_string(),
                    descending,
                });
            }
        } else {
            if !item_template.is_empty() {
                item_template.push('\n');
            }
            item_template.push_str(trimmed);
        }
    }

    Block::List {
        source,
        display,
        item_template,
        filters,
        sort,
        preload,
        stream,
        on_select,
        span,
    }
}

fn parse_board(attrs: &Attrs, content: &str, span: Span) -> Block {
    let source = attr_string(attrs, "source")
        .and_then(|s| validate_source_path(&s))
        .unwrap_or_default();
    let preload = attr_bool(attrs, "preload");

    let mut columns = Vec::new();
    let mut card_template = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(cols) = trimmed.strip_prefix("columns:") {
            columns = cols.split('|').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
        } else if let Some(tmpl) = trimmed.strip_prefix("card-template:") {
            card_template = Some(tmpl.trim().to_string());
        }
    }

    Block::Board {
        source,
        columns,
        card_template,
        preload,
        span,
    }
}

fn parse_action(attrs: &Attrs, content: &str, span: Span) -> Block {
    let method = attr_string(attrs, "method")
        .and_then(|s| match s.to_lowercase().as_str() {
            "get" => Some(HttpMethod::Get),
            "post" => Some(HttpMethod::Post),
            "put" => Some(HttpMethod::Put),
            "patch" => Some(HttpMethod::Patch),
            "delete" => Some(HttpMethod::Delete),
            _ => None,
        })
        .unwrap_or(HttpMethod::Post);

    let target = attr_string(attrs, "target")
        .and_then(|s| validate_source_path(&s))
        .unwrap_or_default();

    let attr_label = attr_string(attrs, "label");
    let confirm = attr_string(attrs, "confirm");

    // Reuse form field parsing from parse_form.
    // Track whether each item had parentheses so we can detect a bare
    // trailing item that should become the submit-button label.
    let mut fields = Vec::new();
    let mut has_parens = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            let (label_part, type_part) = if let Some(paren_start) = rest.find('(') {
                let lbl = rest[..paren_start].trim();
                let after_paren = &rest[paren_start + 1..];
                let paren_end = after_paren.find(')').unwrap_or(after_paren.len());
                let type_str = &after_paren[..paren_end];
                let remainder = &after_paren[paren_end..].trim_start_matches(')');
                (lbl, Some((type_str.to_string(), remainder.to_string())))
            } else {
                (rest.trim_end_matches(" *").trim_end_matches('*'), None)
            };

            let required = rest.ends_with('*') || rest.ends_with("* ");
            let item_has_parens = type_part.is_some();

            let (field_type, placeholder, options) = match &type_part {
                Some((type_str, _)) => {
                    let type_str = type_str.trim();
                    if let Some(opts_part) = type_str.strip_prefix("select:") {
                        let opts: Vec<String> = opts_part
                            .split('|')
                            .map(|o| o.trim().to_string())
                            .filter(|o| !o.is_empty())
                            .collect();
                        (FormFieldType::Select, None, opts)
                    } else {
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
                            "password" => FormFieldType::Password,
                            "select" => FormFieldType::Select,
                            "textarea" | "multiline" => FormFieldType::Textarea,
                            _ => FormFieldType::Text,
                        };
                        (ft, placeholder, Vec::new())
                    }
                }
                None => (FormFieldType::Text, None, Vec::new()),
            };

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
            has_parens.push(item_has_parens);
        }
    }

    // Always pop trailing bare items — they are button labels, not fields.
    let bare_label = if !fields.is_empty() && !has_parens[fields.len() - 1] {
        let btn = fields.pop().unwrap();
        has_parens.pop();
        Some(btn.label)
    } else {
        None
    };

    // Explicit label attr wins, then bare item, then "Submit".
    let label = attr_label
        .or(bare_label)
        .unwrap_or_else(|| "Submit".to_string());

    Block::Action {
        method,
        target,
        label,
        fields,
        confirm,
        span,
    }
}

fn parse_filter_bar(attrs: &Attrs, content: &str, span: Span) -> Block {
    let target_selector = attr_string(attrs, "target").unwrap_or_default();

    let mut fields = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            if let Some(paren_start) = rest.find('(') {
                let label_part = rest[..paren_start].trim().to_string();
                let after_paren = &rest[paren_start + 1..];
                let paren_end = after_paren.find(')').unwrap_or(after_paren.len());
                let type_str = &after_paren[..paren_end];

                if let Some(opts_part) = type_str.strip_prefix("select:") {
                    let options: Vec<String> = opts_part
                        .split('|')
                        .map(|o| o.trim().to_string())
                        .filter(|o| !o.is_empty())
                        .collect();
                    let name = label_part
                        .to_lowercase()
                        .replace(|c: char| !c.is_alphanumeric(), "_")
                        .trim_matches('_')
                        .to_string();
                    fields.push(FilterField {
                        label: label_part,
                        name,
                        options,
                    });
                }
            }
        }
    }

    Block::FilterBar {
        target_selector,
        fields,
        span,
    }
}

fn parse_search(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source")
        .and_then(|s| validate_registry_source(&s))
        .unwrap_or_default();
    let placeholder = attr_string(attrs, "placeholder");

    Block::Search {
        source,
        placeholder,
        span,
    }
}

fn parse_dashboard(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source")
        .and_then(|s| validate_source_path(&s))
        .unwrap_or_default();
    let refresh = attr_u32(attrs, "refresh");

    Block::Dashboard {
        source,
        refresh,
        span,
    }
}

fn parse_chat_input(attrs: &Attrs, content: &str, span: Span) -> Block {
    let action = attr_string(attrs, "action")
        .and_then(|s| validate_source_path(&s))
        .unwrap_or_default();
    let placeholder = attr_string(attrs, "placeholder");

    let mut modes = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(mode_list) = trimmed.strip_prefix("modes:") {
            modes = mode_list
                .split('|')
                .map(|m| m.trim().to_string())
                .filter(|m| !m.is_empty())
                .collect();
        }
    }

    Block::ChatInput {
        action,
        placeholder,
        modes,
        span,
    }
}

fn parse_feed(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source")
        .and_then(|s| validate_source_path(&s))
        .unwrap_or_default();
    let stream = attr_bool(attrs, "stream");

    Block::Feed {
        source,
        stream,
        span,
    }
}

fn parse_editor(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source").and_then(|s| validate_source_path(&s));
    let lang = attr_string(attrs, "lang");
    let preview = attr_bool(attrs, "preview");

    Block::Editor {
        source,
        lang,
        preview,
        span,
    }
}

fn parse_chart(attrs: &Attrs, content: &str, span: Span) -> Block {
    let chart_type = attr_string(attrs, "type")
        .and_then(|s| match s.to_lowercase().as_str() {
            "line" => Some(ChartType::Line),
            "bar" => Some(ChartType::Bar),
            "pie" => Some(ChartType::Pie),
            "area" => Some(ChartType::Area),
            "scatter" => Some(ChartType::Scatter),
            "donut" | "doughnut" => Some(ChartType::Donut),
            "stacked-bar" | "stackedbar" | "stacked" => Some(ChartType::StackedBar),
            "radar" | "spider" => Some(ChartType::Radar),
            _ => None,
        })
        .unwrap_or(ChartType::Line);

    let source = attr_string(attrs, "source")
        .and_then(|s| validate_source_path(&s))
        .unwrap_or_default();
    let period = attr_string(attrs, "period");
    let title = attr_string(attrs, "title");
    let data = parse_chart_data(content);

    Block::Chart {
        chart_type,
        source,
        period,
        title,
        data,
        span,
    }
}

/// Parse a `::chart` block body into an inline [`ChartData`] dataset.
///
/// The body is a pipe-delimited table consistent with `::data[format=table]`:
/// the first non-empty line is the header (`<x-label> | <series1> | <series2>
/// …`), and every following row is `<category> | <value1> | <value2> …`.
/// Markdown separator rows (`|---|---|`) are skipped. Non-numeric value cells
/// parse to `0.0`. Returns `None` when there is no usable data (no rows, or
/// fewer than two columns) so the chart falls back to the mount point.
pub(crate) fn parse_chart_data(content: &str) -> Option<ChartData> {
    let (headers, rows) = parse_table_content(content);
    if headers.len() < 2 || rows.is_empty() {
        return None;
    }

    let series_count = headers.len() - 1;
    let mut series: Vec<ChartSeries> = headers[1..]
        .iter()
        .map(|name| ChartSeries {
            name: name.clone(),
            values: Vec::with_capacity(rows.len()),
        })
        .collect();

    let mut categories = Vec::with_capacity(rows.len());
    for row in &rows {
        categories.push(row.first().cloned().unwrap_or_default());
        for (j, s) in series.iter_mut().enumerate() {
            let cell = row.get(j + 1).map(|c| c.as_str()).unwrap_or("");
            s.values.push(parse_chart_number(cell));
        }
    }

    let _ = series_count;
    Some(ChartData { categories, series })
}

/// Parse a numeric chart cell. Strips common formatting (`$`, `%`, `,`,
/// surrounding whitespace); non-numeric cells become `0.0`.
fn parse_chart_number(cell: &str) -> f64 {
    let cleaned: String = cell
        .chars()
        .filter(|c| !matches!(c, '$' | '%' | ',' | ' ' | '_'))
        .collect();
    cleaned.parse::<f64>().unwrap_or(0.0)
}

fn parse_split_pane(attrs: &Attrs, span: Span) -> Block {
    let ratio = attr_string(attrs, "ratio").unwrap_or_else(|| "50:50".to_string());

    Block::SplitPane { ratio, span }
}

// ------------------------------------------------------------------
// Infrastructure manifest block parsers
// ------------------------------------------------------------------

/// Container block — recursively parse children (like `parse_page`).
fn parse_app(attrs: &Attrs, content: &str, span: Span) -> Block {
    let name = attr_string(attrs, "name").unwrap_or_default();
    let binary = attr_string(attrs, "binary");
    let region = attr_string(attrs, "region");
    let port = attr_u32(attrs, "port");
    let platform = attr_string(attrs, "platform");
    let auth = attr_string(attrs, "auth");

    let children = parse_page_children(content);

    Block::App {
        name,
        binary,
        region,
        port,
        platform,
        auth,
        content: content.to_string(),
        children,
        span,
    }
}

fn parse_build(attrs: &Attrs, content: &str, span: Span) -> Block {
    let base = attr_string(attrs, "base");
    let runtime = attr_string(attrs, "runtime");
    let edition = attr_string(attrs, "edition");

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

    Block::Build {
        base,
        runtime,
        edition,
        properties,
        span,
    }
}

fn parse_infra_database(attrs: &Attrs, content: &str, span: Span) -> Block {
    let name = attr_string(attrs, "name");
    let shared_auth = attr_bool(attrs, "shared_auth");
    let volume_gb = attr_u32(attrs, "volume_gb");

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

    Block::InfraDatabase {
        name,
        shared_auth,
        volume_gb,
        properties,
        span,
    }
}

fn parse_deploy(attrs: &Attrs, content: &str, span: Span) -> Block {
    let env = attr_string(attrs, "env");
    let app = attr_string(attrs, "app");
    let machines = attr_u32(attrs, "machines");
    let memory = attr_u32(attrs, "memory");
    let auto_stop = attr_string(attrs, "auto_stop");
    let min_machines = attr_u32(attrs, "min_machines");
    let strategy = attr_string(attrs, "strategy");

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

    Block::Deploy {
        env,
        app,
        machines,
        memory,
        auto_stop,
        min_machines,
        strategy,
        properties,
        span,
    }
}

/// Parse `NAME` or `NAME = default_value` lines.
fn parse_infra_env(attrs: &Attrs, content: &str, span: Span) -> Block {
    let tier = attr_string(attrs, "tier");

    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, value)) = trimmed.split_once('=') {
            let name = name.trim().to_string();
            let value = value.trim().to_string();
            entries.push(EnvEntry {
                name,
                default_value: if value.is_empty() { None } else { Some(value) },
            });
        } else {
            entries.push(EnvEntry {
                name: trimmed.to_string(),
                default_value: None,
            });
        }
    }

    Block::InfraEnv {
        tier,
        entries,
        span,
    }
}

fn parse_health(attrs: &Attrs, span: Span) -> Block {
    Block::Health {
        path: attr_string(attrs, "path"),
        method: attr_string(attrs, "method"),
        grace: attr_string(attrs, "grace"),
        interval: attr_string(attrs, "interval"),
        timeout: attr_string(attrs, "timeout"),
        span,
    }
}

fn parse_concurrency(attrs: &Attrs, span: Span) -> Block {
    Block::Concurrency {
        concurrency_type: attr_string(attrs, "type"),
        hard_limit: attr_u32(attrs, "hard_limit"),
        soft_limit: attr_u32(attrs, "soft_limit"),
        force_https: attr_bool(attrs, "force_https"),
        span,
    }
}

fn parse_cicd(attrs: &Attrs, content: &str, span: Span) -> Block {
    let provider = attr_string(attrs, "provider");

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

    Block::Cicd {
        provider,
        properties,
        span,
    }
}

/// Parse `METHOD /path -> STATUS` lines.
fn parse_smoke(attrs: &Attrs, content: &str, span: Span) -> Block {
    let script = attr_string(attrs, "script");

    let mut checks = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Expected format: `GET /path -> 200`
        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        if parts.len() < 2 {
            continue;
        }
        let method = parts[0].to_string();
        let rest = parts[1];
        if let Some((path, status_str)) = rest.rsplit_once("->") {
            let path = path.trim().to_string();
            if let Ok(expected) = status_str.trim().parse::<u16>() {
                checks.push(SmokeCheck {
                    method,
                    path,
                    expected,
                });
            }
        }
    }

    Block::Smoke {
        script,
        checks,
        span,
    }
}

/// Parse `domain.com (description)` lines.
fn parse_domains(content: &str, span: Span) -> Block {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((domain, rest)) = trimmed.split_once('(') {
            let domain = domain.trim().to_string();
            let description = rest.trim_end_matches(')').trim().to_string();
            entries.push(DomainEntry {
                domain,
                description: if description.is_empty() {
                    None
                } else {
                    Some(description)
                },
            });
        } else {
            entries.push(DomainEntry {
                domain: trimmed.to_string(),
                description: None,
            });
        }
    }

    Block::Domains { entries, span }
}

/// Parse `name (source: ..., features: ...)` or `name (key: value, ...)` lines.
fn parse_crates(content: &str, span: Span) -> Block {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, rest)) = trimmed.split_once('(') {
            let name = name.trim().to_string();
            let meta = rest.trim_end_matches(')').trim();
            let mut source = None;
            let mut features = None;
            // Parse comma-separated key: value pairs inside parens
            for part in meta.split(',') {
                let part = part.trim();
                if let Some(val) = part.strip_prefix("github:").or_else(|| part.strip_prefix("source:")) {
                    // Combine with remaining comma-separated parts that are part of the source
                    source = Some(val.trim().to_string());
                } else if let Some(val) = part.strip_prefix("features:") {
                    features = Some(val.trim().to_string());
                } else if let Some(val) = part.strip_prefix("branch:") {
                    // Append branch info to source
                    if let Some(ref mut s) = source {
                        s.push_str(&format!(", branch: {}", val.trim()));
                    }
                } else if source.is_some() && features.is_none() && !part.contains(':') {
                    // Continue previous source value
                    if let Some(ref mut s) = source {
                        s.push_str(&format!(", {}", part));
                    }
                }
            }
            entries.push(CrateEntry {
                name,
                source,
                features,
            });
        } else {
            entries.push(CrateEntry {
                name: trimmed.to_string(),
                source: None,
                features: None,
            });
        }
    }

    Block::Crates { entries, span }
}

/// Parse `key: value` lines (same as style).
fn parse_deploy_urls(content: &str, span: Span) -> Block {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                entries.push(StyleProperty { key, value });
            }
        }
    }

    Block::DeployUrls { entries, span }
}

/// Parse `name -> /mount/path` lines.
fn parse_volumes(content: &str, span: Span) -> Block {
    let mut entries = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((name, mount)) = trimmed.split_once("->") {
            entries.push(VolumeEntry {
                name: name.trim().to_string(),
                mount: mount.trim().to_string(),
            });
        }
    }

    Block::Volumes { entries, span }
}

// ------------------------------------------------------------------
// App spec block parsers
// ------------------------------------------------------------------

/// Parse `::model[name=Task]` with field definitions like:
/// `- id: uuid [primary, auto]`
/// `- title: string [required, max=200]`
/// `- status: enum(todo, in_progress, done) [default=todo]`
/// `- assignee_id: ref(User) [optional]`
fn parse_model(attrs: &Attrs, content: &str, span: Span) -> Block {
    let name = attr_string(attrs, "name").unwrap_or_default();
    let mut fields = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('-') {
            continue;
        }
        let rest = trimmed.trim_start_matches('-').trim();
        // Split on first `:` to get name and type+constraints
        let Some((field_name, remainder)) = rest.split_once(':') else {
            continue;
        };
        let field_name = field_name.trim().to_string();
        let remainder = remainder.trim();

        // Extract constraints from [...]
        let (type_part, constraints) = if let Some(bracket_start) = remainder.find('[') {
            let type_str = remainder[..bracket_start].trim();
            let bracket_end = remainder.rfind(']').unwrap_or(remainder.len());
            let constraint_str = &remainder[bracket_start + 1..bracket_end];
            (type_str, parse_field_constraints(constraint_str))
        } else {
            (remainder, Vec::new())
        };

        let field_type = parse_model_field_type(type_part);

        fields.push(ModelField {
            name: field_name,
            field_type,
            constraints,
        });
    }

    Block::Model { name, fields, span }
}

fn parse_model_field_type(s: &str) -> ModelFieldType {
    let s = s.trim();
    let lower = s.to_lowercase();

    // Check for enum(variant1, variant2, ...)
    if lower.starts_with("enum(") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        let variants: Vec<String> = inner.split(',').map(|v| v.trim().to_string()).collect();
        return ModelFieldType::Enum(variants);
    }

    // Check for ref(ModelName)
    if lower.starts_with("ref(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        return ModelFieldType::Ref(inner.trim().to_string());
    }

    match lower.as_str() {
        "uuid" => ModelFieldType::Uuid,
        "string" | "str" | "varchar" => ModelFieldType::String,
        "int" | "integer" | "i32" | "i64" => ModelFieldType::Int,
        "float" | "f32" | "f64" | "decimal" | "numeric" => ModelFieldType::Float,
        "bool" | "boolean" => ModelFieldType::Bool,
        "datetime" | "timestamp" | "timestamptz" => ModelFieldType::Datetime,
        "text" => ModelFieldType::Text,
        "json" | "jsonb" => ModelFieldType::Json,
        "money" | "currency" | "price" => ModelFieldType::Money,
        "image" | "photo" | "picture" | "img" => ModelFieldType::Image,
        "email" | "email_address" => ModelFieldType::Email,
        "url" | "uri" | "link" | "href" => ModelFieldType::Url,
        _ => ModelFieldType::String, // default fallback
    }
}

fn parse_field_constraints(s: &str) -> Vec<FieldConstraint> {
    let mut constraints = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let lower = part.to_lowercase();
        if lower == "primary" {
            constraints.push(FieldConstraint::Primary);
        } else if lower == "auto" {
            constraints.push(FieldConstraint::Auto);
        } else if lower == "required" {
            constraints.push(FieldConstraint::Required);
        } else if lower == "optional" {
            constraints.push(FieldConstraint::Optional);
        } else if lower == "unique" {
            constraints.push(FieldConstraint::Unique);
        } else if lower == "index" || lower == "indexed" {
            constraints.push(FieldConstraint::Index);
        } else if let Some(val) = lower.strip_prefix("max=") {
            if let Ok(n) = val.parse::<u32>() {
                constraints.push(FieldConstraint::Max(n));
            }
        } else if let Some(val) = lower.strip_prefix("min=") {
            if let Ok(n) = val.parse::<u32>() {
                constraints.push(FieldConstraint::Min(n));
            }
        } else if let Some(val) = lower.strip_prefix("default=") {
            constraints.push(FieldConstraint::Default(val.to_string()));
        } else if part.starts_with("default=") {
            // Preserve original case for default values
            constraints.push(FieldConstraint::Default(part[8..].to_string()));
        }
    }
    constraints
}

/// Parse `::route[method=GET path="/api/tasks"]`
/// Content lines: `auth: required`, `returns: list(Task)`, `body: Task`
///
/// If a fenced code block (` ``` `rust ... ``` `) appears in the content, the
/// code inside is extracted as the `handler` field.
fn parse_route(attrs: &Attrs, content: &str, span: Span) -> Block {
    let method_str = attr_string(attrs, "method").unwrap_or_else(|| "get".to_string());
    let method = match method_str.to_lowercase().as_str() {
        "get" => HttpMethod::Get,
        "post" => HttpMethod::Post,
        "put" => HttpMethod::Put,
        "patch" => HttpMethod::Patch,
        "delete" => HttpMethod::Delete,
        _ => HttpMethod::Get,
    };
    let path = attr_string(attrs, "path").unwrap_or_default();

    let mut auth = None;
    let mut returns = None;
    let mut body = None;
    let mut handler = None;
    let mut extra_lines = Vec::new();

    // Detect fenced code block for handler extraction.
    let mut in_fence = false;
    let mut fence_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if !in_fence && trimmed.starts_with("```") {
            in_fence = true;
            continue;
        }
        if in_fence {
            if trimmed.starts_with("```") {
                in_fence = false;
                handler = Some(fence_lines.join("\n"));
                fence_lines.clear();
            } else {
                fence_lines.push(line);
            }
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim().to_string();
            match key.as_str() {
                "auth" => auth = Some(value),
                "returns" => returns = Some(value),
                "body" => body = Some(value),
                _ => extra_lines.push(trimmed.to_string()),
            }
        } else {
            extra_lines.push(trimmed.to_string());
        }
    }

    Block::Route {
        method,
        path,
        auth,
        returns,
        body,
        handler,
        content: extra_lines.join("\n"),
        span,
    }
}

/// Parse `::auth[provider=email]`
/// Content lines: `session: cookie`, `roles: admin, member`, `default_role: member`
fn parse_auth(attrs: &Attrs, content: &str, span: Span) -> Block {
    let provider_str = attr_string(attrs, "provider").unwrap_or_else(|| "email".to_string());
    let provider = match provider_str.to_lowercase().as_str() {
        "email" => AuthProvider::Email,
        "oauth" => AuthProvider::OAuth,
        "api-key" | "api_key" | "apikey" => AuthProvider::ApiKey,
        "token" => AuthProvider::Token,
        _ => AuthProvider::Email,
    };

    let mut session = None;
    let mut roles = Vec::new();
    let mut default_role = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim();
            match key.as_str() {
                "session" => session = Some(value.to_string()),
                "roles" => {
                    roles = value.split(',').map(|r| r.trim().to_string()).filter(|r| !r.is_empty()).collect();
                }
                "default_role" | "default-role" => default_role = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Block::Auth {
        provider,
        session,
        roles,
        default_role,
        span,
    }
}

/// Parse `::binding[source="/api/tasks" target="#task-list"]`
/// Content lines: `on_create: refresh`, `on_update: patch`
fn parse_binding(attrs: &Attrs, content: &str, span: Span) -> Block {
    let source = attr_string(attrs, "source").unwrap_or_default();
    let target = attr_string(attrs, "target").unwrap_or_default();

    let mut events = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                events.push(BindingEvent {
                    event: key,
                    action: value,
                });
            }
        }
    }

    Block::Binding {
        source,
        target,
        events,
        span,
    }
}

// ------------------------------------------------------------------
// Row block — compact list item with icon, title, description
// ------------------------------------------------------------------

fn parse_row(attrs: &Attrs, content: &str, span: Span) -> Block {
    let icon = attr_string(attrs, "icon").unwrap_or_else(|| "doc".to_string());
    let href = attr_string(attrs, "href");
    let state = match attr_string(attrs, "state").as_deref() {
        Some("loading") => RowState::Loading,
        Some("empty") => RowState::Empty,
        _ => RowState::Default,
    };

    // Inline attrs take priority; content-based parsing fills in the rest.
    let attr_title = attr_string(attrs, "title");
    let attr_description = attr_string(attrs, "description");

    // `action:` lines declare per-row actions and are consumed BEFORE the
    // positional title/description take, so an action line is never
    // swallowed as row copy. `action: Label | action_string`; without a
    // pipe the label doubles as the action string.
    let mut actions = Vec::new();
    let mut positional = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix("action:") {
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }
            let (label, action) = match rest.split_once('|') {
                Some((l, a)) if !l.trim().is_empty() && !a.trim().is_empty() => {
                    (l.trim().to_string(), a.trim().to_string())
                }
                _ => (rest.to_string(), rest.to_string()),
            };
            actions.push(RowAction { label, action });
        } else {
            positional.push(line);
        }
    }
    let mut lines = positional.into_iter();
    let title = attr_title.unwrap_or_else(|| lines.next().unwrap_or("").trim().to_string());
    let description = attr_description.unwrap_or_else(|| lines.next().unwrap_or("").trim().to_string());

    Block::Row {
        icon,
        title,
        description,
        href,
        state,
        actions,
        span,
    }
}

// ------------------------------------------------------------------
// InfoCard block — knowledge card with facts/steps
// ------------------------------------------------------------------

fn parse_infocard(attrs: &Attrs, content: &str, span: Span) -> Block {
    let intent = attr_string(attrs, "intent").unwrap_or_else(|| "general".to_string());
    let image = attr_string(attrs, "image");
    let state = match attr_string(attrs, "state").as_deref() {
        Some("loading") => RowState::Loading,
        Some("empty") => RowState::Empty,
        _ => RowState::Default,
    };

    // Inline attrs take priority; content-based parsing fills in the rest.
    let attr_title = attr_string(attrs, "title");
    let attr_subtitle = attr_string(attrs, "subtitle");
    let attr_summary = attr_string(attrs, "summary");

    let mut title = attr_title.unwrap_or_default();
    let mut subtitle = attr_subtitle.unwrap_or_default();
    let mut summary = attr_summary.unwrap_or_default();
    let mut facts: Vec<[String; 2]> = Vec::new();
    let mut steps: Vec<String> = Vec::new();
    let mut in_summary = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !title.is_empty() && subtitle.is_empty() {
                // blank line after subtitle area, switch to summary
                in_summary = true;
            }
            continue;
        }

        // Title: first line starting with # or first non-empty line (only if not set by attr)
        if title.is_empty() {
            title = trimmed.trim_start_matches('#').trim().to_string();
            continue;
        }

        // Subtitle: second non-empty line (before blank line, only if not set by attr)
        if subtitle.is_empty() && !in_summary {
            subtitle = trimmed.to_string();
            in_summary = true;
            continue;
        }

        // Steps: lines starting with number + dot
        if trimmed.len() > 2 && trimmed.as_bytes()[0].is_ascii_digit() && trimmed.contains(". ") {
            if let Some((_num, text)) = trimmed.split_once(". ") {
                steps.push(text.trim().to_string());
                continue;
            }
        }

        // Facts: lines with "Key: Value"
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() && key.len() < 30 {
                facts.push([key.to_string(), value.to_string()]);
                continue;
            }
        }

        // Otherwise it's summary text (only if not set by attr)
        if summary.is_empty() {
            summary.push_str(trimmed);
        } else {
            summary.push(' ');
            summary.push_str(trimmed);
        }
    }

    Block::InfoCard {
        intent,
        title,
        subtitle,
        summary,
        image,
        facts,
        steps,
        state,
        span,
    }
}

// ------------------------------------------------------------------
// App format blocks: schema, use, app-env, app-deploy
// ------------------------------------------------------------------

/// Parse `::schema[name="bookings"]`
/// Content lines: `- field_name (type) constraint1 constraint2 ...`
fn parse_schema(attrs: &Attrs, content: &str, span: Span) -> Block {
    let name = attr_string(attrs, "name").unwrap_or_default();
    let mut fields = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('-') {
            continue;
        }
        let rest = trimmed.trim_start_matches('-').trim();
        if rest.is_empty() {
            continue;
        }

        // Parse: field_name (type) constraint1 constraint2 ...
        if let Some(paren_start) = rest.find('(') {
            let field_name = rest[..paren_start].trim().to_string();
            let after_paren = &rest[paren_start + 1..];
            let paren_end = after_paren.find(')').unwrap_or(after_paren.len());
            let raw_type = after_paren[..paren_end].trim();
            let field_type = parse_schema_field_type(raw_type)
                .unwrap_or(ModelFieldType::String);
            let remainder = if paren_end < after_paren.len() {
                after_paren[paren_end + 1..].trim()
            } else {
                ""
            };
            let constraints: Vec<FieldConstraint> = remainder
                .split_whitespace()
                .filter_map(|s| parse_schema_constraint(s).ok())
                .collect();
            fields.push(SchemaField {
                name: field_name,
                field_type,
                constraints,
            });
        } else {
            // No type in parens — treat the whole thing as a name with no type
            let parts: Vec<&str> = rest.split_whitespace().collect();
            let field_name = parts.first().unwrap_or(&"").to_string();
            let constraints: Vec<FieldConstraint> = parts.iter().skip(1)
                .filter_map(|s| parse_schema_constraint(s).ok())
                .collect();
            fields.push(SchemaField {
                name: field_name,
                field_type: ModelFieldType::String,
                constraints,
            });
        }
    }

    Block::Schema { name, fields, span }
}

pub fn parse_schema_field_type(raw: &str) -> Result<ModelFieldType, crate::error::ParseError> {
    let s = raw.trim();
    let lower = s.to_lowercase();

    // Check for enum:variant1,variant2,...
    if let Some(inner) = lower.strip_prefix("enum:") {
        let variants: Vec<String> = inner.split(',').map(|v| v.trim().to_string()).collect();
        return Ok(ModelFieldType::Enum(variants));
    }

    // Check for ref:ModelName
    if lower.starts_with("ref:") {
        let inner = &s[4..];
        return Ok(ModelFieldType::Ref(inner.trim().to_string()));
    }

    match lower.as_str() {
        "uuid" => Ok(ModelFieldType::Uuid),
        "string" | "str" | "varchar" => Ok(ModelFieldType::String),
        "text" => Ok(ModelFieldType::Text),
        "int" | "integer" | "i64" => Ok(ModelFieldType::Int),
        "float" | "f64" | "double" => Ok(ModelFieldType::Float),
        "bool" | "boolean" => Ok(ModelFieldType::Bool),
        "datetime" | "timestamp" => Ok(ModelFieldType::Datetime),
        "json" | "jsonb" => Ok(ModelFieldType::Json),
        "money" | "cents" | "price" => Ok(ModelFieldType::Money),
        "image" | "img" | "photo" => Ok(ModelFieldType::Image),
        "email" => Ok(ModelFieldType::Email),
        "url" | "uri" | "link" => Ok(ModelFieldType::Url),
        _ => Err(crate::error::ParseError::InvalidAttrs {
            message: format!("unknown schema field type: '{s}'"),
            span: Span::SYNTHETIC,
        }),
    }
}

pub fn parse_schema_constraint(raw: &str) -> Result<FieldConstraint, crate::error::ParseError> {
    let s = raw.trim();
    let lower = s.to_lowercase();

    if let Some(val) = lower.strip_prefix("max:") {
        return val.parse::<u32>()
            .map(FieldConstraint::Max)
            .map_err(|_| crate::error::ParseError::InvalidAttrs {
                message: format!("invalid max value: '{val}'"),
                span: Span::SYNTHETIC,
            });
    }

    if let Some(val) = lower.strip_prefix("min:") {
        return val.parse::<u32>()
            .map(FieldConstraint::Min)
            .map_err(|_| crate::error::ParseError::InvalidAttrs {
                message: format!("invalid min value: '{val}'"),
                span: Span::SYNTHETIC,
            });
    }

    if lower.starts_with("default:") {
        let val = &s[8..];
        return Ok(FieldConstraint::Default(val.to_string()));
    }

    match lower.as_str() {
        "primary" => Ok(FieldConstraint::Primary),
        "auto" => Ok(FieldConstraint::Auto),
        "required" => Ok(FieldConstraint::Required),
        "optional" => Ok(FieldConstraint::Optional),
        "unique" => Ok(FieldConstraint::Unique),
        "index" => Ok(FieldConstraint::Index),
        _ => Err(crate::error::ParseError::InvalidAttrs {
            message: format!("unknown schema constraint: '{s}'"),
            span: Span::SYNTHETIC,
        }),
    }
}

/// Parse `::use` with content lines or inline attrs.
///
/// Content lines: `- reqwest 0.12 [json, rustls-tls]`
/// Inline: `::use[reqwest, lettre, stripe]` — attrs parsed as simple crate names.
fn parse_use(attrs: &Attrs, content: &str, span: Span) -> Block {
    let mut crates = Vec::new();

    // Check for inline attrs: keys are crate names, values are typically Null or String.
    if !attrs.is_empty() && content.trim().is_empty() {
        for (key, _value) in attrs {
            crates.push(CrateDep {
                name: key.clone(),
                version: None,
                features: Vec::new(),
            });
        }
        return Block::Use { crates, span };
    }

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('-') {
            continue;
        }
        let rest = trimmed.trim_start_matches('-').trim();
        if rest.is_empty() {
            continue;
        }

        // Parse: name version [feature1, feature2]
        let (main_part, features) = if let Some(bracket_start) = rest.find('[') {
            let bracket_end = rest.rfind(']').unwrap_or(rest.len());
            let features_str = &rest[bracket_start + 1..bracket_end];
            let feats: Vec<String> = features_str
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect();
            (rest[..bracket_start].trim(), feats)
        } else {
            (rest, Vec::new())
        };

        let parts: Vec<&str> = main_part.split_whitespace().collect();
        let name = parts.first().unwrap_or(&"").to_string();
        let version = parts.get(1).map(|v| v.to_string());

        if !name.is_empty() {
            crates.push(CrateDep {
                name,
                version,
                features,
            });
        }
    }

    Block::Use { crates, span }
}

/// Parse `::app-env` with content lines or inline attrs.
///
/// Content lines: `- STRIPE_KEY * "Stripe secret API key"`
/// Star means required. Quoted string is description.
/// Inline: `::app-env[STRIPE_KEY, SMTP_HOST]`
fn parse_app_env(attrs: &Attrs, content: &str, span: Span) -> Block {
    let mut vars = Vec::new();

    // Inline mode: attrs are variable names.
    if !attrs.is_empty() && content.trim().is_empty() {
        for (key, _value) in attrs {
            vars.push(EnvVar {
                name: key.clone(),
                description: None,
                required: false,
            });
        }
        return Block::AppEnv { vars, span };
    }

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('-') {
            continue;
        }
        let rest = trimmed.trim_start_matches('-').trim();
        if rest.is_empty() {
            continue;
        }

        // Parse: NAME [*] ["description"]
        let mut parts_iter = rest.splitn(2, char::is_whitespace);
        let name = parts_iter.next().unwrap_or("").to_string();
        let remainder = parts_iter.next().unwrap_or("").trim();

        let required = remainder.contains('*');
        let remainder = remainder.replace('*', "").trim().to_string();

        let description = if let Some(start) = remainder.find('"') {
            let after_quote = &remainder[start + 1..];
            let end = after_quote.find('"').unwrap_or(after_quote.len());
            let desc = after_quote[..end].to_string();
            if desc.is_empty() { None } else { Some(desc) }
        } else {
            None
        };

        if !name.is_empty() {
            vars.push(EnvVar {
                name,
                description,
                required,
            });
        }
    }

    Block::AppEnv { vars, span }
}

/// Parse `::app-deploy[region="sjc" scale="2" domain="myapp.surf.new" memory="512mb"]`
/// Also parse content lines as key-value properties.
fn parse_app_deploy(attrs: &Attrs, content: &str, span: Span) -> Block {
    let region = attr_string(attrs, "region");
    let scale = attr_u32(attrs, "scale");
    let domain = attr_string(attrs, "domain");
    let memory = attr_string(attrs, "memory");

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
                properties.push((key, value));
            }
        }
    }

    Block::AppDeploy {
        region,
        scale,
        domain,
        memory,
        properties,
        span,
    }
}

// ------------------------------------------------------------------
// Interactive / application block parsers
// ------------------------------------------------------------------

fn parse_app_shell(attrs: &Attrs, content: &str, span: Span) -> Block {
    let layout = attr_string(attrs, "layout").unwrap_or_else(|| "sidebar-main-panel".to_string());
    let children = parse_page_children(content);
    Block::AppShell {
        layout,
        children,
        span,
    }
}

fn parse_sidebar(attrs: &Attrs, content: &str, span: Span) -> Block {
    let position = attr_string(attrs, "position").unwrap_or_else(|| "left".to_string());
    let collapsible = attr_bool(attrs, "collapsible");
    let width = attr_u32(attrs, "width");
    let children = parse_page_children(content);
    Block::Sidebar {
        position,
        collapsible,
        width,
        children,
        span,
    }
}

fn parse_panel(attrs: &Attrs, content: &str, span: Span) -> Block {
    let position = attr_string(attrs, "position").unwrap_or_else(|| "bottom".to_string());
    let resizable = attr_bool(attrs, "resizable");
    let height = attr_u32(attrs, "height");
    let desktop_only = attr_bool(attrs, "desktop-only");
    let children = parse_page_children(content);
    Block::Panel {
        position,
        resizable,
        height,
        desktop_only,
        children,
        span,
    }
}

fn parse_tab_bar(attrs: &Attrs, content: &str, span: Span) -> Block {
    let active = attr_string(attrs, "active");
    let mut items = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            // Expected format: id "Label" {icon=token role=search}
            let mut rest = rest.trim();
            let mut icon = None;
            let mut role = None;
            if let Some(brace_start) = rest.rfind('{')
                && let Some(brace) = rest[brace_start + 1..].strip_suffix('}')
            {
                // 0.12: strip the brace when ANY known token parsed, not only
                // `icon=` — a role-only brace used to leak into the label.
                let mut parsed_any = false;
                for tok in brace.split_whitespace() {
                    if let Some(v) = tok.strip_prefix("icon=") {
                        icon = Some(v.trim_matches('"').to_string());
                        parsed_any = true;
                    } else if let Some(v) = tok.strip_prefix("role=") {
                        // Carried verbatim; unknown values are never diagnosed.
                        role = Some(v.trim_matches('"').to_string());
                        parsed_any = true;
                    }
                }
                if parsed_any {
                    rest = rest[..brace_start].trim_end();
                }
            }
            if let Some((id, label_part)) = rest.split_once(' ') {
                let label = label_part.trim().trim_matches('"').to_string();
                items.push(TabBarItem {
                    id: id.to_string(),
                    label,
                    icon,
                    role,
                });
            } else {
                items.push(TabBarItem {
                    id: rest.to_string(),
                    label: rest.to_string(),
                    icon,
                    role,
                });
            }
        }
    }
    Block::TabBar {
        active,
        items,
        span,
    }
}

fn parse_tab_content(attrs: &Attrs, content: &str, span: Span) -> Block {
    let tab = attr_string(attrs, "tab").unwrap_or_default();
    let children = parse_page_children(content);
    Block::TabContent {
        tab,
        children,
        span,
    }
}

fn parse_toolbar(attrs: &Attrs, content: &str, span: Span) -> Block {
    // 0.12: optional static title plus a source-bound dynamic title
    // (`title-source=thread.display_name` for the thread screen). The
    // source name is carried verbatim, the `::chat-thread`/`::nav-tree`
    // convention for registry bindings.
    let title = attr_string(attrs, "title");
    let title_source = attr_string(attrs, "title-source");
    let mut items = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            if rest == "separator" {
                items.push(ToolbarItem::Separator);
            } else if rest == "spacer" {
                items.push(ToolbarItem::Spacer);
            } else if let Some(inner) = rest.strip_prefix("button[") {
                let inner = inner.trim_end_matches(']');
                let attrs = crate::attrs::parse_attrs(inner).unwrap_or_default();
                items.push(ToolbarItem::Button {
                    label: attr_string(&attrs, "label"),
                    action: attr_string(&attrs, "action"),
                    icon: attr_string(&attrs, "icon"),
                    style: attr_string(&attrs, "style"),
                    disabled: attr_bool(&attrs, "disabled"),
                });
            } else if let Some(inner) = rest.strip_prefix("badge[") {
                let inner = inner.trim_end_matches(']');
                let attrs = crate::attrs::parse_attrs(inner).unwrap_or_default();
                items.push(ToolbarItem::Badge {
                    value: attr_string(&attrs, "value").unwrap_or_default(),
                    color: attr_string(&attrs, "color"),
                });
            } else if let Some(inner) = rest.strip_prefix("dropdown[") {
                let inner = inner.trim_end_matches(']');
                let attrs = crate::attrs::parse_attrs(inner).unwrap_or_default();
                items.push(ToolbarItem::Dropdown {
                    label: attr_string(&attrs, "label").unwrap_or_default(),
                    options: attr_string(&attrs, "options"),
                    action: attr_string(&attrs, "action"),
                });
            } else if let Some(inner) = rest.strip_prefix("text[") {
                let inner = inner.trim_end_matches(']');
                let attrs = crate::attrs::parse_attrs(inner).unwrap_or_default();
                items.push(ToolbarItem::Text {
                    value: attr_string(&attrs, "value").unwrap_or_default(),
                    editable: attr_bool(&attrs, "editable"),
                    action: attr_string(&attrs, "action"),
                });
            } else if !rest.is_empty() {
                // Plain text item — treat as a button with no action
                items.push(ToolbarItem::Button {
                    label: Some(rest.to_string()),
                    action: None,
                    icon: None,
                    style: None,
                    disabled: false,
                });
            }
        }
    }
    Block::Toolbar {
        title,
        title_source,
        items,
        span,
    }
}

fn parse_drawer(attrs: &Attrs, content: &str, span: Span) -> Block {
    let name = attr_string(attrs, "name").unwrap_or_default();
    let position = attr_string(attrs, "position").unwrap_or_else(|| "right".to_string());
    let width = attr_u32(attrs, "width");
    let trigger = attr_string(attrs, "trigger");
    let children = parse_page_children(content);
    Block::Drawer {
        name,
        position,
        width,
        trigger,
        children,
        span,
    }
}

fn parse_modal(attrs: &Attrs, content: &str, span: Span) -> Block {
    let name = attr_string(attrs, "name").unwrap_or_default();
    let title = attr_string(attrs, "title");
    let children = parse_page_children(content);
    Block::Modal {
        name,
        title,
        children,
        span,
    }
}

fn parse_command_palette(attrs: &Attrs, content: &str, span: Span) -> Block {
    let trigger = attr_string(attrs, "trigger");
    let mut items = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            // Expected: "Label" description="..." action=... icon=... group=...
            if let Some(label_end) = rest.find('"').and_then(|start| {
                rest[start + 1..].find('"').map(|end| start + 1 + end)
            }) {
                let label = rest[1..label_end].to_string();
                let remainder = rest[label_end + 1..].trim();
                let attrs = crate::attrs::parse_attrs(remainder).unwrap_or_default();
                items.push(CommandItem {
                    label,
                    description: attr_string(&attrs, "description"),
                    action: attr_string(&attrs, "action"),
                    icon: attr_string(&attrs, "icon"),
                    group: attr_string(&attrs, "group"),
                });
            }
        }
    }
    Block::CommandPalette {
        trigger,
        items,
        span,
    }
}

fn parse_code_editor(attrs: &Attrs, content: &str, span: Span) -> Block {
    let lang = attr_string(attrs, "lang");
    let source = attr_string(attrs, "source");
    let line_numbers = attr_bool(attrs, "line-numbers");
    Block::CodeEditor {
        lang,
        source,
        line_numbers,
        content: content.to_string(),
        span,
    }
}

fn parse_block_editor(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    Block::BlockEditor { source, span }
}

fn parse_terminal(attrs: &Attrs, span: Span) -> Block {
    let shell = attr_string(attrs, "shell");
    let cwd = attr_string(attrs, "cwd");
    Block::Terminal { shell, cwd, span }
}

fn parse_nav_tree(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    let on_select = attr_string(attrs, "on-select");
    let on_rename = attr_string(attrs, "on-rename");
    let on_delete = attr_string(attrs, "on-delete");
    Block::NavTree {
        source,
        on_select,
        on_rename,
        on_delete,
        span,
    }
}

fn parse_badge(attrs: &Attrs, span: Span) -> Block {
    let value = attr_string(attrs, "value").unwrap_or_default();
    let color = attr_string(attrs, "color");
    Block::Badge { value, color, span }
}

fn parse_suggestion_chips(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    let max = attr_u32(attrs, "max");
    let dismissible = attr_bool(attrs, "dismissible");
    Block::SuggestionChips {
        source,
        max,
        dismissible,
        span,
    }
}

fn parse_chat_thread(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    let on_action = attr_string(attrs, "on-action");
    let on_react = attr_string(attrs, "on-react");
    let on_doc_open = attr_string(attrs, "on-doc-open");
    Block::ChatThread {
        source,
        on_action,
        on_react,
        on_doc_open,
        span,
    }
}

fn parse_recipient_picker(attrs: &Attrs, span: Span) -> Block {
    // Source accepts bare registry names or /-rooted paths, like
    // `::list`/`::search` (the 0.12 registry-source validator).
    let source = attr_string(attrs, "source")
        .and_then(|s| validate_registry_source(&s))
        .unwrap_or_default();
    // Invalid modes fall back to single-select, never a parse failure.
    let mode = attr_string(attrs, "mode")
        .filter(|m| m == "single" || m == "multi")
        .unwrap_or_else(|| "single".to_string());
    let on_submit = attr_string(attrs, "on-submit");
    Block::RecipientPicker {
        source,
        mode,
        on_submit,
        span,
    }
}

fn parse_qr(attrs: &Attrs, span: Span) -> Block {
    // Invalid modes fall back to show, never a parse failure.
    let mode = attr_string(attrs, "mode")
        .filter(|m| m == "show" || m == "scan")
        .unwrap_or_else(|| "show".to_string());
    let on_resolve = attr_string(attrs, "on-resolve");
    Block::Qr {
        mode,
        on_resolve,
        span,
    }
}

fn parse_chat_input_simple(attrs: &Attrs, span: Span) -> Block {
    let placeholder = attr_string(attrs, "placeholder");
    let action = attr_string(attrs, "action");
    Block::ChatInputSimple {
        placeholder,
        action,
        span,
    }
}

fn parse_progress(attrs: &Attrs, content: &str, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    let mut steps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            let (label, status) = if let Some(label) = rest.strip_suffix('\u{2713}') {
                // ✓ = done
                (label.trim().to_string(), "done".to_string())
            } else if let Some(label) = rest.strip_suffix('\u{25CF}') {
                // ● = active
                (label.trim().to_string(), "active".to_string())
            } else if let Some(label) = rest.strip_suffix('\u{25CB}') {
                // ○ = pending
                (label.trim().to_string(), "pending".to_string())
            } else {
                (rest.to_string(), "pending".to_string())
            };
            steps.push(ProgressStep { label, status });
        }
    }
    Block::Progress {
        source,
        steps,
        span,
    }
}

fn parse_log_stream(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    let tail = attr_u32(attrs, "tail");
    Block::LogStream { source, tail, span }
}

fn parse_problem_list(attrs: &Attrs, span: Span) -> Block {
    let source = attr_string(attrs, "source");
    Block::ProblemList { source, span }
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

    // p1-fill-render: a `«FILL: label | default»` marker's own `|` separator
    // must not be mistaken for the table's column delimiter, or the marker
    // splits into two mangled cells and the default is silently dropped.
    #[test]
    fn resolve_data_table_fill_marker_pipe_stays_atomic() {
        let content = "| Item | Price |\n|---|---|\n| Deck | \u{ab}FILL: deck price | $120\u{bb} |";
        let block = unknown("data", Attrs::new(), content);
        match resolve_block(block) {
            Block::Data { headers, rows, .. } => {
                assert_eq!(headers, vec!["Item", "Price"]);
                assert_eq!(rows.len(), 1);
                assert_eq!(
                    rows[0],
                    vec!["Deck", "\u{ab}FILL: deck price | $120\u{bb}"],
                    "the marker's internal `|` must not split the cell"
                );
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
                ("deciders", AttrValue::String("Brady, Ryan".into())),
            ]),
            "Consider options.",
        );
        match resolve_block(block) {
            Block::Decision { deciders, .. } => {
                assert_eq!(deciders, vec!["Brady", "Ryan"]);
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

    // -- Diagram ---------------------------------------------------

    #[test]
    fn resolve_diagram_lowercases_type_and_keeps_content() {
        let block = unknown(
            "diagram",
            attrs(&[
                ("type", AttrValue::String("Architecture".into())),
                ("title", AttrValue::String("System Map".into())),
            ]),
            "web: Web\nweb -> api",
        );
        match resolve_block(block) {
            Block::Diagram {
                diagram_type,
                title,
                content,
                ..
            } => {
                assert_eq!(diagram_type, "architecture");
                assert_eq!(title, Some("System Map".to_string()));
                assert_eq!(content, "web: Web\nweb -> api");
            }
            other => panic!("Expected Diagram, got {other:?}"),
        }
    }

    #[test]
    fn resolve_diagram_missing_type_is_empty_string() {
        let block = unknown("diagram", attrs(&[]), "a -> b");
        match resolve_block(block) {
            Block::Diagram {
                diagram_type,
                title,
                ..
            } => {
                assert_eq!(diagram_type, "");
                assert!(title.is_none());
            }
            other => panic!("Expected Diagram, got {other:?}"),
        }
    }

    #[test]
    fn resolve_diagram_unknown_type_round_trips_raw() {
        // Unknown types stay a String so they survive losslessly.
        let block = unknown(
            "diagram",
            attrs(&[("type", AttrValue::String("flowchart".into()))]),
            "whatever -> body",
        );
        match resolve_block(block) {
            Block::Diagram { diagram_type, content, .. } => {
                assert_eq!(diagram_type, "flowchart");
                assert_eq!(content, "whatever -> body");
            }
            other => panic!("Expected Diagram, got {other:?}"),
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
    fn page_children_fenced_directive_sample_stays_literal() {
        // Regression: a fenced SurfDoc code sample inside a page's content
        // (the exact input parse_page_children resolves) must not have its
        // tolerant `:: name` opener or bare `::` closer parsed as a real
        // directive — the whole fence should survive as one Markdown block.
        let content = "Example usage:\n\n```\n:: hero[title=\"X\"]\nSome text\n::\n```\n\nAfter the sample.";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Markdown { content, .. } => {
                        assert!(
                            content.contains("```\n:: hero[title=\"X\"]\nSome text\n::\n```"),
                            "fence body was mangled: {content:?}"
                        );
                    }
                    other => panic!("Expected Markdown, got {other:?}"),
                }
            }
            other => panic!("Expected Page, got {other:?}"),
        }
    }

    #[test]
    fn page_container_nested_fenced_directive_sample_stays_literal() {
        // Same fenced-sample hazard, but nested one level inside a real
        // container block (exercises scan_container_close's own fence
        // tracking of the tolerant opener, not just parse_page_children's
        // top loop).
        let content = "::callout[type=info]\nExample:\n```\n:: hero[title=\"X\"]\n```\n::";
        let block = unknown(
            "page",
            attrs(&[("route", AttrValue::String("/".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::Page { children, .. } => {
                assert_eq!(children.len(), 1, "children: {children:#?}");
                match &children[0] {
                    Block::Callout { content, .. } => {
                        assert!(
                            content.contains("```\n:: hero[title=\"X\"]\n```"),
                            "nested fence body was mangled: {content:?}"
                        );
                    }
                    other => panic!("Expected Callout, got {other:?}"),
                }
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
    fn resolve_form_action_method_honeypot() {
        let block = unknown(
            "form",
            attrs(&[
                ("action", AttrValue::String("/contact".into())),
                ("method", AttrValue::String("post".into())),
                ("honeypot", AttrValue::Bool(true)),
            ]),
            "- Name (text) *",
        );
        match resolve_block(block) {
            Block::Form { action, method, honeypot, .. } => {
                assert_eq!(action, Some("/contact".to_string()));
                assert_eq!(method, Some("post".to_string()));
                assert!(honeypot);
            }
            other => panic!("Expected Form, got {other:?}"),
        }
    }

    #[test]
    fn resolve_hero_image_alt_and_layout() {
        let block = unknown(
            "hero",
            attrs(&[
                ("image", AttrValue::String("/logo.png".into())),
                ("image-alt", AttrValue::String("CloudSurf".into())),
                ("layout", AttrValue::String("stacked".into())),
            ]),
            "# CloudSurf\n\nInnovate",
        );
        match resolve_block(block) {
            Block::Hero { image, image_alt, layout, .. } => {
                assert_eq!(image, Some("/logo.png".to_string()));
                assert_eq!(image_alt, Some("CloudSurf".to_string()));
                assert_eq!(layout, Some("stacked".to_string()));
            }
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    #[test]
    fn resolve_hero_transparent_flag() {
        let block = unknown(
            "hero",
            attrs(&[("transparent", AttrValue::Bool(true))]),
            "# CloudSurf",
        );
        match resolve_block(block) {
            Block::Hero { transparent, .. } => assert!(transparent),
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    #[test]
    fn resolve_banner_headline_subtitle_buttons() {
        let content = "# Building something custom?\n\nTell us what you need.\n\n[Start a project](/contact){primary}";
        let block = unknown("banner", Attrs::new(), content);
        match resolve_block(block) {
            Block::Banner { headline, subtitle, buttons, .. } => {
                assert_eq!(headline, Some("Building something custom?".to_string()));
                assert_eq!(subtitle, Some("Tell us what you need.".to_string()));
                assert_eq!(buttons.len(), 1);
                assert_eq!(buttons[0].label, "Start a project");
                assert_eq!(buttons[0].href, "/contact");
                assert!(buttons[0].primary);
            }
            other => panic!("Expected Banner, got {other:?}"),
        }
    }

    #[test]
    fn resolve_product_grid_grouped() {
        let content = "### Platforms\n- Surf | /surf.png | https://app.surf | Docs, tasks & apps.\n- Wavesite | /ws.png | https://wave.site | Build sites.\n### Tools\n- Mako | /mako.png | https://mako.app.surf | The AI agent.";
        let block = unknown("product-grid", Attrs::new(), content);
        match resolve_block(block) {
            Block::ProductGrid { groups, .. } => {
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0].label, Some("Platforms".to_string()));
                assert_eq!(groups[0].items.len(), 2);
                assert_eq!(groups[0].items[0].name, "Surf");
                assert_eq!(groups[0].items[0].emblem, Some("/surf.png".to_string()));
                assert_eq!(groups[0].items[0].href, "https://app.surf");
                assert_eq!(groups[0].items[0].tagline, Some("Docs, tasks & apps.".to_string()));
                assert_eq!(groups[1].label, Some("Tools".to_string()));
                assert_eq!(groups[1].items.len(), 1);
            }
            other => panic!("Expected ProductGrid, got {other:?}"),
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

    /// Lines consumed into headline/subtitle/buttons must NOT survive in
    /// `content` — native renderers draw `content` verbatim below the typed
    /// fields, so a full-body copy renders the whole hero twice (iOS
    /// duplicate-hero bug, 2026-07-26).
    #[test]
    fn resolve_hero_consumed_lines_leave_no_content() {
        let content = "# Build Something Great\n\nThe fastest way to ship your next project.\n\n[Get Started](/signup){primary}\n[Learn More](/docs)";
        let block = unknown("hero", Attrs::new(), content);
        match resolve_block(block) {
            Block::Hero { content, .. } => assert_eq!(content, ""),
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    /// Free-form lines the field extraction does not claim (text before the
    /// headline, or after the buttons) still flow through as `content`.
    #[test]
    fn resolve_hero_keeps_unconsumed_lines_as_content() {
        let content = "# Title\n\nSubtitle line.\n\n[Go](/go){primary}\n\nTrusted by 40 surf schools.";
        let block = unknown("hero", Attrs::new(), content);
        match resolve_block(block) {
            Block::Hero {
                headline,
                subtitle,
                buttons,
                content,
                ..
            } => {
                assert_eq!(headline, Some("Title".to_string()));
                assert_eq!(subtitle, Some("Subtitle line.".to_string()));
                assert_eq!(buttons.len(), 1);
                assert_eq!(content, "Trusted by 40 surf schools.");
            }
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    /// Banner reuses the hero grammar and consumes every non-blank line, so
    /// its `content` must always come back empty (same duplicate-render bug).
    #[test]
    fn resolve_banner_leaves_no_content() {
        let content = "# Ready to ride?\n\nJoin today.\n\n[Sign Up](/signup){primary}";
        let block = unknown("banner", Attrs::new(), content);
        match resolve_block(block) {
            Block::Banner { content, .. } => assert_eq!(content, ""),
            other => panic!("Expected Banner, got {other:?}"),
        }
    }

    #[test]
    fn resolve_hero_button_external_flags() {
        let content = "# H\n\n[Signup](https://x.com){primary external}\n[Docs](https://y.com){external}\n[Home](/){primary}";
        let block = unknown("hero", Attrs::new(), content);
        match resolve_block(block) {
            Block::Hero { buttons, .. } => {
                assert_eq!(buttons.len(), 3);
                assert!(buttons[0].primary && buttons[0].external);
                assert!(!buttons[1].primary && buttons[1].external);
                assert!(buttons[2].primary && !buttons[2].external);
            }
            other => panic!("Expected Hero, got {other:?}"),
        }
    }

    #[test]
    fn resolve_product_grid_linkless_card() {
        let content = "- Live | | /live | Ships\n- Soon | | - | Not public yet";
        let block = unknown("product-grid", Attrs::new(), content);
        match resolve_block(block) {
            Block::ProductGrid { groups, .. } => {
                let items = &groups[0].items;
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].href, "/live");
                assert_eq!(items[1].href, "-");
            }
            other => panic!("Expected ProductGrid, got {other:?}"),
        }
    }

    #[test]
    fn resolve_hero_button_dot_primary() {
        // The markdown class form `{.primary}` must mark the button primary,
        // same as the bare `{primary}` flag.
        let content = "# Title\n\nSub.\n\n[Start free](/signup){.primary}\n[Read the spec](/spec)";
        let block = unknown("hero", Attrs::new(), content);
        match resolve_block(block) {
            Block::Hero { buttons, .. } => {
                assert_eq!(buttons.len(), 2);
                assert_eq!(buttons[0].label, "Start free");
                assert!(buttons[0].primary, "{{.primary}} should be primary");
                assert!(!buttons[1].primary);
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
    fn resolve_features_h2_headings() {
        // `## ` headings (not just `### `) must start cards — mirrors parse_steps.
        let content = "## Typed\nEvery block is validated.\n## Portable\nAn open format you own.\n## AI-native\nSpecs that track themselves.";
        let block = unknown("features", Attrs::new(), content);
        match resolve_block(block) {
            Block::Features { cards, .. } => {
                assert_eq!(cards.len(), 3);
                assert_eq!(cards[0].title, "Typed");
                assert!(cards[0].body.contains("validated"));
                assert_eq!(cards[1].title, "Portable");
                assert_eq!(cards[2].title, "AI-native");
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

    #[test]
    fn resolve_stats_pipe_syntax() {
        // `value | label` pipe syntax must produce items (color defaults to None).
        let content = "- 91 | block types\n- 4 | renderers\n- 0 | lock-in";
        let block = unknown("stats", Attrs::new(), content);
        match resolve_block(block) {
            Block::Stats { items, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0].value, "91");
                assert_eq!(items[0].label, "block types");
                assert!(items[0].color.is_none());
                assert_eq!(items[1].value, "4");
                assert_eq!(items[1].label, "renderers");
                assert_eq!(items[2].value, "0");
                assert_eq!(items[2].label, "lock-in");
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
    fn resolve_before_after_h2_plain_bullets() {
        // `## Before/After` headings, plain bullets (no `| detail`), `---` divider.
        let content =
            "## Before\n- Stale docs\n- Manual updates\n---\n## After\n- Living specs\n- Auto-tracked";
        let block = unknown(
            "before-after",
            attrs(&[("transition", AttrValue::String("with SurfDoc".into()))]),
            content,
        );
        match resolve_block(block) {
            Block::BeforeAfter {
                before_items,
                after_items,
                ..
            } => {
                assert_eq!(before_items.len(), 2);
                assert_eq!(before_items[0].label, "Stale docs");
                assert_eq!(before_items[0].detail, "");
                assert_eq!(after_items.len(), 2);
                assert_eq!(after_items[0].label, "Living specs");
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

    #[test]
    fn resolve_product_card_attrs() {
        // Title/subtitle/CTA as attrs; body + features from content.
        let content = "For growing software teams.\n- 5 seats included\n- 10 workspaces";
        let block = unknown(
            "product-card",
            attrs(&[
                ("title", AttrValue::String("Team".into())),
                ("subtitle", AttrValue::String("$6.99/seat".into())),
                ("badge", AttrValue::String("Popular".into())),
                ("cta-label", AttrValue::String("Start trial".into())),
                ("cta-href", AttrValue::String("/signup".into())),
            ]),
            content,
        );
        match resolve_block(block) {
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
                assert_eq!(title, "Team");
                assert_eq!(subtitle, Some("$6.99/seat".to_string()));
                assert_eq!(badge, Some("Popular".to_string()));
                assert!(body.contains("growing software teams"));
                assert_eq!(features.len(), 2);
                assert_eq!(features[0], "5 seats included");
                assert_eq!(cta_label, Some("Start trial".to_string()));
                assert_eq!(cta_href, Some("/signup".to_string()));
            }
            other => panic!("Expected ProductCard, got {other:?}"),
        }
    }

    // ---- App description block tests ----

    #[test]
    fn resolve_list_with_filters() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/tasks".into())),
            ("display", AttrValue::String("card".into())),
        ]);
        let content = "## {= title =}\nStatus: {= status =}\n\nfilter: status\nfilter: priority\nsort: created_at desc";
        let block = unknown("list", a, content);
        match resolve_block(block) {
            Block::List {
                source,
                display,
                filters,
                sort,
                item_template,
                ..
            } => {
                assert_eq!(source, "/api/tasks");
                assert_eq!(display, ListDisplay::Card);
                assert_eq!(filters.len(), 2);
                assert_eq!(filters[0].field, "status");
                assert_eq!(filters[1].field, "priority");
                assert!(sort.is_some());
                let s = sort.unwrap();
                assert_eq!(s.field, "created_at");
                assert!(s.descending);
                assert!(item_template.contains("{= title =}"));
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn resolve_board() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/tasks/board".into())),
        ]);
        let content = "columns: To Do | In Progress | Done\ncard-template: {= title =} @{= assignee =}";
        let block = unknown("board", a, content);
        match resolve_block(block) {
            Block::Board {
                source,
                columns,
                card_template,
                ..
            } => {
                assert_eq!(source, "/api/tasks/board");
                assert_eq!(columns, vec!["To Do", "In Progress", "Done"]);
                assert_eq!(card_template.as_deref(), Some("{= title =} @{= assignee =}"));
            }
            other => panic!("Expected Board, got {other:?}"),
        }
    }

    #[test]
    fn resolve_action_form() {
        let a = attrs(&[
            ("method", AttrValue::String("post".into())),
            ("target", AttrValue::String("/api/tasks".into())),
            ("label", AttrValue::String("Add Task".into())),
        ]);
        let content = "- Name (text) *\n- Priority (select: Low | Medium | High)";
        let block = unknown("action", a, content);
        match resolve_block(block) {
            Block::Action {
                method,
                target,
                label,
                fields,
                ..
            } => {
                assert_eq!(method, HttpMethod::Post);
                assert_eq!(target, "/api/tasks");
                assert_eq!(label, "Add Task");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].label, "Name");
                assert!(fields[0].required);
                assert_eq!(fields[0].field_type, FormFieldType::Text);
                assert_eq!(fields[1].label, "Priority");
                assert_eq!(fields[1].field_type, FormFieldType::Select);
                assert_eq!(fields[1].options, vec!["Low", "Medium", "High"]);
            }
            other => panic!("Expected Action, got {other:?}"),
        }
    }

    #[test]
    fn resolve_filter_bar() {
        let a = attrs(&[
            ("target", AttrValue::String("#task-board".into())),
        ]);
        let content = "- Status (select: All | To Do | In Progress | Done)\n- Priority (select: All | Low | Medium | High)";
        let block = unknown("filter-bar", a, content);
        match resolve_block(block) {
            Block::FilterBar {
                target_selector,
                fields,
                ..
            } => {
                assert_eq!(target_selector, "#task-board");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].label, "Status");
                assert_eq!(fields[0].options, vec!["All", "To Do", "In Progress", "Done"]);
                assert_eq!(fields[1].label, "Priority");
                assert_eq!(fields[1].name, "priority");
            }
            other => panic!("Expected FilterBar, got {other:?}"),
        }
    }

    #[test]
    fn resolve_search() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/research/search".into())),
            ("placeholder", AttrValue::String("Search articles...".into())),
        ]);
        let block = unknown("search", a, "");
        match resolve_block(block) {
            Block::Search {
                source,
                placeholder,
                ..
            } => {
                assert_eq!(source, "/api/research/search");
                assert_eq!(placeholder.as_deref(), Some("Search articles..."));
            }
            other => panic!("Expected Search, got {other:?}"),
        }
    }

    #[test]
    fn resolve_dashboard_with_refresh() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/grow/metrics".into())),
            ("refresh", AttrValue::Number(60.0)),
        ]);
        let block = unknown("dashboard", a, "");
        match resolve_block(block) {
            Block::Dashboard {
                source,
                refresh,
                ..
            } => {
                assert_eq!(source, "/api/grow/metrics");
                assert_eq!(refresh, Some(60));
            }
            other => panic!("Expected Dashboard, got {other:?}"),
        }
    }

    #[test]
    fn resolve_chat_input() {
        let a = attrs(&[
            ("action", AttrValue::String("/chat/message".into())),
            ("placeholder", AttrValue::String("Ask anything...".into())),
        ]);
        let content = "modes: Research | Plan | Build";
        let block = unknown("chat-input", a, content);
        match resolve_block(block) {
            Block::ChatInput {
                action,
                placeholder,
                modes,
                ..
            } => {
                assert_eq!(action, "/chat/message");
                assert_eq!(placeholder.as_deref(), Some("Ask anything..."));
                assert_eq!(modes, vec!["Research", "Plan", "Build"]);
            }
            other => panic!("Expected ChatInput, got {other:?}"),
        }
    }

    #[test]
    fn resolve_feed_streaming() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/chat/messages".into())),
            ("stream", AttrValue::Bool(true)),
        ]);
        let block = unknown("feed", a, "");
        match resolve_block(block) {
            Block::Feed {
                source,
                stream,
                ..
            } => {
                assert_eq!(source, "/api/chat/messages");
                assert!(stream);
            }
            other => panic!("Expected Feed, got {other:?}"),
        }
    }

    #[test]
    fn resolve_editor() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/docs/123".into())),
            ("lang", AttrValue::String("surfdoc".into())),
            ("preview", AttrValue::Bool(true)),
        ]);
        let block = unknown("editor", a, "");
        match resolve_block(block) {
            Block::Editor {
                source,
                lang,
                preview,
                ..
            } => {
                assert_eq!(source.as_deref(), Some("/api/docs/123"));
                assert_eq!(lang.as_deref(), Some("surfdoc"));
                assert!(preview);
            }
            other => panic!("Expected Editor, got {other:?}"),
        }
    }

    #[test]
    fn resolve_chart() {
        let a = attrs(&[
            ("type", AttrValue::String("line".into())),
            ("source", AttrValue::String("/api/grow/metrics".into())),
            ("period", AttrValue::String("30d".into())),
        ]);
        let block = unknown("chart", a, "");
        match resolve_block(block) {
            Block::Chart {
                chart_type,
                source,
                period,
                ..
            } => {
                assert_eq!(chart_type, ChartType::Line);
                assert_eq!(source, "/api/grow/metrics");
                assert_eq!(period.as_deref(), Some("30d"));
            }
            other => panic!("Expected Chart, got {other:?}"),
        }
    }

    /// L1: every chart type alias parses to the right `ChartType`.
    #[test]
    fn resolve_chart_all_types() {
        for (s, want) in [
            ("line", ChartType::Line),
            ("bar", ChartType::Bar),
            ("pie", ChartType::Pie),
            ("area", ChartType::Area),
            ("scatter", ChartType::Scatter),
            ("donut", ChartType::Donut),
            ("doughnut", ChartType::Donut),
            ("stacked-bar", ChartType::StackedBar),
            ("stackedbar", ChartType::StackedBar),
            ("radar", ChartType::Radar),
            ("spider", ChartType::Radar),
        ] {
            let a = attrs(&[("type", AttrValue::String(s.into()))]);
            match resolve_block(unknown("chart", a, "")) {
                Block::Chart { chart_type, .. } => assert_eq!(chart_type, want, "type={s}"),
                other => panic!("Expected Chart for {s}, got {other:?}"),
            }
        }
    }

    /// L2: inline pipe-table data parses into aligned categories + series.
    #[test]
    fn resolve_chart_inline_data() {
        let a = attrs(&[
            ("type", AttrValue::String("line".into())),
            ("title", AttrValue::String("Revenue".into())),
        ]);
        let body = "Month | Web | Mobile\nJan | 120 | 80\nFeb | 150 | 95\nMar | 170 | 110";
        match resolve_block(unknown("chart", a, body)) {
            Block::Chart { title, data, .. } => {
                assert_eq!(title.as_deref(), Some("Revenue"));
                let d = data.expect("inline data parsed");
                assert_eq!(d.categories, vec!["Jan", "Feb", "Mar"]);
                assert_eq!(d.series.len(), 2);
                assert_eq!(d.series[0].name, "Web");
                assert_eq!(d.series[0].values, vec![120.0, 150.0, 170.0]);
                assert_eq!(d.series[1].name, "Mobile");
                assert_eq!(d.series[1].values, vec![80.0, 95.0, 110.0]);
            }
            other => panic!("Expected Chart, got {other:?}"),
        }
    }

    /// L2: messy cells ($, %, commas) parse numerically; separator rows skipped.
    #[test]
    fn resolve_chart_data_cleans_cells() {
        let a = attrs(&[("type", AttrValue::String("bar".into()))]);
        let body = "Q | Sales\n|---|---|\nQ1 | $1,200\nQ2 | 95%\nQ3 | n/a";
        match resolve_block(unknown("chart", a, body)) {
            Block::Chart { data, .. } => {
                let d = data.expect("inline data parsed");
                assert_eq!(d.categories, vec!["Q1", "Q2", "Q3"]);
                assert_eq!(d.series[0].values, vec![1200.0, 95.0, 0.0]);
            }
            other => panic!("Expected Chart, got {other:?}"),
        }
    }

    /// L2: an empty body (or <2 columns) leaves `data` None → mount-point fallback.
    #[test]
    fn resolve_chart_missing_data_falls_back() {
        let a = attrs(&[
            ("type", AttrValue::String("pie".into())),
            ("source", AttrValue::String("/api/metrics".into())),
        ]);
        match resolve_block(unknown("chart", a, "")) {
            Block::Chart { data, source, .. } => {
                assert!(data.is_none(), "empty body must not produce inline data");
                assert_eq!(source, "/api/metrics");
            }
            other => panic!("Expected Chart, got {other:?}"),
        }

        // A single-column body (no series) is also not usable inline data.
        let a = attrs(&[("type", AttrValue::String("pie".into()))]);
        match resolve_block(unknown("chart", a, "Label\nA\nB")) {
            Block::Chart { data, .. } => assert!(data.is_none()),
            other => panic!("Expected Chart, got {other:?}"),
        }
    }

    #[test]
    fn resolve_split_pane() {
        let a = attrs(&[
            ("ratio", AttrValue::String("60:40".into())),
        ]);
        let block = unknown("split-pane", a, "");
        match resolve_block(block) {
            Block::SplitPane { ratio, .. } => {
                assert_eq!(ratio, "60:40");
            }
            other => panic!("Expected SplitPane, got {other:?}"),
        }
    }

    #[test]
    fn source_validation_blocks_javascript() {
        let a = attrs(&[
            ("source", AttrValue::String("javascript:alert(1)".into())),
        ]);
        let block = unknown("list", a, "");
        match resolve_block(block) {
            Block::List { source, .. } => {
                assert!(source.is_empty(), "javascript: URLs must be rejected");
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn source_validation_blocks_external_url() {
        let a = attrs(&[
            ("source", AttrValue::String("https://evil.com/steal".into())),
        ]);
        let block = unknown("search", a, "");
        match resolve_block(block) {
            Block::Search { source, .. } => {
                assert!(source.is_empty(), "external URLs must be rejected");
            }
            other => panic!("Expected Search, got {other:?}"),
        }
    }

    #[test]
    fn source_validation_blocks_path_traversal() {
        let a = attrs(&[
            ("source", AttrValue::String("/api/../../../etc/passwd".into())),
        ]);
        let block = unknown("feed", a, "");
        match resolve_block(block) {
            Block::Feed { source, .. } => {
                assert!(source.is_empty(), "path traversal must be rejected");
            }
            other => panic!("Expected Feed, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_bare_name_survives_on_list() {
        let a = attrs(&[("source", AttrValue::String("conversations".into()))]);
        match resolve_block(unknown("list", a, "")) {
            Block::List { source, .. } => assert_eq!(source, "conversations"),
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_dotted_name_survives_on_list() {
        let a = attrs(&[("source", AttrValue::String("chat.thread".into()))]);
        match resolve_block(unknown("list", a, "")) {
            Block::List { source, .. } => assert_eq!(source, "chat.thread"),
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_bare_name_survives_on_search() {
        let a = attrs(&[("source", AttrValue::String("userSearch".into()))]);
        match resolve_block(unknown("search", a, "")) {
            Block::Search { source, .. } => assert_eq!(source, "userSearch"),
            other => panic!("Expected Search, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_dotted_name_survives_on_search() {
        let a = attrs(&[("source", AttrValue::String("chat.thread".into()))]);
        match resolve_block(unknown("search", a, "")) {
            Block::Search { source, .. } => assert_eq!(source, "chat.thread"),
            other => panic!("Expected Search, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_still_blocks_traversal_on_list() {
        let a = attrs(&[("source", AttrValue::String("/api/../secrets".into()))]);
        match resolve_block(unknown("list", a, "")) {
            Block::List { source, .. } => {
                assert!(source.is_empty(), "path traversal must be rejected");
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_still_blocks_external_url_on_list() {
        let a = attrs(&[("source", AttrValue::String("https://evil.com/x".into()))]);
        match resolve_block(unknown("list", a, "")) {
            Block::List { source, .. } => {
                assert!(source.is_empty(), "external URLs must be rejected");
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn registry_source_still_blocks_scheme_and_relative_traversal() {
        for bad in ["javascript:alert(1)", "data:text/html,x", "../etc/passwd", "//evil.com/x"] {
            let a = attrs(&[("source", AttrValue::String((*bad).into()))]);
            match resolve_block(unknown("search", a, "")) {
                Block::Search { source, .. } => {
                    assert!(source.is_empty(), "{bad} must be rejected");
                }
                other => panic!("Expected Search, got {other:?}"),
            }
        }
    }

    #[test]
    fn parse_list_from_full_surf() {
        let source = "::list[source=\"/api/tasks\" display=card]\n## {= title =}\nfilter: status\nsort: created_at desc\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        let has_list = result.doc.blocks.iter().any(|b| matches!(b, Block::List { .. }));
        assert!(has_list, "Should contain a List block");
    }

    #[test]
    fn parse_list_stream_and_on_select() {
        let source = "::list[source=conversations display=compact stream=conversation_updated on-select=openThread]\n{= display_name =}\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        match &result.doc.blocks[0] {
            Block::List { source, stream, on_select, .. } => {
                assert_eq!(source, "conversations");
                assert_eq!(stream.as_deref(), Some("conversation_updated"));
                assert_eq!(on_select.as_deref(), Some("openThread"));
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn parse_list_without_stream_or_on_select_stays_none() {
        let source = "::list[source=\"/api/tasks\" display=card]\n{= title =}\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::List { stream, on_select, .. } => {
                assert_eq!(*stream, None);
                assert_eq!(*on_select, None);
            }
            other => panic!("Expected List, got {other:?}"),
        }
    }

    #[test]
    fn parse_board_from_full_surf() {
        let source = "::board[source=\"/api/tasks/board\"]\ncolumns: To Do | In Progress | Done\n::";
        let result = crate::parse(source);
        let has_board = result.doc.blocks.iter().any(|b| matches!(b, Block::Board { .. }));
        assert!(has_board, "Should contain a Board block");
    }

    #[test]
    fn parse_action_from_full_surf() {
        let source = "::action[method=post target=\"/api/tasks\" label=\"Add Task\"]\n- Name (text) *\n- Priority (select: Low | Medium | High)\n::";
        let result = crate::parse(source);
        let has_action = result.doc.blocks.iter().any(|b| matches!(b, Block::Action { .. }));
        assert!(has_action, "Should contain an Action block");
    }

    #[test]
    fn parse_filter_bar_from_full_surf() {
        let source = "::filter-bar[target=\"#tasks\"]\n- Status (select: All | To Do | Done)\n::";
        let result = crate::parse(source);
        let has_filter = result.doc.blocks.iter().any(|b| matches!(b, Block::FilterBar { .. }));
        assert!(has_filter, "Should contain a FilterBar block");
    }

    #[test]
    fn parse_booking_services_and_days() {
        let source = "::booking[title=\"Book\" service-label=\"Service\"]\n\
            service: Intro Call | 15 min | Free\n\
            service: Strategy | 45 min | $120\n\
            day: 2026-07-06 | 9:00 AM, 10:00 AM\n\
            day: 2026-07-07 | full\n::";
        let result = crate::parse(source);
        let booking = result
            .doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Booking { services, days, .. } => Some((services, days)),
                _ => None,
            })
            .expect("Should contain a Booking block");
        assert_eq!(booking.0.len(), 2, "two services");
        assert_eq!(booking.0[0].duration.as_deref(), Some("15 min"));
        assert_eq!(booking.1.len(), 2, "two days");
        assert_eq!(booking.1[0].slots.len(), 2, "first day has two slots");
        assert!(booking.1[1].slots.is_empty(), "`full` day has no slots");
    }

    #[test]
    fn parse_store_items_and_categories() {
        let source = "::store[title=\"Shop\" currency=\"$\"]\n\
            category: Ceramics\n\
            item: Mug | 24 | Hand-thrown | Bestseller\n\
            category: Home\n\
            item: Throw | 89 | Wool\n::";
        let result = crate::parse(source);
        let items = result
            .doc
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Store { items, .. } => Some(items),
                _ => None,
            })
            .expect("Should contain a Store block");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category.as_deref(), Some("Ceramics"));
        assert_eq!(items[0].price, "24");
        assert_eq!(items[0].badge.as_deref(), Some("Bestseller"));
        assert_eq!(items[1].category.as_deref(), Some("Home"));
    }

    #[test]
    fn parse_row_block() {
        let source = "::row[icon=sparkle, href=\"/wiki/test\"]\nTest Title\nA description\n::";
        let result = crate::parse(source);
        let has_row = result.doc.blocks.iter().any(|b| matches!(b, Block::Row { .. }));
        assert!(has_row, "Should contain a Row block");
    }

    #[test]
    fn parse_row_loading_state() {
        let source = "::row[icon=book, state=loading]\n::\n";
        let result = crate::parse(source);
        let has_row = result.doc.blocks.iter().any(|b| matches!(b, Block::Row { state: RowState::Loading, .. }));
        assert!(has_row, "Should contain a Row block with loading state");
    }

    #[test]
    fn parse_row_actions_not_swallowed_as_copy() {
        let source = "::row[icon=doc]\naction: Accept | invoke:contacts.accept\nJordan Lee\n@jordan\naction: Deny | mutate:contacts.deny:id\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Row { title, description, actions, .. } => {
                assert_eq!(title, "Jordan Lee", "action lines must not be taken as title");
                assert_eq!(description, "@jordan", "action lines must not be taken as description");
                assert_eq!(actions.len(), 2);
                assert_eq!(actions[0].label, "Accept");
                assert_eq!(actions[0].action, "invoke:contacts.accept");
                assert_eq!(actions[1].label, "Deny");
                assert_eq!(actions[1].action, "mutate:contacts.deny:id");
            }
            other => panic!("Expected Row, got {other:?}"),
        }
    }

    #[test]
    fn parse_row_action_without_pipe_uses_label_as_action() {
        let source = "::row[icon=doc]\nJordan Lee\n@jordan\naction: remove\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Row { actions, .. } => {
                assert_eq!(actions.len(), 1);
                assert_eq!(actions[0].label, "remove");
                assert_eq!(actions[0].action, "remove");
            }
            other => panic!("Expected Row, got {other:?}"),
        }
    }

    #[test]
    fn parse_row_without_actions_stays_empty() {
        let source = "::row[icon=doc]\nTitle\nDesc\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Row { actions, .. } => assert!(actions.is_empty()),
            other => panic!("Expected Row, got {other:?}"),
        }
    }

    #[test]
    fn parse_infocard_block() {
        let source = "::infocard[intent=who, image=\"https://example.com/img.jpg\"]\n# Morgan Freeman\nAmerican actor\n\nSummary text here.\n\nBorn: June 1, 1937\nKnown for: Shawshank\n::";
        let result = crate::parse(source);
        let has_card = result.doc.blocks.iter().any(|b| matches!(b, Block::InfoCard { .. }));
        assert!(has_card, "Should contain an InfoCard block");
    }

    // -- Route with handler ------------------------------------------

    #[test]
    fn resolve_route_without_handler() {
        let a = attrs(&[
            ("method", AttrValue::String("GET".into())),
            ("path", AttrValue::String("/api/tasks".into())),
        ]);
        let block = unknown("route", a, "auth: required\nreturns: list(Task)");
        match resolve_block(block) {
            Block::Route { method, path, auth, returns, handler, .. } => {
                assert_eq!(method, HttpMethod::Get);
                assert_eq!(path, "/api/tasks");
                assert_eq!(auth, Some("required".to_string()));
                assert_eq!(returns, Some("list(Task)".to_string()));
                assert!(handler.is_none());
            }
            other => panic!("Expected Route, got {other:?}"),
        }
    }

    #[test]
    fn resolve_route_with_handler() {
        let a = attrs(&[
            ("method", AttrValue::String("POST".into())),
            ("path", AttrValue::String("/api/book".into())),
        ]);
        let content = "auth: required\n```rust\nasync fn handle(db: &PgPool) -> Result<Redirect> {\n    Ok(Redirect::to(\"/\"))\n}\n```";
        let block = unknown("route", a, content);
        match resolve_block(block) {
            Block::Route { method, path, auth, handler, .. } => {
                assert_eq!(method, HttpMethod::Post);
                assert_eq!(path, "/api/book");
                assert_eq!(auth, Some("required".to_string()));
                assert!(handler.is_some());
                let h = handler.unwrap();
                assert!(h.contains("async fn handle"));
                assert!(h.contains("Redirect::to"));
            }
            other => panic!("Expected Route, got {other:?}"),
        }
    }

    #[test]
    fn parse_route_with_handler_from_full_surf() {
        let source = "::route[method=POST path=\"/api/book\"]\nauth: required\n```rust\nasync fn handle() -> Result<()> { Ok(()) }\n```\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        let route = result.doc.blocks.iter().find(|b| matches!(b, Block::Route { .. }));
        assert!(route.is_some(), "Should contain a Route block");
        if let Some(Block::Route { handler, .. }) = route {
            assert!(handler.is_some(), "Route should have a handler");
        }
    }

    // -- Schema ------------------------------------------------------

    #[test]
    fn resolve_schema_basic() {
        let a = attrs(&[("name", AttrValue::String("bookings".into()))]);
        let content = "- id (uuid) primary\n- name (text) required\n- email (text) required unique\n- slot_id (uuid) required\n- created_at (timestamp) default:now()";
        let block = unknown("schema", a, content);
        match resolve_block(block) {
            Block::Schema { name, fields, .. } => {
                assert_eq!(name, "bookings");
                assert_eq!(fields.len(), 5);
                assert_eq!(fields[0].name, "id");
                assert_eq!(fields[0].field_type, ModelFieldType::Uuid);
                assert_eq!(fields[0].constraints, vec![FieldConstraint::Primary]);
                assert_eq!(fields[2].name, "email");
                assert_eq!(fields[2].constraints, vec![FieldConstraint::Required, FieldConstraint::Unique]);
                assert_eq!(fields[4].field_type, ModelFieldType::Datetime);
                assert_eq!(fields[4].constraints, vec![FieldConstraint::Default("now()".to_string())]);
            }
            other => panic!("Expected Schema, got {other:?}"),
        }
    }

    #[test]
    fn parse_schema_field_type_base_types() {
        assert_eq!(parse_schema_field_type("uuid").unwrap(), ModelFieldType::Uuid);
        assert_eq!(parse_schema_field_type("string").unwrap(), ModelFieldType::String);
        assert_eq!(parse_schema_field_type("text").unwrap(), ModelFieldType::Text);
        assert_eq!(parse_schema_field_type("int").unwrap(), ModelFieldType::Int);
        assert_eq!(parse_schema_field_type("float").unwrap(), ModelFieldType::Float);
        assert_eq!(parse_schema_field_type("bool").unwrap(), ModelFieldType::Bool);
        assert_eq!(parse_schema_field_type("datetime").unwrap(), ModelFieldType::Datetime);
        assert_eq!(parse_schema_field_type("json").unwrap(), ModelFieldType::Json);
        assert_eq!(parse_schema_field_type("money").unwrap(), ModelFieldType::Money);
        assert_eq!(parse_schema_field_type("image").unwrap(), ModelFieldType::Image);
        assert_eq!(parse_schema_field_type("email").unwrap(), ModelFieldType::Email);
        assert_eq!(parse_schema_field_type("url").unwrap(), ModelFieldType::Url);
    }

    #[test]
    fn parse_schema_field_type_aliases() {
        assert_eq!(parse_schema_field_type("str").unwrap(), ModelFieldType::String);
        assert_eq!(parse_schema_field_type("varchar").unwrap(), ModelFieldType::String);
        assert_eq!(parse_schema_field_type("integer").unwrap(), ModelFieldType::Int);
        assert_eq!(parse_schema_field_type("i64").unwrap(), ModelFieldType::Int);
        assert_eq!(parse_schema_field_type("f64").unwrap(), ModelFieldType::Float);
        assert_eq!(parse_schema_field_type("double").unwrap(), ModelFieldType::Float);
        assert_eq!(parse_schema_field_type("boolean").unwrap(), ModelFieldType::Bool);
        assert_eq!(parse_schema_field_type("timestamp").unwrap(), ModelFieldType::Datetime);
        assert_eq!(parse_schema_field_type("jsonb").unwrap(), ModelFieldType::Json);
        assert_eq!(parse_schema_field_type("cents").unwrap(), ModelFieldType::Money);
        assert_eq!(parse_schema_field_type("price").unwrap(), ModelFieldType::Money);
        assert_eq!(parse_schema_field_type("img").unwrap(), ModelFieldType::Image);
        assert_eq!(parse_schema_field_type("photo").unwrap(), ModelFieldType::Image);
        assert_eq!(parse_schema_field_type("uri").unwrap(), ModelFieldType::Url);
        assert_eq!(parse_schema_field_type("link").unwrap(), ModelFieldType::Url);
    }

    #[test]
    fn parse_schema_field_type_parameterized() {
        assert_eq!(
            parse_schema_field_type("enum:active,inactive,pending").unwrap(),
            ModelFieldType::Enum(vec!["active".to_string(), "inactive".to_string(), "pending".to_string()])
        );
        assert_eq!(
            parse_schema_field_type("ref:users").unwrap(),
            ModelFieldType::Ref("users".to_string())
        );
    }

    #[test]
    fn parse_schema_field_type_case_insensitive() {
        assert_eq!(parse_schema_field_type("UUID").unwrap(), ModelFieldType::Uuid);
        assert_eq!(parse_schema_field_type("String").unwrap(), ModelFieldType::String);
        assert_eq!(parse_schema_field_type("Bool").unwrap(), ModelFieldType::Bool);
        assert_eq!(parse_schema_field_type("DATETIME").unwrap(), ModelFieldType::Datetime);
    }

    #[test]
    fn parse_schema_field_type_unknown() {
        assert!(parse_schema_field_type("foobar").is_err());
        assert!(parse_schema_field_type("").is_err());
    }

    #[test]
    fn parse_schema_constraint_all() {
        assert_eq!(parse_schema_constraint("primary").unwrap(), FieldConstraint::Primary);
        assert_eq!(parse_schema_constraint("auto").unwrap(), FieldConstraint::Auto);
        assert_eq!(parse_schema_constraint("required").unwrap(), FieldConstraint::Required);
        assert_eq!(parse_schema_constraint("optional").unwrap(), FieldConstraint::Optional);
        assert_eq!(parse_schema_constraint("unique").unwrap(), FieldConstraint::Unique);
        assert_eq!(parse_schema_constraint("index").unwrap(), FieldConstraint::Index);
    }

    #[test]
    fn parse_schema_constraint_parameterized() {
        assert_eq!(parse_schema_constraint("max:255").unwrap(), FieldConstraint::Max(255));
        assert_eq!(parse_schema_constraint("min:1").unwrap(), FieldConstraint::Min(1));
        assert_eq!(parse_schema_constraint("default:now()").unwrap(), FieldConstraint::Default("now()".to_string()));
        assert_eq!(parse_schema_constraint("default:true").unwrap(), FieldConstraint::Default("true".to_string()));
    }

    #[test]
    fn parse_schema_constraint_case_insensitive() {
        assert_eq!(parse_schema_constraint("PRIMARY").unwrap(), FieldConstraint::Primary);
        assert_eq!(parse_schema_constraint("Required").unwrap(), FieldConstraint::Required);
        assert_eq!(parse_schema_constraint("UNIQUE").unwrap(), FieldConstraint::Unique);
    }

    #[test]
    fn parse_schema_constraint_unknown() {
        assert!(parse_schema_constraint("foobar").is_err());
        assert!(parse_schema_constraint("references").is_err());
    }

    #[test]
    fn parse_schema_from_full_surf() {
        let source = "::schema[name=\"users\"]\n- id (uuid) primary\n- name (text) required\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        let has_schema = result.doc.blocks.iter().any(|b| matches!(b, Block::Schema { .. }));
        assert!(has_schema, "Should contain a Schema block");
    }

    // -- Use ---------------------------------------------------------

    #[test]
    fn resolve_use_content_lines() {
        let content = "- reqwest 0.12 [json, rustls-tls]\n- lettre\n- stripe 0.25";
        let block = unknown("use", Attrs::new(), content);
        match resolve_block(block) {
            Block::Use { crates, .. } => {
                assert_eq!(crates.len(), 3);
                assert_eq!(crates[0].name, "reqwest");
                assert_eq!(crates[0].version, Some("0.12".to_string()));
                assert_eq!(crates[0].features, vec!["json", "rustls-tls"]);
                assert_eq!(crates[1].name, "lettre");
                assert!(crates[1].version.is_none());
                assert_eq!(crates[2].name, "stripe");
                assert_eq!(crates[2].version, Some("0.25".to_string()));
            }
            other => panic!("Expected Use, got {other:?}"),
        }
    }

    #[test]
    fn resolve_use_inline_attrs() {
        let a = attrs(&[
            ("reqwest", AttrValue::Null),
            ("lettre", AttrValue::Null),
            ("stripe", AttrValue::Null),
        ]);
        let block = unknown("use", a, "");
        match resolve_block(block) {
            Block::Use { crates, .. } => {
                assert_eq!(crates.len(), 3);
                let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
                assert!(names.contains(&"reqwest"));
                assert!(names.contains(&"lettre"));
                assert!(names.contains(&"stripe"));
            }
            other => panic!("Expected Use, got {other:?}"),
        }
    }

    #[test]
    fn parse_use_from_full_surf() {
        let source = "::use\n- reqwest 0.12 [json]\n- lettre\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        let has_use = result.doc.blocks.iter().any(|b| matches!(b, Block::Use { .. }));
        assert!(has_use, "Should contain a Use block");
    }

    // -- App ---------------------------------------------------------

    #[test]
    fn parse_app_honors_auth_attr() {
        let source = "::app[name=with-auth auth=password]\n::";
        let result = crate::parse(source);
        match result.doc.blocks.iter().find(|b| matches!(b, Block::App { .. })) {
            Some(Block::App { name, auth, .. }) => {
                assert_eq!(name, "with-auth");
                assert_eq!(auth.as_deref(), Some("password"));
            }
            other => panic!("Expected App, got {other:?}"),
        }
    }

    #[test]
    fn parse_app_without_auth_attr_is_none() {
        let source = "::app[name=no-auth port=8080]\n::";
        let result = crate::parse(source);
        match result.doc.blocks.iter().find(|b| matches!(b, Block::App { .. })) {
            Some(Block::App { auth, .. }) => assert!(auth.is_none()),
            other => panic!("Expected App, got {other:?}"),
        }
    }

    // -- AppEnv ------------------------------------------------------

    #[test]
    fn resolve_app_env_content_lines() {
        let content = "- STRIPE_KEY * \"Stripe secret API key\"\n- SMTP_HOST \"Mail server hostname\"\n- DEBUG_MODE";
        let block = unknown("app-env", Attrs::new(), content);
        match resolve_block(block) {
            Block::AppEnv { vars, .. } => {
                assert_eq!(vars.len(), 3);
                assert_eq!(vars[0].name, "STRIPE_KEY");
                assert!(vars[0].required);
                assert_eq!(vars[0].description, Some("Stripe secret API key".to_string()));
                assert_eq!(vars[1].name, "SMTP_HOST");
                assert!(!vars[1].required);
                assert_eq!(vars[1].description, Some("Mail server hostname".to_string()));
                assert_eq!(vars[2].name, "DEBUG_MODE");
                assert!(!vars[2].required);
                assert!(vars[2].description.is_none());
            }
            other => panic!("Expected AppEnv, got {other:?}"),
        }
    }

    #[test]
    fn resolve_app_env_inline_attrs() {
        let a = attrs(&[
            ("STRIPE_KEY", AttrValue::Null),
            ("SMTP_HOST", AttrValue::Null),
        ]);
        let block = unknown("app-env", a, "");
        match resolve_block(block) {
            Block::AppEnv { vars, .. } => {
                assert_eq!(vars.len(), 2);
                let names: Vec<&str> = vars.iter().map(|v| v.name.as_str()).collect();
                assert!(names.contains(&"STRIPE_KEY"));
                assert!(names.contains(&"SMTP_HOST"));
            }
            other => panic!("Expected AppEnv, got {other:?}"),
        }
    }

    #[test]
    fn parse_app_env_from_full_surf() {
        let source = "::app-env\n- STRIPE_KEY * \"API key\"\n- DEBUG_MODE\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        let has_env = result.doc.blocks.iter().any(|b| matches!(b, Block::AppEnv { .. }));
        assert!(has_env, "Should contain an AppEnv block");
    }

    // -- AppDeploy ---------------------------------------------------

    #[test]
    fn resolve_app_deploy_attrs() {
        let a = attrs(&[
            ("region", AttrValue::String("sjc".into())),
            ("scale", AttrValue::Number(2.0)),
            ("domain", AttrValue::String("myapp.surf.new".into())),
            ("memory", AttrValue::String("512mb".into())),
        ]);
        let block = unknown("app-deploy", a, "");
        match resolve_block(block) {
            Block::AppDeploy { region, scale, domain, memory, properties, .. } => {
                assert_eq!(region, Some("sjc".to_string()));
                assert_eq!(scale, Some(2));
                assert_eq!(domain, Some("myapp.surf.new".to_string()));
                assert_eq!(memory, Some("512mb".to_string()));
                assert!(properties.is_empty());
            }
            other => panic!("Expected AppDeploy, got {other:?}"),
        }
    }

    #[test]
    fn resolve_app_deploy_with_content() {
        let a = attrs(&[("region", AttrValue::String("iad".into()))]);
        let content = "auto_stop: 5m\nmin_machines: 1";
        let block = unknown("app-deploy", a, content);
        match resolve_block(block) {
            Block::AppDeploy { region, properties, .. } => {
                assert_eq!(region, Some("iad".to_string()));
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[0], ("auto_stop".to_string(), "5m".to_string()));
                assert_eq!(properties[1], ("min_machines".to_string(), "1".to_string()));
            }
            other => panic!("Expected AppDeploy, got {other:?}"),
        }
    }

    #[test]
    fn parse_app_deploy_from_full_surf() {
        let source = "::app-deploy[region=\"sjc\" scale=\"2\" domain=\"myapp.surf.new\" memory=\"512mb\"]\n::";
        let result = crate::parse(source);
        assert!(result.diagnostics.is_empty(), "Diagnostics: {:?}", result.diagnostics);
        let has_deploy = result.doc.blocks.iter().any(|b| matches!(b, Block::AppDeploy { .. }));
        assert!(has_deploy, "Should contain an AppDeploy block");
    }

    // ── Interactive / application block parse tests ───────────────

    #[test]
    fn parse_app_shell() {
        let source = "::app-shell[layout=sidebar-main-panel]\nSome content\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::AppShell { layout, .. } => assert_eq!(layout, "sidebar-main-panel"),
            other => panic!("Expected AppShell, got {:?}", other),
        }
    }

    /// Chrome-in-chrome nesting is real nesting: same-depth balanced child
    /// containers belong to the parent when a surplus closer follows them.
    /// This is the v0.11 nesting fix — previously the first same-depth child
    /// opener misclassified the parent as a leaf, emitting it with empty
    /// children and the chrome as top-level siblings.
    #[test]
    fn parse_app_shell_nested_chrome() {
        let source = "\
::app-shell[layout=tabs]

::tab-bar[active=docs]
- docs \"Docs\"
- tasks \"Tasks\"
::

::tab-content[tab=docs]
::nav-tree[source=docsViewModel on-select=open_doc]
::
::

::chat-thread[source=surfy]
::

::

::drawer[name=main position=left width=320]
::sidebar[position=left width=320]
::nav-tree[source=drawerDestinations on-select=switch_root]
::
::
::
";
        let result = crate::parse(source);
        assert!(
            result.diagnostics.is_empty(),
            "nested chrome must parse clean: {:?}",
            result.diagnostics
        );
        assert_eq!(result.doc.blocks.len(), 2, "blocks: {:?}", result.doc.blocks);

        match &result.doc.blocks[0] {
            Block::AppShell { layout, children, .. } => {
                assert_eq!(layout, "tabs");
                let kinds: Vec<&'static str> = children
                    .iter()
                    .map(|b| match b {
                        Block::TabBar { .. } => "tab-bar",
                        Block::TabContent { .. } => "tab-content",
                        Block::ChatThread { .. } => "chat-thread",
                        other => panic!("unexpected app-shell child: {other:?}"),
                    })
                    .collect();
                assert_eq!(kinds, ["tab-bar", "tab-content", "chat-thread"]);
                match &children[1] {
                    Block::TabContent { tab, children, .. } => {
                        assert_eq!(tab, "docs");
                        assert!(
                            matches!(children[0], Block::NavTree { .. }),
                            "tab-content child: {:?}",
                            children
                        );
                    }
                    other => panic!("expected TabContent, got {other:?}"),
                }
            }
            other => panic!("Expected AppShell, got {other:?}"),
        }

        match &result.doc.blocks[1] {
            Block::Drawer { name, children, .. } => {
                assert_eq!(name, "main");
                match &children[0] {
                    Block::Sidebar { children, .. } => {
                        assert!(
                            matches!(children[0], Block::NavTree { .. }),
                            "sidebar child: {:?}",
                            children
                        );
                    }
                    other => panic!("expected Sidebar, got {other:?}"),
                }
            }
            other => panic!("Expected Drawer, got {other:?}"),
        }
    }

    /// The nesting fix must not reclassify true leaves: a closer-less
    /// directive followed by sibling blocks stays a leaf, and sibling
    /// containers stay siblings.
    #[test]
    fn leaf_directives_stay_leaves_after_nesting_fix() {
        let source = "\
::metric[label=Users value=42]

::hero
headline: H
::

::callout[type=info]
Note
::
";
        let result = crate::parse(source);
        let kinds: Vec<&'static str> = result
            .doc
            .blocks
            .iter()
            .map(|b| match b {
                Block::Metric { .. } => "metric",
                Block::Hero { .. } => "hero",
                Block::Callout { .. } => "callout",
                other => panic!("unexpected block: {other:?}"),
            })
            .collect();
        assert_eq!(kinds, ["metric", "hero", "callout"]);
    }

    #[test]
    fn parse_sidebar_block() {
        let source = "::sidebar[position=left collapsible=true width=250]\nContent here\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Sidebar { position, collapsible, width, .. } => {
                assert_eq!(position, "left");
                assert!(*collapsible);
                assert_eq!(*width, Some(250));
            }
            other => panic!("Expected Sidebar, got {:?}", other),
        }
    }

    #[test]
    fn parse_panel_block() {
        let source = "::panel[position=bottom resizable=true height=200 desktop-only=true]\nPanel content\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Panel { position, resizable, height, desktop_only, .. } => {
                assert_eq!(position, "bottom");
                assert!(*resizable);
                assert_eq!(*height, Some(200));
                assert!(*desktop_only);
            }
            other => panic!("Expected Panel, got {:?}", other),
        }
    }

    #[test]
    fn parse_tab_bar_block() {
        let source = "::tab-bar[active=preview]\n- preview \"Preview\"\n- edit \"Edit\"\n- source \"Source\"\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::TabBar { active, items, .. } => {
                assert_eq!(*active, Some("preview".to_string()));
                assert_eq!(items.len(), 3);
                assert_eq!(items[0].id, "preview");
                assert_eq!(items[0].label, "Preview");
                assert_eq!(items[0].icon, None);
                assert_eq!(items[2].id, "source");
            }
            other => panic!("Expected TabBar, got {:?}", other),
        }
    }

    /// 0.11: tab items take an optional trailing `{icon=token}` brace, same
    /// shape as nav rows. Icon-less items and label-less ids still parse.
    #[test]
    fn parse_tab_bar_item_icons() {
        let source = "::tab-bar[active=docs]\n- docs \"Docs\" {icon=doc.text}\n- tasks \"Tasks\" {icon=\"checklist\"}\n- apps \"Apps\"\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::TabBar { items, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0].icon.as_deref(), Some("doc.text"));
                assert_eq!(items[0].label, "Docs");
                assert_eq!(items[1].icon.as_deref(), Some("checklist"));
                assert_eq!(items[2].icon, None);
                // 0.11 items author no role.
                assert!(items.iter().all(|i| i.role.is_none()));
            }
            other => panic!("Expected TabBar, got {other:?}"),
        }
    }

    /// 0.12: the trailing brace also carries `role=`. `search` is the only
    /// value the train recognizes; the value is carried verbatim and NEVER
    /// diagnosed, so a player lacking the concept renders a plain tab.
    #[test]
    fn parse_tab_bar_item_role() {
        let source = "::tab-bar[active=docs]\n- docs \"Docs\" {icon=doc.text}\n- search \"Search\" {icon=magnifyingglass role=search}\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::TabBar { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].role, None);
                assert_eq!(items[1].id, "search");
                assert_eq!(items[1].label, "Search");
                assert_eq!(items[1].icon.as_deref(), Some("magnifyingglass"));
                assert_eq!(items[1].role.as_deref(), Some("search"));
            }
            other => panic!("Expected TabBar, got {other:?}"),
        }
        assert!(
            result.diagnostics.is_empty(),
            "role= must not diagnose: {:?}",
            result.diagnostics
        );
    }

    /// Regression: a role-ONLY brace (no `icon=`) used to leak into the
    /// label because the brace was stripped only when an icon parsed.
    #[test]
    fn parse_tab_bar_item_role_only_brace_leaves_label_clean() {
        let source = "::tab-bar[active=docs]\n- search \"Search\" {role=search}\n- quoted \"Quoted\" {role=\"search\"}\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::TabBar { items, .. } => {
                assert_eq!(items[0].label, "Search");
                assert_eq!(items[0].role.as_deref(), Some("search"));
                assert_eq!(items[0].icon, None);
                // Quotes around the value are stripped, same as `icon=`.
                assert_eq!(items[1].label, "Quoted");
                assert_eq!(items[1].role.as_deref(), Some("search"));
            }
            other => panic!("Expected TabBar, got {other:?}"),
        }
        assert!(result.diagnostics.is_empty());
    }

    /// An unrecognized role value is carried, not rejected — and a brace
    /// holding ONLY unknown tokens is left alone rather than half-stripped.
    #[test]
    fn parse_tab_bar_item_unknown_role_is_silent() {
        let source = "::tab-bar\n- a \"A\" {role=compose}\n- b \"B\" {unknown=x}\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::TabBar { items, .. } => {
                assert_eq!(items[0].role.as_deref(), Some("compose"));
                assert_eq!(items[0].label, "A");
                // No known token parsed, so the brace stays in the label —
                // the pre-0.12 behaviour for unrecognized braces, unchanged.
                assert_eq!(items[1].role, None);
                assert!(items[1].label.contains("{unknown=x}"));
            }
            other => panic!("Expected TabBar, got {other:?}"),
        }
        assert!(
            result.diagnostics.is_empty(),
            "unknown role must be silent: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn parse_tab_content_block() {
        let source = "::tab-content[tab=preview]\nTab body\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::TabContent { tab, .. } => assert_eq!(tab, "preview"),
            other => panic!("Expected TabContent, got {:?}", other),
        }
    }

    #[test]
    fn parse_toolbar_block() {
        let source = "::toolbar\n- button[label=\"Deploy\" action=deploy style=primary]\n- separator\n- spacer\n- badge[value=\"Live\" color=green]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Toolbar { items, .. } => {
                assert_eq!(items.len(), 4);
                assert!(matches!(&items[0], ToolbarItem::Button { label: Some(l), .. } if l == "Deploy"));
                assert!(matches!(&items[1], ToolbarItem::Separator));
                assert!(matches!(&items[2], ToolbarItem::Spacer));
                assert!(matches!(&items[3], ToolbarItem::Badge { value, .. } if value == "Live"));
            }
            other => panic!("Expected Toolbar, got {:?}", other),
        }
    }

    #[test]
    fn parse_toolbar_title_and_title_source() {
        let source = "::toolbar[title=\"Messages\" title-source=thread.display_name]\n- button[label=\"New\" action=compose]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Toolbar { title, title_source, items, .. } => {
                assert_eq!(title.as_deref(), Some("Messages"));
                assert_eq!(title_source.as_deref(), Some("thread.display_name"));
                assert_eq!(items.len(), 1);
            }
            other => panic!("Expected Toolbar, got {other:?}"),
        }
    }

    #[test]
    fn parse_toolbar_without_title_stays_none() {
        let source = "::toolbar\n- button[label=\"Run\" action=run]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Toolbar { title, title_source, .. } => {
                assert_eq!(*title, None);
                assert_eq!(*title_source, None);
            }
            other => panic!("Expected Toolbar, got {other:?}"),
        }
    }

    #[test]
    fn parse_drawer_block() {
        let source = "::drawer[name=mako position=right width=320 trigger=icon]\nDrawer content\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Drawer { name, position, width, trigger, .. } => {
                assert_eq!(name, "mako");
                assert_eq!(position, "right");
                assert_eq!(*width, Some(320));
                assert_eq!(*trigger, Some("icon".to_string()));
            }
            other => panic!("Expected Drawer, got {:?}", other),
        }
    }

    /// 0.12 contract item (b): the drawer's SHELL shape — the pre-2026-07-31
    /// grammar — parses clean and nests two levels deep. `::drawer` wraps a
    /// `::sidebar`, which carries the workspace-switcher `::toolbar` and then
    /// the `drawerDestinations` nav-tree. No new vocabulary is involved: the
    /// switcher region is a plain v0.11 toolbar button, and drawer ROWS are
    /// deliberately NOT authored (a row carries no action, so it could not
    /// bind a verb).
    #[test]
    fn parse_drawer_shell_shape_nests_sidebar_toolbar_nav_tree() {
        let source = "\
::drawer[name=main position=left width=320 trigger=\"Menu\"]
::sidebar[position=left width=320]
::toolbar
- button[label=\"Workspace\" icon=person.2 action=switchWorkspace]
::
::nav-tree[source=drawerDestinations on-select=switch_root]
::
::
::
";
        let result = crate::parse(source);
        assert!(
            result.diagnostics.is_empty(),
            "drawer shell shape must parse clean: {:?}",
            result.diagnostics
        );
        assert_eq!(result.doc.blocks.len(), 1, "blocks: {:?}", result.doc.blocks);

        match &result.doc.blocks[0] {
            Block::Drawer { name, position, width, trigger, children, .. } => {
                assert_eq!(name, "main");
                assert_eq!(position, "left");
                assert_eq!(*width, Some(320));
                assert_eq!(trigger.as_deref(), Some("Menu"));
                assert_eq!(children.len(), 1, "drawer children: {children:?}");

                match &children[0] {
                    Block::Sidebar { position, width, children, .. } => {
                        assert_eq!(position, "left");
                        assert_eq!(*width, Some(320));
                        // Switcher region FIRST, destinations SECOND.
                        assert_eq!(children.len(), 2, "sidebar children: {children:?}");
                        match &children[0] {
                            Block::Toolbar { items, .. } => {
                                assert_eq!(items.len(), 1);
                                match &items[0] {
                                    ToolbarItem::Button { label, action, icon, .. } => {
                                        assert_eq!(label.as_deref(), Some("Workspace"));
                                        assert_eq!(action.as_deref(), Some("switchWorkspace"));
                                        assert_eq!(icon.as_deref(), Some("person.2"));
                                    }
                                    other => panic!("expected Button, got {other:?}"),
                                }
                            }
                            other => panic!("expected Toolbar, got {other:?}"),
                        }
                        match &children[1] {
                            Block::NavTree { source, on_select, .. } => {
                                assert_eq!(source.as_deref(), Some("drawerDestinations"));
                                assert_eq!(on_select.as_deref(), Some("switch_root"));
                            }
                            other => panic!("expected NavTree, got {other:?}"),
                        }
                    }
                    other => panic!("expected Sidebar, got {other:?}"),
                }
            }
            other => panic!("Expected Drawer, got {other:?}"),
        }
    }

    #[test]
    fn parse_modal_block() {
        let source = "::modal[name=deploy title=\"Deploy App\"]\nModal body\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Modal { name, title, .. } => {
                assert_eq!(name, "deploy");
                assert_eq!(*title, Some("Deploy App".to_string()));
            }
            other => panic!("Expected Modal, got {:?}", other),
        }
    }

    #[test]
    fn parse_command_palette_block() {
        let source = "::command-palette[trigger=cmd+/]\n- \"Data Table\" description=\"Define data\" action=add_schema icon=table group=Data\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::CommandPalette { trigger, items, .. } => {
                assert_eq!(*trigger, Some("cmd+/".to_string()));
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].label, "Data Table");
                assert_eq!(items[0].description, Some("Define data".to_string()));
                assert_eq!(items[0].group, Some("Data".to_string()));
            }
            other => panic!("Expected CommandPalette, got {:?}", other),
        }
    }

    #[test]
    fn parse_code_editor_block() {
        let source = "::code-editor[lang=surf source=doc.source line-numbers=true]\nsome code\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::CodeEditor { lang, source, line_numbers, content, .. } => {
                assert_eq!(*lang, Some("surf".to_string()));
                assert_eq!(*source, Some("doc.source".to_string()));
                assert!(*line_numbers);
                assert!(content.contains("some code"));
            }
            other => panic!("Expected CodeEditor, got {:?}", other),
        }
    }

    #[test]
    fn parse_block_editor_block() {
        let source = "::block-editor[source=doc.blocks]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::BlockEditor { source, .. } => {
                assert_eq!(*source, Some("doc.blocks".to_string()));
            }
            other => panic!("Expected BlockEditor, got {:?}", other),
        }
    }

    #[test]
    fn parse_terminal_block() {
        let source = "::terminal[shell=default cwd=workspace.path]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Terminal { shell, cwd, .. } => {
                assert_eq!(*shell, Some("default".to_string()));
                assert_eq!(*cwd, Some("workspace.path".to_string()));
            }
            other => panic!("Expected Terminal, got {:?}", other),
        }
    }

    #[test]
    fn parse_nav_tree_block() {
        let source = "::nav-tree[source=workspace.files on-select=open_file]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::NavTree { source, on_select, .. } => {
                assert_eq!(*source, Some("workspace.files".to_string()));
                assert_eq!(*on_select, Some("open_file".to_string()));
            }
            other => panic!("Expected NavTree, got {:?}", other),
        }
    }

    #[test]
    fn parse_rich_nav_groups_and_attrs() {
        let source = "::nav[drawer brand=\"CloudSurf\" reg logo=/logo.png cta-label=\"Get in touch\" cta-href=#contact]\n## Explore\n- [Research](/research){icon=flask}\n## Products\n- [Surf](https://app.surf){image=/surf.png external}\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Nav { groups, brand, brand_reg, cta, drawer, logo, .. } => {
                assert!(*drawer);
                assert!(*brand_reg);
                assert_eq!(brand.as_deref(), Some("CloudSurf"));
                assert_eq!(logo.as_deref(), Some("/logo.png"));
                let c = cta.as_ref().expect("cta");
                assert_eq!(c.label, "Get in touch");
                assert_eq!(c.href, "#contact");
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0].label.as_deref(), Some("Explore"));
                assert_eq!(groups[0].items[0].icon.as_deref(), Some("flask"));
                assert_eq!(groups[1].items[0].image.as_deref(), Some("/surf.png"));
                assert!(groups[1].items[0].external);
            }
            other => panic!("Expected Nav, got {other:?}"),
        }
    }

    #[test]
    fn parse_rich_footer_brand_attrs() {
        let source = "::footer[brand=\"CloudSurf\" reg logo=/logo.png tagline=\"Innovate • Simplify • Scale\"]\n## Products\n- [Surf](https://app.surf)\n(c) 2026 CloudSurf\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Footer { brand, brand_reg, brand_logo, tagline, sections, .. } => {
                assert_eq!(brand.as_deref(), Some("CloudSurf"));
                assert!(*brand_reg);
                assert_eq!(brand_logo.as_deref(), Some("/logo.png"));
                assert_eq!(tagline.as_deref(), Some("Innovate • Simplify • Scale"));
                assert_eq!(sections.len(), 1);
            }
            other => panic!("Expected Footer, got {other:?}"),
        }
    }

    #[test]
    fn rich_nav_round_trips_through_builder() {
        let source = "::nav[drawer brand=\"CloudSurf\" reg cta-label=\"Apply\" cta-href=/jobs]\n## Explore\n- [Research](/research){icon=flask}\n- [Surf](https://app.surf){image=/surf.png external}\n::";
        let parsed = crate::parse(source);
        let surf = crate::builder::to_surf_source(&parsed.doc);
        // Re-parse the serialized form and confirm structure survives.
        let reparsed = crate::parse(&surf);
        match &reparsed.doc.blocks[0] {
            Block::Nav { groups, brand, brand_reg, cta, drawer, .. } => {
                assert!(*drawer);
                assert!(*brand_reg);
                assert_eq!(brand.as_deref(), Some("CloudSurf"));
                assert_eq!(cta.as_ref().unwrap().label, "Apply");
                assert_eq!(groups.len(), 1);
                assert_eq!(groups[0].items.len(), 2);
                assert_eq!(groups[0].items[0].icon.as_deref(), Some("flask"));
                assert!(groups[0].items[1].external);
                assert_eq!(groups[0].items[1].image.as_deref(), Some("/surf.png"));
            }
            other => panic!("Expected Nav, got {other:?}"),
        }
    }

    #[test]
    fn parse_badge_block() {
        let source = "::badge[value=\"Live\" color=green]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Badge { value, color, .. } => {
                assert_eq!(value, "Live");
                assert_eq!(*color, Some("green".to_string()));
            }
            other => panic!("Expected Badge, got {:?}", other),
        }
    }

    #[test]
    fn parse_suggestion_chips_block() {
        let source = "::suggestion-chips[source=mako.suggestions max=3 dismissible=true]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::SuggestionChips { source, max, dismissible, .. } => {
                assert_eq!(*source, Some("mako.suggestions".to_string()));
                assert_eq!(*max, Some(3));
                assert!(*dismissible);
            }
            other => panic!("Expected SuggestionChips, got {:?}", other),
        }
    }

    #[test]
    fn parse_chat_thread_block() {
        let source = "::chat-thread[source=mako.conversation]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::ChatThread { source, .. } => {
                assert_eq!(*source, Some("mako.conversation".to_string()));
            }
            other => panic!("Expected ChatThread, got {:?}", other),
        }
    }

    #[test]
    fn parse_chat_thread_react_and_doc_open_seams() {
        let source = "::chat-thread[source=chat.thread on-action=run_action on-react=\"mutate:messages.react:emoji\" on-doc-open=\"open:/docs/{id}\"]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::ChatThread { source, on_action, on_react, on_doc_open, .. } => {
                assert_eq!(source.as_deref(), Some("chat.thread"));
                assert_eq!(on_action.as_deref(), Some("run_action"));
                assert_eq!(on_react.as_deref(), Some("mutate:messages.react:emoji"));
                assert_eq!(on_doc_open.as_deref(), Some("open:/docs/{id}"));
            }
            other => panic!("Expected ChatThread, got {other:?}"),
        }
    }

    #[test]
    fn parse_chat_thread_seams_default_none() {
        let source = "::chat-thread[source=chat.thread]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::ChatThread { on_react, on_doc_open, .. } => {
                assert_eq!(*on_react, None);
                assert_eq!(*on_doc_open, None);
            }
            other => panic!("Expected ChatThread, got {other:?}"),
        }
    }

    #[test]
    fn parse_recipient_picker_block() {
        let source = "::recipient-picker[source=contacts mode=multi on-submit=\"invoke:messages.compose\"]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::RecipientPicker { source, mode, on_submit, .. } => {
                assert_eq!(source, "contacts");
                assert_eq!(mode, "multi");
                assert_eq!(on_submit.as_deref(), Some("invoke:messages.compose"));
            }
            other => panic!("Expected RecipientPicker, got {other:?}"),
        }
    }

    #[test]
    fn parse_recipient_picker_mode_defaults_single() {
        let source = "::recipient-picker[source=contacts]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::RecipientPicker { mode, on_submit, .. } => {
                assert_eq!(mode, "single");
                assert_eq!(*on_submit, None);
            }
            other => panic!("Expected RecipientPicker, got {other:?}"),
        }
    }

    #[test]
    fn parse_recipient_picker_invalid_mode_falls_back_single() {
        let source = "::recipient-picker[source=contacts mode=everything]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::RecipientPicker { mode, .. } => assert_eq!(mode, "single"),
            other => panic!("Expected RecipientPicker, got {other:?}"),
        }
    }

    #[test]
    fn parse_recipient_picker_source_uses_registry_validator() {
        let source = "::recipient-picker[source=\"https://evil.com/x\"]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::RecipientPicker { source, .. } => {
                assert!(source.is_empty(), "external URLs must be rejected");
            }
            other => panic!("Expected RecipientPicker, got {other:?}"),
        }
    }

    #[test]
    fn parse_qr_block_scan_with_resolve() {
        let source = "::qr[mode=scan on-resolve=\"invoke:contacts.add\"]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Qr { mode, on_resolve, .. } => {
                assert_eq!(mode, "scan");
                assert_eq!(on_resolve.as_deref(), Some("invoke:contacts.add"));
            }
            other => panic!("Expected Qr, got {other:?}"),
        }
    }

    #[test]
    fn parse_qr_mode_defaults_show() {
        let source = "::qr\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Qr { mode, on_resolve, .. } => {
                assert_eq!(mode, "show");
                assert_eq!(*on_resolve, None);
            }
            other => panic!("Expected Qr, got {other:?}"),
        }
    }

    #[test]
    fn parse_qr_invalid_mode_falls_back_show() {
        let source = "::qr[mode=teleport]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Qr { mode, .. } => assert_eq!(mode, "show"),
            other => panic!("Expected Qr, got {other:?}"),
        }
    }

    #[test]
    fn parse_chat_input_simple_block() {
        let source = "::chat-input-simple[placeholder=\"Message Mako...\" action=mako.send]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::ChatInputSimple { placeholder, action, .. } => {
                assert_eq!(*placeholder, Some("Message Mako...".to_string()));
                assert_eq!(*action, Some("mako.send".to_string()));
            }
            other => panic!("Expected ChatInputSimple, got {:?}", other),
        }
    }

    #[test]
    fn parse_progress_block() {
        let source = "::progress[source=deploy.progress]\n- Parsing app \u{2713}\n- Creating database \u{25CF}\n- Registering routes \u{25CB}\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::Progress { source, steps, .. } => {
                assert_eq!(*source, Some("deploy.progress".to_string()));
                assert_eq!(steps.len(), 3);
                assert_eq!(steps[0].label, "Parsing app");
                assert_eq!(steps[0].status, "done");
                assert_eq!(steps[1].status, "active");
                assert_eq!(steps[2].status, "pending");
            }
            other => panic!("Expected Progress, got {:?}", other),
        }
    }

    #[test]
    fn parse_log_stream_block() {
        let source = "::log-stream[source=app.logs tail=100]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::LogStream { source, tail, .. } => {
                assert_eq!(*source, Some("app.logs".to_string()));
                assert_eq!(*tail, Some(100));
            }
            other => panic!("Expected LogStream, got {:?}", other),
        }
    }

    #[test]
    fn parse_problem_list_block() {
        let source = "::problem-list[source=doc.diagnostics]\n::";
        let result = crate::parse(source);
        let block = &result.doc.blocks[0];
        match block {
            Block::ProblemList { source, .. } => {
                assert_eq!(*source, Some("doc.diagnostics".to_string()));
            }
            other => panic!("Expected ProblemList, got {:?}", other),
        }
    }

    // -- PostGrid --------------------------------------------------

    #[test]
    fn resolve_post_grid_full() {
        let content = "\
- [First Post](/blog/first){external} | News · 2026-01-01 | A short excerpt | /img/a.png
- [Second Post](/blog/second) | Events · 2026-02-02 | Another excerpt
- [Bare Post](/blog/bare)";
        let block = unknown(
            "post-grid",
            attrs(&[
                ("title", AttrValue::String("Blog".into())),
                ("subtitle", AttrValue::String("Latest news".into())),
            ]),
            content,
        );
        match resolve_block(block) {
            Block::PostGrid { title, subtitle, items, .. } => {
                assert_eq!(title, Some("Blog".to_string()));
                assert_eq!(subtitle, Some("Latest news".to_string()));
                assert_eq!(items.len(), 3);

                assert_eq!(items[0].title, "First Post");
                assert_eq!(items[0].href, "/blog/first");
                assert!(items[0].external);
                assert_eq!(items[0].meta.as_deref(), Some("News · 2026-01-01"));
                assert_eq!(items[0].excerpt.as_deref(), Some("A short excerpt"));
                assert_eq!(items[0].image.as_deref(), Some("/img/a.png"));

                assert!(!items[1].external);
                assert_eq!(items[1].meta.as_deref(), Some("Events · 2026-02-02"));
                assert_eq!(items[1].excerpt.as_deref(), Some("Another excerpt"));
                assert_eq!(items[1].image, None);

                assert_eq!(items[2].title, "Bare Post");
                assert_eq!(items[2].meta, None);
                assert_eq!(items[2].excerpt, None);
            }
            other => panic!("Expected PostGrid, got {other:?}"),
        }
    }

    #[test]
    fn parse_post_grid_round_trip() {
        let source = "\
::post-grid[title=\"News\" subtitle=\"Updates\"]
- [Hello](/x){external} | Meta | Excerpt | /img.png
::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::PostGrid { title, items, .. } => {
                assert_eq!(title.as_deref(), Some("News"));
                assert_eq!(items.len(), 1);
                assert!(items[0].external);
                assert_eq!(items[0].image.as_deref(), Some("/img.png"));
            }
            other => panic!("Expected PostGrid, got {other:?}"),
        }
    }

    // -- Gate ------------------------------------------------------

    #[test]
    fn resolve_gate_full() {
        let block = unknown(
            "gate",
            attrs(&[
                ("title", AttrValue::String("Members".into())),
                ("subtitle", AttrValue::String("Enter the code".into())),
                ("action", AttrValue::String("/unlock".into())),
                ("field", AttrValue::String("Your code".into())),
                ("submit", AttrValue::String("Unlock".into())),
                ("error", AttrValue::String("Wrong code".into())),
            ]),
            "",
        );
        match resolve_block(block) {
            Block::Gate { title, subtitle, action, field_label, submit_label, error, .. } => {
                assert_eq!(title.as_deref(), Some("Members"));
                assert_eq!(subtitle.as_deref(), Some("Enter the code"));
                assert_eq!(action, "/unlock");
                assert_eq!(field_label.as_deref(), Some("Your code"));
                assert_eq!(submit_label.as_deref(), Some("Unlock"));
                assert_eq!(error.as_deref(), Some("Wrong code"));
            }
            other => panic!("Expected Gate, got {other:?}"),
        }
    }

    #[test]
    fn resolve_gate_body_becomes_subtitle() {
        let block = unknown("gate", Attrs::new(), "This page is private.");
        match resolve_block(block) {
            Block::Gate { subtitle, action, .. } => {
                assert_eq!(subtitle.as_deref(), Some("This page is private."));
                assert_eq!(action, "");
            }
            other => panic!("Expected Gate, got {other:?}"),
        }
    }

    // -- Nav minimal ----------------------------------------------

    #[test]
    fn parse_nav_minimal() {
        let source = "::nav[brand=\"Acme\" minimal]\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Nav { minimal, brand, .. } => {
                assert!(*minimal);
                assert_eq!(brand.as_deref(), Some("Acme"));
            }
            other => panic!("Expected Nav, got {other:?}"),
        }
    }

    #[test]
    fn parse_nav_minimal_defaults_false() {
        let source = "::nav[logo=\"Surf\"]\n- [Home](/)\n::";
        let result = crate::parse(source);
        match &result.doc.blocks[0] {
            Block::Nav { minimal, .. } => assert!(!*minimal),
            other => panic!("Expected Nav, got {other:?}"),
        }
    }

    // -- p3-copy-sweep: attrs displaced onto their own line ---------

    #[test]
    fn attrs_on_own_line_adopted() {
        // `::features` newline `[cols=3 title="Why us"]` — the bracket line is
        // adopted as block attrs instead of rendering as body text.
        let block = unknown(
            "features",
            Attrs::new(),
            "[cols=3 title=\"Why us\"]\n### Fast\nQuick.\n### Safe\nSecure.",
        );
        match resolve_block(block) {
            Block::Features { cards, cols, .. } => {
                assert_eq!(cols, Some(3), "cols=3 honored");
                assert_eq!(cards.len(), 2);
                for card in &cards {
                    assert!(
                        !card.title.contains("[cols=") && !card.body.contains("[cols="),
                        "bracket text leaked into card: {card:?}"
                    );
                }
            }
            other => panic!("Expected Features, got {other:?}"),
        }
    }

    #[test]
    fn attrs_on_own_line_not_adopted_for_markdown_shapes() {
        // A markdown link-reference definition is NOT a displaced attr line.
        let block = unknown("summary", Attrs::new(), "[1]: https://example.com\nBody.");
        match resolve_block(block) {
            Block::Summary { content, .. } => {
                assert!(content.contains("[1]: https://example.com"), "{content:?}");
            }
            other => panic!("Expected Summary, got {other:?}"),
        }
        // Existing attrs win: the bracket line stays content.
        let block = unknown(
            "callout",
            attrs(&[("type", AttrValue::String("info".into()))]),
            "[cols=3]\nBody.",
        );
        match resolve_block(block) {
            Block::Callout { content, .. } => {
                assert!(content.contains("[cols=3]"), "{content:?}");
            }
            other => panic!("Expected Callout, got {other:?}"),
        }
    }
}

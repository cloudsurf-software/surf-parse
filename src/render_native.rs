//! Native block renderer for mobile/desktop native rendering via UniFFI.
//!
//! Converts a `SurfDoc` into a flat `Vec<NativeBlock>` suitable for export
//! across the FFI boundary. Web-only block types (Nav, Footer, Site, Page,
//! Form, Gallery, Embed, Style, Logo, HeroImage, PricingTable, Section,
//! ProductCard, Unknown) are degraded to their markdown equivalent.

use crate::render_md;
use crate::types::{Block, CalloutType, DecisionStatus, SurfDoc, Trend};

// ═══════════════════════════════════════════════════════════════════════
// NativeBlock enum — 25 native variants
// ═══════════════════════════════════════════════════════════════════════

/// Simplified block representation for native mobile rendering via UniFFI.
///
/// Every field uses only UniFFI-safe types: `String`, `bool`, `u32`,
/// `Option<T>`, `Vec<T>`, and simple structs of the same. No `BTreeMap`,
/// no `Span`, no serde tags, no `enum` sub-types with complex discriminants.
///
/// Web-only blocks (Nav, Footer, Site, Page, Form, Gallery, Embed, Style,
/// Logo, HeroImage, PricingTable, Section, ProductCard, Unknown) are
/// degraded to their markdown equivalent and emitted as `NativeBlock::Markdown`.
#[derive(Debug, Clone, PartialEq)]
pub enum NativeBlock {
    /// Plain markdown text. Also the fallback for unsupported block types.
    Markdown { content: String },

    /// Callout/admonition box with colored border.
    /// `callout_type` is one of: "info", "warning", "danger", "tip", "note", "success".
    Callout {
        callout_type: String,
        title: Option<String>,
        content: String,
    },

    /// Fenced code block with optional language tag and file path.
    Code {
        language: Option<String>,
        file_path: Option<String>,
        content: String,
    },

    /// Structured data table with headers and rows.
    DataTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        sortable: bool,
    },

    /// Task checklist with checkbox items.
    Tasks { items: Vec<NativeTaskItem> },

    /// Decision record.
    /// `status` is one of: "proposed", "accepted", "rejected", "superseded".
    Decision {
        status: String,
        date: Option<String>,
        deciders: Vec<String>,
        content: String,
    },

    /// Single metric display with trend indicator.
    /// `trend` is one of: "up", "down", "flat", or None.
    Metric {
        label: String,
        value: String,
        trend: Option<String>,
        unit: Option<String>,
    },

    /// Executive summary box.
    Summary { content: String },

    /// Image with optional caption and alt text.
    Figure {
        src: String,
        caption: Option<String>,
        alt: Option<String>,
    },

    /// Tabbed content panels (renders as segmented picker or TabView).
    Tabs { tabs: Vec<NativeTabPanel> },

    /// Multi-column layout.
    Columns { columns: Vec<NativeColumnContent> },

    /// Attributed quote with optional source.
    Quote {
        content: String,
        attribution: Option<String>,
    },

    /// Call-to-action button/link.
    Cta {
        label: String,
        href: String,
        primary: bool,
    },

    /// Customer testimonial with author info.
    Testimonial {
        content: String,
        author: Option<String>,
        role: Option<String>,
        company: Option<String>,
    },

    /// FAQ accordion with question/answer pairs.
    Faq { items: Vec<NativeFaqItem> },

    /// Collapsible content section.
    Details {
        title: Option<String>,
        open: bool,
        content: String,
    },

    /// Thematic divider with optional label.
    Divider { label: Option<String> },

    /// Hero section with headline, subtitle, and optional badge.
    Hero {
        headline: Option<String>,
        subtitle: Option<String>,
        badge: Option<String>,
    },

    /// Feature card grid.
    Features { cards: Vec<NativeFeatureCard> },

    /// Numbered process/timeline steps.
    Steps { steps: Vec<NativeStepItem> },

    /// Row of stat cards.
    Stats { items: Vec<NativeStatItem> },

    /// Feature comparison matrix.
    Comparison {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        highlight: Option<String>,
    },

    /// Table of contents with navigation entries.
    Toc {
        depth: u32,
        entries: Vec<NativeTocEntry>,
    },

    /// Before/After comparison visualization.
    BeforeAfter {
        before_items: Vec<NativeBeforeAfterItem>,
        after_items: Vec<NativeBeforeAfterItem>,
        transition: Option<String>,
    },

    /// Pipeline flow with labeled steps.
    Pipeline { steps: Vec<NativePipelineStep> },
}

// ═══════════════════════════════════════════════════════════════════════
// Supporting record types — all simple, UniFFI-friendly
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct NativeTaskItem {
    pub done: bool,
    pub text: String,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeTabPanel {
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeColumnContent {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeFaqItem {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeFeatureCard {
    pub title: String,
    pub icon: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeStepItem {
    pub title: String,
    pub time: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeStatItem {
    pub value: String,
    pub label: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeTocEntry {
    pub text: String,
    pub id: String,
    pub level: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeBeforeAfterItem {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativePipelineStep {
    pub label: String,
    pub description: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
// Conversion functions
// ═══════════════════════════════════════════════════════════════════════

/// Convert a parsed SurfDoc into a Vec<NativeBlock> for native rendering.
pub fn to_native_blocks(doc: &SurfDoc) -> Vec<NativeBlock> {
    doc.blocks.iter().map(convert_block).collect()
}

fn convert_block(block: &Block) -> NativeBlock {
    match block {
        // ── Native variants: direct conversion ──────────────────────

        Block::Markdown { content, .. } => NativeBlock::Markdown {
            content: content.clone(),
        },

        Block::Callout {
            callout_type,
            title,
            content,
            ..
        } => NativeBlock::Callout {
            callout_type: callout_type_str(*callout_type),
            title: title.clone(),
            content: content.clone(),
        },

        Block::Code {
            lang,
            file,
            content,
            ..
        } => NativeBlock::Code {
            language: lang.clone(),
            file_path: file.clone(),
            content: content.clone(),
        },

        Block::Data {
            headers,
            rows,
            sortable,
            ..
        } => NativeBlock::DataTable {
            headers: headers.clone(),
            rows: rows.clone(),
            sortable: *sortable,
        },

        Block::Tasks { items, .. } => NativeBlock::Tasks {
            items: items
                .iter()
                .map(|i| NativeTaskItem {
                    done: i.done,
                    text: i.text.clone(),
                    assignee: i.assignee.clone(),
                })
                .collect(),
        },

        Block::Decision {
            status,
            date,
            deciders,
            content,
            ..
        } => NativeBlock::Decision {
            status: decision_status_str(*status),
            date: date.clone(),
            deciders: deciders.clone(),
            content: content.clone(),
        },

        Block::Metric {
            label,
            value,
            trend,
            unit,
            ..
        } => NativeBlock::Metric {
            label: label.clone(),
            value: value.clone(),
            trend: trend.map(trend_str),
            unit: unit.clone(),
        },

        Block::Summary { content, .. } => NativeBlock::Summary {
            content: content.clone(),
        },

        Block::Figure {
            src,
            caption,
            alt,
            ..
        } => NativeBlock::Figure {
            src: src.clone(),
            caption: caption.clone(),
            alt: alt.clone(),
        },

        Block::Tabs { tabs, .. } => NativeBlock::Tabs {
            tabs: tabs
                .iter()
                .map(|t| NativeTabPanel {
                    label: t.label.clone(),
                    content: t.content.clone(),
                })
                .collect(),
        },

        Block::Columns { columns, .. } => NativeBlock::Columns {
            columns: columns
                .iter()
                .map(|c| NativeColumnContent {
                    content: c.content.clone(),
                })
                .collect(),
        },

        Block::Quote {
            content,
            attribution,
            ..
        } => NativeBlock::Quote {
            content: content.clone(),
            attribution: attribution.clone(),
        },

        Block::Cta {
            label,
            href,
            primary,
            ..
        } => NativeBlock::Cta {
            label: label.clone(),
            href: href.clone(),
            primary: *primary,
        },

        Block::Testimonial {
            content,
            author,
            role,
            company,
            ..
        } => NativeBlock::Testimonial {
            content: content.clone(),
            author: author.clone(),
            role: role.clone(),
            company: company.clone(),
        },

        Block::Faq { items, .. } => NativeBlock::Faq {
            items: items
                .iter()
                .map(|i| NativeFaqItem {
                    question: i.question.clone(),
                    answer: i.answer.clone(),
                })
                .collect(),
        },

        Block::Details {
            title,
            open,
            content,
            ..
        } => NativeBlock::Details {
            title: title.clone(),
            open: *open,
            content: content.clone(),
        },

        Block::Divider { label, .. } => NativeBlock::Divider {
            label: label.clone(),
        },

        Block::Hero {
            headline,
            subtitle,
            badge,
            ..
        } => NativeBlock::Hero {
            headline: headline.clone(),
            subtitle: subtitle.clone(),
            badge: badge.clone(),
        },

        Block::Features { cards, .. } => NativeBlock::Features {
            cards: cards
                .iter()
                .map(|c| NativeFeatureCard {
                    title: c.title.clone(),
                    icon: c.icon.clone(),
                    body: c.body.clone(),
                })
                .collect(),
        },

        Block::Steps { steps, .. } => NativeBlock::Steps {
            steps: steps
                .iter()
                .map(|s| NativeStepItem {
                    title: s.title.clone(),
                    time: s.time.clone(),
                    body: s.body.clone(),
                })
                .collect(),
        },

        Block::Stats { items, .. } => NativeBlock::Stats {
            items: items
                .iter()
                .map(|i| NativeStatItem {
                    value: i.value.clone(),
                    label: i.label.clone(),
                    color: i.color.clone(),
                })
                .collect(),
        },

        Block::Comparison {
            headers,
            rows,
            highlight,
            ..
        } => NativeBlock::Comparison {
            headers: headers.clone(),
            rows: rows.clone(),
            highlight: highlight.clone(),
        },

        Block::Toc {
            depth, entries, ..
        } => NativeBlock::Toc {
            depth: *depth,
            entries: entries
                .iter()
                .map(|e| NativeTocEntry {
                    text: e.text.clone(),
                    id: e.id.clone(),
                    level: e.level,
                })
                .collect(),
        },

        Block::BeforeAfter {
            before_items,
            after_items,
            transition,
            ..
        } => NativeBlock::BeforeAfter {
            before_items: before_items
                .iter()
                .map(|i| NativeBeforeAfterItem {
                    label: i.label.clone(),
                    detail: i.detail.clone(),
                })
                .collect(),
            after_items: after_items
                .iter()
                .map(|i| NativeBeforeAfterItem {
                    label: i.label.clone(),
                    detail: i.detail.clone(),
                })
                .collect(),
            transition: transition.clone(),
        },

        Block::Pipeline { steps, .. } => NativeBlock::Pipeline {
            steps: steps
                .iter()
                .map(|s| NativePipelineStep {
                    label: s.label.clone(),
                    description: s.description.clone(),
                })
                .collect(),
        },

        // ── Markdown fallback: web-only / unsupported block types ───

        Block::Unknown { .. }
        | Block::Nav { .. }
        | Block::HeroImage { .. }
        | Block::Style { .. }
        | Block::PricingTable { .. }
        | Block::Site { .. }
        | Block::Page { .. }
        | Block::Embed { .. }
        | Block::Form { .. }
        | Block::Gallery { .. }
        | Block::Footer { .. }
        | Block::Logo { .. }
        | Block::Section { .. }
        | Block::ProductCard { .. } => {
            let md = render_md::render_block(block);
            NativeBlock::Markdown { content: md }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Helper functions for enum-to-string conversion
// ═══════════════════════════════════════════════════════════════════════

fn callout_type_str(ct: CalloutType) -> String {
    match ct {
        CalloutType::Info => "info",
        CalloutType::Warning => "warning",
        CalloutType::Danger => "danger",
        CalloutType::Tip => "tip",
        CalloutType::Note => "note",
        CalloutType::Success => "success",
    }
    .to_string()
}

fn decision_status_str(ds: DecisionStatus) -> String {
    match ds {
        DecisionStatus::Proposed => "proposed",
        DecisionStatus::Accepted => "accepted",
        DecisionStatus::Rejected => "rejected",
        DecisionStatus::Superseded => "superseded",
    }
    .to_string()
}

fn trend_str(t: Trend) -> String {
    match t {
        Trend::Up => "up",
        Trend::Down => "down",
        Trend::Flat => "flat",
    }
    .to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// Unit tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::collections::BTreeMap;

    fn syn() -> Span {
        Span::SYNTHETIC
    }

    #[test]
    fn native_markdown_passthrough() {
        let block = Block::Markdown {
            content: "# Hello\n\nWorld".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Markdown {
                content: "# Hello\n\nWorld".to_string()
            }
        );
    }

    #[test]
    fn native_callout_info() {
        let block = Block::Callout {
            callout_type: CalloutType::Info,
            title: Some("Watch out".to_string()),
            content: "Sharp edges".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Callout {
                callout_type: "info".to_string(),
                title: Some("Watch out".to_string()),
                content: "Sharp edges".to_string(),
            }
        );
    }

    #[test]
    fn native_callout_all_types() {
        let types = [
            (CalloutType::Info, "info"),
            (CalloutType::Warning, "warning"),
            (CalloutType::Danger, "danger"),
            (CalloutType::Tip, "tip"),
            (CalloutType::Note, "note"),
            (CalloutType::Success, "success"),
        ];
        for (ct, expected) in types {
            let block = Block::Callout {
                callout_type: ct,
                title: None,
                content: String::new(),
                span: syn(),
            };
            match convert_block(&block) {
                NativeBlock::Callout { callout_type, .. } => {
                    assert_eq!(callout_type, expected);
                }
                other => panic!("Expected Callout, got {:?}", other),
            }
        }
    }

    #[test]
    fn native_code_with_lang() {
        let block = Block::Code {
            lang: Some("rust".to_string()),
            file: Some("main.rs".to_string()),
            highlight: vec![],
            content: "fn main() {}".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Code {
                language: Some("rust".to_string()),
                file_path: Some("main.rs".to_string()),
                content: "fn main() {}".to_string(),
            }
        );
    }

    #[test]
    fn native_code_no_lang() {
        let block = Block::Code {
            lang: None,
            file: None,
            highlight: vec![],
            content: "echo hi".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Code {
                language: None,
                file_path: None,
                content: "echo hi".to_string(),
            }
        );
    }

    #[test]
    fn native_data_table() {
        let block = Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: true,
            headers: vec!["Name".to_string(), "Age".to_string()],
            rows: vec![vec!["Alice".to_string(), "30".to_string()]],
            raw_content: String::new(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::DataTable {
                headers: vec!["Name".to_string(), "Age".to_string()],
                rows: vec![vec!["Alice".to_string(), "30".to_string()]],
                sortable: true,
            }
        );
    }

    #[test]
    fn native_data_table_empty() {
        let block = Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec![],
            rows: vec![],
            raw_content: String::new(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::DataTable {
                headers: vec![],
                rows: vec![],
                sortable: false,
            }
        );
    }

    #[test]
    fn native_tasks() {
        let block = Block::Tasks {
            items: vec![
                TaskItem {
                    done: false,
                    text: "Write tests".to_string(),
                    assignee: None,
                },
                TaskItem {
                    done: true,
                    text: "Ship".to_string(),
                    assignee: Some("brady".to_string()),
                },
            ],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Tasks {
                items: vec![
                    NativeTaskItem {
                        done: false,
                        text: "Write tests".to_string(),
                        assignee: None,
                    },
                    NativeTaskItem {
                        done: true,
                        text: "Ship".to_string(),
                        assignee: Some("brady".to_string()),
                    },
                ],
            }
        );
    }

    #[test]
    fn native_decision_accepted() {
        let block = Block::Decision {
            status: DecisionStatus::Accepted,
            date: Some("2026-02-24".to_string()),
            deciders: vec!["brady".to_string()],
            content: "We chose Rust.".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Decision {
                status: "accepted".to_string(),
                date: Some("2026-02-24".to_string()),
                deciders: vec!["brady".to_string()],
                content: "We chose Rust.".to_string(),
            }
        );
    }

    #[test]
    fn native_metric_with_trend() {
        let block = Block::Metric {
            label: "MRR".to_string(),
            value: "$2K".to_string(),
            trend: Some(Trend::Up),
            unit: Some("USD".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Metric {
                label: "MRR".to_string(),
                value: "$2K".to_string(),
                trend: Some("up".to_string()),
                unit: Some("USD".to_string()),
            }
        );
    }

    #[test]
    fn native_metric_no_trend() {
        let block = Block::Metric {
            label: "Users".to_string(),
            value: "100".to_string(),
            trend: None,
            unit: None,
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Metric {
                label: "Users".to_string(),
                value: "100".to_string(),
                trend: None,
                unit: None,
            }
        );
    }

    #[test]
    fn native_summary() {
        let block = Block::Summary {
            content: "Executive overview.".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Summary {
                content: "Executive overview.".to_string()
            }
        );
    }

    #[test]
    fn native_figure() {
        let block = Block::Figure {
            src: "diagram.png".to_string(),
            caption: Some("Arch".to_string()),
            alt: Some("Diagram".to_string()),
            width: Some("400px".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Figure {
                src: "diagram.png".to_string(),
                caption: Some("Arch".to_string()),
                alt: Some("Diagram".to_string()),
            }
        );
    }

    #[test]
    fn native_tabs() {
        let block = Block::Tabs {
            tabs: vec![TabPanel {
                label: "Rust".to_string(),
                content: "fn main() {}".to_string(),
            }],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Tabs {
                tabs: vec![NativeTabPanel {
                    label: "Rust".to_string(),
                    content: "fn main() {}".to_string(),
                }],
            }
        );
    }

    #[test]
    fn native_columns() {
        let block = Block::Columns {
            columns: vec![
                ColumnContent {
                    content: "Col 1".to_string(),
                },
                ColumnContent {
                    content: "Col 2".to_string(),
                },
            ],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Columns {
                columns: vec![
                    NativeColumnContent {
                        content: "Col 1".to_string()
                    },
                    NativeColumnContent {
                        content: "Col 2".to_string()
                    },
                ],
            }
        );
    }

    #[test]
    fn native_quote() {
        let block = Block::Quote {
            content: "To be or not".to_string(),
            attribution: Some("Shakespeare".to_string()),
            cite: Some("Hamlet".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Quote {
                content: "To be or not".to_string(),
                attribution: Some("Shakespeare".to_string()),
            }
        );
    }

    #[test]
    fn native_cta() {
        let block = Block::Cta {
            label: "Sign Up".to_string(),
            href: "/signup".to_string(),
            primary: true,
            icon: Some("rocket".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Cta {
                label: "Sign Up".to_string(),
                href: "/signup".to_string(),
                primary: true,
            }
        );
    }

    #[test]
    fn native_testimonial() {
        let block = Block::Testimonial {
            content: "Great!".to_string(),
            author: Some("Jane".to_string()),
            role: Some("Eng".to_string()),
            company: Some("Acme".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Testimonial {
                content: "Great!".to_string(),
                author: Some("Jane".to_string()),
                role: Some("Eng".to_string()),
                company: Some("Acme".to_string()),
            }
        );
    }

    #[test]
    fn native_faq() {
        let block = Block::Faq {
            items: vec![FaqItem {
                question: "Free?".to_string(),
                answer: "Yes.".to_string(),
            }],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Faq {
                items: vec![NativeFaqItem {
                    question: "Free?".to_string(),
                    answer: "Yes.".to_string(),
                }],
            }
        );
    }

    #[test]
    fn native_details() {
        let block = Block::Details {
            title: Some("More info".to_string()),
            open: true,
            content: "Hidden content".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Details {
                title: Some("More info".to_string()),
                open: true,
                content: "Hidden content".to_string(),
            }
        );
    }

    #[test]
    fn native_divider() {
        let block = Block::Divider {
            label: Some("Section".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Divider {
                label: Some("Section".to_string()),
            }
        );
    }

    #[test]
    fn native_divider_no_label() {
        let block = Block::Divider {
            label: None,
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Divider { label: None }
        );
    }

    #[test]
    fn native_hero() {
        let block = Block::Hero {
            headline: Some("Welcome".to_string()),
            subtitle: Some("To SurfDoc".to_string()),
            badge: Some("New".to_string()),
            align: "center".to_string(),
            image: Some("hero.png".to_string()),
            buttons: vec![],
            content: "Some content".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Hero {
                headline: Some("Welcome".to_string()),
                subtitle: Some("To SurfDoc".to_string()),
                badge: Some("New".to_string()),
            }
        );
    }

    #[test]
    fn native_features() {
        let block = Block::Features {
            cards: vec![FeatureCard {
                title: "Fast".to_string(),
                icon: Some("bolt".to_string()),
                body: "Very fast.".to_string(),
                link_label: Some("Learn more".to_string()),
                link_href: Some("/fast".to_string()),
            }],
            cols: Some(2),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Features {
                cards: vec![NativeFeatureCard {
                    title: "Fast".to_string(),
                    icon: Some("bolt".to_string()),
                    body: "Very fast.".to_string(),
                }],
            }
        );
    }

    #[test]
    fn native_steps() {
        let block = Block::Steps {
            steps: vec![StepItem {
                title: "Step 1".to_string(),
                time: Some("5 min".to_string()),
                body: "Do this".to_string(),
            }],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Steps {
                steps: vec![NativeStepItem {
                    title: "Step 1".to_string(),
                    time: Some("5 min".to_string()),
                    body: "Do this".to_string(),
                }],
            }
        );
    }

    #[test]
    fn native_stats() {
        let block = Block::Stats {
            items: vec![StatItem {
                value: "99%".to_string(),
                label: "Uptime".to_string(),
                color: Some("green".to_string()),
            }],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Stats {
                items: vec![NativeStatItem {
                    value: "99%".to_string(),
                    label: "Uptime".to_string(),
                    color: Some("green".to_string()),
                }],
            }
        );
    }

    #[test]
    fn native_comparison() {
        let block = Block::Comparison {
            headers: vec!["".to_string(), "Free".to_string(), "Pro".to_string()],
            rows: vec![vec![
                "Storage".to_string(),
                "1GB".to_string(),
                "100GB".to_string(),
            ]],
            highlight: Some("Pro".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Comparison {
                headers: vec!["".to_string(), "Free".to_string(), "Pro".to_string()],
                rows: vec![vec![
                    "Storage".to_string(),
                    "1GB".to_string(),
                    "100GB".to_string(),
                ]],
                highlight: Some("Pro".to_string()),
            }
        );
    }

    #[test]
    fn native_toc() {
        let block = Block::Toc {
            depth: 3,
            entries: vec![TocEntry {
                text: "Intro".to_string(),
                id: "intro".to_string(),
                level: 1,
            }],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Toc {
                depth: 3,
                entries: vec![NativeTocEntry {
                    text: "Intro".to_string(),
                    id: "intro".to_string(),
                    level: 1,
                }],
            }
        );
    }

    #[test]
    fn native_before_after() {
        let block = Block::BeforeAfter {
            before_items: vec![BeforeAfterItem {
                label: "Old".to_string(),
                detail: "Slow".to_string(),
            }],
            after_items: vec![BeforeAfterItem {
                label: "New".to_string(),
                detail: "Fast".to_string(),
            }],
            transition: Some("SurfDoc".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::BeforeAfter {
                before_items: vec![NativeBeforeAfterItem {
                    label: "Old".to_string(),
                    detail: "Slow".to_string(),
                }],
                after_items: vec![NativeBeforeAfterItem {
                    label: "New".to_string(),
                    detail: "Fast".to_string(),
                }],
                transition: Some("SurfDoc".to_string()),
            }
        );
    }

    #[test]
    fn native_pipeline() {
        let block = Block::Pipeline {
            steps: vec![PipelineStep {
                label: "Parse".to_string(),
                description: Some("tokenize".to_string()),
            }],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block),
            NativeBlock::Pipeline {
                steps: vec![NativePipelineStep {
                    label: "Parse".to_string(),
                    description: Some("tokenize".to_string()),
                }],
            }
        );
    }

    // ── Fallback tests ──────────────────────────────────────────────

    #[test]
    fn fallback_unknown() {
        let block = Block::Unknown {
            name: "custom".to_string(),
            attrs: BTreeMap::new(),
            content: "some content".to_string(),
            span: syn(),
        };
        match convert_block(&block) {
            NativeBlock::Markdown { content } => {
                assert!(
                    content.contains("custom"),
                    "Fallback should contain block name: {content}"
                );
            }
            other => panic!("Expected Markdown fallback, got {:?}", other),
        }
    }

    #[test]
    fn fallback_nav() {
        let block = Block::Nav {
            items: vec![NavItem {
                label: "Home".to_string(),
                href: "/".to_string(),
                icon: None,
            }],
            logo: None,
            span: syn(),
        };
        match convert_block(&block) {
            NativeBlock::Markdown { .. } => {}
            other => panic!("Expected Markdown fallback, got {:?}", other),
        }
    }

    #[test]
    fn fallback_hero_image() {
        let block = Block::HeroImage {
            src: "hero.png".to_string(),
            alt: Some("Shot".to_string()),
            span: syn(),
        };
        match convert_block(&block) {
            NativeBlock::Markdown { content } => {
                assert!(content.contains("hero.png"));
            }
            other => panic!("Expected Markdown fallback, got {:?}", other),
        }
    }

    #[test]
    fn fallback_style_empty() {
        let block = Block::Style {
            properties: vec![StyleProperty {
                key: "bg".to_string(),
                value: "blue".to_string(),
            }],
            span: syn(),
        };
        match convert_block(&block) {
            NativeBlock::Markdown { .. } => {}
            other => panic!("Expected Markdown fallback, got {:?}", other),
        }
    }

    #[test]
    fn to_native_blocks_multi_block() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![
                Block::Markdown {
                    content: "Hello".to_string(),
                    span: syn(),
                },
                Block::Callout {
                    callout_type: CalloutType::Info,
                    title: None,
                    content: "Note".to_string(),
                    span: syn(),
                },
                Block::Nav {
                    items: vec![],
                    logo: None,
                    span: syn(),
                },
            ],
            source: String::new(),
        };
        let native = to_native_blocks(&doc);
        assert_eq!(native.len(), 3);
        assert!(matches!(&native[0], NativeBlock::Markdown { .. }));
        assert!(matches!(&native[1], NativeBlock::Callout { .. }));
        assert!(matches!(&native[2], NativeBlock::Markdown { .. })); // Nav falls back
    }

    #[test]
    fn to_native_blocks_empty_doc() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![],
            source: String::new(),
        };
        let native = to_native_blocks(&doc);
        assert!(native.is_empty());
    }
}

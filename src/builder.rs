//! Programmatic SurfDoc builder and `.surf` source serializer.
//!
//! The [`SurfDocBuilder`] provides a fluent API for constructing `SurfDoc`
//! documents without writing raw `.surf` text.  The [`to_surf_source`] function
//! serializes any `SurfDoc` back to valid `.surf` format that round-trips
//! through [`crate::parse`].

use crate::types::{
    Block, CalloutType, ColumnContent, DataFormat, DecisionStatus, EmbedType, FaqItem,
    FooterSection, FormField, FormFieldType, FrontMatter, GalleryItem, NavItem, SocialLink, Span,
    StyleProperty, SurfDoc, TabPanel, TaskItem, Trend,
};

// -----------------------------------------------------------------------
// SurfDocBuilder
// -----------------------------------------------------------------------

/// Fluent builder for constructing `SurfDoc` documents programmatically.
///
/// # Example
///
/// ```
/// use surf_parse::builder::SurfDocBuilder;
/// use surf_parse::types::CalloutType;
///
/// let doc = SurfDocBuilder::new()
///     .title("My Doc")
///     .heading(1, "Welcome")
///     .callout(CalloutType::Info, "Important note")
///     .build();
///
/// assert_eq!(doc.blocks.len(), 2);
/// ```
pub struct SurfDocBuilder {
    front_matter: Option<FrontMatter>,
    blocks: Vec<Block>,
}

impl Default for SurfDocBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfDocBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        SurfDocBuilder {
            front_matter: None,
            blocks: Vec::new(),
        }
    }

    // -- Front matter setters -------------------------------------------

    /// Set the document title.
    pub fn title(mut self, title: &str) -> Self {
        self.ensure_front_matter().title = Some(title.to_string());
        self
    }

    /// Set the document type.
    pub fn doc_type(mut self, dt: crate::types::DocType) -> Self {
        self.ensure_front_matter().doc_type = Some(dt);
        self
    }

    /// Set the document status.
    pub fn status(mut self, s: crate::types::DocStatus) -> Self {
        self.ensure_front_matter().status = Some(s);
        self
    }

    /// Set the document author.
    pub fn author(mut self, author: &str) -> Self {
        self.ensure_front_matter().author = Some(author.to_string());
        self
    }

    /// Set the document tags.
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.ensure_front_matter().tags = Some(tags);
        self
    }

    /// Set the document description.
    pub fn description(mut self, desc: &str) -> Self {
        self.ensure_front_matter().description = Some(desc.to_string());
        self
    }

    /// Set the entire front matter at once.
    pub fn front_matter(mut self, fm: FrontMatter) -> Self {
        self.front_matter = Some(fm);
        self
    }

    // -- Block methods --------------------------------------------------

    /// Add a raw markdown block.
    pub fn markdown(mut self, content: &str) -> Self {
        self.blocks.push(Block::Markdown {
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a markdown heading (syntactic sugar for `markdown("## text")`).
    pub fn heading(mut self, level: u8, text: &str) -> Self {
        let prefix = "#".repeat(level as usize);
        self.blocks.push(Block::Markdown {
            content: format!("{prefix} {text}"),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a callout block.
    pub fn callout(mut self, callout_type: CalloutType, content: &str) -> Self {
        self.blocks.push(Block::Callout {
            callout_type,
            title: None,
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a callout block with a title.
    pub fn callout_titled(
        mut self,
        callout_type: CalloutType,
        title: &str,
        content: &str,
    ) -> Self {
        self.blocks.push(Block::Callout {
            callout_type,
            title: Some(title.to_string()),
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a code block with optional language.
    pub fn code(mut self, content: &str, lang: Option<&str>) -> Self {
        self.blocks.push(Block::Code {
            lang: lang.map(|s| s.to_string()),
            file: None,
            highlight: vec![],
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a code block with language and file path.
    pub fn code_file(mut self, content: &str, lang: &str, file: &str) -> Self {
        self.blocks.push(Block::Code {
            lang: Some(lang.to_string()),
            file: Some(file.to_string()),
            highlight: vec![],
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a data table block.
    pub fn data_table(mut self, headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        // Build raw_content from headers + rows for round-tripping
        let mut raw_lines = Vec::new();
        if !headers.is_empty() {
            raw_lines.push(format!("| {} |", headers.join(" | ")));
            let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
            raw_lines.push(format!("| {} |", sep.join(" | ")));
        }
        for row in &rows {
            raw_lines.push(format!("| {} |", row.join(" | ")));
        }
        let raw_content = raw_lines.join("\n");

        self.blocks.push(Block::Data {
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers,
            rows,
            raw_content,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a single task item.
    pub fn task(mut self, text: &str, done: bool) -> Self {
        // If the last block is a Tasks block, append to it.
        if let Some(Block::Tasks { items, .. }) = self.blocks.last_mut() {
            items.push(TaskItem {
                done,
                text: text.to_string(),
                assignee: None,
            });
        } else {
            self.blocks.push(Block::Tasks {
                items: vec![TaskItem {
                    done,
                    text: text.to_string(),
                    assignee: None,
                }],
                span: Span::SYNTHETIC,
            });
        }
        self
    }

    /// Add a tasks block with multiple items.
    pub fn tasks(mut self, items: Vec<TaskItem>) -> Self {
        self.blocks.push(Block::Tasks {
            items,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a decision block.
    pub fn decision(mut self, status: DecisionStatus, content: &str) -> Self {
        self.blocks.push(Block::Decision {
            status,
            date: None,
            deciders: vec![],
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a metric block.
    pub fn metric(mut self, label: &str, value: &str) -> Self {
        self.blocks.push(Block::Metric {
            label: label.to_string(),
            value: value.to_string(),
            trend: None,
            unit: None,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a metric block with trend and optional unit.
    pub fn metric_with_trend(
        mut self,
        label: &str,
        value: &str,
        trend: Trend,
        unit: Option<&str>,
    ) -> Self {
        self.blocks.push(Block::Metric {
            label: label.to_string(),
            value: value.to_string(),
            trend: Some(trend),
            unit: unit.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a summary block.
    pub fn summary(mut self, content: &str) -> Self {
        self.blocks.push(Block::Summary {
            content: content.to_string(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a figure block.
    pub fn figure(mut self, src: &str) -> Self {
        self.blocks.push(Block::Figure {
            src: src.to_string(),
            caption: None,
            alt: None,
            width: None,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a figure block with caption and optional alt text.
    pub fn figure_with_caption(mut self, src: &str, caption: &str, alt: Option<&str>) -> Self {
        self.blocks.push(Block::Figure {
            src: src.to_string(),
            caption: Some(caption.to_string()),
            alt: alt.map(|s| s.to_string()),
            width: None,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a quote block.
    pub fn quote(mut self, content: &str) -> Self {
        self.blocks.push(Block::Quote {
            content: content.to_string(),
            attribution: None,
            cite: None,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a quote block with attribution.
    pub fn quote_attributed(mut self, content: &str, attribution: &str) -> Self {
        self.blocks.push(Block::Quote {
            content: content.to_string(),
            attribution: Some(attribution.to_string()),
            cite: None,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a call-to-action block.
    pub fn cta(mut self, label: &str, href: &str, primary: bool) -> Self {
        self.blocks.push(Block::Cta {
            label: label.to_string(),
            href: href.to_string(),
            primary,
            icon: None,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a hero image block.
    pub fn hero_image(mut self, src: &str, alt: Option<&str>) -> Self {
        self.blocks.push(Block::HeroImage {
            src: src.to_string(),
            alt: alt.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a testimonial block.
    pub fn testimonial(
        mut self,
        content: &str,
        author: Option<&str>,
        role: Option<&str>,
        company: Option<&str>,
    ) -> Self {
        self.blocks.push(Block::Testimonial {
            content: content.to_string(),
            author: author.map(|s| s.to_string()),
            role: role.map(|s| s.to_string()),
            company: company.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a style block.
    pub fn style(mut self, properties: Vec<StyleProperty>) -> Self {
        self.blocks.push(Block::Style {
            properties,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add an FAQ block.
    pub fn faq(mut self, items: Vec<FaqItem>) -> Self {
        self.blocks.push(Block::Faq {
            items,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a pricing table block.
    pub fn pricing_table(mut self, headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        self.blocks.push(Block::PricingTable {
            headers,
            rows,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a site configuration block.
    pub fn site(mut self, domain: Option<&str>, properties: Vec<StyleProperty>) -> Self {
        self.blocks.push(Block::Site {
            domain: domain.map(|s| s.to_string()),
            properties,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a page block.
    pub fn page(
        mut self,
        route: &str,
        layout: Option<&str>,
        title: Option<&str>,
        content: &str,
    ) -> Self {
        // Parse children from content (same as the parser does)
        let children = Vec::new(); // children are re-parsed on round-trip
        self.blocks.push(Block::Page {
            route: route.to_string(),
            layout: layout.map(|s| s.to_string()),
            title: title.map(|s| s.to_string()),
            sidebar: false,
            content: content.to_string(),
            children,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a navigation block.
    pub fn nav(mut self, items: Vec<NavItem>, logo: Option<&str>) -> Self {
        self.blocks.push(Block::Nav {
            items,
            logo: logo.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add an embed block.
    pub fn embed(
        mut self,
        src: &str,
        embed_type: Option<EmbedType>,
        title: Option<&str>,
    ) -> Self {
        self.blocks.push(Block::Embed {
            src: src.to_string(),
            embed_type,
            width: None,
            height: None,
            title: title.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a form block.
    pub fn form(mut self, fields: Vec<FormField>, submit_label: Option<&str>) -> Self {
        self.blocks.push(Block::Form {
            fields,
            submit_label: submit_label.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a gallery block.
    pub fn gallery(mut self, items: Vec<GalleryItem>, columns: Option<u32>) -> Self {
        self.blocks.push(Block::Gallery {
            items,
            columns,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a footer block.
    pub fn footer(
        mut self,
        sections: Vec<FooterSection>,
        copyright: Option<&str>,
        social: Vec<SocialLink>,
    ) -> Self {
        self.blocks.push(Block::Footer {
            sections,
            copyright: copyright.map(|s| s.to_string()),
            social,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a tabs block.
    pub fn tabs(mut self, tabs: Vec<TabPanel>) -> Self {
        self.blocks.push(Block::Tabs {
            tabs,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a columns block.
    pub fn columns(mut self, columns: Vec<ColumnContent>) -> Self {
        self.blocks.push(Block::Columns {
            columns,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Consume the builder and produce a `SurfDoc`.
    pub fn build(self) -> SurfDoc {
        let source = to_surf_source_inner(&self.front_matter, &self.blocks);
        SurfDoc {
            front_matter: self.front_matter,
            blocks: self.blocks,
            source,
        }
    }

    // -- Internal helpers -----------------------------------------------

    fn ensure_front_matter(&mut self) -> &mut FrontMatter {
        if self.front_matter.is_none() {
            self.front_matter = Some(FrontMatter::default());
        }
        self.front_matter.as_mut().unwrap()
    }
}

// -----------------------------------------------------------------------
// to_surf_source — serializer
// -----------------------------------------------------------------------

/// Serialize a `SurfDoc` to valid `.surf` format text.
///
/// The output can be parsed back with [`crate::parse`] to produce an
/// equivalent document (round-trip).
pub fn to_surf_source(doc: &SurfDoc) -> String {
    to_surf_source_inner(&doc.front_matter, &doc.blocks)
}

fn to_surf_source_inner(front_matter: &Option<FrontMatter>, blocks: &[Block]) -> String {
    let mut out = String::new();

    // Front matter
    if let Some(fm) = front_matter {
        out.push_str(&serialize_front_matter(fm));
    }

    // Blocks — separate each pair with a blank line.
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 || front_matter.is_some() {
            out.push('\n');
        }
        out.push_str(&serialize_block(block));
        out.push('\n');
    }

    out
}

// -----------------------------------------------------------------------
// Front matter serialization
// -----------------------------------------------------------------------

fn serialize_front_matter(fm: &FrontMatter) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());

    if let Some(title) = &fm.title {
        lines.push(format!("title: \"{}\"", escape_yaml_string(title)));
    }
    if let Some(dt) = &fm.doc_type {
        lines.push(format!("type: {}", doc_type_str(*dt)));
    }
    if let Some(s) = &fm.status {
        lines.push(format!("status: {}", doc_status_str(*s)));
    }
    if let Some(scope) = &fm.scope {
        lines.push(format!("scope: {}", scope_str(*scope)));
    }
    if let Some(tags) = &fm.tags {
        let tag_strs: Vec<String> = tags.iter().map(|t| t.clone()).collect();
        lines.push(format!("tags: [{}]", tag_strs.join(", ")));
    }
    if let Some(created) = &fm.created {
        lines.push(format!("created: \"{}\"", escape_yaml_string(created)));
    }
    if let Some(updated) = &fm.updated {
        lines.push(format!("updated: \"{}\"", escape_yaml_string(updated)));
    }
    if let Some(author) = &fm.author {
        lines.push(format!("author: \"{}\"", escape_yaml_string(author)));
    }
    if let Some(confidence) = &fm.confidence {
        lines.push(format!("confidence: {}", confidence_str(*confidence)));
    }
    if let Some(version) = &fm.version {
        lines.push(format!("version: {version}"));
    }
    if let Some(contributors) = &fm.contributors {
        let cs: Vec<String> = contributors.iter().map(|c| format!("\"{}\"", escape_yaml_string(c))).collect();
        lines.push(format!("contributors: [{}]", cs.join(", ")));
    }
    if let Some(description) = &fm.description {
        lines.push(format!("description: \"{}\"", escape_yaml_string(description)));
    }
    if let Some(workspace) = &fm.workspace {
        lines.push(format!("workspace: \"{}\"", escape_yaml_string(workspace)));
    }
    if let Some(decision) = &fm.decision {
        lines.push(format!("decision: \"{}\"", escape_yaml_string(decision)));
    }
    if let Some(related) = &fm.related {
        if !related.is_empty() {
            lines.push("related:".to_string());
            for r in related {
                let mut entry = format!("  - path: \"{}\"", escape_yaml_string(&r.path));
                if let Some(rel) = &r.relationship {
                    entry = format!(
                        "  - path: \"{}\"\n    relationship: {}",
                        escape_yaml_string(&r.path),
                        relationship_str(*rel)
                    );
                }
                lines.push(entry);
            }
        }
    }
    // Extra fields
    for (key, value) in &fm.extra {
        lines.push(format!("{key}: {}", serde_yaml_value_to_inline(value)));
    }

    lines.push("---".to_string());
    let mut result = lines.join("\n");
    result.push('\n');
    result
}

fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn doc_type_str(dt: crate::types::DocType) -> &'static str {
    use crate::types::DocType;
    match dt {
        DocType::Doc => "doc",
        DocType::Guide => "guide",
        DocType::Conversation => "conversation",
        DocType::Plan => "plan",
        DocType::Agent => "agent",
        DocType::Preference => "preference",
        DocType::Report => "report",
        DocType::Proposal => "proposal",
        DocType::Incident => "incident",
        DocType::Review => "review",
    }
}

fn doc_status_str(s: crate::types::DocStatus) -> &'static str {
    use crate::types::DocStatus;
    match s {
        DocStatus::Draft => "draft",
        DocStatus::Active => "active",
        DocStatus::Closed => "closed",
        DocStatus::Archived => "archived",
    }
}

fn scope_str(s: crate::types::Scope) -> &'static str {
    use crate::types::Scope;
    match s {
        Scope::Personal => "personal",
        Scope::WorkspacePrivate => "workspace-private",
        Scope::Workspace => "workspace",
        Scope::Repo => "repo",
        Scope::Public => "public",
    }
}

fn confidence_str(c: crate::types::Confidence) -> &'static str {
    use crate::types::Confidence;
    match c {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn relationship_str(r: crate::types::Relationship) -> &'static str {
    use crate::types::Relationship;
    match r {
        Relationship::Produces => "produces",
        Relationship::Consumes => "consumes",
        Relationship::References => "references",
        Relationship::Supersedes => "supersedes",
    }
}

fn serde_yaml_value_to_inline(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => format!("\"{}\"", escape_yaml_string(s)),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => "null".to_string(),
        _ => serde_yaml::to_string(value).unwrap_or_default().trim().to_string(),
    }
}

// -----------------------------------------------------------------------
// Block serialization
// -----------------------------------------------------------------------

fn serialize_block(block: &Block) -> String {
    match block {
        Block::Markdown { content, .. } => {
            // Trim leading/trailing blank lines to prevent blank-line
            // accumulation on round-trips. The separator logic between blocks
            // already inserts the blank line.
            let trimmed = content.trim_matches('\n');
            trimmed.to_string()
        }

        Block::Callout {
            callout_type,
            title,
            content,
            ..
        } => {
            let type_str = callout_type_str(*callout_type);
            let attrs = match title {
                Some(t) => format!("[type={type_str} title=\"{}\"]", escape_attr(t)),
                None => format!("[type={type_str}]"),
            };
            if content.is_empty() {
                format!("::callout{attrs}\n::")
            } else {
                format!("::callout{attrs}\n{content}\n::")
            }
        }

        Block::Data {
            id,
            format,
            sortable,
            headers,
            rows,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(id) = id {
                attr_parts.push(format!("id=\"{}\"", escape_attr(id)));
            }
            let fmt = match format {
                DataFormat::Table => "table",
                DataFormat::Csv => "csv",
                DataFormat::Json => "json",
            };
            attr_parts.push(format!("format={fmt}"));
            if *sortable {
                attr_parts.push("sortable".to_string());
            }
            let attrs = format!("[{}]", attr_parts.join(" "));

            let mut content_lines = Vec::new();
            if !headers.is_empty() {
                content_lines.push(format!("| {} |", headers.join(" | ")));
                let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
                content_lines.push(format!("| {} |", sep.join(" | ")));
            }
            for row in rows {
                content_lines.push(format!("| {} |", row.join(" | ")));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::data{attrs}\n::")
            } else {
                format!("::data{attrs}\n{content}\n::")
            }
        }

        Block::Code {
            lang,
            file,
            content,
            highlight,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(l) = lang {
                attr_parts.push(format!("lang={l}"));
            }
            if let Some(f) = file {
                attr_parts.push(format!("file=\"{}\"", escape_attr(f)));
            }
            if !highlight.is_empty() {
                attr_parts.push(format!("highlight=\"{}\"", highlight.join(",")));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            if content.is_empty() {
                format!("::code{attrs}\n::")
            } else {
                format!("::code{attrs}\n{content}\n::")
            }
        }

        Block::Tasks { items, .. } => {
            let mut lines = Vec::new();
            for item in items {
                let check = if item.done { "x" } else { " " };
                match &item.assignee {
                    Some(a) => lines.push(format!("- [{check}] {} @{a}", item.text)),
                    None => lines.push(format!("- [{check}] {}", item.text)),
                }
            }
            let content = lines.join("\n");
            format!("::tasks\n{content}\n::")
        }

        Block::Decision {
            status,
            date,
            deciders,
            content,
            ..
        } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("status={}", decision_status_str(*status)));
            if let Some(d) = date {
                attr_parts.push(format!("date=\"{}\"", escape_attr(d)));
            }
            if !deciders.is_empty() {
                attr_parts.push(format!("deciders=\"{}\"", deciders.join(",")));
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            if content.is_empty() {
                format!("::decision{attrs}\n::")
            } else {
                format!("::decision{attrs}\n{content}\n::")
            }
        }

        Block::Metric {
            label,
            value,
            trend,
            unit,
            ..
        } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("label=\"{}\"", escape_attr(label)));
            attr_parts.push(format!("value=\"{}\"", escape_attr(value)));
            if let Some(t) = trend {
                attr_parts.push(format!("trend={}", trend_str(*t)));
            }
            if let Some(u) = unit {
                attr_parts.push(format!("unit=\"{}\"", escape_attr(u)));
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            format!("::metric{attrs}\n::")
        }

        Block::Summary { content, .. } => {
            if content.is_empty() {
                "::summary\n::".to_string()
            } else {
                format!("::summary\n{content}\n::")
            }
        }

        Block::Figure {
            src,
            caption,
            alt,
            width,
            ..
        } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("src=\"{}\"", escape_attr(src)));
            if let Some(c) = caption {
                attr_parts.push(format!("caption=\"{}\"", escape_attr(c)));
            }
            if let Some(a) = alt {
                attr_parts.push(format!("alt=\"{}\"", escape_attr(a)));
            }
            if let Some(w) = width {
                attr_parts.push(format!("width=\"{}\"", escape_attr(w)));
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            format!("::figure{attrs}\n::")
        }

        Block::Tabs { tabs, .. } => {
            let mut content_parts = Vec::new();
            for tab in tabs {
                content_parts.push(format!("## {}", tab.label));
                if !tab.content.is_empty() {
                    content_parts.push(tab.content.clone());
                }
            }
            let content = content_parts.join("\n");
            format!("::tabs\n{content}\n::")
        }

        Block::Columns { columns, .. } => {
            let mut content_parts = Vec::new();
            for col in columns {
                content_parts.push(":::column".to_string());
                content_parts.push(col.content.clone());
                content_parts.push(":::".to_string());
            }
            let content = content_parts.join("\n");
            format!("::columns\n{content}\n::")
        }

        Block::Quote {
            content,
            attribution,
            cite,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(a) = attribution {
                attr_parts.push(format!("by=\"{}\"", escape_attr(a)));
            }
            if let Some(c) = cite {
                attr_parts.push(format!("cite=\"{}\"", escape_attr(c)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            if content.is_empty() {
                format!("::quote{attrs}\n::")
            } else {
                format!("::quote{attrs}\n{content}\n::")
            }
        }

        Block::Cta {
            label,
            href,
            primary,
            icon,
            ..
        } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("label=\"{}\"", escape_attr(label)));
            attr_parts.push(format!("href=\"{}\"", escape_attr(href)));
            if *primary {
                attr_parts.push("primary".to_string());
            }
            if let Some(i) = icon {
                attr_parts.push(format!("icon=\"{}\"", escape_attr(i)));
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            format!("::cta{attrs}\n::")
        }

        Block::Nav { items, logo, .. } => {
            let mut attr_parts = Vec::new();
            if let Some(l) = logo {
                attr_parts.push(format!("logo=\"{}\"", escape_attr(l)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for item in items {
                match &item.icon {
                    Some(icon) => content_lines.push(format!(
                        "- [{}]({}) {{icon={}}}",
                        item.label, item.href, icon
                    )),
                    None => content_lines.push(format!("- [{}]({})", item.label, item.href)),
                }
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::nav{attrs}\n::")
            } else {
                format!("::nav{attrs}\n{content}\n::")
            }
        }

        Block::HeroImage { src, alt, .. } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("src=\"{}\"", escape_attr(src)));
            if let Some(a) = alt {
                attr_parts.push(format!("alt=\"{}\"", escape_attr(a)));
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            format!("::hero-image{attrs}\n::")
        }

        Block::Testimonial {
            content,
            author,
            role,
            company,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(a) = author {
                attr_parts.push(format!("author=\"{}\"", escape_attr(a)));
            }
            if let Some(r) = role {
                attr_parts.push(format!("role=\"{}\"", escape_attr(r)));
            }
            if let Some(c) = company {
                attr_parts.push(format!("company=\"{}\"", escape_attr(c)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            if content.is_empty() {
                format!("::testimonial{attrs}\n::")
            } else {
                format!("::testimonial{attrs}\n{content}\n::")
            }
        }

        Block::Style { properties, .. } => {
            let mut lines = Vec::new();
            for p in properties {
                lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = lines.join("\n");
            if content.is_empty() {
                "::style\n::".to_string()
            } else {
                format!("::style\n{content}\n::")
            }
        }

        Block::Faq { items, .. } => {
            let mut content_parts = Vec::new();
            for item in items {
                content_parts.push(format!("### {}", item.question));
                if !item.answer.is_empty() {
                    content_parts.push(item.answer.clone());
                }
            }
            let content = content_parts.join("\n");
            format!("::faq\n{content}\n::")
        }

        Block::PricingTable { headers, rows, .. } => {
            let mut content_lines = Vec::new();
            if !headers.is_empty() {
                content_lines.push(format!("| {} |", headers.join(" | ")));
                let sep: Vec<&str> = headers.iter().map(|_| "---").collect();
                content_lines.push(format!("| {} |", sep.join(" | ")));
            }
            for row in rows {
                content_lines.push(format!("| {} |", row.join(" | ")));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::pricing-table\n::".to_string()
            } else {
                format!("::pricing-table\n{content}\n::")
            }
        }

        Block::Site {
            domain,
            properties,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(d) = domain {
                attr_parts.push(format!("domain=\"{}\"", escape_attr(d)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for p in properties {
                content_lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::site{attrs}\n::")
            } else {
                format!("::site{attrs}\n{content}\n::")
            }
        }

        Block::Page {
            route,
            layout,
            title,
            sidebar,
            content,
            ..
        } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("route=\"{}\"", escape_attr(route)));
            if let Some(l) = layout {
                attr_parts.push(format!("layout=\"{}\"", escape_attr(l)));
            }
            if let Some(t) = title {
                attr_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            if *sidebar {
                attr_parts.push("sidebar".to_string());
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            if content.is_empty() {
                format!("::page{attrs}\n::")
            } else {
                format!("::page{attrs}\n{content}\n::")
            }
        }

        Block::Embed {
            src,
            embed_type,
            width,
            height,
            title,
            ..
        } => {
            let mut attr_parts = Vec::new();
            attr_parts.push(format!("src=\"{}\"", escape_attr(src)));
            if let Some(et) = embed_type {
                attr_parts.push(format!("type={}", embed_type_str(*et)));
            }
            if let Some(w) = width {
                attr_parts.push(format!("width=\"{}\"", escape_attr(w)));
            }
            if let Some(h) = height {
                attr_parts.push(format!("height=\"{}\"", escape_attr(h)));
            }
            if let Some(t) = title {
                attr_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            let attrs = format!("[{}]", attr_parts.join(" "));
            format!("::embed{attrs}\n::")
        }

        Block::Form {
            fields,
            submit_label,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(s) = submit_label {
                attr_parts.push(format!("submit=\"{}\"", escape_attr(s)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for field in fields {
                let req = if field.required { " *" } else { "" };
                let type_str = form_field_type_str(field.field_type);
                match field.field_type {
                    FormFieldType::Select if !field.options.is_empty() => {
                        content_lines.push(format!(
                            "- {} (select: {}){req}",
                            field.label,
                            field.options.join(" | ")
                        ));
                    }
                    _ => {
                        if let Some(ph) = &field.placeholder {
                            content_lines.push(format!(
                                "- {} ({type_str}, \"{ph}\"){req}",
                                field.label
                            ));
                        } else {
                            content_lines.push(format!("- {} ({type_str}){req}", field.label));
                        }
                    }
                }
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::form{attrs}\n::")
            } else {
                format!("::form{attrs}\n{content}\n::")
            }
        }

        Block::Gallery { items, .. } => {
            let mut content_lines = Vec::new();
            for item in items {
                let alt = item.alt.as_deref().unwrap_or("");
                let suffix = match (&item.category, &item.caption) {
                    (Some(cat), Some(cap)) => format!(" {cat}: {cap}"),
                    (None, Some(cap)) => format!(" {cap}"),
                    (Some(cat), None) => format!(" {cat}:"),
                    (None, None) => String::new(),
                };
                content_lines.push(format!("![{alt}]({}){suffix}", item.src));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::gallery\n::".to_string()
            } else {
                format!("::gallery\n{content}\n::")
            }
        }

        Block::Footer {
            sections,
            copyright,
            social,
            ..
        } => {
            let mut content_lines = Vec::new();
            for section in sections {
                content_lines.push(format!("## {}", section.heading));
                for link in &section.links {
                    if link.href.is_empty() {
                        content_lines.push(format!("- {}", link.label));
                    } else {
                        content_lines.push(format!("- [{}]({})", link.label, link.href));
                    }
                }
            }
            for link in social {
                content_lines.push(format!("@{} {}", link.platform, link.href));
            }
            if let Some(cr) = copyright {
                content_lines.push(cr.clone());
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::footer\n::".to_string()
            } else {
                format!("::footer\n{content}\n::")
            }
        }

        Block::Unknown {
            name,
            attrs,
            content,
            ..
        } => {
            let attrs_str = if attrs.is_empty() {
                String::new()
            } else {
                format!("[{}]", serialize_attrs(attrs))
            };
            if content.is_empty() {
                format!("::{name}{attrs_str}\n::")
            } else {
                format!("::{name}{attrs_str}\n{content}\n::")
            }
        }

        Block::Details { title, open, content, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(t) = title {
                attrs_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            if *open {
                attrs_parts.push("open".to_string());
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::details{attrs_str}\n{content}\n::")
        }

        Block::Divider { label, .. } => {
            match label {
                Some(l) => format!("::divider[label=\"{}\"]", escape_attr(l)),
                None => "::divider".to_string(),
            }
        }
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

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

fn embed_type_str(et: EmbedType) -> &'static str {
    match et {
        EmbedType::Map => "map",
        EmbedType::Video => "video",
        EmbedType::Audio => "audio",
        EmbedType::Generic => "generic",
    }
}

fn form_field_type_str(ft: FormFieldType) -> &'static str {
    match ft {
        FormFieldType::Text => "text",
        FormFieldType::Email => "email",
        FormFieldType::Tel => "tel",
        FormFieldType::Date => "date",
        FormFieldType::Number => "number",
        FormFieldType::Select => "select",
        FormFieldType::Textarea => "textarea",
    }
}

/// Escape a string value for use inside `[key="value"]` attribute brackets.
fn escape_attr(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Serialize an `Attrs` map to a string suitable for inside `[...]`.
fn serialize_attrs(attrs: &crate::types::Attrs) -> String {
    let mut parts = Vec::new();
    for (key, value) in attrs {
        match value {
            crate::types::AttrValue::String(s) => {
                if s.contains(' ') || s.contains('"') {
                    parts.push(format!("{key}=\"{}\"", escape_attr(s)));
                } else {
                    parts.push(format!("{key}={s}"));
                }
            }
            crate::types::AttrValue::Number(n) => parts.push(format!("{key}={n}")),
            crate::types::AttrValue::Bool(true) => parts.push(key.clone()),
            crate::types::AttrValue::Bool(false) => parts.push(format!("{key}=false")),
            crate::types::AttrValue::Null => parts.push(format!("{key}=null")),
        }
    }
    parts.join(" ")
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use crate::types::*;

    // === Builder tests ==================================================

    #[test]
    fn test_empty_doc() {
        let doc = SurfDocBuilder::new().build();
        assert!(doc.front_matter.is_none());
        assert!(doc.blocks.is_empty());
    }

    #[test]
    fn test_with_front_matter() {
        let doc = SurfDocBuilder::new()
            .title("My Doc")
            .doc_type(DocType::Guide)
            .status(DocStatus::Active)
            .author("Brady")
            .build();

        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.title.unwrap(), "My Doc");
        assert_eq!(fm.doc_type.unwrap(), DocType::Guide);
        assert_eq!(fm.status.unwrap(), DocStatus::Active);
        assert_eq!(fm.author.unwrap(), "Brady");
    }

    #[test]
    fn test_front_matter_tags_and_description() {
        let doc = SurfDocBuilder::new()
            .tags(vec!["rust".into(), "parser".into()])
            .description("A test document")
            .build();

        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.tags.unwrap(), vec!["rust", "parser"]);
        assert_eq!(fm.description.unwrap(), "A test document");
    }

    #[test]
    fn test_set_full_front_matter() {
        let mut fm = FrontMatter::default();
        fm.title = Some("Override".into());
        fm.version = Some(3);

        let doc = SurfDocBuilder::new().front_matter(fm).build();
        let fm = doc.front_matter.unwrap();
        assert_eq!(fm.title.unwrap(), "Override");
        assert_eq!(fm.version.unwrap(), 3);
    }

    #[test]
    fn test_markdown_block() {
        let doc = SurfDocBuilder::new()
            .markdown("Hello **world**")
            .build();

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Markdown { content, .. } => assert_eq!(content, "Hello **world**"),
            _ => panic!("Expected Markdown block"),
        }
    }

    #[test]
    fn test_heading_sugar() {
        let doc = SurfDocBuilder::new().heading(2, "Foo").build();

        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Markdown { content, .. } => assert_eq!(content, "## Foo"),
            _ => panic!("Expected Markdown block"),
        }
    }

    #[test]
    fn test_callout() {
        let doc = SurfDocBuilder::new()
            .callout(CalloutType::Warning, "Be careful!")
            .build();

        match &doc.blocks[0] {
            Block::Callout {
                callout_type,
                title,
                content,
                ..
            } => {
                assert_eq!(*callout_type, CalloutType::Warning);
                assert!(title.is_none());
                assert_eq!(content, "Be careful!");
            }
            _ => panic!("Expected Callout block"),
        }
    }

    #[test]
    fn test_callout_titled() {
        let doc = SurfDocBuilder::new()
            .callout_titled(CalloutType::Tip, "Pro Tip", "Use the builder!")
            .build();

        match &doc.blocks[0] {
            Block::Callout {
                callout_type,
                title,
                content,
                ..
            } => {
                assert_eq!(*callout_type, CalloutType::Tip);
                assert_eq!(title.as_deref(), Some("Pro Tip"));
                assert_eq!(content, "Use the builder!");
            }
            _ => panic!("Expected Callout block"),
        }
    }

    #[test]
    fn test_code_block() {
        let doc = SurfDocBuilder::new()
            .code("fn main() {}", Some("rust"))
            .build();

        match &doc.blocks[0] {
            Block::Code {
                lang,
                file,
                content,
                ..
            } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert!(file.is_none());
                assert_eq!(content, "fn main() {}");
            }
            _ => panic!("Expected Code block"),
        }
    }

    #[test]
    fn test_code_file() {
        let doc = SurfDocBuilder::new()
            .code_file("fn main() {}", "rust", "src/main.rs")
            .build();

        match &doc.blocks[0] {
            Block::Code {
                lang,
                file,
                content,
                ..
            } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(file.as_deref(), Some("src/main.rs"));
                assert_eq!(content, "fn main() {}");
            }
            _ => panic!("Expected Code block"),
        }
    }

    #[test]
    fn test_data_table() {
        let doc = SurfDocBuilder::new()
            .data_table(
                vec!["Name".into(), "Age".into()],
                vec![vec!["Alice".into(), "30".into()]],
            )
            .build();

        match &doc.blocks[0] {
            Block::Data {
                headers, rows, ..
            } => {
                assert_eq!(headers, &vec!["Name".to_string(), "Age".to_string()]);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0], vec!["Alice".to_string(), "30".to_string()]);
            }
            _ => panic!("Expected Data block"),
        }
    }

    #[test]
    fn test_single_task() {
        let doc = SurfDocBuilder::new()
            .task("Write tests", false)
            .task("Ship it", true)
            .build();

        // Both tasks should be in one Tasks block
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            Block::Tasks { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(!items[0].done);
                assert_eq!(items[0].text, "Write tests");
                assert!(items[1].done);
                assert_eq!(items[1].text, "Ship it");
            }
            _ => panic!("Expected Tasks block"),
        }
    }

    #[test]
    fn test_tasks() {
        let items = vec![
            TaskItem {
                done: true,
                text: "Done".into(),
                assignee: Some("brady".into()),
            },
            TaskItem {
                done: false,
                text: "Todo".into(),
                assignee: None,
            },
        ];
        let doc = SurfDocBuilder::new().tasks(items).build();

        match &doc.blocks[0] {
            Block::Tasks { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].assignee.as_deref(), Some("brady"));
            }
            _ => panic!("Expected Tasks block"),
        }
    }

    #[test]
    fn test_decision() {
        let doc = SurfDocBuilder::new()
            .decision(DecisionStatus::Accepted, "We chose Rust.")
            .build();

        match &doc.blocks[0] {
            Block::Decision {
                status, content, ..
            } => {
                assert_eq!(*status, DecisionStatus::Accepted);
                assert_eq!(content, "We chose Rust.");
            }
            _ => panic!("Expected Decision block"),
        }
    }

    #[test]
    fn test_metric() {
        let doc = SurfDocBuilder::new().metric("MRR", "$2K").build();

        match &doc.blocks[0] {
            Block::Metric {
                label,
                value,
                trend,
                ..
            } => {
                assert_eq!(label, "MRR");
                assert_eq!(value, "$2K");
                assert!(trend.is_none());
            }
            _ => panic!("Expected Metric block"),
        }
    }

    #[test]
    fn test_metric_with_trend() {
        let doc = SurfDocBuilder::new()
            .metric_with_trend("Revenue", "$10K", Trend::Up, Some("USD"))
            .build();

        match &doc.blocks[0] {
            Block::Metric {
                label,
                value,
                trend,
                unit,
                ..
            } => {
                assert_eq!(label, "Revenue");
                assert_eq!(value, "$10K");
                assert_eq!(*trend, Some(Trend::Up));
                assert_eq!(unit.as_deref(), Some("USD"));
            }
            _ => panic!("Expected Metric block"),
        }
    }

    #[test]
    fn test_fluent_chaining() {
        let doc = SurfDocBuilder::new()
            .title("Test")
            .heading(1, "Hello")
            .callout(CalloutType::Info, "Note")
            .code("x = 1", Some("python"))
            .summary("A summary")
            .build();

        assert!(doc.front_matter.is_some());
        assert_eq!(doc.blocks.len(), 4);
    }

    #[test]
    fn test_default_impl() {
        let builder: SurfDocBuilder = Default::default();
        let doc = builder.build();
        assert!(doc.front_matter.is_none());
        assert!(doc.blocks.is_empty());
    }

    // === to_surf_source tests ==========================================

    #[test]
    fn test_serialize_markdown() {
        let doc = SurfDocBuilder::new()
            .markdown("# Hello World\n\nSome text here.")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("# Hello World"));
        assert!(source.contains("Some text here."));
        // No :: markers for markdown
        assert!(!source.contains("::"));
    }

    #[test]
    fn test_serialize_callout() {
        let doc = SurfDocBuilder::new()
            .callout(CalloutType::Warning, "Careful!")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::callout[type=warning]"));
        assert!(source.contains("Careful!"));
        assert!(source.contains("\n::"));
    }

    #[test]
    fn test_serialize_callout_with_title() {
        let doc = SurfDocBuilder::new()
            .callout_titled(CalloutType::Info, "My Title", "Content here")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::callout[type=info title=\"My Title\"]"));
        assert!(source.contains("Content here"));
    }

    #[test]
    fn test_serialize_code() {
        let doc = SurfDocBuilder::new()
            .code_file("fn main() {}", "rust", "src/main.rs")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::code[lang=rust file=\"src/main.rs\"]"));
        assert!(source.contains("fn main() {}"));
    }

    #[test]
    fn test_serialize_front_matter() {
        let doc = SurfDocBuilder::new()
            .title("My Doc")
            .doc_type(DocType::Guide)
            .status(DocStatus::Active)
            .author("Brady")
            .tags(vec!["rust".into(), "test".into()])
            .build();
        let source = to_surf_source(&doc);
        assert!(source.starts_with("---\n"));
        assert!(source.contains("title: \"My Doc\""));
        assert!(source.contains("type: guide"));
        assert!(source.contains("status: active"));
        assert!(source.contains("author: \"Brady\""));
        assert!(source.contains("tags: [rust, test]"));
    }

    #[test]
    fn test_serialize_metric() {
        let doc = SurfDocBuilder::new()
            .metric_with_trend("MRR", "$2K", Trend::Up, Some("USD"))
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::metric[label=\"MRR\" value=\"$2K\" trend=up unit=\"USD\"]"));
    }

    #[test]
    fn test_serialize_tasks() {
        let doc = SurfDocBuilder::new()
            .tasks(vec![
                TaskItem {
                    done: true,
                    text: "Done".into(),
                    assignee: Some("brady".into()),
                },
                TaskItem {
                    done: false,
                    text: "Todo".into(),
                    assignee: None,
                },
            ])
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::tasks"));
        assert!(source.contains("- [x] Done @brady"));
        assert!(source.contains("- [ ] Todo"));
    }

    #[test]
    fn test_serialize_decision() {
        let doc = SurfDocBuilder::new()
            .decision(DecisionStatus::Accepted, "Use Rust.")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::decision[status=accepted]"));
        assert!(source.contains("Use Rust."));
    }

    #[test]
    fn test_serialize_figure() {
        let doc = SurfDocBuilder::new()
            .figure_with_caption("arch.png", "Architecture", Some("Diagram"))
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::figure[src=\"arch.png\" caption=\"Architecture\" alt=\"Diagram\"]"));
    }

    #[test]
    fn test_serialize_quote() {
        let doc = SurfDocBuilder::new()
            .quote_attributed("The future is here.", "Alan Kay")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::quote[by=\"Alan Kay\"]"));
        assert!(source.contains("The future is here."));
    }

    #[test]
    fn test_serialize_cta() {
        let doc = SurfDocBuilder::new()
            .cta("Sign Up", "/signup", true)
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::cta[label=\"Sign Up\" href=\"/signup\" primary]"));
    }

    #[test]
    fn test_serialize_hero_image() {
        let doc = SurfDocBuilder::new()
            .hero_image("hero.png", Some("Hero"))
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::hero-image[src=\"hero.png\" alt=\"Hero\"]"));
    }

    #[test]
    fn test_serialize_summary() {
        let doc = SurfDocBuilder::new()
            .summary("Executive overview.")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::summary\nExecutive overview.\n::"));
    }

    #[test]
    fn test_serialize_site() {
        let doc = SurfDocBuilder::new()
            .site(
                Some("example.com"),
                vec![StyleProperty {
                    key: "theme".into(),
                    value: "dark".into(),
                }],
            )
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::site[domain=\"example.com\"]"));
        assert!(source.contains("theme: dark"));
    }

    #[test]
    fn test_serialize_page() {
        let doc = SurfDocBuilder::new()
            .page("/about", None, Some("About Us"), "We build things.")
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::page[route=\"/about\" title=\"About Us\"]"));
        assert!(source.contains("We build things."));
    }

    #[test]
    fn test_serialize_tabs() {
        let doc = SurfDocBuilder::new()
            .tabs(vec![
                TabPanel {
                    label: "Overview".into(),
                    content: "Tab 1 content".into(),
                },
                TabPanel {
                    label: "Details".into(),
                    content: "Tab 2 content".into(),
                },
            ])
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::tabs"));
        assert!(source.contains("## Overview\nTab 1 content"));
        assert!(source.contains("## Details\nTab 2 content"));
    }

    #[test]
    fn test_serialize_columns() {
        let doc = SurfDocBuilder::new()
            .columns(vec![
                ColumnContent {
                    content: "Left column".into(),
                },
                ColumnContent {
                    content: "Right column".into(),
                },
            ])
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::columns"));
        assert!(source.contains(":::column\nLeft column\n:::"));
        assert!(source.contains(":::column\nRight column\n:::"));
    }

    #[test]
    fn test_serialize_testimonial() {
        let doc = SurfDocBuilder::new()
            .testimonial("Amazing tool!", Some("Jane"), Some("CTO"), Some("Acme"))
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::testimonial[author=\"Jane\" role=\"CTO\" company=\"Acme\"]"));
        assert!(source.contains("Amazing tool!"));
    }

    #[test]
    fn test_serialize_style() {
        let doc = SurfDocBuilder::new()
            .style(vec![
                StyleProperty {
                    key: "accent".into(),
                    value: "#6366f1".into(),
                },
            ])
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::style\naccent: #6366f1\n::"));
    }

    #[test]
    fn test_serialize_faq() {
        let doc = SurfDocBuilder::new()
            .faq(vec![FaqItem {
                question: "Is it free?".into(),
                answer: "Yes.".into(),
            }])
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::faq"));
        assert!(source.contains("### Is it free?\nYes."));
    }

    #[test]
    fn test_serialize_pricing_table() {
        let doc = SurfDocBuilder::new()
            .pricing_table(
                vec!["Plan".into(), "Price".into()],
                vec![vec!["Free".into(), "$0".into()]],
            )
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::pricing-table"));
        assert!(source.contains("| Plan | Price |"));
        assert!(source.contains("| Free | $0 |"));
    }

    #[test]
    fn test_serialize_nav() {
        let doc = SurfDocBuilder::new()
            .nav(
                vec![NavItem {
                    label: "Home".into(),
                    href: "/".into(),
                    icon: None,
                }],
                None,
            )
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::nav"));
        assert!(source.contains("- [Home](/)"));
    }

    #[test]
    fn test_serialize_embed() {
        let doc = SurfDocBuilder::new()
            .embed("https://youtube.com/watch?v=123", Some(EmbedType::Video), Some("My Video"))
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::embed[src=\"https://youtube.com/watch?v=123\" type=video title=\"My Video\"]"));
    }

    #[test]
    fn test_serialize_gallery() {
        let doc = SurfDocBuilder::new()
            .gallery(
                vec![GalleryItem {
                    src: "photo.jpg".into(),
                    caption: Some("A photo".into()),
                    alt: Some("Photo".into()),
                    category: None,
                }],
                Some(3),
            )
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::gallery"));
        assert!(source.contains("![Photo](photo.jpg) A photo"));
    }

    #[test]
    fn test_serialize_footer() {
        let doc = SurfDocBuilder::new()
            .footer(
                vec![FooterSection {
                    heading: "Links".into(),
                    links: vec![NavItem {
                        label: "Home".into(),
                        href: "/".into(),
                        icon: None,
                    }],
                }],
                Some("(c) 2026 CloudSurf"),
                vec![SocialLink {
                    platform: "twitter".into(),
                    href: "https://twitter.com/cloudsurf".into(),
                }],
            )
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("::footer"));
        assert!(source.contains("## Links"));
        assert!(source.contains("- [Home](/)"));
        assert!(source.contains("@twitter https://twitter.com/cloudsurf"));
        assert!(source.contains("(c) 2026 CloudSurf"));
    }

    // === Round-trip tests ===============================================

    #[test]
    fn test_roundtrip_basic() {
        let original = SurfDocBuilder::new()
            .heading(1, "Hello")
            .callout(CalloutType::Info, "Important note")
            .markdown("Some text here.")
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.doc.blocks.len(), original.blocks.len());

        // First block: markdown heading
        match &parsed.doc.blocks[0] {
            Block::Markdown { content, .. } => assert!(content.contains("# Hello")),
            _ => panic!("Expected Markdown block, got {:?}", parsed.doc.blocks[0]),
        }

        // Second block: callout
        match &parsed.doc.blocks[1] {
            Block::Callout {
                callout_type,
                content,
                ..
            } => {
                assert_eq!(*callout_type, CalloutType::Info);
                assert_eq!(content, "Important note");
            }
            _ => panic!("Expected Callout block, got {:?}", parsed.doc.blocks[1]),
        }

        // Third block: markdown
        match &parsed.doc.blocks[2] {
            Block::Markdown { content, .. } => assert!(content.contains("Some text here.")),
            _ => panic!("Expected Markdown block, got {:?}", parsed.doc.blocks[2]),
        }
    }

    #[test]
    fn test_roundtrip_front_matter() {
        let original = SurfDocBuilder::new()
            .title("Round Trip Test")
            .doc_type(DocType::Guide)
            .status(DocStatus::Active)
            .author("Brady")
            .tags(vec!["test".into(), "roundtrip".into()])
            .description("Testing round-trip")
            .markdown("Body text.")
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}",
            parsed.diagnostics
        );

        let fm = parsed.doc.front_matter.as_ref().unwrap();
        assert_eq!(fm.title.as_deref(), Some("Round Trip Test"));
        assert_eq!(fm.doc_type, Some(DocType::Guide));
        assert_eq!(fm.status, Some(DocStatus::Active));
        assert_eq!(fm.author.as_deref(), Some("Brady"));
        assert_eq!(
            fm.tags.as_ref().unwrap(),
            &vec!["test".to_string(), "roundtrip".to_string()]
        );
        assert_eq!(fm.description.as_deref(), Some("Testing round-trip"));
    }

    #[test]
    fn test_roundtrip_all_blocks() {
        let original = SurfDocBuilder::new()
            .title("All Blocks")
            .heading(1, "Introduction")
            .callout(CalloutType::Warning, "Watch out!")
            .code("fn main() {}", Some("rust"))
            .tasks(vec![
                TaskItem {
                    done: true,
                    text: "Implement parser".into(),
                    assignee: None,
                },
                TaskItem {
                    done: false,
                    text: "Write tests".into(),
                    assignee: Some("brady".into()),
                },
            ])
            .decision(DecisionStatus::Accepted, "We chose Rust.")
            .metric_with_trend("MRR", "$2K", Trend::Up, Some("USD"))
            .summary("A summary of the document.")
            .figure_with_caption("diagram.png", "Architecture", Some("Diagram"))
            .quote_attributed("The future is here.", "Alan Kay")
            .cta("Sign Up", "/signup", true)
            .hero_image("hero.png", Some("Hero shot"))
            .testimonial("Great tool!", Some("Jane"), Some("CTO"), None)
            .data_table(
                vec!["Name".into(), "Value".into()],
                vec![vec!["A".into(), "1".into()]],
            )
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}\nSource:\n{}",
            parsed.diagnostics,
            source
        );

        // Verify each block type survived the round-trip
        let blocks = &parsed.doc.blocks;
        assert_eq!(
            blocks.len(),
            original.blocks.len(),
            "Block count mismatch.\nSource:\n{}\nParsed blocks: {:?}",
            source,
            blocks
        );

        // Verify specific block types
        assert!(matches!(&blocks[0], Block::Markdown { .. }), "Block 0: Markdown heading");
        assert!(matches!(&blocks[1], Block::Callout { .. }), "Block 1: Callout");
        assert!(matches!(&blocks[2], Block::Code { .. }), "Block 2: Code");
        assert!(matches!(&blocks[3], Block::Tasks { .. }), "Block 3: Tasks");
        assert!(matches!(&blocks[4], Block::Decision { .. }), "Block 4: Decision");
        assert!(matches!(&blocks[5], Block::Metric { .. }), "Block 5: Metric");
        assert!(matches!(&blocks[6], Block::Summary { .. }), "Block 6: Summary");
        assert!(matches!(&blocks[7], Block::Figure { .. }), "Block 7: Figure");
        assert!(matches!(&blocks[8], Block::Quote { .. }), "Block 8: Quote");
        assert!(matches!(&blocks[9], Block::Cta { .. }), "Block 9: Cta");
        assert!(matches!(&blocks[10], Block::HeroImage { .. }), "Block 10: HeroImage");
        assert!(matches!(&blocks[11], Block::Testimonial { .. }), "Block 11: Testimonial");
        assert!(matches!(&blocks[12], Block::Data { .. }), "Block 12: Data");

        // Verify specific field values survived
        match &blocks[1] {
            Block::Callout {
                callout_type,
                content,
                ..
            } => {
                assert_eq!(*callout_type, CalloutType::Warning);
                assert_eq!(content, "Watch out!");
            }
            _ => unreachable!(),
        }

        match &blocks[2] {
            Block::Code { lang, content, .. } => {
                assert_eq!(lang.as_deref(), Some("rust"));
                assert_eq!(content, "fn main() {}");
            }
            _ => unreachable!(),
        }

        match &blocks[3] {
            Block::Tasks { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(items[0].done);
                assert_eq!(items[0].text, "Implement parser");
                assert!(!items[1].done);
                assert_eq!(items[1].text, "Write tests");
                assert_eq!(items[1].assignee.as_deref(), Some("brady"));
            }
            _ => unreachable!(),
        }

        match &blocks[5] {
            Block::Metric {
                label,
                value,
                trend,
                unit,
                ..
            } => {
                assert_eq!(label, "MRR");
                assert_eq!(value, "$2K");
                assert_eq!(*trend, Some(Trend::Up));
                assert_eq!(unit.as_deref(), Some("USD"));
            }
            _ => unreachable!(),
        }

        match &blocks[8] {
            Block::Quote {
                content,
                attribution,
                ..
            } => {
                assert_eq!(content, "The future is here.");
                assert_eq!(attribution.as_deref(), Some("Alan Kay"));
            }
            _ => unreachable!(),
        }

        match &blocks[10] {
            Block::HeroImage { src, alt, .. } => {
                assert_eq!(src, "hero.png");
                assert_eq!(alt.as_deref(), Some("Hero shot"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_roundtrip_style_faq_pricing() {
        let original = SurfDocBuilder::new()
            .style(vec![
                StyleProperty {
                    key: "accent".into(),
                    value: "#6366f1".into(),
                },
                StyleProperty {
                    key: "theme".into(),
                    value: "dark".into(),
                },
            ])
            .faq(vec![
                FaqItem {
                    question: "Is it free?".into(),
                    answer: "Yes.".into(),
                },
                FaqItem {
                    question: "Can I export?".into(),
                    answer: "PDF and HTML.".into(),
                },
            ])
            .pricing_table(
                vec!["".into(), "Free".into(), "Pro".into()],
                vec![vec!["Price".into(), "$0".into(), "$9/mo".into()]],
            )
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}\nSource:\n{}",
            parsed.diagnostics,
            source
        );

        assert_eq!(parsed.doc.blocks.len(), 3);

        match &parsed.doc.blocks[0] {
            Block::Style { properties, .. } => {
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[0].key, "accent");
                assert_eq!(properties[0].value, "#6366f1");
            }
            _ => panic!("Expected Style block"),
        }

        match &parsed.doc.blocks[1] {
            Block::Faq { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].question, "Is it free?");
                assert_eq!(items[0].answer, "Yes.");
            }
            _ => panic!("Expected Faq block"),
        }

        match &parsed.doc.blocks[2] {
            Block::PricingTable { headers, rows, .. } => {
                assert_eq!(headers.len(), 3);
                assert_eq!(rows.len(), 1);
            }
            _ => panic!("Expected PricingTable block"),
        }
    }

    #[test]
    fn test_roundtrip_site_and_page() {
        let original = SurfDocBuilder::new()
            .site(
                Some("example.com"),
                vec![
                    StyleProperty {
                        key: "name".into(),
                        value: "Test Site".into(),
                    },
                    StyleProperty {
                        key: "theme".into(),
                        value: "dark".into(),
                    },
                ],
            )
            .page("/", Some("hero"), Some("Home"), "Welcome to our site.")
            .page("/about", None, Some("About"), "About us content.")
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}\nSource:\n{}",
            parsed.diagnostics,
            source
        );

        assert_eq!(parsed.doc.blocks.len(), 3);

        match &parsed.doc.blocks[0] {
            Block::Site {
                domain,
                properties,
                ..
            } => {
                assert_eq!(domain.as_deref(), Some("example.com"));
                assert_eq!(properties.len(), 2);
            }
            _ => panic!("Expected Site block, got {:?}", parsed.doc.blocks[0]),
        }

        match &parsed.doc.blocks[1] {
            Block::Page {
                route,
                layout,
                title,
                ..
            } => {
                assert_eq!(route, "/");
                assert_eq!(layout.as_deref(), Some("hero"));
                assert_eq!(title.as_deref(), Some("Home"));
            }
            _ => panic!("Expected Page block, got {:?}", parsed.doc.blocks[1]),
        }

        match &parsed.doc.blocks[2] {
            Block::Page {
                route,
                title,
                ..
            } => {
                assert_eq!(route, "/about");
                assert_eq!(title.as_deref(), Some("About"));
            }
            _ => panic!("Expected Page block, got {:?}", parsed.doc.blocks[2]),
        }
    }

    #[test]
    fn test_roundtrip_tabs_and_columns() {
        let original = SurfDocBuilder::new()
            .tabs(vec![
                TabPanel {
                    label: "Overview".into(),
                    content: "Overview content here.".into(),
                },
                TabPanel {
                    label: "Details".into(),
                    content: "Details content here.".into(),
                },
            ])
            .columns(vec![
                ColumnContent {
                    content: "Left column content".into(),
                },
                ColumnContent {
                    content: "Right column content".into(),
                },
            ])
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}\nSource:\n{}",
            parsed.diagnostics,
            source
        );

        assert_eq!(parsed.doc.blocks.len(), 2);

        match &parsed.doc.blocks[0] {
            Block::Tabs { tabs, .. } => {
                assert_eq!(tabs.len(), 2);
                assert_eq!(tabs[0].label, "Overview");
                assert_eq!(tabs[0].content, "Overview content here.");
                assert_eq!(tabs[1].label, "Details");
                assert_eq!(tabs[1].content, "Details content here.");
            }
            _ => panic!("Expected Tabs block"),
        }

        match &parsed.doc.blocks[1] {
            Block::Columns { columns, .. } => {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].content, "Left column content");
                assert_eq!(columns[1].content, "Right column content");
            }
            _ => panic!("Expected Columns block"),
        }
    }

    #[test]
    fn test_roundtrip_nav_embed_gallery_footer() {
        let original = SurfDocBuilder::new()
            .nav(
                vec![
                    NavItem {
                        label: "Home".into(),
                        href: "/".into(),
                        icon: None,
                    },
                    NavItem {
                        label: "About".into(),
                        href: "/about".into(),
                        icon: None,
                    },
                ],
                None,
            )
            .embed(
                "https://example.com/video",
                Some(EmbedType::Video),
                Some("Demo Video"),
            )
            .gallery(
                vec![GalleryItem {
                    src: "photo.jpg".into(),
                    caption: Some("A photo".into()),
                    alt: Some("Photo".into()),
                    category: None,
                }],
                Some(3),
            )
            .footer(
                vec![FooterSection {
                    heading: "Links".into(),
                    links: vec![NavItem {
                        label: "Home".into(),
                        href: "/".into(),
                        icon: None,
                    }],
                }],
                Some("(c) 2026 Test"),
                vec![SocialLink {
                    platform: "twitter".into(),
                    href: "https://twitter.com/test".into(),
                }],
            )
            .build();

        let source = to_surf_source(&original);
        let parsed = parse::parse(&source);

        assert!(
            parsed.diagnostics.is_empty(),
            "Parse diagnostics: {:?}\nSource:\n{}",
            parsed.diagnostics,
            source
        );

        assert_eq!(parsed.doc.blocks.len(), 4);
        assert!(matches!(&parsed.doc.blocks[0], Block::Nav { .. }));
        assert!(matches!(&parsed.doc.blocks[1], Block::Embed { .. }));
        assert!(matches!(&parsed.doc.blocks[2], Block::Gallery { .. }));
        assert!(matches!(&parsed.doc.blocks[3], Block::Footer { .. }));

        // Verify nav items survived
        match &parsed.doc.blocks[0] {
            Block::Nav { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].label, "Home");
                assert_eq!(items[1].label, "About");
            }
            _ => unreachable!(),
        }

        // Verify footer details survived
        match &parsed.doc.blocks[3] {
            Block::Footer {
                sections,
                copyright,
                social,
                ..
            } => {
                assert_eq!(sections.len(), 1);
                assert_eq!(sections[0].heading, "Links");
                assert_eq!(copyright.as_deref(), Some("(c) 2026 Test"));
                assert_eq!(social.len(), 1);
                assert_eq!(social[0].platform, "twitter");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_existing_fixture_roundtrip() {
        let fixture = include_str!("../tests/fixtures/single.surf");
        let first_parse = parse::parse(fixture);
        assert!(
            first_parse.diagnostics.is_empty(),
            "First parse diagnostics: {:?}",
            first_parse.diagnostics
        );

        let serialized = to_surf_source(&first_parse.doc);
        let second_parse = parse::parse(&serialized);
        assert!(
            second_parse.diagnostics.is_empty(),
            "Second parse diagnostics: {:?}\nSerialized:\n{}",
            second_parse.diagnostics,
            serialized
        );

        // Verify same number of blocks
        assert_eq!(
            first_parse.doc.blocks.len(),
            second_parse.doc.blocks.len(),
            "Block count mismatch in fixture round-trip"
        );

        // Verify front matter survived
        let fm1 = first_parse.doc.front_matter.as_ref().unwrap();
        let fm2 = second_parse.doc.front_matter.as_ref().unwrap();
        assert_eq!(fm1.title, fm2.title);
        assert_eq!(fm1.doc_type, fm2.doc_type);
        assert_eq!(fm1.status, fm2.status);

        // Verify block types match
        for (i, (b1, b2)) in first_parse
            .doc
            .blocks
            .iter()
            .zip(second_parse.doc.blocks.iter())
            .enumerate()
        {
            assert_eq!(
                std::mem::discriminant(b1),
                std::mem::discriminant(b2),
                "Block {} type mismatch: {:?} vs {:?}",
                i,
                b1,
                b2
            );
        }
    }

    #[test]
    fn test_double_roundtrip() {
        // Build -> serialize -> parse -> serialize -> parse -> compare
        let doc = SurfDocBuilder::new()
            .title("Double Trip")
            .heading(1, "Hello")
            .callout(CalloutType::Info, "Note")
            .code("let x = 1;", Some("rust"))
            .metric("MRR", "$5K")
            .build();

        let source1 = to_surf_source(&doc);
        let parsed1 = parse::parse(&source1);
        let source2 = to_surf_source(&parsed1.doc);
        let parsed2 = parse::parse(&source2);

        assert_eq!(
            parsed1.doc.blocks.len(),
            parsed2.doc.blocks.len(),
            "Block count changed between round-trips"
        );

        // The serialized forms should be identical after the first round-trip
        assert_eq!(
            source2,
            to_surf_source(&parsed2.doc),
            "Third serialization differs from second"
        );
    }
}

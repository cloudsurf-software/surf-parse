//! Programmatic SurfDoc builder and `.surf` source serializer.
//!
//! The [`SurfDocBuilder`] provides a fluent API for constructing `SurfDoc`
//! documents without writing raw `.surf` text.  The [`to_surf_source`] function
//! serializes any `SurfDoc` back to valid `.surf` format that round-trips
//! through [`crate::parse`].

use crate::citation::{Author, Reference, RefType};
use crate::types::{
    Block, CalloutType, ChartType, ColumnContent, DataFormat, DecisionStatus, EmbedType, FaqItem,
    FeatureCard, Format, FooterSection, FormField, FormFieldType, FrontMatter, GalleryItem, HeroButton,
    HttpMethod, ListDisplay, NavItem, RowState, SocialLink, Span, StatItem, StepItem,
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
            groups: Vec::new(),
            brand: None,
            brand_reg: false,
            cta: None,
            drawer: false,
            minimal: false,
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
            action: None,
            method: None,
            honeypot: false,
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
            brand: None,
            brand_reg: false,
            brand_logo: None,
            tagline: None,
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

    /// Add a hero section block.
    pub fn hero(
        mut self,
        headline: Option<&str>,
        subtitle: Option<&str>,
        badge: Option<&str>,
        buttons: Vec<HeroButton>,
    ) -> Self {
        self.blocks.push(Block::Hero {
            headline: headline.map(|s| s.to_string()),
            subtitle: subtitle.map(|s| s.to_string()),
            badge: badge.map(|s| s.to_string()),
            align: "center".to_string(),
            image: None,
            image_alt: None,
            layout: None,
            transparent: false,
            buttons,
            content: String::new(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a features card grid block.
    pub fn features(mut self, cards: Vec<FeatureCard>, cols: Option<u32>) -> Self {
        self.blocks.push(Block::Features {
            cards,
            cols,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a steps block.
    pub fn steps(mut self, steps: Vec<StepItem>) -> Self {
        self.blocks.push(Block::Steps {
            steps,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a stats block.
    pub fn stats(mut self, items: Vec<StatItem>) -> Self {
        self.blocks.push(Block::Stats {
            items,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a comparison table block.
    pub fn comparison(
        mut self,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        highlight: Option<&str>,
    ) -> Self {
        self.blocks.push(Block::Comparison {
            headers,
            rows,
            highlight: highlight.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a logo block.
    pub fn logo(mut self, src: &str, alt: Option<&str>, size: Option<u32>) -> Self {
        self.blocks.push(Block::Logo {
            src: src.to_string(),
            alt: alt.map(|s| s.to_string()),
            size,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a table-of-contents block.
    pub fn toc(mut self, depth: u32) -> Self {
        self.blocks.push(Block::Toc {
            depth,
            entries: Vec::new(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a before/after comparison block.
    pub fn before_after(
        mut self,
        before_items: Vec<crate::types::BeforeAfterItem>,
        after_items: Vec<crate::types::BeforeAfterItem>,
        transition: Option<&str>,
    ) -> Self {
        self.blocks.push(Block::BeforeAfter {
            before_items,
            after_items,
            transition: transition.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a pipeline flow block.
    pub fn pipeline(mut self, steps: Vec<crate::types::PipelineStep>) -> Self {
        self.blocks.push(Block::Pipeline {
            steps,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a section container block.
    pub fn section(
        mut self,
        bg: Option<&str>,
        headline: Option<&str>,
        subtitle: Option<&str>,
        content: &str,
    ) -> Self {
        self.blocks.push(Block::Section {
            bg: bg.map(|s| s.to_string()),
            headline: headline.map(|s| s.to_string()),
            subtitle: subtitle.map(|s| s.to_string()),
            content: content.to_string(),
            children: Vec::new(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a product card block.
    #[allow(clippy::too_many_arguments)]
    pub fn product_card(
        mut self,
        title: &str,
        subtitle: Option<&str>,
        badge: Option<&str>,
        badge_color: Option<&str>,
        body: &str,
        features: Vec<String>,
        cta_label: Option<&str>,
        cta_href: Option<&str>,
    ) -> Self {
        self.blocks.push(Block::ProductCard {
            title: title.to_string(),
            subtitle: subtitle.map(|s| s.to_string()),
            badge: badge.map(|s| s.to_string()),
            badge_color: badge_color.map(|s| s.to_string()),
            body: body.to_string(),
            features,
            cta_label: cta_label.map(|s| s.to_string()),
            cta_href: cta_href.map(|s| s.to_string()),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add an app manifest container block.
    pub fn app(mut self, name: &str, binary: Option<&str>, region: Option<&str>, port: Option<u32>, children: Vec<Block>) -> Self {
        self.blocks.push(Block::App {
            name: name.to_string(),
            binary: binary.map(|s| s.to_string()),
            region: region.map(|s| s.to_string()),
            port,
            platform: None,
            auth: None,
            content: String::new(),
            children,
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add a deploy block.
    pub fn deploy(mut self, env: &str, app: Option<&str>, machines: Option<u32>, memory: Option<u32>) -> Self {
        self.blocks.push(Block::Deploy {
            env: Some(env.to_string()),
            app: app.map(|s| s.to_string()),
            machines,
            memory,
            auto_stop: None,
            min_machines: None,
            strategy: None,
            properties: Vec::new(),
            span: Span::SYNTHETIC,
        });
        self
    }

    /// Add an infrastructure database block.
    pub fn infra_database(mut self, name: Option<&str>, shared_auth: bool) -> Self {
        self.blocks.push(Block::InfraDatabase {
            name: name.map(|s| s.to_string()),
            shared_auth,
            volume_gb: None,
            properties: Vec::new(),
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
        let tag_strs: Vec<String> = tags.iter().cloned().collect();
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
    if let Some(related) = &fm.related
        && !related.is_empty() {
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
        DocType::App => "app",
        DocType::Manifest => "manifest",
        DocType::Website => "website",
        DocType::Web => "web",
        DocType::Deck => "deck",
        DocType::Slides => "slides",
        DocType::Presentation => "presentation",
        DocType::Paper => "paper",
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

        Block::Diagram {
            diagram_type,
            title,
            content,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if !diagram_type.is_empty() {
                attr_parts.push(format!("type={diagram_type}"));
            }
            if let Some(t) = title {
                attr_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            if content.is_empty() {
                format!("::diagram{attrs}\n::")
            } else {
                format!("::diagram{attrs}\n{content}\n::")
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

        Block::Cite { reference, .. } => cite_to_surf(reference),

        Block::Bibliography { style, .. } => match style {
            Some(s) => format!("::bibliography[style={}]\n::", citation_format_slug(*s)),
            None => "::bibliography\n::".to_string(),
        },

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

        Block::Nav { items, logo, groups, brand, brand_reg, cta, drawer, .. } => {
            let mut attr_parts = Vec::new();
            if *drawer {
                attr_parts.push("drawer".to_string());
            }
            if let Some(b) = brand {
                attr_parts.push(format!("brand=\"{}\"", escape_attr(b)));
            }
            if *brand_reg {
                attr_parts.push("reg".to_string());
            }
            if let Some(l) = logo {
                attr_parts.push(format!("logo=\"{}\"", escape_attr(l)));
            }
            if let Some(c) = cta {
                attr_parts.push(format!("cta-label=\"{}\"", escape_attr(&c.label)));
                attr_parts.push(format!("cta-href=\"{}\"", escape_attr(&c.href)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for item in items {
                content_lines.push(serialize_nav_item(item));
            }
            for g in groups {
                if let Some(label) = &g.label {
                    content_lines.push(format!("## {}", label));
                }
                for item in &g.items {
                    content_lines.push(serialize_nav_item(item));
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

        Block::Deck { properties, .. } => {
            let mut content_lines = Vec::new();
            for p in properties {
                content_lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::deck\n::".to_string()
            } else {
                format!("::deck\n{content}\n::")
            }
        }

        Block::Slide {
            layout,
            kicker,
            notes,
            content,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(l) = layout {
                attr_parts.push(format!("layout={}", l.css_class()));
            }
            if let Some(k) = kicker {
                attr_parts.push(format!("kicker=\"{}\"", escape_attr(k)));
            }
            if let Some(n) = notes {
                attr_parts.push(format!("notes=\"{}\"", escape_attr(n)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            if content.is_empty() {
                format!("::slide{attrs}\n::")
            } else {
                format!("::slide{attrs}\n{content}\n::")
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
            action,
            method,
            honeypot,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(s) = submit_label {
                attr_parts.push(format!("submit=\"{}\"", escape_attr(s)));
            }
            if let Some(a) = action {
                attr_parts.push(format!("action=\"{}\"", escape_attr(a)));
            }
            if let Some(m) = method {
                attr_parts.push(format!("method=\"{}\"", escape_attr(m)));
            }
            if *honeypot {
                attr_parts.push("honeypot".to_string());
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
            brand,
            brand_reg,
            brand_logo,
            tagline,
            ..
        } => {
            let mut attr_parts = Vec::new();
            if let Some(b) = brand {
                attr_parts.push(format!("brand=\"{}\"", escape_attr(b)));
            }
            if *brand_reg {
                attr_parts.push("reg".to_string());
            }
            if let Some(l) = brand_logo {
                attr_parts.push(format!("logo=\"{}\"", escape_attr(l)));
            }
            if let Some(t) = tagline {
                attr_parts.push(format!("tagline=\"{}\"", escape_attr(t)));
            }
            let attrs = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
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
                format!("::footer{attrs}\n::")
            } else {
                format!("::footer{attrs}\n{content}\n::")
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

        Block::Divider { label, .. } => match label {
            Some(l) => format!("::divider[label=\"{}\"]", escape_attr(l)),
            None => "::divider".to_string(),
        },

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
            ..
        } => {
            let mut attrs_parts = Vec::new();
            if align != "center" {
                attrs_parts.push(format!("align=\"{}\"", escape_attr(align)));
            }
            if let Some(b) = badge {
                attrs_parts.push(format!("badge=\"{}\"", escape_attr(b)));
            }
            if let Some(img) = image {
                attrs_parts.push(format!("image=\"{}\"", escape_attr(img)));
            }
            if let Some(a) = image_alt {
                attrs_parts.push(format!("image-alt=\"{}\"", escape_attr(a)));
            }
            if let Some(l) = layout {
                attrs_parts.push(format!("layout=\"{}\"", escape_attr(l)));
            }
            if *transparent {
                attrs_parts.push("transparent".to_string());
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            if let Some(h) = headline {
                content_lines.push(format!("# {h}"));
                content_lines.push(String::new());
            }
            if let Some(s) = subtitle {
                content_lines.push(s.clone());
                content_lines.push(String::new());
            }
            for btn in buttons {
                let mut flags: Vec<&str> = Vec::new();
                if btn.primary { flags.push("primary"); }
                if btn.external { flags.push("external"); }
                let suffix = if flags.is_empty() { String::new() } else { format!("{{{}}}", flags.join(" ")) };
                content_lines.push(format!("[{}]({}){}", btn.label, btn.href, suffix));
            }
            format!("::hero{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Banner {
            headline,
            subtitle,
            buttons,
            id,
            ..
        } => {
            let attrs_str = id
                .as_ref()
                .map(|i| format!("[id=\"{}\"]", escape_attr(i)))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            if let Some(h) = headline {
                content_lines.push(format!("# {h}"));
                content_lines.push(String::new());
            }
            if let Some(s) = subtitle {
                content_lines.push(s.clone());
                content_lines.push(String::new());
            }
            for btn in buttons {
                let mut flags: Vec<&str> = Vec::new();
                if btn.primary { flags.push("primary"); }
                if btn.external { flags.push("external"); }
                let suffix = if flags.is_empty() { String::new() } else { format!("{{{}}}", flags.join(" ")) };
                content_lines.push(format!("[{}]({}){}", btn.label, btn.href, suffix));
            }
            format!("::banner{attrs_str}\n{}\n::", content_lines.join("\n").trim_end())
        }

        Block::ProductGrid { groups, tiles, .. } => {
            let mut content_lines = Vec::new();
            for group in groups {
                let brace = group.cols.map(|c| format!(" {{cols={c}}}")).unwrap_or_default();
                if let Some(label) = &group.label {
                    content_lines.push(format!("### {label}{brace}"));
                } else if !brace.is_empty() {
                    content_lines.push(format!("###{brace}"));
                }
                for item in &group.items {
                    let emblem = item.emblem.as_deref().unwrap_or("");
                    let tagline = item.tagline.as_deref().unwrap_or("");
                    // The bg field is emitted only when set, so bg-less docs
                    // round-trip byte-identical to the 4-field form.
                    let cta1 = match (&item.cta1_label, &item.cta1_href) {
                        (Some(l), Some(h)) => Some(format!("[{l}]({h})")),
                        _ => None,
                    };
                    let cta2 = match (&item.cta2_label, &item.cta2_href) {
                        (Some(l), Some(h)) => Some(format!("[{l}]({h})")),
                        _ => None,
                    };
                    let mut tail = String::new();
                    if item.bg.is_some() || cta1.is_some() || cta2.is_some() {
                        tail.push_str(&format!(" | {}", item.bg.as_deref().unwrap_or("")));
                    }
                    if let Some(c) = cta1 {
                        tail.push_str(&format!(" | {c}"));
                    }
                    if let Some(c) = cta2 {
                        tail.push_str(&format!(" | {c}"));
                    }
                    content_lines.push(format!(
                        "- {} | {} | {} | {}{}",
                        item.name, emblem, item.href, tagline, tail
                    ));
                }
            }
            let attrs_str = if *tiles { "[tiles]" } else { "" };
            format!("::product-grid{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::PostGrid { title, subtitle, items, .. } => {
            let mut attr_parts = Vec::new();
            if let Some(t) = title {
                attr_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            if let Some(s) = subtitle {
                attr_parts.push(format!("subtitle=\"{}\"", escape_attr(s)));
            }
            let attrs_str = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for item in items {
                let ext = if item.external { "{external}" } else { "" };
                let meta = item.meta.as_deref().unwrap_or("");
                let excerpt = item.excerpt.as_deref().unwrap_or("");
                let image = item.image.as_deref().unwrap_or("");
                content_lines.push(format!(
                    "- [{}]({}){} | {} | {} | {}",
                    item.title, item.href, ext, meta, excerpt, image
                ));
            }
            format!("::post-grid{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Gate { title, subtitle, action, field_label, submit_label, error, .. } => {
            let mut attr_parts = Vec::new();
            if let Some(t) = title {
                attr_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            if let Some(s) = subtitle {
                attr_parts.push(format!("subtitle=\"{}\"", escape_attr(s)));
            }
            if !action.is_empty() {
                attr_parts.push(format!("action=\"{}\"", escape_attr(action)));
            }
            if let Some(f) = field_label {
                attr_parts.push(format!("field=\"{}\"", escape_attr(f)));
            }
            if let Some(s) = submit_label {
                attr_parts.push(format!("submit=\"{}\"", escape_attr(s)));
            }
            if let Some(e) = error {
                attr_parts.push(format!("error=\"{}\"", escape_attr(e)));
            }
            let attrs_str = if attr_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attr_parts.join(" "))
            };
            format!("::gate{attrs_str}\n::")
        }

        Block::Features { cards, cols, .. } => {
            let attrs_str = cols
                .map(|c| format!("[cols={}]", c))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            for card in cards {
                let icon_str = card
                    .icon
                    .as_ref()
                    .map(|i| format!(" {{icon={i}}}"))
                    .unwrap_or_default();
                content_lines.push(format!("### {}{}", card.title, icon_str));
                content_lines.push(String::new());
                if !card.body.is_empty() {
                    content_lines.push(card.body.clone());
                    content_lines.push(String::new());
                }
                if let (Some(label), Some(href)) = (&card.link_label, &card.link_href) {
                    content_lines.push(format!("[{label}]({href})"));
                    content_lines.push(String::new());
                }
            }
            format!("::features{attrs_str}\n{}\n::", content_lines.join("\n").trim())
        }

        Block::Steps { steps, .. } => {
            let mut content_lines = Vec::new();
            for step in steps {
                let time_str = step
                    .time
                    .as_ref()
                    .map(|t| format!(" {{time=\"{t}\"}}"))
                    .unwrap_or_default();
                content_lines.push(format!("### {}{}", step.title, time_str));
                content_lines.push(String::new());
                if !step.body.is_empty() {
                    content_lines.push(step.body.clone());
                    content_lines.push(String::new());
                }
            }
            format!("::steps\n{}\n::", content_lines.join("\n").trim())
        }

        Block::Stats { items, .. } => {
            let mut content_lines = Vec::new();
            for item in items {
                let color_str = item
                    .color
                    .as_ref()
                    .map(|c| format!(" color=\"{c}\""))
                    .unwrap_or_default();
                content_lines.push(format!(
                    "- {} {{label=\"{}\"{}}}",
                    item.value, escape_attr(&item.label), color_str
                ));
            }
            format!("::stats\n{}\n::", content_lines.join("\n"))
        }

        Block::Comparison {
            headers,
            rows,
            highlight,
            ..
        } => {
            let attrs_str = highlight
                .as_ref()
                .map(|h| format!("[highlight=\"{}\"]", escape_attr(h)))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            content_lines.push(format!("| {} |", headers.join(" | ")));
            content_lines.push(format!(
                "| {} |",
                headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
            ));
            for row in rows {
                content_lines.push(format!("| {} |", row.join(" | ")));
            }
            format!("::comparison{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Logo { src, alt, size, .. } => {
            let mut attrs_parts = vec![format!("src=\"{}\"", escape_attr(src))];
            if let Some(a) = alt {
                attrs_parts.push(format!("alt=\"{}\"", escape_attr(a)));
            }
            if let Some(s) = size {
                attrs_parts.push(format!("size={}", s));
            }
            format!("::logo[{}]", attrs_parts.join(" "))
        }

        Block::Toc { depth, .. } => {
            if *depth == 3 {
                "::toc".to_string()
            } else {
                format!("::toc[depth={}]", depth)
            }
        }

        Block::BeforeAfter {
            before_items,
            after_items,
            transition,
            ..
        } => {
            let attrs_str = transition
                .as_ref()
                .map(|t| format!("[transition=\"{}\"]", escape_attr(t)))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            content_lines.push("### Before".to_string());
            for item in before_items {
                content_lines.push(format!("- {} | {}", item.label, item.detail));
            }
            content_lines.push(String::new());
            content_lines.push("### After".to_string());
            for item in after_items {
                content_lines.push(format!("- {} | {}", item.label, item.detail));
            }
            format!("::before-after{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Pipeline { steps, .. } => {
            let content_lines: Vec<String> = steps
                .iter()
                .map(|s| match &s.description {
                    Some(d) => format!("{} | {}", s.label, d),
                    None => s.label.clone(),
                })
                .collect();
            format!("::pipeline\n{}\n::", content_lines.join("\n"))
        }

        Block::Section {
            bg,
            headline,
            subtitle,
            content,
            ..
        } => {
            let attrs_str = bg
                .as_ref()
                .map(|b| format!("[bg=\"{}\"]", escape_attr(b)))
                .unwrap_or_default();
            if content.is_empty() {
                let mut inner = Vec::new();
                if let Some(h) = headline {
                    inner.push(format!("## {h}"));
                }
                if let Some(s) = subtitle {
                    inner.push(s.clone());
                }
                if inner.is_empty() {
                    format!("::section{attrs_str}\n::")
                } else {
                    format!("::section{attrs_str}\n{}\n::", inner.join("\n"))
                }
            } else {
                format!("::section{attrs_str}\n{content}\n::")
            }
        }

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
            let mut attrs_parts = Vec::new();
            if let Some(b) = badge {
                attrs_parts.push(format!("badge=\"{}\"", escape_attr(b)));
            }
            if let Some(bc) = badge_color {
                attrs_parts.push(format!("badge-color=\"{}\"", escape_attr(bc)));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            content_lines.push(format!("## {title}"));
            if let Some(s) = subtitle {
                content_lines.push(s.clone());
            }
            content_lines.push(String::new());
            if !body.is_empty() {
                content_lines.push(body.clone());
                content_lines.push(String::new());
            }
            for f in features {
                content_lines.push(format!("- {f}"));
            }
            if let (Some(label), Some(href)) = (cta_label, cta_href) {
                content_lines.push(String::new());
                content_lines.push(format!("[{label}]({href})"));
            }
            format!("::product-card{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        // ----- App description blocks -----

        Block::List { source, display, item_template, filters, sort, preload, stream, on_select, .. } => {
            let mut attrs_parts = Vec::new();
            attrs_parts.push(format!("source=\"{}\"", escape_attr(source)));
            let display_str = match display {
                ListDisplay::Card => "card",
                ListDisplay::Table => "table",
                ListDisplay::Compact => "compact",
            };
            attrs_parts.push(format!("display={display_str}"));
            if *preload {
                attrs_parts.push("preload".to_string());
            }
            if let Some(s) = stream {
                attrs_parts.push(format!("stream=\"{}\"", escape_attr(s)));
            }
            if let Some(a) = on_select {
                attrs_parts.push(format!("on-select=\"{}\"", escape_attr(a)));
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut content_lines = Vec::new();
            if !item_template.is_empty() {
                content_lines.push(item_template.clone());
                content_lines.push(String::new());
            }
            for f in filters {
                content_lines.push(format!("filter: {}", f.field));
            }
            if let Some(s) = sort {
                let dir = if s.descending { " desc" } else { " asc" };
                content_lines.push(format!("sort: {}{dir}", s.field));
            }
            format!("::list{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Board { source, columns, card_template, preload, .. } => {
            let mut attrs_parts = vec![format!("source=\"{}\"", escape_attr(source))];
            if *preload {
                attrs_parts.push("preload".to_string());
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut content_lines = Vec::new();
            if !columns.is_empty() {
                content_lines.push(format!("columns: {}", columns.join(" | ")));
            }
            if let Some(tmpl) = card_template {
                content_lines.push(format!("card-template: {tmpl}"));
            }
            format!("::board{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Action { method, target, label, fields, confirm, .. } => {
            let method_str = match method {
                HttpMethod::Get => "get",
                HttpMethod::Post => "post",
                HttpMethod::Put => "put",
                HttpMethod::Patch => "patch",
                HttpMethod::Delete => "delete",
            };
            let mut attrs_parts = vec![
                format!("method={method_str}"),
                format!("target=\"{}\"", escape_attr(target)),
                format!("label=\"{}\"", escape_attr(label)),
            ];
            if let Some(c) = confirm {
                attrs_parts.push(format!("confirm=\"{}\"", escape_attr(c)));
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut content_lines = Vec::new();
            for field in fields {
                let type_str = match field.field_type {
                    FormFieldType::Text => "text",
                    FormFieldType::Email => "email",
                    FormFieldType::Tel => "tel",
                    FormFieldType::Date => "date",
                    FormFieldType::Number => "number",
                    FormFieldType::Password => "password",
                    FormFieldType::Select => "select",
                    FormFieldType::Textarea => "textarea",
                };
                let req = if field.required { " *" } else { "" };
                if field.field_type == FormFieldType::Select && !field.options.is_empty() {
                    content_lines.push(format!(
                        "- {} (select: {}){req}",
                        field.label,
                        field.options.join(" | "),
                    ));
                } else {
                    content_lines.push(format!("- {} ({type_str}){req}", field.label));
                }
            }
            format!("::action{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::FilterBar { target_selector, fields, .. } => {
            let attrs_str = format!("[target=\"{}\"]", escape_attr(target_selector));
            let mut content_lines = Vec::new();
            for field in fields {
                content_lines.push(format!(
                    "- {} (select: {})",
                    field.label,
                    field.options.join(" | "),
                ));
            }
            format!("::filter-bar{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Search { source, placeholder, .. } => {
            let mut attrs_parts = vec![format!("source=\"{}\"", escape_attr(source))];
            if let Some(ph) = placeholder {
                attrs_parts.push(format!("placeholder=\"{}\"", escape_attr(ph)));
            }
            format!("::search[{}]\n::", attrs_parts.join(" "))
        }

        Block::Dashboard { source, refresh, .. } => {
            let mut attrs_parts = vec![format!("source=\"{}\"", escape_attr(source))];
            if let Some(r) = refresh {
                attrs_parts.push(format!("refresh={r}"));
            }
            format!("::dashboard[{}]\n::", attrs_parts.join(" "))
        }

        Block::ChatInput { action, placeholder, modes, .. } => {
            let mut attrs_parts = vec![format!("action=\"{}\"", escape_attr(action))];
            if let Some(ph) = placeholder {
                attrs_parts.push(format!("placeholder=\"{}\"", escape_attr(ph)));
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut content_lines = Vec::new();
            if !modes.is_empty() {
                content_lines.push(format!("modes: {}", modes.join(" | ")));
            }
            format!("::chat-input{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Feed { source, stream, .. } => {
            let mut attrs_parts = vec![format!("source=\"{}\"", escape_attr(source))];
            if *stream {
                attrs_parts.push("stream".to_string());
            }
            format!("::feed[{}]\n::", attrs_parts.join(" "))
        }

        Block::Store {
            title,
            currency,
            items,
            ..
        } => {
            let mut attrs_parts = Vec::new();
            if let Some(t) = title {
                attrs_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            if let Some(c) = currency {
                attrs_parts.push(format!("currency=\"{}\"", escape_attr(c)));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            let mut last_cat: Option<String> = None;
            for it in items {
                if it.category != last_cat {
                    if let Some(c) = &it.category {
                        content_lines.push(format!("category: {c}"));
                    }
                    last_cat = it.category.clone();
                }
                let mut parts = vec![it.name.clone(), it.price.clone()];
                if let Some(b) = &it.blurb {
                    parts.push(b.clone());
                }
                if let Some(bd) = &it.badge {
                    // Keep badge in the 4th slot: pad blurb if absent.
                    if it.blurb.is_none() {
                        parts.push(String::new());
                    }
                    parts.push(bd.clone());
                }
                content_lines.push(format!("item: {}", parts.join(" | ")));
            }
            format!("::store{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Booking {
            title,
            service_label,
            services,
            days,
            ..
        } => {
            let mut attrs_parts = Vec::new();
            if let Some(t) = title {
                attrs_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            if let Some(l) = service_label {
                attrs_parts.push(format!("service-label=\"{}\"", escape_attr(l)));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for s in services {
                let mut parts = vec![s.name.clone()];
                if let Some(d) = &s.duration {
                    parts.push(d.clone());
                }
                if let Some(p) = &s.price {
                    parts.push(p.clone());
                }
                content_lines.push(format!("service: {}", parts.join(" | ")));
            }
            for d in days {
                let slots = if d.slots.is_empty() {
                    "full".to_string()
                } else {
                    d.slots.join(", ")
                };
                content_lines.push(format!("day: {} | {}", d.date, slots));
            }
            format!("::booking{attrs_str}\n{}\n::", content_lines.join("\n"))
        }

        Block::Editor { source, lang, preview, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(s) = source {
                attrs_parts.push(format!("source=\"{}\"", escape_attr(s)));
            }
            if let Some(l) = lang {
                attrs_parts.push(format!("lang={l}"));
            }
            if *preview {
                attrs_parts.push("preview".to_string());
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::editor{attrs_str}\n::")
        }

        Block::Chart { chart_type, source, period, title, data, .. } => {
            let type_str = match chart_type {
                ChartType::Line => "line",
                ChartType::Bar => "bar",
                ChartType::Pie => "pie",
                ChartType::Area => "area",
                ChartType::Scatter => "scatter",
                ChartType::Donut => "donut",
                ChartType::StackedBar => "stacked-bar",
                ChartType::Radar => "radar",
            };
            let mut attrs_parts = vec![format!("type={type_str}")];
            if !source.is_empty() {
                attrs_parts.push(format!("source=\"{}\"", escape_attr(source)));
            }
            if let Some(p) = period {
                attrs_parts.push(format!("period={p}"));
            }
            if let Some(t) = title {
                attrs_parts.push(format!("title=\"{}\"", escape_attr(t)));
            }
            // Inline data → emit a pipe table body so the chart round-trips.
            if let Some(d) = data {
                let mut body = String::new();
                body.push_str("| label | ");
                body.push_str(&d.series.iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(" | "));
                body.push_str(" |");
                for (i, cat) in d.categories.iter().enumerate() {
                    body.push_str(&format!("\n{cat} | "));
                    let cells: Vec<String> = d
                        .series
                        .iter()
                        .map(|s| s.values.get(i).map(|v| format!("{v}")).unwrap_or_default())
                        .collect();
                    body.push_str(&cells.join(" | "));
                }
                return format!("::chart[{}]\n{body}\n::", attrs_parts.join(" "));
            }
            format!("::chart[{}]\n::", attrs_parts.join(" "))
        }

        Block::SplitPane { ratio, back_label, back_action, left, right, .. } => {
            let mut attrs_parts = vec![format!("ratio=\"{ratio}\"")];
            if let Some(l) = back_label {
                attrs_parts.push(format!("back-label=\"{}\"", escape_attr(l)));
            }
            if let Some(a) = back_action {
                attrs_parts.push(format!("back-action=\"{}\"", escape_attr(a)));
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut panes: Vec<String> = Vec::new();
            for (side, blocks) in [("left", left), ("right", right)] {
                if !blocks.is_empty() {
                    let inner = serialize_children(blocks);
                    panes.push(format!("::pane[side={side}]\n{inner}\n::"));
                }
            }
            if panes.is_empty() {
                format!("::split-pane{attrs_str}\n::")
            } else {
                format!("::split-pane{attrs_str}\n{}\n::", panes.join("\n\n"))
            }
        }

        // ----- Infrastructure manifest blocks -----

        Block::App { name, binary, region, port, platform, auth, content, .. } => {
            let mut attrs_parts = vec![format!("name=\"{}\"", escape_attr(name))];
            if let Some(b) = binary {
                attrs_parts.push(format!("binary=\"{}\"", escape_attr(b)));
            }
            if let Some(r) = region {
                attrs_parts.push(format!("region=\"{}\"", escape_attr(r)));
            }
            if let Some(p) = port {
                attrs_parts.push(format!("port={p}"));
            }
            if let Some(pl) = platform {
                attrs_parts.push(format!("platform=\"{}\"", escape_attr(pl)));
            }
            if let Some(a) = auth {
                attrs_parts.push(format!("auth=\"{}\"", escape_attr(a)));
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            if content.is_empty() {
                format!("::app{attrs_str}\n::")
            } else {
                format!("::app{attrs_str}\n{content}\n::")
            }
        }

        Block::Build { base, runtime, edition, properties, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(b) = base {
                attrs_parts.push(format!("base=\"{}\"", escape_attr(b)));
            }
            if let Some(r) = runtime {
                attrs_parts.push(format!("runtime=\"{}\"", escape_attr(r)));
            }
            if let Some(e) = edition {
                attrs_parts.push(format!("edition=\"{}\"", escape_attr(e)));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for p in properties {
                content_lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::build{attrs_str}\n::")
            } else {
                format!("::build{attrs_str}\n{content}\n::")
            }
        }

        Block::InfraDatabase { name, shared_auth, volume_gb, properties, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(n) = name {
                attrs_parts.push(format!("name=\"{}\"", escape_attr(n)));
            }
            if *shared_auth {
                attrs_parts.push("shared-auth".to_string());
            }
            if let Some(v) = volume_gb {
                attrs_parts.push(format!("volume-gb={v}"));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for p in properties {
                content_lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::database{attrs_str}\n::")
            } else {
                format!("::database{attrs_str}\n{content}\n::")
            }
        }

        Block::Deploy { env, app, machines, memory, auto_stop, min_machines, strategy, properties, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(e) = env {
                attrs_parts.push(format!("env=\"{}\"", escape_attr(e)));
            }
            if let Some(a) = app {
                attrs_parts.push(format!("app=\"{}\"", escape_attr(a)));
            }
            if let Some(m) = machines {
                attrs_parts.push(format!("machines={m}"));
            }
            if let Some(mem) = memory {
                attrs_parts.push(format!("memory={mem}"));
            }
            if let Some(a) = auto_stop {
                attrs_parts.push(format!("auto-stop=\"{}\"", escape_attr(a)));
            }
            if let Some(min) = min_machines {
                attrs_parts.push(format!("min-machines={min}"));
            }
            if let Some(s) = strategy {
                attrs_parts.push(format!("strategy=\"{}\"", escape_attr(s)));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for p in properties {
                content_lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::deploy{attrs_str}\n::")
            } else {
                format!("::deploy{attrs_str}\n{content}\n::")
            }
        }

        Block::InfraEnv { tier, entries, .. } => {
            let attrs_str = tier
                .as_ref()
                .map(|t| format!("[tier=\"{}\"]", escape_attr(t)))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            for e in entries {
                match &e.default_value {
                    Some(v) => content_lines.push(format!("{}: {}", e.name, v)),
                    None => content_lines.push(e.name.clone()),
                }
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::env{attrs_str}\n::")
            } else {
                format!("::env{attrs_str}\n{content}\n::")
            }
        }

        Block::Health { path, method, grace, interval, timeout, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(p) = path {
                attrs_parts.push(format!("path=\"{}\"", escape_attr(p)));
            }
            if let Some(m) = method {
                attrs_parts.push(format!("method=\"{}\"", escape_attr(m)));
            }
            if let Some(g) = grace {
                attrs_parts.push(format!("grace=\"{}\"", escape_attr(g)));
            }
            if let Some(i) = interval {
                attrs_parts.push(format!("interval=\"{}\"", escape_attr(i)));
            }
            if let Some(t) = timeout {
                attrs_parts.push(format!("timeout=\"{}\"", escape_attr(t)));
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::health{attrs_str}\n::")
        }

        Block::Concurrency { concurrency_type, hard_limit, soft_limit, force_https, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(ct) = concurrency_type {
                attrs_parts.push(format!("type=\"{}\"", escape_attr(ct)));
            }
            if let Some(h) = hard_limit {
                attrs_parts.push(format!("hard-limit={h}"));
            }
            if let Some(s) = soft_limit {
                attrs_parts.push(format!("soft-limit={s}"));
            }
            if *force_https {
                attrs_parts.push("force-https".to_string());
            }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::concurrency{attrs_str}\n::")
        }

        Block::Cicd { provider, properties, .. } => {
            let attrs_str = provider
                .as_ref()
                .map(|p| format!("[provider=\"{}\"]", escape_attr(p)))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            for p in properties {
                content_lines.push(format!("{}: {}", p.key, p.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::cicd{attrs_str}\n::")
            } else {
                format!("::cicd{attrs_str}\n{content}\n::")
            }
        }

        Block::Smoke { script, checks, .. } => {
            let attrs_str = script
                .as_ref()
                .map(|s| format!("[script=\"{}\"]", escape_attr(s)))
                .unwrap_or_default();
            let mut content_lines = Vec::new();
            for c in checks {
                content_lines.push(format!("{} {} {}", c.method, c.path, c.expected));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::smoke{attrs_str}\n::")
            } else {
                format!("::smoke{attrs_str}\n{content}\n::")
            }
        }

        Block::Domains { entries, .. } => {
            let mut content_lines = Vec::new();
            for e in entries {
                content_lines.push(e.domain.clone());
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::domains\n::".to_string()
            } else {
                format!("::domains\n{content}\n::")
            }
        }

        Block::Crates { entries, .. } => {
            let mut content_lines = Vec::new();
            for e in entries {
                let src = e.source.as_deref().unwrap_or("*");
                content_lines.push(format!("{} = \"{src}\"", e.name));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::crates\n::".to_string()
            } else {
                format!("::crates\n{content}\n::")
            }
        }

        Block::DeployUrls { entries, .. } => {
            let mut content_lines = Vec::new();
            for e in entries {
                content_lines.push(format!("{}: {}", e.key, e.value));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::deploy-urls\n::".to_string()
            } else {
                format!("::deploy-urls\n{content}\n::")
            }
        }

        Block::Volumes { entries, .. } => {
            let mut content_lines = Vec::new();
            for e in entries {
                content_lines.push(format!("{}: {}", e.name, e.mount));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                "::volumes\n::".to_string()
            } else {
                format!("::volumes\n{content}\n::")
            }
        }

        Block::Model { name, fields, .. } => {
            let attrs_str = if name.is_empty() {
                String::new()
            } else {
                format!("[name=\"{}\"]", escape_attr(name))
            };
            let mut content_lines = Vec::new();
            for f in fields {
                let type_str = model_field_type_surf(&f.field_type);
                let constraints: Vec<String> = f.constraints.iter().map(|c| constraint_surf(c)).collect();
                if constraints.is_empty() {
                    content_lines.push(format!("- {}: {}", f.name, type_str));
                } else {
                    content_lines.push(format!("- {}: {} [{}]", f.name, type_str, constraints.join(", ")));
                }
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::model{attrs_str}\n::")
            } else {
                format!("::model{attrs_str}\n{content}\n::")
            }
        }

        Block::Route { method, path, auth, returns, body, handler, content, .. } => {
            let method_str = match method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
                HttpMethod::Put => "PUT",
                HttpMethod::Patch => "PATCH",
                HttpMethod::Delete => "DELETE",
            };
            let mut attrs_parts = vec![format!("method={method_str}")];
            if !path.is_empty() {
                attrs_parts.push(format!("path=\"{}\"", escape_attr(path)));
            }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut content_lines = Vec::new();
            if let Some(a) = auth { content_lines.push(format!("auth: {a}")); }
            if let Some(r) = returns { content_lines.push(format!("returns: {r}")); }
            if let Some(b) = body { content_lines.push(format!("body: {b}")); }
            if let Some(h) = handler {
                content_lines.push("```rust".to_string());
                content_lines.push(h.clone());
                content_lines.push("```".to_string());
            }
            if !content.is_empty() { content_lines.push(content.clone()); }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::route{attrs_str}\n::")
            } else {
                format!("::route{attrs_str}\n{content}\n::")
            }
        }

        Block::Auth { provider, session, roles, default_role, .. } => {
            let provider_str = match provider {
                crate::types::AuthProvider::Email => "email",
                crate::types::AuthProvider::OAuth => "oauth",
                crate::types::AuthProvider::ApiKey => "api-key",
                crate::types::AuthProvider::Token => "token",
            };
            let attrs_str = format!("[provider={provider_str}]");
            let mut content_lines = Vec::new();
            if let Some(s) = session { content_lines.push(format!("session: {s}")); }
            if !roles.is_empty() { content_lines.push(format!("roles: {}", roles.join(", "))); }
            if let Some(dr) = default_role { content_lines.push(format!("default_role: {dr}")); }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::auth{attrs_str}\n::")
            } else {
                format!("::auth{attrs_str}\n{content}\n::")
            }
        }

        Block::Binding { source, target, events, .. } => {
            let mut attrs_parts = Vec::new();
            if !source.is_empty() { attrs_parts.push(format!("source=\"{}\"", escape_attr(source))); }
            if !target.is_empty() { attrs_parts.push(format!("target=\"{}\"", escape_attr(target))); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut content_lines = Vec::new();
            for e in events {
                content_lines.push(format!("{}: {}", e.event, e.action));
            }
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::binding{attrs_str}\n::")
            } else {
                format!("::binding{attrs_str}\n{content}\n::")
            }
        }

        Block::Schema { name, fields, .. } => {
            let attrs_str = if name.is_empty() {
                String::new()
            } else {
                format!("[name=\"{}\"]", escape_attr(name))
            };
            let content_lines: Vec<String> = fields.iter().map(|f| {
                let constraints = if f.constraints.is_empty() {
                    String::new()
                } else {
                    format!(" {}", f.constraints.iter().map(|c| constraint_surf(c)).collect::<Vec<_>>().join(" "))
                };
                format!("- {} ({}){}", f.name, model_field_type_surf(&f.field_type), constraints)
            }).collect();
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::schema{attrs_str}\n::")
            } else {
                format!("::schema{attrs_str}\n{content}\n::")
            }
        }

        Block::Use { crates, .. } => {
            if crates.iter().all(|c| c.version.is_none() && c.features.is_empty()) {
                let names: Vec<String> = crates.iter().map(|c| c.name.clone()).collect();
                format!("::use[{}]\n::", names.join(", "))
            } else {
                let content_lines: Vec<String> = crates.iter().map(|c| {
                    let mut parts = vec![c.name.clone()];
                    if let Some(v) = &c.version { parts.push(v.clone()); }
                    if !c.features.is_empty() {
                        parts.push(format!("[{}]", c.features.join(", ")));
                    }
                    format!("- {}", parts.join(" "))
                }).collect();
                format!("::use\n{}\n::", content_lines.join("\n"))
            }
        }

        Block::AppEnv { vars, .. } => {
            if vars.iter().all(|v| !v.required && v.description.is_none()) {
                let names: Vec<String> = vars.iter().map(|v| v.name.clone()).collect();
                format!("::app-env[{}]\n::", names.join(", "))
            } else {
                let content_lines: Vec<String> = vars.iter().map(|v| {
                    let mut parts = vec![v.name.clone()];
                    if v.required { parts.push("*".to_string()); }
                    if let Some(d) = &v.description { parts.push(format!("\"{}\"", d)); }
                    format!("- {}", parts.join(" "))
                }).collect();
                format!("::app-env\n{}\n::", content_lines.join("\n"))
            }
        }

        Block::AppDeploy { region, scale, domain, memory, properties, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(r) = region { attrs_parts.push(format!("region=\"{}\"", escape_attr(r))); }
            if let Some(s) = scale { attrs_parts.push(format!("scale=\"{s}\"")); }
            if let Some(d) = domain { attrs_parts.push(format!("domain=\"{}\"", escape_attr(d))); }
            if let Some(m) = memory { attrs_parts.push(format!("memory=\"{}\"", escape_attr(m))); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let content_lines: Vec<String> = properties.iter().map(|(k, v)| format!("{k}: {v}")).collect();
            let content = content_lines.join("\n");
            if content.is_empty() {
                format!("::app-deploy{attrs_str}\n::")
            } else {
                format!("::app-deploy{attrs_str}\n{content}\n::")
            }
        }

        Block::Row { icon, title, description, href, state, unread, avatar, rtime, unread_count, trailing_label, trailing_action, action, actions, .. } => {
            let mut attrs_parts = vec![format!("icon={icon}")];
            if let Some(h) = href { attrs_parts.push(format!("href=\"{}\"", escape_attr(h))); }
            match state { RowState::Loading => attrs_parts.push("state=loading".to_string()), RowState::Empty => attrs_parts.push("state=empty".to_string()), _ => {} }
            if *unread { attrs_parts.push("unread=true".to_string()); }
            // 0.17: avatar serializes the resolved value (auto is already
            // derived to initials at parse, so round-trips are stable).
            if let Some(a) = avatar { attrs_parts.push(format!("avatar=\"{}\"", escape_attr(a))); }
            if let Some(t) = rtime { attrs_parts.push(format!("rtime=\"{}\"", escape_attr(t))); }
            if let Some(c) = unread_count { attrs_parts.push(format!("unread-count={c}")); }
            if let Some(l) = trailing_label { attrs_parts.push(format!("trailing-label=\"{}\"", escape_attr(l))); }
            if let Some(a) = trailing_action { attrs_parts.push(format!("trailing-action={a}")); }
            if let Some(a) = action { attrs_parts.push(format!("action={a}")); }
            // Space-separated: the attr grammar (attrs.rs) rejects commas
            // between pairs, so a comma join serialized rows whose attrs
            // (icon/href/action) silently vanished on re-parse.
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut content = format!("{title}\n{description}");
            for a in actions {
                if a.label == a.action {
                    content.push_str(&format!("\naction: {}", a.action));
                } else {
                    content.push_str(&format!("\naction: {} | {}", a.label, a.action));
                }
            }
            format!("::row{attrs_str}\n{content}\n::")
        }

        Block::InfoCard { intent, title, subtitle, summary, image, facts, steps, state, .. } => {
            let mut attrs_parts = vec![format!("intent={intent}")];
            if let Some(img) = image { attrs_parts.push(format!("image=\"{}\"", escape_attr(img))); }
            match state { RowState::Loading => attrs_parts.push("state=loading".to_string()), RowState::Empty => attrs_parts.push("state=empty".to_string()), _ => {} }
            // Space-separated for the same comma-rejection reason as ::row.
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let mut lines = vec![format!("# {title}")];
            if !subtitle.is_empty() { lines.push(subtitle.clone()); }
            if !summary.is_empty() { lines.push(String::new()); lines.push(summary.clone()); }
            for fact in facts { lines.push(format!("{}: {}", fact[0], fact[1])); }
            for (i, step) in steps.iter().enumerate() { lines.push(format!("{}. {}", i + 1, step)); }
            format!("::infocard{attrs_str}\n{}\n::", lines.join("\n"))
        }

        // ── Interactive / application blocks ──────────────────────

        Block::AppShell { layout, height, children, .. } => {
            let mut attrs_parts = vec![format!("layout=\"{}\"", escape_attr(layout))];
            if let Some(h) = height { attrs_parts.push(format!("height={h}")); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let inner = serialize_children(children);
            if inner.is_empty() {
                format!("::app-shell{attrs_str}\n::")
            } else {
                format!("::app-shell{attrs_str}\n{inner}\n::")
            }
        }

        Block::Sidebar { position, collapsible, width, children, .. } => {
            let mut attrs_parts = vec![format!("position={position}")];
            if *collapsible { attrs_parts.push("collapsible=true".to_string()); }
            if let Some(w) = width { attrs_parts.push(format!("width={w}")); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let inner = serialize_children(children);
            if inner.is_empty() {
                format!("::sidebar{attrs_str}\n::")
            } else {
                format!("::sidebar{attrs_str}\n{inner}\n::")
            }
        }

        Block::Panel { position, resizable, height, desktop_only, children, .. } => {
            let mut attrs_parts = vec![format!("position={position}")];
            if *resizable { attrs_parts.push("resizable=true".to_string()); }
            if let Some(h) = height { attrs_parts.push(format!("height={h}")); }
            if *desktop_only { attrs_parts.push("desktop-only=true".to_string()); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let inner = serialize_children(children);
            if inner.is_empty() {
                format!("::panel{attrs_str}\n::")
            } else {
                format!("::panel{attrs_str}\n{inner}\n::")
            }
        }

        Block::TabBar { active, items, .. } => {
            let attrs_str = match active {
                Some(a) => format!("[active=\"{}\"]", escape_attr(a)),
                None => String::new(),
            };
            let mut lines = Vec::new();
            for item in items {
                let mut brace_parts = Vec::new();
                if let Some(icon) = &item.icon { brace_parts.push(format!("icon={icon}")); }
                if item.unread { brace_parts.push("unread".to_string()); }
                if brace_parts.is_empty() {
                    lines.push(format!("- {} \"{}\"", item.id, escape_attr(&item.label)));
                } else {
                    lines.push(format!(
                        "- {} \"{}\" {{{}}}",
                        item.id,
                        escape_attr(&item.label),
                        brace_parts.join(" ")
                    ));
                }
            }
            let content = lines.join("\n");
            if content.is_empty() {
                format!("::tab-bar{attrs_str}\n::")
            } else {
                format!("::tab-bar{attrs_str}\n{content}\n::")
            }
        }

        Block::TabContent { tab, width, align, children, .. } => {
            let mut attrs_parts = vec![format!("tab=\"{}\"", escape_attr(tab))];
            if let Some(w) = width { attrs_parts.push(format!("width={w}")); }
            if let Some(a) = align { attrs_parts.push(format!("align={a}")); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let inner = serialize_children(children);
            if inner.is_empty() {
                format!("::tab-content{attrs_str}\n::")
            } else {
                format!("::tab-content{attrs_str}\n{inner}\n::")
            }
        }

        Block::Toolbar { title, title_source, items, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(t) = title { attrs_parts.push(format!("title=\"{}\"", escape_attr(t))); }
            if let Some(s) = title_source { attrs_parts.push(format!("title-source={s}")); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let mut lines = Vec::new();
            for item in items {
                match item {
                    crate::types::ToolbarItem::Button { label, action, icon, style, disabled, toggled, avatar, aria_label } => {
                        let mut parts = Vec::new();
                        if let Some(l) = label { parts.push(format!("label=\"{}\"", escape_attr(l))); }
                        if let Some(a) = action { parts.push(format!("action={a}")); }
                        if let Some(i) = icon { parts.push(format!("icon={i}")); }
                        if let Some(s) = style { parts.push(format!("style={s}")); }
                        if *disabled { parts.push("disabled=true".to_string()); }
                        if *toggled { parts.push("toggled=true".to_string()); }
                        if let Some(av) = avatar { parts.push(format!("avatar=\"{}\"", escape_attr(av))); }
                        if let Some(al) = aria_label { parts.push(format!("aria-label=\"{}\"", escape_attr(al))); }
                        lines.push(format!("- button[{}]", parts.join(" ")));
                    }
                    crate::types::ToolbarItem::Separator => lines.push("- separator".to_string()),
                    crate::types::ToolbarItem::Spacer => lines.push("- spacer".to_string()),
                    crate::types::ToolbarItem::Badge { value, color } => {
                        let mut parts = vec![format!("value=\"{}\"", escape_attr(value))];
                        if let Some(c) = color { parts.push(format!("color={c}")); }
                        lines.push(format!("- badge[{}]", parts.join(" ")));
                    }
                    crate::types::ToolbarItem::Dropdown { label, options, action } => {
                        let mut parts = vec![format!("label=\"{}\"", escape_attr(label))];
                        if let Some(o) = options { parts.push(format!("options=\"{}\"", escape_attr(o))); }
                        if let Some(a) = action { parts.push(format!("action={a}")); }
                        lines.push(format!("- dropdown[{}]", parts.join(" ")));
                    }
                    crate::types::ToolbarItem::Text { value, editable, action, size } => {
                        let mut parts = vec![format!("value=\"{}\"", escape_attr(value))];
                        if *editable { parts.push("editable=true".to_string()); }
                        if let Some(a) = action { parts.push(format!("action={a}")); }
                        if let Some(s) = size { parts.push(format!("size={s}")); }
                        lines.push(format!("- text[{}]", parts.join(" ")));
                    }
                }
            }
            let content = lines.join("\n");
            if content.is_empty() {
                format!("::toolbar{attrs_str}\n::")
            } else {
                format!("::toolbar{attrs_str}\n{content}\n::")
            }
        }

        Block::Drawer { name, position, width, trigger, children, .. } => {
            let mut attrs_parts = vec![format!("name=\"{}\"", escape_attr(name)), format!("position={position}")];
            if let Some(w) = width { attrs_parts.push(format!("width={w}")); }
            if let Some(t) = trigger { attrs_parts.push(format!("trigger={t}")); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let inner = serialize_children(children);
            if inner.is_empty() {
                format!("::drawer{attrs_str}\n::")
            } else {
                format!("::drawer{attrs_str}\n{inner}\n::")
            }
        }

        Block::Modal { name, title, width, placement, dismissible, children, .. } => {
            let mut attrs_parts = vec![format!("name=\"{}\"", escape_attr(name))];
            if let Some(t) = title { attrs_parts.push(format!("title=\"{}\"", escape_attr(t))); }
            if let Some(w) = width { attrs_parts.push(format!("width={w}")); }
            if placement != "centered" { attrs_parts.push(format!("placement={placement}")); }
            if !dismissible { attrs_parts.push("dismissible=false".to_string()); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            let inner = serialize_children(children);
            if inner.is_empty() {
                format!("::modal{attrs_str}\n::")
            } else {
                format!("::modal{attrs_str}\n{inner}\n::")
            }
        }

        Block::SegmentedControl { active, size, action, segments, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(a) = active { attrs_parts.push(format!("active={a}")); }
            if size != "compact" { attrs_parts.push(format!("size={size}")); }
            if let Some(a) = action { attrs_parts.push(format!("action={a}")); }
            let attrs_str = if attrs_parts.is_empty() { String::new() } else { format!("[{}]", attrs_parts.join(" ")) };
            let lines: Vec<String> = segments
                .iter()
                .map(|s| format!("- {} \"{}\"", s.id, escape_attr(&s.label)))
                .collect();
            if lines.is_empty() {
                format!("::segmented-control{attrs_str}\n::")
            } else {
                format!("::segmented-control{attrs_str}\n{}\n::", lines.join("\n"))
            }
        }

        Block::DropdownSelect { label, icon, selected, align, options, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(l) = label { attrs_parts.push(format!("label=\"{}\"", escape_attr(l))); }
            if let Some(i) = icon { attrs_parts.push(format!("icon={i}")); }
            if let Some(s) = selected { attrs_parts.push(format!("selected=\"{}\"", escape_attr(s))); }
            if align != "left" { attrs_parts.push(format!("align={align}")); }
            let attrs_str = if attrs_parts.is_empty() { String::new() } else { format!("[{}]", attrs_parts.join(" ")) };
            let mut lines = Vec::new();
            for opt in options {
                let mut parts = Vec::new();
                if let Some(d) = &opt.description { parts.push(format!("description=\"{}\"", escape_attr(d))); }
                if let Some(a) = &opt.action { parts.push(format!("action={a}")); }
                if let Some(i) = &opt.icon { parts.push(format!("icon={i}")); }
                let extra = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" ")) };
                lines.push(format!("- \"{}\"{extra}", escape_attr(&opt.label)));
            }
            if lines.is_empty() {
                format!("::dropdown-select{attrs_str}\n::")
            } else {
                format!("::dropdown-select{attrs_str}\n{}\n::", lines.join("\n"))
            }
        }

        Block::CommandPalette { trigger, items, .. } => {
            let attrs_str = match trigger {
                Some(t) => format!("[trigger=\"{}\"]", escape_attr(t)),
                None => String::new(),
            };
            let mut lines = Vec::new();
            for item in items {
                let mut parts = Vec::new();
                if let Some(d) = &item.description { parts.push(format!("description=\"{}\"", escape_attr(d))); }
                if let Some(a) = &item.action { parts.push(format!("action={a}")); }
                if let Some(i) = &item.icon { parts.push(format!("icon={i}")); }
                if let Some(g) = &item.group { parts.push(format!("group={g}")); }
                let extra = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" ")) };
                lines.push(format!("- \"{}\"{extra}", escape_attr(&item.label)));
            }
            let content = lines.join("\n");
            if content.is_empty() {
                format!("::command-palette{attrs_str}\n::")
            } else {
                format!("::command-palette{attrs_str}\n{content}\n::")
            }
        }

        Block::CodeEditor { lang, source, line_numbers, content, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(l) = lang { attrs_parts.push(format!("lang={l}")); }
            if let Some(s) = source { attrs_parts.push(format!("source={s}")); }
            if *line_numbers { attrs_parts.push("line-numbers=true".to_string()); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            if content.is_empty() {
                format!("::code-editor{attrs_str}\n::")
            } else {
                format!("::code-editor{attrs_str}\n{content}\n::")
            }
        }

        Block::BlockEditor { source, .. } => {
            let attrs_str = match source {
                Some(s) => format!("[source={s}]"),
                None => String::new(),
            };
            format!("::block-editor{attrs_str}\n::")
        }

        Block::Terminal { shell, cwd, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(s) = shell { attrs_parts.push(format!("shell={s}")); }
            if let Some(c) = cwd { attrs_parts.push(format!("cwd={c}")); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::terminal{attrs_str}\n::")
        }

        Block::NavTree { source, on_select, on_rename, on_delete, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(s) = source { attrs_parts.push(format!("source={s}")); }
            if let Some(s) = on_select { attrs_parts.push(format!("on-select={s}")); }
            if let Some(s) = on_rename { attrs_parts.push(format!("on-rename={s}")); }
            if let Some(s) = on_delete { attrs_parts.push(format!("on-delete={s}")); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::nav-tree{attrs_str}\n::")
        }

        Block::Badge { value, color, .. } => {
            let mut attrs_parts = vec![format!("value=\"{}\"", escape_attr(value))];
            if let Some(c) = color { attrs_parts.push(format!("color={c}")); }
            let attrs_str = format!("[{}]", attrs_parts.join(" "));
            format!("::badge{attrs_str}\n::")
        }

        Block::SuggestionChips { source, max, dismissible, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(s) = source { attrs_parts.push(format!("source={s}")); }
            if let Some(m) = max { attrs_parts.push(format!("max={m}")); }
            if *dismissible { attrs_parts.push("dismissible=true".to_string()); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::suggestion-chips{attrs_str}\n::")
        }

        Block::ChatThread { source, on_action, on_react, on_doc_open, messages, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(s) = source { attrs_parts.push(format!("source={s}")); }
            if let Some(a) = on_action { attrs_parts.push(format!("on-action={a}")); }
            if let Some(a) = on_react { attrs_parts.push(format!("on-react=\"{}\"", escape_attr(a))); }
            if let Some(a) = on_doc_open { attrs_parts.push(format!("on-doc-open=\"{}\"", escape_attr(a))); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            // 0.17: message children round-trip as dash items in the
            // parse grammar (`- side[attrs] text`).
            let mut lines = Vec::new();
            for m in messages {
                let mut mparts = Vec::new();
                if let Some(s) = &m.sender { mparts.push(format!("sender=\"{}\"", escape_attr(s))); }
                if let Some(t) = &m.timestamp { mparts.push(format!("time=\"{}\"", escape_attr(t))); }
                if !m.reactions.is_empty() {
                    let entries: Vec<String> = m.reactions.iter().map(|r| {
                        let mut e = r.label.clone();
                        if let Some(c) = r.count { e.push_str(&format!(":{c}")); }
                        if r.mine { e.push_str(":mine"); }
                        e
                    }).collect();
                    mparts.push(format!("reactions=\"{}\"", escape_attr(&entries.join("|"))));
                }
                let mattrs = if mparts.is_empty() {
                    String::new()
                } else {
                    format!("[{}]", mparts.join(" "))
                };
                lines.push(format!("- {}{mattrs} {}", m.side, m.text));
            }
            if lines.is_empty() {
                format!("::chat-thread{attrs_str}\n::")
            } else {
                format!("::chat-thread{attrs_str}\n{}\n::", lines.join("\n"))
            }
        }

        Block::ChipInput { label, placeholder, source, on_change, chips, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(l) = label { attrs_parts.push(format!("label=\"{}\"", escape_attr(l))); }
            if let Some(p) = placeholder { attrs_parts.push(format!("placeholder=\"{}\"", escape_attr(p))); }
            if let Some(s) = source { attrs_parts.push(format!("source={s}")); }
            if let Some(a) = on_change { attrs_parts.push(format!("on-change=\"{}\"", escape_attr(a))); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            let lines: Vec<String> = chips.iter().map(|c| format!("- {c}")).collect();
            if lines.is_empty() {
                format!("::chip-input{attrs_str}\n::")
            } else {
                format!("::chip-input{attrs_str}\n{}\n::", lines.join("\n"))
            }
        }

        Block::ChatInputSimple { placeholder, action, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(p) = placeholder { attrs_parts.push(format!("placeholder=\"{}\"", escape_attr(p))); }
            if let Some(a) = action { attrs_parts.push(format!("action={a}")); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::chat-input-simple{attrs_str}\n::")
        }

        Block::Progress { source, steps, .. } => {
            let attrs_str = match source {
                Some(s) => format!("[source={s}]"),
                None => String::new(),
            };
            let mut lines = Vec::new();
            for step in steps {
                let marker = match step.status.as_str() {
                    "done" => "\u{2713}",
                    "active" => "\u{25CF}",
                    _ => "\u{25CB}",
                };
                lines.push(format!("- {} {marker}", step.label));
            }
            let content = lines.join("\n");
            if content.is_empty() {
                format!("::progress{attrs_str}\n::")
            } else {
                format!("::progress{attrs_str}\n{content}\n::")
            }
        }

        Block::LogStream { source, tail, .. } => {
            let mut attrs_parts = Vec::new();
            if let Some(s) = source { attrs_parts.push(format!("source={s}")); }
            if let Some(t) = tail { attrs_parts.push(format!("tail={t}")); }
            let attrs_str = if attrs_parts.is_empty() {
                String::new()
            } else {
                format!("[{}]", attrs_parts.join(" "))
            };
            format!("::log-stream{attrs_str}\n::")
        }

        Block::ProblemList { source, .. } => {
            let attrs_str = match source {
                Some(s) => format!("[source={s}]"),
                None => String::new(),
            };
            format!("::problem-list{attrs_str}\n::")
        }

        // ── Messages/Contacts vocabulary (0.12) ───────────────────
        Block::RecipientPicker { source, mode, on_submit, .. } => {
            let mut attrs_parts = vec![format!("source=\"{}\"", escape_attr(source)), format!("mode={mode}")];
            if let Some(a) = on_submit { attrs_parts.push(format!("on-submit=\"{}\"", escape_attr(a))); }
            format!("::recipient-picker[{}]\n::", attrs_parts.join(" "))
        }

        Block::Qr { mode, on_resolve, .. } => {
            let mut attrs_parts = vec![format!("mode={mode}")];
            if let Some(a) = on_resolve { attrs_parts.push(format!("on-resolve=\"{}\"", escape_attr(a))); }
            format!("::qr[{}]\n::", attrs_parts.join(" "))
        }
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Serialize child blocks into a single string for container blocks.
fn serialize_children(children: &[Block]) -> String {
    let parts: Vec<String> = children.iter().map(|b| serialize_block(b)).collect();
    parts.join("\n\n")
}

fn callout_type_str(ct: CalloutType) -> &'static str {
    match ct {
        CalloutType::Info => "info",
        CalloutType::Warning => "warning",
        CalloutType::Danger => "danger",
        CalloutType::Tip => "tip",
        CalloutType::Note => "note",
        CalloutType::Success => "success",
        CalloutType::Context => "context",
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

fn citation_format_slug(f: Format) -> &'static str {
    match f {
        Format::Ieee => "ieee",
        Format::Acm => "acm",
        Format::Article => "article",
        Format::Mla => "mla",
        Format::Apa => "apa",
        Format::Chicago => "chicago",
    }
}

fn ref_type_str(t: RefType) -> &'static str {
    match t {
        RefType::Article => "article",
        RefType::Book => "book",
        RefType::Chapter => "chapter",
        RefType::Web => "web",
        RefType::Report => "report",
        RefType::Conference => "conference",
    }
}

/// Serialize an author list back to the `Last, First; Last2, First2` form.
fn authors_to_string(authors: &[Author]) -> String {
    authors
        .iter()
        .map(|a| match &a.given {
            Some(g) => format!("{}, {}", a.family, g),
            None => a.family.clone(),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Serialize a [`Reference`] back to a `::cite` block.
fn cite_to_surf(r: &Reference) -> String {
    let mut lines: Vec<String> = Vec::new();
    let attrs = format!("[key={} type={}]", r.key, ref_type_str(r.ref_type));
    if !r.authors.is_empty() {
        lines.push(format!("author = {}", authors_to_string(&r.authors)));
    }
    if !r.editors.is_empty() {
        lines.push(format!("editor = {}", authors_to_string(&r.editors)));
    }
    let field = |name: &str, v: &Option<String>| v.as_ref().map(|x| format!("{name} = {x}"));
    for l in [
        field("title", &r.title),
        field("container", &r.container),
        field("publisher", &r.publisher),
        field("year", &r.year),
        field("volume", &r.volume),
        field("issue", &r.issue),
        field("pages", &r.pages),
        field("url", &r.url),
        field("doi", &r.doi),
        field("accessed", &r.accessed),
        field("edition", &r.edition),
    ]
    .into_iter()
    .flatten()
    {
        lines.push(l);
    }
    if lines.is_empty() {
        format!("::cite{attrs}\n::")
    } else {
        format!("::cite{attrs}\n{}\n::", lines.join("\n"))
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
        FormFieldType::Password => "password",
        FormFieldType::Select => "select",
        FormFieldType::Textarea => "textarea",
    }
}

/// Escape a string value for use inside `[key="value"]` attribute brackets.
fn escape_attr(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Serialize one nav row back to `.surf` source, emitting the optional
/// `{icon= image= external}` brace when any are set (round-trips `parse_nav`).
fn serialize_nav_item(item: &crate::types::NavItem) -> String {
    let mut brace = Vec::new();
    if let Some(icon) = &item.icon {
        brace.push(format!("icon={}", icon));
    }
    if let Some(image) = &item.image {
        brace.push(format!("image={}", image));
    }
    if item.external {
        brace.push("external".to_string());
    }
    if brace.is_empty() {
        format!("- [{}]({})", item.label, item.href)
    } else {
        format!("- [{}]({}) {{{}}}", item.label, item.href, brace.join(" "))
    }
}

fn model_field_type_surf(ft: &crate::types::ModelFieldType) -> String {
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

fn constraint_surf(c: &crate::types::FieldConstraint) -> String {
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
    fn test_serialize_hero_button_flags_round_trip() {
        let doc = SurfDocBuilder::new()
            .hero(
                Some("H"),
                None,
                None,
                vec![
                    HeroButton { label: "Go".into(), href: "https://x.com".into(), primary: true, external: true },
                    HeroButton { label: "Docs".into(), href: "https://y.com".into(), primary: false, external: true },
                    HeroButton { label: "Home".into(), href: "/".into(), primary: false, external: false },
                ],
            )
            .build();
        let source = to_surf_source(&doc);
        assert!(source.contains("[Go](https://x.com){primary external}"));
        assert!(source.contains("[Docs](https://y.com){external}"));
        assert!(source.contains("[Home](/)\n"));
        // And the emitted source parses back to the same flags.
        let reparsed = crate::parse(&source).doc;
        let hero = reparsed.blocks.iter().find_map(|b| match b {
            Block::Hero { buttons, .. } => Some(buttons.clone()),
            _ => None,
        }).expect("hero present");
        assert!(hero[0].primary && hero[0].external);
        assert!(!hero[1].primary && hero[1].external);
        assert!(!hero[2].primary && !hero[2].external);
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
                    image: None, external: false,
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
                        image: None, external: false,
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
    fn test_roundtrip_modal_attrs() {
        let source = "::modal[name=\"confirm\" title=\"Confirm\" width=480 placement=top dismissible=false]\nBody\n::";
        let parsed = parse::parse(source);
        let out = to_surf_source(&parsed.doc);
        let reparsed = parse::parse(&out);
        match &reparsed.doc.blocks[0] {
            Block::Modal { name, title, width, placement, dismissible, .. } => {
                assert_eq!(name, "confirm");
                assert_eq!(title.as_deref(), Some("Confirm"));
                assert_eq!(*width, Some(480));
                assert_eq!(placement, "top");
                assert!(!*dismissible);
            }
            other => panic!("Expected Modal, got {:?}", other),
        }
    }

    #[test]
    fn test_roundtrip_toolbar_button_toggled() {
        let source = "::toolbar\n- button[label=\"Panel\" action=toggle_panel toggled=true]\n- button[label=\"Save\" action=save]\n::";
        let parsed = parse::parse(source);
        let out = to_surf_source(&parsed.doc);
        let reparsed = parse::parse(&out);
        match &reparsed.doc.blocks[0] {
            Block::Toolbar { items, .. } => {
                assert!(matches!(&items[0], crate::types::ToolbarItem::Button { toggled: true, .. }));
                assert!(matches!(&items[1], crate::types::ToolbarItem::Button { toggled: false, .. }));
            }
            other => panic!("Expected Toolbar, got {:?}", other),
        }
    }

    #[test]
    fn test_roundtrip_row_attrs_and_split_pane_children() {
        // Regression (0.14 messages round 2): the row/infocard serializers
        // joined attrs with ", " but the attr grammar rejects commas, so
        // every serialized row silently lost icon/href/action on re-parse
        // (the block fell back to defaults). Also pins the new split-pane
        // pane-children serialization shape end to end.
        let source = "::split-pane[ratio=\"40:60\" back-label=\"Chats\" back-action=closeConversation]\n:::pane[side=left]\n::::row[icon=knowledge href=\"/messages/sam\" action=openConversation avatar=\"SR\" rtime=\"1:42 PM\" unread-count=3]\nSam Rose\n::::\n:::\n:::pane[side=right]\nThread body\n:::\n::";
        let parsed = parse::parse(source);
        let out = to_surf_source(&parsed.doc);
        let reparsed = parse::parse(&out);
        assert_eq!(
            parsed.doc.to_html(),
            reparsed.doc.to_html(),
            "split-pane + row attrs must survive a builder round-trip"
        );
        match &reparsed.doc.blocks[0] {
            Block::SplitPane { back_label, back_action, left, right, .. } => {
                assert_eq!(back_label.as_deref(), Some("Chats"));
                assert_eq!(back_action.as_deref(), Some("closeConversation"));
                assert_eq!(right.len(), 1);
                match &left[0] {
                    Block::Row { icon, href, action, avatar, rtime, unread_count, .. } => {
                        assert_eq!(icon, "knowledge");
                        assert_eq!(href.as_deref(), Some("/messages/sam"));
                        assert_eq!(action.as_deref(), Some("openConversation"));
                        // 0.17 roster meta survives the round-trip.
                        assert_eq!(avatar.as_deref(), Some("SR"));
                        assert_eq!(rtime.as_deref(), Some("1:42 PM"));
                        assert_eq!(*unread_count, Some(3));
                    }
                    other => panic!("Expected Row in left pane, got {other:?}"),
                }
            }
            other => panic!("Expected SplitPane, got {other:?}"),
        }
    }

    #[test]
    fn test_roundtrip_chat_thread_messages() {
        // 0.17: message children (side/sender/time/text/reactions) must
        // survive serialize -> reparse, or the builder drops data.
        let source = "::chat-thread[source=chat.thread on-react=\"mutate:messages.react:emoji\"]\n\
            - them[sender=\"Danny\" time=\"1:42 PM\" reactions=\"Love:2:mine|Wave\"] Tahoe update finished\n\
            - own[time=\"1:44 PM\"] Yes, retry now\n\
            ::";
        let parsed = parse::parse(source);
        let out = to_surf_source(&parsed.doc);
        let reparsed = parse::parse(&out);
        assert_eq!(
            parsed.doc.to_html(),
            reparsed.doc.to_html(),
            "chat-thread messages must survive a builder round-trip"
        );
        match &reparsed.doc.blocks[0] {
            Block::ChatThread { messages, on_react, .. } => {
                assert_eq!(on_react.as_deref(), Some("mutate:messages.react:emoji"));
                assert_eq!(messages.len(), 2);
                assert_eq!(messages[0].sender.as_deref(), Some("Danny"));
                assert_eq!(messages[0].reactions.len(), 2);
                assert_eq!(messages[0].reactions[0].count, Some(2));
                assert!(messages[0].reactions[0].mine);
                assert_eq!(messages[1].side, "own");
            }
            other => panic!("Expected ChatThread, got {other:?}"),
        }
    }

    #[test]
    fn test_roundtrip_chip_input() {
        let source = "::chip-input[label=\"To:\" placeholder=\"Type a name…\" source=contacts on-change=\"invoke:messages.compose\"]\n- Danny Pappageorge\n- Ashley\n::";
        let parsed = parse::parse(source);
        let out = to_surf_source(&parsed.doc);
        let reparsed = parse::parse(&out);
        assert_eq!(
            parsed.doc.to_html(),
            reparsed.doc.to_html(),
            "chip-input must survive a builder round-trip"
        );
        match &reparsed.doc.blocks[0] {
            Block::ChipInput { label, chips, on_change, .. } => {
                assert_eq!(label.as_deref(), Some("To:"));
                assert_eq!(on_change.as_deref(), Some("invoke:messages.compose"));
                assert_eq!(chips.len(), 2);
            }
            other => panic!("Expected ChipInput, got {other:?}"),
        }
    }

    #[test]
    fn test_roundtrip_segmented_control() {
        let source = "::segmented-control[active=all size=regular action=filter]\n- all \"All\"\n- posts \"Posts\"\n::";
        let parsed = parse::parse(source);
        let out = to_surf_source(&parsed.doc);
        let reparsed = parse::parse(&out);
        match &reparsed.doc.blocks[0] {
            Block::SegmentedControl { active, size, action, segments, .. } => {
                assert_eq!(active.as_deref(), Some("all"));
                assert_eq!(size, "regular");
                assert_eq!(action.as_deref(), Some("filter"));
                assert_eq!(segments.len(), 2);
                assert_eq!(segments[1].id, "posts");
                assert_eq!(segments[1].label, "Posts");
            }
            other => panic!("Expected SegmentedControl, got {:?}", other),
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
                        image: None, external: false,
                    },
                    NavItem {
                        label: "About".into(),
                        href: "/about".into(),
                        icon: None,
                        image: None, external: false,
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
                        image: None, external: false,
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
    fn test_roundtrip_diagram() {
        // Round-trip (c1 acceptance): parse -> to_surf_source -> parse and
        // assert the Diagram fields are equal across the trip, for both kinds,
        // an unknown type, a no-title block, and an empty body.
        let cases = [
            "::diagram[type=architecture title=\"System\"]\nweb: Web Frontend\napi: API\nweb -> api: HTTPS\napi <-> web\n::",
            "::diagram[type=erd title=\"Data Model\"]\nusers: id pk, email unique\norders: id pk, user_id fk\nusers 1--* orders: places\n::",
            // Unknown type must survive losslessly (renders as prose fallback).
            "::diagram[type=flowchart title=\"Later\"]\nstart => end\n::",
            // No title.
            "::diagram[type=architecture]\na -> b\n::",
            // Empty body.
            "::diagram[type=erd title=\"Empty\"]\n::",
        ];

        for case in cases {
            let first = parse::parse(case);
            assert!(
                first.diagnostics.is_empty(),
                "Parse diagnostics for {case:?}: {:?}",
                first.diagnostics
            );
            let source = to_surf_source(&first.doc);
            let second = parse::parse(&source);

            let (Block::Diagram { diagram_type: t1, title: ti1, content: c1, .. },
                 Block::Diagram { diagram_type: t2, title: ti2, content: c2, .. }) =
                (&first.doc.blocks[0], &second.doc.blocks[0])
            else {
                panic!(
                    "Expected Diagram blocks for {case:?}, got {:?} / {:?}",
                    first.doc.blocks, second.doc.blocks
                );
            };
            assert_eq!(t1, t2, "diagram_type drifted for {case:?}");
            assert_eq!(ti1, ti2, "title drifted for {case:?}");
            assert_eq!(c1, c2, "content drifted for {case:?}");
        }
    }

    #[test]
    fn test_diagram_serialize_attrs() {
        // type then title; both quoted-title escaping and bare-type form.
        let titled = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Diagram {
                diagram_type: "erd".to_string(),
                title: Some("Data \"v2\"".to_string()),
                content: "users: id pk".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let source = to_surf_source(&titled);
        assert!(source.contains("::diagram[type=erd title=\"Data \\\"v2\\\"\"]"));
        assert!(source.contains("users: id pk"));

        // Empty type + no title -> bare ::diagram with no attr brackets.
        let bare = SurfDoc {
            front_matter: None,
            blocks: vec![Block::Diagram {
                diagram_type: String::new(),
                title: None,
                content: "a -> b".to_string(),
                span: Span::SYNTHETIC,
            }],
            source: String::new(),
        };
        let source = to_surf_source(&bare);
        assert!(source.contains("::diagram\na -> b\n::"));
        assert!(!source.contains("::diagram["));
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

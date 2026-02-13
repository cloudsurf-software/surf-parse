use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

/// A parsed SurfDoc document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfDoc {
    /// Parsed YAML front matter, if present.
    pub front_matter: Option<FrontMatter>,
    /// Ordered sequence of blocks in the document body.
    pub blocks: Vec<Block>,
    /// Original source text that was parsed.
    pub source: String,
}

/// YAML front matter fields.
///
/// Known fields are typed; unknown fields are captured in `extra`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<DocType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<DocStatus>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<Confidence>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<Vec<Related>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributors: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,

    /// Any front matter fields not covered by typed fields above.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// A cross-reference to another document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Related {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<Relationship>,
}

/// Relationship type for cross-references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Relationship {
    Produces,
    Consumes,
    References,
    Supersedes,
}

/// SurfDoc document types (front matter `type` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocType {
    Doc,
    Guide,
    Conversation,
    Plan,
    Agent,
    Preference,
    Report,
    Proposal,
    Incident,
    Review,
}

/// Document lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocStatus {
    Draft,
    Active,
    Closed,
    Archived,
}

/// Visibility/access scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    Personal,
    WorkspacePrivate,
    Workspace,
    Repo,
    Public,
}

/// Confidence level for guides and estimates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// A parsed block in the document body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Block {
    /// A block directive that has not yet been typed (Chunk 1 catch-all).
    Unknown {
        name: String,
        attrs: Attrs,
        content: String,
        span: Span,
    },
    /// Plain markdown content between directives.
    Markdown {
        content: String,
        span: Span,
    },
    /// Callout/admonition box.
    Callout {
        callout_type: CalloutType,
        title: Option<String>,
        content: String,
        span: Span,
    },
    /// Structured data table (CSV/JSON/inline rows).
    Data {
        id: Option<String>,
        format: DataFormat,
        sortable: bool,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        raw_content: String,
        span: Span,
    },
    /// Code block with optional language and file path.
    Code {
        lang: Option<String>,
        file: Option<String>,
        highlight: Vec<String>,
        content: String,
        span: Span,
    },
    /// Task list with checkbox items.
    Tasks {
        items: Vec<TaskItem>,
        span: Span,
    },
    /// Decision record.
    Decision {
        status: DecisionStatus,
        date: Option<String>,
        deciders: Vec<String>,
        content: String,
        span: Span,
    },
    /// Single metric display.
    Metric {
        label: String,
        value: String,
        trend: Option<Trend>,
        unit: Option<String>,
        span: Span,
    },
    /// Executive summary block.
    Summary {
        content: String,
        span: Span,
    },
    /// Figure with image source and caption.
    Figure {
        src: String,
        caption: Option<String>,
        alt: Option<String>,
        width: Option<String>,
        span: Span,
    },
    /// Tabbed content with named panels.
    Tabs {
        tabs: Vec<TabPanel>,
        span: Span,
    },
    /// Multi-column layout.
    Columns {
        columns: Vec<ColumnContent>,
        span: Span,
    },
    /// Attributed quote with optional source.
    Quote {
        content: String,
        attribution: Option<String>,
        cite: Option<String>,
        span: Span,
    },
    /// Call-to-action button.
    Cta {
        label: String,
        href: String,
        primary: bool,
        icon: Option<String>,
        span: Span,
    },
    /// Navigation bar with links.
    Nav {
        items: Vec<NavItem>,
        logo: Option<String>,
        span: Span,
    },
    /// Hero image visual.
    HeroImage {
        src: String,
        alt: Option<String>,
        span: Span,
    },
    /// Customer testimonial.
    Testimonial {
        content: String,
        author: Option<String>,
        role: Option<String>,
        company: Option<String>,
        span: Span,
    },
    /// Presentation style overrides (key-value pairs).
    Style {
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// FAQ accordion with question/answer pairs.
    Faq {
        items: Vec<FaqItem>,
        span: Span,
    },
    /// Pricing comparison table.
    PricingTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        span: Span,
    },
    /// Site-level configuration (one per document).
    Site {
        domain: Option<String>,
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// Page/route definition — container block with child blocks.
    Page {
        route: String,
        layout: Option<String>,
        title: Option<String>,
        sidebar: bool,
        /// Raw content for degradation renderers.
        content: String,
        /// Parsed child blocks (leaf directives resolved, rest as Markdown).
        children: Vec<Block>,
        span: Span,
    },
    /// Embedded external content (iframe).
    Embed {
        src: String,
        embed_type: Option<EmbedType>,
        width: Option<String>,
        height: Option<String>,
        title: Option<String>,
        span: Span,
    },
    /// Form with arbitrary fields that submits to the inbox.
    Form {
        fields: Vec<FormField>,
        submit_label: Option<String>,
        span: Span,
    },
    /// Image gallery with optional categories.
    Gallery {
        items: Vec<GalleryItem>,
        columns: Option<u32>,
        span: Span,
    },
    /// Structured footer with sections, copyright, and social links.
    Footer {
        sections: Vec<FooterSection>,
        copyright: Option<String>,
        social: Vec<SocialLink>,
        span: Span,
    },
}

/// Callout/admonition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalloutType {
    Info,
    Warning,
    Danger,
    Tip,
    Note,
    Success,
}

/// Data block format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataFormat {
    Table,
    Csv,
    Json,
}

/// A single task item within a `Tasks` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub done: bool,
    pub text: String,
    pub assignee: Option<String>,
}

/// Decision record status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Proposed,
    Accepted,
    Rejected,
    Superseded,
}

/// Metric trend direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trend {
    Up,
    Down,
    Flat,
}

/// A single tab panel within a `Tabs` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabPanel {
    pub label: String,
    pub content: String,
}

/// A single column in a `Columns` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnContent {
    pub content: String,
}

/// A key-value style override within a `Style` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleProperty {
    pub key: String,
    pub value: String,
}

/// A question/answer pair within a `Faq` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaqItem {
    pub question: String,
    pub answer: String,
}

/// A navigation link within a `Nav` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavItem {
    pub label: String,
    pub href: String,
    pub icon: Option<String>,
}

/// Type of embedded content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbedType {
    Map,
    Video,
    Audio,
    Generic,
}

/// A single field in a `Form` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub label: String,
    pub name: String,
    pub field_type: FormFieldType,
    pub required: bool,
    pub placeholder: Option<String>,
    pub options: Vec<String>,
}

/// Form field input types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormFieldType {
    Text,
    Email,
    Tel,
    Date,
    Number,
    Select,
    Textarea,
}

/// A single item in a `Gallery` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryItem {
    pub src: String,
    pub caption: Option<String>,
    pub alt: Option<String>,
    pub category: Option<String>,
}

/// A section within a `Footer` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FooterSection {
    pub heading: String,
    pub links: Vec<NavItem>,
}

/// A social media link within a `Footer` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLink {
    pub platform: String,
    pub href: String,
}

/// Inline extension found within text content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InlineExt {
    Evidence {
        tier: Option<u8>,
        source: Option<String>,
        text: String,
    },
    Status {
        value: String,
    },
}

/// Ordered map of attribute key-value pairs.
pub type Attrs = BTreeMap<String, AttrValue>;

/// A value inside a block directive attribute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttrValue {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

/// Source location of a block in the original document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// 1-based starting line number.
    pub start_line: usize,
    /// 1-based ending line number (inclusive).
    pub end_line: usize,
    /// 0-based byte offset of the first character.
    pub start_offset: usize,
    /// 0-based byte offset past the last character.
    pub end_offset: usize,
}

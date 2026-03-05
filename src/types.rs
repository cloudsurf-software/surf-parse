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
    App,
    Manifest,
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
    /// Collapsible content section.
    Details {
        title: Option<String>,
        open: bool,
        content: String,
        span: Span,
    },
    /// Labeled thematic break.
    Divider {
        label: Option<String>,
        span: Span,
    },
    /// Full hero section with headline, subtitle, CTA buttons.
    Hero {
        headline: Option<String>,
        subtitle: Option<String>,
        badge: Option<String>,
        align: String,
        image: Option<String>,
        buttons: Vec<HeroButton>,
        content: String,
        span: Span,
    },
    /// Card grid for features, products, or values.
    Features {
        cards: Vec<FeatureCard>,
        cols: Option<u32>,
        span: Span,
    },
    /// Numbered process/timeline steps.
    Steps {
        steps: Vec<StepItem>,
        span: Span,
    },
    /// Row of metric/stat cards.
    Stats {
        items: Vec<StatItem>,
        span: Span,
    },
    /// Feature comparison matrix with check/dash rendering.
    Comparison {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        highlight: Option<String>,
        span: Span,
    },
    /// Centered brand/logo display.
    Logo {
        src: String,
        alt: Option<String>,
        size: Option<u32>,
        span: Span,
    },
    /// Auto-generated table of contents from document headings.
    Toc {
        depth: u32,
        entries: Vec<TocEntry>,
        span: Span,
    },
    /// Before/After problem→solution visualization.
    BeforeAfter {
        before_items: Vec<BeforeAfterItem>,
        after_items: Vec<BeforeAfterItem>,
        transition: Option<String>,
        span: Span,
    },
    /// Horizontal flow pipeline with arrows between steps.
    Pipeline {
        steps: Vec<PipelineStep>,
        span: Span,
    },
    /// Page section container with background control and child blocks.
    Section {
        bg: Option<String>,
        headline: Option<String>,
        subtitle: Option<String>,
        content: String,
        children: Vec<Block>,
        span: Span,
    },
    /// Rich product card with badge, body, features, and CTA.
    ProductCard {
        title: String,
        subtitle: Option<String>,
        badge: Option<String>,
        badge_color: Option<String>,
        body: String,
        features: Vec<String>,
        cta_label: Option<String>,
        cta_href: Option<String>,
        span: Span,
    },

    // ----- App description blocks (data-bound) -----

    /// Dynamic data list with filtering and sorting.
    List {
        source: String,
        display: ListDisplay,
        item_template: String,
        filters: Vec<ListFilter>,
        sort: Option<SortSpec>,
        preload: bool,
        span: Span,
    },
    /// Kanban board with draggable cards.
    Board {
        source: String,
        columns: Vec<String>,
        card_template: Option<String>,
        preload: bool,
        span: Span,
    },
    /// CRUD form that submits via HTMX (extends ::form with action).
    Action {
        method: HttpMethod,
        target: String,
        label: String,
        fields: Vec<FormField>,
        confirm: Option<String>,
        span: Span,
    },
    /// Filter controls for data views.
    FilterBar {
        target_selector: String,
        fields: Vec<FilterField>,
        span: Span,
    },
    /// Search input with typeahead results.
    Search {
        source: String,
        placeholder: Option<String>,
        span: Span,
    },
    /// Metrics dashboard with auto-refresh.
    Dashboard {
        source: String,
        refresh: Option<u32>,
        span: Span,
    },
    /// Smart-routed chat input.
    ChatInput {
        action: String,
        placeholder: Option<String>,
        modes: Vec<String>,
        span: Span,
    },
    /// Real-time content feed (SSE or polling).
    Feed {
        source: String,
        stream: bool,
        span: Span,
    },

    // ----- Compound widget mount points -----

    /// Code/SurfDoc editor mount point.
    Editor {
        source: Option<String>,
        lang: Option<String>,
        preview: bool,
        span: Span,
    },
    /// Data visualization mount point.
    Chart {
        chart_type: ChartType,
        source: String,
        period: Option<String>,
        span: Span,
    },
    /// Resizable side-by-side layout mount point.
    SplitPane {
        ratio: String,
        span: Span,
    },

    // ----- Infrastructure manifest blocks -----

    /// Top-level app manifest container (like Page — recursively parses children).
    App {
        name: String,
        binary: Option<String>,
        region: Option<String>,
        port: Option<u32>,
        platform: Option<String>,
        content: String,
        children: Vec<Block>,
        span: Span,
    },
    /// Build configuration (base image, runtime, edition).
    Build {
        base: Option<String>,
        runtime: Option<String>,
        edition: Option<String>,
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// Infrastructure database configuration.
    InfraDatabase {
        name: Option<String>,
        shared_auth: bool,
        volume_gb: Option<u32>,
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// Deployment environment configuration.
    Deploy {
        env: Option<String>,
        app: Option<String>,
        machines: Option<u32>,
        memory: Option<u32>,
        auto_stop: Option<String>,
        min_machines: Option<u32>,
        strategy: Option<String>,
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// Environment variable group (required/recommended/optional/defaults).
    InfraEnv {
        tier: Option<String>,
        entries: Vec<EnvEntry>,
        span: Span,
    },
    /// Health check configuration.
    Health {
        path: Option<String>,
        method: Option<String>,
        grace: Option<String>,
        interval: Option<String>,
        timeout: Option<String>,
        span: Span,
    },
    /// Concurrency/connection limits.
    Concurrency {
        concurrency_type: Option<String>,
        hard_limit: Option<u32>,
        soft_limit: Option<u32>,
        force_https: bool,
        span: Span,
    },
    /// CI/CD pipeline configuration.
    Cicd {
        provider: Option<String>,
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// Smoke test checks (HTTP method + path + expected status).
    Smoke {
        script: Option<String>,
        checks: Vec<SmokeCheck>,
        span: Span,
    },
    /// Domain entries for the app.
    Domains {
        entries: Vec<DomainEntry>,
        span: Span,
    },
    /// Shared crate dependencies.
    Crates {
        entries: Vec<CrateEntry>,
        span: Span,
    },
    /// Per-environment deploy URLs.
    DeployUrls {
        entries: Vec<StyleProperty>,
        span: Span,
    },
    /// Named volume mounts.
    Volumes {
        entries: Vec<VolumeEntry>,
        span: Span,
    },

    // ----- App spec blocks (data layer + API) -----

    /// Data model definition with typed fields and constraints.
    Model {
        name: String,
        fields: Vec<ModelField>,
        span: Span,
    },
    /// API route/endpoint definition.
    Route {
        method: HttpMethod,
        path: String,
        auth: Option<String>,
        returns: Option<String>,
        body: Option<String>,
        content: String,
        span: Span,
    },
    /// Authentication configuration.
    Auth {
        provider: AuthProvider,
        session: Option<String>,
        roles: Vec<String>,
        default_role: Option<String>,
        span: Span,
    },
    /// Data-to-UI binding connecting a route/model to a UI block.
    Binding {
        source: String,
        target: String,
        events: Vec<BindingEvent>,
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

/// A button within a `Hero` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroButton {
    pub label: String,
    pub href: String,
    pub primary: bool,
}

/// A card within a `Features` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCard {
    pub title: String,
    pub icon: Option<String>,
    pub body: String,
    pub link_label: Option<String>,
    pub link_href: Option<String>,
}

/// A step within a `Steps` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepItem {
    pub title: String,
    pub time: Option<String>,
    pub body: String,
}

/// A stat within a `Stats` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatItem {
    pub value: String,
    pub label: String,
    pub color: Option<String>,
}

/// A TOC entry within a `Toc` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    pub text: String,
    pub id: String,
    pub level: u32,
}

/// An item within a `BeforeAfter` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeAfterItem {
    pub label: String,
    pub detail: String,
}

/// A step within a `Pipeline` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub label: String,
    pub description: Option<String>,
}

// ----- Infrastructure manifest supporting types -----

/// An environment variable entry within an `InfraEnv` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvEntry {
    pub name: String,
    pub default_value: Option<String>,
}

/// A smoke test check: HTTP method, path, expected status code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmokeCheck {
    pub method: String,
    pub path: String,
    pub expected: u16,
}

/// A domain entry with optional description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEntry {
    pub domain: String,
    pub description: Option<String>,
}

/// A shared crate dependency entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateEntry {
    pub name: String,
    pub source: Option<String>,
    pub features: Option<String>,
}

/// A named volume mount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeEntry {
    pub name: String,
    pub mount: String,
}

// ----- App description language supporting types -----

/// Display style for a `List` block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListDisplay {
    Card,
    Table,
    Compact,
}

/// A filter declared inside a `List` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilter {
    pub field: String,
}

/// Sort specification: field name + direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortSpec {
    pub field: String,
    pub descending: bool,
}

/// HTTP method for `Action` blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// A filter field in a `FilterBar` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterField {
    pub label: String,
    pub name: String,
    pub options: Vec<String>,
}

/// Chart visualization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartType {
    Line,
    Bar,
    Pie,
    Area,
}

// ----- App spec supporting types -----

/// A field within a `Model` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelField {
    pub name: String,
    pub field_type: ModelFieldType,
    pub constraints: Vec<FieldConstraint>,
}

/// Data types for model fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFieldType {
    Uuid,
    String,
    Int,
    Float,
    Bool,
    Datetime,
    Text,
    Json,
    /// Monetary value stored as i64 cents (e.g. 1999 = $19.99).
    Money,
    /// Image URL/path — stored as String, triggers upload codegen.
    Image,
    /// Email address — stored as String, auto-capped at 254 chars per RFC 5321.
    Email,
    /// URL — stored as String, auto-capped at 2048 chars.
    Url,
    /// Enum with named variants.
    Enum(Vec<String>),
    /// Foreign key reference to another model.
    Ref(String),
}

/// Constraints on a model field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldConstraint {
    Primary,
    Auto,
    Required,
    Optional,
    Unique,
    Max(u32),
    Min(u32),
    Default(String),
    /// Database index hint for query performance.
    Index,
}

/// Authentication provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    Email,
    OAuth,
    ApiKey,
    Token,
}

/// An event in a `Binding` block (on_create, on_update, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingEvent {
    pub event: String,
    pub action: String,
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

impl Span {
    /// A zero-valued span for programmatically constructed blocks that have no
    /// source location.
    pub const SYNTHETIC: Span = Span {
        start_line: 0,
        end_line: 0,
        start_offset: 0,
        end_offset: 0,
    };
}

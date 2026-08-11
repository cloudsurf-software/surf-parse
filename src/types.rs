use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::citation::Reference;

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

    /// Publication/citation format for `paper`/`report` doc types
    /// (`ieee`/`acm`/`article`/`mla`/`apa`/`chicago`). Optional; default None.
    #[serde(rename = "format", skip_serializing_if = "Option::is_none")]
    pub format: Option<Format>,

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
    /// Multi-page website (rendered via the `::site`/`::page` block family).
    Website,
    /// Multi-page web document. Synonym of [`DocType::Website`]: drives the
    /// exact same `::site`/`::page` multipage render path (rendering keys off
    /// the blocks, not the type), so `type: web` and `type: website` produce
    /// identical output for the same body.
    Web,
    /// Presentation deck (rendered via the `::deck`/`::slide` block family).
    Deck,
    /// Alias for [`DocType::Deck`] — `type: slides` in front matter.
    Slides,
    /// Presentation. Resolves to the same render profile as
    /// [`DocType::Deck`]/[`DocType::Slides`] (the slides path).
    Presentation,
    /// Scientific/academic paper. Pairs with a [`Format`] (`ieee`/`acm`/
    /// `article`) to select a paper template at render time.
    Paper,
}

/// Publication / citation format for papers and reports (front matter
/// `format` field).
///
/// Optional — when absent the render profile falls back to a sensible default
/// for the document type (`article` for papers, `mla` for reports). Aliases
/// accept the common upper/title-case spellings so `format: IEEE` and
/// `format: ieee` both parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// IEEE paper format (two-column, numbered citations).
    #[serde(alias = "IEEE", alias = "Ieee")]
    Ieee,
    /// ACM paper format (acmart).
    #[serde(alias = "ACM", alias = "Acm")]
    Acm,
    /// Generic single-column article paper format.
    #[serde(alias = "Article", alias = "ARTICLE")]
    Article,
    /// MLA report format (Works Cited, author-page citations).
    #[serde(alias = "MLA", alias = "Mla")]
    Mla,
    /// APA report format (References, author-date citations).
    #[serde(alias = "APA", alias = "Apa")]
    Apa,
    /// Chicago report format (notes-bibliography or author-date).
    #[serde(alias = "Chicago", alias = "CHICAGO")]
    Chicago,
}

/// A resolved rendering profile: the engine-level decision of *what kind of
/// artifact* a document produces, derived once from `(DocType, Option<Format>)`
/// so renderers consult a single small enum instead of re-deriving intent.
///
/// This is the Chunk 1 foundation other chunks build on: Chunk 5 keys the
/// slides path off [`RenderProfile::Presentation`], and Chunk 6 keys the
/// paper/report templates off [`RenderProfile::Paper`]/[`RenderProfile::Report`]
/// and the carried [`Format`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderProfile {
    /// Standard single-document rendering (doc, guide, plan, report-less, …).
    Document,
    /// Multi-page web rendering (`::site`/`::page`); from `web`/`website`.
    Web,
    /// Presentation/slides rendering; from `deck`/`slides`/`presentation`.
    Presentation,
    /// Scientific paper with the carried citation/template [`Format`].
    Paper(Format),
    /// Academic report with the carried citation/template [`Format`].
    Report(Format),
}

/// Resolve a `(DocType, Option<Format>)` pair to a [`RenderProfile`].
///
/// Pure and total: every [`DocType`] maps to exactly one profile, and every
/// [`Format`] reaches a profile (drift-tested). `None` for the doc type
/// resolves to [`RenderProfile::Document`]. Papers default to
/// [`Format::Article`] and reports to [`Format::Mla`] when `format` is absent.
pub fn render_profile(doc_type: Option<DocType>, format: Option<Format>) -> RenderProfile {
    match doc_type {
        None => RenderProfile::Document,
        Some(dt) => match dt {
            DocType::Website | DocType::Web => RenderProfile::Web,
            DocType::Deck | DocType::Slides | DocType::Presentation => {
                RenderProfile::Presentation
            }
            DocType::Paper => RenderProfile::Paper(format.unwrap_or(Format::Article)),
            DocType::Report => RenderProfile::Report(format.unwrap_or(Format::Mla)),
            DocType::Doc
            | DocType::Guide
            | DocType::Conversation
            | DocType::Plan
            | DocType::Agent
            | DocType::Preference
            | DocType::Proposal
            | DocType::Incident
            | DocType::Review
            | DocType::App
            | DocType::Manifest => RenderProfile::Document,
        },
    }
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
    /// A bibliographic reference definition (`::cite`). Renders nothing on its
    /// own; it registers a [`Reference`] consumed by inline `[@key]` citations
    /// and the `::bibliography` list. See [`crate::citation`].
    Cite {
        /// The parsed reference (carries its own `key`).
        reference: Reference,
        span: Span,
    },
    /// A rendered reference list (`::bibliography` / `::references`). Collects
    /// every `::cite`-defined reference and formats them per the active citation
    /// style (or the optional per-block `style=` override).
    Bibliography {
        /// Optional style override; defaults to the document's `format:` (or APA).
        style: Option<Format>,
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
    /// Native diagram block (`::diagram`) — architecture diagrams + ERDs.
    /// `content` is the raw DSL source, preserved verbatim for lossless
    /// round-trip; unknown `diagram_type` values degrade to prose at render.
    Diagram {
        /// Raw `type` attr value, lowercased; `""` if absent.
        diagram_type: String,
        title: Option<String>,
        content: String,
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
        /// Labelled link groups for the rich-shell drawer (e.g. Explore /
        /// Company / Products). Empty for a plain flat nav (the legacy form,
        /// which uses `items`).
        #[serde(default)]
        groups: Vec<NavGroup>,
        /// Brand wordmark text shown beside the logo (e.g. "CloudSurf").
        #[serde(default)]
        brand: Option<String>,
        /// Append a `®` superscript to the brand wordmark.
        #[serde(default)]
        brand_reg: bool,
        /// Trailing primary call-to-action link (per-page "Get in touch" etc.).
        #[serde(default)]
        cta: Option<NavItem>,
        /// Opt into the rich shell layout: a JS-driven slide-in drawer + scrim,
        /// grouped/iconned links, brand wordmark, and a head-provided theme
        /// toggle. When `false` (and no groups/brand), renders the legacy
        /// checkbox-drawer nav so existing consumers are unchanged.
        #[serde(default)]
        drawer: bool,
        /// Render a stripped topbar only: brand/logo + theme toggle, no
        /// hamburger, drawer, or scrim. Backward compatible (defaults false).
        #[serde(default)]
        minimal: bool,
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
    /// Deck-level configuration (one per document) — the `::deck` block.
    ///
    /// Peer of [`Block::Site`]: a leaf config block holding presentation
    /// properties (`theme`, `aspect`, `transition`, `accent`, `font`). The
    /// slides themselves are top-level [`Block::Slide`] siblings, exactly as
    /// `::page` blocks are siblings of `::site`.
    Deck {
        properties: Vec<StyleProperty>,
        span: Span,
    },
    /// A single slide — container block with child blocks.
    ///
    /// Peer of [`Block::Page`]: its `children` reuse the existing content
    /// block families (Hero/Features/Stats/Comparison/Quote/Code/…), so a
    /// slide is an *arrangement* of blocks that already exist, never a new
    /// content primitive.
    Slide {
        layout: Option<SlideLayout>,
        kicker: Option<String>,
        notes: Option<String>,
        /// Raw content for degradation renderers.
        content: String,
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
        /// Form submission target. When `Some`, the rendered `<form>` carries a
        /// real `action`/`method` so it can POST to a server route.
        action: Option<String>,
        /// HTTP method (`post`/`get`). Defaults to `post` when `action` is set.
        method: Option<String>,
        /// Emit a hidden `_honey` honeypot field for spam mitigation.
        honeypot: bool,
        span: Span,
    },
    /// Centered call-to-action band: heading + subtext + optional buttons.
    Banner {
        headline: Option<String>,
        subtitle: Option<String>,
        buttons: Vec<HeroButton>,
        /// Optional anchor id for in-page links (e.g. `#contact`).
        id: Option<String>,
        content: String,
        span: Span,
    },
    /// Grid of product link-cards, optionally split into labelled groups.
    ProductGrid {
        groups: Vec<ProductGroup>,
        /// `[tiles]` — apple.com-style promo tiles (full-bleed 2-up band,
        /// centered headline/tagline/CTA over a per-card background) instead
        /// of the compact emblem link-cards.
        tiles: bool,
        span: Span,
    },
    /// Card grid for a blog/news/events index.
    PostGrid {
        title: Option<String>,
        subtitle: Option<String>,
        items: Vec<PostItem>,
        span: Span,
    },
    /// Access-code card: password field + submit button.
    Gate {
        title: Option<String>,
        subtitle: Option<String>,
        /// Form POST target. Defaults to "".
        action: String,
        field_label: Option<String>,
        submit_label: Option<String>,
        error: Option<String>,
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
        /// Brand wordmark for the footer brand column (e.g. "CloudSurf").
        #[serde(default)]
        brand: Option<String>,
        /// Append a `®` superscript to the footer brand wordmark.
        #[serde(default)]
        brand_reg: bool,
        /// Logo image src for the footer brand column.
        #[serde(default)]
        brand_logo: Option<String>,
        /// Tagline under the brand (e.g. "Innovate • Simplify • Scale").
        #[serde(default)]
        tagline: Option<String>,
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
        /// Alt text for `image` (accessibility). `None` → decorative (`alt=""`).
        image_alt: Option<String>,
        /// Explicit layout hint: `stacked` (image above headline). When absent,
        /// the legacy `align`-driven behavior applies (centered → above,
        /// left → side).
        layout: Option<String>,
        /// Drop the hero's card background/shadow so it blends into the page
        /// (text over the page background instead of a gradient card).
        transparent: bool,
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
        /// Stream-seam event name the list live-updates on
        /// (`stream=conversation_updated`); the `stream=` attribute name
        /// follows the `::feed` precedent. (0.12)
        stream: Option<String>,
        /// Primary row-select action (`on-select=`), the `::nav-tree`
        /// convention. (0.12)
        on_select: Option<String>,
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
    /// Self-contained storefront widget: category-filtered product grid with
    /// add-to-cart, a live cart (qty steppers + line totals), and a checkout
    /// form that resolves to an order-confirmation card. Static data-bound (no
    /// backend; payment links out). Drives the commerce examples (Marketplace,
    /// Delivery, Local-service, …).
    Store {
        title: Option<String>,
        /// Currency symbol prefixed to prices (e.g. "$"). Defaults to "$".
        currency: Option<String>,
        items: Vec<StoreItem>,
        span: Span,
    },
    /// Self-contained appointment/booking widget: optional service selector +
    /// month calendar driven by per-day availability + slot picker + booking
    /// confirmation. Static data-bound (no backend); the inlined client script
    /// renders the calendar and drives selection → confirmation entirely in the
    /// browser. Drives the scheduling examples (Scheduler, Restaurant-reserve,
    /// Local-service-book, …).
    Booking {
        title: Option<String>,
        /// Heading shown above the service radios (e.g. "Service", "Treatment").
        service_label: Option<String>,
        services: Vec<BookingService>,
        days: Vec<BookingDay>,
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
        /// Optional chart title (from the `title=` attribute).
        title: Option<String>,
        /// Inline dataset parsed from the block body. When `Some`, the chart
        /// is rendered as real deterministic SVG; when `None` it falls back to
        /// the `source=` live-data mount point / static placeholder.
        data: Option<ChartData>,
        span: Span,
    },
    /// Resizable side-by-side layout mount point. Authored `::pane[side=left]`
    /// / `::pane[side=right]` children fill the two planes (order is the
    /// fallback when `side` is omitted: first pane left, second right; stray
    /// non-pane children fall to the left pane). `back-label` / `back-action`
    /// emit the small-screen back control in the right plane.
    SplitPane {
        ratio: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        back_label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        back_action: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        left: Vec<Block>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        right: Vec<Block>,
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
        /// App-level auth marker from the `auth=` attribute (e.g.
        /// `auth=password`). Free-form; distinct from the structured child
        /// `::auth` block.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth: Option<String>,
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
    /// API route/endpoint definition with optional embedded Rust handler.
    Route {
        method: HttpMethod,
        path: String,
        auth: Option<String>,
        returns: Option<String>,
        body: Option<String>,
        handler: Option<String>,
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

    // ----- App format blocks (schema, deps, config, deploy) -----

    /// Data schema definition with typed fields and constraints.
    Schema {
        name: String,
        fields: Vec<SchemaField>,
        span: Span,
    },
    /// Crate/dependency declarations for an app.
    Use {
        crates: Vec<CrateDep>,
        span: Span,
    },
    /// App-level environment variable declarations with descriptions.
    AppEnv {
        vars: Vec<EnvVar>,
        span: Span,
    },
    /// App-level deployment configuration.
    AppDeploy {
        region: Option<String>,
        scale: Option<u32>,
        domain: Option<String>,
        memory: Option<String>,
        properties: Vec<(String, String)>,
        span: Span,
    },

    /// A compact row component with icon, title, description, and optional link.
    /// Three states: default (content), loading (skeleton), empty (placeholder).
    /// `::row[icon=sparkle, href=/wiki/article, state=loading]`
    /// Content lines prefixed `action:` declare per-row actions
    /// (`action: Accept | invoke:contacts.accept`). (0.12)
    Row {
        icon: String,
        title: String,
        description: String,
        href: Option<String>,
        state: RowState,
        /// Right-side blue unread dot. Renderer invariant: the dot is a
        /// right-side element only — accent-left-border is BANNED.
        unread: bool,
        /// Right-side trailing action control: display label + action verb.
        trailing_label: Option<String>,
        trailing_action: Option<String>,
        /// Row-level action verb (0.14): stamped verbatim as `data-action`
        /// on the row root so dispatcher verbs (openConversation,
        /// askSurfyDoc, …) are reachable from authored rows.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        /// Per-row actions mapping labels to action strings. (0.12)
        actions: Vec<RowAction>,
        span: Span,
    },

    /// A knowledge/info card with title, subtitle, summary, facts, and optional image.
    /// `::infocard[intent=who, image=https://...]`
    InfoCard {
        intent: String,
        title: String,
        subtitle: String,
        summary: String,
        image: Option<String>,
        facts: Vec<[String; 2]>,
        steps: Vec<String>,
        state: RowState,
        span: Span,
    },

    // ----- Interactive / application blocks -----

    /// Root app container with layout mode.
    AppShell {
        layout: String,
        /// Explicit shell height in px; overrides the static-render clamp.
        height: Option<u32>,
        children: Vec<Block>,
        span: Span,
    },
    /// Collapsible side panel.
    Sidebar {
        position: String,
        collapsible: bool,
        width: Option<u32>,
        children: Vec<Block>,
        span: Span,
    },
    /// Resizable bottom/side panel.
    Panel {
        position: String,
        resizable: bool,
        height: Option<u32>,
        desktop_only: bool,
        children: Vec<Block>,
        span: Span,
    },
    /// Tab strip navigation.
    TabBar {
        active: Option<String>,
        items: Vec<TabBarItem>,
        span: Span,
    },
    /// Tab-associated content pane.
    TabContent {
        tab: String,
        /// Content-column width cap in px (`width=880`) — the ruled
        /// centered-column idiom for library lists. HTML-only, like
        /// app-shell `height`. (0.13)
        width: Option<u32>,
        /// Horizontal alignment of the capped column (`align=center`).
        /// HTML-only. (0.13)
        align: Option<String>,
        children: Vec<Block>,
        span: Span,
    },
    /// Horizontal toolbar with buttons, separators, badges, etc.
    Toolbar {
        /// Static toolbar/screen title (`title=`). (0.12)
        title: Option<String>,
        /// Source-bound dynamic title (`title-source=`), a registry name
        /// carried verbatim (e.g. `thread.display_name`). (0.12)
        title_source: Option<String>,
        items: Vec<ToolbarItem>,
        span: Span,
    },
    /// Slide-out drawer panel.
    Drawer {
        name: String,
        position: String,
        width: Option<u32>,
        trigger: Option<String>,
        children: Vec<Block>,
        span: Span,
    },
    /// Dialog overlay.
    Modal {
        name: String,
        title: Option<String>,
        width: Option<u32>,
        placement: String,
        dismissible: bool,
        children: Vec<Block>,
        span: Span,
    },
    /// Searchable command picker.
    CommandPalette {
        trigger: Option<String>,
        items: Vec<CommandItem>,
        span: Span,
    },
    /// Compact pill single-select control — a filter idiom, NOT a tab-bar
    /// style (segments select a filter value; they do not switch panes).
    SegmentedControl {
        active: Option<String>,
        size: String,
        action: Option<String>,
        segments: Vec<SegmentItem>,
        span: Span,
    },
    /// Anchored dropdown select menu with a trigger and an option list.
    DropdownSelect {
        label: Option<String>,
        icon: Option<String>,
        selected: Option<String>,
        align: String,
        options: Vec<DropdownOption>,
        span: Span,
    },
    /// Syntax-highlighted code editor.
    CodeEditor {
        lang: Option<String>,
        source: Option<String>,
        line_numbers: bool,
        content: String,
        span: Span,
    },
    /// Visual block editor mount point.
    BlockEditor {
        source: Option<String>,
        span: Span,
    },
    /// Shell/terminal panel.
    Terminal {
        shell: Option<String>,
        cwd: Option<String>,
        span: Span,
    },
    /// File/navigation tree.
    NavTree {
        source: Option<String>,
        on_select: Option<String>,
        on_rename: Option<String>,
        on_delete: Option<String>,
        span: Span,
    },
    /// Status badge pill.
    Badge {
        value: String,
        color: Option<String>,
        span: Span,
    },
    /// Clickable suggestion chips.
    SuggestionChips {
        source: Option<String>,
        max: Option<u32>,
        dismissible: bool,
        span: Span,
    },
    /// Chat conversation thread display.
    ChatThread {
        source: Option<String>,
        on_action: Option<String>,
        /// Reaction/tapback seam (`on-react=`). (0.12)
        on_react: Option<String>,
        /// Doc-chip open seam (`on-doc-open=`). (0.12)
        on_doc_open: Option<String>,
        span: Span,
    },
    /// Simple chat message input (distinct from app-bound ChatInput).
    ChatInputSimple {
        placeholder: Option<String>,
        action: Option<String>,
        span: Span,
    },
    /// Step/progress indicator.
    Progress {
        source: Option<String>,
        steps: Vec<ProgressStep>,
        span: Span,
    },
    /// Live log output stream.
    LogStream {
        source: Option<String>,
        tail: Option<u32>,
        span: Span,
    },
    /// Error/warning problem list.
    ProblemList {
        source: Option<String>,
        span: Span,
    },

    // ----- Messages/Contacts vocabulary (0.12) -----

    /// Recipient picker: choose one or more entries from a data source and
    /// submit the selection (`::recipient-picker[source=contacts mode=multi
    /// on-submit=...]`). The group-compose seam `::search` cannot express
    /// (typeahead-only). (0.12)
    RecipientPicker {
        source: String,
        /// Selection mode: "single" | "multi" (default "single").
        mode: String,
        /// Submit action for the completed selection.
        on_submit: Option<String>,
        span: Span,
    },
    /// Platform-conditional QR block (`::qr[mode=show|scan on-resolve=...]`):
    /// show-my-code or scan-a-code, with a resolve action fired on a
    /// successful scan/exchange. (0.12)
    Qr {
        /// "show" | "scan" (default "show").
        mode: String,
        /// Action fired with the resolved payload.
        on_resolve: Option<String>,
        span: Span,
    },
}

/// State for Row and InfoCard blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowState {
    Default,
    Loading,
    Empty,
}

/// A labelled action on a `Row` block (`action: Label | action_string`).
/// (0.12)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowAction {
    pub label: String,
    /// Authored action string in the minimal action grammar
    /// (`verb:target[:payload]`, bare name = invoke).
    pub action: String,
}

/// A tab item within a `TabBar` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabBarItem {
    pub id: String,
    pub label: String,
    /// Optional icon token (SF-symbol-ish, e.g. "doc.text"); native clients
    /// map it to an SFSymbol/Material icon, the web emits it as `data-icon`.
    pub icon: Option<String>,
    /// Right-side blue unread dot on the tab item.
    pub unread: bool,
}

/// An item within a `Toolbar` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarItem {
    Button {
        label: Option<String>,
        action: Option<String>,
        icon: Option<String>,
        style: Option<String>,
        disabled: bool,
        /// Accent-ring open/pressed state (e.g. a toolbar button whose
        /// panel is currently open).
        toggled: bool,
        /// Workspace-chip avatar slot (0.13.3): a short initial (e.g. "C")
        /// rendered as a circular badge before the label. Render concern
        /// only — same precedent as `icon` (no native schema field).
        avatar: Option<String>,
        /// Explicit accessible name (0.13.3, `aria-label=` attr): the G3
        /// icon-only buttons carry their old visible label here. Takes
        /// priority over the action/icon-derived fallback. Render concern
        /// only — same precedent as `avatar` (no native schema field).
        aria_label: Option<String>,
    },
    Separator,
    Spacer,
    Badge {
        value: String,
        color: Option<String>,
    },
    Dropdown {
        label: String,
        options: Option<String>,
        action: Option<String>,
    },
    Text {
        value: String,
        editable: bool,
        action: Option<String>,
        /// Font size in px (e.g. 22 for a toolbar wordmark).
        size: Option<u32>,
    },
}

/// A command within a `CommandPalette` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandItem {
    pub label: String,
    pub description: Option<String>,
    pub action: Option<String>,
    pub icon: Option<String>,
    pub group: Option<String>,
}

/// A segment within a `SegmentedControl` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentItem {
    pub id: String,
    pub label: String,
}

/// An option within a `DropdownSelect` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropdownOption {
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub action: Option<String>,
}

/// A step within a `Progress` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressStep {
    pub label: String,
    /// One of: "done", "active", "pending".
    pub status: String,
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
    Context,
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
    /// Image emblem src (product link-cards). Renders before the label, in
    /// place of an `icon`. Optional — links render text-only when absent.
    #[serde(default)]
    pub image: Option<String>,
    /// Open in a new tab (`target="_blank" rel="noopener"`). Used for external
    /// product links in the rich shell nav.
    #[serde(default)]
    pub external: bool,
}

/// A labelled group of navigation links within a `Nav` block (rich shell).
///
/// Mirrors [`ProductGroup`]: the grouped form parses its rows inside the nav
/// block's own grammar (`## Heading` introduces a group), never via nested
/// `::` blocks. Ungrouped navs leave `groups` empty and use `Nav.items`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavGroup {
    /// Group heading (e.g. "Explore"). `None` for an unlabelled lead group.
    pub label: Option<String>,
    pub items: Vec<NavItem>,
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

/// Layout of a single slide in a `::deck`.
///
/// A fixed set of presentation arrangements — the escape hatch for anything
/// outside this set is a per-slide `::style` block, not a new layout. This
/// keeps slides expressive without trying to be Figma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SlideLayout {
    /// Title/opening slide with large headline.
    Cover,
    /// Title-only slide.
    Title,
    /// Section divider.
    Section,
    /// Default body slide — bulleted content.
    #[default]
    Bullets,
    /// Card grid (maps to `::features`).
    Cards,
    /// Side-by-side comparison (maps to `::comparison`).
    Compare,
    /// Single big statistic (maps to `::stats`).
    Stat,
    /// Pull-quote slide (maps to `::quote`).
    Quote,
    /// Live-demo / terminal slide.
    Demo,
    /// Full-bleed image.
    Image,
    /// Two-column split.
    Two,
    /// Code-focused slide (monospace, full-bleed code block).
    Code,
    /// Empty canvas — content controls itself.
    Blank,
}

impl SlideLayout {
    /// Parse a layout name from a `::slide` `layout:` attribute.
    pub fn from_name(s: &str) -> Option<SlideLayout> {
        match s.trim().to_ascii_lowercase().as_str() {
            "cover" => Some(SlideLayout::Cover),
            "title" => Some(SlideLayout::Title),
            "section" => Some(SlideLayout::Section),
            "bullets" | "default" => Some(SlideLayout::Bullets),
            "cards" => Some(SlideLayout::Cards),
            "compare" | "comparison" => Some(SlideLayout::Compare),
            "stat" | "stats" => Some(SlideLayout::Stat),
            "quote" => Some(SlideLayout::Quote),
            "demo" => Some(SlideLayout::Demo),
            "image" => Some(SlideLayout::Image),
            "two" | "split" | "two-column" | "two-col" | "twocolumn" => Some(SlideLayout::Two),
            "code" => Some(SlideLayout::Code),
            "blank" => Some(SlideLayout::Blank),
            _ => None,
        }
    }

    /// The CSS class suffix for this layout (e.g. `cover` → `slide cover`).
    pub fn css_class(self) -> &'static str {
        match self {
            SlideLayout::Cover => "cover",
            SlideLayout::Title => "title",
            SlideLayout::Section => "section",
            SlideLayout::Bullets => "bullets",
            SlideLayout::Cards => "cards",
            SlideLayout::Compare => "compare",
            SlideLayout::Stat => "stat",
            SlideLayout::Quote => "quote",
            SlideLayout::Demo => "demo",
            SlideLayout::Image => "image",
            SlideLayout::Two => "two",
            SlideLayout::Code => "code",
            SlideLayout::Blank => "blank",
        }
    }
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
    Password,
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

/// A button within a `Hero` or `Banner` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroButton {
    pub label: String,
    pub href: String,
    pub primary: bool,
    /// `{external}` — the button opens in a new tab (`target="_blank" rel="noopener"`).
    /// Combines with primary as `{primary external}`.
    #[serde(default)]
    pub external: bool,
}

/// A labelled group of product link-cards within a `ProductGrid`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductGroup {
    /// Group heading (e.g. "Platforms"). Empty when the grid is ungrouped.
    pub label: Option<String>,
    pub items: Vec<ProductItem>,
    /// `{cols=N}` on the heading line (tiles mode): max columns for this
    /// group's row, clamped 1–3. None → the 2-col default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u8>,
}

/// A single product link-card: emblem + name + tagline + link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductItem {
    pub name: String,
    pub href: String,
    /// Emblem image src. Optional — cards render without an image when absent.
    pub emblem: Option<String>,
    pub tagline: Option<String>,
    /// Primary CTA override (`[Label](href)`, first of two trailing link
    /// fields): replaces the default "Learn more" pill (tiles mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta1_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta1_href: Option<String>,
    /// Secondary CTA (`[Label](href)` trailing pipe field): renders as the
    /// second, outline pill in the tile's CTA row (tiles mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta2_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cta2_href: Option<String>,
    /// Tile background spec (5th pipe field, raw author value):
    /// `image:<src>` | `color:<css-color>` | `gradient:<css-gradient>` |
    /// `transparent` (default when absent). A trailing ` dark` token flags a
    /// dark background → the tile renders light text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
}

/// A card within a `PostGrid` block: a blog/news/events index entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostItem {
    pub title: String,
    pub href: String,
    /// Meta line above the title (e.g. "Category · Date").
    pub meta: Option<String>,
    /// One-line excerpt under the title.
    pub excerpt: Option<String>,
    /// Optional lead image src.
    pub image: Option<String>,
    /// Open in a new tab (renders `target="_blank" rel="noopener"`).
    pub external: bool,
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

/// A product line in a `Store` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreItem {
    pub name: String,
    /// Numeric price as authored (e.g. "48", "12.50"); the widget parses it.
    pub price: String,
    /// One-line description shown under the name. Optional.
    pub blurb: Option<String>,
    /// Corner badge (e.g. "Bestseller", "New"). Optional.
    pub badge: Option<String>,
    /// Category for the filter chips. `None` groups under "All".
    pub category: Option<String>,
}

/// A bookable service in a `Booking` block (e.g. "60-min Strategy Call").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingService {
    pub name: String,
    /// Free-text duration (e.g. "60 min"). Optional.
    pub duration: Option<String>,
    /// Free-text price (e.g. "$120", "Free"). Optional.
    pub price: Option<String>,
}

/// One day's availability in a `Booking` block. An empty `slots` (or the
/// literal `full`) marks the day as unavailable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingDay {
    /// ISO date `YYYY-MM-DD`.
    pub date: String,
    /// Selectable time-slot labels (e.g. "9:00 AM"); empty = unavailable.
    pub slots: Vec<String>,
}

/// Chart visualization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartType {
    Line,
    Bar,
    Pie,
    Area,
    /// Numeric x/y point cloud (one or more series).
    Scatter,
    /// Pie with an inner radius (ring).
    Donut,
    /// Bars stacked per category across series.
    StackedBar,
    /// Polygon per series over N labelled axes.
    Radar,
}

/// One named numeric series of a [`ChartData`] dataset. `values` is aligned
/// position-for-position with the owning dataset's `categories`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    /// Series name (shown in the legend).
    pub name: String,
    /// y-values, one per category. Missing/non-numeric cells parse to `0.0`.
    pub values: Vec<f64>,
}

/// Inline chart dataset parsed from a `::chart` block body — a shared category
/// axis plus one or more named series. When a `Chart` block has `data: None`
/// it falls back to the live-data mount point (`source=`) / static placeholder.
///
/// For cartesian charts (line/area/bar/stacked-bar) `categories` are x-axis
/// labels; for scatter they are the numeric x-values (kept as strings); for
/// radar they are the axis labels; for pie/donut they are the slice labels and
/// only the first series is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    /// Category / x-axis / slice / axis labels (one per data point).
    pub categories: Vec<String>,
    /// One or more named series of values aligned to `categories`.
    pub series: Vec<ChartSeries>,
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

// ----- App format supporting types -----

/// A field within a `Schema` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub field_type: ModelFieldType,
    pub constraints: Vec<FieldConstraint>,
}

/// A crate dependency within a `Use` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateDep {
    pub name: String,
    pub version: Option<String>,
    pub features: Vec<String>,
}

/// An environment variable declaration within an `AppEnv` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
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

#[cfg(test)]
mod doc_type_format_tests {
    use super::*;

    fn parse_fm(yaml: &str) -> FrontMatter {
        serde_yaml::from_str::<FrontMatter>(yaml).expect("front matter should parse")
    }

    // ----- L1/L2: new DocType variants deserialize from `type:` -----

    #[test]
    fn new_doc_types_deserialize() {
        assert_eq!(parse_fm("type: presentation").doc_type, Some(DocType::Presentation));
        assert_eq!(parse_fm("type: web").doc_type, Some(DocType::Web));
        assert_eq!(parse_fm("type: website").doc_type, Some(DocType::Website));
        assert_eq!(parse_fm("type: paper").doc_type, Some(DocType::Paper));
        assert_eq!(parse_fm("type: report").doc_type, Some(DocType::Report));
    }

    #[test]
    fn existing_doc_types_unchanged() {
        assert_eq!(parse_fm("type: deck").doc_type, Some(DocType::Deck));
        assert_eq!(parse_fm("type: slides").doc_type, Some(DocType::Slides));
        assert_eq!(parse_fm("type: doc").doc_type, Some(DocType::Doc));
    }

    // ----- L1/L2: Format values + aliases + missing -----

    #[test]
    fn format_values_deserialize() {
        for (s, want) in [
            ("ieee", Format::Ieee),
            ("acm", Format::Acm),
            ("article", Format::Article),
            ("mla", Format::Mla),
            ("apa", Format::Apa),
            ("chicago", Format::Chicago),
        ] {
            assert_eq!(parse_fm(&format!("format: {s}")).format, Some(want));
        }
    }

    #[test]
    fn format_aliases_are_case_insensitive() {
        assert_eq!(parse_fm("format: IEEE").format, Some(Format::Ieee));
        assert_eq!(parse_fm("format: ACM").format, Some(Format::Acm));
        assert_eq!(parse_fm("format: MLA").format, Some(Format::Mla));
        assert_eq!(parse_fm("format: APA").format, Some(Format::Apa));
        assert_eq!(parse_fm("format: Chicago").format, Some(Format::Chicago));
        assert_eq!(parse_fm("format: Article").format, Some(Format::Article));
    }

    #[test]
    fn format_missing_defaults_to_none() {
        assert_eq!(parse_fm("type: paper").format, None);
        assert_eq!(parse_fm("title: Hello").format, None);
    }

    // ----- L3 drift: every Format reaches a RenderProfile -----

    #[test]
    fn every_format_maps_to_a_render_profile() {
        for f in [
            Format::Ieee,
            Format::Acm,
            Format::Article,
            Format::Mla,
            Format::Apa,
            Format::Chicago,
        ] {
            // Papers carry the format through.
            assert_eq!(
                render_profile(Some(DocType::Paper), Some(f)),
                RenderProfile::Paper(f)
            );
            // Reports carry the format through.
            assert_eq!(
                render_profile(Some(DocType::Report), Some(f)),
                RenderProfile::Report(f)
            );
        }
    }

    #[test]
    fn render_profile_mapping_is_total_and_stable() {
        // None → Document.
        assert_eq!(render_profile(None, None), RenderProfile::Document);
        // Web == Website profile.
        assert_eq!(render_profile(Some(DocType::Web), None), RenderProfile::Web);
        assert_eq!(render_profile(Some(DocType::Website), None), RenderProfile::Web);
        // Deck / Slides / Presentation → Presentation.
        assert_eq!(
            render_profile(Some(DocType::Deck), None),
            RenderProfile::Presentation
        );
        assert_eq!(
            render_profile(Some(DocType::Slides), None),
            RenderProfile::Presentation
        );
        assert_eq!(
            render_profile(Some(DocType::Presentation), None),
            RenderProfile::Presentation
        );
        // Defaults when format omitted.
        assert_eq!(
            render_profile(Some(DocType::Paper), None),
            RenderProfile::Paper(Format::Article)
        );
        assert_eq!(
            render_profile(Some(DocType::Report), None),
            RenderProfile::Report(Format::Mla)
        );
        // Ordinary docs → Document.
        for dt in [DocType::Doc, DocType::Guide, DocType::Plan, DocType::App] {
            assert_eq!(render_profile(Some(dt), None), RenderProfile::Document);
        }
    }
}

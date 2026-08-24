//! Native block renderer for mobile/desktop native rendering via UniFFI.
//!
//! Converts a `SurfDoc` into a flat `Vec<NativeBlock>` suitable for export
//! across the FFI boundary. Wavesite-specific block types (Site, Page, Nav,
//! HeroImage, Footer, Embed, PricingTable) are now native. Remaining web-only
//! types (Style, Logo, Unknown, and app-spec / infra blocks) still degrade to
//! their markdown equivalent.

use serde::{Deserialize, Serialize};

use crate::diagram_scene::NativeDiagramScene;
use crate::render_md;
use crate::types::{
    Block, CalloutType, DecisionStatus, EmbedType, FormFieldType, PerClass, RowState, SizeClass,
    SurfDoc, ToolbarItem, Trend,
};

/// Maximum nesting depth for SectionContainer children.
/// At this depth, nested sections fall back to Markdown.
const MAX_SECTION_DEPTH: u32 = 8;

// ═══════════════════════════════════════════════════════════════════════
// NativeBlock enum — 74 native variants (pinned cross-platform by the
// SurfDocKit DispatchCoverageTests / Android NativeBlockCoverageTest census)
// ═══════════════════════════════════════════════════════════════════════

/// Simplified block representation for native mobile rendering via UniFFI.
///
/// Every field uses only UniFFI-safe types: `String`, `bool`, `u32`,
/// `Option<T>`, `Vec<T>`, and simple structs of the same. No `BTreeMap`,
/// no `Span`, no serde tags, no `enum` sub-types with complex discriminants.
///
/// Web-only blocks (Style, Logo, Banner, the build-engine/manifest blocks,
/// Unknown, …) are degraded to their markdown equivalent and emitted as
/// `NativeBlock::Markdown`. The reader-content blocks ProductCard, Chart,
/// Row, InfoCard and Diagram render structurally (below).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(tag = "type", rename_all = "snake_case")]
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

    /// Hero section — headline + subtitle + optional badge, optional
    /// banner image, alignment hint (`left` / `center` / `right`), a
    /// list of action buttons, and free-form body content that renders
    /// between the subtitle and the buttons on the web.
    Hero {
        headline: Option<String>,
        subtitle: Option<String>,
        badge: Option<String>,
        align: String,
        image: Option<String>,
        buttons: Vec<NativeHeroButton>,
        content: String,
    },

    /// Feature card grid.
    Features {
        cards: Vec<NativeFeatureCard>,
        /// Per-size-class column count (schema v5); `None` = client default.
        cols: Option<NativePerClassU32>,
    },

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

    /// Form with typed input fields for native rendering.
    /// No action URL — the native app controls form submission.
    Form {
        fields: Vec<NativeFormField>,
        submit_label: String,
    },

    /// Image gallery with grid layout and optional category filtering.
    Gallery {
        items: Vec<NativeGalleryItem>,
        /// Per-size-class column count (schema v5); a document that authored
        /// a single value carries the same number in all three fields.
        columns: NativePerClassU32,
    },

    /// Page section container with optional background and headline.
    /// This is the only recursive NativeBlock variant — `children` contains
    /// nested NativeBlock values. UniFFI supports recursive enums via boxing.
    SectionContainer {
        bg: Option<String>,
        headline: Option<String>,
        subtitle: Option<String>,
        children: Vec<NativeBlock>,
    },

    // ── Interactive block types (20 new variants) ──────────────────

    // Layout
    /// Application shell with layout mode and nested children.
    /// `layout` is one of: "sidebar", "split", "tabs".
    AppShell {
        layout: String,
        /// Present only for `layout == "adaptive"` (schema v5).
        adaptive: Option<NativeAdaptiveLayout>,
        children: Vec<NativeBlock>,
    },
    /// Collapsible sidebar navigation panel.
    /// `position` is one of: "left", "right".
    Sidebar {
        position: String,
        collapsible: bool,
        /// Per-size-class since schema v5.
        width: Option<NativePerClassU32>,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },
    /// Resizable panel (bottom or side).
    /// `position` is one of: "bottom", "right", "left".
    Panel {
        position: String,
        resizable: bool,
        height: Option<u32>,
        /// DEPRECATED at schema v5 — read `gate` instead.
        desktop_only: bool,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },

    // Navigation
    /// Tab strip navigation bar with selectable items.
    TabBar {
        active: Option<String>,
        items: Vec<NativeTabBarItem>,
    },
    /// Content pane associated with a specific tab.
    TabContent {
        tab: String,
        /// Content-column width cap, per size class. Reached HTML from 0.13
        /// but died at the FFI until schema v5.
        width: Option<NativePerClassU32>,
        /// Horizontal alignment of the capped column ("center"). Same hole.
        align: Option<String>,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },
    /// Horizontal toolbar with buttons, separators, badges, dropdowns.
    Toolbar {
        /// Static toolbar/screen title (0.12).
        title: Option<String>,
        /// Source-bound dynamic title — a registry name the client
        /// resolves at render time (e.g. `thread.display_name`). (0.12)
        title_source: Option<String>,
        items: Vec<NativeToolbarItem>,
    },

    // Overlays
    /// Slide-out drawer panel.
    /// `position` is one of: "left", "right".
    Drawer {
        name: String,
        position: String,
        /// Per-size-class since schema v5.
        width: Option<NativePerClassU32>,
        trigger: Option<String>,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },
    /// Dialog overlay / modal.
    Modal {
        name: String,
        title: Option<String>,
        children: Vec<NativeBlock>,
    },
    /// Searchable command palette / picker.
    CommandPalette {
        trigger: Option<String>,
        items: Vec<NativeCommandItem>,
    },

    // Interactive
    /// Syntax-highlighted code editor.
    CodeEditor {
        lang: Option<String>,
        source: Option<String>,
        line_numbers: bool,
        content: String,
    },
    /// Visual block editor mount point.
    BlockEditor {
        source: Option<String>,
    },
    /// Shell/terminal panel.
    Terminal {
        shell: Option<String>,
        cwd: Option<String>,
    },

    // Data
    /// File/navigation tree.
    NavTree {
        source: Option<String>,
        on_select: Option<NativeAction>,
        on_rename: Option<NativeAction>,
        on_delete: Option<NativeAction>,
    },
    /// Status badge pill.
    Badge {
        value: String,
        color: Option<String>,
    },
    /// Clickable suggestion chip list.
    SuggestionChips {
        source: Option<String>,
        max: Option<u32>,
        dismissible: bool,
    },
    /// Chat conversation thread display.
    ChatThread {
        source: Option<String>,
        on_action: Option<NativeAction>,
        /// Reaction/tapback seam (0.12).
        on_react: Option<NativeAction>,
        /// Doc-chip open seam (0.12).
        on_doc_open: Option<NativeAction>,
        /// Authored message children (0.17); empty = registry-bound thread.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        messages: Vec<NativeChatMessage>,
    },
    /// Simple chat message input.
    ChatInputSimple {
        placeholder: Option<String>,
        action: Option<NativeAction>,
    },
    /// Recipient chip input (0.17) — the compose "To:" line: label,
    /// removable chips, inline filter input.
    ChipInput {
        label: Option<String>,
        placeholder: Option<String>,
        source: Option<String>,
        on_change: Option<NativeAction>,
        chips: Vec<String>,
    },
    /// Step/progress indicator.
    Progress {
        source: Option<String>,
        steps: Vec<NativeProgressStep>,
    },
    /// Live log output stream.
    LogStream {
        source: Option<String>,
        tail: Option<u32>,
    },
    /// Error/warning problem list.
    ProblemList {
        source: Option<String>,
    },

    // ── App data views promoted from tier 4 (0.11) ─────────────────

    /// Data-bound list view (::list).
    /// `display` is one of: "card", "table", "compact".
    List {
        source: String,
        display: String,
        item_template: String,
        /// Filterable field names declared on the list.
        filters: Vec<String>,
        sort_field: Option<String>,
        sort_descending: bool,
        preload: bool,
        /// Stream-seam event name the list live-updates on (0.12).
        stream: Option<String>,
        /// Primary row-select action (0.12).
        on_select: Option<NativeAction>,
    },
    /// Kanban board with cards grouped into columns (::board).
    Board {
        source: String,
        columns: Vec<String>,
        card_template: Option<String>,
        preload: bool,
    },
    /// Filter controls for data views (::filter-bar).
    FilterBar {
        target_selector: String,
        fields: Vec<NativeFilterField>,
    },
    /// Search input with typeahead results (::search).
    Search {
        source: String,
        placeholder: Option<String>,
    },

    // ── Messages/Contacts vocabulary (0.12) ────────────────────────

    /// Recipient picker (::recipient-picker) — choose one or more entries
    /// from a data source and submit the selection (group compose).
    /// `mode` is one of: "single", "multi".
    RecipientPicker {
        source: String,
        mode: String,
        on_submit: Option<NativeAction>,
    },
    /// Platform-conditional QR block (::qr) — show-my-code or scan.
    /// `mode` is one of: "show", "scan"; `on_resolve` fires with the
    /// resolved payload after a successful scan/exchange.
    Qr {
        mode: String,
        on_resolve: Option<NativeAction>,
    },

    // ── Wavesite site-format variants (7 new) ──────────────────────

    /// Site-level configuration block (::site).
    ///
    /// Flattens the `{key: value}` properties vec into a handful of
    /// well-known fields used for native theming + chrome. Any additional
    /// keys live in `extras` as `"{key}={value}"` strings.
    Site {
        name: Option<String>,
        description: Option<String>,
        accent: Option<String>,
        font: Option<String>,
        domain: Option<String>,
        extras: Vec<String>,
    },

    /// Single-page-app style page container (::page).
    ///
    /// `children` holds the parsed body of the page; the native renderer
    /// walks them recursively. The web renderer maps one `::page` to one
    /// route; native renderers can either render all pages stacked and
    /// scroll between them, or show one at a time.
    Page {
        route: String,
        title: Option<String>,
        layout: Option<String>,
        children: Vec<NativeBlock>,
    },

    /// Navigation bar (::nav) with logo + labelled links.
    Nav {
        logo: Option<String>,
        items: Vec<NativeNavItem>,
    },

    /// Hero image (::hero-image) — full-bleed illustrative image.
    HeroImage {
        src: String,
        alt: Option<String>,
    },

    /// Footer (::footer) with link sections, copyright, social icons.
    Footer {
        copyright: Option<String>,
        sections: Vec<NativeFooterSection>,
        social: Vec<NativeSocialLink>,
    },

    /// External embed (::embed) — map / video / audio / generic iframe.
    /// `embed_type` is one of: "map", "video", "audio", "generic".
    Embed {
        src: String,
        title: Option<String>,
        embed_type: String,
    },

    /// Pricing comparison table (::pricing-table).
    PricingTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },

    /// Product/pricing card (::product-card) — title, optional subtitle and
    /// badge, body prose, feature bullets, and an optional CTA.
    ProductCard {
        title: String,
        subtitle: Option<String>,
        badge: Option<String>,
        badge_color: Option<String>,
        body: String,
        features: Vec<String>,
        cta_label: Option<String>,
        cta_href: Option<String>,
    },

    /// Chart (::chart). `chart_type` is the chart kind (line/bar/pie/…),
    /// `source` names the data series for live-data mount points. `scene`
    /// carries the typed chart geometry for blocks with an inline dataset
    /// (same layout math as the web SVG); source-only charts stay `None`
    /// and keep the labelled-preview path.
    Chart {
        chart_type: String,
        source: String,
        period: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scene: Option<NativeDiagramScene>,
    },

    /// Compact navigable list row (::row) — icon + title + description, an
    /// optional link target, and a `state` of "default"/"loading"/"empty".
    /// `actions` (0.12) carries the per-row labelled action seam
    /// (contact rows, accept/deny request rows).
    Row {
        icon: String,
        title: String,
        description: String,
        href: Option<String>,
        state: String,
        /// Avatar spec (0.17): initials text or "group" for the users
        /// glyph; `auto` is already derived to initials at parse.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar: Option<String>,
        /// Right-side bucketed relative-time meta (0.17).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rtime: Option<String>,
        /// Unread count pill (0.17); replaces the dot when present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unread_count: Option<u32>,
        /// Labelled per-row actions, typed through the action grammar (0.12).
        actions: Vec<NativeRowAction>,
    },

    /// Rich entity card (::info-card / ::infocard) — an intent badge, title +
    /// subtitle, a summary line, an optional image, and EITHER numbered steps
    /// OR a label/value fact list. `state` is "default"/"loading"/"empty".
    InfoCard {
        intent: String,
        title: String,
        subtitle: String,
        summary: String,
        image: Option<String>,
        facts: Vec<NativeInfoFact>,
        steps: Vec<String>,
        state: String,
    },

    /// Diagram (::diagram) — `diagram_type` is e.g. "architecture"/"erd".
    /// `scene` carries the laid-out geometry (same layout the web SVG is
    /// serialized from) so native clients draw typed shapes; it is `None`
    /// when the DSL fails to parse, and the raw `content` remains for the
    /// titled-card fallback either way.
    Diagram {
        diagram_type: String,
        title: Option<String>,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scene: Option<NativeDiagramScene>,
    },

    // ── FFI-hole closure (0.11): tier-1–3 kinds that previously degraded ──

    /// Full-width banner strip (::banner) — headline + subtitle + action
    /// buttons over free-form body content. `anchor_id` is the optional
    /// in-page anchor (`#contact`).
    Banner {
        headline: Option<String>,
        subtitle: Option<String>,
        anchor_id: Option<String>,
        buttons: Vec<NativeHeroButton>,
        content: String,
    },

    /// A single reference definition (::cite). Renders as nothing or as a
    /// compact reference chip; the formatted entry is resolved in Rust with
    /// the document's active citation style so clients never reimplement
    /// citation formatting.
    Cite {
        key: String,
        formatted: String,
    },

    /// Rendered reference list (::bibliography / ::references). Entries are
    /// pre-formatted in Rust (same string on every platform) in the active
    /// citation style, numbered/ordered per that style's rules.
    Bibliography {
        heading: String,
        entries: Vec<NativeReferenceEntry>,
    },

    /// Access-code card (::gate) — password field + submit button. The
    /// native app controls submission (like `Form`); `action` names the
    /// POST target for the client to bind.
    Gate {
        title: Option<String>,
        subtitle: Option<String>,
        action: String,
        field_label: Option<String>,
        submit_label: Option<String>,
        error: Option<String>,
    },

    /// Grid of product link-cards (::product-grid), optionally grouped.
    /// `tiles` selects the full-bleed promo-tile rendering.
    ProductGrid {
        tiles: bool,
        /// Block-level per-size-class column count (schema v5). Per-group
        /// `cols` on [`NativeProductGroup`] still wins locally.
        cols: Option<NativePerClassU32>,
        groups: Vec<NativeProductGroup>,
    },

    /// Card grid for a blog/news/events index (::post-grid).
    PostGrid {
        title: Option<String>,
        subtitle: Option<String>,
        items: Vec<NativePostItem>,
    },

    /// Presentation slide (::slide) rendered outside the deck renderer.
    /// `layout` is the SlideLayout css-class token ("cover", "bullets", …).
    /// Recursive like `SectionContainer` (UniFFI boxes recursive enums).
    Slide {
        layout: String,
        kicker: Option<String>,
        notes: Option<String>,
        children: Vec<NativeBlock>,
    },

    /// Resizable side-by-side layout (::split-pane) with left/right planes.
    /// `back_label` / `back_action` drive the small-screen back control in
    /// the right plane. Recursive like `SectionContainer` and `Slide`
    /// (UniFFI boxes recursive enums).
    SplitPane {
        ratio: String,
        back_label: Option<String>,
        back_action: Option<String>,
        left: Vec<NativeBlock>,
        right: Vec<NativeBlock>,
    },
}

// ═══════════════════════════════════════════════════════════════════════
// Supporting record types — all simple, UniFFI-friendly
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeTaskItem {
    pub done: bool,
    pub text: String,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeTabPanel {
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeColumnContent {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeFaqItem {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeInfoFact {
    pub label: String,
    pub value: String,
}

/// A labelled action on a native `Row` (0.12) — the per-row dispatch seam
/// (accept/deny request rows, contact-row verbs), typed through
/// [`parse_native_action`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeRowAction {
    pub label: String,
    pub action: NativeAction,
}

/// One authored message child of a chat thread (0.17).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeChatMessage {
    /// "own" (outgoing) or "them" (incoming).
    pub side: String,
    pub sender: Option<String>,
    /// Display timestamp, rendered inside the bubble.
    pub timestamp: Option<String>,
    pub text: String,
    /// Read-only reaction pills (0.17, ruling D-3).
    pub reactions: Vec<NativeChatReaction>,
}

/// A read-only reaction pill on a chat message (0.17, ruling D-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeChatReaction {
    pub label: String,
    pub count: Option<u32>,
    pub mine: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeFeatureCard {
    pub title: String,
    pub icon: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeStepItem {
    pub title: String,
    pub time: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeStatItem {
    pub value: String,
    pub label: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeTocEntry {
    pub text: String,
    pub id: String,
    pub level: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeBeforeAfterItem {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativePipelineStep {
    pub label: String,
    pub description: Option<String>,
}

/// A single field in a native form.
/// `field_type` is one of: "text", "email", "tel", "date", "number", "select", "textarea".
/// `options` is non-empty only when `field_type` is "select".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeFormField {
    pub label: String,
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub placeholder: Option<String>,
    pub options: Vec<String>,
}

/// A single image item in a native gallery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeGalleryItem {
    pub src: String,
    pub caption: Option<String>,
    pub alt: Option<String>,
    pub category: Option<String>,
}

/// A value that varies per size class, crossing the FFI as a resolved
/// triple (schema v5). Clients pick with the class the host resolved from
/// [`crate::resolve::resolve_size_class`] — they never re-derive
/// breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativePerClassU32 {
    pub mobile: u32,
    pub tablet: u32,
    pub desktop: u32,
}

impl From<PerClass<u32>> for NativePerClassU32 {
    fn from(p: PerClass<u32>) -> Self {
        NativePerClassU32 {
            mobile: p.mobile,
            tablet: p.tablet,
            desktop: p.desktop,
        }
    }
}

/// The resolved `mobile=`/`tablet=`/`desktop=` navigation modes of an
/// `::app-shell[layout=adaptive]` (schema v5). Values are the
/// [`crate::types::AdaptiveMode`] tokens: "tabs", "rail", "sidebar".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeAdaptiveLayout {
    pub mobile: String,
    pub tablet: String,
    pub desktop: String,
}

/// The class-conditional visibility of a chrome block (schema v5).
/// `classes` empty means "every class"; `min_class` is `None` when
/// unconstrained.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeClassGate {
    pub classes: Vec<String>,
    pub min_class: Option<String>,
}

fn class_gate(classes: &Option<Vec<SizeClass>>, min_class: &Option<SizeClass>) -> NativeClassGate {
    NativeClassGate {
        classes: classes
            .as_ref()
            .map(|cs| cs.iter().map(|c| c.as_str().to_string()).collect())
            .unwrap_or_default(),
        min_class: min_class.map(|c| c.as_str().to_string()),
    }
}

/// A tab item within a native `TabBar`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeTabBarItem {
    pub id: String,
    pub label: String,
    /// Optional icon token (SF-symbol-ish, e.g. "doc.text"); clients map it
    /// to an SFSymbol (iOS/macOS) or Material icon (Android).
    pub icon: Option<String>,
    /// Right-side unread dot. Reached HTML from 0.13 but died at the FFI
    /// until schema v5.
    pub unread: bool,
    /// Semantic role token, carried verbatim. New at schema v5 — the second
    /// half of the same FFI hole.
    pub role: Option<String>,
}

/// An item within a native `Toolbar`.
/// Discriminated by `kind`: "button", "separator", "badge", "dropdown", "text".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeToolbarItem {
    pub kind: String,
    pub label: Option<String>,
    pub action: Option<NativeAction>,
    pub icon: Option<String>,
    pub style: Option<String>,
    pub disabled: bool,
    pub value: Option<String>,
    pub color: Option<String>,
    pub options: Option<String>,
}

/// A command within a native `CommandPalette`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeCommandItem {
    pub label: String,
    pub description: Option<String>,
    pub action: Option<NativeAction>,
    pub icon: Option<String>,
    pub group: Option<String>,
}

/// A typed action parsed from a spec `on*=` / `action=` string (0.11).
///
/// The minimal action grammar — `verb:target[:payload]`, bare name =
/// `invoke` — makes mutations spec-expressible without clients parsing
/// strings themselves:
///
/// - `open:/docs/123` — navigate to a route. verb `open`, target the route.
/// - `invoke:open_doc` (or bare `open_doc`) — call a named binding-registry
///   action. verb `invoke`, target the registry name.
/// - `mutate:tasks.set_stage:stage` — run a named mutation against a data
///   source; the optional third segment names the payload key the client
///   supplies at call time.
///
/// `raw` always preserves the exact authored string, so registries keyed by
/// the legacy bare names keep working unchanged. Anything richer than these
/// three verbs is deliberately deferred until spec-driven surfaces prove the
/// need (V-C evidence rule).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeAction {
    /// One of: "open", "invoke", "mutate".
    pub verb: String,
    /// Route (open), registry action name (invoke), or mutation name (mutate).
    pub target: String,
    /// Payload key for `mutate` actions; None otherwise.
    pub payload: Option<String>,
    /// The exact authored action string.
    pub raw: String,
}

/// Parse an authored action string into a [`NativeAction`] per the minimal
/// grammar above. Never fails: unknown shapes become `invoke` on the whole
/// string, so authoring mistakes degrade to a registry miss (caught by the
/// build-step validator), not a parse crash.
pub fn parse_native_action(raw: &str) -> NativeAction {
    let raw_trim = raw.trim();
    let (verb, rest) = match raw_trim.split_once(':') {
        Some((v @ ("open" | "invoke" | "mutate"), rest)) if !rest.is_empty() => (v, rest),
        _ => ("invoke", raw_trim),
    };
    let (target, payload) = if verb == "mutate" {
        match rest.rsplit_once(':') {
            Some((t, p)) if !t.is_empty() && !p.is_empty() => (t, Some(p.to_string())),
            _ => (rest, None),
        }
    } else {
        (rest, None)
    };
    NativeAction {
        verb: verb.to_string(),
        target: target.to_string(),
        payload,
        raw: raw_trim.to_string(),
    }
}

/// A single formatted entry within a native `Bibliography`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeReferenceEntry {
    pub key: String,
    pub formatted: String,
}

/// A labelled group of product cards within a native `ProductGrid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeProductGroup {
    pub label: Option<String>,
    /// Max columns for this group's tile row (clamped 1–3 upstream); None →
    /// the 2-column default.
    pub cols: Option<u32>,
    pub items: Vec<NativeProductItem>,
}

/// A single product link-card within a native `ProductGrid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeProductItem {
    pub name: String,
    pub href: String,
    pub emblem: Option<String>,
    pub tagline: Option<String>,
    pub cta1_label: Option<String>,
    pub cta1_href: Option<String>,
    pub cta2_label: Option<String>,
    pub cta2_href: Option<String>,
    /// Tile background spec, raw author value ("image:…", "color:…",
    /// "gradient:…", "transparent", optional trailing " dark").
    pub bg: Option<String>,
}

/// A single card within a native `PostGrid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativePostItem {
    pub title: String,
    pub href: String,
    pub meta: Option<String>,
    pub excerpt: Option<String>,
    pub image: Option<String>,
    pub external: bool,
}

/// A single filter control within a native `FilterBar`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeFilterField {
    pub label: String,
    pub name: String,
    pub options: Vec<String>,
}

/// A step within a native `Progress` indicator.
/// `status` is one of: "done", "active", "pending".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeProgressStep {
    pub label: String,
    pub status: String,
}

/// A link within a native `Nav` or `Footer` section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeNavItem {
    pub label: String,
    pub href: String,
    pub icon: Option<String>,
}

/// A single link section within a native `Footer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeFooterSection {
    pub heading: String,
    pub links: Vec<NativeNavItem>,
}

/// A single social-media link within a native `Footer`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeSocialLink {
    pub platform: String,
    pub href: String,
}

/// A single action button within a native `Hero`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeHeroButton {
    pub label: String,
    pub href: String,
    pub primary: bool,
}

// ═══════════════════════════════════════════════════════════════════════
// NativeTheme + NativeDoc — the resolved-theme projection over FFI
// ═══════════════════════════════════════════════════════════════════════

/// The native analog of the `--ws-*` CSS custom properties plus the derived
/// accent colors: style-pack tokens, fonts, and WCAG contrast math resolved
/// ONCE in Rust ([`crate::resolve`]) and shipped to Swift/Kotlin as data.
/// Views read these instead of hardcoding styling — pack semantics are never
/// reimplemented on the native side, so native rendering cannot drift from
/// the pack (the 0025 property, extended to apps).
///
/// Radii / border width are numeric points parsed from the px tokens; CSS
/// recipe strings (shadow, texture, hero background) cross as-is and the
/// native side maps the recipes it knows (pill radii arrive as 999 —
/// clamp to a capsule).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeTheme {
    /// Resolved style-pack key ("surf", "comic", …).
    pub pack_id: String,
    /// Brand accent (hex).
    pub accent: String,
    /// WCAG-compliant text color for content ON the accent (hex).
    pub on_accent: String,
    /// Accent adjusted to read as TEXT at AA on light surfaces (hex).
    pub accent_ink_light: String,
    /// Accent adjusted to read as TEXT at AA on dark surfaces (hex).
    pub accent_ink_dark: String,
    /// Display/heading font stack (CSS names, comma-separated), if set.
    pub font_display: Option<String>,
    /// Body font stack, if set.
    pub font_body: Option<String>,
    /// Card corner radius in points (hero, feature/stat/testimonial cards).
    pub radius_card: f64,
    /// Button/CTA corner radius in points (999 = pill/capsule).
    pub radius_btn: f64,
    /// Chip/badge corner radius in points (999 = pill/capsule).
    pub radius_chip: f64,
    /// Image corner radius in points.
    pub radius_img: f64,
    /// Card/badge border width in points.
    pub border_w: f64,
    /// Border style ("solid", …).
    pub border_style: String,
    /// Card shadow recipe (CSS string; "none" or e.g. Comic's hard offset).
    pub shadow: String,
    /// Hover/lift shadow recipe (CSS string).
    pub shadow_hover: String,
    /// Page background texture recipe (CSS string; "none" when absent).
    pub bg_texture: String,
    /// Hero surface recipe (CSS string; gradient or flat accent).
    pub hero_bg: String,

    // ── SS-1 additive component tokens (0.9.3) ─────────────────────────
    // Purely additive projection of the grown WsTokens contract; schema
    // version stays 1. Radii/pad cross as points (px_value, with the
    // identity var() chains resolved to the same fallbacks the CSS uses);
    // bg / transform / drawer-link tokens cross as CSS strings — the
    // native side maps the recipes it knows.
    /// Hero action-button corner in points (999 = pill/capsule).
    pub hero_btn_radius: f64,
    /// ::banner action-button corner in points.
    pub banner_btn_radius: f64,
    /// Standalone CTA corner in points (999 = pill/capsule).
    pub cta_radius: f64,
    /// ::form submit-button corner in points.
    pub form_submit_radius: f64,
    /// App-control corner in points (booking/store controls; identity 8).
    pub control_radius: f64,
    /// Feature-card corner in points.
    pub feature_card_radius: f64,
    /// Feature-card padding in points.
    pub feature_card_pad: f64,
    /// Feature-card hover transform recipe (CSS string; "none" to disable).
    pub feature_card_hover_transform: String,
    /// Feature-card surface recipe (CSS string).
    pub feature_card_bg: String,
    /// ::product-grid tile-surface fill recipe (CSS string).
    pub tile_surface_bg: String,
    /// ::post-grid card surface recipe (CSS string).
    pub post_card_bg: String,
    /// ::post-grid card corner in points.
    pub post_card_radius: f64,
    /// ::product-grid row-card surface recipe (CSS string).
    pub pg_card_bg: String,
    /// ::product-grid row-card corner in points.
    pub pg_card_radius: f64,
    /// ::product-grid tile corner in points (0 = square, the spec default).
    pub pg_tile_radius: f64,
    /// ::details disclosure surface recipe (CSS string).
    pub details_bg: String,
    /// ::details corner in points.
    pub details_radius: f64,
    /// ::form input fill recipe (CSS string).
    pub form_input_bg: String,
    /// Doc-page sheet surface recipe (CSS string).
    pub doc_page_bg: String,
    /// Doc-page sheet corner in points.
    pub doc_page_radius: f64,
    /// Shell drawer link font size (CSS length string, e.g. "0.9375rem").
    pub drawer_link_size: String,
    /// Shell drawer link font weight (CSS string, e.g. "500").
    pub drawer_link_weight: String,
}

/// Parse a CSS px length ("14px" → 14.0). Non-px values fall back to the
/// given default so a future token change degrades gracefully on old apps.
fn px_value(css: &str, fallback: f64) -> f64 {
    css.trim()
        .strip_suffix("px")
        .and_then(|n| n.trim().parse::<f64>().ok())
        .unwrap_or(fallback)
}

impl From<&crate::resolve::ResolvedTheme> for NativeTheme {
    fn from(t: &crate::resolve::ResolvedTheme) -> Self {
        let radius_card = px_value(t.tokens.radius_card, 14.0);
        let radius_btn = px_value(t.tokens.radius_btn, 999.0);
        NativeTheme {
            pack_id: t.pack_id.clone(),
            accent: t.accent.clone(),
            on_accent: t.on_accent.clone(),
            accent_ink_light: t.accent_ink_light.clone(),
            accent_ink_dark: t.accent_ink_dark.clone(),
            font_display: t.font_display.clone(),
            font_body: t.font_body.clone(),
            radius_card,
            radius_btn,
            radius_chip: px_value(t.tokens.radius_chip, 999.0),
            radius_img: px_value(t.tokens.radius_img, 10.0),
            border_w: px_value(t.tokens.border_w, 1.0),
            border_style: t.tokens.border_style.to_string(),
            shadow: t.tokens.shadow.to_string(),
            shadow_hover: t.tokens.shadow_hover.to_string(),
            bg_texture: t.tokens.bg_texture.to_string(),
            hero_bg: t.tokens.hero_bg.to_string(),
            // SS-1 (0.9.3): fallbacks mirror the CSS identity chains, so the
            // identity var() strings resolve to the pack's own base values
            // (hero/cta chain to radius-btn, card corners to radius-card,
            // forms to the 2px --radius-sm chain, app controls to 8px).
            hero_btn_radius: px_value(t.tokens.hero_btn_radius, radius_btn),
            banner_btn_radius: px_value(t.tokens.banner_btn_radius, 2.0),
            cta_radius: px_value(t.tokens.cta_radius, radius_btn),
            form_submit_radius: px_value(
                t.tokens.form_submit_radius,
                px_value(t.tokens.control_radius, 2.0),
            ),
            control_radius: px_value(t.tokens.control_radius, 8.0),
            feature_card_radius: px_value(t.tokens.feature_card_radius, radius_card),
            feature_card_pad: px_value(t.tokens.feature_card_pad, 24.0),
            feature_card_hover_transform: t.tokens.feature_card_hover_transform.to_string(),
            feature_card_bg: t.tokens.feature_card_bg.to_string(),
            tile_surface_bg: t.tokens.tile_surface_bg.to_string(),
            post_card_bg: t.tokens.post_card_bg.to_string(),
            post_card_radius: px_value(t.tokens.post_card_radius, radius_card),
            pg_card_bg: t.tokens.pg_card_bg.to_string(),
            pg_card_radius: px_value(t.tokens.pg_card_radius, 20.0),
            pg_tile_radius: px_value(t.tokens.pg_tile_radius, 0.0),
            details_bg: t.tokens.details_bg.to_string(),
            details_radius: px_value(t.tokens.details_radius, 2.0),
            form_input_bg: t.tokens.form_input_bg.to_string(),
            doc_page_bg: t.tokens.doc_page_bg.to_string(),
            doc_page_radius: px_value(t.tokens.doc_page_radius, radius_card),
            drawer_link_size: t.tokens.drawer_link_size.to_string(),
            drawer_link_weight: t.tokens.drawer_link_weight.to_string(),
        }
    }
}

/// Schema version carried by every [`NativeDoc`]. Bump when the NativeBlock
/// or NativeTheme shape changes incompatibly; older app binaries render
/// unknown future content via the markdown degradation strings.
///
/// v2 (0.11): real chrome nesting (appShell/drawer children populated),
/// list/board/filterBar/search promoted native, TabBarItem.icon, the seven
/// former FFI-hole kinds (banner/cite/bibliography/gate/productGrid/postGrid/
/// slide) structured, and `on*=`/`action=` strings typed as `NativeAction`.
///
/// v3 (0.12) — the Messages/Contacts vocabulary, eight additions:
/// 1. `List.stream` — stream-seam event name for live list updates.
/// 2. `List.on_select` — typed primary row-select action.
/// 3. `ChatThread.on_react` + `ChatThread.on_doc_open` — reaction/tapback
///    and doc-chip open seams, typed.
/// 4. `Row.actions` — labelled per-row typed actions (`NativeRowAction`).
/// 5. `Toolbar.title` + `Toolbar.title_source` — static and source-bound
///    toolbar titles.
/// 6. `RecipientPicker` — new kind (source, single/multi mode, on_submit).
/// 7. `Qr` — new platform-conditional kind (show/scan mode, on_resolve).
/// 8. Bare registry names accepted in `List`/`Search` `source=` (parse-side;
///    previously lifted as empty strings).
///
/// v4 (0.17) — the Messages mockup-fidelity round, four additions:
/// 1. `ChatThread.messages` — authored message children (side, sender,
///    in-bubble timestamp, text, read-only reaction pills).
/// 2. `ChipInput` — new kind (the compose "To:" line: label, removable
///    chips, inline filter input, on_change seam).
/// 3. `Row.avatar` + `Row.rtime` + `Row.unread_count` — roster-row
///    initials/group avatar, right-side time meta, unread count pill.
/// 4. `NativeChatMessage`/`NativeChatReaction` — new child records.
/// v5 (0.18) — the size-class axis, plus the FFI holes it exposed:
/// 1. `NativePerClassU32` — a resolved mobile/tablet/desktop triple, now
///    carried by `Sidebar.width`, `Drawer.width` and `TabContent.width`.
/// 2. `NativeClassGate` on `Sidebar`/`Panel`/`TabContent`/`Drawer` — the
///    `classes=` / `min-class=` conditional (`Panel.desktop_only` is the
///    deprecated alias and stays only for source fidelity).
/// 3. `AppShell.adaptive` — the resolved `layout=adaptive` navigation modes.
/// 4. `NativeTabBarItem.unread` + `.role`, and `TabContent.width`/`.align`:
///    three values that reached HTML but died at the FFI before v5.
/// 5. `size_class_tablet_min()` / `size_class_desktop_min()` /
///    `resolve_size_class()` exported so clients share one breakpoint table.
pub const NATIVE_DOC_SCHEMA_VERSION: u32 = 5;

/// A parsed document plus its resolved theme — the unit that crosses the
/// FFI for themed native rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeDoc {
    /// [`NATIVE_DOC_SCHEMA_VERSION`] at build time of the producing binary.
    pub schema_version: u32,
    /// Resolved style-pack/font/contrast values (see [`NativeTheme`]).
    pub theme: NativeTheme,
    /// The block tree.
    pub blocks: Vec<NativeBlock>,
}

// ═══════════════════════════════════════════════════════════════════════
// Conversion functions
// ═══════════════════════════════════════════════════════════════════════

/// Convert a parsed SurfDoc into a Vec<NativeBlock> for native rendering.
pub fn to_native_blocks(doc: &SurfDoc) -> Vec<NativeBlock> {
    let _cite_scope = crate::citation::install_context(crate::citation::build_context(
        &doc.blocks,
        doc.front_matter.as_ref().and_then(|fm| fm.format),
    ));
    doc.blocks.iter().flat_map(|b| convert_block_flat(b, 0)).collect()
}

/// Convert a list of child blocks, expanding GFM pipe tables that live inside
/// `Block::Markdown` content into standalone `NativeBlock::DataTable` blocks.
/// Used by every container variant so tables render natively at any nesting.
fn convert_children(children: &[Block], depth: u32) -> Vec<NativeBlock> {
    children
        .iter()
        .flat_map(|c| convert_block_flat(c, depth))
        .collect()
}

/// Convert a single block, but allow a `Block::Markdown` to expand into
/// multiple native blocks when its content embeds one or more GFM pipe tables
/// (markdown text → DataTable → markdown text …). Every other block converts
/// 1:1 via [`convert_block`].
fn convert_block_flat(block: &Block, depth: u32) -> Vec<NativeBlock> {
    match block {
        Block::Markdown { content, .. } => expand_markdown_tables(content),
        other => vec![convert_block(other, depth)],
    }
}

/// Split a markdown string into a sequence of native blocks, lifting any GFM
/// pipe tables out into `NativeBlock::DataTable`. Surrounding prose stays in
/// `NativeBlock::Markdown` blocks. When the content holds no table, returns a
/// single `Markdown` block (identical to the previous behavior).
///
/// A GFM pipe table is: a header row (`| a | b |` or `a | b`), a delimiter row
/// (`|---|:--:|` — dashes with optional leading/trailing colons for alignment),
/// then zero or more data rows. The table ends at a blank line, a non-table
/// line, or EOF. Ragged rows are padded/truncated to the header column count.
/// Escaped `\|` inside a cell is treated as a literal pipe, not a separator.
pub(crate) fn expand_markdown_tables(content: &str) -> Vec<NativeBlock> {
    let lines: Vec<&str> = content.split('\n').collect();

    // Fast path: no GFM table present → return the content verbatim, byte-for-
    // byte identical to the previous 1:1 behavior (no prose re-trimming).
    let has_table = lines
        .windows(2)
        .any(|w| is_table_row(w[0]) && is_delimiter_row(w[1]));
    if !has_table {
        return vec![NativeBlock::Markdown {
            content: content.to_string(),
        }];
    }

    let mut out: Vec<NativeBlock> = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    let mut i = 0;

    let flush_prose = |prose: &mut Vec<&str>, out: &mut Vec<NativeBlock>| {
        if prose.iter().any(|l| !l.trim().is_empty()) {
            // Trim leading/trailing blank lines for a tight prose block.
            let start = prose.iter().position(|l| !l.trim().is_empty()).unwrap();
            let end = prose.iter().rposition(|l| !l.trim().is_empty()).unwrap();
            out.push(NativeBlock::Markdown {
                content: prose[start..=end].join("\n"),
            });
        }
        prose.clear();
    };

    while i < lines.len() {
        // A table requires a header line followed by a delimiter line.
        if i + 1 < lines.len() && is_table_row(lines[i]) && is_delimiter_row(lines[i + 1]) {
            let headers = split_table_cells(lines[i]);
            let ncols = headers.len();
            // Collect data rows until blank/non-table/EOF.
            let mut rows: Vec<Vec<String>> = Vec::new();
            let mut j = i + 2;
            while j < lines.len() && is_table_row(lines[j]) {
                let mut cells = split_table_cells(lines[j]);
                // Pad/truncate ragged rows to the header column count.
                if cells.len() < ncols {
                    cells.resize(ncols, String::new());
                } else {
                    cells.truncate(ncols);
                }
                rows.push(cells);
                j += 1;
            }
            flush_prose(&mut prose, &mut out);
            out.push(NativeBlock::DataTable {
                headers,
                rows,
                sortable: false,
            });
            i = j;
            continue;
        }
        prose.push(lines[i]);
        i += 1;
    }
    flush_prose(&mut prose, &mut out);

    if out.is_empty() {
        // No real content (e.g. all-whitespace) — preserve the original string
        // so spans/round-trips behave as before.
        out.push(NativeBlock::Markdown {
            content: content.to_string(),
        });
    }
    out
}

/// True if a line looks like a pipe-table row: after trimming it contains at
/// least one unescaped `|` and is not a fenced-code or blank line.
fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    // Must contain at least one unescaped pipe.
    let mut escaped = false;
    for c in t.chars() {
        match c {
            '\\' => escaped = !escaped,
            '|' if !escaped => return true,
            _ => escaped = false,
        }
    }
    false
}

/// True if a line is a GFM delimiter row: each cell is dashes with optional
/// leading/trailing colons (`---`, `:--`, `--:`, `:-:`), at least one cell.
fn is_delimiter_row(line: &str) -> bool {
    let t = line.trim();
    if !is_table_row(t) {
        return false;
    }
    let cells = split_table_cells(t);
    if cells.is_empty() {
        return false;
    }
    cells.iter().all(|cell| {
        let c = cell.trim();
        if c.is_empty() {
            return false;
        }
        let inner = c.trim_start_matches(':').trim_end_matches(':');
        !inner.is_empty() && inner.chars().all(|ch| ch == '-')
    })
}

/// Split a pipe-table row into trimmed cell strings, honoring `\|` escapes
/// (rendered as a literal `|`) and dropping the optional leading/trailing pipe.
fn split_table_cells(line: &str) -> Vec<String> {
    let mut t = line.trim();
    // Drop a single leading/trailing pipe (the optional GFM border pipes).
    if let Some(stripped) = t.strip_prefix('|') {
        t = stripped;
    }
    if let Some(stripped) = t.strip_suffix('|') {
        // Only strip a trailing pipe that isn't escaped.
        if !t.ends_with("\\|") {
            t = stripped;
        }
    }

    let mut cells: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for c in t.chars() {
        if escaped {
            // Keep `\|` as a literal pipe; other escapes keep both chars.
            if c == '|' {
                cur.push('|');
            } else {
                cur.push('\\');
                cur.push(c);
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    if escaped {
        cur.push('\\');
    }
    cells.push(cur.trim().to_string());
    cells
}

/// The `state` attribute string shared by ::row and ::info-card.
fn row_state_str(state: &RowState) -> &'static str {
    match state {
        RowState::Default => "default",
        RowState::Loading => "loading",
        RowState::Empty => "empty",
    }
}

fn convert_block(block: &Block, depth: u32) -> NativeBlock {
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

        // Native gets the laid-out geometry scene (typed shapes, same
        // layout the web SVG is serialized from) plus the raw DSL for the
        // titled-card fallback; `scene` is None when the DSL fails to parse.
        Block::Diagram {
            diagram_type,
            title,
            content,
            ..
        } => NativeBlock::Diagram {
            diagram_type: diagram_type.clone(),
            title: title.clone(),
            content: content.clone(),
            scene: crate::diagram::native_scene(diagram_type, content, title.as_deref()),
        },

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
        } => NativeBlock::ProductCard {
            title: title.clone(),
            subtitle: subtitle.clone(),
            badge: badge.clone(),
            badge_color: badge_color.clone(),
            body: body.clone(),
            features: features.clone(),
            cta_label: cta_label.clone(),
            cta_href: cta_href.clone(),
        },

        Block::Chart {
            chart_type,
            source,
            period,
            title,
            data,
            ..
        } => NativeBlock::Chart {
            chart_type: crate::render_html::chart_type_str(*chart_type).to_string(),
            source: source.clone(),
            period: period.clone(),
            // Inline datasets carry their laid-out geometry scene (the same
            // layout math the web SVG uses); source-only charts stay `None`
            // and keep the live-data mount-point path.
            scene: data
                .as_ref()
                .map(|d| crate::chart::build_scene(*chart_type, d, title.as_deref())),
        },

        Block::Row {
            icon,
            title,
            description,
            href,
            state,
            avatar,
            rtime,
            unread_count,
            actions,
            ..
        } => NativeBlock::Row {
            icon: icon.clone(),
            title: title.clone(),
            description: description.clone(),
            href: href.clone(),
            state: row_state_str(state).to_string(),
            avatar: avatar.clone(),
            rtime: rtime.clone(),
            unread_count: *unread_count,
            actions: actions
                .iter()
                .map(|a| NativeRowAction {
                    label: a.label.clone(),
                    action: parse_native_action(&a.action),
                })
                .collect(),
        },

        Block::InfoCard {
            intent,
            title,
            subtitle,
            summary,
            image,
            facts,
            steps,
            state,
            ..
        } => NativeBlock::InfoCard {
            intent: intent.clone(),
            title: title.clone(),
            subtitle: subtitle.clone(),
            summary: summary.clone(),
            image: image.clone(),
            facts: facts
                .iter()
                .map(|f| NativeInfoFact {
                    label: f[0].clone(),
                    value: f[1].clone(),
                })
                .collect(),
            steps: steps.clone(),
            state: row_state_str(state).to_string(),
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
            align,
            image,
            buttons,
            content,
            ..
        } => NativeBlock::Hero {
            headline: headline.clone(),
            subtitle: subtitle.clone(),
            badge: badge.clone(),
            align: align.clone(),
            image: image.clone(),
            buttons: buttons
                .iter()
                .map(|b| NativeHeroButton {
                    label: b.label.clone(),
                    href: b.href.clone(),
                    primary: b.primary,
                })
                .collect(),
            content: content.clone(),
        },

        Block::Features { cards, cols, .. } => NativeBlock::Features {
            cols: cols.map(NativePerClassU32::from),
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

        // ── New native variants: Form, Gallery, SectionContainer ────

        Block::Form {
            fields,
            submit_label,
            ..
        } => NativeBlock::Form {
            fields: fields
                .iter()
                .map(|f| NativeFormField {
                    label: f.label.clone(),
                    name: f.name.clone(),
                    field_type: form_field_type_str(f.field_type),
                    required: f.required,
                    placeholder: f.placeholder.clone(),
                    options: f.options.clone(),
                })
                .collect(),
            submit_label: submit_label
                .clone()
                .unwrap_or_else(|| "Submit".to_string()),
        },

        Block::Gallery {
            items, columns, ..
        } => NativeBlock::Gallery {
            items: items
                .iter()
                .map(|i| NativeGalleryItem {
                    src: i.src.clone(),
                    caption: i.caption.clone(),
                    alt: i.alt.clone(),
                    category: i.category.clone(),
                })
                .collect(),
            columns: NativePerClassU32::from(columns.unwrap_or_else(|| PerClass::uniform(3))),
        },

        Block::Section {
            bg,
            headline,
            subtitle,
            children,
            ..
        } => {
            if depth >= MAX_SECTION_DEPTH {
                // Depth limit reached — fall back to Markdown
                let md = render_md::render_block(block);
                NativeBlock::Markdown { content: md }
            } else {
                NativeBlock::SectionContainer {
                    bg: bg.clone(),
                    headline: headline.clone(),
                    subtitle: subtitle.clone(),
                    children: convert_children(children, depth + 1),
                }
            }
        }

        // ── Interactive block types: native conversion ─────────────

        // Layout
        Block::AppShell {
            layout,
            adaptive,
            children,
            ..
        } => NativeBlock::AppShell {
            layout: layout.as_str().to_string(),
            adaptive: adaptive.map(|a| NativeAdaptiveLayout {
                mobile: a.mobile.as_str().to_string(),
                tablet: a.tablet.as_str().to_string(),
                desktop: a.desktop.as_str().to_string(),
            }),
            children: convert_children(children, depth + 1),
        },

        Block::Sidebar {
            position,
            collapsible,
            width,
            classes,
            min_class,
            children,
            ..
        } => NativeBlock::Sidebar {
            position: position.clone(),
            collapsible: *collapsible,
            width: width.map(NativePerClassU32::from),
            gate: class_gate(classes, min_class),
            children: convert_children(children, depth + 1),
        },

        Block::Panel {
            position,
            resizable,
            height,
            desktop_only,
            classes,
            min_class,
            children,
            ..
        } => NativeBlock::Panel {
            position: position.clone(),
            resizable: *resizable,
            height: *height,
            desktop_only: *desktop_only,
            gate: class_gate(classes, min_class),
            children: convert_children(children, depth + 1),
        },

        // Navigation
        Block::TabBar { active, items, .. } => NativeBlock::TabBar {
            active: active.clone(),
            items: items
                .iter()
                .map(|i| NativeTabBarItem {
                    id: i.id.clone(),
                    label: i.label.clone(),
                    icon: i.icon.clone(),
                    unread: i.unread,
                    role: i.role.clone(),
                })
                .collect(),
        },

        Block::TabContent {
            tab,
            width,
            align,
            classes,
            min_class,
            children,
            ..
        } => NativeBlock::TabContent {
            tab: tab.clone(),
            width: width.map(NativePerClassU32::from),
            align: align.clone(),
            gate: class_gate(classes, min_class),
            children: convert_children(children, depth + 1),
        },

        Block::Toolbar {
            title,
            title_source,
            items,
            ..
        } => NativeBlock::Toolbar {
            title: title.clone(),
            title_source: title_source.clone(),
            items: items.iter().map(toolbar_item_to_native).collect(),
        },

        // Overlays
        Block::Drawer {
            name,
            position,
            width,
            trigger,
            classes,
            min_class,
            children,
            ..
        } => NativeBlock::Drawer {
            name: name.clone(),
            position: position.clone(),
            width: width.map(NativePerClassU32::from),
            trigger: trigger.clone(),
            gate: class_gate(classes, min_class),
            children: convert_children(children, depth + 1),
        },

        Block::Modal {
            name,
            title,
            children,
            ..
        } => NativeBlock::Modal {
            name: name.clone(),
            title: title.clone(),
            children: convert_children(children, depth + 1),
        },

        // No dedicated NativeBlock variant (no schema bump needed): a
        // segmented-control maps onto TabBar — same id/label single-select
        // shape — until a native round gives it its own variant.
        Block::SegmentedControl { active, segments, .. } => NativeBlock::TabBar {
            active: active.clone(),
            items: segments
                .iter()
                .map(|s| NativeTabBarItem {
                    id: s.id.clone(),
                    label: s.label.clone(),
                    icon: None,
                    unread: false,
                    role: None,
                })
                .collect(),
        },

        // No dedicated NativeBlock variant (no schema bump needed): a
        // dropdown-select degrades to a CommandPalette — same trigger +
        // option-list shape — until a native round gives it its own variant.
        Block::DropdownSelect { label, selected, options, .. } => NativeBlock::CommandPalette {
            trigger: label.clone().or_else(|| selected.clone()),
            items: options
                .iter()
                .map(|o| NativeCommandItem {
                    label: o.label.clone(),
                    description: o.description.clone(),
                    action: o.action.as_deref().map(parse_native_action),
                    icon: o.icon.clone(),
                    group: None,
                })
                .collect(),
        },

        Block::CommandPalette {
            trigger, items, ..
        } => NativeBlock::CommandPalette {
            trigger: trigger.clone(),
            items: items
                .iter()
                .map(|i| NativeCommandItem {
                    label: i.label.clone(),
                    description: i.description.clone(),
                    action: i.action.as_deref().map(parse_native_action),
                    icon: i.icon.clone(),
                    group: i.group.clone(),
                })
                .collect(),
        },

        // Interactive
        Block::CodeEditor {
            lang,
            source,
            line_numbers,
            content,
            ..
        } => NativeBlock::CodeEditor {
            lang: lang.clone(),
            source: source.clone(),
            line_numbers: *line_numbers,
            content: content.clone(),
        },

        Block::BlockEditor { source, .. } => NativeBlock::BlockEditor {
            source: source.clone(),
        },

        Block::Terminal { shell, cwd, .. } => NativeBlock::Terminal {
            shell: shell.clone(),
            cwd: cwd.clone(),
        },

        // Data
        Block::NavTree {
            source,
            on_select,
            on_rename,
            on_delete,
            ..
        } => NativeBlock::NavTree {
            source: source.clone(),
            on_select: on_select.as_deref().map(parse_native_action),
            on_rename: on_rename.as_deref().map(parse_native_action),
            on_delete: on_delete.as_deref().map(parse_native_action),
        },

        Block::Badge { value, color, .. } => NativeBlock::Badge {
            value: value.clone(),
            color: color.clone(),
        },

        Block::SuggestionChips {
            source,
            max,
            dismissible,
            ..
        } => NativeBlock::SuggestionChips {
            source: source.clone(),
            max: *max,
            dismissible: *dismissible,
        },

        Block::ChatThread {
            source,
            on_action,
            on_react,
            on_doc_open,
            messages,
            ..
        } => NativeBlock::ChatThread {
            source: source.clone(),
            on_action: on_action.as_deref().map(parse_native_action),
            on_react: on_react.as_deref().map(parse_native_action),
            on_doc_open: on_doc_open.as_deref().map(parse_native_action),
            messages: messages
                .iter()
                .map(|m| NativeChatMessage {
                    side: m.side.clone(),
                    sender: m.sender.clone(),
                    timestamp: m.timestamp.clone(),
                    text: m.text.clone(),
                    reactions: m
                        .reactions
                        .iter()
                        .map(|r| NativeChatReaction {
                            label: r.label.clone(),
                            count: r.count,
                            mine: r.mine,
                        })
                        .collect(),
                })
                .collect(),
        },

        Block::ChatInputSimple {
            placeholder,
            action,
            ..
        } => NativeBlock::ChatInputSimple {
            placeholder: placeholder.clone(),
            action: action.as_deref().map(parse_native_action),
        },

        Block::ChipInput {
            label,
            placeholder,
            source,
            on_change,
            chips,
            ..
        } => NativeBlock::ChipInput {
            label: label.clone(),
            placeholder: placeholder.clone(),
            source: source.clone(),
            on_change: on_change.as_deref().map(parse_native_action),
            chips: chips.clone(),
        },

        Block::Progress {
            source, steps, ..
        } => NativeBlock::Progress {
            source: source.clone(),
            steps: steps
                .iter()
                .map(|s| NativeProgressStep {
                    label: s.label.clone(),
                    status: s.status.clone(),
                })
                .collect(),
        },

        Block::LogStream { source, tail, .. } => NativeBlock::LogStream {
            source: source.clone(),
            tail: *tail,
        },

        Block::ProblemList { source, .. } => NativeBlock::ProblemList {
            source: source.clone(),
        },

        // ── App data views promoted from tier 4 (0.11) ──────────────

        Block::List {
            source,
            display,
            item_template,
            filters,
            sort,
            preload,
            stream,
            on_select,
            ..
        } => NativeBlock::List {
            source: source.clone(),
            display: list_display_str(*display),
            item_template: item_template.clone(),
            filters: filters.iter().map(|f| f.field.clone()).collect(),
            sort_field: sort.as_ref().map(|s| s.field.clone()),
            sort_descending: sort.as_ref().is_some_and(|s| s.descending),
            preload: *preload,
            stream: stream.clone(),
            on_select: on_select.as_deref().map(parse_native_action),
        },

        Block::Board {
            source,
            columns,
            card_template,
            preload,
            ..
        } => NativeBlock::Board {
            source: source.clone(),
            columns: columns.clone(),
            card_template: card_template.clone(),
            preload: *preload,
        },

        Block::FilterBar {
            target_selector,
            fields,
            ..
        } => NativeBlock::FilterBar {
            target_selector: target_selector.clone(),
            fields: fields
                .iter()
                .map(|f| NativeFilterField {
                    label: f.label.clone(),
                    name: f.name.clone(),
                    options: f.options.clone(),
                })
                .collect(),
        },

        Block::Search {
            source,
            placeholder,
            ..
        } => NativeBlock::Search {
            source: source.clone(),
            placeholder: placeholder.clone(),
        },

        // ── Messages/Contacts vocabulary (0.12) ─────────────────────

        Block::RecipientPicker {
            source,
            mode,
            on_submit,
            ..
        } => NativeBlock::RecipientPicker {
            source: source.clone(),
            mode: mode.clone(),
            on_submit: on_submit.as_deref().map(parse_native_action),
        },

        Block::Qr {
            mode, on_resolve, ..
        } => NativeBlock::Qr {
            mode: mode.clone(),
            on_resolve: on_resolve.as_deref().map(parse_native_action),
        },

        // ── Wavesite site-format blocks: native conversion ──────────

        Block::Site {
            domain, properties, ..
        } => {
            let mut name = None;
            let mut description = None;
            let mut accent = None;
            let mut font = None;
            let mut extras = Vec::new();
            for p in properties {
                match p.key.as_str() {
                    "name" => name = Some(p.value.clone()),
                    "description" => description = Some(p.value.clone()),
                    "accent" => accent = Some(p.value.clone()),
                    "font" => font = Some(p.value.clone()),
                    _ => extras.push(format!("{}={}", p.key, p.value)),
                }
            }
            NativeBlock::Site {
                name,
                description,
                accent,
                font,
                domain: domain.clone(),
                extras,
            }
        }

        Block::Page {
            route,
            title,
            layout,
            children,
            ..
        } => NativeBlock::Page {
            route: route.clone(),
            title: title.clone(),
            layout: layout.clone(),
            children: convert_children(children, depth + 1),
        },

        // A `::deck` is a config block; rendered natively (in a non-deck
        // context) it produces nothing — presentation chrome lives in
        // render_slides.
        Block::Deck { .. } => NativeBlock::Markdown {
            content: String::new(),
        },

        // A `::slide` rendered outside the deck renderer is a real Slide
        // (0.11) — layout token + kicker + notes + child blocks.
        Block::Slide {
            layout,
            kicker,
            notes,
            children,
            ..
        } => {
            if depth >= MAX_SECTION_DEPTH {
                let md = render_md::render_block(block);
                NativeBlock::Markdown { content: md }
            } else {
                NativeBlock::Slide {
                    layout: layout.unwrap_or_default().css_class().to_string(),
                    kicker: kicker.clone(),
                    notes: notes.clone(),
                    children: convert_children(children, depth + 1),
                }
            }
        }

        // A `::split-pane` carries two recursive child planes (left/right)
        // plus the small-screen back control metadata.
        Block::SplitPane {
            ratio,
            back_label,
            back_action,
            left,
            right,
            ..
        } => {
            if depth >= MAX_SECTION_DEPTH {
                let md = render_md::render_block(block);
                NativeBlock::Markdown { content: md }
            } else {
                NativeBlock::SplitPane {
                    ratio: ratio.clone(),
                    back_label: back_label.clone(),
                    back_action: back_action.clone(),
                    left: convert_children(left, depth + 1),
                    right: convert_children(right, depth + 1),
                }
            }
        }

        Block::Nav { logo, items, .. } => NativeBlock::Nav {
            logo: logo.clone(),
            items: items
                .iter()
                .map(|i| NativeNavItem {
                    label: i.label.clone(),
                    href: i.href.clone(),
                    icon: i.icon.clone(),
                })
                .collect(),
        },

        Block::HeroImage { src, alt, .. } => NativeBlock::HeroImage {
            src: src.clone(),
            alt: alt.clone(),
        },

        Block::Footer {
            sections,
            copyright,
            social,
            ..
        } => NativeBlock::Footer {
            copyright: copyright.clone(),
            sections: sections
                .iter()
                .map(|s| NativeFooterSection {
                    heading: s.heading.clone(),
                    links: s
                        .links
                        .iter()
                        .map(|l| NativeNavItem {
                            label: l.label.clone(),
                            href: l.href.clone(),
                            icon: l.icon.clone(),
                        })
                        .collect(),
                })
                .collect(),
            social: social
                .iter()
                .map(|s| NativeSocialLink {
                    platform: s.platform.clone(),
                    href: s.href.clone(),
                })
                .collect(),
        },

        Block::Embed {
            src,
            embed_type,
            title,
            ..
        } => NativeBlock::Embed {
            src: src.clone(),
            title: title.clone(),
            embed_type: embed_type.map(embed_type_str).unwrap_or_else(|| "generic".to_string()),
        },

        Block::PricingTable { headers, rows, .. } => NativeBlock::PricingTable {
            headers: headers.clone(),
            rows: rows.clone(),
        },

        // ── FFI-hole closure (0.11) ─────────────────────────────────

        Block::Banner {
            headline,
            subtitle,
            buttons,
            id,
            content,
            ..
        } => NativeBlock::Banner {
            headline: headline.clone(),
            subtitle: subtitle.clone(),
            anchor_id: id.clone(),
            buttons: buttons
                .iter()
                .map(|b| NativeHeroButton {
                    label: b.label.clone(),
                    href: b.href.clone(),
                    primary: b.primary,
                })
                .collect(),
            content: content.clone(),
        },

        // Formatted with the document's active citation style — the context
        // is installed by `to_native_blocks` before conversion runs.
        Block::Cite { reference, .. } => {
            let style = crate::citation::with_active(|ctx| {
                crate::citation::active_style(ctx.map(|c| c.style))
            });
            NativeBlock::Cite {
                key: reference.key.clone(),
                formatted: crate::citation::format_reference(reference, style, None),
            }
        }

        Block::Bibliography { style, .. } => {
            let (heading, entries) = crate::citation::with_active(|ctx| {
                let Some(ctx) = ctx else {
                    return (String::new(), Vec::new());
                };
                if ctx.references.is_empty() {
                    return (String::new(), Vec::new());
                }
                let active = style.unwrap_or(ctx.style);
                // Mirrors render_bibliography_html: citation-number order for
                // the document style, definition order under an override.
                let refs = if style.is_some() {
                    ctx.references.clone()
                } else {
                    crate::citation::ordered_references(ctx)
                };
                let entries = crate::citation::reference_list_keyed(&refs, active)
                    .into_iter()
                    .map(|(key, formatted)| NativeReferenceEntry { key, formatted })
                    .collect();
                (
                    crate::citation::bibliography_heading(active).to_string(),
                    entries,
                )
            });
            NativeBlock::Bibliography { heading, entries }
        }

        Block::Gate {
            title,
            subtitle,
            action,
            field_label,
            submit_label,
            error,
            ..
        } => NativeBlock::Gate {
            title: title.clone(),
            subtitle: subtitle.clone(),
            action: action.clone(),
            field_label: field_label.clone(),
            submit_label: submit_label.clone(),
            error: error.clone(),
        },

        Block::ProductGrid { groups, cols, tiles, .. } => NativeBlock::ProductGrid {
            tiles: *tiles,
            cols: cols.map(NativePerClassU32::from),
            groups: groups
                .iter()
                .map(|g| NativeProductGroup {
                    label: g.label.clone(),
                    cols: g.cols.map(u32::from),
                    items: g
                        .items
                        .iter()
                        .map(|i| NativeProductItem {
                            name: i.name.clone(),
                            href: i.href.clone(),
                            emblem: i.emblem.clone(),
                            tagline: i.tagline.clone(),
                            cta1_label: i.cta1_label.clone(),
                            cta1_href: i.cta1_href.clone(),
                            cta2_label: i.cta2_label.clone(),
                            cta2_href: i.cta2_href.clone(),
                            bg: i.bg.clone(),
                        })
                        .collect(),
                })
                .collect(),
        },

        Block::PostGrid {
            title,
            subtitle,
            items,
            ..
        } => NativeBlock::PostGrid {
            title: title.clone(),
            subtitle: subtitle.clone(),
            items: items
                .iter()
                .map(|i| NativePostItem {
                    title: i.title.clone(),
                    href: i.href.clone(),
                    meta: i.meta.clone(),
                    excerpt: i.excerpt.clone(),
                    image: i.image.clone(),
                    external: i.external,
                })
                .collect(),
        },

        // ── Markdown fallback: web-only / unsupported block types ───

        Block::Unknown { .. }
        | Block::Style { .. }
        | Block::Logo { .. }
        | Block::Action { .. }
        | Block::Dashboard { .. }
        | Block::ChatInput { .. }
        | Block::Feed { .. }
        | Block::Booking { .. }
        | Block::Store { .. }
        | Block::Editor { .. }
        | Block::App { .. }
        | Block::Build { .. }
        | Block::InfraDatabase { .. }
        | Block::Deploy { .. }
        | Block::InfraEnv { .. }
        | Block::Health { .. }
        | Block::Concurrency { .. }
        | Block::Cicd { .. }
        | Block::Smoke { .. }
        | Block::Domains { .. }
        | Block::Crates { .. }
        | Block::DeployUrls { .. }
        | Block::Volumes { .. }
        | Block::Model { .. }
        | Block::Route { .. }
        | Block::Auth { .. }
        | Block::Binding { .. }
        | Block::Schema { .. }
        | Block::Use { .. }
        | Block::AppEnv { .. }
        | Block::AppDeploy { .. } => {
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
        CalloutType::Context => "context",
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

fn toolbar_item_to_native(item: &ToolbarItem) -> NativeToolbarItem {
    match item {
        ToolbarItem::Button {
            label,
            action,
            icon,
            style,
            disabled,
            // No NativeToolbarItem field yet (no schema bump needed);
            // the accent-ring open state is web-only until a native round.
            toggled: _,
            // Same precedent (0.13.3): the workspace-chip avatar initial is
            // web-only render styling — no native schema field.
            avatar: _,
            // Same precedent (0.13.3): the explicit accessible name is
            // web-only render styling — no native schema field.
            aria_label: _,
        } => NativeToolbarItem {
            kind: "button".to_string(),
            label: label.clone(),
            action: action.as_deref().map(parse_native_action),
            icon: icon.clone(),
            style: style.clone(),
            disabled: *disabled,
            value: None,
            color: None,
            options: None,
        },
        ToolbarItem::Separator => NativeToolbarItem {
            kind: "separator".to_string(),
            label: None,
            action: None,
            icon: None,
            style: None,
            disabled: false,
            value: None,
            color: None,
            options: None,
        },
        ToolbarItem::Spacer => NativeToolbarItem {
            kind: "spacer".to_string(),
            label: None,
            action: None,
            icon: None,
            style: None,
            disabled: false,
            value: None,
            color: None,
            options: None,
        },
        ToolbarItem::Badge { value, color } => NativeToolbarItem {
            kind: "badge".to_string(),
            label: None,
            action: None,
            icon: None,
            style: None,
            disabled: false,
            value: Some(value.clone()),
            color: color.clone(),
            options: None,
        },
        ToolbarItem::Dropdown {
            label,
            options,
            action,
        } => NativeToolbarItem {
            kind: "dropdown".to_string(),
            label: Some(label.clone()),
            action: action.as_deref().map(parse_native_action),
            icon: None,
            style: None,
            disabled: false,
            value: None,
            color: None,
            options: options.clone(),
        },
        ToolbarItem::Text { value, .. } => NativeToolbarItem {
            kind: "text".to_string(),
            label: None,
            action: None,
            icon: None,
            style: None,
            disabled: false,
            value: Some(value.clone()),
            color: None,
            options: None,
        },
    }
}

fn list_display_str(d: crate::types::ListDisplay) -> String {
    match d {
        crate::types::ListDisplay::Card => "card",
        crate::types::ListDisplay::Table => "table",
        crate::types::ListDisplay::Compact => "compact",
    }
    .to_string()
}

fn embed_type_str(et: EmbedType) -> String {
    match et {
        EmbedType::Map => "map",
        EmbedType::Video => "video",
        EmbedType::Audio => "audio",
        EmbedType::Generic => "generic",
    }
    .to_string()
}

fn form_field_type_str(ft: FormFieldType) -> String {
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
    .to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// Conformance: block tier model
// ═══════════════════════════════════════════════════════════════════════

/// Native rendering tier for a `Block` variant.
///
/// This is the explicit version of the tier model from the render-unification
/// plan: every `Block` variant is either rendered as a structured
/// `NativeBlock` (tiers 1–3) or explicitly degraded to markdown (tier 4).
/// There is no silent fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTier {
    /// Tier 1 — document content (markdown, code, callout, …).
    Content,
    /// Tier 2 — site/marketing blocks (hero, features, pricing, …).
    Site,
    /// Tier 3 — app chrome (appShell, tabBar, modal, commandPalette, …).
    Chrome,
    /// Tier 4 — manifest/infra/web-only blocks; degrade to a markdown string
    /// computed in Rust (`render_md`), same string on every platform.
    Degraded,
}

/// Classify a `Block` variant into its native rendering tier.
///
/// **Drift guard**: this match is deliberately exhaustive with no wildcard
/// arm, exactly like `convert_block` and `render_html::render_block`. Adding
/// a `Block` variant fails compilation here until the variant is classified —
/// and the conformance tests below pin the classification against what
/// `convert_block` actually produces.
pub fn block_tier(block: &Block) -> BlockTier {
    match block {
        // ── Tier 1: content ──────────────────────────────────────────
        Block::Markdown { .. }
        | Block::Code { .. }
        | Block::Callout { .. }
        | Block::Data { .. }
        | Block::Tasks { .. }
        | Block::Figure { .. }
        | Block::Diagram { .. }
        | Block::Quote { .. }
        | Block::Divider { .. }
        | Block::Details { .. }
        | Block::Decision { .. }
        | Block::Metric { .. }
        | Block::Summary { .. }
        | Block::Cite { .. }
        | Block::Bibliography { .. }
        // Reader-content blocks promoted from Degraded → native structural
        // rendering (A-04 / BR-APP-7).
        | Block::ProductCard { .. }
        | Block::Chart { .. }
        | Block::Row { .. }
        | Block::InfoCard { .. }
        | Block::Toc { .. } => BlockTier::Content,

        // ── Tier 2: site/marketing ───────────────────────────────────
        Block::Hero { .. }
        | Block::Features { .. }
        | Block::Steps { .. }
        | Block::Stats { .. }
        | Block::Comparison { .. }
        | Block::BeforeAfter { .. }
        | Block::Pipeline { .. }
        | Block::Testimonial { .. }
        | Block::Cta { .. }
        | Block::Banner { .. }
        | Block::ProductGrid { .. }
        | Block::PostGrid { .. }
        | Block::Gate { .. }
        | Block::Gallery { .. }
        | Block::Faq { .. }
        | Block::PricingTable { .. }
        | Block::Columns { .. }
        | Block::Tabs { .. }
        | Block::Section { .. }
        | Block::Form { .. }
        | Block::Site { .. }
        | Block::Page { .. }
        | Block::Nav { .. }
        | Block::HeroImage { .. }
        | Block::Footer { .. }
        | Block::Embed { .. }
        // A ::slide outside the deck renderer is a SectionContainer.
        | Block::Slide { .. } => BlockTier::Site,

        // ── Tier 3: app chrome ───────────────────────────────────────
        Block::AppShell { .. }
        | Block::Sidebar { .. }
        | Block::Panel { .. }
        | Block::TabBar { .. }
        | Block::TabContent { .. }
        | Block::Toolbar { .. }
        | Block::Drawer { .. }
        | Block::Modal { .. }
        | Block::CommandPalette { .. }
        | Block::DropdownSelect { .. }
        | Block::SegmentedControl { .. }
        | Block::CodeEditor { .. }
        | Block::BlockEditor { .. }
        | Block::Terminal { .. }
        | Block::NavTree { .. }
        | Block::Badge { .. }
        | Block::SuggestionChips { .. }
        | Block::ChatThread { .. }
        | Block::ChatInputSimple { .. }
        | Block::ChipInput { .. }
        | Block::Progress { .. }
        | Block::LogStream { .. }
        | Block::ProblemList { .. }
        // App data views promoted from tier 4 (0.11).
        | Block::List { .. }
        | Block::Board { .. }
        | Block::FilterBar { .. }
        | Block::Search { .. }
        // Messages/Contacts vocabulary (0.12).
        | Block::RecipientPicker { .. }
        | Block::Qr { .. }
        // Split-pane layout crosses the FFI boundary natively (0.16).
        | Block::SplitPane { .. } => BlockTier::Chrome,

        // ── Tier 4: explicit markdown degradation ────────────────────
        Block::Unknown { .. }
        | Block::Style { .. }
        | Block::Logo { .. }
        | Block::Action { .. }
        | Block::Dashboard { .. }
        | Block::ChatInput { .. }
        | Block::Feed { .. }
        | Block::Booking { .. }
        | Block::Store { .. }
        | Block::Editor { .. }
        | Block::App { .. }
        | Block::Build { .. }
        | Block::InfraDatabase { .. }
        | Block::Deploy { .. }
        | Block::InfraEnv { .. }
        | Block::Health { .. }
        | Block::Concurrency { .. }
        | Block::Cicd { .. }
        | Block::Smoke { .. }
        | Block::Domains { .. }
        | Block::Crates { .. }
        | Block::DeployUrls { .. }
        | Block::Volumes { .. }
        | Block::Model { .. }
        | Block::Route { .. }
        | Block::Auth { .. }
        | Block::Binding { .. }
        | Block::Schema { .. }
        | Block::Use { .. }
        | Block::AppEnv { .. }
        | Block::AppDeploy { .. }
        // ::deck is presentation config; produces no native content.
        | Block::Deck { .. } => BlockTier::Degraded,
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Unit tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use std::collections::BTreeMap;

    /// V-A5: the minimal action grammar — bare = invoke, open:/route,
    /// mutate:name:payload — and its never-fail degradation rules.
    #[test]
    fn native_action_grammar() {
        let a = parse_native_action("open_doc");
        assert_eq!((a.verb.as_str(), a.target.as_str(), a.payload, a.raw.as_str()),
                   ("invoke", "open_doc", None, "open_doc"));

        let a = parse_native_action("open:/docs/123");
        assert_eq!((a.verb.as_str(), a.target.as_str()), ("open", "/docs/123"));

        let a = parse_native_action("invoke:switch_root");
        assert_eq!((a.verb.as_str(), a.target.as_str()), ("invoke", "switch_root"));

        let a = parse_native_action("mutate:tasks.set_stage:stage");
        assert_eq!(
            (a.verb.as_str(), a.target.as_str(), a.payload.as_deref()),
            ("mutate", "tasks.set_stage", Some("stage"))
        );

        let a = parse_native_action("mutate:messages.send");
        assert_eq!(
            (a.verb.as_str(), a.target.as_str(), a.payload),
            ("mutate", "messages.send", None)
        );

        // Unknown verb prefix degrades to invoke-on-the-whole-string.
        let a = parse_native_action("frobnicate:thing");
        assert_eq!((a.verb.as_str(), a.target.as_str()), ("invoke", "frobnicate:thing"));
        assert_eq!(a.raw, "frobnicate:thing");

        // Trailing colon with empty rest also degrades whole.
        let a = parse_native_action("open:");
        assert_eq!((a.verb.as_str(), a.target.as_str()), ("invoke", "open:"));
    }

    /// 0.12: `::list` stream and on-select cross the FFI — the event name
    /// verbatim, the select action typed through the minimal grammar.
    #[test]
    fn list_stream_and_on_select_cross_typed() {
        let source = "::list[source=conversations display=compact stream=conversation_updated on-select=openThread]\n{= display_name =}\n::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::List { source, stream, on_select, .. } => {
                assert_eq!(source, "conversations");
                assert_eq!(stream.as_deref(), Some("conversation_updated"));
                let sel = on_select.as_ref().expect("on_select");
                assert_eq!(sel.verb, "invoke");
                assert_eq!(sel.target, "openThread");
                assert_eq!(sel.raw, "openThread");
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    /// 0.12: `::row` action lines cross the FFI as labelled typed actions.
    #[test]
    fn row_actions_cross_typed() {
        let source = "::row[icon=doc]\nJordan Lee\n@jordan\naction: Accept | invoke:contacts.accept\naction: Deny | mutate:contacts.deny:id\n::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::Row { actions, .. } => {
                assert_eq!(actions.len(), 2);
                assert_eq!(actions[0].label, "Accept");
                assert_eq!(actions[0].action.verb, "invoke");
                assert_eq!(actions[0].action.target, "contacts.accept");
                assert_eq!(actions[1].action.verb, "mutate");
                assert_eq!(actions[1].action.payload.as_deref(), Some("id"));
            }
            other => panic!("expected Row, got {other:?}"),
        }
    }

    /// 0.12: `::chat-thread` reaction and doc-chip seams cross the FFI
    /// typed through the same minimal action grammar as `on-action`.
    #[test]
    fn chat_thread_react_and_doc_open_cross_typed() {
        let source = "::chat-thread[source=chat.thread on-action=run_action on-react=\"mutate:messages.react:emoji\" on-doc-open=\"open:/docs/123\"]\n::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::ChatThread { on_action, on_react, on_doc_open, .. } => {
                assert_eq!(on_action.as_ref().expect("on_action").verb, "invoke");
                let react = on_react.as_ref().expect("on_react");
                assert_eq!(react.verb, "mutate");
                assert_eq!(react.target, "messages.react");
                assert_eq!(react.payload.as_deref(), Some("emoji"));
                let doc_open = on_doc_open.as_ref().expect("on_doc_open");
                assert_eq!(doc_open.verb, "open");
                assert_eq!(doc_open.target, "/docs/123");
            }
            other => panic!("expected ChatThread, got {other:?}"),
        }
    }

    /// 0.17: authored chat-thread message children cross the FFI —
    /// side/sender/timestamp/text plus read-only reaction pills.
    #[test]
    fn chat_thread_messages_cross_native() {
        let source = "::chat-thread[source=chat.thread]\n\
            - them[sender=\"Danny\" time=\"1:42 PM\" reactions=\"Love:2:mine|Wave\"] Tahoe update finished\n\
            - own[time=\"1:44 PM\"] Yes, retry now\n\
            ::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::ChatThread { messages, .. } => {
                assert_eq!(messages.len(), 2);
                assert_eq!(messages[0].side, "them");
                assert_eq!(messages[0].sender.as_deref(), Some("Danny"));
                assert_eq!(messages[0].timestamp.as_deref(), Some("1:42 PM"));
                assert_eq!(messages[0].text, "Tahoe update finished");
                assert_eq!(messages[0].reactions.len(), 2);
                assert_eq!(messages[0].reactions[0].label, "Love");
                assert_eq!(messages[0].reactions[0].count, Some(2));
                assert!(messages[0].reactions[0].mine);
                assert!(!messages[0].reactions[1].mine);
                assert_eq!(messages[1].side, "own");
                assert_eq!(messages[1].sender, None);
            }
            other => panic!("expected ChatThread, got {other:?}"),
        }
    }

    /// 0.17: attrs-only chat-thread keeps empty native messages (the
    /// registry-bound shape, backward-compatible).
    #[test]
    fn chat_thread_attrs_only_crosses_with_empty_messages() {
        let result = crate::parse("::chat-thread[source=chat.thread]\n::");
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::ChatThread { messages, .. } => assert!(messages.is_empty()),
            other => panic!("expected ChatThread, got {other:?}"),
        }
    }

    /// 0.17: the chip-input kind crosses the FFI with a typed on_change.
    #[test]
    fn chip_input_crosses_native() {
        let source = "::chip-input[label=\"To:\" placeholder=\"Type a name…\" source=contacts on-change=\"invoke:messages.compose\"]\n\
            - Danny Pappageorge\n\
            ::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::ChipInput { label, placeholder, source, on_change, chips } => {
                assert_eq!(label.as_deref(), Some("To:"));
                assert_eq!(placeholder.as_deref(), Some("Type a name…"));
                assert_eq!(source.as_deref(), Some("contacts"));
                let ch = on_change.as_ref().expect("on_change");
                assert_eq!(ch.verb, "invoke");
                assert_eq!(ch.target, "messages.compose");
                assert_eq!(chips, &["Danny Pappageorge".to_string()]);
            }
            other => panic!("expected ChipInput, got {other:?}"),
        }
    }

    /// 0.17: row avatar/rtime/unread-count cross the FFI.
    #[test]
    fn row_avatar_rtime_unread_count_cross_native() {
        let source = "::row[icon=doc avatar=auto rtime=\"1:42 PM\" unread-count=3]\nDanny Pappageorge\nDirect message\n::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::Row { avatar, rtime, unread_count, .. } => {
                // avatar=auto is derived to initials at parse.
                assert_eq!(avatar.as_deref(), Some("DP"));
                assert_eq!(rtime.as_deref(), Some("1:42 PM"));
                assert_eq!(*unread_count, Some(3));
            }
            other => panic!("expected Row, got {other:?}"),
        }
    }

    /// Actions on parsed blocks cross the FFI typed: registry-bound bare
    /// names keep their raw form while gaining verb/target structure.
    #[test]
    fn nav_tree_actions_cross_typed() {
        let source = "::nav-tree[source=docs on-select=open_doc on-delete=\"mutate:docs.delete:id\"]\n::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::NavTree { on_select, on_delete, .. } => {
                let sel = on_select.as_ref().expect("on_select");
                assert_eq!(sel.verb, "invoke");
                assert_eq!(sel.target, "open_doc");
                assert_eq!(sel.raw, "open_doc");
                let del = on_delete.as_ref().expect("on_delete");
                assert_eq!(del.verb, "mutate");
                assert_eq!(del.target, "docs.delete");
                assert_eq!(del.payload.as_deref(), Some("id"));
            }
            other => panic!("expected NavTree, got {other:?}"),
        }
    }

    /// 0.16: messages-shaped `::split-pane` markup crosses the FFI as a
    /// recursive NativeBlock with structured children on BOTH panes.
    #[test]
    fn split_pane_messages_markup_crosses_ffi() {
        let source = "::split-pane[ratio=\"30:70\" back-label=\"Chats\" back-action=closeConversation]\n::pane[side=left]\n::row[icon=knowledge href=#]\nSam Rose\n::\n::\n::pane[side=right]\nThread body\n::\n::";
        let result = crate::parse(source);
        let native = to_native_blocks(&result.doc);
        match &native[0] {
            NativeBlock::SplitPane {
                ratio,
                back_label,
                back_action,
                left,
                right,
            } => {
                assert_eq!(ratio, "30:70");
                assert_eq!(back_label.as_deref(), Some("Chats"));
                assert_eq!(back_action.as_deref(), Some("closeConversation"));
                assert!(!left.is_empty(), "left pane must carry children");
                assert!(!right.is_empty(), "right pane must carry children");
                assert!(
                    left.iter().any(|b| matches!(b, NativeBlock::Row { .. })),
                    "left pane row converts structurally: {left:?}"
                );
                assert!(
                    right.iter().any(|b| matches!(b, NativeBlock::Markdown { content } if content.contains("Thread body"))),
                    "right pane thread body crosses: {right:?}"
                );
            }
            other => panic!("expected SplitPane, got {other:?}"),
        }
    }

    /// 0.16: optional back attrs stay None when absent; ratio passes through.
    #[test]
    fn split_pane_optional_attrs_passthrough() {
        let source = "::split-pane[ratio=\"60:40\"]\n::pane\nRail\n::\n::pane\nThread\n::\n::";
        let native = to_native_blocks(&crate::parse(source).doc);
        match &native[0] {
            NativeBlock::SplitPane {
                ratio,
                back_label,
                back_action,
                left,
                right,
            } => {
                assert_eq!(ratio, "60:40");
                assert!(back_label.is_none());
                assert!(back_action.is_none());
                assert_eq!(left.len(), 1);
                assert_eq!(right.len(), 1);
            }
            other => panic!("expected SplitPane, got {other:?}"),
        }
    }

    /// 0.16: depth guard — at MAX_SECTION_DEPTH a split-pane degrades to
    /// Markdown just like SectionContainer and Slide.
    #[test]
    fn native_split_pane_depth_limit() {
        let block = Block::SplitPane {
            ratio: "50:50".to_string(),
            back_label: Some("Back".to_string()),
            back_action: None,
            left: vec![Block::Markdown {
                content: "left body".to_string(),
                span: syn(),
            }],
            right: vec![Block::Markdown {
                content: "right body".to_string(),
                span: syn(),
            }],
            span: syn(),
        };
        match convert_block(&block, 7) {
            NativeBlock::SplitPane { left, right, .. } => {
                assert_eq!(left.len(), 1);
                assert_eq!(right.len(), 1);
            }
            other => panic!("expected SplitPane at depth 7, got {other:?}"),
        }
        match convert_block(&block, 8) {
            NativeBlock::Markdown { .. } => {}
            other => panic!("expected Markdown fallback at depth 8, got {other:?}"),
        }
    }

    fn syn() -> Span {
        Span::SYNTHETIC
    }

    /// Conformance: `block_tier` agrees with what `convert_block` produces —
    /// Degraded-tier blocks convert to `NativeBlock::Markdown`, structured
    /// tiers convert to structured variants. Parses source covering every
    /// tier so the check runs on real parser output, not hand-built blocks.
    #[test]
    fn tier_classification_matches_conversion() {
        let source = "\
# Heading

::callout[type=info]\nNote\n::\n
::hero\nheadline: H\n::\n
::badge[value=3]\n::\n
::app[name=demo]\n::\n
::section\nInner\n::\n";
        let result = crate::parse(source);
        let mut saw_degraded = false;
        let mut saw_structured = false;
        for block in &result.doc.blocks {
            let native = convert_block(block, 0);
            match block_tier(block) {
                BlockTier::Degraded => {
                    saw_degraded = true;
                    assert!(
                        matches!(native, NativeBlock::Markdown { .. }),
                        "Degraded-tier {block:?} must convert to Markdown"
                    );
                }
                BlockTier::Content | BlockTier::Site | BlockTier::Chrome => {
                    saw_structured = true;
                    if !matches!(block, Block::Markdown { .. }) {
                        assert!(
                            !matches!(native, NativeBlock::Markdown { .. }),
                            "structured-tier {block:?} must not degrade to Markdown"
                        );
                    }
                }
            }
        }
        assert!(saw_degraded && saw_structured, "fixture must cover both paths");
    }

    /// A-04 / BR-APP-7: the five reader-content blocks render structurally on
    /// native (no longer degrade to Markdown). Parses real source so the arm
    /// + the parser agree.
    #[test]
    fn content_cards_convert_structurally() {
        let source = "\
::product-card[badge=\"New\"]\n## Pro\nsub\n\nbody\n\n- feat\n\n[Buy](/p)\n::\n
::chart[type=bar source=\"/api/rev\" period=monthly]\n::\n
::row[icon=doc href=\"/d\"]\nTitle\nDesc\n::\n
::infocard[intent=info]\n# Card\nsub\n\nsummary\nKey: Val\n::\n
::diagram[type=erd title=\"M\"]\na: id pk\n::\n";
        let result = crate::parse(source);
        let natives: Vec<NativeBlock> = result
            .doc
            .blocks
            .iter()
            .map(|b| convert_block(b, 0))
            .collect();

        assert!(matches!(natives[0], NativeBlock::ProductCard { ref features, .. } if features == &["feat".to_string()]));
        assert!(matches!(natives[1], NativeBlock::Chart { ref chart_type, ref source, .. } if chart_type == "bar" && source == "/api/rev"));
        assert!(matches!(natives[2], NativeBlock::Row { ref icon, ref state, .. } if icon == "doc" && state == "default"));
        match &natives[3] {
            NativeBlock::InfoCard { intent, facts, .. } => {
                assert_eq!(intent, "info");
                assert_eq!(facts, &[NativeInfoFact { label: "Key".into(), value: "Val".into() }]);
            }
            other => panic!("expected InfoCard, got {other:?}"),
        }
        assert!(matches!(natives[4], NativeBlock::Diagram { ref diagram_type, .. } if diagram_type == "erd"));
        // None silently degraded.
        assert!(!natives.iter().any(|n| matches!(n, NativeBlock::Markdown { .. })));
    }

    /// Diagram blocks carry the laid-out geometry scene across the FFI:
    /// a parseable DSL yields `Some` scene with shapes, a malformed DSL
    /// yields `None` (the raw DSL stays either way). Charts with an inline
    /// dataset carry a scene too; source-only charts stay `None` (they keep
    /// the live-data mount-point path).
    #[test]
    fn diagram_native_scene_population() {
        let source = "\
::diagram[type=erd title=\"M\"]\na: id pk\n::\n
::diagram[type=architecture]\nnot ! a % statement\n::\n
::chart[type=bar source=\"/api/rev\"]\n::\n
::chart[type=bar title=\"Rev\"]\nMonth | Rev\nJan | 10\nFeb | 20\n::\n
::diagram[type=pie]\nSlice | Share\nA | 60\nB | 40\n::\n
::diagram[type=mermaid]\nflowchart LR\nA --> B\n::\n";
        let result = crate::parse(source);
        let natives: Vec<NativeBlock> = result
            .doc
            .blocks
            .iter()
            .map(|b| convert_block(b, 0))
            .collect();

        match &natives[0] {
            NativeBlock::Diagram { scene: Some(scene), content, .. } => {
                assert!(scene.width > 0.0 && scene.height > 0.0);
                assert!(!scene.shapes.is_empty(), "erd scene must carry shapes");
                assert_eq!(content, "a: id pk");
            }
            other => panic!("expected Diagram with scene, got {other:?}"),
        }
        match &natives[1] {
            NativeBlock::Diagram { scene: None, content, .. } => {
                assert_eq!(content, "not ! a % statement");
            }
            other => panic!("expected Diagram without scene, got {other:?}"),
        }
        assert!(matches!(&natives[2], NativeBlock::Chart { scene: None, .. }));
        // Inline-data chart: scene populated from the same layout math as
        // the SVG (canvas is the fixed 680×380 chart frame).
        match &natives[3] {
            NativeBlock::Chart { scene: Some(scene), .. } => {
                assert_eq!(scene.width, 680.0);
                assert_eq!(scene.height, 380.0);
                assert!(!scene.shapes.is_empty());
            }
            other => panic!("expected Chart with scene, got {other:?}"),
        }
        // Chart-alias diagram (pie) gets a chart scene through the same path.
        match &natives[4] {
            NativeBlock::Diagram { scene: Some(scene), diagram_type, .. } => {
                assert_eq!(diagram_type, "pie");
                assert_eq!(scene.width, 680.0);
            }
            other => panic!("expected pie Diagram with chart scene, got {other:?}"),
        }
        // Mermaid bodies translate before scene layout.
        match &natives[5] {
            NativeBlock::Diagram { scene: Some(scene), content, .. } => {
                assert!(!scene.shapes.is_empty(), "mermaid flowchart must lay out");
                assert!(content.contains("flowchart LR"), "raw source is preserved");
            }
            other => panic!("expected mermaid Diagram with scene, got {other:?}"),
        }
    }

    #[test]
    fn native_markdown_passthrough() {
        let block = Block::Markdown {
            content: "# Hello\n\nWorld".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            match convert_block(&block, 0) {
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            image_alt: None,
            layout: None,
            transparent: false,
            buttons: vec![],
            content: "Some content".to_string(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::Hero {
                headline: Some("Welcome".to_string()),
                subtitle: Some("To SurfDoc".to_string()),
                badge: Some("New".to_string()),
                align: "center".to_string(),
                image: Some("hero.png".to_string()),
                buttons: vec![],
                content: "Some content".to_string(),
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
            cols: Some(PerClass::uniform(2)),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::Features {
                cols: Some(NativePerClassU32 { mobile: 2, tablet: 2, desktop: 2 }),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
            convert_block(&block, 0),
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
        match convert_block(&block, 0) {
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
    fn native_nav_block() {
        let block = Block::Nav {
            items: vec![NavItem {
                label: "Home".to_string(),
                href: "/".to_string(),
                icon: None,
                image: None, external: false,
            }],
            logo: Some("Acme".to_string()),
            groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Nav { logo, items } => {
                assert_eq!(logo, Some("Acme".to_string()));
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].label, "Home");
                assert_eq!(items[0].href, "/");
            }
            other => panic!("Expected Nav, got {:?}", other),
        }
    }

    #[test]
    fn native_hero_image_block() {
        let block = Block::HeroImage {
            src: "hero.png".to_string(),
            alt: Some("Shot".to_string()),
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::HeroImage { src, alt } => {
                assert_eq!(src, "hero.png");
                assert_eq!(alt, Some("Shot".to_string()));
            }
            other => panic!("Expected HeroImage, got {:?}", other),
        }
    }

    #[test]
    fn native_site_block_extracts_well_known_keys() {
        let block = Block::Site {
            domain: Some("example.com".to_string()),
            properties: vec![
                StyleProperty {
                    key: "name".to_string(),
                    value: "Acme".to_string(),
                },
                StyleProperty {
                    key: "accent".to_string(),
                    value: "#ff0000".to_string(),
                },
                StyleProperty {
                    key: "font".to_string(),
                    value: "montserrat".to_string(),
                },
                StyleProperty {
                    key: "description".to_string(),
                    value: "Tagline".to_string(),
                },
                StyleProperty {
                    key: "other".to_string(),
                    value: "extra-value".to_string(),
                },
            ],
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Site {
                name,
                description,
                accent,
                font,
                domain,
                extras,
            } => {
                assert_eq!(name, Some("Acme".to_string()));
                assert_eq!(description, Some("Tagline".to_string()));
                assert_eq!(accent, Some("#ff0000".to_string()));
                assert_eq!(font, Some("montserrat".to_string()));
                assert_eq!(domain, Some("example.com".to_string()));
                assert_eq!(extras, vec!["other=extra-value".to_string()]);
            }
            other => panic!("Expected Site, got {:?}", other),
        }
    }

    #[test]
    fn native_page_block_converts_children() {
        let block = Block::Page {
            route: "/".to_string(),
            layout: Some("hero".to_string()),
            title: Some("Home".to_string()),
            sidebar: false,
            content: String::new(),
            children: vec![
                Block::HeroImage {
                    src: "a.png".to_string(),
                    alt: None,
                    span: syn(),
                },
                Block::Markdown {
                    content: "Hello".to_string(),
                    span: syn(),
                },
            ],
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Page {
                route,
                title,
                layout,
                children,
            } => {
                assert_eq!(route, "/");
                assert_eq!(title, Some("Home".to_string()));
                assert_eq!(layout, Some("hero".to_string()));
                assert_eq!(children.len(), 2);
                assert!(matches!(&children[0], NativeBlock::HeroImage { .. }));
                assert!(matches!(&children[1], NativeBlock::Markdown { .. }));
            }
            other => panic!("Expected Page, got {:?}", other),
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
        match convert_block(&block, 0) {
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
                    groups: vec![], brand: None, brand_reg: false, cta: None, drawer: false, minimal: false,
                    span: syn(),
                },
            ],
            source: String::new(),
        };
        let native = to_native_blocks(&doc);
        assert_eq!(native.len(), 3);
        assert!(matches!(&native[0], NativeBlock::Markdown { .. }));
        assert!(matches!(&native[1], NativeBlock::Callout { .. }));
        assert!(matches!(&native[2], NativeBlock::Nav { .. }));
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

    // ── Form tests ─────────────────────────────────────────────────

    #[test]
    fn native_form_basic() {
        let block = Block::Form {
            fields: vec![
                FormField {
                    label: "Name".to_string(),
                    name: "name".to_string(),
                    field_type: FormFieldType::Text,
                    required: true,
                    placeholder: Some("Enter your name".to_string()),
                    options: vec![],
                },
                FormField {
                    label: "Email".to_string(),
                    name: "email".to_string(),
                    field_type: FormFieldType::Email,
                    required: true,
                    placeholder: None,
                    options: vec![],
                },
            ],
            submit_label: Some("Send".to_string()),
            action: None, method: None, honeypot: false,
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::Form {
                fields: vec![
                    NativeFormField {
                        label: "Name".to_string(),
                        name: "name".to_string(),
                        field_type: "text".to_string(),
                        required: true,
                        placeholder: Some("Enter your name".to_string()),
                        options: vec![],
                    },
                    NativeFormField {
                        label: "Email".to_string(),
                        name: "email".to_string(),
                        field_type: "email".to_string(),
                        required: true,
                        placeholder: None,
                        options: vec![],
                    },
                ],
                submit_label: "Send".to_string(),
            }
        );
    }

    #[test]
    fn native_form_default_submit_label() {
        let block = Block::Form {
            fields: vec![],
            submit_label: None,
            action: None, method: None, honeypot: false,
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Form {
                submit_label,
                fields,
            } => {
                assert_eq!(submit_label, "Submit");
                assert!(fields.is_empty());
            }
            other => panic!("Expected Form, got {:?}", other),
        }
    }

    #[test]
    fn native_form_all_field_types() {
        let types = [
            (FormFieldType::Text, "text"),
            (FormFieldType::Email, "email"),
            (FormFieldType::Tel, "tel"),
            (FormFieldType::Date, "date"),
            (FormFieldType::Number, "number"),
            (FormFieldType::Select, "select"),
            (FormFieldType::Textarea, "textarea"),
        ];
        for (ft, expected) in types {
            let block = Block::Form {
                fields: vec![FormField {
                    label: "Test".to_string(),
                    name: "test".to_string(),
                    field_type: ft,
                    required: false,
                    placeholder: None,
                    options: vec![],
                }],
                submit_label: None,
                action: None, method: None, honeypot: false,
                span: syn(),
            };
            match convert_block(&block, 0) {
                NativeBlock::Form { fields, .. } => {
                    assert_eq!(fields[0].field_type, expected);
                }
                other => panic!("Expected Form, got {:?}", other),
            }
        }
    }

    #[test]
    fn native_form_select_with_options() {
        let block = Block::Form {
            fields: vec![FormField {
                label: "Country".to_string(),
                name: "country".to_string(),
                field_type: FormFieldType::Select,
                required: false,
                placeholder: None,
                options: vec!["US".to_string(), "CA".to_string(), "UK".to_string()],
            }],
            submit_label: Some("Go".to_string()),
            action: None, method: None, honeypot: false,
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Form { fields, .. } => {
                assert_eq!(fields[0].field_type, "select");
                assert_eq!(fields[0].options, vec!["US", "CA", "UK"]);
            }
            other => panic!("Expected Form, got {:?}", other),
        }
    }

    // ── Gallery tests ──────────────────────────────────────────────

    #[test]
    fn native_gallery_basic() {
        let block = Block::Gallery {
            items: vec![
                GalleryItem {
                    src: "photo1.jpg".to_string(),
                    caption: Some("Sunset".to_string()),
                    alt: Some("A sunset".to_string()),
                    category: Some("Nature".to_string()),
                },
                GalleryItem {
                    src: "photo2.jpg".to_string(),
                    caption: None,
                    alt: None,
                    category: None,
                },
            ],
            columns: Some(PerClass::uniform(4)),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::Gallery {
                items: vec![
                    NativeGalleryItem {
                        src: "photo1.jpg".to_string(),
                        caption: Some("Sunset".to_string()),
                        alt: Some("A sunset".to_string()),
                        category: Some("Nature".to_string()),
                    },
                    NativeGalleryItem {
                        src: "photo2.jpg".to_string(),
                        caption: None,
                        alt: None,
                        category: None,
                    },
                ],
                columns: NativePerClassU32 { mobile: 4, tablet: 4, desktop: 4 },
            }
        );
    }

    #[test]
    fn native_gallery_default_columns() {
        let block = Block::Gallery {
            items: vec![],
            columns: None,
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Gallery { columns, items } => {
                assert_eq!(columns, NativePerClassU32 { mobile: 3, tablet: 3, desktop: 3 });
                assert!(items.is_empty());
            }
            other => panic!("Expected Gallery, got {:?}", other),
        }
    }

    // ── SectionContainer tests ─────────────────────────────────────

    #[test]
    fn native_section_container_basic() {
        let block = Block::Section {
            bg: Some("muted".to_string()),
            headline: Some("Features".to_string()),
            subtitle: Some("What we offer".to_string()),
            content: String::new(),
            children: vec![
                Block::Markdown {
                    content: "Hello world".to_string(),
                    span: syn(),
                },
                Block::Callout {
                    callout_type: CalloutType::Info,
                    title: None,
                    content: "A note".to_string(),
                    span: syn(),
                },
            ],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::SectionContainer {
                bg: Some("muted".to_string()),
                headline: Some("Features".to_string()),
                subtitle: Some("What we offer".to_string()),
                children: vec![
                    NativeBlock::Markdown {
                        content: "Hello world".to_string(),
                    },
                    NativeBlock::Callout {
                        callout_type: "info".to_string(),
                        title: None,
                        content: "A note".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn native_section_container_empty() {
        let block = Block::Section {
            bg: None,
            headline: None,
            subtitle: None,
            content: String::new(),
            children: vec![],
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::SectionContainer {
                bg: None,
                headline: None,
                subtitle: None,
                children: vec![],
            }
        );
    }

    #[test]
    fn native_section_depth_limit() {
        let block = Block::Section {
            bg: None,
            headline: Some("Deep section".to_string()),
            subtitle: None,
            content: String::new(),
            children: vec![Block::Markdown {
                content: "deep content".to_string(),
                span: syn(),
            }],
            span: syn(),
        };
        // At depth 7 (< 8), should produce SectionContainer
        match convert_block(&block, 7) {
            NativeBlock::SectionContainer {
                headline, children, ..
            } => {
                assert_eq!(headline, Some("Deep section".to_string()));
                assert_eq!(children.len(), 1);
            }
            other => panic!("Expected SectionContainer at depth 7, got {:?}", other),
        }
        // At depth 8 (== MAX_SECTION_DEPTH), should fall back to Markdown
        match convert_block(&block, 8) {
            NativeBlock::Markdown { content } => {
                assert!(
                    content.contains("Deep section"),
                    "Markdown fallback should contain headline: {content}"
                );
            }
            other => panic!("Expected Markdown fallback at depth 8, got {:?}", other),
        }
        // At depth 100 (>> MAX_SECTION_DEPTH), should also fall back
        match convert_block(&block, 100) {
            NativeBlock::Markdown { .. } => {}
            other => panic!("Expected Markdown fallback at depth 100, got {:?}", other),
        }
    }

    #[test]
    fn native_section_depth_propagates() {
        // Section containing a Section child — both should convert at depth 0
        let inner = Block::Section {
            bg: None,
            headline: Some("Inner".to_string()),
            subtitle: None,
            content: String::new(),
            children: vec![],
            span: syn(),
        };
        let outer = Block::Section {
            bg: None,
            headline: Some("Outer".to_string()),
            subtitle: None,
            content: String::new(),
            children: vec![inner],
            span: syn(),
        };
        match convert_block(&outer, 0) {
            NativeBlock::SectionContainer {
                headline,
                children,
                ..
            } => {
                assert_eq!(headline, Some("Outer".to_string()));
                assert_eq!(children.len(), 1);
                match &children[0] {
                    NativeBlock::SectionContainer {
                        headline: inner_hl, ..
                    } => {
                        assert_eq!(*inner_hl, Some("Inner".to_string()));
                    }
                    other => panic!("Expected inner SectionContainer, got {:?}", other),
                }
            }
            other => panic!("Expected outer SectionContainer, got {:?}", other),
        }
    }

    #[test]
    fn to_native_blocks_with_new_variants() {
        let doc = SurfDoc {
            front_matter: None,
            blocks: vec![
                Block::Form {
                    fields: vec![FormField {
                        label: "Email".to_string(),
                        name: "email".to_string(),
                        field_type: FormFieldType::Email,
                        required: true,
                        placeholder: None,
                        options: vec![],
                    }],
                    submit_label: Some("Subscribe".to_string()),
                    action: None, method: None, honeypot: false,
                    span: syn(),
                },
                Block::Gallery {
                    items: vec![GalleryItem {
                        src: "img.png".to_string(),
                        caption: None,
                        alt: None,
                        category: None,
                    }],
                    columns: Some(PerClass::uniform(2)),
                    span: syn(),
                },
                Block::Section {
                    bg: Some("dark".to_string()),
                    headline: Some("CTA".to_string()),
                    subtitle: None,
                    content: String::new(),
                    children: vec![Block::Markdown {
                        content: "Sign up now".to_string(),
                        span: syn(),
                    }],
                    span: syn(),
                },
            ],
            source: String::new(),
        };
        let native = to_native_blocks(&doc);
        assert_eq!(native.len(), 3);
        assert!(matches!(&native[0], NativeBlock::Form { .. }));
        assert!(matches!(&native[1], NativeBlock::Gallery { .. }));
        assert!(matches!(&native[2], NativeBlock::SectionContainer { .. }));
    }

    // ── GFM pipe-table expansion ──────────────────────────────────────

    #[test]
    fn pipe_table_becomes_data_table() {
        // A markdown block containing a GFM pipe table must lift the table out
        // into a DataTable with the right headers and rows (the bug fix).
        let src = "| Name | Role |\n|------|------|\n| Ada | Eng |\n| Bo | PM |\n";
        let blocks = to_native_blocks(&crate::parse(src).doc);
        let table = blocks
            .iter()
            .find_map(|b| match b {
                NativeBlock::DataTable { headers, rows, .. } => Some((headers, rows)),
                _ => None,
            })
            .expect("a DataTable block");
        assert_eq!(table.0, &vec!["Name".to_string(), "Role".to_string()]);
        assert_eq!(
            table.1,
            &vec![
                vec!["Ada".to_string(), "Eng".to_string()],
                vec!["Bo".to_string(), "PM".to_string()],
            ]
        );
    }

    #[test]
    fn prose_around_table_is_split_out() {
        let src = "Intro paragraph.\n\n| A | B |\n|:-:|--:|\n| 1 | 2 |\n\nOutro paragraph.\n";
        let blocks = expand_markdown_tables(src);
        assert_eq!(blocks.len(), 3, "intro / table / outro: {blocks:?}");
        assert!(
            matches!(&blocks[0], NativeBlock::Markdown { content } if content.contains("Intro"))
        );
        assert!(matches!(&blocks[1], NativeBlock::DataTable { .. }));
        assert!(
            matches!(&blocks[2], NativeBlock::Markdown { content } if content.contains("Outro"))
        );
    }

    #[test]
    fn ragged_rows_are_padded_and_truncated() {
        let src = "| A | B | C |\n|---|---|---|\n| 1 | 2 |\n| 1 | 2 | 3 | 4 |\n";
        let blocks = expand_markdown_tables(src);
        match &blocks[0] {
            NativeBlock::DataTable { headers, rows, .. } => {
                assert_eq!(headers.len(), 3);
                assert_eq!(rows[0], vec!["1", "2", ""]); // padded
                assert_eq!(rows[1], vec!["1", "2", "3"]); // truncated
            }
            other => panic!("expected DataTable, got {other:?}"),
        }
    }

    #[test]
    fn headerless_border_pipes_optional() {
        // No leading/trailing border pipes — still a valid GFM table.
        let src = "Name | Score\n---- | -----\nAda | 99\n";
        let blocks = expand_markdown_tables(src);
        assert!(matches!(&blocks[0], NativeBlock::DataTable { headers, .. }
            if headers == &vec!["Name".to_string(), "Score".to_string()]));
    }

    #[test]
    fn escaped_pipe_inside_cell_is_literal() {
        let src = "| Expr | Note |\n|------|------|\n| a \\| b | or |\n";
        let blocks = expand_markdown_tables(src);
        match &blocks[0] {
            NativeBlock::DataTable { rows, .. } => {
                assert_eq!(rows[0], vec!["a | b".to_string(), "or".to_string()]);
            }
            other => panic!("expected DataTable, got {other:?}"),
        }
    }

    #[test]
    fn pipe_text_without_delimiter_stays_markdown() {
        // A line with pipes but NO delimiter row is plain prose, not a table.
        let src = "Use the `cat foo | grep bar` pattern to filter.\n";
        let blocks = to_native_blocks(&crate::parse(src).doc);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], NativeBlock::Markdown { content }
            if content.contains("cat foo | grep bar")));
        assert!(!blocks
            .iter()
            .any(|b| matches!(b, NativeBlock::DataTable { .. })));
    }

    #[test]
    fn table_terminated_by_blank_line() {
        let src = "| A | B |\n|---|---|\n| 1 | 2 |\n\nAfter.\n";
        let blocks = expand_markdown_tables(src);
        match &blocks[0] {
            NativeBlock::DataTable { rows, .. } => assert_eq!(rows.len(), 1),
            other => panic!("expected DataTable, got {other:?}"),
        }
        assert!(
            matches!(blocks.last(), Some(NativeBlock::Markdown { content }) if content.contains("After"))
        );
    }

    /// SS-1 (0.9.3): the grown WsTokens contract projects additively into
    /// NativeTheme. Default (surf) theme: identity var() chains resolve to
    /// the same point values the CSS fallbacks produce, and CSS-recipe
    /// tokens cross as strings untouched.
    #[test]
    fn native_theme_projects_ss1_tokens_default_theme() {
        let t = crate::resolve::resolve_theme(None, None, None);
        let n = NativeTheme::from(&t);
        // Radii chain to the pack's base values (surf: btn 10, card 16).
        assert_eq!(n.hero_btn_radius, 10.0);
        assert_eq!(n.cta_radius, 10.0);
        assert_eq!(n.feature_card_radius, 16.0);
        assert_eq!(n.post_card_radius, 16.0);
        assert_eq!(n.doc_page_radius, 16.0);
        // Per-element fallback chains (forms 2px, app controls 8px, pg 20px).
        assert_eq!(n.banner_btn_radius, 2.0);
        assert_eq!(n.form_submit_radius, 2.0);
        assert_eq!(n.details_radius, 2.0);
        assert_eq!(n.control_radius, 8.0);
        assert_eq!(n.pg_card_radius, 20.0);
        // Square-by-spec tiles and the 1.5rem pad (24pt).
        assert_eq!(n.pg_tile_radius, 0.0);
        assert_eq!(n.feature_card_pad, 24.0);
        // Recipe tokens cross as CSS strings.
        assert_eq!(n.feature_card_hover_transform, "translateY(-2px)");
        assert_eq!(n.tile_surface_bg, "var(--surface)");
        assert_eq!(n.details_bg, "var(--surface-alt)");
        assert_eq!(n.doc_page_bg, "var(--surface)");
        assert_eq!(n.drawer_link_size, "0.9375rem");
        assert_eq!(n.drawer_link_weight, "500");
        // 0.17: the NativeBlock shape grew the Messages mockup-fidelity
        // round (chat-thread message children, chipInput kind, row
        // avatar/rtime/unread-count) — schema v4.
        // 0.18: the size-class axis + the FFI holes it closed — schema v5.
        assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 5);
    }

    /// SS-1: px overrides parse to points and pill radii (999) survive the
    /// crossing; comic identity chains follow comic's own base radii.
    #[test]
    fn native_theme_ss1_px_overrides_and_pills_parse() {
        let mut tokens = crate::resolve::SURF_SIMPLE_TOKENS.clone();
        tokens.hero_btn_radius = "999px";
        tokens.form_submit_radius = "999px";
        tokens.control_radius = "12px";
        tokens.pg_tile_radius = "16px";
        tokens.feature_card_pad = "32px";
        let mut t = crate::resolve::resolve_theme(None, None, None);
        t.tokens = tokens;
        let n = NativeTheme::from(&t);
        assert_eq!(n.hero_btn_radius, 999.0, "pill radius must survive as 999");
        assert_eq!(n.form_submit_radius, 999.0);
        assert_eq!(n.control_radius, 12.0);
        assert_eq!(n.pg_tile_radius, 16.0);
        assert_eq!(n.feature_card_pad, 32.0);
        // Comic leaves SS-1 tokens at identity → chains follow comic radii.
        let comic = crate::resolve::resolve_theme(None, None, Some("comic"));
        let nc = NativeTheme::from(&comic);
        assert_eq!(nc.hero_btn_radius, 6.0, "comic hero buttons chain to its 6px btn radius");
        assert_eq!(nc.feature_card_radius, 4.0, "comic cards chain to its 4px card radius");
    }

    #[test]
    fn table_inside_section_children_expands() {
        // Tables nested in a container (e.g. ::section) must also expand.
        let block = Block::Section {
            bg: None,
            headline: None,
            subtitle: None,
            content: String::new(),
            children: vec![Block::Markdown {
                content: "| A | B |\n|---|---|\n| 1 | 2 |".to_string(),
                span: syn(),
            }],
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::SectionContainer { children, .. } => {
                assert!(
                    children
                        .iter()
                        .any(|c| matches!(c, NativeBlock::DataTable { .. })),
                    "section child table should expand: {children:?}"
                );
            }
            other => panic!("expected SectionContainer, got {other:?}"),
        }
    }
}

//! Native block renderer for mobile/desktop native rendering via UniFFI.
//!
//! Converts a `SurfDoc` into a flat `Vec<NativeBlock>` suitable for export
//! across the FFI boundary. Wavesite-specific block types (Site, Page, Nav,
//! HeroImage, Footer, Embed, PricingTable) are now native, and schema v7
//! added the last eight that were borrowed or degraded (Style, Logo, Route,
//! Action, Model, App, SegmentedControl, DropdownSelect), schema v8 the
//! manifest's children and three widgets, and schema v9 the ten infra blocks
//! of the app-format manifest (Concurrency, Crates, Dashboard,
//! InfraDatabase, Deploy, DeployUrls, Domains, Editor, InfraEnv, Feed), and
//! schema v10 the last twenty: the six web-only blocks that still degraded
//! (Health, Hours, Marquee, Smoke, Use, Volumes) and the fourteen blocks
//! that were `planned` until 0.25.0 (Related, Turn, Timeline, Output,
//! AiGenerated, Alternatives, AiContext, Countdown, Css, Footnote, Kernel,
//! LogoCloud, Subscribe, Notes). Only `Unknown` (and `Deck`, a config
//! block) still degrade to a markdown string.
//!
//! # `NativeBlock` variant ledger
//!
//! The prose that used to sit on the enum's variants and fields lives here
//! since 0.23.0 (session 9, lane 0, D-S9-1): uniffi 0.28.3 packs every `///`
//! inside a `derive(uniffi::Enum)` into ONE 16 KiB metadata buffer
//! (`uniffi_core::metadata::BUF_SIZE`), and the enum's docstrings measured
//! about 9 KB of it at schema v7. Module docs cost the buffer nothing. Each
//! variant keeps a one-line `/// ::block` inside the enum; the record types
//! keep their own field docs (their buffers are nearly empty).
//!
//! - **`Markdown`**
//!   Plain markdown text. Also the fallback for unsupported block types.
//! - **`Callout`**
//!   Callout/admonition box with colored border.
//!   `callout_type` is one of: "info", "warning", "danger", "tip", "note", "success".
//! - **`Code`**
//!   Fenced code block with optional language tag and file path.
//! - **`DataTable`**
//!   Structured data table with headers and rows.
//!   - `caption`: Table caption (schema v6).
//!   - `total`: Summary row rendered under the body (schema v6).
//! - **`Tasks`**
//!   Task checklist with checkbox items.
//! - **`Decision`**
//!   Decision record.
//!   `status` is one of: "proposed", "accepted", "rejected", "superseded".
//! - **`Metric`**
//!   Single metric display with trend indicator.
//!   `trend` is one of: "up", "down", "flat", or None.
//!   - `min`: Gauge floor / ceiling (schema v6). With `max` set the metric draws
//!     as a gauge natively.
//! - **`Summary`**
//!   Executive summary box.
//! - **`Figure`**
//!   Image with optional caption and alt text.
//! - **`Tabs`**
//!   Tabbed content panels (renders as segmented picker or TabView).
//! - **`Columns`**
//!   Multi-column layout.
//! - **`Quote`**
//!   Attributed quote with optional source.
//! - **`Cta`**
//!   Call-to-action button/link.
//! - **`Testimonial`**
//!   Customer testimonial with author info.
//! - **`Faq`**
//!   FAQ accordion with question/answer pairs.
//! - **`Details`**
//!   Collapsible content section.
//! - **`Divider`**
//!   Thematic divider with optional label.
//! - **`Hero`**
//!   Hero section — headline + subtitle + optional badge, optional
//!   banner image, alignment hint (`left` / `center` / `right`), a
//!   list of action buttons, and free-form body content that renders
//!   between the subtitle and the buttons on the web.
//! - **`Features`**
//!   Feature card grid.
//!   - `cols`: Per-size-class column count (schema v5); `None` = client default.
//! - **`Steps`**
//!   Numbered process/timeline steps.
//! - **`Stats`**
//!   Row of stat cards.
//! - **`Comparison`**
//!   Feature comparison matrix.
//! - **`Toc`**
//!   Table of contents with navigation entries.
//! - **`BeforeAfter`**
//!   Before/After comparison visualization.
//! - **`Pipeline`**
//!   Pipeline flow with labeled steps.
//! - **`Form`**
//!   Form with typed input fields for native rendering.
//!   No action URL — the native app controls form submission.
//! - **`Gallery`**
//!   Image gallery with grid layout and optional category filtering.
//!   - `columns`: Per-size-class column count (schema v5); a document that authored
//!     a single value carries the same number in all three fields.
//! - **`SectionContainer`**
//!   Page section container with optional background and headline.
//!   This is the only recursive NativeBlock variant — `children` contains
//!   nested NativeBlock values. UniFFI supports recursive enums via boxing.
//! - **`AppShell`**
//!   Application shell with layout mode and nested children.
//!   `layout` is one of: "sidebar", "split", "tabs".
//!   - `adaptive`: Present only for `layout == "adaptive"` (schema v5).
//! - **`Sidebar`**
//!   Collapsible sidebar navigation panel.
//!   `position` is one of: "left", "right".
//!   - `width`: Per-size-class since schema v5.
//! - **`Panel`**
//!   Resizable panel (bottom or side).
//!   `position` is one of: "bottom", "right", "left".
//!   - `desktop_only`: DEPRECATED at schema v5 — read `gate` instead.
//! - **`TabBar`**
//!   Tab strip navigation bar with selectable items.
//! - **`TabContent`**
//!   Content pane associated with a specific tab.
//!   - `width`: Content-column width cap, per size class. Reached HTML from 0.13
//!     but died at the FFI until schema v5.
//!   - `align`: Horizontal alignment of the capped column ("center"). Same hole.
//! - **`Toolbar`**
//!   Horizontal toolbar with buttons, separators, badges, dropdowns.
//!   - `title`: Static toolbar/screen title (0.12).
//!   - `title_source`: Source-bound dynamic title — a registry name the client
//!     resolves at render time (e.g. `thread.display_name`). (0.12)
//! - **`Drawer`**
//!   Slide-out drawer panel.
//!   `position` is one of: "left", "right".
//!   - `width`: Per-size-class since schema v5.
//! - **`Modal`**
//!   Dialog overlay / modal.
//! - **`CommandPalette`**
//!   Searchable command palette / picker.
//! - **`CodeEditor`**
//!   Syntax-highlighted code editor.
//! - **`BlockEditor`**
//!   Visual block editor mount point.
//! - **`Terminal`**
//!   Shell/terminal panel.
//! - **`NavTree`**
//!   File/navigation tree.
//! - **`Badge`**
//!   Status badge pill.
//! - **`SuggestionChips`**
//!   Clickable suggestion chip list.
//! - **`ChatThread`**
//!   Chat conversation thread display.
//!   - `on_react`: Reaction/tapback seam (0.12).
//!   - `on_doc_open`: Doc-chip open seam (0.12).
//!   - `messages`: Authored message children (0.17); empty = registry-bound thread.
//! - **`ChatInputSimple`**
//!   Simple chat message input.
//! - **`ChipInput`**
//!   Recipient chip input (0.17) — the compose "To:" line: label,
//!   removable chips, inline filter input.
//! - **`Progress`**
//!   Step/progress indicator.
//!   - `value`: Numeric mode (schema v6): with `value` set the block is a
//!     determinate bar and `steps` is empty.
//! - **`LogStream`**
//!   Live log output stream.
//! - **`ProblemList`**
//!   Error/warning problem list.
//! - **`List`**
//!   Data-bound list view (::list).
//!   `display` is one of: "card", "table", "compact".
//!   - `filters`: Filterable field names declared on the list.
//!   - `stream`: Stream-seam event name the list live-updates on (0.12).
//!   - `on_select`: Primary row-select action (0.12).
//! - **`Board`**
//!   Kanban board with cards grouped into columns (::board).
//! - **`FilterBar`**
//!   Filter controls for data views (::filter-bar).
//! - **`Search`**
//!   Search input with typeahead results (::search).
//! - **`RecipientPicker`**
//!   Recipient picker (::recipient-picker) — choose one or more entries
//!   from a data source and submit the selection (group compose).
//!   `mode` is one of: "single", "multi".
//! - **`Qr`**
//!   Platform-conditional QR block (::qr) — show-my-code or scan.
//!   `mode` is one of: "show", "scan"; `on_resolve` fires with the
//!   resolved payload after a successful scan/exchange.
//! - **`Site`**
//!   Site-level configuration block (::site).
//!
//!   Flattens the `{key: value}` properties vec into a handful of
//!   well-known fields used for native theming + chrome. Any additional
//!   keys live in `extras` as `"{key}={value}"` strings.
//! - **`Page`**
//!   Single-page-app style page container (::page).
//!
//!   `children` holds the parsed body of the page; the native renderer
//!   walks them recursively. The web renderer maps one `::page` to one
//!   route; native renderers can either render all pages stacked and
//!   scroll between them, or show one at a time.
//! - **`Nav`**
//!   Navigation bar (::nav) with logo + labelled links.
//! - **`HeroImage`**
//!   Hero image (::hero-image) — full-bleed illustrative image.
//! - **`Footer`**
//!   Footer (::footer) with link sections, copyright, social icons.
//! - **`Embed`**
//!   External embed (::embed) — map / video / audio / generic iframe.
//!   `embed_type` is one of: "map", "video", "audio", "generic".
//! - **`PricingTable`**
//!   Pricing comparison table (::pricing-table).
//!   - `highlight`: Tier names to feature / mark as the viewer's own (schema v6).
//! - **`ProductCard`**
//!   Product/pricing card (::product-card) — title, optional subtitle and
//!   badge, body prose, feature bullets, and an optional CTA.
//!   - `price`: Price as authored, plus its currency code (schema v6).
//! - **`Chart`**
//!   Chart (::chart). `chart_type` is the chart kind (line/bar/pie/…),
//!   `source` names the data series for live-data mount points. `scene`
//!   carries the typed chart geometry for blocks with an inline dataset
//!   (same layout math as the web SVG); source-only charts stay `None`
//!   and keep the labelled-preview path.
//! - **`Row`**
//!   Compact navigable list row (::row) — icon + title + description, an
//!   optional link target, and a `state` of "default"/"loading"/"empty".
//!   `actions` (0.12) carries the per-row labelled action seam
//!   (contact rows, accept/deny request rows).
//!   - `avatar`: Avatar spec (0.17): initials text or "group" for the users
//!     glyph; `auto` is already derived to initials at parse.
//!   - `rtime`: Right-side bucketed relative-time meta (0.17).
//!   - `unread_count`: Unread count pill (0.17); replaces the dot when present.
//!   - `actions`: Labelled per-row actions, typed through the action grammar (0.12).
//! - **`InfoCard`**
//!   Rich entity card (::info-card / ::infocard) — an intent badge, title +
//!   subtitle, a summary line, an optional image, and EITHER numbered steps
//!   OR a label/value fact list. `state` is "default"/"loading"/"empty".
//! - **`Diagram`**
//!   Diagram (::diagram) — `diagram_type` is e.g. "architecture"/"erd".
//!   `scene` carries the laid-out geometry (same layout the web SVG is
//!   serialized from) so native clients draw typed shapes; it is `None`
//!   when the DSL fails to parse, and the raw `content` remains for the
//!   titled-card fallback either way.
//! - **`Banner`**
//!   Full-width banner strip (::banner) — headline + subtitle + action
//!   buttons over free-form body content. `anchor_id` is the optional
//!   in-page anchor (`#contact`).
//! - **`Cite`**
//!   A single reference definition (::cite). Renders as nothing or as a
//!   compact reference chip; the formatted entry is resolved in Rust with
//!   the document's active citation style so clients never reimplement
//!   citation formatting.
//! - **`Bibliography`**
//!   Rendered reference list (::bibliography / ::references). Entries are
//!   pre-formatted in Rust (same string on every platform) in the active
//!   citation style, numbered/ordered per that style's rules.
//! - **`Gate`**
//!   Access-code card (::gate) — password field + submit button. The
//!   native app controls submission (like `Form`); `action` names the
//!   POST target for the client to bind.
//! - **`ProductGrid`**
//!   Grid of product link-cards (::product-grid), optionally grouped.
//!   `tiles` selects the full-bleed promo-tile rendering.
//!   - `cols`: Block-level per-size-class column count (schema v5). Per-group
//!     `cols` on [`NativeProductGroup`] still wins locally.
//! - **`PostGrid`**
//!   Card grid for a blog/news/events index (::post-grid).
//! - **`Slide`**
//!   Presentation slide (::slide) rendered outside the deck renderer.
//!   `layout` is the SlideLayout css-class token ("cover", "bullets", …).
//!   Recursive like `SectionContainer` (UniFFI boxes recursive enums).
//! - **`SplitPane`**
//!   Resizable side-by-side layout (::split-pane) with left/right planes.
//!   `back_label` / `back_action` drive the small-screen back control in
//!   the right plane. Recursive like `SectionContainer` and `Slide`
//!   (UniFFI boxes recursive enums).
//! - **`Style`**
//!   `::style` — `properties` = the `key: value` body lines (`accent`,
//!   `font`, `heading-font`, `body-font`), already in [`NativeTheme`].
//! - **`DropdownSelect`**
//!   `::dropdown-select` — `label=`/`icon=`/`selected=`/`align=` trigger,
//!   `options` = the `- "Label" description= icon= action=` lines.
//! - **`SegmentedControl`**
//!   `::segmented-control` — filter pills, not tabs: `active=`, `size=`,
//!   `action=` (via [`parse_native_action`]), `- id "Label"` segments.
//! - **`Route`**
//!   `::route` — `method=` uppercased, `path=`, the `auth:`/`returns:`/
//!   `body:` lines, `handler` = fenced source, `content` = the rest.
//! - **`Action`**
//!   `::action` — `method=` uppercased, `target=`, `label=`, `confirm=`;
//!   `fields` map exactly as `::form`'s `- Label (type)` lines.
//! - **`Model`**
//!   `::model` — `name=` and the `- field: type [constraints]` lines
//!   (see [`NativeModelField`]).
//! - **`App`**
//!   `::app` — `name=`/`binary=`/`region=`/`port=`/`platform=`/`auth=`,
//!   `content` = raw body, `children` = the parsed child blocks.
//! - **`Logo`**
//!   `::logo` — `src=`, `alt=`, `size=` in pixels.
//! - **`Auth`**
//!   `::auth` — `provider=` as the parser's lowercase word (`email`, `oauth`,
//!   `api-key`, `token`); `session`, `roles` (comma list) and `default_role`
//!   from the `key: value` body lines (attributes accepted since 0.23.0).
//!   Schema v8 (session 9).
//! - **`AppDeploy`**
//!   `::app-deploy` — `region=`, `scale=`, `domain=`, `memory=`; `properties` =
//!   the remaining `key: value` body lines as [`NativeStyleProperty`] (the
//!   Rust `Vec<(String, String)>` tuple cannot cross UniFFI, D-S9-3). v8.
//! - **`Booking`**
//!   `::booking` — `title=`, `service-label=`; `services` = the
//!   `- service: Name | duration | price` lines ([`NativeBookingService`]),
//!   `days` = the `- day: YYYY-MM-DD | slot, slot` lines ([`NativeBookingDay`],
//!   `full`/`none`/empty = no slots). v8.
//! - **`Schema`**
//!   `::schema` — `name=`; `fields` = the `- name (type) constraint…` body
//!   lines (`enum:a,b`, `ref:Model`, `min:1`, `default:x` — the schema
//!   dialect) as [`NativeModelField`], spelled the MODEL way (`enum(a, b)`,
//!   `min=1`) because a schema field IS a model field (D-S9-2). v8.
//! - **`ChatInput`**
//!   `::chat-input` — `action=` (through `validate_source_path`; an external
//!   target arrives blank), `placeholder=`, `modes` = the `modes: a | b` body
//!   line (or the attribute since 0.23.0). NOT `::chat-input-simple`, which is
//!   `ChatInputSimple`. v8.
//! - **`Store`**
//!   `::store` — `title=`, `currency=`; `items` = the
//!   `- item: Name | price | blurb | badge` lines under `- category: …`
//!   lines ([`NativeStoreItem`]). v8.
//! - **`AppEnv`**
//!   `::app-env` — `vars` = the `- NAME * "description"` body lines
//!   ([`NativeEnvVar`]; `*` = required); attributes alone name variables
//!   with no description. v8.
//! - **`Binding`**
//!   `::binding` — `source=`, `target=`; `events` = the `event: action` body
//!   lines ([`NativeBindingEvent`]). v8.
//! - **`Build`**
//!   `::build` — `base=`, `runtime=`, `edition=`; `properties` = the remaining
//!   `key: value` body lines ([`NativeStyleProperty`]). v8.
//! - **`Cicd`**
//!   `::cicd` — `provider=`; `properties` = the `key: value` body lines
//!   ([`NativeStyleProperty`]). v8.
//! - **`Concurrency`**
//!   `::concurrency` — `type=` (crossing as `concurrency_type`),
//!   `hard_limit=`, `soft_limit=`, `force_https=`; attributes only, no
//!   body. Schema v9 (session 10, the tail: the ten web-only infra blocks of
//!   the app-format manifest that no document uses yet).
//! - **`Crates`**
//!   `::crates` — `entries` = the `name (github: owner/repo, features: a b,
//!   branch: x)` or bare `name` body lines ([`NativeCrateEntry`]; the
//!   parser folds `branch:` and a bare continuation into `source`). v9.
//! - **`Dashboard`**
//!   `::dashboard` — `source=` (through `validate_source_path`; an external
//!   target arrives blank), `refresh=` in seconds. The kit polls nothing. v9.
//! - **`InfraDatabase`**
//!   `::database` — `name=`, `shared_auth=`, `volume_gb=`; `properties` =
//!   the `key: value` body lines ([`NativeStyleProperty`]). The Rust name
//!   keeps the parser's `Infra` prefix (grep-able against
//!   `Block::InfraDatabase`); the serde tag is the spec's `database`. v9.
//! - **`Deploy`**
//!   `::deploy` — `env=`, `app=`, `machines=`, `memory=` (MB), `auto_stop=`,
//!   `min_machines=`, `strategy=`; `properties` = the `key: value` body
//!   lines ([`NativeStyleProperty`]). NOT `::app-deploy`, which is
//!   `AppDeploy`. v9.
//! - **`DeployUrls`**
//!   `::deploy-urls` — `entries` = the `env: url` body lines
//!   ([`NativeStyleProperty`], key = env, value = url); no attributes. v9.
//! - **`Domains`**
//!   `::domains` — `entries` = the `domain (description)` or bare domain
//!   body lines ([`NativeDomainEntry`]); no attributes. v9.
//! - **`Editor`**
//!   `::editor` — `source=` (validated; external arrives absent), `lang=`,
//!   `preview=`. The editor MOUNT POINT of the app format — not
//!   `CodeEditor` or `BlockEditor`, which are the two editors. v9.
//! - **`InfraEnv`**
//!   `::env` — `tier=`; `entries` = the `NAME=default` or bare `NAME` body
//!   lines ([`NativeEnvEntry`]). NOT `::app-env`, which is `AppEnv` over
//!   [`NativeEnvVar`]. The serde tag is the spec's `env`. v9.
//! - **`Feed`**
//!   `::feed` — `source=` (validated; external arrives blank), `stream=`
//!   (SSE vs polling). The kit streams nothing. v9.
//! - **`Health`**
//!   `::health` — `path=`, `method=`, `grace=`, `interval=`, `timeout=`;
//!   attributes only, every one optional. Schema v10 (sessions 11 + 12, the
//!   last twenty: the six web-only blocks and the fourteen planned ones).
//! - **`Hours`**
//!   `::hours` — `title=`, `timezone=` (an IANA name, never interpreted);
//!   `rows` = one [`NativeHoursRow`] per authored weekday (`day` 0 = Sunday,
//!   `opens`/`closes` in minutes since local midnight, `text` verbatim). The
//!   block carries no clock — the client stamps "open now" itself. v10.
//! - **`Marquee`**
//!   `::marquee` — `items` = the ticker's lines. v10.
//! - **`Smoke`**
//!   `::smoke` — `script=`; `checks` = the `METHOD /path -> STATUS` body
//!   lines ([`NativeSmokeCheck`]). v10.
//! - **`Use`**
//!   `::use` — `crates` = the `- name version [features]` body lines (or the
//!   inline `::use[a, b]` names) as [`NativeCrateDep`]. Not `Crates`, which
//!   is the app format's `::crates`. v10.
//! - **`Volumes`**
//!   `::volumes` — `entries` = the `name -> /mount` body lines
//!   ([`NativeVolumeEntry`]). v10.
//! - **`Related`**
//!   `::related` — `items` = one [`NativeRelatedItem`] per line: `title`
//!   (the `[Title](href)` form's text), `href`, `relation` (the word or note
//!   after the dash, or the `relation:` prefix). v10.
//! - **`Turn`**
//!   `::turn` — `participant=`, `time=` (or `timestamp=`), `model=`; `role`
//!   is RESOLVED (`human` / `ai` / `system` — `types::turn_role`, inferred
//!   from the participant when not authored); `content` the turn's markdown.
//!   v10.
//! - **`Timeline`**
//!   `::timeline` — `title=`; `entries` = [`NativeTimelineEntry`] rows
//!   (`when`, `label`, `group` = the `## heading` above). v10.
//! - **`Output`**
//!   `::output` — `for_id` (the `for=` attribute; a keyword in Rust and in
//!   Swift), `timestamp=`, `exit=`, `format=`; `content` verbatim. v10.
//! - **`AiGenerated`**
//!   `::ai-generated` — `model=`, `date=`, `reviewed=`; `content` the
//!   generated markdown. v10.
//! - **`Alternatives`**
//!   `::alternatives` — `headers` and `rows` ([`NativeAlternativeRow`],
//!   one `cells` list per row) of the authored pipe table; the client
//!   colour-codes the verdict column (`Verdict` by name, else the last). v10.
//! - **`AiContext`**
//!   `::ai-context` — `model=`, `tokens=`, `loaded=`; `content` the note. v10.
//! - **`Countdown`**
//!   `::countdown` — `date=`, `label=`; no body. The client counts. v10.
//! - **`Css`**
//!   `::css` — `content` verbatim; the client applies nothing (the web's
//!   escape hatch, carried so the explorer can show it). v10.
//! - **`Footnote`**
//!   `::footnote` — `id=`; `content` the citation's markdown. v10.
//! - **`Kernel`**
//!   `::kernel` — `lang=`, `env=`; `runtime`, `packages` (a list), `sandbox`
//!   from body lines or attributes (body wins); `properties` = every other
//!   `key: value` line ([`NativeStyleProperty`]). v10.
//! - **`LogoCloud`**
//!   `::logo-cloud` — `title=`; `items` = [`NativeLogoItem`] (`src`, `name`
//!   optional — the client derives an alt from the file name). v10.
//! - **`Subscribe`**
//!   `::subscribe` — `action=`, `placeholder=`; `content` the pitch. The
//!   client posts nothing itself. v10.
//! - **`Notes`**
//!   `::notes` standalone (a slide folds its own into `Slide.notes`) —
//!   `content` verbatim. v10.

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

/// The parser's `key: value` lines as the v7 key/value record — the three
/// v9 ledgers (`::database`, `::deploy`, `::deploy-urls`) share it.
fn style_properties(properties: &[crate::types::StyleProperty]) -> Vec<NativeStyleProperty> {
    properties
        .iter()
        .map(|p| NativeStyleProperty {
            key: p.key.clone(),
            value: p.value.clone(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════
// NativeBlock enum — 122 native variants (pinned cross-platform by the
// SurfDocKit DispatchCoverageTests / Android NativeBlockCoverageTest census)
//
// HARD CAP (measured 0.22.0, S8; cleared 0.23.0, S9 lane 0): uniffi 0.28.3
// encodes this enum's whole metadata — module path, every variant and field
// name, every TYPE_ID_META and EVERY `///` docstring (variant AND field) —
// into one 16 KiB const buffer (`uniffi_core::metadata::BUF_SIZE`).
// Overflowing it is a const-eval panic at the `derive(uniffi::Enum)` line,
// not a readable error. At v7 the docstrings alone measured ~9 KB and left
// ~200 bytes of slack; at v8 every variant keeps ONE `/// ::block` line and
// the prose lives in this file's module docs (the variant ledger above, which
// costs the buffer nothing) or on the record types (whose own buffers are
// nearly empty). Keep it that way: a new variant here gets its one line and
// a ledger entry, never a paragraph.
// ═══════════════════════════════════════════════════════════════════════

/// Simplified block representation for native rendering via UniFFI —
/// see the variant ledger in this file's module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NativeBlock {
    /// ::markdown
    Markdown { content: String },

    /// ::callout
    Callout {
        callout_type: String,
        title: Option<String>,
        content: String,
    },

    /// ::code
    Code {
        language: Option<String>,
        file_path: Option<String>,
        content: String,
    },

    /// ::data
    DataTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        sortable: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        caption: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        total: Vec<String>,
    },

    /// ::tasks
    Tasks { items: Vec<NativeTaskItem> },

    /// ::decision
    Decision {
        status: String,
        date: Option<String>,
        deciders: Vec<String>,
        content: String,
    },

    /// ::metric
    Metric {
        label: String,
        value: String,
        trend: Option<String>,
        unit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<String>,
    },

    /// ::summary
    Summary { content: String },

    /// ::figure
    Figure {
        src: String,
        caption: Option<String>,
        alt: Option<String>,
    },

    /// ::tabs
    Tabs { tabs: Vec<NativeTabPanel> },

    /// ::columns
    Columns { columns: Vec<NativeColumnContent> },

    /// ::quote
    Quote {
        content: String,
        attribution: Option<String>,
    },

    /// ::cta
    Cta {
        label: String,
        href: String,
        primary: bool,
    },

    /// ::testimonial
    Testimonial {
        content: String,
        author: Option<String>,
        role: Option<String>,
        company: Option<String>,
    },

    /// ::faq
    Faq { items: Vec<NativeFaqItem> },

    /// ::details
    Details {
        title: Option<String>,
        open: bool,
        content: String,
    },

    /// ::divider
    Divider { label: Option<String> },

    /// ::hero
    Hero {
        headline: Option<String>,
        anchor: Option<String>,
        subtitle: Option<String>,
        badge: Option<String>,
        align: String,
        image: Option<String>,
        buttons: Vec<NativeHeroButton>,
        content: String,
    },

    /// ::features
    Features {
        cards: Vec<NativeFeatureCard>,
        cols: Option<NativePerClassU32>,
    },

    /// ::steps
    Steps { steps: Vec<NativeStepItem> },

    /// ::stats
    Stats { items: Vec<NativeStatItem> },

    /// ::comparison
    Comparison {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        highlight: Option<String>,
    },

    /// ::toc
    Toc {
        depth: u32,
        entries: Vec<NativeTocEntry>,
    },

    /// ::before-after
    BeforeAfter {
        before_items: Vec<NativeBeforeAfterItem>,
        after_items: Vec<NativeBeforeAfterItem>,
        transition: Option<String>,
    },

    /// ::pipeline
    Pipeline { steps: Vec<NativePipelineStep> },

    /// ::form
    Form {
        fields: Vec<NativeFormField>,
        submit_label: String,
    },

    /// ::gallery
    Gallery {
        items: Vec<NativeGalleryItem>,
        columns: NativePerClassU32,
    },

    /// ::section
    SectionContainer {
        bg: Option<String>,
        headline: Option<String>,
        anchor: Option<String>,
        subtitle: Option<String>,
        children: Vec<NativeBlock>,
    },

    // ── Interactive block types (20 new variants) ──────────────────

    // Layout
    /// ::app-shell
    AppShell {
        layout: String,
        adaptive: Option<NativeAdaptiveLayout>,
        children: Vec<NativeBlock>,
    },
    /// ::sidebar
    Sidebar {
        position: String,
        collapsible: bool,
        width: Option<NativePerClassU32>,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },
    /// ::panel
    Panel {
        position: String,
        resizable: bool,
        height: Option<u32>,
        desktop_only: bool,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },

    // Navigation
    /// ::tab-bar
    TabBar {
        active: Option<String>,
        items: Vec<NativeTabBarItem>,
    },
    /// ::tab-content
    TabContent {
        tab: String,
        width: Option<NativePerClassU32>,
        align: Option<String>,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },
    /// ::toolbar
    Toolbar {
        title: Option<String>,
        title_source: Option<String>,
        items: Vec<NativeToolbarItem>,
    },

    // Overlays
    /// ::drawer
    Drawer {
        name: String,
        position: String,
        width: Option<NativePerClassU32>,
        trigger: Option<String>,
        gate: NativeClassGate,
        children: Vec<NativeBlock>,
    },
    /// ::modal
    Modal {
        name: String,
        title: Option<String>,
        children: Vec<NativeBlock>,
    },
    /// ::command-palette
    CommandPalette {
        trigger: Option<String>,
        items: Vec<NativeCommandItem>,
    },

    // Interactive
    /// ::code-editor
    CodeEditor {
        lang: Option<String>,
        source: Option<String>,
        line_numbers: bool,
        content: String,
    },
    /// ::block-editor
    BlockEditor {
        source: Option<String>,
    },
    /// ::terminal
    Terminal {
        shell: Option<String>,
        cwd: Option<String>,
    },

    // Data
    /// ::nav-tree
    NavTree {
        source: Option<String>,
        on_select: Option<NativeAction>,
        on_rename: Option<NativeAction>,
        on_delete: Option<NativeAction>,
    },
    /// ::badge
    Badge {
        value: String,
        color: Option<String>,
    },
    /// ::suggestion-chips
    SuggestionChips {
        source: Option<String>,
        max: Option<u32>,
        dismissible: bool,
    },
    /// ::chat-thread
    ChatThread {
        source: Option<String>,
        on_action: Option<NativeAction>,
        on_react: Option<NativeAction>,
        on_doc_open: Option<NativeAction>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        messages: Vec<NativeChatMessage>,
    },
    /// ::chat-input-simple
    ChatInputSimple {
        placeholder: Option<String>,
        action: Option<NativeAction>,
    },
    /// ::chip-input
    ChipInput {
        label: Option<String>,
        placeholder: Option<String>,
        source: Option<String>,
        on_change: Option<NativeAction>,
        chips: Vec<String>,
    },
    /// ::progress
    Progress {
        source: Option<String>,
        steps: Vec<NativeProgressStep>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<String>,
    },
    /// ::log-stream
    LogStream {
        source: Option<String>,
        tail: Option<u32>,
    },
    /// ::problem-list
    ProblemList {
        source: Option<String>,
    },

    // ── App data views promoted from tier 4 (0.11) ─────────────────

    /// ::list
    List {
        source: String,
        display: String,
        item_template: String,
        filters: Vec<String>,
        sort_field: Option<String>,
        sort_descending: bool,
        preload: bool,
        stream: Option<String>,
        on_select: Option<NativeAction>,
    },
    /// ::board
    Board {
        source: String,
        columns: Vec<String>,
        card_template: Option<String>,
        preload: bool,
    },
    /// ::filter-bar
    FilterBar {
        target_selector: String,
        fields: Vec<NativeFilterField>,
    },
    /// ::search
    Search {
        source: String,
        placeholder: Option<String>,
    },

    // ── Messages/Contacts vocabulary (0.12) ────────────────────────

    /// ::recipient-picker
    RecipientPicker {
        source: String,
        mode: String,
        on_submit: Option<NativeAction>,
    },
    /// ::qr
    Qr {
        mode: String,
        on_resolve: Option<NativeAction>,
    },

    // ── Wavesite site-format variants (7 new) ──────────────────────

    /// ::site
    Site {
        name: Option<String>,
        description: Option<String>,
        accent: Option<String>,
        font: Option<String>,
        domain: Option<String>,
        extras: Vec<String>,
    },

    /// ::page
    Page {
        route: String,
        title: Option<String>,
        layout: Option<String>,
        children: Vec<NativeBlock>,
    },

    /// ::nav
    Nav {
        logo: Option<String>,
        items: Vec<NativeNavItem>,
    },

    /// ::hero-image
    HeroImage {
        src: String,
        alt: Option<String>,
    },

    /// ::footer
    Footer {
        copyright: Option<String>,
        sections: Vec<NativeFooterSection>,
        social: Vec<NativeSocialLink>,
    },

    /// ::embed
    Embed {
        src: String,
        title: Option<String>,
        embed_type: String,
    },

    /// ::pricing-table
    PricingTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highlight: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current: Option<String>,
    },

    /// ::product-card
    ProductCard {
        title: String,
        subtitle: Option<String>,
        badge: Option<String>,
        badge_color: Option<String>,
        body: String,
        features: Vec<String>,
        cta_label: Option<String>,
        cta_href: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        currency: Option<String>,
    },

    /// ::chart
    Chart {
        chart_type: String,
        source: String,
        period: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scene: Option<NativeDiagramScene>,
    },

    /// ::row
    Row {
        icon: String,
        title: String,
        description: String,
        href: Option<String>,
        state: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rtime: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unread_count: Option<u32>,
        actions: Vec<NativeRowAction>,
    },

    /// ::infocard
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

    /// ::diagram
    Diagram {
        diagram_type: String,
        title: Option<String>,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scene: Option<NativeDiagramScene>,
    },

    // ── FFI-hole closure (0.11): tier-1–3 kinds that previously degraded ──

    /// ::banner
    Banner {
        headline: Option<String>,
        subtitle: Option<String>,
        anchor_id: Option<String>,
        buttons: Vec<NativeHeroButton>,
        content: String,
    },

    /// ::cite
    Cite {
        key: String,
        formatted: String,
    },

    /// ::bibliography
    Bibliography {
        heading: String,
        entries: Vec<NativeReferenceEntry>,
    },

    /// ::gate
    Gate {
        title: Option<String>,
        subtitle: Option<String>,
        action: String,
        field_label: Option<String>,
        submit_label: Option<String>,
        error: Option<String>,
    },

    /// ::product-grid
    ProductGrid {
        tiles: bool,
        cols: Option<NativePerClassU32>,
        groups: Vec<NativeProductGroup>,
    },

    /// ::post-grid
    PostGrid {
        title: Option<String>,
        subtitle: Option<String>,
        items: Vec<NativePostItem>,
    },

    /// ::slide
    Slide {
        layout: String,
        kicker: Option<String>,
        notes: Option<String>,
        children: Vec<NativeBlock>,
    },

    /// ::split-pane
    SplitPane {
        ratio: String,
        back_label: Option<String>,
        back_action: Option<String>,
        left: Vec<NativeBlock>,
        right: Vec<NativeBlock>,
    },

    // ── Schema v7 (0.22.0): the eight blocks that used to cross the FFI
    //    borrowed (SegmentedControl→TabBar, DropdownSelect→CommandPalette)
    //    or as Markdown (Style, Logo, Route, Action, Model, App). ────────

    /// ::style
    Style { properties: Vec<NativeStyleProperty> },

    /// ::dropdown-select
    DropdownSelect {
        label: Option<String>,
        icon: Option<String>,
        selected: Option<String>,
        align: String,
        options: Vec<NativeDropdownOption>,
    },

    /// ::segmented-control
    SegmentedControl {
        active: Option<String>,
        size: String,
        action: Option<NativeAction>,
        segments: Vec<NativeSegmentItem>,
    },

    /// ::route
    Route {
        method: String,
        path: String,
        auth: Option<String>,
        returns: Option<String>,
        body: Option<String>,
        handler: Option<String>,
        content: String,
    },

    /// ::action
    Action {
        method: String,
        target: String,
        label: String,
        fields: Vec<NativeFormField>,
        confirm: Option<String>,
    },

    /// ::model
    Model {
        name: String,
        fields: Vec<NativeModelField>,
    },

    /// ::app
    App {
        name: String,
        binary: Option<String>,
        region: Option<String>,
        port: Option<u32>,
        platform: Option<String>,
        auth: Option<String>,
        content: String,
        children: Vec<NativeBlock>,
    },

    /// ::logo
    Logo {
        src: String,
        alt: Option<String>,
        size: Option<u32>,
    },

    // ── Schema v8 (0.23.0, session 9): the last ten blocks with measured
    //    use — the app manifest's children and three site widgets. ───────

    /// ::auth
    Auth {
        provider: String,
        session: Option<String>,
        roles: Vec<String>,
        default_role: Option<String>,
    },

    /// ::app-deploy
    AppDeploy {
        region: Option<String>,
        scale: Option<u32>,
        domain: Option<String>,
        memory: Option<String>,
        properties: Vec<NativeStyleProperty>,
    },

    /// ::booking
    Booking {
        title: Option<String>,
        service_label: Option<String>,
        services: Vec<NativeBookingService>,
        days: Vec<NativeBookingDay>,
    },

    /// ::schema
    Schema {
        name: String,
        fields: Vec<NativeModelField>,
    },

    /// ::chat-input
    ChatInput {
        action: String,
        placeholder: Option<String>,
        modes: Vec<String>,
    },

    /// ::store
    Store {
        title: Option<String>,
        currency: Option<String>,
        items: Vec<NativeStoreItem>,
    },

    /// ::app-env
    AppEnv { vars: Vec<NativeEnvVar> },

    /// ::binding
    Binding {
        source: String,
        target: String,
        events: Vec<NativeBindingEvent>,
    },

    /// ::build
    Build {
        base: Option<String>,
        runtime: Option<String>,
        edition: Option<String>,
        properties: Vec<NativeStyleProperty>,
    },

    /// ::cicd
    Cicd {
        provider: Option<String>,
        properties: Vec<NativeStyleProperty>,
    },

    // ── Schema v9 (0.24.0, session 10): the ten web-only infra blocks of
    //    the app-format manifest — used by no document yet. ────────────

    /// ::concurrency
    Concurrency {
        concurrency_type: Option<String>,
        hard_limit: Option<u32>,
        soft_limit: Option<u32>,
        force_https: bool,
    },

    /// ::crates
    Crates { entries: Vec<NativeCrateEntry> },

    /// ::dashboard
    Dashboard {
        source: String,
        refresh: Option<u32>,
    },

    /// ::database
    #[serde(rename = "database")]
    InfraDatabase {
        name: Option<String>,
        shared_auth: bool,
        volume_gb: Option<u32>,
        properties: Vec<NativeStyleProperty>,
    },

    /// ::deploy
    Deploy {
        env: Option<String>,
        app: Option<String>,
        machines: Option<u32>,
        memory: Option<u32>,
        auto_stop: Option<String>,
        min_machines: Option<u32>,
        strategy: Option<String>,
        properties: Vec<NativeStyleProperty>,
    },

    /// ::deploy-urls
    DeployUrls { entries: Vec<NativeStyleProperty> },

    /// ::domains
    Domains { entries: Vec<NativeDomainEntry> },

    /// ::editor
    Editor {
        source: Option<String>,
        lang: Option<String>,
        preview: bool,
    },

    /// ::env
    #[serde(rename = "env")]
    InfraEnv {
        tier: Option<String>,
        entries: Vec<NativeEnvEntry>,
    },

    /// ::feed
    Feed { source: String, stream: bool },

    // ── Schema v10 (0.25.0, sessions 11 + 12): the last twenty — the six
    //    web-only blocks that still degraded, then the fourteen that were
    //    planned until this release. ─────────────────────────────────────

    /// ::health
    Health {
        path: Option<String>,
        method: Option<String>,
        grace: Option<String>,
        interval: Option<String>,
        timeout: Option<String>,
    },

    /// ::hours
    Hours {
        title: Option<String>,
        timezone: Option<String>,
        rows: Vec<NativeHoursRow>,
    },

    /// ::marquee
    Marquee { items: Vec<String> },

    /// ::smoke
    Smoke {
        script: Option<String>,
        checks: Vec<NativeSmokeCheck>,
    },

    /// ::use
    Use { crates: Vec<NativeCrateDep> },

    /// ::volumes
    Volumes { entries: Vec<NativeVolumeEntry> },

    /// ::related
    Related { items: Vec<NativeRelatedItem> },

    /// ::turn
    Turn {
        participant: String,
        time: Option<String>,
        role: String,
        model: Option<String>,
        content: String,
    },

    /// ::timeline
    Timeline {
        title: Option<String>,
        entries: Vec<NativeTimelineEntry>,
    },

    /// ::output
    Output {
        for_id: Option<String>,
        timestamp: Option<String>,
        exit: Option<i32>,
        format: Option<String>,
        content: String,
    },

    /// ::ai-generated
    AiGenerated {
        model: Option<String>,
        date: Option<String>,
        reviewed: bool,
        content: String,
    },

    /// ::alternatives
    Alternatives {
        headers: Vec<String>,
        rows: Vec<NativeAlternativeRow>,
    },

    /// ::ai-context
    AiContext {
        model: Option<String>,
        tokens: Option<u32>,
        loaded: bool,
        content: String,
    },

    /// ::countdown
    Countdown {
        date: Option<String>,
        label: Option<String>,
    },

    /// ::css
    Css { content: String },

    /// ::footnote
    Footnote {
        id: Option<String>,
        content: String,
    },

    /// ::kernel
    Kernel {
        lang: Option<String>,
        env: Option<String>,
        runtime: Option<String>,
        packages: Vec<String>,
        sandbox: Option<String>,
        properties: Vec<NativeStyleProperty>,
    },

    /// ::logo-cloud
    LogoCloud {
        title: Option<String>,
        items: Vec<NativeLogoItem>,
    },

    /// ::subscribe
    Subscribe {
        action: Option<String>,
        placeholder: Option<String>,
        content: String,
    },

    /// ::notes
    Notes { content: String },
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
    /// Fieldset label this field belongs to (schema v6). Consecutive fields
    /// sharing a value belong to one group; `None` is an ungrouped field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
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

/// One `key: value` presentation override inside a native `Style` block —
/// the body lines of `::style` (`accent: #ff0000`, `font: inter`,
/// `heading-font: …`, `body-font: …`). New in schema v7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeStyleProperty {
    /// The override name, left of the colon, verbatim.
    pub key: String,
    /// The override value, right of the colon, verbatim.
    pub value: String,
}

/// One option within a native `DropdownSelect` — a `- "Label"
/// description= icon= action=` body line of `::dropdown-select`.
/// New in schema v7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeDropdownOption {
    /// The option's quoted label.
    pub label: String,
    /// `description=` — the secondary line under the label.
    pub description: Option<String>,
    /// `icon=` — the leading glyph token.
    pub icon: Option<String>,
    /// `action=` parsed through [`parse_native_action`]; `raw` keeps the
    /// authored string so bare-name registries keep working.
    pub action: Option<NativeAction>,
}

/// One segment within a native `SegmentedControl` — a `- id "Label"` body
/// line of `::segmented-control`. New in schema v7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeSegmentItem {
    /// The segment id, matched against the block's `active=`.
    pub id: String,
    /// The segment's display label (the id itself when none was quoted).
    pub label: String,
}

/// One typed field within a native `Model` — a `- name: type [constraints]`
/// body line of `::model`. New in schema v7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeModelField {
    /// The field name, left of the colon.
    pub name: String,
    /// The spec spelling of the field's type: `uuid`, `string`, `int`,
    /// `float`, `bool`, `datetime`, `text`, `json`, `money`, `image`,
    /// `email`, `url`, `enum(a, b)` or `ref(Model)`.
    pub field_type: String,
    /// The spec spellings of the bracketed constraints, in authored order:
    /// `primary`, `auto`, `required`, `optional`, `unique`, `index`,
    /// `max=255`, `min=1`, `default=now()`.
    pub constraints: Vec<String>,
}

/// One bookable service within a native `Booking` — a
/// `- service: Name | duration | price` body line of `::booking`.
/// New in schema v8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeBookingService {
    /// The service name, the first `|` field.
    pub name: String,
    /// Free-text duration (`60 min`), the second field; absent when blank.
    pub duration: Option<String>,
    /// Free-text price (`$120`, `Free`), the third field; absent when blank.
    pub price: Option<String>,
}

/// One day of availability within a native `Booking` — a
/// `- day: YYYY-MM-DD | 9:00 AM, 10:00 AM` body line of `::booking`.
/// New in schema v8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeBookingDay {
    /// The ISO date as authored.
    pub date: String,
    /// The selectable slot labels, in authored order; empty when the day
    /// was written `full`, `none` or with no slots at all.
    pub slots: Vec<String>,
}

/// One product within a native `Store` — a
/// `- item: Name | price | blurb | badge` body line of `::store`, filed
/// under the most recent `- category: …` line. New in schema v8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeStoreItem {
    /// The product name.
    pub name: String,
    /// The price as authored (`48`, `12.50`); the block's `currency=` is
    /// the prefix.
    pub price: String,
    /// The one-line description under the name.
    pub blurb: Option<String>,
    /// The corner badge (`Bestseller`, `New`).
    pub badge: Option<String>,
    /// The category chip the item files under; `None` groups under "All".
    pub category: Option<String>,
}

/// One declared environment variable within a native `AppEnv` — a
/// `- NAME * "description"` body line of `::app-env`. New in schema v8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeEnvVar {
    /// The variable name, the first word of the line.
    pub name: String,
    /// The quoted description, when one was written.
    pub description: Option<String>,
    /// True when the line carried a `*`.
    pub required: bool,
}

/// One event within a native `Binding` — an `event: action` body line of
/// `::binding`. New in schema v8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeBindingEvent {
    /// The event word, left of the colon.
    pub event: String,
    /// The action, right of the colon, verbatim.
    pub action: String,
}

/// One shared crate dependency within a native `Crates` — a
/// `name (github: owner/repo, features: a b, branch: x)` or bare `name`
/// body line of `::crates`. New in schema v9.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeCrateEntry {
    /// The crate name, left of the paren (or the whole line).
    pub name: String,
    /// The `github:`/`source:` value, with any `branch:` folded in.
    pub source: Option<String>,
    /// The `features:` value, verbatim (space-separated as authored).
    pub features: Option<String>,
}

/// One domain within a native `Domains` — a `domain (description)` or
/// bare domain body line of `::domains`. New in schema v9.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeDomainEntry {
    /// The domain, left of the paren (or the whole line).
    pub domain: String,
    /// The parenthesised description, when one was written.
    pub description: Option<String>,
}

/// One variable within a native `InfraEnv` — a `NAME=default` or bare
/// `NAME` body line of `::env`. Not `NativeEnvVar`, which is `::app-env`'s
/// (a description and a required flag, no default). New in schema v9.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeEnvEntry {
    /// The variable name, left of the `=` (or the whole line).
    pub name: String,
    /// The default, right of the `=`; absent for a bare name or a blank value.
    pub default_value: Option<String>,
}

/// One weekday row of a native `Hours` — `types::HoursRow` across the FFI.
/// New in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeHoursRow {
    /// Weekday index, `0` = Sunday … `6` = Saturday.
    pub day: u8,
    /// The day name exactly as authored ("Monday", "Mon").
    pub label: String,
    /// Opening time, minutes since local midnight; absent = closed.
    pub opens: Option<u16>,
    /// Closing time, minutes since local midnight; absent = closed.
    pub closes: Option<u16>,
    /// The authored right-hand text ("11am - 9pm", "Closed"), verbatim.
    pub text: String,
}

/// One check of a native `Smoke` — a `METHOD /path -> STATUS` body line of
/// `::smoke`. New in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeSmokeCheck {
    pub method: String,
    pub path: String,
    /// The expected HTTP status.
    pub expected: u16,
}

/// One dependency of a native `Use` — a `- name version [features]` body
/// line of `::use`. Not `NativeCrateEntry`, which is `::crates`'s. New in
/// schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeCrateDep {
    pub name: String,
    pub version: Option<String>,
    pub features: Vec<String>,
}

/// One mount of a native `Volumes` — a `name -> /mount` body line of
/// `::volumes`. New in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeVolumeEntry {
    pub name: String,
    pub mount: String,
}

/// One cross-reference of a native `Related` — `types::RelatedItem` across
/// the FFI. New in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeRelatedItem {
    /// The `[Title](href)` form's text; absent for a bare path.
    pub title: Option<String>,
    /// The target as authored (a script scheme already replaced by `#`).
    pub href: String,
    /// The relationship word or note.
    pub relation: Option<String>,
}

/// One entry of a native `Timeline` — `types::TimelineEntry` across the
/// FFI. New in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeTimelineEntry {
    /// The date or time as authored; absent for a bare `- label`.
    pub when: Option<String>,
    pub label: String,
    /// The `## heading` the entry sits under.
    pub group: Option<String>,
}

/// One row of a native `Alternatives` table — the cells in header order
/// (padded to the header count). A record rather than a nested list so
/// every UniFFI target names it the same way. New in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeAlternativeRow {
    pub cells: Vec<String>,
}

/// One logo of a native `LogoCloud` — `types::LogoItem` across the FFI. New
/// in schema v10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeLogoItem {
    pub src: String,
    /// The alt text and list name; absent means "derive it from the file".
    pub name: Option<String>,
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
/// v6 (0.18.1) — form vocabulary:
/// 1. `NativeFormField.group` — the fieldset a field belongs to
///    (`group: Label` inside `::form`); `None` on every ungrouped field.
/// 2. `NativeFormField.field_type` gains `checkbox`, `radio`, `toggle`,
///    `file` and `hidden`; clients that match the string exhaustively must
///    add the five cases (Toggle/Switch for `toggle`).
/// 3. `NativeDoc.block_meta` — span-indexed `id=`/`label=` for any block
///    ([`NativeBlockMeta`]); empty on documents that author neither.
/// 4. `NativeBlock::Metric` gains `min`/`max`, `Progress` gains
///    `value`/`max`, `ProductCard` gains `price`/`currency`,
///    `PricingTable` gains `highlight`/`current`, `Data` gains `caption`
///    and `total`.
/// v7 (0.22.0) — the last eight borrowed/degraded blocks get their own
/// variants (S8, the first FFI session):
/// 1. `NativeBlock::SegmentedControl` — was borrowed as `TabBar`, losing
///    `size=` and the block-level `action=`; `NativeBlock::DropdownSelect`
///    — was borrowed as `CommandPalette`, losing `icon=`, `align=` and the
///    label/selected distinction. Clients matching those two variants must
///    stop expecting segmented-control / dropdown payloads there.
/// 2. `NativeBlock::Style`, `Logo`, `Route`, `Action`, `Model` and `App` —
///    were `Markdown` strings; they now carry their parsed shape, with
///    `Style`/`App` filed under Chrome, `Logo` under Site and
///    `Route`/`Action`/`Model` under Content in [`block_tier`].
/// 3. New records `NativeStyleProperty`, `NativeDropdownOption`,
///    `NativeSegmentItem`, `NativeModelField`.
/// 4. `NativeBlock` is now an 83-variant enum (82 structural + `Markdown`).
/// 5. [`NativeTheme`] now carries a document's `::style` overrides: the
///    resolver reads `accent` / `font` / `heading-font` / `body-font` from
///    `::style` body lines (D-S8-5), so a themed `::style` no longer
///    reaches native only through the style pack.
/// v8 (0.23.0) — the last ten blocks with measured use (S9, the second FFI
/// session; every one was a `Markdown` string before):
/// 1. `NativeBlock::Auth`, `AppDeploy`, `AppEnv`, `Binding`, `Build`, `Cicd`
///    — the app manifest's children, filed under Chrome, so an `App`'s
///    `children` are structural now; `ChatInput` (the routed composer,
///    Chrome); `Booking` and `Store` (Site); `Schema` (Content — a schema is
///    a definition table, like a model).
/// 2. New records `NativeBookingService`, `NativeBookingDay`,
///    `NativeStoreItem`, `NativeEnvVar`, `NativeBindingEvent`;
///    `NativeModelField` (schema) and `NativeStyleProperty` (app-deploy,
///    build, cicd) reused.
/// 3. `NativeBlock` is now a 93-variant enum (92 structural + `Markdown`).
///    Its docstrings moved to the module-level variant ledger (lane 0 of
///    S9): the enum's UniFFI metadata buffer is 16 KiB and the prose alone
///    measured ~9 KB at v7.
/// 4. The parser accepts `::style`, `::route`, `::auth` and `::chat-input`
///    keys as ATTRIBUTES as well as body lines (D-S9-5; body lines win) —
///    a parse change, not a schema change, listed here because the corpus
///    snapshot for the manifest fixture moved with it.
/// v9 (0.24.0) — the ten web-only infra blocks of the app-format manifest
/// (S10, the tail opens; every one was a `Markdown` string before, and no
/// document in the company's corpus authors any of them yet):
/// 1. `NativeBlock::Concurrency`, `Crates`, `Dashboard`, `InfraDatabase`,
///    `Deploy`, `DeployUrls`, `Domains`, `Editor`, `InfraEnv`, `Feed` — all
///    filed under Chrome, so every infra block the spec names inside an
///    `App` is structural now; only `Unknown`, `Hours`, `Marquee`,
///    `Health`, `Smoke`, `Volumes`, `Use` and `Deck` stay Degraded.
/// 2. New records `NativeCrateEntry`, `NativeDomainEntry`,
///    `NativeEnvEntry`; `NativeStyleProperty` reused for the database,
///    deploy and deploy-urls ledgers.
/// 3. `InfraDatabase` and `InfraEnv` keep the parser's Rust names and carry
///    the spec's block names as their serde tags (`database`, `env`).
/// 4. `NativeBlock` is now a 103-variant enum (102 structural + `Markdown`).
///    Its docstrings stay in the module ledger (one line per variant inside
///    the enum, the UniFFI metadata buffer measured with room).
/// v10 (0.25.0) — the last twenty (S11 + S12 as one session; the program
/// closes at 121 of 122 components, `::pane` absorbed by `SplitPane`):
/// 1. The six web-only blocks that still degraded — `NativeBlock::Health`,
///    `Smoke`, `Use`, `Volumes` (Chrome) and `Hours`, `Marquee` (Site).
/// 2. The fourteen blocks that were `planned` in `spec/blocks.toml` until
///    this release, now parsed, rendered on the web and crossing
///    structurally — `Related`, `Turn`, `Timeline`, `Output`, `AiGenerated`,
///    `Alternatives`, `AiContext`, `Footnote`, `Notes` (Content),
///    `Countdown`, `LogoCloud`, `Subscribe` (Site), `Css`, `Kernel` (Chrome).
/// 3. New records `NativeHoursRow`, `NativeSmokeCheck`, `NativeCrateDep`,
///    `NativeVolumeEntry`, `NativeRelatedItem`, `NativeTimelineEntry`,
///    `NativeAlternativeRow`, `NativeLogoItem`; `NativeStyleProperty` reused
///    for the kernel's ledger.
/// 4. `Turn.role` crosses RESOLVED (`human` / `ai` / `system`); `Output`'s
///    `for=` crosses as `for_id` (a keyword on both sides of the FFI).
/// 5. `NativeBlock` is now a 123-variant enum (122 structural + `Markdown`);
///    only `Unknown` and `Deck` still convert to a `Markdown` string.
/// v11 (0.26.0) — Khoury's on macOS, L0: `Hero` and `SectionContainer`
/// gain `anchor` (the headline's trailing `{#slug}`, split off the text the
/// way the web's `split_explicit_anchor` does); markdown bodies (`Markdown`,
/// `Columns`) cross with `{#slug}` removed from their ATX headings.
pub const NATIVE_DOC_SCHEMA_VERSION: u32 = 11;

/// One block's authored addressing attributes, keyed by source span.
///
/// `NativeBlock` is a 123-variant enum, so `block_id`/`label` cannot be flat
/// fields on it; the metadata rides beside the tree instead, indexed by the
/// same `Span` byte extent the HTML renderer uses. Empty for a document that
/// authored no `id=`/`label=`. New in schema v6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct NativeBlockMeta {
    /// 1-based first line of the block directive.
    pub start_line: u32,
    /// 1-based last line of the block directive (inclusive).
    pub end_line: u32,
    /// 0-based byte offset of the directive's first character.
    pub start_offset: u32,
    /// 0-based byte offset past the directive's last character.
    pub end_offset: u32,
    /// Authored `id=` (`data-block-id` in HTML).
    pub block_id: Option<String>,
    /// Authored `label=` (`aria-label` in HTML); `None` on the directives that
    /// spend `label=` on their own semantics.
    pub label: Option<String>,
}

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
    /// Span-indexed `id=`/`label=` metadata (schema v6).
    pub block_meta: Vec<NativeBlockMeta>,
}

// ═══════════════════════════════════════════════════════════════════════
// Conversion functions
// ═══════════════════════════════════════════════════════════════════════

/// The span-indexed `id=`/`label=` metadata of `doc`, in span order.
pub fn to_native_block_meta(doc: &SurfDoc) -> Vec<NativeBlockMeta> {
    let _meta_scope = crate::block_meta::activate_for(&doc.source);
    crate::block_meta::snapshot()
        .into_iter()
        .map(|(span, meta)| NativeBlockMeta {
            start_line: span.start_line as u32,
            end_line: span.end_line as u32,
            start_offset: span.start_offset as u32,
            end_offset: span.end_offset as u32,
            block_id: meta.id,
            label: meta.label,
        })
        .collect()
}

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

/// Split a hero/section headline's trailing explicit anchor (`Title {#slug}`)
/// off the text — the web does it at render (`split_explicit_anchor`), so a
/// native renderer must never see the `{#…}` (schema v11). The AST keeps the
/// author's line so the fixed-point round trip is unchanged.
fn split_headline_anchor(headline: Option<&str>) -> (Option<String>, Option<String>) {
    match headline {
        None => (None, None),
        Some(h) => match crate::render_html::split_explicit_anchor(h) {
            Some((text, slug)) => (Some(text.to_string()), Some(slug.to_string())),
            None => (Some(h.to_string()), None),
        },
    }
}

/// Remove a trailing explicit anchor (`## Title {#slug}`) from every ATX
/// heading line of a markdown body, outside fenced code (schema v11). A body
/// with no such heading comes back byte-identical.
pub(crate) fn strip_heading_anchors(content: &str) -> String {
    if !content.contains("{#") {
        return content.to_string();
    }
    let mut out = String::with_capacity(content.len());
    let mut fence: Option<&str> = None;
    for (i, line) in content.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            out.push_str(line);
            continue;
        }
        let hashes = t.bytes().take_while(|b| *b == b'#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            if let Some((text, _)) = crate::render_html::split_explicit_anchor(line) {
                out.push_str(text);
                continue;
            }
        }
        out.push_str(line);
    }
    out
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
    let content = &strip_heading_anchors(content);
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
                // Markdown tables in prose carry no caption or summary row.
                caption: None,
                total: Vec::new(),
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
        // 0.19.0: additive value — native consumers treat unknown states
        // as default chrome.
        RowState::Active => "active",
    }
}

fn convert_block(block: &Block, depth: u32) -> NativeBlock {
    match block {
        // ── Native variants: direct conversion ──────────────────────

        Block::Markdown { content, .. } => NativeBlock::Markdown {
            content: strip_heading_anchors(content),
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
            caption,
            total,
            ..
        } => NativeBlock::DataTable {
            headers: headers.clone(),
            rows: rows.clone(),
            sortable: *sortable,
            caption: caption.clone(),
            total: total.clone(),
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
            min,
            max,
            ..
        } => NativeBlock::Metric {
            label: label.clone(),
            value: value.clone(),
            trend: trend.map(trend_str),
            unit: unit.clone(),
            min: min.clone(),
            max: max.clone(),
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
            price,
            currency,
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
            price: price.clone(),
            currency: currency.clone(),
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
                    content: strip_heading_anchors(&c.content),
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
        } => {
            let (headline, anchor) = split_headline_anchor(headline.as_deref());
            NativeBlock::Hero {
            headline,
            anchor,
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
        }
        }

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
            fields: convert_form_fields(fields),
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
                let (headline, anchor) = split_headline_anchor(headline.as_deref());
                NativeBlock::SectionContainer {
                    bg: bg.clone(),
                    headline,
                    anchor,
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

        // Schema v7: a segmented-control is its own variant. It used to
        // ride TabBar's id/label shape, which dropped `size=` and the
        // block-level `action=` and told the client it was switching panes.
        Block::SegmentedControl {
            active,
            size,
            action,
            segments,
            ..
        } => NativeBlock::SegmentedControl {
            active: active.clone(),
            size: size.clone(),
            action: action.as_deref().map(parse_native_action),
            segments: segments
                .iter()
                .map(|s| NativeSegmentItem {
                    id: s.id.clone(),
                    label: s.label.clone(),
                })
                .collect(),
        },

        // Schema v7: a dropdown-select is its own variant. It used to ride
        // CommandPalette's trigger/item shape, which dropped `icon=`,
        // `align=` and the label/selected distinction.
        Block::DropdownSelect {
            label,
            icon,
            selected,
            align,
            options,
            ..
        } => NativeBlock::DropdownSelect {
            label: label.clone(),
            icon: icon.clone(),
            selected: selected.clone(),
            align: align.clone(),
            options: options
                .iter()
                .map(|o| NativeDropdownOption {
                    label: o.label.clone(),
                    description: o.description.clone(),
                    icon: o.icon.clone(),
                    action: o.action.as_deref().map(parse_native_action),
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
            source,
            steps,
            value,
            max,
            ..
        } => NativeBlock::Progress {
            source: source.clone(),
            value: value.clone(),
            max: max.clone(),
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

        Block::PricingTable {
            headers,
            rows,
            highlight,
            current,
            ..
        } => NativeBlock::PricingTable {
            headers: headers.clone(),
            rows: rows.clone(),
            highlight: highlight.clone(),
            current: current.clone(),
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

        // ── Schema v7: six blocks that used to degrade to Markdown ──

        Block::Style { properties, .. } => NativeBlock::Style {
            properties: properties
                .iter()
                .map(|p| NativeStyleProperty {
                    key: p.key.clone(),
                    value: p.value.clone(),
                })
                .collect(),
        },

        Block::Logo { src, alt, size, .. } => NativeBlock::Logo {
            src: src.clone(),
            alt: alt.clone(),
            size: *size,
        },

        Block::Route {
            method,
            path,
            auth,
            returns,
            body,
            handler,
            content,
            ..
        } => NativeBlock::Route {
            method: http_method_str(*method),
            path: path.clone(),
            auth: auth.clone(),
            returns: returns.clone(),
            body: body.clone(),
            handler: handler.clone(),
            content: content.clone(),
        },

        Block::Action {
            method,
            target,
            label,
            fields,
            confirm,
            ..
        } => NativeBlock::Action {
            method: http_method_str(*method),
            target: target.clone(),
            label: label.clone(),
            fields: convert_form_fields(fields),
            confirm: confirm.clone(),
        },

        Block::Model { name, fields, .. } => NativeBlock::Model {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|f| NativeModelField {
                    name: f.name.clone(),
                    field_type: model_field_type_str(&f.field_type),
                    constraints: f.constraints.iter().map(field_constraint_str).collect(),
                })
                .collect(),
        },

        Block::App {
            name,
            binary,
            region,
            port,
            platform,
            auth,
            content,
            children,
            ..
        } => NativeBlock::App {
            name: name.clone(),
            binary: binary.clone(),
            region: region.clone(),
            port: *port,
            platform: platform.clone(),
            auth: auth.clone(),
            content: content.clone(),
            children: convert_children(children, depth + 1),
        },

        // ── Schema v8 (session 9): the last ten blocks with measured use ──

        Block::Auth {
            provider,
            session,
            roles,
            default_role,
            ..
        } => NativeBlock::Auth {
            provider: auth_provider_str(*provider),
            session: session.clone(),
            roles: roles.clone(),
            default_role: default_role.clone(),
        },

        Block::AppDeploy {
            region,
            scale,
            domain,
            memory,
            properties,
            ..
        } => NativeBlock::AppDeploy {
            region: region.clone(),
            scale: *scale,
            domain: domain.clone(),
            memory: memory.clone(),
            // The parser keeps `(key, value)` tuples here; a tuple cannot
            // cross UniFFI, so the pairs ride the v7 key/value record (D-S9-3).
            properties: properties
                .iter()
                .map(|(k, v)| NativeStyleProperty {
                    key: k.clone(),
                    value: v.clone(),
                })
                .collect(),
        },

        Block::Booking {
            title,
            service_label,
            services,
            days,
            ..
        } => NativeBlock::Booking {
            title: title.clone(),
            service_label: service_label.clone(),
            services: services
                .iter()
                .map(|s| NativeBookingService {
                    name: s.name.clone(),
                    duration: s.duration.clone(),
                    price: s.price.clone(),
                })
                .collect(),
            days: days
                .iter()
                .map(|d| NativeBookingDay {
                    date: d.date.clone(),
                    slots: d.slots.clone(),
                })
                .collect(),
        },

        // A schema field IS a model field (same name / type / constraints
        // shape), so `::schema` reuses the v7 record (D-S9-2).
        Block::Schema { name, fields, .. } => NativeBlock::Schema {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|f| NativeModelField {
                    name: f.name.clone(),
                    field_type: model_field_type_str(&f.field_type),
                    constraints: f.constraints.iter().map(field_constraint_str).collect(),
                })
                .collect(),
        },

        Block::ChatInput {
            action,
            placeholder,
            modes,
            ..
        } => NativeBlock::ChatInput {
            action: action.clone(),
            placeholder: placeholder.clone(),
            modes: modes.clone(),
        },

        Block::Store {
            title,
            currency,
            items,
            ..
        } => NativeBlock::Store {
            title: title.clone(),
            currency: currency.clone(),
            items: items
                .iter()
                .map(|it| NativeStoreItem {
                    name: it.name.clone(),
                    price: it.price.clone(),
                    blurb: it.blurb.clone(),
                    badge: it.badge.clone(),
                    category: it.category.clone(),
                })
                .collect(),
        },

        Block::AppEnv { vars, .. } => NativeBlock::AppEnv {
            vars: vars
                .iter()
                .map(|v| NativeEnvVar {
                    name: v.name.clone(),
                    description: v.description.clone(),
                    required: v.required,
                })
                .collect(),
        },

        Block::Binding {
            source,
            target,
            events,
            ..
        } => NativeBlock::Binding {
            source: source.clone(),
            target: target.clone(),
            events: events
                .iter()
                .map(|e| NativeBindingEvent {
                    event: e.event.clone(),
                    action: e.action.clone(),
                })
                .collect(),
        },

        Block::Build {
            base,
            runtime,
            edition,
            properties,
            ..
        } => NativeBlock::Build {
            base: base.clone(),
            runtime: runtime.clone(),
            edition: edition.clone(),
            properties: properties
                .iter()
                .map(|p| NativeStyleProperty {
                    key: p.key.clone(),
                    value: p.value.clone(),
                })
                .collect(),
        },

        Block::Cicd {
            provider,
            properties,
            ..
        } => NativeBlock::Cicd {
            provider: provider.clone(),
            properties: properties
                .iter()
                .map(|p| NativeStyleProperty {
                    key: p.key.clone(),
                    value: p.value.clone(),
                })
                .collect(),
        },

        // ── Schema v9 (session 10): the ten infra blocks of the app format ──

        Block::Concurrency {
            concurrency_type,
            hard_limit,
            soft_limit,
            force_https,
            ..
        } => NativeBlock::Concurrency {
            concurrency_type: concurrency_type.clone(),
            hard_limit: *hard_limit,
            soft_limit: *soft_limit,
            force_https: *force_https,
        },

        Block::Crates { entries, .. } => NativeBlock::Crates {
            entries: entries
                .iter()
                .map(|e| NativeCrateEntry {
                    name: e.name.clone(),
                    source: e.source.clone(),
                    features: e.features.clone(),
                })
                .collect(),
        },

        Block::Dashboard { source, refresh, .. } => NativeBlock::Dashboard {
            source: source.clone(),
            refresh: *refresh,
        },

        Block::InfraDatabase {
            name,
            shared_auth,
            volume_gb,
            properties,
            ..
        } => NativeBlock::InfraDatabase {
            name: name.clone(),
            shared_auth: *shared_auth,
            volume_gb: *volume_gb,
            properties: style_properties(properties),
        },

        Block::Deploy {
            env,
            app,
            machines,
            memory,
            auto_stop,
            min_machines,
            strategy,
            properties,
            ..
        } => NativeBlock::Deploy {
            env: env.clone(),
            app: app.clone(),
            machines: *machines,
            memory: *memory,
            auto_stop: auto_stop.clone(),
            min_machines: *min_machines,
            strategy: strategy.clone(),
            properties: style_properties(properties),
        },

        Block::DeployUrls { entries, .. } => NativeBlock::DeployUrls {
            entries: style_properties(entries),
        },

        Block::Domains { entries, .. } => NativeBlock::Domains {
            entries: entries
                .iter()
                .map(|e| NativeDomainEntry {
                    domain: e.domain.clone(),
                    description: e.description.clone(),
                })
                .collect(),
        },

        Block::Editor {
            source,
            lang,
            preview,
            ..
        } => NativeBlock::Editor {
            source: source.clone(),
            lang: lang.clone(),
            preview: *preview,
        },

        Block::InfraEnv { tier, entries, .. } => NativeBlock::InfraEnv {
            tier: tier.clone(),
            entries: entries
                .iter()
                .map(|e| NativeEnvEntry {
                    name: e.name.clone(),
                    default_value: e.default_value.clone(),
                })
                .collect(),
        },

        Block::Feed { source, stream, .. } => NativeBlock::Feed {
            source: source.clone(),
            stream: *stream,
        },

        // ── Schema v10: the last twenty ─────────────────────────────

        Block::Health { path, method, grace, interval, timeout, .. } => NativeBlock::Health {
            path: path.clone(),
            method: method.clone(),
            grace: grace.clone(),
            interval: interval.clone(),
            timeout: timeout.clone(),
        },

        Block::Hours { title, timezone, rows, .. } => NativeBlock::Hours {
            title: title.clone(),
            timezone: timezone.clone(),
            rows: rows
                .iter()
                .map(|r| NativeHoursRow {
                    day: r.day,
                    label: r.label.clone(),
                    opens: r.opens,
                    closes: r.closes,
                    text: r.text.clone(),
                })
                .collect(),
        },

        Block::Marquee { items, .. } => NativeBlock::Marquee { items: items.clone() },

        Block::Smoke { script, checks, .. } => NativeBlock::Smoke {
            script: script.clone(),
            checks: checks
                .iter()
                .map(|c| NativeSmokeCheck {
                    method: c.method.clone(),
                    path: c.path.clone(),
                    expected: c.expected,
                })
                .collect(),
        },

        Block::Use { crates, .. } => NativeBlock::Use {
            crates: crates
                .iter()
                .map(|c| NativeCrateDep {
                    name: c.name.clone(),
                    version: c.version.clone(),
                    features: c.features.clone(),
                })
                .collect(),
        },

        Block::Volumes { entries, .. } => NativeBlock::Volumes {
            entries: entries
                .iter()
                .map(|v| NativeVolumeEntry { name: v.name.clone(), mount: v.mount.clone() })
                .collect(),
        },

        Block::Related { items, .. } => NativeBlock::Related {
            items: items
                .iter()
                .map(|i| NativeRelatedItem {
                    title: i.title.clone(),
                    href: i.href.clone(),
                    relation: i.relation.clone(),
                })
                .collect(),
        },

        Block::Turn { participant, time, role, model, content, .. } => NativeBlock::Turn {
            participant: participant.clone(),
            time: time.clone(),
            role: crate::types::turn_role(participant, role.as_deref()).to_string(),
            model: model.clone(),
            content: content.clone(),
        },

        Block::Timeline { title, entries, .. } => NativeBlock::Timeline {
            title: title.clone(),
            entries: entries
                .iter()
                .map(|e| NativeTimelineEntry {
                    when: e.when.clone(),
                    label: e.label.clone(),
                    group: e.group.clone(),
                })
                .collect(),
        },

        Block::Output { for_id, timestamp, exit, format, content, .. } => NativeBlock::Output {
            for_id: for_id.clone(),
            timestamp: timestamp.clone(),
            exit: *exit,
            format: format.clone(),
            content: content.clone(),
        },

        Block::AiGenerated { model, date, reviewed, content, .. } => NativeBlock::AiGenerated {
            model: model.clone(),
            date: date.clone(),
            reviewed: *reviewed,
            content: content.clone(),
        },

        Block::Alternatives { headers, rows, .. } => NativeBlock::Alternatives {
            headers: headers.clone(),
            rows: rows
                .iter()
                .map(|r| {
                    let mut cells = r.clone();
                    while cells.len() < headers.len() {
                        cells.push(String::new());
                    }
                    NativeAlternativeRow { cells }
                })
                .collect(),
        },

        Block::AiContext { model, tokens, loaded, content, .. } => NativeBlock::AiContext {
            model: model.clone(),
            tokens: *tokens,
            loaded: *loaded,
            content: content.clone(),
        },

        Block::Countdown { date, label, .. } => NativeBlock::Countdown {
            date: date.clone(),
            label: label.clone(),
        },

        Block::Css { content, .. } => NativeBlock::Css { content: content.clone() },

        Block::Footnote { id, content, .. } => NativeBlock::Footnote {
            id: id.clone(),
            content: content.clone(),
        },

        Block::Kernel { lang, env, runtime, packages, sandbox, properties, .. } => NativeBlock::Kernel {
            lang: lang.clone(),
            env: env.clone(),
            runtime: runtime.clone(),
            packages: packages.clone(),
            sandbox: sandbox.clone(),
            properties: style_properties(properties),
        },

        Block::LogoCloud { title, items, .. } => NativeBlock::LogoCloud {
            title: title.clone(),
            items: items
                .iter()
                .map(|i| NativeLogoItem { src: i.src.clone(), name: i.name.clone() })
                .collect(),
        },

        Block::Subscribe { action, placeholder, content, .. } => NativeBlock::Subscribe {
            action: action.clone(),
            placeholder: placeholder.clone(),
            content: content.clone(),
        },

        Block::Notes { content, .. } => NativeBlock::Notes { content: content.clone() },

        // ── Markdown fallback: the one untyped block ─────────────────

        Block::Unknown { .. } => {
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
        FormFieldType::Checkbox => "checkbox",
        FormFieldType::Radio => "radio",
        FormFieldType::Toggle => "toggle",
        FormFieldType::File => "file",
        FormFieldType::Hidden => "hidden",
    }
    .to_string()
}

/// Map a `::form` / `::action` field list onto its native shape. Shared so
/// the two blocks can never drift apart at the FFI.
fn convert_form_fields(fields: &[crate::types::FormField]) -> Vec<NativeFormField> {
    fields
        .iter()
        .map(|f| NativeFormField {
            label: f.label.clone(),
            name: f.name.clone(),
            field_type: form_field_type_str(f.field_type),
            required: f.required,
            placeholder: f.placeholder.clone(),
            options: f.options.clone(),
            group: f.group.clone(),
        })
        .collect()
}

/// The uppercase HTTP verb of a `::route` / `::action` `method=`.
fn http_method_str(m: crate::types::HttpMethod) -> String {
    use crate::types::HttpMethod;
    match m {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
    .to_string()
}

/// The spec spelling of a `::model` field type (schema v7). Same strings the
/// markdown and HTML renderers emit, so a native client and a web preview
/// name a type identically.
/// The spec spelling of an `::auth` provider — the same four words
/// `parse_auth` reads (schema v8).
fn auth_provider_str(p: crate::types::AuthProvider) -> String {
    use crate::types::AuthProvider;
    match p {
        AuthProvider::Email => "email",
        AuthProvider::OAuth => "oauth",
        AuthProvider::ApiKey => "api-key",
        AuthProvider::Token => "token",
    }
    .to_string()
}

fn model_field_type_str(ft: &crate::types::ModelFieldType) -> String {
    crate::render_md::model_field_type_md(ft)
}

/// The spec spelling of a `::model` field constraint (schema v7).
fn field_constraint_str(c: &crate::types::FieldConstraint) -> String {
    crate::render_md::constraint_md(c)
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
        | Block::Toc { .. }
        // Schema v7: the app-spec trio now converts structurally.
        | Block::Route { .. }
        | Block::Action { .. }
        | Block::Model { .. }
        // Schema v8: a schema is a definition table, like a model.
        | Block::Schema { .. }
        // Schema v10: the reader's annotations and references.
        | Block::Related { .. }
        | Block::Turn { .. }
        | Block::Timeline { .. }
        | Block::Output { .. }
        | Block::AiGenerated { .. }
        | Block::Alternatives { .. }
        | Block::AiContext { .. }
        | Block::Footnote { .. }
        | Block::Notes { .. } => BlockTier::Content,

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
        | Block::Slide { .. }
        // Schema v7: ::logo is a brand element, not a degraded string.
        | Block::Logo { .. }
        // Schema v8: the two site widgets.
        | Block::Booking { .. }
        | Block::Store { .. }
        // Schema v10: the 0.21.0 site pair and the three marketing leaves.
        | Block::Hours { .. }
        | Block::Marquee { .. }
        | Block::Countdown { .. }
        | Block::LogoCloud { .. }
        | Block::Subscribe { .. } => BlockTier::Site,

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
        | Block::SplitPane { .. }
        // Schema v7: presentation overrides and the app manifest shell.
        | Block::Style { .. }
        | Block::App { .. }
        // Schema v8: the manifest's children and the routed composer.
        | Block::Auth { .. }
        | Block::AppDeploy { .. }
        | Block::AppEnv { .. }
        | Block::Binding { .. }
        | Block::Build { .. }
        | Block::Cicd { .. }
        | Block::ChatInput { .. }
        // Schema v9: the ten infra blocks of the app format — every infra
        // block the spec names inside an ::app is structural now.
        | Block::Concurrency { .. }
        | Block::Crates { .. }
        | Block::Dashboard { .. }
        | Block::InfraDatabase { .. }
        | Block::Deploy { .. }
        | Block::DeployUrls { .. }
        | Block::Domains { .. }
        | Block::Editor { .. }
        | Block::InfraEnv { .. }
        | Block::Feed { .. }
        // Schema v10: the last manifest facts and the two utility blocks.
        | Block::Health { .. }
        | Block::Smoke { .. }
        | Block::Volumes { .. }
        | Block::Use { .. }
        | Block::Css { .. }
        | Block::Kernel { .. } => BlockTier::Chrome,

        // ── Tier 4: explicit markdown degradation ────────────────────
        Block::Unknown { .. }
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
::deck[theme=dark]\n::\n
::section\nInner\n::\n
::style\naccent: #ff0000\n::\n
::logo[src=/logo.png]\n::\n
::route[method=GET path=/api/x]\n::\n
::action[method=DELETE target=/api/x label=Delete]\n- Reason (text)\n::\n
::model[name=User]\n- id: uuid [primary]\n::\n
::segmented-control[active=all]\n- all \"All\"\n::\n
::dropdown-select[label=Sort]\n- \"Newest\" action=sort_newest\n::\n
::app[name=demo]\n::\n
::auth[provider=oauth]\nroles: admin, member\n::\n
::app-deploy[region=sjc scale=2]\nmemory: 512mb\n::\n
::booking[title=Book]\n- service: Cut | 30 min | $40\n- day: 2026-10-01 | 9:00 AM\n::\n
::schema[name=User]\n- id (uuid) primary\n::\n
::chat-input[action=/api/chat]\nmodes: ask | build\n::\n
::store[title=Shop currency=$]\n- item: Tee | 20\n::\n
::app-env\n- DATABASE_URL * \"postgres\"\n::\n
::binding[source=/api/tasks target=list]\nchange: refresh\n::\n
::build[base=rust]\nfeatures: full\n::\n
::cicd[provider=github]\ndeploy: main\n::\n
::concurrency[type=connections hard_limit=1000]\n::\n
::crates\nserde (features: derive)\n::\n
::dashboard[source=/api/metrics refresh=30]\n::\n
::database[name=main volume_gb=10]\nengine: postgres\n::\n
::deploy[env=production machines=2]\nregion: sjc\n::\n
::deploy-urls\nproduction: https://surf.space\n::\n
::domains\nsurf.space (the product)\n::\n
::editor[source=/docs/readme.surf lang=surfdoc]\n::\n
::env[tier=required]\nDATABASE_URL\n::\n
::feed[source=/api/events stream=true]\n::\n
::health[path=/healthz method=GET]\n::\n
::hours[title=Hours]\nMonday: 9am - 5pm\n::\n
::marquee\n- Fresh daily\n::\n
::smoke\nGET /healthz -> 200\n::\n
::use\n- reqwest 0.12 [json]\n::\n
::volumes\ndata -> /data\n::\n
::related\n- [Plan](plans/plan.md) \u{2014} produces\n::\n
::turn[participant=claude]\nYes.\n::\n
::timeline\n- 2026-01 \u{2014} Beta\n::\n
::output[for=analysis exit=0]\nok\n::\n
::ai-generated[model=opus]\nMaybe.\n::\n
::alternatives\n| Option | Verdict |\n|---|---|\n| A | Selected |\n::\n
::ai-context[model=opus tokens=10]\nNote.\n::\n
::countdown[date=2026-03-15 label=Launch]\n::\n
::css\n.x { color: red; }\n::\n
::footnote[id=1]\nA source.\n::\n
::kernel[lang=python]\nruntime: python3.12\n::\n
::logo-cloud[title=Trusted]\n- assets/acme.svg\n::\n
::subscribe[action=/subscribe]\nJoin.\n::\n
::notes\nPause.\n::\n";
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

    // ═══════════════════════════════════════════════════════════════
    // Schema v7 (S8): the eight blocks that stopped being borrowed or
    // degraded. One test per variant, parsed from real source so the
    // conversion arm and the parser are pinned together.
    // ═══════════════════════════════════════════════════════════════

    fn convert_first(source: &str) -> NativeBlock {
        let result = crate::parse(source);
        convert_block(&result.doc.blocks[0], 0)
    }

    #[test]
    fn style_converts_structurally() {
        // `::style` reads `key: value` BODY lines, not attributes.
        match convert_first("::style\naccent: #ff0000\nheading-font: inter\n::\n") {
            NativeBlock::Style { properties } => {
                assert_eq!(
                    properties,
                    vec![
                        NativeStyleProperty { key: "accent".into(), value: "#ff0000".into() },
                        NativeStyleProperty { key: "heading-font".into(), value: "inter".into() },
                    ]
                );
            }
            other => panic!("expected Style, got {other:?}"),
        }
    }

    #[test]
    fn logo_converts_structurally() {
        match convert_first("::logo[src=/brand.png alt=\"Mark\" size=48]\n::\n") {
            NativeBlock::Logo { src, alt, size } => {
                assert_eq!(src, "/brand.png");
                assert_eq!(alt.as_deref(), Some("Mark"));
                assert_eq!(size, Some(48));
            }
            other => panic!("expected Logo, got {other:?}"),
        }
    }

    #[test]
    fn route_converts_structurally() {
        let source = "::route[method=post path=/api/users]\n\
                      auth: required\n\
                      returns: list(User)\n\
                      body: User\n\
                      Creates a user.\n\
                      ```rust\n\
                      fn handler() {}\n\
                      ```\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Route { method, path, auth, returns, body, handler, content } => {
                assert_eq!(method, "POST", "method crosses uppercased");
                assert_eq!(path, "/api/users");
                assert_eq!(auth.as_deref(), Some("required"));
                assert_eq!(returns.as_deref(), Some("list(User)"));
                assert_eq!(body.as_deref(), Some("User"));
                assert_eq!(handler.as_deref(), Some("fn handler() {}"));
                assert_eq!(content, "Creates a user.");
            }
            other => panic!("expected Route, got {other:?}"),
        }
    }

    #[test]
    fn action_converts_structurally() {
        let source = "::action[method=delete target=\"/api/users/1\" confirm=\"Sure?\"]\n\
                      - Reason (text)\n\
                      - Delete\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Action { method, target, label, fields, confirm } => {
                assert_eq!(method, "DELETE");
                assert_eq!(target, "/api/users/1");
                // Trailing bare item becomes the submit label, not a field.
                assert_eq!(label, "Delete");
                assert_eq!(confirm.as_deref(), Some("Sure?"));
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].label, "Reason");
                assert_eq!(fields[0].name, "reason");
                assert_eq!(fields[0].field_type, "text");
                assert_eq!(fields[0].group, None, "fieldsets are a ::form feature");
            }
            other => panic!("expected Action, got {other:?}"),
        }
    }

    #[test]
    fn model_converts_structurally() {
        let source = "::model[name=User]\n\
                      - id: uuid [primary, auto]\n\
                      - email: string [required, unique, max=254]\n\
                      - role: enum(admin, member) []\n\
                      - team: ref(Team) [optional]\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Model { name, fields } => {
                assert_eq!(name, "User");
                assert_eq!(fields.len(), 4);
                assert_eq!(
                    fields[0],
                    NativeModelField {
                        name: "id".into(),
                        field_type: "uuid".into(),
                        constraints: vec!["primary".into(), "auto".into()],
                    }
                );
                assert_eq!(
                    fields[1].constraints,
                    vec!["required".to_string(), "unique".to_string(), "max=254".to_string()]
                );
                assert_eq!(fields[2].field_type, "enum(admin, member)");
                assert_eq!(fields[3].field_type, "ref(Team)");
            }
            other => panic!("expected Model, got {other:?}"),
        }
    }

    #[test]
    fn app_converts_structurally_with_children() {
        let source = "::app[name=demo binary=demo-bin region=sjc port=8080 platform=fly auth=password]\n\
                      ::callout[type=info]\n\
                      Inner\n\
                      ::\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::App {
                name, binary, region, port, platform, auth, content, children,
            } => {
                assert_eq!(name, "demo");
                assert_eq!(binary.as_deref(), Some("demo-bin"));
                assert_eq!(region.as_deref(), Some("sjc"));
                assert_eq!(port, Some(8080));
                assert_eq!(platform.as_deref(), Some("fly"));
                assert_eq!(auth.as_deref(), Some("password"));
                assert!(content.contains("::callout"), "raw body is carried: {content}");
                assert!(
                    children.iter().any(|c| matches!(c, NativeBlock::Callout { .. })),
                    "children convert through convert_children: {children:?}"
                );
            }
            other => panic!("expected App, got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Schema v8 (S9): the last ten blocks with measured use. One test
    // per variant from real source, so the parser's grammar (attributes
    // vs body lines, the `|` fields, the `*` flag) is pinned with the arm.
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn auth_converts_structurally() {
        let source = "::auth[provider=OAuth]\n\
                      session: jwt\n\
                      roles: admin, member, guest\n\
                      default-role: member\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Auth { provider, session, roles, default_role } => {
                assert_eq!(provider, "oauth", "the spec's lowercase word, whatever the author's case");
                assert_eq!(session.as_deref(), Some("jwt"));
                assert_eq!(roles, vec!["admin", "member", "guest"]);
                assert_eq!(default_role.as_deref(), Some("member"));
            }
            other => panic!("expected Auth, got {other:?}"),
        }
        // D-S9-5: the toml's attribute form parses too; a body line wins.
        match convert_first("::auth[provider=token session=cookie default_role=viewer]\nsession: jwt\n::\n") {
            NativeBlock::Auth { provider, session, default_role, .. } => {
                assert_eq!(provider, "token");
                assert_eq!(session.as_deref(), Some("jwt"), "the body line overrides the attribute");
                assert_eq!(default_role.as_deref(), Some("viewer"));
            }
            other => panic!("expected Auth, got {other:?}"),
        }
        // A bare `::auth` is an email provider with nothing else (parse_auth's default).
        match convert_first("::auth\n::\n") {
            NativeBlock::Auth { provider, session, roles, default_role } => {
                assert_eq!(provider, "email");
                assert!(session.is_none() && roles.is_empty() && default_role.is_none());
            }
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    #[test]
    fn app_deploy_converts_structurally() {
        let source = "::app-deploy[region=sjc scale=3 domain=app.example.com memory=1gb]\n\
                      min_instances: 1\n\
                      cpu: shared\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::AppDeploy { region, scale, domain, memory, properties } => {
                assert_eq!(region.as_deref(), Some("sjc"));
                assert_eq!(scale, Some(3));
                assert_eq!(domain.as_deref(), Some("app.example.com"));
                assert_eq!(memory.as_deref(), Some("1gb"));
                assert_eq!(
                    properties,
                    vec![
                        NativeStyleProperty { key: "min_instances".into(), value: "1".into() },
                        NativeStyleProperty { key: "cpu".into(), value: "shared".into() },
                    ],
                    "the (key, value) tuples cross as the v7 key/value record (D-S9-3)"
                );
            }
            other => panic!("expected AppDeploy, got {other:?}"),
        }
    }

    #[test]
    fn booking_converts_structurally() {
        let source = "::booking[title=\"Book a visit\" service-label=Treatment]\n\
                      - service: Consultation | 30 min | Free\n\
                      - service: Deep clean | 60 min | $120\n\
                      - service: Whitening\n\
                      - day: 2026-10-01 | 9:00 AM, 10:30 AM, 2:00 PM\n\
                      - day: 2026-10-02 | full\n\
                      - day: 2026-10-03\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Booking { title, service_label, services, days } => {
                assert_eq!(title.as_deref(), Some("Book a visit"));
                assert_eq!(service_label.as_deref(), Some("Treatment"));
                assert_eq!(services.len(), 3);
                assert_eq!(
                    services[1],
                    NativeBookingService {
                        name: "Deep clean".into(),
                        duration: Some("60 min".into()),
                        price: Some("$120".into()),
                    }
                );
                assert_eq!(services[2].duration, None, "a two-field line has no duration");
                assert_eq!(days.len(), 3);
                assert_eq!(days[0].slots, vec!["9:00 AM", "10:30 AM", "2:00 PM"]);
                assert!(days[1].slots.is_empty(), "`full` is a day with no slots");
                assert!(days[2].slots.is_empty(), "no `|` is a day with no slots");
            }
            other => panic!("expected Booking, got {other:?}"),
        }
    }

    #[test]
    fn schema_converts_structurally_through_the_model_field_record() {
        // `parse_schema`'s grammar is `- name (type) constraint…` with
        // `enum:a,b` / `ref:Model` types and `min:1` / `default:x`
        // constraints — NOT the model's `- name: type [constraints]`. The
        // crossing shape is the same record, and it spells types and
        // constraints the MODEL way (`enum(a, b)`, `min=1`) so one Swift
        // grid draws both (D-S9-2).
        let source = "::schema[name=Order]\n\
                      - id (uuid) primary auto\n\
                      - total (money) required min:1\n\
                      - status (enum:open,paid) default:open\n\
                      - owner (ref:User)\n\
                      - note\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Schema { name, fields } => {
                assert_eq!(name, "Order");
                assert_eq!(fields.len(), 5);
                assert_eq!(
                    fields[0],
                    NativeModelField {
                        name: "id".into(),
                        field_type: "uuid".into(),
                        constraints: vec!["primary".into(), "auto".into()],
                    }
                );
                assert_eq!(fields[1].constraints, vec!["required".to_string(), "min=1".to_string()]);
                assert_eq!(fields[2].field_type, "enum(open, paid)");
                assert_eq!(fields[3].field_type, "ref(User)");
                assert_eq!(fields[4].field_type, "string", "a bare name is a string field");
            }
            other => panic!("expected Schema, got {other:?}"),
        }
    }

    #[test]
    fn chat_input_converts_structurally() {
        match convert_first("::chat-input[action=/api/chat placeholder=\"Ask Surfy\"]\nmodes: ask | build | plan\n::\n") {
            NativeBlock::ChatInput { action, placeholder, modes } => {
                assert_eq!(action, "/api/chat");
                assert_eq!(placeholder.as_deref(), Some("Ask Surfy"));
                assert_eq!(modes, vec!["ask", "build", "plan"]);
            }
            other => panic!("expected ChatInput, got {other:?}"),
        }
        // D-S9-5: `modes=` as the toml's attribute; an external action arrives blank.
        match convert_first("::chat-input[action=https://evil.example/x modes=\"ask | build\"]\n::\n") {
            NativeBlock::ChatInput { action, modes, .. } => {
                assert_eq!(action, "", "validate_source_path drops an external target");
                assert_eq!(modes, vec!["ask", "build"]);
            }
            other => panic!("expected ChatInput, got {other:?}"),
        }
    }

    #[test]
    fn store_converts_structurally() {
        let source = "::store[title=\"Surf Shop\" currency=€]\n\
                      - category: Boards\n\
                      - item: Longboard | 480 | Nine feet of glide | Bestseller\n\
                      - item: Fish | 390\n\
                      - category:\n\
                      - item: Wax | 4 | Cold water\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Store { title, currency, items } => {
                assert_eq!(title.as_deref(), Some("Surf Shop"));
                assert_eq!(currency.as_deref(), Some("€"));
                assert_eq!(items.len(), 3);
                assert_eq!(
                    items[0],
                    NativeStoreItem {
                        name: "Longboard".into(),
                        price: "480".into(),
                        blurb: Some("Nine feet of glide".into()),
                        badge: Some("Bestseller".into()),
                        category: Some("Boards".into()),
                    }
                );
                assert_eq!(items[1].category.as_deref(), Some("Boards"), "a category holds until the next one");
                assert_eq!(items[2].category, None, "an empty category line groups under All");
                assert_eq!(items[2].badge, None);
            }
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn app_env_converts_structurally() {
        let source = "::app-env\n\
                      - DATABASE_URL * \"Postgres connection string\"\n\
                      - LOG_LEVEL \"info by default\"\n\
                      - SECRET_KEY *\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::AppEnv { vars } => {
                assert_eq!(vars.len(), 3);
                assert_eq!(
                    vars[0],
                    NativeEnvVar {
                        name: "DATABASE_URL".into(),
                        description: Some("Postgres connection string".into()),
                        required: true,
                    }
                );
                assert!(!vars[1].required);
                assert_eq!(vars[2].description, None);
                assert!(vars[2].required);
            }
            other => panic!("expected AppEnv, got {other:?}"),
        }
        // Attributes alone name variables with no description (parse_app_env's first arm).
        match convert_first("::app-env[PORT=8080]\n::\n") {
            NativeBlock::AppEnv { vars } => {
                assert_eq!(vars.len(), 1);
                assert_eq!(vars[0].name, "PORT");
                assert!(!vars[0].required);
            }
            other => panic!("expected AppEnv, got {other:?}"),
        }
    }

    #[test]
    fn binding_converts_structurally() {
        match convert_first("::binding[source=/api/tasks target=task-list]\nchange: refresh\nsubmit: post\n::\n") {
            NativeBlock::Binding { source, target, events } => {
                assert_eq!(source, "/api/tasks");
                assert_eq!(target, "task-list");
                assert_eq!(
                    events,
                    vec![
                        NativeBindingEvent { event: "change".into(), action: "refresh".into() },
                        NativeBindingEvent { event: "submit".into(), action: "post".into() },
                    ]
                );
            }
            other => panic!("expected Binding, got {other:?}"),
        }
    }

    #[test]
    fn build_converts_structurally() {
        match convert_first("::build[base=rust runtime=tokio edition=2024]\nfeatures: pdf, native\nprofile: release\n::\n") {
            NativeBlock::Build { base, runtime, edition, properties } => {
                assert_eq!(base.as_deref(), Some("rust"));
                assert_eq!(runtime.as_deref(), Some("tokio"));
                assert_eq!(edition.as_deref(), Some("2024"));
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[0], NativeStyleProperty { key: "features".into(), value: "pdf, native".into() });
            }
            other => panic!("expected Build, got {other:?}"),
        }
    }

    #[test]
    fn cicd_converts_structurally() {
        match convert_first("::cicd[provider=github]\ndeploy: on push to main\ntest: cargo test\n::\n") {
            NativeBlock::Cicd { provider, properties } => {
                assert_eq!(provider.as_deref(), Some("github"));
                assert_eq!(properties.len(), 2);
                assert_eq!(properties[1], NativeStyleProperty { key: "test".into(), value: "cargo test".into() });
            }
            other => panic!("expected Cicd, got {other:?}"),
        }
        match convert_first("::cicd\n::\n") {
            NativeBlock::Cicd { provider, properties } => {
                assert!(provider.is_none() && properties.is_empty());
            }
            other => panic!("expected Cicd, got {other:?}"),
        }
    }

    /// The manifest's children turn structural: an `::app` whose body
    /// authors the infra blocks carries them as their own variants now.
    #[test]
    fn app_children_are_structural_at_schema_v8() {
        let source = "::app[name=demo]\n\
                      ::build[base=rust]\n::\n\
                      ::app-env\n- PORT\n::\n\
                      ::app-deploy[region=sjc]\n::\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::App { children, .. } => {
                assert!(matches!(children[0], NativeBlock::Build { .. }), "{children:?}");
                assert!(matches!(children[1], NativeBlock::AppEnv { .. }), "{children:?}");
                assert!(matches!(children[2], NativeBlock::AppDeploy { .. }), "{children:?}");
            }
            other => panic!("expected App, got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Schema v9 (S10): the ten infra blocks of the app format. One test
    // per variant from real source — the parser's own grammar each.
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn concurrency_converts_structurally() {
        match convert_first("::concurrency[type=connections hard_limit=1000 soft_limit=800 force_https=true]\n::\n") {
            NativeBlock::Concurrency { concurrency_type, hard_limit, soft_limit, force_https } => {
                assert_eq!(concurrency_type.as_deref(), Some("connections"));
                assert_eq!(hard_limit, Some(1000));
                assert_eq!(soft_limit, Some(800));
                assert!(force_https);
            }
            other => panic!("expected Concurrency, got {other:?}"),
        }
        match convert_first("::concurrency\n::\n") {
            NativeBlock::Concurrency { concurrency_type, hard_limit, soft_limit, force_https } => {
                assert!(concurrency_type.is_none() && hard_limit.is_none() && soft_limit.is_none());
                assert!(!force_https);
            }
            other => panic!("expected Concurrency, got {other:?}"),
        }
    }

    /// `parse_crates`' paren grammar: `github:`/`source:` is the source,
    /// `features:` the features, `branch:` folds into the source.
    #[test]
    fn crates_convert_structurally() {
        let source = "::crates\n\
                      surf-parse (github: cloudsurf/surf-parse, features: pdf native, branch: main)\n\
                      serde\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Crates { entries } => {
                assert_eq!(entries.len(), 2);
                assert_eq!(
                    entries[0],
                    NativeCrateEntry {
                        name: "surf-parse".into(),
                        source: Some("cloudsurf/surf-parse, branch: main".into()),
                        features: Some("pdf native".into()),
                    }
                );
                assert_eq!(entries[1], NativeCrateEntry { name: "serde".into(), source: None, features: None });
            }
            other => panic!("expected Crates, got {other:?}"),
        }
    }

    /// `source=` goes through `validate_source_path`: an external target
    /// arrives blank (the `::chat-input` precedent, S9).
    #[test]
    fn dashboard_converts_structurally() {
        match convert_first("::dashboard[source=/api/metrics refresh=30]\n::\n") {
            NativeBlock::Dashboard { source, refresh } => {
                assert_eq!(source, "/api/metrics");
                assert_eq!(refresh, Some(30));
            }
            other => panic!("expected Dashboard, got {other:?}"),
        }
        match convert_first("::dashboard[source=https://evil.example/metrics]\n::\n") {
            NativeBlock::Dashboard { source, refresh } => {
                assert!(source.is_empty(), "an external source arrives blank");
                assert!(refresh.is_none());
            }
            other => panic!("expected Dashboard, got {other:?}"),
        }
    }

    #[test]
    fn database_converts_structurally() {
        let source = "::database[name=main shared_auth=true volume_gb=10]\n\
                      engine: postgres\n\
                      : orphan\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::InfraDatabase { name, shared_auth, volume_gb, properties } => {
                assert_eq!(name.as_deref(), Some("main"));
                assert!(shared_auth);
                assert_eq!(volume_gb, Some(10));
                assert_eq!(properties, vec![NativeStyleProperty { key: "engine".into(), value: "postgres".into() }]);
            }
            other => panic!("expected InfraDatabase, got {other:?}"),
        }
    }

    #[test]
    fn deploy_converts_structurally() {
        let source = "::deploy[env=production app=surf machines=2 memory=512 auto_stop=suspend min_machines=1 strategy=rolling]\n\
                      region: sjc\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Deploy { env, app, machines, memory, auto_stop, min_machines, strategy, properties } => {
                assert_eq!(env.as_deref(), Some("production"));
                assert_eq!(app.as_deref(), Some("surf"));
                assert_eq!(machines, Some(2));
                assert_eq!(memory, Some(512));
                assert_eq!(auto_stop.as_deref(), Some("suspend"));
                assert_eq!(min_machines, Some(1));
                assert_eq!(strategy.as_deref(), Some("rolling"));
                assert_eq!(properties, vec![NativeStyleProperty { key: "region".into(), value: "sjc".into() }]);
            }
            other => panic!("expected Deploy, got {other:?}"),
        }
        // The corpus manifest's own line: `target=` is not an attribute the
        // parser reads, so every fact is absent and the block is still structural.
        match convert_first("::deploy[target=fly]\n::\n") {
            NativeBlock::Deploy { env, properties, .. } => {
                assert!(env.is_none() && properties.is_empty());
            }
            other => panic!("expected Deploy, got {other:?}"),
        }
    }

    /// `env: url` — the first colon splits, so the URL's own `://` survives.
    #[test]
    fn deploy_urls_convert_structurally() {
        let source = "::deploy-urls\n\
                      production: https://surf.space\n\
                      staging: https://staging.surf.space\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::DeployUrls { entries } => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0], NativeStyleProperty { key: "production".into(), value: "https://surf.space".into() });
                assert_eq!(entries[1].key, "staging");
            }
            other => panic!("expected DeployUrls, got {other:?}"),
        }
        // The underscored spelling parses to the same block.
        assert!(matches!(convert_first("::deploy_urls\nprod: https://x.y\n::\n"), NativeBlock::DeployUrls { .. }));
    }

    #[test]
    fn domains_convert_structurally() {
        let source = "::domains\n\
                      surf.space (the product)\n\
                      app.surf.space\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::Domains { entries } => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0], NativeDomainEntry { domain: "surf.space".into(), description: Some("the product".into()) });
                assert_eq!(entries[1], NativeDomainEntry { domain: "app.surf.space".into(), description: None });
            }
            other => panic!("expected Domains, got {other:?}"),
        }
    }

    #[test]
    fn editor_converts_structurally() {
        match convert_first("::editor[source=/docs/readme.surf lang=surfdoc preview=true]\n::\n") {
            NativeBlock::Editor { source, lang, preview } => {
                assert_eq!(source.as_deref(), Some("/docs/readme.surf"));
                assert_eq!(lang.as_deref(), Some("surfdoc"));
                assert!(preview);
            }
            other => panic!("expected Editor, got {other:?}"),
        }
        match convert_first("::editor[source=https://evil.example/x.surf]\n::\n") {
            NativeBlock::Editor { source, lang, preview } => {
                assert!(source.is_none(), "an external source arrives absent");
                assert!(lang.is_none() && !preview);
            }
            other => panic!("expected Editor, got {other:?}"),
        }
    }

    /// `parse_infra_env`: `NAME=default` or a bare `NAME`; a blank default
    /// is absent.
    #[test]
    fn env_converts_structurally() {
        let source = "::env[tier=required]\n\
                      DATABASE_URL\n\
                      PORT=8080\n\
                      LOG_LEVEL=\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::InfraEnv { tier, entries } => {
                assert_eq!(tier.as_deref(), Some("required"));
                assert_eq!(entries.len(), 3);
                assert_eq!(entries[0], NativeEnvEntry { name: "DATABASE_URL".into(), default_value: None });
                assert_eq!(entries[1], NativeEnvEntry { name: "PORT".into(), default_value: Some("8080".into()) });
                assert_eq!(entries[2], NativeEnvEntry { name: "LOG_LEVEL".into(), default_value: None });
            }
            other => panic!("expected InfraEnv, got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Schema v10 (S11 + S12): the last twenty. One test per variant,
    // parsed from real source, so the parser and the arm are pinned
    // together; the six web-only ones were Markdown strings before.
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn health_converts_structurally() {
        match convert_first("::health[path=/healthz method=GET grace=10s interval=30s timeout=5s]\n::\n") {
            NativeBlock::Health { path, method, grace, interval, timeout } => {
                assert_eq!(path.as_deref(), Some("/healthz"));
                assert_eq!(method.as_deref(), Some("GET"));
                assert_eq!(grace.as_deref(), Some("10s"));
                assert_eq!(interval.as_deref(), Some("30s"));
                assert_eq!(timeout.as_deref(), Some("5s"));
            }
            other => panic!("expected Health, got {other:?}"),
        }
    }

    #[test]
    fn hours_converts_structurally_with_minutes() {
        match convert_first("::hours[title=\"Hours\" timezone=\"America/Los_Angeles\"]\nMonday: 11am - 9pm\nSunday: Closed\n::\n") {
            NativeBlock::Hours { title, timezone, rows } => {
                assert_eq!(title.as_deref(), Some("Hours"));
                assert_eq!(timezone.as_deref(), Some("America/Los_Angeles"));
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].day, 1);
                assert_eq!(rows[0].label, "Monday");
                assert_eq!(rows[0].opens, Some(11 * 60));
                assert_eq!(rows[0].closes, Some(21 * 60));
                assert_eq!(rows[0].text, "11am - 9pm");
                assert_eq!(rows[1].day, 0);
                assert_eq!(rows[1].opens, None, "a closed day carries no minutes");
                assert_eq!(rows[1].text, "Closed");
            }
            other => panic!("expected Hours, got {other:?}"),
        }
    }

    #[test]
    fn marquee_smoke_use_and_volumes_convert_structurally() {
        match convert_first("::marquee\n- Fresh daily\n- Open late\n::\n") {
            NativeBlock::Marquee { items } => assert_eq!(items, vec!["Fresh daily", "Open late"]),
            other => panic!("expected Marquee, got {other:?}"),
        }
        match convert_first("::smoke[script=/smoke.sh]\nGET /healthz -> 200\nPOST /api/x -> 201\n::\n") {
            NativeBlock::Smoke { script, checks } => {
                assert_eq!(script.as_deref(), Some("/smoke.sh"));
                assert_eq!(checks, vec![
                    NativeSmokeCheck { method: "GET".into(), path: "/healthz".into(), expected: 200 },
                    NativeSmokeCheck { method: "POST".into(), path: "/api/x".into(), expected: 201 },
                ]);
            }
            other => panic!("expected Smoke, got {other:?}"),
        }
        match convert_first("::use\n- reqwest 0.12 [json, rustls-tls]\n- lettre\n::\n") {
            NativeBlock::Use { crates } => {
                assert_eq!(crates, vec![
                    NativeCrateDep { name: "reqwest".into(), version: Some("0.12".into()), features: vec!["json".into(), "rustls-tls".into()] },
                    NativeCrateDep { name: "lettre".into(), version: None, features: vec![] },
                ]);
            }
            other => panic!("expected Use, got {other:?}"),
        }
        match convert_first("::volumes\ndata -> /data\ncache -> /var/cache\n::\n") {
            NativeBlock::Volumes { entries } => {
                assert_eq!(entries, vec![
                    NativeVolumeEntry { name: "data".into(), mount: "/data".into() },
                    NativeVolumeEntry { name: "cache".into(), mount: "/var/cache".into() },
                ]);
            }
            other => panic!("expected Volumes, got {other:?}"),
        }
    }

    #[test]
    fn related_converts_structurally() {
        match convert_first("::related\n- [Architecture Plan](plans/plan.md) \u{2014} produces\n- consumes: research/FINDINGS.md\n- plans/wiki.md\n::\n") {
            NativeBlock::Related { items } => {
                assert_eq!(items, vec![
                    NativeRelatedItem { title: Some("Architecture Plan".into()), href: "plans/plan.md".into(), relation: Some("produces".into()) },
                    NativeRelatedItem { title: None, href: "research/FINDINGS.md".into(), relation: Some("consumes".into()) },
                    NativeRelatedItem { title: None, href: "plans/wiki.md".into(), relation: None },
                ]);
            }
            other => panic!("expected Related, got {other:?}"),
        }
    }

    #[test]
    fn turn_converts_with_a_resolved_role() {
        match convert_first("::turn[participant=claude time=2026-02-10T04:01Z model=opus]\nYes \u{2014} file before launch.\n::\n") {
            NativeBlock::Turn { participant, time, role, model, content } => {
                assert_eq!(participant, "claude");
                assert_eq!(time.as_deref(), Some("2026-02-10T04:01Z"));
                assert_eq!(role, "ai", "inferred from the participant");
                assert_eq!(model.as_deref(), Some("opus"));
                assert_eq!(content, "Yes \u{2014} file before launch.");
            }
            other => panic!("expected Turn, got {other:?}"),
        }
        match convert_first("::turn[participant=brady role=human]\nShould we?\n::\n") {
            NativeBlock::Turn { role, .. } => assert_eq!(role, "human"),
            other => panic!("expected Turn, got {other:?}"),
        }
        match convert_first("::turn[participant=user timestamp=2026-02-22]\nCan you?\n::\n") {
            NativeBlock::Turn { role, time, .. } => {
                assert_eq!(role, "human");
                assert_eq!(time.as_deref(), Some("2026-02-22"), "timestamp= reads as time=");
            }
            other => panic!("expected Turn, got {other:?}"),
        }
    }

    #[test]
    fn timeline_converts_structurally() {
        match convert_first("::timeline[title=\"Product Milestones\"]\n## Q1 2026\n- 2026-01: TaskSurf beta\n- 18:30 \u{2014} Deploy Build #38\n::\n") {
            NativeBlock::Timeline { title, entries } => {
                assert_eq!(title.as_deref(), Some("Product Milestones"));
                assert_eq!(entries, vec![
                    NativeTimelineEntry { when: Some("2026-01".into()), label: "TaskSurf beta".into(), group: Some("Q1 2026".into()) },
                    NativeTimelineEntry { when: Some("18:30".into()), label: "Deploy Build #38".into(), group: Some("Q1 2026".into()) },
                ]);
            }
            other => panic!("expected Timeline, got {other:?}"),
        }
    }

    #[test]
    fn output_ai_generated_and_ai_context_convert_structurally() {
        match convert_first("::output[for=analysis timestamp=\"2026-02-10T12:00:00Z\" exit=0 format=text]\nMean: $12,000\n::\n") {
            NativeBlock::Output { for_id, timestamp, exit, format, content } => {
                assert_eq!(for_id.as_deref(), Some("analysis"));
                assert_eq!(timestamp.as_deref(), Some("2026-02-10T12:00:00Z"));
                assert_eq!(exit, Some(0));
                assert_eq!(format.as_deref(), Some("text"));
                assert_eq!(content, "Mean: $12,000");
            }
            other => panic!("expected Output, got {other:?}"),
        }
        match convert_first("::ai-generated[model=claude-opus-4 date=2026-02-10 reviewed=false]\nThis analysis suggests growth.\n::\n") {
            NativeBlock::AiGenerated { model, date, reviewed, content } => {
                assert_eq!(model.as_deref(), Some("claude-opus-4"));
                assert_eq!(date.as_deref(), Some("2026-02-10"));
                assert!(!reviewed);
                assert_eq!(content, "This analysis suggests growth.");
            }
            other => panic!("expected AiGenerated, got {other:?}"),
        }
        match convert_first("::ai-context[model=opus tokens=2400 loaded=true]\nHow much context was available.\n::\n") {
            NativeBlock::AiContext { model, tokens, loaded, content } => {
                assert_eq!(model.as_deref(), Some("opus"));
                assert_eq!(tokens, Some(2400));
                assert!(loaded);
                assert_eq!(content, "How much context was available.");
            }
            other => panic!("expected AiContext, got {other:?}"),
        }
    }

    #[test]
    fn alternatives_converts_with_padded_rows() {
        match convert_first("::alternatives\n| Option | Pros | Verdict |\n|---|---|---|\n| GTK4 | 5MB | **Selected** |\n| Tauri | 8MB |\n::\n") {
            NativeBlock::Alternatives { headers, rows } => {
                assert_eq!(headers, vec!["Option", "Pros", "Verdict"]);
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].cells, vec!["GTK4", "5MB", "**Selected**"]);
                assert_eq!(rows[1].cells, vec!["Tauri", "8MB", ""], "a short row is padded to the headers");
            }
            other => panic!("expected Alternatives, got {other:?}"),
        }
    }

    #[test]
    fn countdown_css_footnote_and_notes_convert_structurally() {
        match convert_first("::countdown[date=2026-03-15 label=\"Launch day\"]\n::\n") {
            NativeBlock::Countdown { date, label } => {
                assert_eq!(date.as_deref(), Some("2026-03-15"));
                assert_eq!(label.as_deref(), Some("Launch day"));
            }
            other => panic!("expected Countdown, got {other:?}"),
        }
        match convert_first("::css\n.custom-thing { border: 2px dashed red; }\n::\n") {
            NativeBlock::Css { content } => assert_eq!(content, ".custom-thing { border: 2px dashed red; }"),
            other => panic!("expected Css, got {other:?}"),
        }
        match convert_first("::footnote[id=1]\nGartner, 2025. Tier 1 source.\n::\n") {
            NativeBlock::Footnote { id, content } => {
                assert_eq!(id.as_deref(), Some("1"));
                assert_eq!(content, "Gartner, 2025. Tier 1 source.");
            }
            other => panic!("expected Footnote, got {other:?}"),
        }
        match convert_first("::notes\nPause here.\n::\n") {
            NativeBlock::Notes { content } => assert_eq!(content, "Pause here."),
            other => panic!("expected Notes, got {other:?}"),
        }
    }

    #[test]
    fn kernel_logo_cloud_and_subscribe_convert_structurally() {
        match convert_first("::kernel[lang=python env=analysis]\n  runtime: python3.12\n  packages: [numpy, pandas]\n  sandbox: strict\n  memory: 2gb\n::\n") {
            NativeBlock::Kernel { lang, env, runtime, packages, sandbox, properties } => {
                assert_eq!(lang.as_deref(), Some("python"));
                assert_eq!(env.as_deref(), Some("analysis"));
                assert_eq!(runtime.as_deref(), Some("python3.12"));
                assert_eq!(packages, vec!["numpy", "pandas"]);
                assert_eq!(sandbox.as_deref(), Some("strict"));
                assert_eq!(properties, vec![NativeStyleProperty { key: "memory".into(), value: "2gb".into() }]);
            }
            other => panic!("expected Kernel, got {other:?}"),
        }
        match convert_first("::logo-cloud[title=\"Trusted by\"]\n- assets/logos/acme.svg\n- assets/logos/initech.svg | Initech\n::\n") {
            NativeBlock::LogoCloud { title, items } => {
                assert_eq!(title.as_deref(), Some("Trusted by"));
                assert_eq!(items, vec![
                    NativeLogoItem { src: "assets/logos/acme.svg".into(), name: None },
                    NativeLogoItem { src: "assets/logos/initech.svg".into(), name: Some("Initech".into()) },
                ]);
            }
            other => panic!("expected LogoCloud, got {other:?}"),
        }
        match convert_first("::subscribe[action=https://api.example.com/newsletter placeholder=\"you@email.com\"]\nGet notified when we launch.\n::\n") {
            NativeBlock::Subscribe { action, placeholder, content } => {
                assert_eq!(action.as_deref(), Some("https://api.example.com/newsletter"));
                assert_eq!(placeholder.as_deref(), Some("you@email.com"));
                assert_eq!(content, "Get notified when we launch.");
            }
            other => panic!("expected Subscribe, got {other:?}"),
        }
    }

    /// Schema v10: nothing but an unknown directive (and a deck) degrades.
    #[test]
    fn only_unknown_and_deck_degrade_at_schema_v10() {
        for src in [
            "::health[path=/healthz]\n::\n",
            "::hours\nMonday: 9am - 5pm\n::\n",
            "::marquee\n- a\n::\n",
            "::smoke\nGET / -> 200\n::\n",
            "::use\n- serde\n::\n",
            "::volumes\ndata -> /data\n::\n",
        ] {
            let block = &crate::parse(src).doc.blocks[0];
            assert_ne!(block_tier(block), BlockTier::Degraded, "{src}");
            assert!(!matches!(convert_block(block, 0), NativeBlock::Markdown { .. }), "{src}");
        }
        let unknown = &crate::parse("::hologram\nx\n::\n").doc.blocks[0];
        assert_eq!(block_tier(unknown), BlockTier::Degraded);
    }

    #[test]
    fn feed_converts_structurally() {
        match convert_first("::feed[source=/api/events stream=true]\n::\n") {
            NativeBlock::Feed { source, stream } => {
                assert_eq!(source, "/api/events");
                assert!(stream);
            }
            other => panic!("expected Feed, got {other:?}"),
        }
        match convert_first("::feed[source=/api/events]\n::\n") {
            NativeBlock::Feed { stream, .. } => assert!(!stream, "polling by default"),
            other => panic!("expected Feed, got {other:?}"),
        }
    }

    /// Schema v9: an `::app` whose body authors the app-format infra blocks
    /// carries them structurally too — every infra child the spec names.
    #[test]
    fn infra_children_are_structural_at_schema_v9() {
        let source = "::app[name=demo]\n\
                      ::deploy[env=production]\n::\n\
                      ::env[tier=required]\nPORT=8080\n::\n\
                      ::domains\ndemo.surf.space\n::\n\
                      ::database[name=main]\n::\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::App { children, .. } => {
                assert!(matches!(children[0], NativeBlock::Deploy { .. }), "{children:?}");
                assert!(matches!(children[1], NativeBlock::InfraEnv { .. }), "{children:?}");
                assert!(matches!(children[2], NativeBlock::Domains { .. }), "{children:?}");
                assert!(matches!(children[3], NativeBlock::InfraDatabase { .. }), "{children:?}");
            }
            other => panic!("expected App, got {other:?}"),
        }
    }

    /// The serde tags of the two `Infra`-prefixed variants are the spec's
    /// block names, not the Rust names.
    #[test]
    fn infra_variants_serialize_under_the_spec_names() {
        let db = serde_json::to_string(&convert_first("::database[name=main]\n::\n")).unwrap();
        assert!(db.starts_with("{\"type\":\"database\""), "{db}");
        let env = serde_json::to_string(&convert_first("::env\nPORT\n::\n")).unwrap();
        assert!(env.starts_with("{\"type\":\"env\""), "{env}");
    }

    #[test]
    fn segmented_control_converts_structurally() {
        let source = "::segmented-control[active=all size=regular action=setTasksView]\n\
                      - all \"All\"\n\
                      - mine \"Mine\"\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::SegmentedControl { active, size, action, segments } => {
                assert_eq!(active.as_deref(), Some("all"));
                assert_eq!(size, "regular");
                // A bare verb parses as `invoke`, and `raw` keeps the
                // authored text so bare-name registries still resolve.
                let action = action.expect("action= crosses the FFI");
                assert_eq!(action.verb, "invoke");
                assert_eq!(action.target, "setTasksView");
                assert_eq!(action.payload, None);
                assert_eq!(action.raw, "setTasksView");
                assert_eq!(
                    segments,
                    vec![
                        NativeSegmentItem { id: "all".into(), label: "All".into() },
                        NativeSegmentItem { id: "mine".into(), label: "Mine".into() },
                    ]
                );
            }
            other => panic!("expected SegmentedControl, got {other:?}"),
        }
    }

    #[test]
    fn dropdown_select_converts_structurally() {
        let source = "::dropdown-select[label=\"Sort\" icon=arrow selected=\"Newest\" align=right]\n\
                      - \"Newest\" description=\"Most recent first\" action=open:/docs/1 icon=clock\n\
                      - \"Oldest\"\n\
                      ::\n";
        match convert_first(source) {
            NativeBlock::DropdownSelect { label, icon, selected, align, options } => {
                assert_eq!(label.as_deref(), Some("Sort"));
                assert_eq!(icon.as_deref(), Some("arrow"));
                assert_eq!(selected.as_deref(), Some("Newest"));
                assert_eq!(align, "right");
                assert_eq!(options.len(), 2);
                assert_eq!(options[0].label, "Newest");
                assert_eq!(options[0].description.as_deref(), Some("Most recent first"));
                assert_eq!(options[0].icon.as_deref(), Some("clock"));
                let action = options[0].action.clone().expect("option action=");
                assert_eq!((action.verb.as_str(), action.target.as_str()), ("open", "/docs/1"));
                assert_eq!(options[1].action, None);
            }
            other => panic!("expected DropdownSelect, got {other:?}"),
        }
    }

    /// The two borrowed arms are gone: a segmented-control is no longer a
    /// TabBar and a dropdown-select is no longer a CommandPalette.
    #[test]
    fn borrowed_arms_are_retired() {
        assert!(matches!(
            convert_first("::segmented-control\n- one\n::\n"),
            NativeBlock::SegmentedControl { .. }
        ));
        assert!(matches!(
            convert_first("::dropdown-select\n- \"One\"\n::\n"),
            NativeBlock::DropdownSelect { .. }
        ));
    }

    /// D-S8-5: a document's own `::style` now reaches [`NativeTheme`].
    /// The pure half (no `uniffi` feature needed): the resolver reads the
    /// block, and the projection carries it.
    #[test]
    fn style_block_theme_inputs_reach_the_native_theme() {
        let doc = crate::parse("::style\naccent: #ff0000\n::\n\n# Doc\n").doc;
        let inputs = crate::resolve::style_theme_inputs(&doc.blocks);
        assert_eq!(inputs.accent.as_deref(), Some("#ff0000"));

        let theme = NativeTheme::from(&crate::resolve::resolve_theme_with_fonts(
            inputs.accent.as_deref(),
            inputs.font.as_deref(),
            inputs.heading_font.as_deref(),
            inputs.body_font.as_deref(),
            None,
        ));
        assert_eq!(theme.accent, "#ff0000");
    }

    /// D-S8-5: the split font keys land in the right slots, and the legacy
    /// `font:` still sets both.
    #[test]
    fn style_block_fonts_reach_the_native_theme() {
        let native_theme = |source: &str| {
            let doc = crate::parse(source).doc;
            let i = crate::resolve::style_theme_inputs(&doc.blocks);
            NativeTheme::from(&crate::resolve::resolve_theme_with_fonts(
                i.accent.as_deref(),
                i.font.as_deref(),
                i.heading_font.as_deref(),
                i.body_font.as_deref(),
                None,
            ))
        };

        let split = native_theme("::style\nheading-font: playfair\nbody-font: inter\n::\n");
        let display = split.font_display.expect("heading-font resolves");
        let body = split.font_body.expect("body-font resolves");
        assert!(display.to_lowercase().contains("playfair"), "{display}");
        assert!(body.to_lowercase().contains("inter"), "{body}");

        let legacy = native_theme("::style\nfont: inter\n::\n");
        assert_eq!(legacy.font_display, legacy.font_body);
        assert!(legacy.font_display.is_some());
    }

    /// D-S8-5 must be additive: a document with no `::style` resolves the
    /// same theme it always did.
    #[test]
    fn a_document_without_style_keeps_the_default_theme() {
        let doc = crate::parse("# Just a heading\n").doc;
        let inputs = crate::resolve::style_theme_inputs(&doc.blocks);
        assert_eq!(inputs, crate::resolve::StyleThemeInputs::default());

        let theme = NativeTheme::from(&crate::resolve::resolve_theme_with_fonts(
            inputs.accent.as_deref(),
            inputs.font.as_deref(),
            inputs.heading_font.as_deref(),
            inputs.body_font.as_deref(),
            None,
        ));
        assert_eq!(theme, NativeTheme::from(&crate::resolve::resolve_theme(None, None, None)));
        assert_eq!(theme.accent, crate::resolve::DEFAULT_ACCENT);
        assert_eq!(theme.font_display, None);
        assert_eq!(theme.font_body, None);
    }

    /// D-S8-5 end to end, through the real FFI entry point.
    #[cfg(feature = "uniffi")]
    #[test]
    fn style_block_accent_reaches_a_native_doc() {
        let doc =
            crate::ffi::parse_to_native("::style\naccent: #ff0000\n::\n\n# Doc\n".to_string())
                .expect("parses");
        assert_eq!(doc.theme.accent, "#ff0000");
        assert_eq!(doc.schema_version, NATIVE_DOC_SCHEMA_VERSION);

        let plain = crate::ffi::parse_to_native("# Doc\n".to_string()).expect("parses");
        assert_eq!(plain.theme.accent, crate::resolve::DEFAULT_ACCENT);
    }

    /// Schema v8 end to end, through the real FFI entry point: a manifest's
    /// children cross as their own variants.
    #[cfg(feature = "uniffi")]
    #[test]
    fn manifest_children_cross_the_ffi_structurally() {
        let doc = crate::ffi::parse_to_native(
            "::app[name=demo]\n::auth[provider=email]\n::\n::build[base=rust]\n::\n::\n".to_string(),
        )
        .expect("parses");
        assert_eq!(doc.schema_version, NATIVE_DOC_SCHEMA_VERSION);
        match &doc.blocks[0] {
            NativeBlock::App { children, .. } => {
                assert!(matches!(children[0], NativeBlock::Auth { .. }));
                assert!(matches!(children[1], NativeBlock::Build { .. }));
            }
            other => panic!("expected App, got {other:?}"),
        }
    }

    /// Schema v9 end to end, through the real FFI entry point: the app
    /// format's infra children cross as their own variants.
    #[cfg(feature = "uniffi")]
    #[test]
    fn infra_children_cross_the_ffi_structurally() {
        let doc = crate::ffi::parse_to_native(
            "::app[name=demo]\n::deploy[env=production]\n::\n::domains\ndemo.surf.space\n::\n::\n".to_string(),
        )
        .expect("parses");
        assert_eq!(doc.schema_version, NATIVE_DOC_SCHEMA_VERSION);
        match &doc.blocks[0] {
            NativeBlock::App { children, .. } => {
                assert!(matches!(children[0], NativeBlock::Deploy { .. }));
                assert!(matches!(children[1], NativeBlock::Domains { .. }));
            }
            other => panic!("expected App, got {other:?}"),
        }
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
            caption: None,
            total: Vec::new(),
            id: None,
            format: DataFormat::Table,
            sortable: true,
            headers: vec!["Name".to_string(), "Age".to_string()],
            rows: vec![vec!["Alice".to_string(), "30".to_string()]],
            name: None,
            source: None,
            source_rows: None,
            source_cols: None,
            raw_content: String::new(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::DataTable {
                caption: None,
                total: Vec::new(),
                headers: vec!["Name".to_string(), "Age".to_string()],
                rows: vec![vec!["Alice".to_string(), "30".to_string()]],
                sortable: true,
            }
        );
    }

    #[test]
    fn native_data_table_empty() {
        let block = Block::Data {
            caption: None,
            total: Vec::new(),
            id: None,
            format: DataFormat::Table,
            sortable: false,
            headers: vec![],
            rows: vec![],
            name: None,
            source: None,
            source_rows: None,
            source_cols: None,
            raw_content: String::new(),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::DataTable {
                caption: None,
                total: Vec::new(),
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
            min: None,
            max: None,
            label: "MRR".to_string(),
            value: "$2K".to_string(),
            trend: Some(Trend::Up),
            unit: Some("USD".to_string()),
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::Metric {
                min: None,
                max: None,
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
            min: None,
            max: None,
            label: "Users".to_string(),
            value: "100".to_string(),
            trend: None,
            unit: None,
            span: syn(),
        };
        assert_eq!(
            convert_block(&block, 0),
            NativeBlock::Metric {
                min: None,
                max: None,
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
                anchor: None,
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

    /// Schema v7 retired this block's Markdown fallback: `::style` now
    /// crosses as its own variant carrying the authored properties.
    #[test]
    fn style_no_longer_falls_back_to_markdown() {
        let block = Block::Style {
            properties: vec![StyleProperty {
                key: "bg".to_string(),
                value: "blue".to_string(),
            }],
            span: syn(),
        };
        match convert_block(&block, 0) {
            NativeBlock::Style { properties } => {
                assert_eq!(
                    properties,
                    vec![NativeStyleProperty { key: "bg".into(), value: "blue".into() }]
                );
            }
            other => panic!("Expected Style, got {:?}", other),
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
                    group: None,
                },
                FormField {
                    label: "Email".to_string(),
                    name: "email".to_string(),
                    field_type: FormFieldType::Email,
                    required: true,
                    placeholder: None,
                    options: vec![],
                    group: None,
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
                        group: None,
                    },
                    NativeFormField {
                        label: "Email".to_string(),
                        name: "email".to_string(),
                        field_type: "email".to_string(),
                        required: true,
                        placeholder: None,
                        options: vec![],
                        group: None,
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
            (FormFieldType::Checkbox, "checkbox"),
            (FormFieldType::Radio, "radio"),
            (FormFieldType::Toggle, "toggle"),
            (FormFieldType::File, "file"),
            (FormFieldType::Hidden, "hidden"),
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
                    group: None,
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
                group: None,
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
                anchor: None,
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
                anchor: None,
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

    /// 0.18.1 (schema v6): the new block attributes must cross the FFI, not
    /// die at the boundary the way `NativeFormField.group` nearly did.
    #[test]
    fn native_0_18_1_attributes_cross_the_ffi() {
        let src = "::data[caption=\"Q3\"]\n| Line | Amount |\n|---|---|\n| Coffee | 800 |\ntotal: All | 800\n::\n\n\
                   ::metric[label=Seats value=42 min=0 max=100]\n::\n\n\
                   ::progress[value=30 max=120]\n::\n\n\
                   ::product-card[price=49 currency=USD]\n## Corpus Co Pro\nBody.\n::\n\n\
                   ::pricing-table[highlight=Pro current=Free]\n| Tier | Price |\n|---|---|\n| Free | $0 |\n| Pro | $20 |\n::\n";
        let blocks = to_native_blocks(&crate::parse(src).doc);
        let mut seen = 0;
        for b in &blocks {
            match b {
                NativeBlock::DataTable { caption, total, .. } => {
                    assert_eq!(caption.as_deref(), Some("Q3"));
                    assert_eq!(total, &vec!["All".to_string(), "800".to_string()]);
                    seen += 1;
                }
                NativeBlock::Metric { min, max, .. } => {
                    assert_eq!(min.as_deref(), Some("0"));
                    assert_eq!(max.as_deref(), Some("100"));
                    seen += 1;
                }
                NativeBlock::Progress {
                    value, max, steps, ..
                } => {
                    assert_eq!(value.as_deref(), Some("30"));
                    assert_eq!(max.as_deref(), Some("120"));
                    assert!(steps.is_empty());
                    seen += 1;
                }
                NativeBlock::ProductCard { price, currency, .. } => {
                    assert_eq!(price.as_deref(), Some("49"));
                    assert_eq!(currency.as_deref(), Some("USD"));
                    seen += 1;
                }
                NativeBlock::PricingTable {
                    highlight, current, ..
                } => {
                    assert_eq!(highlight.as_deref(), Some("Pro"));
                    assert_eq!(current.as_deref(), Some("Free"));
                    seen += 1;
                }
                _ => {}
            }
        }
        assert_eq!(seen, 5, "all five carriers must be present: {blocks:?}");
    }

    /// The `group` label a `::form` fieldset carries must survive conversion.
    #[test]
    fn native_form_field_group_crosses_the_ffi() {
        let src = "::form\ngroup: Contact\n- Name (text)\n- Email (email)\n::";
        let blocks = to_native_blocks(&crate::parse(src).doc);
        match &blocks[0] {
            NativeBlock::Form { fields, .. } => {
                assert_eq!(fields.len(), 2);
                for f in fields {
                    assert_eq!(f.group.as_deref(), Some("Contact"));
                }
            }
            other => panic!("Expected Form, got {other:?}"),
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
                        group: None,
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
        // 0.22: the eight web-only blocks — schema v7. 0.23: the last ten
        // with measured use (the manifest's children) — schema v8. 0.24: the
        // ten infra blocks of the app format — schema v9. 0.25: the last
        // twenty (the six web-only blocks and the fourteen that were
        // planned) — schema v10; every registered block crosses. 0.26:
        // hero/section `anchor` + heading anchors stripped — schema v11.
        assert_eq!(NATIVE_DOC_SCHEMA_VERSION, 11);
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

    // ── schema v11: headline anchors split, heading anchors stripped ──

    fn native_of(src: &str) -> Vec<NativeBlock> {
        to_native_blocks(&crate::parse(src).doc)
    }

    #[test]
    fn v11_hero_headline_anchor_is_split_off() {
        let native = native_of("::hero\n# Home of the *fresh-baked* pita. {#top}\nSub.\n::\n");
        match &native[0] {
            NativeBlock::Hero { headline, anchor, .. } => {
                assert_eq!(headline.as_deref(), Some("Home of the *fresh-baked* pita."));
                assert_eq!(anchor.as_deref(), Some("top"));
            }
            other => panic!("expected Hero, got {other:?}"),
        }
    }

    #[test]
    fn v11_section_headline_anchor_is_split_off() {
        let native = native_of("::section[bg=favorites]\n## Start with the favorites. {#favorites}\nThe plates.\n\nBody.\n::\n");
        match &native[0] {
            NativeBlock::SectionContainer { headline, anchor, bg, .. } => {
                assert_eq!(headline.as_deref(), Some("Start with the favorites."));
                assert_eq!(anchor.as_deref(), Some("favorites"));
                assert_eq!(bg.as_deref(), Some("favorites"));
            }
            other => panic!("expected SectionContainer, got {other:?}"),
        }
    }

    #[test]
    fn v11_headline_without_anchor_is_untouched() {
        let native = native_of("::section\n## Join the family.\n::\n");
        match &native[0] {
            NativeBlock::SectionContainer { headline, anchor, .. } => {
                assert_eq!(headline.as_deref(), Some("Join the family."));
                assert_eq!(anchor, &None);
            }
            other => panic!("expected SectionContainer, got {other:?}"),
        }
    }

    #[test]
    fn v11_markdown_heading_anchors_are_stripped_outside_fences() {
        let md = "## Visit us {#visit}\n\nText {#not-a-heading}\n\n```\n## kept {#x}\n```\n### Deep {#deep}";
        assert_eq!(
            strip_heading_anchors(md),
            "## Visit us\n\nText {#not-a-heading}\n\n```\n## kept {#x}\n```\n### Deep"
        );
        // No anchor → byte-identical.
        let plain = "# Title\n\nBody {with braces}";
        assert_eq!(strip_heading_anchors(plain), plain);
        // A malformed slug is not an anchor.
        assert_eq!(strip_heading_anchors("## A {#bad slug}"), "## A {#bad slug}");
    }

    #[test]
    fn v11_markdown_and_columns_cross_without_heading_anchors() {
        let native = native_of("## Pull up a chair. {#visit}\n\nCome by.\n\n::columns\n:::column\n### Hummus {#hummus}\nSilky.\n:::\n::\n");
        let md = native.iter().find_map(|b| match b {
            NativeBlock::Markdown { content } => Some(content.clone()),
            _ => None,
        });
        assert_eq!(md.as_deref().map(|c| c.contains("{#")), Some(false));
        let cols = native.iter().find_map(|b| match b {
            NativeBlock::Columns { columns } => Some(columns.clone()),
            _ => None,
        });
        let cols = cols.expect("columns");
        assert!(cols.iter().all(|c| !c.content.contains("{#")), "{cols:?}");
        assert!(cols[0].content.contains("### Hummus"));
    }
}

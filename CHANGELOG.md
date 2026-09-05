# Changelog

All notable changes to surf-parse. The crate is consumed by git tag; each
entry below corresponds to a tagged (or about-to-be-tagged) release.

## 0.19.2 — 2026-08-27 (`::data` preview contract)

### Added

- **`DATA_PREVIEW_ROWS`** (`= 20`) — one public constant naming how many body
  rows of a `::data` block the web renderers paint inline. Both web backends
  read it, so the string renderer and the constructive DOM renderer cap at the
  same row and stay byte-identical.
- **`.surfdoc-table-preview`** on the table wrap, with **`data-rows`** (the
  TOTAL body-row count) and **`data-cols`** (the header width or the widest
  row, whichever is larger), whenever a block carries more rows than the cap.
  The capped table keeps its `<tfoot>` summary row and gains a trailing
  **`.surfdoc-table-more`** paragraph reading `N rows · open as spreadsheet`.
- **`.surfdoc-table-wide`** on the wrap of any `::data` table with eight or
  more columns, independent of its row count.
- Stylesheet rules for the contract: the wrap scrolls on both axes with a
  capped height so `thead th` can freeze against it (`position: sticky`, an
  opaque `--surface-alt` background, and an inset hairline shadow because a
  collapsed border does not travel with a sticky cell); a caption-weight, cell-padded
  `.surfdoc-table-more` rule; and a print block where the wrap loses its
  scroll cap, `thead`/`tfoot` repeat as header and footer groups, rows and
  cells never break inside, and `.surfdoc-table-wide` takes the named
  `@page surfdoc-wide` in landscape and is locked to that page box
  (`table-layout: fixed`, wrapping headers, tighter cells) so its last
  columns print instead of overflowing the sheet.

### Changed

- A `::data` block with more than 20 body rows renders a preview in HTML.
  At or under the cap the markup is byte-identical to 0.19.1 — no extra class,
  no `data-` attributes, no count line — so existing snapshots do not churn.
- The native, markdown, LaTeX, Typst, PDF and slides backends and the
  serializer are untouched and never truncated; the Apple kit applies its own
  cap.
- No new block kind, attribute or front-matter value: the block registry,
  the SurfDoc grammar and the spec are unchanged.

## 0.19.1 — 2026-08-26 (front matter `type: specification`)

### Added

- **`type: specification`** joins the front matter type vocabulary as an
  additive entry (`DocType::Specification`). It names a normative standard
  document — the standard itself — as distinct from `type: contract`, which
  names ratified law a build validates against. It resolves to the ordinary
  `RenderProfile::Document`, so rendering is unchanged, and it is accepted by
  the parser, the serializer and the lint enum vocabulary alike: a document
  headed `type: specification` now parses with its front matter intact and
  lints clean instead of falling out of schema and cascading front-matter
  diagnostics from that one cause.
- Every existing type value, render profile and diagnostic code is unchanged;
  this release adds a vocabulary entry and nothing else.

## 0.19.0 — 2026-08-26 (web-shell DOM coverage + serializer fixed point)

### Added — `render_dom` coverage of the whole web-shell census

- **`::row`** (517 uses in the shell — the highest-census block), **`::split-pane`**,
  **`::diagram`** and **`::chart`** now build constructively, alongside markdown
  **tables**, **fenced/indented code blocks** and **code spans**. `check_coverage`
  is now Ok for every vendored web-shell source except the three that emit a
  `<script>`; entry points and signatures are unchanged (additive only).
- **Verified-markup path for generated SVG.** `::diagram` / `::chart` hand back a
  pre-serialized SVG *string*, which cannot carry the `&'static str` bound that
  makes `build_static` safe. `build_verified_markup` supplies the proof instead of
  assuming it: the markup is tokenized into a scratch `NativeDom`, serialized back
  and compared byte-for-byte with the source, and only an exact match is replayed
  into the caller's sink. In this mode the tokenizer also refuses `<script>` /
  `<style>` elements and any attribute outside `attr_allowed`. Anything it cannot
  reproduce declines as `static-svg:diagram` / `static-svg:chart` rather than
  building a tree that disagrees with `render_html`.
- **Attribute allowlist widened by six SVG marker names** (`marker-end`,
  `markerWidth`, `markerHeight`, `orient`, `refX`, `refY`) — all geometry/paint,
  each with an emission proof in
  `render_dom::tests::allowlist_widened_only_for_emitted_leaf_attributes`.

### Changed — the drawer toggle is runtime-owned (verified on the main thread, 2026-08-26)

- **`APP_SHELL_PANEL_JS` is GONE from both backends.** A shell with a direct
  right `::panel` no longer emits the self-contained drawer-toggle `<script>`
  (spec web-runtime-v1 §2: script-emitting behavior moves to the versioned
  runtime at P3/P4). The markup keeps the entire state contract the runtime
  drives — `data-panel-open` on the root, `aria-hidden` on the panel,
  `aria-expanded` on the FAB and every `[data-action=toggleSurfy]` control —
  and `script_emitting_kind` no longer declines the shell, so the composed
  /next chrome (which ALWAYS carries the Surfy right panel) is constructively
  coverable. Measured live pre-fix: `coverage_check_doc` was false for every
  authenticated /next source, so takeover could never arm. JS-off degradation:
  the drawer stays closed. Hosts owning the toggle: /next's dispatcher
  (guarded fallback when `__surfyWired` is absent); published-site runtimes
  pick it up at P4. Regression: `render_dom::tests::right_panel_shell_covers_without_script`.
- **`RowState::Active` exists, round-trips and renders.** Authored
  `state=active` on `::row` used to parse to `Default` (silently dropped), so
  the active sidebar rail was a CLIENT-side stamp — a mount mutation that can
  never attest. `active` now parses, serializes (`state=active`), and renders
  `is-active` + `aria-current="page"` in both web backends (aria-current
  emitted last, so an idempotent client re-stamp is a byte no-op); native
  reports `"active"` (additive value). Regression:
  `serialize_fixed_point::active_row_state_round_trips_and_renders`.

### Fixed

- **`::summary` emitted `<p>` inside `<p>`.** The arm wrapped
  `render_inline_markdown(content)` — which already returns a paragraph — in a
  second `<p>`. An HTML parser auto-closes the outer `<p>` at the inner one, so
  the SSR-parsed DOM diverged from the literal tree `render_dom` builds: identical
  bytes, different DOM, which only attestation would have caught. The body now
  goes through `render_wrapped_phrasing_or_blocks` (and
  `build_wrapped_phrasing_or_blocks` on the DOM side), so phrasing-only summaries
  keep the historical single `<p>` and block-level bodies get a `<div>`. Pinned by
  the parser-stability gate over the web-shell corpus.
- **The script-emitting gate missed nested containers.** `find_script_emitter`
  recursed through `page` / `section` / `app-shell` / `sidebar` / `panel` / `modal`
  but not `tab-content`, `drawer` or `split-pane` panes, so a `::gallery` inside a
  `::tab-content` reported the document constructible while `render_html` emitted
  its lightbox `<script>`. All covered container kinds are now walked.

### Fixed — serializer

- **Nested blocks serialize at the right fence depth.** `to_surf_source`
  emitted every block with a two-colon fence regardless of nesting, so the
  source it produced re-parsed as a FLAT sibling list: a `::app-shell`
  holding a sidebar, toolbars, rows and a panel came back as an empty
  `::app-shell` followed by ~17 top-level blocks. `serialize_block` now
  carries a depth and emits `::` / `:::` / `::::` to match, including the
  child fences the `::columns` (`:::column`) and `::split-pane` (`:::pane`)
  arms inline.
- **Closer-less leaves nested in a container now carry a closer.**
  `::divider`, `::toc` and `::logo` serialize as a single line, which is
  canonical at top level. Nested, the unmatched opener left the parser's
  leaf-vs-container look-ahead (`parse::is_leaf_before_sibling`) with a
  non-zero pending count, so the ENCLOSING container was classified as a leaf
  as soon as a same-depth directive appeared later in the document — one
  `::::divider` in a sidebar flattened the whole shell. Nested emission is now
  `::::divider` + `::::`; the top-level form is unchanged.
- **`::gallery[columns=N]` survives serialization.** The column count was
  dropped, so a re-parsed gallery fell back to the 3-column default and
  rendered different HTML.
- **`- id "Label"` item labels no longer grow escapes.** `::tab-bar` and
  `::segmented-control` labels are read back with `trim_matches('"')`, which
  does not honour backslash escapes, but were written out through
  `escape_attr` — a label carrying a quote gained one `\` per round trip.
  They are now quoted verbatim (`builder::quote_list_label`). `::dropdown-select`
  options are unaffected: their parse side stops at the second quote.

### Added

- **`tests/serialize_fixed_point.rs`** — `parse(to_surf_source(parse(src)))`
  is now pinned as a fixed point over the whole vendored web-shell corpus
  (`tests/fixtures/web-shell/`, 56 shell/surface/modal sources plus the 5
  hostile sources): the re-parsed document must render byte-identically to
  the first parse AND re-serialize to the same source, the chrome tree must
  keep its shape (no container may come back with its children flattened),
  and the fixture count has an add-only floor so a shrunken corpus fails
  instead of passing quietly. Four regression tests pin the defects above.

No public API changed: `builder::to_surf_source` and `SurfDoc::to_surf_source`
keep their signatures.

## 0.18.1 — 2026-08-26 (registry drift closed, form vocabulary, block attributes, schema v6)

An attributes-and-registry release: no new `Block` variant, no grammar change.
Every addition is an attribute, an enum case, or a registry row.

### Fixed

- **Nested blocks now carry real spans.** Children of `::page`, `::section`,
  `::slide`, `::app-shell`, `::sidebar`, `::panel`, `::tab-content`,
  `::drawer`, `::modal`, `::split-pane` panes and `::app` used to be parsed
  from the container's content string with a placeholder `0..0` span. With
  0.18.1's span-keyed addressing table that placeholder collapsed every
  nested sibling onto one entry, so a page holding `::hero[id=login-hero]`
  and `::form[id=login-form]` rendered BOTH roots as
  `data-block-id="login-form"`. `parse_page_children_in` now anchors each
  child at the container's content start (`block_meta::content_start`), so
  nested spans slice the source at the directive itself (line numbers too)
  and every nested block keeps its own identity — in `data-block-id`, in
  `NativeDoc.block_meta`, and in the native `span` field. Hand-built blocks
  (no parse) keep the placeholder span. Regression:
  `tests/block_addressing_nested.rs`.

### Added

- **Registry drift closed.** `spec/blocks.toml` now registers `banner`,
  `booking`, `store`, `cite` and `bibliography` (all long since implemented in
  the parser) plus `notes` as `planned` — the presenter-notes directive the
  slide parser folds into `Slide.notes`, which has no standalone variant.
  `::store`, `::booking`, `::banner`, `::cite` and `::bibliography` no longer
  raise **L020**. The registry keeps ONE canonical name per directive; the
  parser aliases `reference-def`, `references`, `speaker-notes` and
  `presenter-notes` live in `lint::EXTRA_KNOWN_BLOCK_NAMES` beside the existing
  `action-items` / `info-card` precedent. `meta.total_blocks` 114 → 120.
- **Five form field types.** `FormFieldType` gains `Checkbox`, `Radio`,
  `Toggle`, `File` and `Hidden` (serde lowercase). The grammar is
  `- Label (checkbox)`, and `radio` reads its choices exactly the way `select`
  does: `- Plan (radio: Free | Pro | Team)`. `switch` is an accepted alias of
  `toggle`, `multiline` of `textarea`. The `- <type>: Label` colon shorthand
  learned all five keywords. Both parse sites (`::form` and `::action`) now
  share one spec parser, so they can never drift.
  - HTML: `<input type="checkbox|radio|file|hidden">`; a toggle is a checkbox
    carrying `role="switch"`; a radio group with options emits one
    `<input type="radio">` per choice inside `.surfdoc-form-options`; a hidden
    field emits no label and no wrapper and takes its value from the
    placeholder slot (`- Source (hidden, "pricing-page")`).
  - Native: `field_type` gains the five strings `checkbox`/`radio`/`toggle`/
    `file`/`hidden`.
- **Form field groups.** A `group: Label` line inside `::form` opens a
  `<fieldset>` with that `<legend>`, running until the next `group:` line or
  the end of the block; a bare `group:` closes the run. **Shape:** the group
  name is carried as an optional `group` field on `FormField`, NOT as a new
  vector on `Block::Form` — `Form`'s public fields are unchanged and every
  existing construction site keeps compiling with `group: None`. Renderers
  wrap each run of fields sharing a value. `::action` is unaffected.
- **Block attributes.**
  - `::product-card[price= currency=]` → `<span class="surfdoc-product-price"
    data-currency="…">`. The price is emitted verbatim; the currency rides as
    data so hosts can localise without the parser reformatting money.
  - `::pricing-table[highlight= current=]` — the tier named by `highlight=`
    renders featured and carries `data-highlight`; the tier named by
    `current=` carries `data-current`. Matching is on the col-0 tier label,
    case-insensitive, bold markers ignored.
  - `::data[caption=]` → `<caption>`, and a trailing `total: a | b | c` line
    becomes a `<tfoot>` summary row instead of a data row. Only a line whose
    first token is `total:` counts, so a cell that merely says "Total revenue"
    is untouched.
  - `::metric[min= max=]` → a `<meter>` under the metric card when `max=` is
    set and both numbers parse; a non-numeric value never produces a gauge.
  - `::progress[value= max=]` → a determinate `<progress>` element. `max=`
    defaults to 100. Without `value=` the block keeps its step list exactly as
    before.
  - Registry `attributes` lists updated for `data`, `metric`, `pricing-table`,
    `progress`, `product-card` and `form` (whose row was also missing
    `action`, `method` and `honeypot`).
- **Block addressing — `id=` and `label=` on every block kind.** Both parsed
  silently before; now they are captured and rendered. `id=` becomes
  `data-block-id` and `label=` becomes `aria-label`, spliced into the block
  root's opening tag ahead of the renderer's own attributes
  (`<div data-block-id="hero" class="surfdoc-hero">`); values are
  HTML-escaped. Nested blocks (inside `::page`, `::section`) are addressable
  too. A directive that already spends `id=` on its own semantics — `::data`,
  `::banner` — keeps that meaning and gains the addressing attribute beside
  it.
  - **Where they are captured (design decision).** In a span-keyed side table,
    `src/block_meta.rs`, filled from `blocks::resolve_block` — the one funnel
    both the top-level scan and `parse_page_children` pass through — and read
    back by the renderers through the new `Block::span()` accessor. The two
    alternatives were worse: a field on all 109 `Block` variants is exactly
    what the drift guards exist to prevent, and a field on `SurfDoc` means 64
    struct-literal construction sites plus a serde shape change for every
    JSON/WASM consumer. The table is thread-local and holds one document —
    `parse()` clears it, and the full-document renderers resolve lookups only
    when a hash of `doc.source` matches the source that filled it, so a
    hand-built or previously-parsed document emits nothing rather than
    something wrong.
  - **The `label=` gate.** Six implemented directives already spend `label=`
    on their own semantics (`::metric`, `::cta`, `::divider`, `::action`,
    `::dropdown-select`, `::chip-input`; `::countdown` is registered but
    planned). On those, `label=` stays the caption or button text and no
    `aria-label` is emitted. The gate reads the registry — a directive is
    label-typed exactly when `spec/blocks.toml` lists `label` among its
    `attributes` — so a future row that adopts `label=` is covered without a
    code change.
  - **Native.** `NativeDoc.block_meta`, a span-indexed `NativeBlockMeta` list
    (`start_line`/`end_line`/`start_offset`/`end_offset`/`block_id`/`label`).
    `NativeBlock` is a 75-variant enum, so flat fields on it were impossible;
    the metadata rides beside the tree instead. Empty for documents that
    author neither attribute.
  - **DOM parity.** `render_dom` sets the same two attributes on the block
    root before any other `setAttribute`, so the constructive path stays
    byte-identical to the string path — pinned by a new corpus fixture
    (`tests/fixtures/dom/block-ids.surf`) including a quote/ampersand/angle
    bracket label.
- **Lint L043 — duplicate block id within one page** (warning). Two blocks
  sharing an `id=` make that address ambiguous: the first match wins and the
  second block is unreachable. Scope is the **page**, not the document — a
  site doc's `::page` blocks each serve as their own HTML document, so
  `id=hero` on the home page and on the pricing page is correct authoring.
  **Not fixable:** a machine rewrite would have to invent a new id while every
  reference to the old spelling — a template manifest, an edit request, a
  stylesheet — kept pointing at it, so `fix_source` leaves duplicates exactly
  as authored and the author renames one. `spec/rules.toml` `total_rules`
  18 → 19.

### Changed

- **Native schema v5 → v6** (one bump for the whole release):
  `NativeDoc.block_meta`; `NativeFormField.group`; the five new
  `field_type` strings;
  `NativeBlock::Metric.min`/`.max`, `Progress.value`/`.max`,
  `ProductCard.price`/`.currency`, `PricingTable.highlight`/`.current`,
  `DataTable.caption`/`.total`. Every new field is `Option`/`Vec` with
  `skip_serializing_if`, so existing JSON is byte-unchanged. Clients that pin
  the version — including the Swift kit in `repos/surf` — repin with the
  `v0.18.1` tag.
- **README** block counts corrected from a stale 91 to the real registry
  numbers (120 registered · 106 implemented), and the per-category variant
  lists brought back in line with `spec/blocks.toml`.
- **`docs/architecture.surf`** counts corrected from a stale 32 block types
  (now 120 registered / 106 implemented) along with the line and test totals
  in the same stats block.
- `render_html` grew `render_form_field_html` / `render_form_fields_html`,
  shared by the `::form` and `::action` renderers; `render_dom` mirrors both
  and the byte-identity suite pins them together.
- CSS: rules for `<meter>`, `<progress>`, `<caption>`, `<tfoot>`, fieldset and
  legend, the radio option row, the product price, `[data-current]` tiers, and
  the two previously ruleless store classes (`surfdoc-store-main`,
  `surfdoc-store-cart-items`).
- `Block::span()` — one or-pattern over all 109 variants, so a variant added
  without a `span` field fails to compile.
- One corpus snapshot moved: `tests/snapshots/ffi-hole-closure.html.snap`,
  whose `::banner[id=announce]` is the only fixture block in the corpus that
  authors `id=`. It now carries `data-block-id="announce"` beside its existing
  `id="announce"`. No native snapshot moved.

## 0.18.0 — 2026-08-24 (size-class axis: typed layouts, per-class values, schema v5)

### Added

- **Size-class axis.** `SizeClass {Mobile, Tablet, Desktop}` with the two
  breakpoint constants `SIZE_CLASS_TABLET_MIN = 768` and
  `SIZE_CLASS_DESKTOP_MIN = 1024` (logical pt / dp / css-px at 1x) and the
  total resolver `resolve_size_class(width)`. Resolution happens ONCE, in
  Rust, beside style-pack resolution — no client carries its own breakpoint
  table. Spec: `spec/size-class-axis.surf`.
- **Typed `layout=`.** `::app-shell[layout=]` is the closed set
  `{sidebar-main-panel, tabs, adaptive}`; `layout=adaptive` takes
  `mobile=`/`tablet=`/`desktop=` sub-attrs from `{tabs, rail, sidebar}`.
  `::page[layout=]` is the recognized set `{default, hero, cards, split}`.
  Unknown values degrade to the default and raise new lint **L041** — an
  out-of-vocabulary layout is never a failed render.
- **Per-size-class attribute values.** `cols="1 2 3"` (mobile/tablet/desktop
  order) on `::features` and `::product-grid`, `columns=` on `::gallery`,
  and `width=` on `::sidebar`, `::drawer` and `::tab-content`. A single
  value broadcasts to all three classes and round-trips byte-identically;
  two values let desktop inherit tablet; a non-numeric token degrades the
  attribute to absent.
- **Class conditionals.** `classes=` (comma list) and `min-class=` on
  `::sidebar`, `::panel`, `::tab-content` and `::drawer`.
- `SurfDoc::for_width(width)` / `for_size_class(class)` — the width seam.
  The render paths still take no width; the projection collapses per-class
  values and drops gated chrome, then the ordinary renderers run. Documents
  that use none of the 0.18 attributes project to themselves.
- `role=` in the `::tab-bar` item brace grammar, alongside `icon=` and
  `unread`.
- `::product-grid` is now registered in `spec/blocks.toml` (it was
  implemented but unregistered); `total_blocks` 113 -> 114.
- Front matter: `type: contract` (`DocType::Contract`) and
  `status: ratified` (`DocStatus::Ratified`) — the header values governing
  contract documents carry. Previously serde rejected both, which discarded
  the entire front matter (P005) and cascaded V001/V002 from that one
  cause.
- `assets/surfdoc.css` Section 80 — the axis stylesheet. Chrome media
  queries are restricted to 767/768/1023/1024, and a new `css_coverage`
  test asserts those numbers equal the exported Rust constants. The
  480/640/720 content-block queries are untouched.

### Changed

- **`NATIVE_DOC_SCHEMA_VERSION` 4 -> 5.** New records `NativePerClassU32`,
  `NativeAdaptiveLayout` and `NativeClassGate`; `Sidebar.width`,
  `Drawer.width` and `TabContent.width` are now per-class; `Gallery.columns`
  and `Features.cols` / `ProductGrid.cols` cross as triples;
  `AppShell.adaptive` is new. The breakpoints cross as the exported
  functions `size_class_tablet_min()` / `size_class_desktop_min()` /
  `resolve_size_class()` (UniFFI 0.28 has no plain-const export).
- `Block::AppShell.layout` is now the typed `AppShellLayout` instead of a
  free `String`, and carries `adaptive: Option<AdaptiveLayout>`.
- `::gallery[columns=]` is now READ. The attribute has been registered since
  0.1 but nothing consumed it; an authored value now wins over the
  item-count heuristic.

### Fixed

- **Three FFI holes closed at schema v5.** `NativeTabBarItem` gained
  `unread` and `role`, and `NativeBlock::TabContent` gained `width` and
  `align` — all four reached HTML but died at the native boundary.

### Deprecated

- `::panel[desktop-only=true]` is a deprecated alias for `classes=desktop`.
  It still parses and normalizes into the same class set; new lint **L042**
  reports it with a safe fix. The raw flag is preserved so re-serialization
  stays a fixed point.

## 0.17.2 — 2026-08-20 (action-items: plain list markers no longer dropped)

### Fixed

- `::action-items` (and `::tasks`) bodies written as plain lists silently
  dropped every line: `parse_tasks` only accepted `- [ ]` / `- [x]`
  checkboxes, so `- text`, `* text`, `1. text`, and `1) text` items parsed
  to an empty `Block::Tasks`. Plain markers are now stripped and captured as
  not-done items, sharing the existing `extract_assignee` path so a trailing
  `@username` behaves identically to the checkbox form. Ordered markers
  require a space after the `.` or `)`, and indented markers are accepted.
  Malformed checkbox remainders (`- []`, `- [ ]` with no trailing space,
  `- [x]done`, `* [ ] text`) keep falling through to the skip arm as before,
  so the literal brackets never land inside an emitted checkbox.
  Marker-less prose lines still drop — unchanged behavior.
- Note on re-emission: typed `Block::Tasks` stores no source marker, so the
  builder normalizes plain markers to `- [ ]` checkbox form and the directive
  name to `::tasks` — the same normalization `::action-items` already
  received on 0.17.1. A test pins re-serialization as a fixed point.
- No uniffi interface changes; FFI binding checksums unaffected.

## 0.17.1 — 2026-08-19 (builder round-trip fix: bracket-leading chat message text)

### Fixed

- Builder round-trip data loss: an attr-less chat message whose text starts
  with `[` lost the leading bracket group to the attrs parse on reparse. The
  builder now emits an explicit empty `[]` attrs group when the message text
  starts with `[`. Regression test added
  (`test_roundtrip_chat_message_bracket_leading_text`). No uniffi interface
  changes; FFI binding checksums unaffected.

## 0.17.0 — 2026-08-19 (Messages mockup-fidelity round: chat-thread children, chip-input, roster rows, shark fin)

Native schema v4 (`NATIVE_DOC_SCHEMA_VERSION` 3 → 4).

### Added
- `::chat-thread` renders REAL message children: `- side[sender= time=
  reactions=] text` dash items parse into `ChatThread.messages`
  (`ChatMessage` / `ChatReaction`). HTML render: mockup bubble anatomy —
  width cap `min(75%, 640px)`, timestamp INSIDE the bubble, sender-name
  lead above incoming bubbles in group threads (>= 2 distinct incoming
  senders), the named Surfy sender always leads (accent variant), and
  read-only reaction pills (ruling D-3: static spans, no button, `mine`
  variant). Attrs-only threads keep the pre-0.17 two-message sample
  preview byte-identical — the parse shape is backward-compatible.
  Native parity: `NativeBlock::ChatThread.messages`
  (`NativeChatMessage` / `NativeChatReaction`); builder round-trips the
  children; markdown degrades to a sequential message list.
- New `chip-input` kind (registry #113) — the compose "To:" line: `label`,
  removable chips (`- ` content lines) with a close glyph per dismiss
  canon, and an inline filter input. HTML carries the SHAPE only (the
  /next dispatcher owns behavior); native `NativeBlock::ChipInput` with a
  typed `on_change`; markdown degrades to a labeled chip list.
- `::row` roster meta: `avatar=` (initials text, `group` = users glyph,
  `auto` = initials derived from the title at parse), `rtime=` right-side
  bucketed time meta, `unread-count=` count pill that replaces the unread
  dot when present (right-side elements only — accent-left-border stays
  BANNED). Avatar swaps the icon slot; avatar-absent rows render
  byte-identical to 0.16. Native `Row.avatar/rtime/unread_count`.
- Golden union fixture `tests/golden/union-0_17.surf` + pinned HTML
  snapshot covering the whole round.

### Changed
- `surfy-fin` vendored glyph replaced with the full Surfy shark mark
  (ruling D-2, re-authored from `brand/surfy/surfy-final.svg` to
  16×16 `currentColor`); all clients inherit via the icon registry —
  no call-site change.

## 0.16.0 — 2026-08-16 (SplitPane crosses the native FFI)

### Added
- `NativeBlock::SplitPane`: `::split-pane` crosses the native FFI boundary
  as a recursive NativeBlock — `ratio`, optional `back_label` /
  `back_action`, and `left` / `right` child block lists (recursive like
  `SectionContainer` and `Slide`; UniFFI boxes recursive enums). Depth
  guard degrades to Markdown at `MAX_SECTION_DEPTH`, matching the other
  containers. Promoted from tier-4 degraded to tier-3 chrome in
  `block_tier`.

## 0.15.0 — 2026-08-13 (zero-sink train: `dom` backend + TT-clean SSR)

The zero-sink pilot train. Adds the constructive DOM render backend and
removes every Trusted-Types-incompatible construct from the HTML render
surface. Spec: `spec/web-runtime-v1.surf`.

### Added
- `render_dom` backend behind the new `dom` feature (off by default; never
  in server/native builds): `DomSink` abstraction with a native arena sink
  (byte-exact serializer, drives the identity corpus) and a wasm32/web-sys
  sink. Covers the pilot block census byte-identical to `render_html`;
  everything else gets a typed `Unimplemented(kind)` decline.
  `coverage_check` dry-runs the native sink and also declines
  TT-inconstructible output (script-emitting blocks: store, booking,
  gallery-lightbox — `<script>` text is itself a TrustedScript sink).
- Cross-backend byte-identity corpus (`tests/render_dom_identity.rs` +
  `tests/fixtures/dom/`), hostile fixtures included — never-weaken.
- Web runtime spec `spec/web-runtime-v1.surf` (DOM rendering law,
  constructive navigation contract, security profile) + architecture-doc
  DOM Renderer section.

### Changed (HTML render surface — TT-clean SSR)
- Image fallbacks: every inline `onerror` handler (a TrustedScript sink)
  replaced with `data-img-fallback` attributes (`hide` / `broken` / `logo`
  + `data-img-fallback-text`); the serving shell's single delegated
  capture-phase error listener performs the swap. Sites: figure, gallery,
  hero (both layouts), `::hero-image`, nav-shell logo, product-grid
  emblems (tile + row).
- Store/booking widget JS rewritten sink-free: `replaceChildren` /
  `createElement` / `textContent` only (7 former `innerHTML` sites) — the
  widgets now run under `require-trusted-types-for 'script'` with no
  policy defined.
- Parser-stable emission for block-bearing bodies (feature bodies et al.):
  phrasing-only bodies keep the historical `<p>` byte-for-byte;
  block-bearing bodies emit a `<div>` the HTML parser nests literally
  (fixes parser-hoisting divergence — bytes identical, DOMs different —
  which byte-identity tests structurally cannot catch). Pinned by
  never-weaken parser-stability tests in both backends.

## 0.14.1 — 2026-08-12 (R-lane UI fix round: D3/D4/D5+D8/D9)

HTML/CSS render surface only; registry/native schema untouched apart from
one additive, optional `::row` attribute.

### Fixed
- D3 — doc-detail toolbar in the 880px web detail column: tab-content
  toolbars now wrap (`flex-wrap: wrap`, `overflow-x: visible`, row-gap)
  instead of growing a horizontal scroller under the bar; long breadcrumb
  text ellipsizes. The desktop chrome-bar scroll escape valve from 0.13.1
  (`b1e5723`) is untouched — the change is scoped to
  `.surfdoc-tab-content` toolbars, which are content rows, not chrome.
- D5 + D8 — modal chrome: toolbars inside a modal no longer paint the
  chrome-bar treatment (dark full-width band + hairline behind secondary
  buttons); a modal-scoped reset makes them transparent, borderless,
  wrapping, with real vertical padding (also D4's roomier modal button
  rows). Modal corner radius raised to a sheet-scale 16px in both the
  base rule and a post-containment override, keeping the overflow clip so
  children cannot square the corners.
- D9 — filter-dropdown dead zone: the toolbar-dropdown pill release
  (`min-width: 0`, `margin: 0`) now carries `.surfdoc-dropdown-select`
  specificity so the later base rule can no longer re-impose its 220px
  min-width on the pill, and the trigger fills its pill (`width: 100%`)
  instead of sizing to its label — clicks right of the text land on the
  button, not an inert wrapper. Diagnosed as crate-side CSS geometry; no
  surf dispatcher change needed.

### Added
- D4 — `::row[progress=0.42]`: optional token-usage fraction (0..=1) on
  row blocks, rendered as a `surfdoc-row-progress` /
  `surfdoc-row-progress-fill` bar under the row description with
  `role="progressbar"` and percentage aria values. Absent attribute =
  byte-identical output.

## 0.14.0 — responsive app-shell chrome + Surfy right drawer (R-series rulings)

One responsive shell (ruling R-A): the renderer itself now emits the
small-screen navigation and the right-hand Surfy drawer — no web-layer
markup required. Registry/native schema untouched; everything below is
HTML/CSS render surface, minor bump for the new emitted structures.

### Added
- Surfy right drawer (WP-R2): `:::panel[position=right]` renders as a
  drawer — `surfdoc-panel-right` (role=complementary, aria-hidden) with a
  fixed-width `surfdoc-panel-inner` wrapper so content never squashes
  while the width animates. Wide screens: in-flow right column, 0 → 360px
  transition, blur surface, left hairline, main pane reflows. Medium
  (≤1023px): absolute overlay, `min(400px, 100vw)`, shadow, no reflow.
  Small (≤767px): full-width takeover; tab-bar + FAB hidden while open.
  Bottom/left panel output is byte-identical to 0.13.3.
- Drawer state + FAB (WP-R2): a shell with a direct right-panel child
  stamps `data-panel-open="false"` on `surfdoc-app-shell` and appends a
  `surfdoc-panel-fab` toggle (surfy fin glyph, aria-expanded/-controls),
  visible only ≤767px as the accent circle FAB. A self-contained inline
  script flips the state (also honoring `[data-action=toggleSurfy]`
  topbar buttons), syncs ARIA, closes on Escape, and returns focus. No
  persistence — the drawer is closed on every load (ruling R-D).
- Generated tab-bar (WP-R1, ruling R-A): the shell renders a
  `surfdoc-app-tabbar` floating pill from the sidebar's nav rows (rows up
  to the first divider) — icon over a 10px Surf Display label, corner
  unread dot, active item accent-on-accent-soft, matched against the
  shell's initially active tab pane. Hidden above 767px; distinct from
  the document-strip `surfdoc-tab-bar`.

### Changed
- Responsive app-shell chrome (WP-R1): new `max-width: 1023px` /
  `max-width: 767px` media blocks scoped to the shell. Medium: 68px
  icon-only sidebar rail (wordmark, labels, hub rows hidden; rows
  centered). Small: sidebar hidden in favor of the generated tab-bar,
  topbar Surfy pill + adjacent separator hidden, content bottom padding
  110px, detail bars wrap with full-width titles, sheets go edge-to-edge,
  kanban board columns stack vertically, and split panes collapse to one
  plane driven by `data-thread="open"` (ruling R-C).
- Native topbar treatments (WP-R5): the shell topbar's
  `[data-action=openSearch]` button renders icon-only and
  `[data-action=toggleSurfy]` renders as the branded accent-tinted pill
  directly from `surfdoc.css` — previously web-layer overrides in
  next-shell.css. Scoped to the direct-child topbar only.
- Section 74's legacy interactive-block media query moved from 768px to
  767px so the medium icon-rail range (768–1023px) is not clipped by the
  old sidebar-hiding rule at exactly 768px. Its single-column
  `grid-template-columns: 1fr` shell override is removed: children are
  pinned to grid-column 2, so it stranded the main pane in an implicit
  auto column beside an empty 1fr track — with the 3-track base template,
  column 1 auto-collapses when the sidebar hides. The medium rail also
  carries min/max-width clamps so the renderer's inline
  `style="width:NNNpx"` (from a sidebar `width=` attr) cannot pin the
  rail at desktop width.

### Added (drawer polish round, WP-N0/WP-N1)
- Drawer anatomy composer: a right panel's children now compose to the
  ruled drawer shape regardless of source block order — ONE head row
  (`div.surfdoc-panel-head`: `span.surfdoc-panel-fin` carrying the new
  bare 26px accent fin glyph, then the first dropdown-select as the tier
  switcher, then the first toolbar's items inlined without their
  `surfdoc-toolbar` wrapper), the grounding chip, a flex-1
  `div.surfdoc-panel-body` region, and the composer pinned last. The
  body region takes all remaining height whatever the panel's child
  count, so the composer no longer lands under the prose when the panel
  has no chat-thread child.
- Head tier switcher: the panel-head dropdown-select renders in
  toolbar-dropdown clothing (`div.surfdoc-dropdown-select
  .surfdoc-toolbar-dropdown` > `button.surfdoc-dropdown-trigger` >
  `span.surfdoc-dropdown-selected`) with ONLY the selected value as its
  title — the `label` attr never paints beside `selected`, killing the
  "Surfy Standard / Standard" duplication — and the caret flips 180°
  under `.is-open`.
- Grounding chip (cross-lane contract): every right panel emits
  `div.surfdoc-panel-grounding[hidden]` directly below the head row,
  containing an empty `span.surfdoc-grounding-label` and
  `button.surfdoc-grounding-clear` with `data-action=clearSurfyGrounding`
  and `aria-label="Clear grounding"`. Hidden when empty (explicit
  `[hidden]{display:none}` rule so the flex display cannot defeat the
  attribute), no persistence.
- Attach control (cross-lane contract): the drawer composer emits
  `button.surfdoc-chat-attach` with `data-action=attachToSurfy`,
  `aria-label="Attach"` and the registry plus glyph BEFORE the input in
  the chat-input row, styled as a 32px gray surface-alt circle
  (deliberately not accent). The old blanket rule that grayed every
  drawer chat button is gone, so Send returns to its accent styling.
  Bottom/left-panel and standalone `chat-input-simple` markup stays
  byte-identical (golden pin unchanged).
- `surfy-fin` is now a real registry glyph (`assets/icons/surfy-fin.svg`,
  bare dorsal-fin silhouette on the 24px grid) instead of an alias onto
  the whole shark `surfy.svg`; the FAB and the drawer head pick it up
  automatically. The fin-head treatment is native, so the frontend mask
  stopgap (surf repo `frontend/static/css/next-shell.css`, the
  `.surfdoc-panel-right .surfdoc-dropdown-trigger[data-icon="surfy-fin"]
  ::before` rule, ~lines 184–196) can be DELETED per its own ritual —
  it is inert anyway now that the native head drops `data-icon` from
  the trigger.
- `messages` glyph re-vendored from the brand trace
  (`brand/icons-new/messages-new-icon.svg`), normalized from the 1024
  viewBox onto the 24px grid (scale 0.0234375), currentColor, intrinsic
  16px. Measured weight is identical to the outgoing glyph (same 0.91px
  outline thickness at 24px; scanline-rasterized ink coverage 200.1px²
  at 48px for both), so no tiny-size thickening was needed.

### Added (messages round 2, G1–G3)
- Split-pane children (G2): `::split-pane` is no longer a leaf — authored
  `:::pane[side=left]` / `:::pane[side=right]` children render into the
  two planes (`surfdoc-split-left` / `surfdoc-split-right`) through the
  same chrome-children path as sidebar/panel bodies, so a messages
  surface authors its roster rail in the left pane and toolbar +
  chat-thread + composer in the right. `side` is optional — order is the
  fallback (first pane left, second right); stray non-pane children fall
  to the left pane; an empty split-pane keeps the historical two empty
  divs byte for byte. `pane` is registered in spec/blocks.toml (112
  blocks), which also silences lint L020 for authored panes.
- Two-plane back control + state (G2): `::split-pane` takes optional
  `back-label` / `back-action` attrs; when either is present the renderer
  emits `button.surfdoc-split-back` (with `data-action` from
  `back-action`) as the first child of the right plane. It lands on the
  hooks that shipped in the responsive round: hidden everywhere except
  the ≤767px thread-open state, where `data-thread="open"` swaps the
  planes. A shell with a split-pane among its descendants now stamps
  `data-thread="closed"` on `surfdoc-app-shell` so the live layer has an
  explicit attribute to flip (flipping stays a live-layer duty; no
  persistence). Known gap, recorded: `data-ratio` is emitted but no CSS
  consumes it yet, so authored ratios have no visual effect.
- Row-level action passthrough (G3): `::row[action=…]` is now
  first-class — the verb is stamped verbatim (escaped, no interpretation)
  as `data-action` on the row root (anchor or div), matching the existing
  trailing-control / per-row-button / toolbar emission pattern, so
  dispatcher verbs (openConversation, askSurfyDoc, askSurfyTask) are
  reachable from authored markup. Generated tab-bar items forward the
  same verb. Bottom/left panel markup stays byte-identical (guard green).
  Native schema untouched. The tolerated-unknown-attrs pin was rewritten
  deliberately (its stated contract for adoption): the `action` hook
  marker moved from `::row` onto `::callout`, and the adoption is pinned
  positively (`adopted_row_action_reaches_html_as_data_action`).

### Changed (messages round 2)
- The `knowledge` design-vocabulary alias now resolves to the vendored
  `messages` trace instead of `book-open` (G1): every authored
  `icon=knowledge` site in the shell/surface sources is messages-semantic
  (messages nav rows, roster rows), so the alias exists solely to glyph
  those rows — rail, medium rail, and generated tab-bar all pick up the
  filled messages glyph through the one alias entry. `book-open` stays
  reachable under its own name; no other alias targets it.

### Fixed (messages round 2)
- Builder round-trip for `::row` / `::infocard`: the serializers joined
  attrs with `", "` but the attr grammar rejects commas, so every
  serialized row silently lost its attrs on re-parse (icon fell back to
  `doc`; href, unread, trailing controls — and the new `action` verb —
  vanished). Both now join with spaces, matching authored syntax; pinned
  by `test_roundtrip_row_attrs_and_split_pane_children`, which also pins
  the new split-pane pane-children serialization end to end.

### Fixed (drawer polish round 3, DOM-probe defects)
- Drawer-head close ✕ clipped off-viewport at 1280: the tier dropdown in
  the drawer head inherited the content dropdown-select's 220px min-width
  (Section 72b), inflating the head row past the 360px inner width and
  pushing the ✕ to x-right 1313. A panel-head-scoped
  `.surfdoc-dropdown-select { min-width: 0 }` releases it — 3-class
  selector, so it wins whatever the source order; toolbar and in-content
  dropdowns keep their 220px geometry byte-for-byte. Pinned by
  `panel_head_dropdown_releases_the_base_min_width`.
- Split-pane children clipped at 390/900: the `flex: 1` panes had the
  default `min-width: auto`, so min-content (long thread lines, the
  composer input + Send) forced a pane past its track — roster chevrons
  clipped at 390, thread text and Send overflowed the viewport by ~101px
  at 900. `min-width: 0` on `.surfdoc-split-left/-right` lets panes
  shrink to their track and inner content wrap/ellipsize. Pinned by
  `split_pane_children_shrink_to_their_track`.

### Fixed (drawer polish round 4, DOM-probe defects)
- Tier dropdown OPTIONS popup collapsed to a ~91px column at panel-head
  placement (option descriptions wrapping to 11 lines): the round-3
  head-scoped min-width release correctly shrank the closed trigger, but
  the absolutely-positioned popup then shrink-to-fit the collapsed
  trigger wrapper. The head-scoped fix pins `width: 220px` on
  `.surfdoc-panel-head .surfdoc-dropdown-options` — `width` on purpose,
  not a min-width floor, because the live layer's is-open options rule
  pins `min-width: 100%` at 5-class specificity but never touches width;
  220px matches the base content dropdown floor, and the popup is
  positioned so it cannot inflate the head row (round-3 fix intact,
  popup fully inside the viewport at 1280). Toolbar and in-content
  options popups stay byte-identical. Pinned by
  `panel_head_dropdown_options_popup_keeps_a_readable_width`.
- Messages thread header title ("Danny Pappageorge — Direct message ·
  cloudsurf workspace") hard-cut at 390 full-plane: the nowrap
  `surfdoc-toolbar-text` flex item kept `min-width: auto`, refusing to
  shrink past min-content and overflowing its bar (title 364.8px in a
  328px bar). Split-right-scoped ellipsis chain on the title element —
  `min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space:
  nowrap` — completes the pane's min-width:0 chain; every other toolbar
  title keeps its current geometry. Pinned by
  `split_right_toolbar_title_ellipsizes_instead_of_hard_cutting`.

## 0.13.3 — mockup-parity chrome round 2 (G-series rulings)

Shell/surface parity with the web-surfspace design mockup. Icons stay a
pure render concern and the only typed change is a new optional
`ToolbarItem::Button` field (same precedent as `icon` — no native schema
field), hence a patch bump.

### Added
- Icon registry (WP-A): the 127-glyph surf-icons web set (CloudSurf's own
  MIT icon library) is vendored under `assets/icons/` and folded into
  `icons::get_icon` behind the existing built-in constants, plus a
  design-vocabulary alias table (`doc`→`docs`, `sort`→`list`,
  `people`/`members`→`users`, …; `knowledge` originally aliased
  `book-open` here — remapped to `messages` in 0.14.0, see above).
  Row icons now resolve from this one registry; unknown names keep the
  circle fallback.
- Icon-only toolbar buttons (G3): `- button[icon=…]` renders its registry
  glyph before the label; a label-less icon button renders icon-only as a
  square pill with an `aria-label` from the action (or icon name).
- Workspace-chip avatar (G6): `- button[… avatar="C"]` renders a circular
  initial badge before the label (`surfdoc-toolbar-avatar`); translucent
  white on primary chips. Web-only render styling — no native field.
- Embedded fonts for static renders (WP-D): opt-in
  `PageConfig::embed_fonts` emits a data-URI `@font-face` for the vendored
  Surf Display Black (`assets/fonts/SurfDisplay-Black.woff2`) plus the
  Inter import, on both page and shell-page paths. Default off =
  byte-identical output. See `docs/embedded-fonts.surf`.
- Explicit toolbar-button accessible names: `- button[… aria-label="New
  Doc"]` parses into a render-only `aria_label` field (same precedent as
  `avatar` — no native schema field) and wins over the action/icon-derived
  fallback on icon-only and label-less avatar buttons, so G3 icon-only
  actions carry their old visible labels instead of raw verb names.
- `bug` icon: bug-type task rows get the mockup's beetle glyph as a
  built-in (preview/mockup.html is design ground truth; supersedes the
  earlier circle-fallback-by-design call).

### Changed
- Toolbar dropdowns are dispatchable (consolidation round): `- dropdown[…]`
  toolbar items render in the dropdown-select markup shape (trigger +
  option list, the item's `data-action` on every option) instead of the
  old inert single-`<option>` bare `<select>` that discarded `options` and
  `action`. In toolbar context the control paints as the same collapsed
  pill (`surfdoc-toolbar-dropdown` modifier: options hidden until
  `.is-open`, floated as a popover).
- Row blocks (G2 ruling): tinted elevated card rows EVERYWHERE, light and
  dark — mockup metrics (13px gap, 11×14px padding, 14px radius, surface
  fill + hairline, hover lift `0 4px 18px` with accent-tinted border) and
  a 34px rounded accent-soft icon badge. The 0.13.2 in-shell ruled-column
  override is removed; in-shell lists stack cards on a 6px rhythm.
- Sidebar rail (G5): active row is a filled accent-soft tint block; icons
  color with their row states (muted → text on hover → accent active); the
  unread dot now rides the icon's top-right corner (7px dot, 2px surface
  ring, absolutely positioned — emitted markup order unchanged; the
  right-side-dot canon still holds outside the rail); tighter row rhythm
  and inset section dividers. Sidebar rows keep the bare 18px glyph slot,
  not the card badge.
- Dark theme (G7 ruling): neutrals shift from the pure-black family to the
  mockup's blue-navy family — bg `#0b1117`, sheet `#151e28`, soft card
  `#1b2734`, text `#e8eef4`, muted `#93a4b3`, hairline `rgba(255,255,255,.09)`,
  accent `#2E8AD8`. Both dark blocks (explicit toggle + auto-detect) move
  in lockstep; mockup faint `#5f7180` fails AA so `--text-faint` lifts to
  `#8496a6` (≥4.5:1 on the soft surface). Light theme unchanged.

### Fixed
- G10: hovering a link row underlined the title and meta — the prose
  `.surfdoc a:hover` underline outranked the row base rule. Anchor rows
  now suppress text decoration for every state, covering title, desc,
  trailing and action spans (same precedent as event-card anchors).

## 0.13.2 — chrome visual design pass

CSS-only product styling for the app-shell chrome families, harmonized with
the production Surf shell (tokens.css / surf-shell.css): app-shell/sidebar/
rows/toolbar/controls product styling; doc-scoped font overrides now paint
(`--ws-*` font indirection moved onto `.surfdoc`).

### Changed
- Sidebar: quiet rail — sheet surface + hairline, nav rows as inset pill
  rows (hover fill, `.is-active`/`aria-current` accent state, hidden
  chevrons, brand strip without bar chrome).
- Main-pane `.surfdoc-row` lists: ruled-column treatment (hairline
  separators, rounded hover fill, title/meta type ramp); standalone rows
  become quiet sheet cards (fill + hairline, no border-color hover).
- `:::toolbar`: 48px header bar on the page surface; buttons/dropdowns as
  bordered pills (30px, production hover/active recipe); toggled state =
  accent-soft fill + accent ring; in-pane toolbars turn transparent with a
  type ramp (display-face title row, uppercase section headers, muted meta).
- `::tab-bar`, `::segmented-control`, `::dropdown-select`, `::modal`,
  command palette, chips, recipient picker: finished control styling —
  radius scale, focus-visible rings, floating-layer shadows
  (`--sd-shadow-*`), 120ms ease-out transitions, accent-soft selected
  states; modal body gets a proper inner gutter.
- App-shell grid tracks auto-size (no dead strips when a shell has no
  tab-bar/panel); chrome typography routes through `--ws-font-body`.
- Dark theme: all of the above holds; dark elevation recipes added.

### Fixed
- Doc-scoped font overrides never painted: `--ws-font-display/--ws-font-body`
  were declared only on `:root`, so the indirection resolved before a
  `.surfdoc { --font-heading: … }` override existed. `--font-heading/--font-body`
  are now unset-by-default (`initial`) with the default stack as var()
  fallback, and the indirection is re-declared on `.surfdoc` with an
  ancestor-captured fallback so style packs / host overrides keep winning
  when the doc itself sets nothing.

## 0.13.1 — unreleased (test-hardening round + toolbar overflow fix)

### Fixed
- Toolbar overflow: the desktop `.surfdoc-toolbar` rule now scrolls
  horizontally (`overflow-x: auto` + `min-width: 0`, and `min-width: 0`
  on its grid placement) instead of clipping when the bar outgrows its
  track — e.g. a 5-facet filter set inside an 880px ruled column. The
  escape valve previously existed only inside the 768px media query.
- `::segmented-control`: exactly one active pill even when segment ids
  are duplicated (first match wins); previously every matching segment
  was marked active.

### Added
- `.surfdoc-toolbar-title` and `.surfdoc-schema` rules (both classes were
  emitted with no styling).
- Test hardening: renderer-fix regressions (tab-content width/align,
  static modal dialog-open, link-row control demotion boundary), a CSS
  coverage guard (every emitted `surfdoc-*` class has a rule, a styled
  sibling, or a justified allowlist entry) with a toolbar-overflow pin,
  an adversarial no-panic + determinism sweep across all registry kinds
  and output formats, container-context unread/dropdown/segmented
  invariants, a committed golden union render pinning the 0.12 + 0.13
  vocabulary, and a tolerated-unknown hook-attr pin (R3: behavior pinned,
  grammar debt intentionally not paid).

## 0.13.0 — unreleased (tag ships this round plus the 0.12 train)

Web chrome round plus the previously untagged 0.12 work, merged.

### Added (0.13 round)
- `::modal`: `width`, `placement` (centered), and `dismissible` attributes.
  The header always renders the title and a top-right close control —
  dismiss canon is baked into the renderer; `dismissible=false` only
  disables backdrop/escape dismissal.
- `::dropdown-select`: new block kind. Trigger attrs `label`, `icon`,
  `selected`, `align`; options carry label, description, icon, action.
- `::segmented-control`: new block kind — compact pill single-select
  (filter idiom, not a tab-bar style). Attrs `active`, `size`, `action`.
  Renders as a radiogroup.
- `unread` attribute on `::::row` and tab-bar items: right-side blue dot.
  Renderer invariant: accent-left-border is banned.
- `toggled` attribute on toolbar button items: accent-ring open state
  with pressed accessibility state.
- `height` attribute on `::app-shell` (overrides the static-render clamp).
- `size` attribute on toolbar text items (wordmark sizing).
- `trailing-label` / `trailing-action` attributes on `::::row`.
- Dividers nested inside `::sidebar` render as hairlines.

### Added (merged 0.12 train)
- Registry-name sources, list streaming and selection callbacks,
  chat-thread seams, toolbar `title` / `title-source` attributes.
- Per-row actions via `action:`-prefixed content lines on `::::row`
  (coexists with the attribute-based trailing action above: the trailing
  control is a single visible button; row actions replace the chevron).
- New kinds: `::recipient-picker` and `::qr`.
- Native schema v3 (Messages/Contacts vocabulary in NativeBlock).

### Registry
- 111 block kinds total (107 in 0.11.0, +2 in 0.12, +2 in this round).

## 0.12.0 — 2026-07-29 (previously untagged; folded into the 0.13 tag)

See the merged-train section above.

## 0.11.0 — tagged v0.11.0

- Real chrome nesting; native list/board/filter-bar/search; tab-bar icons.
- FFI-hole closure for banner, cite, bibliography, gate, product-grid,
  post-grid, slide. Typed native actions. Native schema v2.
- Diagrams: 10 new types (17 total), geometry scenes over FFI.

## 0.10.0 — tagged v0.10.0

- Initial public release: SurfDoc reference implementation.

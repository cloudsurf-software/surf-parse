# Changelog

All notable changes to surf-parse. The crate is consumed by git tag; each
entry below corresponds to a tagged (or about-to-be-tagged) release.

## 0.14.1 — unreleased (R-lane UI fix round: D3/D4/D5+D8/D9)

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

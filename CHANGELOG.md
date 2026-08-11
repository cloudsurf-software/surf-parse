# Changelog

All notable changes to surf-parse. The crate is consumed by git tag; each
entry below corresponds to a tagged (or about-to-be-tagged) release.

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

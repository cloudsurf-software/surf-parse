# Changelog

All notable changes to surf-parse. The crate is consumed by git tag; each
entry below corresponds to a tagged (or about-to-be-tagged) release.

## 0.40.0 — 2026-10-06 (`::segmented-control` `fold=auto` — pills while they fit, the menu when they would not; `fold-at=<px>`)

The navigator controls' `fold=always` (0.39) is the chip for a 320 px panel; a wider panel or a builder's page wants the
pill row while it fits. Script-free is impossible for a true measure, so the rule is the CSS one. Additive — a 0.39
document renders byte for byte (`fold=never` and `fold=always` are untouched).

- **`fold=auto`** renders BOTH the pill row (`<div class="surfdoc-segmented-row">`) and the 0.39 chip menu (the same
  `role=radio` buttons with the same `data-id`, no script; the host owns selection and lights both sets or either).
  The control is an inline-size container (`display: block; container-type: inline-size`); the stylesheet shows the
  row and hides the menu, and one `@container (max-width: …)` query per bucket flips them under the block's
  `data-fold-at`. The control is block-level under `fold=auto`, so a one-row head beside an inline filter bar keeps
  `fold=always`.
- **`fold-at=<px>`** names the width the block folds at; it is rounded UP to one of `SEGMENT_FOLD_BUCKETS` (240 · 320 ·
  400 · 480 · 560 · 640 · 720) because the stylesheet carries one query per bucket. Without it the pill row's width is
  ESTIMATED from the segments (`segmented_fold_at`: ~7 px a label character, 23 px an icon, 22 px of pill padding,
  2 px between pills, 4 px of control padding) and rounded up the same way — the row folds a little early rather than
  clip. The authored value survives the builder round trip; the estimate is never written.
- The constructive DOM renderer mirrors the new markup byte for byte; `spec/blocks.toml` lists `fold-at`.
- Tests: the parser (auto, fold-at, fold-at dropped under other folds), `render_html` (the two sets of buttons, the
  bucket from an authored value, two estimates, the last bucket, the older folds untouched), the builder round trip,
  the DOM identity fixture's third block, and `css_coverage` pins the row's rule and every bucket's query.

## 0.39.0 — 2026-10-06 (the navigator controls — `::segmented-control` icons, tints and `fold=always`; `::filter-bar` as a chip row; TASK-1373's walk, the CloudSurf web Docs panel)

The web Docs panel at app.cloudsurf.com is spec-driven, and its head read as a form because the two navigator
blocks did: seven text pills clipped sideways in a 320 px panel, a Sort in a boxed `<select>`. The blocks are fixed
here so every SurfDoc on the platform gets the look, not one panel. Additive — a 0.37 or 0.38 document renders byte for byte.

- **`::segmented-control` segments carry an icon and a tint.** `- docs "Docs" {icon=file-text tint=blue}` — the
  `::tab-bar` brace idiom. The icon is a glyph from the icon set, drawn as `<span class="surfdoc-icon">` before the
  label (an unknown name draws nothing); the tint is one of ten palette names (`blue green amber red violet teal
  pink orange yellow slate`, `SEGMENT_TINTS`) painted on the glyph only through `data-tint` and the `--tint-*`
  tokens (each falls back to the theme's accent / success / warning / danger); a hex or any other word is dropped at
  parse, never written. Both survive the builder round trip.
- **`fold=always`** renders the control as one chip — the segment on show (the active one, else the first) as
  icon · label · chevron — that opens a menu of the segments as rows with the active row filled and checked. It is a
  `<details>`, so it opens and closes with no script (the constructive DOM renderer stays script-free; the host owns
  selection, as before: the same `role=radio` buttons with the same `data-id` and the block's `action=`). `fold=never`
  is the default and the 0.37 pill row. `fold=auto` is not in this release.
- **`::filter-bar` is a chip row.** No box, no inner padding; each field is one pill (label · value · chevron) with
  the native `<select>` laid over it, so the picker opens as before and a host's change listener is unchanged.
  `size=compact` is the 28 px row for a navigator head; `inline=true` drops the block margins so the bar sits beside
  a preceding inline control. Both ride as data attributes, absent by default.
- The constructive DOM renderer (`dom` feature) mirrors the segmented control's new markup byte for byte.
- `spec/blocks.toml`: the two blocks' attribute lists and purposes.
- Tests: `render_html` units for the icon, the tint, the dropped hex, the folded markup and the unchanged unfolded
  markup; the builder round trip with the brace group; the filter bar's attributes; the DOM identity fixture
  `tests/fixtures/dom/navigator-controls.surf`.

## 0.38.0 — 2026-10-06 (the workbook learns pipe tables and the DOM twin: `type: spreadsheet` on the web, TASK-1314; and blocks as named fields: `fields`, `surf-fields`, field rows in the registry, TASK-1446)

### The workbook (TASK-1314)

A `type: spreadsheet` document opened as a workbook in the Mac app and as prose on app.cloudsurf.com: the web's
player draws every doc through the constructive DOM renderer, which had no workbook, and the HTML workbook counted
top-level `::data` blocks only — a register written as twelve markdown pipe tables under headings had zero sheets.

- **One rule for what a sheet is** (`workbook::plan_workbook`, D-WS-1): every top-level `::data` block AND every GFM
  pipe table inside a top-level markdown block is a sheet, in source order. The label is `name=` → `caption=` → the
  nearest preceding markdown heading → `Sheet<n>`, never twice the same ("Status", "Status (2)" — the kit's own
  form). Everything else — the summary, the callouts, the headings that became labels — is the About aside, after
  the sheets, never interleaved.
- **The markup, in both renderers.** `section.surfdoc-workbook[data-sheets]` › `nav.surfdoc-sheet-strip` of
  `a[href=#surfdoc-sheet-n][data-sheet-index][title=label]` (or `span.surfdoc-sheet-empty` "No sheets") ›
  `section.surfdoc-sheet[id][data-sheet][data-rows][data-cols][data-source]` each holding ONE table with EVERY
  inline row (the 20-row preview cap is the prose contract; a sheet has no `surfdoc-table-more` line) ›
  `aside.surfdoc-workbook-about`. `render_dom::render_doc_dom` builds the same bytes for a spreadsheet doc
  (`render_doc_string` / `to_html_workbook_fragment` are the identity pair) and `check_coverage` dry-runs that
  branch, so a wasm host's `render_doc` can take the workbook over.
- **The stylesheet**: a strip tab is at most 180px wide with a tail ellipsis, the full name in its title
  (D-WS-9); the strip scrolls in one row; the server page shows one sheet at a time by `:target` with no script;
  the About aside's rules.
- Native schema unchanged (v15): `NativeBlock::DataTable` does not carry `name=` yet — the uniffi bindings would
  have to be regenerated for Swift and Kotlin; that is the bindings lane's, and the kit's adapter keeps caption →
  heading → `Sheet<n>` until then.
- Tests: `src/workbook.rs` (the plan over the invented register fixture `tests/fixtures/workbook-register.surf`,
  the label rule, the splitter), `tests/render_dom_identity.rs` (the workbook byte-identity pair on the register,
  a `::data` workbook and an empty one), `tests/data_preview_contract.rs` (the markup pins).

### Blocks as named fields (TASK-1446)

A model never writes block source: `surf_parse::fields` reads any admitted block as named fields and lists and
writes them back, and the `surf-fields` binary (`--features cli`) speaks that over JSON on stdin and stdout —
`kinds`, `stamp`, `pages`, `read`, `apply` (every op or none) and `admit`.

- **One registry.** Each block's field rows (`fields`, `lists`, `fields_raw`, `fields_admitted`) sit beside its
  `attributes` in `spec/blocks.toml`; the module reads its schema from `spec_registry::BLOCKS_TOML`. Seventeen kinds
  carry rows: hero, cta, product-card, form, callout, stats, features, data, testimonial, pricing-table, faq, steps,
  metric, gallery, quote, infocard, comparison. Loose Markdown reads as the pseudo-kind `text`.
- **One write path.** A write mutates the typed block's JSON, builds the block back, serializes that one block at
  its authored fence (`builder::serialize_block_at`), carries over every authored opener token the serializer did
  not write (`id=` first), splices, and refuses itself when the result does not parse back to the intended block.
- **Admission by round trip.** `surf-fields admit <file.surf>...` reports, per kind, instances · no-change round
  trips · same-HTML renders · set-then-read-back per field, and the admitted list.
- **product-card serializer:** a card with a body and no subtitle now writes its title on the opener; the `## title`
  form made the parser read the first body line as the subtitle, so such a card never round-tripped.
- **form field rows:** `name` follows the label (derived by the parser) and is not a field; `type` and
  `placeholder` failed set-then-read-back on choice fields in the corpus and are not exposed.
- **Two shapes the write path refuses rather than corrupts** (found by `admit` over `tests/corpus`): a hero
  without a `#` headline cannot take a `subtitle` (the parser reads the first body line as prose), and a form range
  field with constraints loses them when its label is rewritten (the serializer does not write `constraints`).
  Both are refused by name at apply time; the fixture `tests/fixtures/fields/all-kinds.surf` carries one canonical
  instance of every admitted kind so `admit` over `tests/fixtures/fields/` is the crate's own gate.

## 0.37.2 — 2026-10-02 (the allow-list passes the host's own pages: the scheme grammar, and a scoped door only a host can open; TASK-1300)

0.37.1's allow-list refused three things a host application hands the renderer for its own pages. 0.37.2 passes
them and keeps every refusal a document met.

- **A scheme is a scheme.** Text before a colon is read as a scheme only when it is `ALPHA *( ALPHA / DIGIT / "+" /
  "-" / "." )` — what a browser requires. A template's `«IMG: description»` slot marker in a `src` (and `1:2`) is a
  relative reference again and is written untouched, so the slot can still be filled. `javascript:` and every other
  real scheme are read exactly as before.
- **`install_host_urls(HostUrls) -> HostUrlScope`** — a thread-local RAII guard, the shape of `install_media_resolver`.
  A host that renders blocks IT built may name `same_origin_frame_prefixes` (a relative frame `src` under `/email/`
  — never a `..` segment, a backslash, an encoded dot, slash or backslash, never protocol-relative) and `link_schemes`
  (`cursor`, `vscode`; `javascript`, `vbscript`, `data`, `file` and `blob` are refused even when named). Nothing a
  document writes installs it: no front-matter key, no directive, no FFI or wasm export. Without the guard the rules
  are 0.37.1's — a relative frame from a document is still refused.
- Tests: `tests/host_urls.rs` (the three inputs and the negative of each) and the unit tests beside the policy.
  No snapshot changed.

## 0.37.1 — 2026-10-01 (security patch, the 0.32.1 allow-list carried onto 0.37.0: one URL allow-list at render time)

- **One URL allow-list** (`url_policy`, crate-private). Every URL a document supplies is checked when it is
  written, not only when it is parsed: `href`, `src`, form `action` (and `::action`'s `data-surf-action` twin) and
  CSS `url('…')` across `render_html` and its constructive twin `render_dom`. The scheme is read the way a browser
  reads it — ASCII whitespace and control characters ignored, case folded, a colon before any `/`, `?` or `#` is a
  scheme — and an allowed URL is written unchanged.
  - link: `http`, `https`, `mailto`, `tel`, protocol-relative, relative, fragment; refused → `#`.
  - image: `http`, `https`, `data:image/…`, protocol-relative, relative; refused → empty `src` / empty `url()`.
  - frame: `https` only (no `http`, no protocol-relative, no relative path: a document must not frame the app
    origin's own routes); refused → empty `src`.
  - form action: `https` and relative; refused → the `action` attribute is omitted.
  - CSS `url('…')` (hero cover, post-card image, tile image, drawer emblem): the image rule, then `'`, `(`, `)`, `\`
    and controls percent-encoded so the value cannot leave the string.
- CSS values a document supplies (product tile `color:` / `gradient:`, stat `color`, embed `width` / `height`, deck
  `accent` / `font`) are refused — the declaration is dropped, an embed width falls back to `100%` — when they carry a
  `url(` / `image(` / `image-set(` / `cross-fade(` / `element(` / `src(` / `expression(` function, a backslash escape,
  a comment opener, or `;` `{` `}`.
- `sanitize_href` (parse time) now applies the link rule, so `ftp:`, `file:`, `blob:` and other schemes outside the
  list become `#` there too, alongside `javascript:` / `data:` / `vbscript:`.
- Tests: the three tests that pinned the gap (`html_cta_escapes_xss`, `html_nav_escapes_xss`,
  `html_image_src_xss_escaped`) and `hostile_shell_javascript_and_data_urls` now assert the script address is
  absent; a hostile table test covers each block type and kind (mixed case, tab, newline, NUL, leading spaces,
  entity text). No snapshot changed.

## 0.37.0 — 2026-10-01 (video — a `::video` block, a hero background video, the video-file embed, the `media:` scheme and the host's resolver; native schema v15; TASK-1277)

surf-parse names a library file by id and never turns it into a URL: the host does, through the resolver seam.
This release stacks on 0.35.0 (the site nav standard) and 0.36.0 (the note doc type) and carries both. The grammar, the
exact HTML and the seam are in `docs/video-grammar-0.37.0.surf`.

- **`::video`** — `Block::Video { src, poster, autoplay, loops, muted, controls: Option<bool>, caption, alt, width,
  aspect, span }`. `src` is `media:<file-id>`, a site-relative path or an `https:` URL; `poster` defaults to
  `media:<id>/poster` on a `media:` source. Flags are bare (`autoplay`) or valued (`controls=false`) and stored as
  authored; `media::video_flags` is the ONE place the rules live — `autoplay` forces `muted` and `playsinline` with no
  override, `controls` is on by default and off with `autoplay` unless stated. Registry row `video` (131 blocks),
  builder `.video(src)` · `.video_with_caption(src, poster, caption, alt)` · `.video_loop(src, poster, alt)`, and the
  serializer round-trips on the first pass.
- **HTML** — `<figure class="surfdoc-video"><video class="surfdoc-video-player" … playsinline preload><source src
  type></video><figcaption class="surfdoc-video-cap">`. `preload="none"` with a poster and no autoplay, `metadata`
  otherwise; `<source type>` from the extension, `video/mp4` for `media:`; `aria-label` from `alt`; `width` and
  `aspect` reach a `style` only as checked CSS (`max-width:640px`, `aspect-ratio:16 / 9`). New styles in
  `assets/surfdoc.css` (`.surfdoc-video*`, `.surfdoc-hero-bg`, `.surfdoc-hero-video`).
- **Reduced motion, no script.** A `<video autoplay>` cannot be stopped by a style rule and a fragment carries no
  runtime, so an autoplaying video is drawn as TWO elements: `.surfdoc-video-motion` (autoplaying; its `<source>`
  carries `media="(prefers-reduced-motion: no-preference)"`, so under reduced motion it loads nothing) and
  `.surfdoc-video-still` (the same source with the poster and controls, no `autoplay`, `preload="none"`). The
  stylesheet hides the stand-in by default and, under `prefers-reduced-motion: reduce`, hides the playing element and
  shows the stand-in. A hero's background video is hidden the same way; its poster picture remains.
- **`::hero[video= poster=]`** — `Block::Hero` gains `video` and `poster` (both `Option<String>`, serde-skipped when
  absent). A playable video draws the hero as the cover layout with `<video class="surfdoc-hero-bg" autoplay muted
  loop playsinline …>` behind the headline; the poster is `poster=`, else `image=`, else the library poster. An
  unusable or unresolved `video=` leaves the hero byte-identical to the one without it. `::figure` is unchanged.
- **`::embed`** — an embed whose `src` is a video FILE (`media:<id>`, or a path / `https:` URL ending `.mp4 .webm
  .mov .m4v .ogv`, query string tolerated) parses as `type=video` and renders through the `::video` path with
  controls, its `title` as the caption. A provider's page keeps the link card. **The test
  `html_video_embed_stays_link_card` changes on purpose**: it now pins the card for a PROVIDER URL only, and the new
  `html_video_file_embed_is_a_player` pins the player for a direct file. Behaviour change: a generic embed of a
  `.mp4` with a `height` was an `<iframe>` and is now a `<video>`.
- **The `media:` scheme and the resolver seam** (new module `media`, thread-local with an RAII guard — the shape of
  `render_typst::install_image_context`): `install_media_resolver(impl Fn(&str, MediaUse) -> Option<String> + 'static)
  -> MediaScope`, `media_templates(video, video_loop, poster)`, `enum MediaUse { Video, VideoLoop, Poster }` — an
  autoplaying video and a hero background ask for `VideoLoop`, so a host can serve a smaller silent rendition. An
  UNRESOLVED id draws `<figure class="surfdoc-video surfdoc-video-unavailable">` with a placeholder box and the
  caption — never a `<video src="media:…">`. `media_refs(&SurfDoc) -> Vec<MediaRef { id, media_use }>` (and
  `media_refs_in(&[Block])`) lists every library file a document references, through every container, in
  first-appearance order — exactly what the renderers ask the resolver for. Also public in `media`:
  `classify_media_src` / `MediaSrc`, `video_flags` / `VideoFlags`, `video_mime`, `is_direct_video_src`,
  `embed_is_video_file`, `video_aspect`, `video_width`, `resolve_media`, `media_url`, `effective_poster`,
  `hero_poster`, `video_player` / `VideoPlayer`, `hero_video_player`, and the constants `MEDIA_SCHEME`,
  `MEDIA_POSTER_SUFFIX`, `MEDIA_ID_MAX_LEN`, `MOTION_OK_MEDIA_QUERY`. Re-exported at the crate root:
  `install_media_resolver`, `media_refs`, `media_refs_in`, `media_templates`, `video_flags`, `MediaRef`,
  `MediaScope`, `MediaUse`, `VideoFlags`. wasm: `set_media_templates(video, video_loop, poster)`.
- **Twins.** `render_dom` draws `::video`, the hero video and the video-file embed byte-identically (allowlist gains
  `poster controls autoplay muted loop playsinline preload media`; the browser sink also sets the `muted` PROPERTY,
  which a constructed `<video>` needs to autoplay — `web-sys` feature `HtmlMediaElement`). **Native schema 14 → 15**:
  `NativeBlock::Video { src, poster, autoplay, loops, muted, controls, playsinline, caption, alt, aspect }` (sources
  cross as authored for the client to resolve; flags are the effective ones), a video-file embed crosses as `Video`,
  and `NativeBlock::Hero` gains `video` and `poster`. `NativeBlock` is a 132-variant enum. Markdown, terminal, Typst
  / PDF (`collect_image_srcs` lists a video's poster) and LaTeX degrade to the poster picture, the caption and a link;
  the slides draw the HTML element.
- **Lint** — `L047` (error): `::video` with no `src`, or a scheme other than `media:` / `https:` / relative. `L048`
  (warning): `autoplay` with no poster on a path or URL, neither `alt` nor `caption`, a malformed `media:`
  reference, an unusable `poster`, a `::hero[video=]` that cannot play. Nothing else about video is an error. 24 rules.
- **Model field** — `ModelFieldType::Video` from `video` · `clip` · `movie` (model lines and `parse_schema_field_type`);
  a form shows it as a file field, as it does an image.
- Tests: `tests/video.rs` (the element end to end, the DOM twin and the native node under their features), the
  corpus fixture `tier10-video` with its HTML and native snapshots, `tests/fixtures/video.surf` in the DOM identity
  suite (unresolved and resolved), two lint fixtures, a `Block::Video` strategy in the property suite; the registry
  sweep covers `video` in the no-panic suite.
## 0.36.0 — 2026-10-01 (the note doc type — `type: note`; additive, no render or schema move)

- **`DocType::Note`** (`type: note` in front matter): quick words under a cursor, titled by their first line in the
  CloudSurf clients. It is an ordinary SurfDoc — it resolves to `RenderProfile::Document`, lints and builds under the
  word `note`, and changing `type:` promotes it to any other kind. `app` and `site` are NOT new types: `App`, `Website`
  and `Web` already exist.
- Additive only: no block, render path, native schema (stays 14), FFI or wasm surface moves. A consumer on an earlier
  tag reads `type: note` as an unknown type exactly as before.

## 0.35.0 — 2026-10-01 (the site nav standard — the Khoury's nav for every doc.surf site; web CSS only, no schema move)

- **`SITE_NAV_CSS` is rewritten as the doc.surf nav standard.** The bar keeps its markup (every needle surf's container
  reads — the logo anchor, the `.site-nav-links` close, the toggle id — is byte-identical) and changes only how it is
  drawn: a sticky glass bar (the blur on a `::before` pseudo-element, never on the bar — a backdrop-filter on the bar
  makes it the containing block for the fixed sheet, which then collapses into the bar; measured on the Khoury's site
  2026-09-19) with the brand on the left and the controls on the right. On a wide screen (`>= 901px`) a site with at
  most seven links (pages + the CTA) shows them INLINE on the right — plain words with an accent underline that grows
  on hover, the one CTA a pill, the theme toggle beside it, no menu button. Every other case (a small screen, or more
  than seven links) keeps the menu button, now on the RIGHT with no ring, which opens a FULL-SCREEN SHEET that settles
  in from the top (400 ms, opacity + a 2.5 % translate): large rows parted by hairlines, the CTA pinned to the bottom as
  a full-width pill, the bar's button — an X while open — the ONE close control. The drawer-era head, group label,
  link icons and scrim stay in the markup and are not drawn. The page behind the open sheet does not scroll
  (`html:has(.site-nav-toggle:checked)`). The seven-link rule rides `:has()`; a browser without it keeps the button +
  sheet at every width. `.site-nav-logo-img` (the container's logo swap) gains its own rule: a plain 32 px picture,
  no tile behind it.
- **Pinned by the a11y tests exactly as before:** the focusable checkbox, its focus ring on the menu button, the
  `:focus-visible` ring list, the skip link, the 38 px theme toggle at `order: 2`. The drawer-era
  `site_nav_css_theme_arms` test (a deeper panel shadow in the dark arms — a full-screen sheet has no panel) is
  replaced by `site_nav_css_is_the_full_screen_standard`, which pins the standard's seams.
- **`SITE_NAV_CSS` is `pub`** (beside `SURFDOC_CSS`), so a consumer can pin the standard it links.
- `tests/integration.rs::render_editor_surf_to_html` SKIPS (says so, passes) on a machine with no surf checkout — the
  build pool's cargo device — instead of failing the suite on a laptop path.
- No parser, schema (native stays 14), FFI or wasm surface moves; the Mac and iOS kits are untouched. The HTML of a
  site page changes only inside the inlined `<style>`.

## 0.34.0 — 2026-09-30 (the page furniture is the renderer's — the brand line, the "SurfDoc" running head retired, the callout card without its stroke; TASK-1167 under TASK-1045)

- **`PdfConfig.brand: Option<String>`** (default `None`; `Debug` prints it; `for_doc` passes it through — no front-matter
  key reads it, a document cannot opt out). With `Some(words)`, EVERY page's footer — the first included — carries the
  words bottom-right, 8 pt in the page number's grey (`luma(150)`), as a `#link("https://cloudsurf.com")`; the page
  counter stays centred between two equal `1fr` grid columns. The words are the caller's: the engine never hears the
  word "plan". Rust-only — `PdfConfig` crosses no FFI or wasm boundary; the native schema stays 14.
- **The running footer is emitted by the renderer** (`build_page_furniture`, with the page geometry, BEFORE the profile
  template + the markup — a `#set page` rule after content starts a new page in Typst, so a footer cannot follow the
  markup). `assets/surfdoc.typ` sets `margin:` only: the **"SurfDoc" running head** that sat top-right of every page
  after the first is gone from every profile, and nothing in the engine names itself on a page.
- **`surfdoc-callout` draws a tinted block with 6 pt corners and NO stroke on any side** (the 3 pt left accent bar is
  gone); the six kinds keep their tints and title colours. The resume's head and section rules are untouched.
- **`typst_source(doc, config)`** (new, public): the exact Typst source the compile sees (geometry · furniture ·
  template + markup) — the seam the profile tests read the footer through, since the page SVGs draw glyphs as paths.
- Tests (+5 in `tests/pdf_profiles.rs`): no running head on a three-page report; the brand in every page's footer
  (the markup, the PDF's `/URI` per page, the page count unchanged); no words without a brand (blank words are none);
  the callout card without a stroke; the V7 resume still one page with a brand. `examples/render_pdf.rs` takes
  `--brand "…"` for the eyes-on loop.

## 0.33.0 — 2026-09-29 (the backends grammar — the stateful blocks a phone app needs; native schema v14; the FFI fails open; TASK-1176 under TASK-1174)

surf-parse describes these blocks and never runs them: no expression is evaluated, no query run, nothing
scheduled, no script emitted. The platform runtime reads the parsed blocks (or the `data-*` attributes the HTML
carries) and does the work. The grammar and the exact HTML data contract are in `docs/backends-grammar-0.33.0.surf`.

- **`::model`.** `owner=` (`viewer` default · `workspace` · `public`; `Block::Model` gains `owner: String`, serde
  default `viewer`). New types `date` (`ModelFieldType::Date`), `range` (`Range`) and `list(Model)` (`List`);
  `number` is `float`, `textarea` is `text`. New constraints `sentences=1..4` (`Sentences(min, max)`),
  `prompt="…"` (`Prompt`), `labels="a|b"` (`Labels`), `levels="2|4|7|9"` (`Levels`), `pattern="…"` (`Pattern`,
  verbatim). `pk` is `primary`, and bare constraint words may follow the type (`- id: uuid pk` — before 0.33.0 that
  line read as a `string` with no constraints). A COMPUTED field: `- score: number = <expr>` — `ModelField` gains
  `computed: Option<String>`, the expression text verbatim. The bracket list now splits on commas OUTSIDE double
  quotes and a quoted value is verbatim (case kept, `\"` a quote); an unquoted `default=` is still read
  lower-cased. The model table escapes its constraint cell (a prompt or pattern carries author text) and shows a
  computed field's expression after its type.
- **`::route`** keeps `filter=` and `sort=` (`Block::Route` gains both, `Option<String>`), shown as detail rows.
- **`::picker[bind= tiers= layout= emoji=]`** — `- Core: a · b · c | info="…"` rows (`Block::Picker`,
  `PickerRow { core, choices, info }`): a `fieldset.surfdoc-picker` of `div.surfdoc-picker-row[data-core]` rows,
  one radio per graded choice named by `bind=`, `data-tier` 1..tiers.
- **`::when[bind= op= value=]`** or **`::when[expr=]`** — a `section.surfdoc-when[hidden]` of ordinary blocks with
  `data-when-*` attributes (`Block::When`).
- **`::compute[name= expr= source=]`** — an empty `output.surfdoc-compute` (`Block::Compute`).
- **`::flow[model=]`** — `:::step[title= repeat=true]` children holding blocks (a nested `::::picker`, prose) and
  field lines (`Block::Flow`, `FlowStep`): the 0.32.0 stepped-form shell posting to `/_api/{model}`, radio prefix
  `id=` else `flow`, a repeatable step marked `data-repeat="true"`, submit `Save` unless `submit=`.
- **`::form[model=]`** — `Block::Form` gains `model: Option<String>`: the form posts one row to `/_api/{model}`
  (an authored `action=` still wins) with `data-model`, and reads the model's own field lines
  (`- intensity: range [min=1, max=10]`) beside the form grammar. `FormField` gains `constraints` and
  `FormFieldType` gains `Range`: a range renders `<input type="range" min max step="1" data-labels data-levels>`
  plus an empty `<output>`, a textarea gains `maxlength` / `data-sentences`, a number `min` / `max`, a text input
  `maxlength` / `pattern`. A form without `model=` renders byte-for-byte as before.
- **`::schedule[bind= tz= title= body= link=]`** — a hidden `span.surfdoc-schedule` with `data-schedule-*`
  attributes (`Block::Schedule`, `tz` default `viewer`).
- Every URL-ish value (`link=`, `source=`) with a script scheme (`javascript:` · `data:` · `vbscript:`, ignoring
  embedded whitespace) becomes `#`; a model name reaches `/_api/` with name characters only; every attribute is
  escaped.
- **Native schema v14.** `NativeBlock::Picker` · `When` · `Compute` · `Flow` · `Schedule` (app-chrome tier) with the
  records `NativePickerRow` and `NativeFlowStep`; `Model.owner`, `NativeModelField.computed`, `Route.filter` /
  `sort`, `Form.model`, `NativeFormField.constraints`. `NativeBlock` is a 131-variant enum.
- **The FFI fails open.** `parse_surfdoc`, `parse_to_native` and `parse_to_native_styled` return the document's
  blocks whatever diagnostics it earns — before 0.33.0 any error-severity diagnostic (an unquoted colon in a
  front-matter value, P002) refused the whole document on native while the web rendered it. New export
  `surfdoc_diagnostics(source) -> String`: the parse, schema and lint diagnostics as JSON
  `[{severity, code, message, line, column}]`, for a "rendered with N problems" banner. `SurfDocError` keeps its
  variants; nothing was removed or renamed. The kit's view model and the bindings regen belong to the consuming
  repo's repin.
- Constructive DOM twins for all five blocks, the model table (so a backends document renders constructively
  whole) and the route's new rows, byte-identical to `render_html`; the allowlist gains `step`, `pattern` and
  `maxlength` (the pin that listed `step` / `pattern` as never-widened now proves an arm emits them). Markdown and
  terminal degradations; the serializer writes every new block back to a fixed point (a model-bound form or flow
  field in the model grammar when it reads back exactly). Registry 125 → 130 (`form` declares `model`, `model`
  `owner`, `route` `filter` / `sort`); lint `L046` (an unknown owner, a `::when` without a predicate or with an
  unknown `op=`, a `::flow` without `model=`, an unknown picker layout, a row wider than `tiers=`); validate
  `V350`–`V355`; fixture `tests/fixtures/backends/moodmap-app.surf`, corpus tier `tier9-backends` (both
  snapshots; `tier4-manifest` re-pinned for `- id: uuid pk`), `tests/backends_grammar.rs`.
- Breaking for struct-literal construction of `Block::Model` / `Route` / `Form`, `FormField`, `ModelField`,
  `NativeBlock::Model` / `Route` / `Form`, `NativeModelField`, `NativeFormField`, and for exhaustive matches on
  `Block`, `NativeBlock`, `ModelFieldType`, `FieldConstraint`, `FormFieldType`.

## 0.32.0 — 2026-09-29 (::carousel and ::form steps=true — the Elevate lanes C and Q; TASK-1115 · TASK-1112 under TASK-1110)

- **`::carousel`** (lane C). `Block::Carousel { slides, id, aspect, span }` with `CarouselSlide { title, body, image,
  alt }`. Two grammars: `:::slide[image="…" alt="…"]` children closed by `:::` (the first `### ` / `## ` line in a
  slide is its title, the rest its markdown body), or no `:::slide` child at all — every `### ` heading starts a
  slide, the way `::features` cards are written. In both, a bare `![alt](src)` body line is the slide's image when
  `image=` named none; a script scheme (`javascript:` · `data:` · `vbscript:`) in either becomes `#`. `aspect=`
  (`square` · `wide` · `tall`) rides as `data-aspect`. HTML: `section.surfdoc-carousel#{id} > div.surfdoc-carousel-track`
  of `article.surfdoc-carousel-slide#{id}-slide-{n}` (figure + img when there is an image, h3 title, body), then
  `nav.surfdoc-carousel-dots[aria-label=Slides]` with one `<a href="#{id}-slide-{n}" aria-label="Slide n">` per
  slide. NO script: the base sheet makes the track a scroll-snap row (`scroll-snap-type: x mandatory`, smooth,
  touch momentum, no scrollbar, each slide `flex: 0 0 min(100%, 320px)`), the dots are accent-coloured anchors, and
  `prefers-reduced-motion` turns the smooth scroll off. The id defaults to `carousel` — a pure renderer keeps no
  per-page counter, so a page with two carousels gives each an `id=`. Markdown: the slides in order as heading,
  image and body; terminal: the markdown degradation; the serializer writes the explicit `:::slide` form (a
  heading-only carousel lands on it at the first pass).
- **`::form[steps=true]`** (lane Q). `:::step[title="…"]` children are form groups (the `group:` fieldset
  mechanism — `group:` lines keep working, and under `steps=true` a `group:` line is a step too; an untitled step
  is named `Step n`). `Block::Form` gains `steps: bool` and `id: Option<String>` (serde-defaulted). With
  `steps=true` the form is a CSS-only stepped form: the tag keeps `class="surfdoc-form"` verbatim with
  `data-steps="true"` (and `id=` when authored) AFTER it; right after the opening tag one `_step` radio per step
  (`id="{prefix}-step-{n}"`, prefix = the form's `id=` else `form`, the first `checked`), a progress line
  (`--surfdoc-steps` inline), then each step as `fieldset.surfdoc-form-step[data-step=n]` with the legend
  `Step n of N · {title}` and a footer of `<label for>` Previous (not on the first) / Next (not on the last)
  buttons; the submit button only in the last step's footer. The base sheet shows the fieldset whose `data-step`
  matches the checked radio (`:nth-of-type(k):checked ~ …` written out for twelve steps, no `:has()`), keeps the
  radios visually hidden but focusable (the arrow keys page the steps), and styles the labels as the submit
  button's twins. A form without `steps=true` renders byte-for-byte as before, `:::step` children included (plain
  fieldsets). `_step` is a new control field a host's form ingest should drop, like `_honey`. A required field on
  a hidden step still blocks the final submit — without script the browser cannot reveal it.
- **Native schema v13.** `NativeBlock::Carousel { slides: Vec<NativeCarouselSlide> }` (Site tier; a client with no
  scroll-snap renders the slides as stacked cards) and `NativeBlock::Form` gains `steps`. `NativeBlock` is a
  126-variant enum.
- Both constructive DOM twins are byte-identical to `render_html` (the allowlist gains `checked` and `for`); the
  registry has 125 blocks (`carousel`; `form` declares `steps` and `id`); `step` joins the lint's sub-directive
  names; a fixture (`tests/fixtures/carousel-steps.surf`) and a corpus tier (`tier8-carousel-steps`, both
  snapshots). Breaking for struct-literal `Block::Form { … }` and `NativeBlock::Form { … }` construction (add
  `steps: false, id: None` / `steps: false`) and for exhaustive matches on `Block` / `NativeBlock`.

## 0.31.1 — 2026-09-28 (the resume's spacing at the V8 numbers; the paper the config asks for)

- **The resume profile's spacing.** Brady's side-by-side (05:06): the engine's page was tighter than the V8
  reference at every seam — the line pitch, the gap before a section title, between entries, in the head. Two
  causes: `leading: 0.5em` (a 1.23 pitch against the V8's 1.3) and a block's weak `above`/`below` spacing, which
  collapsed against its neighbour so sections butted and entries touched. Now every gap is a `gap(h)` SPACER
  BLOCK the template or the generator writes (no weak spacing, no `v()`), sized as the V8 gap plus 0.287 em of the
  line above and of the line below (Typst measures a line to its baseline, Chrome to the bottom of its line box):
  13.2 pt between sections, 12.2 between entries, 9.5 between credentials, 10.6 between skill groups, 10.4 after
  the head rule, 8.8 after a section rule; leading 0.575 em = the 1.3 pitch over Inter's cap height; paragraph
  spacing 10.5 pt, list items 6.6 pt. Ashley's V7 fixture still lays out on one page.
- **The paper the config asks for.** `assets/surfdoc.typ` no longer pins `paper: "a4"`: the PDF config's
  `#set page(paper: …)` override precedes the template and now wins, so a generic doc lays out on the route's
  Letter or its own `paper:` (every generic doc came out A4 on the web while the pages JSON said Letter).
  `assets/resume.typ` states margins only, for the same reason.
- `examples/render_pdf.rs` (`--features pdf`): one `.surf` → a PDF + its Typst source, the eyes-on loop for the
  profiles. Tests: the generic paper (the route's Letter · the doc's A4), the spacing pin.

## 0.31.0 — 2026-09-28 (page profiles — the resume, the doc's own paper, one compile for the PDF and its pages; TASK-1074 lane R)

- **The RESUME page profile.** `profile: resume` in front matter, or a `template: resume/…` / `cv/…` (the resume doc
  template stamps `template: resume/v1-classic`), selects `assets/resume.typ` instead of the generic layout:
  US Letter, margins 0.42 / 0.65 / 0.3 / 0.65 in, Inter 9.75 pt / 1.3, no running head or page counter; the head
  (the first `#` = the name, the next two paragraphs the headline and the contact line joined by grey `·`) closed by
  a 2 pt rule; `##` sections as 8.5 pt uppercase spaced titles over a 1 pt rule; `::steps` steps as ENTRIES (role
  bold, ` — org` medium, the `time=` right-aligned in lining figures, the first non-bullet line the muted where-line,
  the `- ` lines the bullets) that never split across pages; `::data` rows as credentials (the first cell bold, the
  rest a muted sub-line); `::features` cards as skill groups ("Group: a · b · c"); `::stats` dropped in print; a
  quote or callout as a paragraph; any other block through the generic mapping; the LAST TWO sections side by side
  when both are short. `render_typst::is_resume(&doc)` says which path a doc takes. The numbers are the measured V7
  resume build (headless Chrome) this profile reproduces; the fixture `tests/fixtures/resume-ashley-yeghiayan-v7.surf`
  lays out on ONE page (pinned).
- **Inter bundled** (SIL OFL 1.1, `assets/fonts/inter/`): Regular · Medium · SemiBold · Bold static faces registered
  beside Liberation Sans for the `pdf` feature (+2.5 MB in the native crate; the wasm build is untouched). The
  generic layout keeps Liberation Sans.
- **The document decides its paper.** Front matter `paper: letter | a4 | legal` (`FrontMatter::paper`,
  `PaperSize::parse`) and `margins: "<t> <r> <b> <l>"` in CSS 1/2/4-value order with `in` · `cm` · `mm` · `pt`
  (`FrontMatter::margins`, `Margins::parse` — a bad value is ignored, never an error). `PdfConfig::for_doc(&doc, base)`
  / `SurfDoc::pdf_config(base)`: the caller's defaults, then the profile's paper and margins (`Margins::RESUME`), then
  the doc's own. Both keys ride the open `extra` map — the exported front-matter shape (uniffi · wasm) is unchanged.
- **One compile, two outputs.** `to_pdf_and_pages(&doc, &config) -> (Vec<u8>, Vec<String>)` (what a server caches), `to_pages(&doc, &config) -> Vec<String>` (one `<svg>` per page, the page's `viewBox`
  in points — Letter = `0 0 612 792`) and `page_count` share the compile `to_pdf` runs (`typst-svg 0.14` joins the
  `pdf` feature), so a preview built from the pages is byte-for-byte the document the PDF carries.
- Unchanged: the generic and academic layouts, `PdfConfig`'s fields and defaults (A4, 1 in), the image degrade-don't-die
  retry, every other feature.

## 0.30.0 — 2026-09-26 (text-anchored edits — the block is found before the model runs, TASK-1040 lane P)

- `surf_parse::edit::find_text(source, query, route?)` — every place a quoted phrase occurs, in document order, as a
  `TextHit { id, kind, route, slot, line, col, len, start_offset, end_offset, exact, snippet }`: the block it sits in
  (`id: None`, kind `markdown` for loose text under a page), the SLOT inside the block (`headline` · `subtitle` ·
  `body(n)` · `item(n)` · `attr(key)` · `cell(r,c)`), and whether the bytes there are the phrase verbatim. The
  normaliser is a table of seven rules, one test each: case · whitespace runs · curly quotes · `*` and `` ` ``
  transparent (a span that would cut an emphasis run is widened over it) · the query's trailing punctuation · an
  ellipsis in the middle · a link's `(href)` and a trailing `{…}` invisible. Ids, routes, hrefs, colours and layout
  tokens are never text. With a `route`, that page's blocks plus the site-wide ones are searched.
- `resolve(hits, current_route) -> Resolved` — the ONE ambiguity policy every caller runs: exactly one verbatim hit →
  `Exact`; none verbatim and exactly one normalised → `Normalized`; several and exactly one on the current route →
  `CurrentRoute`; still several → `Ambiguous(candidates)`, never a guess; none → `None`. `find_text_json` carries
  `{hits, policy, pick}` for the FFI (`surfdoc_find_text`), the wasm module (`find_text`) and a server tool.
- `replace_text(source, route?, id?, find, replace) -> TextReplaced { source, hit, before, after }` and the
  `EditOp` `replace_text` (`find` · `replace`) — the phrase is replaced INSIDE its line so a bullet, `[label](href)`,
  a heading prefix, `{#anchor}`, the `[attrs]` line and the value side of `key: value` survive; the scope is the block
  `id`, else the page `route` (loose Markdown reachable), else the document; within it the phrase must occur once
  (verbatim first, then normalised) or the edit is refused with `AmbiguousText { candidates }` / `TextNotFound`.
  `set_text` stays as sugar for a block's first line; its doc steers a phrase at `replace_text`.
- `list_blocks` — every `BlockRef` gains `text` (the block's first visible text, markup stripped, ≤ 120 chars; a
  `::site`'s name, a `::page`'s title, a `::cta`'s label) and `count` (list items · heading-started cards, questions,
  steps · table rows · child blocks). JSON grows only; a 0.28.0 listing still decodes. `visible_text(line)` is public.
- `find` (every id verb): a site-wide block (`nav`, `footer`) is found whatever `route` is passed; an id that appears
  twice on ONE page is `DuplicateId`, never silently the first.
- Corpus: `tests/fixtures/edit/text.surf` + eight pinned `replace_text` cases (a nav item · a second card's title · a
  FAQ answer · a button label · a headline with an anchor · a loose paragraph · an attribute value · a table cell),
  the listing `text.blocks.json`, and `replace_text` in the per-verb loop over `site.surf`. Native schema unchanged (v12).

## 0.29.0 — the DOM twins the strategy docs need (TASK-1039 lane R)

The constructive DOM renderer (`render_dom`, feature `dom` — the web player's zero-sink path) learns the blocks and markdown constructs the strategy corpus declines on. Measured before this train (mini-m4-pro, 2026-09-25): 48/124 registry kinds, 2,208/5,140 docs rendered; the declines were `markdown:rule` (1,400 docs), `::tasks` (594), `markdown:blockquote` (477), `markdown:tasklist` (143), `markdown:strikethrough` (72), `::decision` (30), unknown blocks (19), `::steps` (14).

- Markdown: `> blockquote`, `---` rules, `~~strikethrough~~`, `- [ ]` task-list markers (the sanitized HTML path drops the `<input>` and keeps the newline; the twin does the same).
- Blocks: `::tasks` / `::action-items`, `::decision`, `::steps`, an unknown directive (`surfdoc-unknown`), `::quote`, `::cta` (with `to_html_fragment`'s consecutive-CTA grouping mirrored in `render_blocks_dom`), `::columns`, `::stats`, `::testimonial`, `::faq`, `::details`, `::comparison`, `::route`, `::slide` (inline), `::deck` (renders nothing inline), `::logo`.
- `examples/dom_coverage_sweep.rs` (feature `dom`): the registry and a directory of `.surf` files through the coverage gate, with the decline histogram — the measurement this train answers.
- `tests/render_dom_identity.rs`: every newly covered kind's snippet and corpus examples render byte-identical to `to_html_fragment`; a markdown-constructs fixture pins the four markdown twins.

Still declined by design: raw HTML in markdown (a sink), `::css`, script-emitting blocks (store · booking · gallery · tab-bar), charts and diagrams (pre-serialized SVG), and the remaining registry kinds (`::nav`, `::footer`, `::toc`, the infra cards …) — the next train.

## 0.28.0 — 2026-09-24 (block edits by id — the site is a versioned SurfDoc, TASK-1015 lane P)

- `surf_parse::edit` — the editing API the 0.18.1 block addressing was built for: `replace_block` · `insert_after` ·
  `remove_block` · `move_block` · `set_attr` · `set_site_key` (the `::site` body's `accent:` / `name:` …) · `set_text`
  (a block's first text line, heading prefix kept) · `stamp_ids` (every unlabelled directive gets a stable
  `b-<kind>-<n>` once; `::site` and `::page` never) · `list_blocks` (every block with its id, route, depth and span).
  Pure functions over the source through `block_meta`'s spans: one block moves, everything else stays byte-identical,
  an unknown id / route / op is an `EditError`, never a silent no-op. Ids resolve WITHIN a page (L043's law): every
  verb takes an optional `route`; an id on two pages without one is `AmbiguousId`.
- `EditOp` / `apply` / `apply_json` — one edit as data, the shape the FFI, the wasm module and a JSON tool carry.
- FFI: `surfdoc_list_blocks(source)` and `surfdoc_apply_edit(source, op_json)`; wasm: `list_blocks` and `apply_edit`
  (`{"ok":true,"source"}` / `{"ok":false,"error"}`).
- Corpus: `tests/edit_fixtures.rs` — one `(before, op, after)` triple per verb under `tests/fixtures/edit/`, pinned
  byte-for-byte. Native schema unchanged (v12).

## 0.27.0 — 2026-09-24 (native schema v12: the `panels` layout — CloudSurf on the web, lane G)

- `::app-shell[layout=panels]` — the CloudSurf app's arrangement enters the grammar (`AppShellLayout::Panels`), and
  `adaptive` accepts `panels` for any class (`AdaptiveMode::Panels`), so the canonical shell is
  `::app-shell[layout=adaptive desktop=panels tablet=rail mobile=tabs]`. L041's vocabularies gain the token.
- Two new blocks, registered first in `spec/blocks.toml` (122 → 124): `::panel-slot[role=navigator|work kind= pinned
  parks classes= min-class=]` (a navigator seat or a work slot — a Desk whose `tab-bar` / `tab-content` strip is
  authored empty and user-populated at runtime) and the leaf `::preset[name= title= icon= columns= rows= spans= slots=
  default]` (a named grid shape; the ten Mac presets are ten lines). Everything else the layout needs is existing
  vocabulary: `sidebar` (the Navbar rail), `toolbar` (the top bar), `tab-bar` / `tab-content`, `nav-tree`, `row`.
- Per-size-class resolution (`resolve.rs`): a panels shell keeps every slot on desktop, the first unpinned navigator
  seat plus ONE work slot on tablet, one work slot on mobile (the navigators become the generated tab bar's targets).
- HTML + DOM twin: the work slots render inside ONE `surfdoc-panels-work` grid carrying the default preset's tracks as
  `--panels-columns` / `--panels-rows` / `--panels-preset`; each seated slot carries `grid-column` / `grid-row` inline,
  an unseated slot is `hidden`; slots carry `data-slot` / `data-role` / `data-kind` / `data-pinned` / `data-parks`;
  presets render as inert `<template class="surfdoc-preset" data-…>` records; an `adaptive` shell that names
  `panels` in any class also wears `surfdoc-layout-panels`, so the stylesheet keys on one class. The player reads
  these, never re-derives them. Stylesheet §56a + §81.
- Native: `NativeBlock::PanelSlot` (recursive) and `NativeBlock::Preset` (leaf, `is_default`), record
  `NativePresetSpan`; `NATIVE_DOC_SCHEMA_VERSION` 11 → 12.
- Lint L045 (`spec/rules.toml` 20 → 21): a work slot without a `::tab-bar`, a seat without `kind=`, a preset seating
  an undeclared slot.
- Corpus: `tests/corpus/tier7-panels.surf`, pinned at 390 / 834 / 1280 like `tier5-size-class` (F-6).

## 0.26.0 — 2026-09-23 (native schema v11: headline anchors — Khoury's on macOS, L0)

- `NativeBlock::Hero` and `NativeBlock::SectionContainer` gain `anchor: Option<String>`: a headline's trailing
  explicit anchor (`## Title {#slug}`) is split off the text the way the web's `split_explicit_anchor` does, so a
  native renderer never draws `{#slug}`. The AST keeps the author's line (the fixed-point round trip is unchanged).
- Markdown bodies (`NativeBlock::Markdown`, `NativeColumnContent`) cross with `{#slug}` removed from their ATX
  headings, outside fenced code; a body with no such heading crosses byte-identical.
- `NATIVE_DOC_SCHEMA_VERSION` 10 → 11.

## 0.25.0 — 2026-09-22 (native schema v10: the last twenty — every registered block is implemented)

### Added

- **The fourteen planned blocks are implemented** (sessions 11 + 12 of the
  blocks program, one session; the spec batch first): every
  `status = "planned"` row of `spec/blocks.toml` is `implemented` now —
  `::related`, `::turn`, `::timeline`, `::output`, `::ai-generated`,
  `::alternatives`, `::ai-context`, `::countdown`, `::css`, `::footnote`,
  `::kernel`, `::logo-cloud`, `::subscribe`, `::notes`. Each has a `Block`
  variant, a parser whose grammar is the corpus's authored one (the format
  specification's examples and the plans that use the block), an HTML
  render with a byte-identical `render_dom` twin, the markdown degradation
  the spec row names, a serializer arm that reaches a fixed point on the
  first pass, a stylesheet rule for every class, and the L020 vocabulary.
  - `Related { items }` over the new `RelatedItem { title, href, relation }`
    — `- [Title](href) — relation`, `- relation: href` or `- href — note`;
    a `javascript:` / `data:` / `vbscript:` target is replaced by `#`.
  - `Turn { participant, time, role, model, content }` — `timestamp=` is
    read as `time=`; `role` is the authored value, and `types::turn_role`
    resolves `human` / `ai` / `system` (inferring from the participant).
  - `Timeline { title, entries }` over `TimelineEntry { when, label, group }`
    — `- when — label` / `- when: label` (bold markers stripped), `## heading`
    lines grouping; `title=` joins the spec row (the corpus authors it).
  - `Output { for_id, timestamp, exit, format, content }` — `for=` crosses
    as `for_id`.
  - `AiGenerated { model, date, reviewed, content }`,
    `AiContext { model, tokens, loaded, content }`.
  - `Alternatives { headers, rows }` — a pipe table; the renderer colour-codes
    the `Verdict` column (else the last) by its words.
  - `Countdown { date, label }` — no clock in the crate; the web render
    stamps `data-date` for the host.
  - `Css { content }` — a `<style class="surfdoc-css">` with `</` written as
    `<\/`; the constructive DOM declines the document as
    `script-emitting:css` (a rawtext body) while staying byte-identical.
  - `Footnote { id, content }`, `Notes { content }` (standalone; inside a
    `::slide` the parser still folds `::notes` into the slide's `notes`).
  - `Kernel { lang, env, runtime, packages, sandbox, properties }` — body
    lines win over attributes; `packages: [a, b]`.
  - `LogoCloud { title, items }` over `LogoItem { src, name }`;
    `Subscribe { action, placeholder, content }`.
- **Twenty new `NativeBlock` variants — schema v10.** The six web-only blocks
  that still degraded (`Health`, `Hours`, `Marquee`, `Smoke`, `Use`,
  `Volumes`) and the fourteen above cross structurally; eight new records
  (`NativeHoursRow`, `NativeSmokeCheck`, `NativeCrateDep`,
  `NativeVolumeEntry`, `NativeRelatedItem`, `NativeTimelineEntry`,
  `NativeAlternativeRow`, `NativeLogoItem`); `NativeStyleProperty` reused
  for the kernel's ledger. `Turn.role` crosses resolved. `block_tier`:
  `Health` / `Smoke` / `Use` / `Volumes` / `Css` / `Kernel` → Chrome,
  `Hours` / `Marquee` / `Countdown` / `LogoCloud` / `Subscribe` → Site, the
  other nine → Content; only `Unknown` and `Deck` still degrade.
  `NATIVE_DOC_SCHEMA_VERSION = 10`; `NativeBlock` is a 123-variant enum
  (the enum keeps one `///` line per variant; the ledger is in the module
  docs).
- A corpus fixture for the twenty (`tests/corpus/tier6-the-last-twenty.surf`)
  with HTML + native snapshot pins; identity fixtures
  `tests/fixtures/planned-blocks.surf` and `planned-css.surf`.

### Changed

- `spec/blocks.toml`: fourteen rows `planned` → `implemented`; `[blocks.timeline]`
  gains `title`. 122 registered, 122 implemented, 0 planned — the registry
  is complete.

## 0.24.0 — 2026-09-22 (native schema v9: the ten infra blocks of the app format)

### Added

- **Ten new `NativeBlock` variants — schema v9** (session 10 of the blocks
  program, the third FFI session and the first of the tail: these ten are
  used by no document in the company's corpus yet — they are the app-format
  manifest's remaining children, and every one crossed as a `Markdown`
  string before):
  - **`Concurrency { concurrency_type, hard_limit, soft_limit, force_https }`**
    — `::concurrency` (attributes only; `type=` crosses as `concurrency_type`).
  - **`Crates { entries }`** — `::crates`, via the new
    `NativeCrateEntry { name, source, features }` (the paren grammar of
    `parse_crates`: `github:`/`source:`, `features:`, `branch:` folded into
    the source).
  - **`Dashboard { source, refresh }`** — `::dashboard`; `source=` through
    `validate_source_path` (an external target arrives blank).
  - **`InfraDatabase { name, shared_auth, volume_gb, properties }`** —
    `::database`; the `key: value` body lines over `NativeStyleProperty`.
    The Rust name keeps the parser's `Infra` prefix; the serde tag is the
    spec's `database`.
  - **`Deploy { env, app, machines, memory, auto_stop, min_machines,
    strategy, properties }`** — `::deploy` (not `::app-deploy`, which stays
    `AppDeploy`); the body lines over `NativeStyleProperty`.
  - **`DeployUrls { entries }`** — `::deploy-urls` / `::deploy_urls`; each
    `env: url` line as a `NativeStyleProperty` (key = env, value = url).
  - **`Domains { entries }`** — `::domains`, via the new
    `NativeDomainEntry { domain, description }`.
  - **`Editor { source, lang, preview }`** — `::editor`, the editor mount
    point (not `CodeEditor` / `BlockEditor`); `source=` validated.
  - **`InfraEnv { tier, entries }`** — `::env`, via the new
    `NativeEnvEntry { name, default_value }` (`NAME=default` or a bare
    `NAME`; not `::app-env`'s `NativeEnvVar`). The serde tag is the spec's
    `env`.
  - **`Feed { source, stream }`** — `::feed`; `source=` validated, `stream`
    = SSE vs polling.
- `block_tier`: all ten → Chrome. Every infra block the spec names inside an
  `::app` is structural now; only `unknown`, `hours`, `marquee`, `health`,
  `smoke`, `volumes`, `use` and `deck` stay Degraded.
- `NATIVE_DOC_SCHEMA_VERSION` 8 → 9. `NativeBlock` is a 103-variant enum.
  The enum's docstrings stay one line per variant (1,889 bytes inside the
  enum, measured; `cargo check --features uniffi` clean — the 16 KiB
  metadata buffer still has room).

### Tests

- The tier fixture covers the ten (`::deck` stays the Degraded probe); one
  conversion test per variant from real source (the crates and domains paren
  grammars, `NAME=default`, the validated sources, the underscored
  `::deploy_urls`); `infra_children_are_structural_at_schema_v9`; the serde
  tags of the two `Infra` variants; a uniffi-gated end-to-end through
  `parse_to_native`; the `tier4-manifest` native snapshot regenerated
  (`::deploy[target=fly]` serializes structurally with every fact absent —
  `target=` is not an attribute the parser reads; the HTML snapshot is
  byte-identical, the web renderer is untouched).

## 0.23.0 — 2026-09-22 (native schema v8: the last ten blocks with measured use)

### Added

- **Ten new `NativeBlock` variants — schema v8** (session 9 of the blocks
  program, the second FFI session). Every one crossed as a `Markdown` string
  before; each now carries its parsed shape:
  - **`Auth { provider, session, roles, default_role }`** — `::auth`;
    `provider` is the parser's lowercase word (`email` · `oauth` · `api-key`
    · `token`).
  - **`AppDeploy { region, scale, domain, memory, properties }`** —
    `::app-deploy`; the parser's `(key, value)` tuples cross as
    `NativeStyleProperty` (a tuple cannot cross UniFFI).
  - **`Booking { title, service_label, services, days }`** — `::booking`,
    via the new `NativeBookingService { name, duration, price }` and
    `NativeBookingDay { date, slots }`.
  - **`Schema { name, fields }`** — `::schema`, reusing `NativeModelField`
    (a schema field is a model field; types and constraints spelled the
    model way: `enum(a, b)`, `ref(X)`, `min=1`, `default=x`).
  - **`ChatInput { action, placeholder, modes }`** — `::chat-input` (the
    routed composer; `::chat-input-simple` stays `ChatInputSimple`).
  - **`Store { title, currency, items }`** — `::store`, via the new
    `NativeStoreItem { name, price, blurb, badge, category }`.
  - **`AppEnv { vars }`** — `::app-env`, via the new
    `NativeEnvVar { name, description, required }`.
  - **`Binding { source, target, events }`** — `::binding`, via the new
    `NativeBindingEvent { event, action }`.
  - **`Build { base, runtime, edition, properties }`** and
    **`Cicd { provider, properties }`** — `::build`, `::cicd`, over
    `NativeStyleProperty`.
- `block_tier`: auth / app-deploy / app-env / binding / build / cicd /
  chat-input → Chrome, booking / store → Site, schema → Content. An `::app`
  manifest's children are therefore structural now — the S8 frame stops
  drawing markdown inside itself.
- `NATIVE_DOC_SCHEMA_VERSION` 7 → 8. `NativeBlock` is a 93-variant enum.
- `spec/blocks.toml`: `[blocks.app].attributes` gains `auth` (the parser
  has read it since the manifest format existed).

### Changed

- **The `NativeBlock` enum's docstrings moved to a module-level variant
  ledger** (lane 0 of session 9). uniffi 0.28.3 packs every variant and
  field docstring into one 16 KiB metadata buffer
  (`uniffi_core::metadata::BUF_SIZE`); the prose measured ~9 KB of it at
  v7 and left ~200 bytes of slack. Each variant keeps a one-line
  `/// ::block`; the generated Swift comments change accordingly. No
  shape change.
- **Attributes-vs-body (D-S9-5):** `::style` (accent / font / heading-font
  / body-font), `::route` (auth / returns / body), `::auth` (session /
  roles / default_role) and `::chat-input` (modes) now parse their keys as
  ATTRIBUTES as well as body lines — the form `spec/blocks.toml` lists. A
  body line wins over the attribute. Documents that author body lines are
  byte-identical; the corpus manifest fixture, which authors
  `::route[… returns=list(User)]`, gains its `returns` in both snapshots.

### Tests

- The tier fixture covers the ten (`::deck` stays the Degraded probe); one
  conversion test per variant from real source (the attribute forms
  included); `app_children_are_structural_at_schema_v8`; a uniffi-gated
  end-to-end through `parse_to_native`; `tier4-manifest` snapshots
  regenerated (auth and route serialize structurally, `returns` present).

## 0.22.0 — 2026-09-22 (native schema v7: the last eight web-only blocks)

### Added

- **Eight new `NativeBlock` variants — schema v7.** The blocks that crossed
  the FFI borrowed or as a Markdown string now carry their parsed shape:
  - **`Style { properties }`** — the `key: value` body lines of `::style`
    (`accent`, `font`, `heading-font`, `body-font`, …), via the new
    `NativeStyleProperty`.
  - **`Logo { src, alt, size }`** — `::logo`.
  - **`Route { method, path, auth, returns, body, handler, content }`** —
    `::route`; `method` crosses as the uppercase verb (GET/POST/PUT/PATCH/
    DELETE) and `handler` as the fenced source.
  - **`Action { method, target, label, fields, confirm }`** — `::action`;
    `fields` are `NativeFormField`s mapped by the same helper `::form` uses,
    so the two can never drift.
  - **`Model { name, fields }`** — `::model`, via the new
    `NativeModelField { name, field_type, constraints }` carrying the spec
    spellings (`uuid`, `enum(a, b)`, `ref(Team)`; `primary`, `max=254`,
    `default=now()`).
  - **`App { name, binary, region, port, platform, auth, content, children }`**
    — `::app`; children convert through `convert_children`, so the infra
    blocks inside still degrade to Markdown.
  - **`SegmentedControl { active, size, action, segments }`** — `::segmented-
    control`, via the new `NativeSegmentItem`. It used to be BORROWED as a
    `TabBar`, which dropped `size=` and the block-level `action=` and told
    the client it was switching panes rather than picking a filter value.
  - **`DropdownSelect { label, icon, selected, align, options }`** —
    `::dropdown-select`, via the new `NativeDropdownOption`. It used to be
    BORROWED as a `CommandPalette`, which dropped `icon=`, `align=` and the
    label/selected distinction.
  `NativeBlock` is now an 83-variant enum. `block_tier` files Route, Action
  and Model under Content, Logo under Site, and Style and App under Chrome;
  the two borrowed arms are gone, so a client matching `TabBar` or
  `CommandPalette` no longer receives segmented-control or dropdown payloads
  there.
- **`NATIVE_DOC_SCHEMA_VERSION` 6 → 7.**
- **D-S8-5 — `::style` reaches `NativeTheme`.** Nothing in `resolve` read
  `Block::Style` before, so a document's own `accent`/`font` crossed the FFI
  only if it also carried a `::site` block. New
  `resolve::style_theme_inputs(&blocks) -> StyleThemeInputs` collects the
  four theme-bearing keys (`accent`, `font`, `heading-font`, `body-font` —
  the same set `render_html::apply_style_overrides` honours, last `::style`
  wins), and new `resolve::resolve_theme_with_fonts(...)` addresses the
  display and body stacks separately. `ffi::parse_to_native_styled` applies
  them with precedence host argument > `::style` > `::site` > default.
  `resolve_theme` is unchanged and delegates, so a document with no
  `::style` resolves exactly the theme it did before.

### Changed

- `spec/blocks.toml` now states what the parser actually reads (D-S8-6): the
  attributes the parser reads hyphenated are spelled hyphenated
  (`badge-color`, `cta-label`, `cta-href`, `on-select`, `desktop-only`,
  `title-source`, `line-numbers`, `on-rename`, `on-delete`, `on-submit`,
  `on-resolve`, `on-action`, `on-react`, `on-doc-open`, `on-change`);
  `[blocks.filter-bar]` reads `target=`, not `target_selector`;
  `[blocks.board]`'s `columns` and `card_template` are BODY lines
  (`columns: A | B`, `card-template: …`), not attributes, and moved into
  `purpose`; `[blocks.command-palette].purpose` now names the `icon` its
  parser reads. The registry's table count is unchanged (122). The
  underscored attributes the parser really does read underscored
  (`::database`'s `shared_auth`/`volume_gb`, `::deploy`'s
  `auto_stop`/`min_machines`, `::concurrency`'s
  `hard_limit`/`soft_limit`/`force_https`) are left as they are.

### Notes

- `NativeBlock`'s uniffi metadata (names, types and EVERY `///` docstring)
  must fit one 16 KiB const buffer in uniffi 0.28.3; v7 leaves roughly 200
  bytes of slack. New variants need one- or two-line docstrings — the prose
  belongs on the supporting record types or the module docs.

## 0.21.0 — 2026-09-18 (`::hours` + `::marquee`, section body fix, site-page stylesheet config)

### Added

- **`::hours`** — an opening-hours table: one `Monday: 11am - 9pm` row per
  line (hyphen or en dash; `9`, `9:30`, `21:00`, `9am`, `9:30 PM`; `Closed`),
  with `title=` and an IANA `timezone=` recorded but never interpreted. The
  block parses each row to a weekday index (0 = Sunday) and opening/closing
  minutes since local midnight; a `closes` at or below `opens` is an overnight
  range. The renderer emits `.surfdoc-hours` → an `h3.surfdoc-hours-title`
  carrying a `span.surfdoc-hours-status`, then a table whose rows carry
  `data-day`. **The pure render states no open state — this crate holds no
  clock.**
- **`render_hours_with_now(block, weekday, minutes_since_midnight)`** — the
  same markup with the caller's LOCAL time stamped in: today's row gains
  `is-today`, the status span gains `is-open`/`is-closed` and reads
  `Open now · until 10pm` / `Closed · opens 11am` (naming the day when the
  next opening is not today). The hosted container calls it at serve time
  with the site's timezone, so the band needs no client script.
  **`hours_opening_specification(block)`** projects the rows onto schema.org
  `openingHoursSpecification` entries for JSON-LD.
- **`::marquee`** — a looping ticker band: one item per body line (optional
  leading `- `), rendered as `div.surfdoc-marquee[aria-hidden]` wrapping a
  `.surfdoc-marquee-track` whose item/separator sequence is emitted TWICE, so
  the stylesheet's `translateX(-50%)` loop meets itself seamlessly. The
  animation and a `prefers-reduced-motion: reduce` rule that stops it ship in
  `assets/surfdoc.css`. No script.
- Registry rows for both (`total_blocks` 120 → 122), parsers, the `render_dom`
  twins (byte-identical), markdown degradation (hours → a plain table,
  marquee → a comma-joined line), serializer fixed points, a `css_coverage`
  snippet each, and the fixture `tests/fixtures/site-blocks.surf`, pinned in
  the lint parse baseline and the DOM byte-identity corpus.

### Fixed

- **`::section` dropped its body** when it carried no leading `## ` headline:
  the scan pushed the body start past every blank line while it was still
  hunting for one, so only the text after the LAST blank line survived — and
  a `## ` appearing later, including inside a nested child, was stolen as the
  section's headline. Only the FIRST non-blank line can be the headline now;
  otherwise the body starts there. The subtitle rule and the child span math
  are unchanged. A hostile-corpus expectation that had pinned the dropped
  branch as "`::section` does not adopt directive children" is corrected: it
  does, and now renders whole.

### Changed

- `render_site_page` and `render_site_document` honour `PageConfig::embed_css`
  and `PageConfig::stylesheets`, which only the shell path read before:
  `embed-css: false` omits the base sheet but KEEPS the site-nav sheet and the
  accent override block, and each configured stylesheet emits an escaped
  `<link>` after the style tag. With the default config the bytes are
  unchanged, so no golden or corpus snapshot churns.

## 0.20.0 — 2026-09-04 (type: spreadsheet + `::data` name/source/rows/cols)

### Added

- **`type: spreadsheet`** — a new document type whose top-level `::data`
  blocks are the sheets of one workbook, and the matching
  **`RenderProfile::Spreadsheet`**. The front-matter vocabulary, the
  serializer, the lint doc-type list and the render-profile map all carry it;
  every other document type resolves exactly as before.
- **Four `::data` attributes**: `name=` labels the sheet, `source=` points at
  rows held out of line (`file:<id>` or `doc:<id>#<sheet>`), and `rows=`/`cols=`
  carry the size of what is out there. A count that is not a whole number is
  ignored rather than fatal, and all four survive the parse–serialize round
  trip.
- **Lint `L044`** — a `::data` block carrying `source=` without both `rows=`
  and `cols=`. Warning, not fixable: only the referenced source knows its own
  dimensions.
- **The workbook layout** for a spreadsheet document: a `.surfdoc-workbook`
  section wrapping a `.surfdoc-sheet-strip` nav that names every sheet, then
  one `.surfdoc-sheet` section per sheet carrying its label in `data-sheet`.
  A sheet with no `name=` is `Sheet1`, `Sheet2`, … by position. Stylesheet
  rules for the strip, the sheets and the linked count line come with it.

### Changed

- The `.surfdoc-table-more` count line becomes an anchor when the block
  carries `source=` — `/files/<id>` for a file reference, `/docs/<id>` for a
  document reference (the sheet fragment names the sheet, not the path) — and
  its count comes from `rows=`, because the inline body is then only a
  preview. An unresolvable reference keeps the inert paragraph rather than
  inventing a URL, and a sourced block with no inline rows still renders its
  header row and the count line.
- Markdown degradation follows: a `source=` block emits its preview rows and
  then the plain count line, and a spreadsheet document precedes each sheet
  with a level-two heading holding the sheet name. Blocks without `source=`
  and documents of every other type keep their present bytes.
- A markdown pipe table over twenty body rows is now capped in both web
  backends in the same shape a `::data` block uses — the preview class, the
  `data-rows`/`data-cols` pair and the inert count line. At or under twenty
  rows the bytes are unchanged, so no snapshot churns.
- The block registry records the eight authored `::data` attributes and keeps
  `filterable` and `chart` as planned, not implemented: the parser does not
  honour them yet.

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

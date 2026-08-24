//! CSS coverage guard.
//!
//! Every `surfdoc-*` class the HTML renderer emits for any implemented
//! registry kind must have at least one rule in the public stylesheet
//! (`assets/surfdoc.css`, exposed as `surf_parse::SURFDOC_CSS`). A class
//! emitted with no rule is invisible styling debt: the block renders
//! unstyled and nothing fails.
//!
//! Mechanics: one small document per implemented registry kind (the
//! snippet table below), rendered through `to_html`; every token inside a
//! `class="…"` attribute that starts with `surfdoc-` must appear as a
//! selector (`.token` at a selector boundary) in SURFDOC_CSS.
//!
//! Acceptance rules, in order:
//!  1. Direct rule: `.class` appears at a selector boundary in the CSS.
//!  2. Variant tag: the token is co-emitted in the SAME class attribute
//!     as a token that has a direct rule (e.g. `surfdoc-infra-card
//!     surfdoc-build` — the kind tag exists so CSS *may* specialize, the
//!     styled base carries the look; same for `surfdoc-qr surfdoc-qr-show`,
//!     `surfdoc-diagram surfdoc-diagram-architecture`, sample/template
//!     twins on list/board/recipient-picker/block-editor previews).
//!  3. Named ALLOWLIST below — small, justified per entry.
//!
//! Out of scope by construction: site-page chrome (`surfdoc-site-*`) is
//! emitted only through the site pipeline (`render_site_page`), which
//! embeds its own stylesheet inside render_html.rs, not surfdoc.css; and
//! state modifiers that don't start with `surfdoc-` (`active`,
//! `is-selected`, …) only make sense composed with their base class.

use std::collections::BTreeSet;

/// Classes accepted without a rule of their own. Every entry must carry a
/// justification; grow this list only with the same scrutiny as a
/// renderer change.
const ALLOWLIST: &[&str] = &[
    // SVG scene-graph hooks inside ::diagram output; presentation is
    // carried by inline SVG attributes (diagram.rs), the classes exist
    // for downstream tooling/theming.
    "surfdoc-diagram-node",
    "surfdoc-diagram-edge",
    "surfdoc-diagram-edge-label",
    // <template> holder for the live list-item template — inert DOM,
    // never painted, so a rule would be dead weight.
    "surfdoc-list-item-template",
    // Command-palette entry internals; styled via the descendant
    // selector `.surfdoc-command-list li` rather than per-class rules.
    "surfdoc-command-item",
    "surfdoc-command-label",
    "surfdoc-command-desc",
    // Nav-tree label span; styled via its `.surfdoc-tree-folder` /
    // `.surfdoc-tree-file` parents.
    "surfdoc-tree-name",
    // ::progress status hook (`surfdoc-step-<status>`); rows are styled
    // via `.surfdoc-progress li`, the status class is a styling hook.
    "surfdoc-step-pending",
    // ::product-grid link-card text column — a bare flow wrapper inside
    // the styled `.surfdoc-pg-body` flex row; its children
    // (`.surfdoc-pg-name`, `.surfdoc-pg-tagline`) carry the type rules.
    "surfdoc-pg-text",
];

/// One minimal source document per implemented registry kind
/// (spec/blocks.toml, status = "implemented"; registry currently has 99
/// implemented of 112 total). When a kind is added to the registry, the
/// companion completeness check below fails until it gets a snippet here.
const SNIPPETS: &[(&str, &str)] = &[
    ("callout", "::callout[type=warning title=\"Heads up\"]\nBody\n::"),
    ("chart", "::chart[type=line source=\"/api/metrics\" period=weekly title=\"WAU\"]\n::"),
    ("code", "::code[lang=rust file=src/main.rs]\nfn main() {}\n::"),
    ("columns", "::columns\n::: column\nLeft\n:::\n::: column\nRight\n:::\n::"),
    ("cta", "::cta[label=\"Go\" href=/go primary=true]\n::"),
    ("data", "::data[sortable=true]\n| A | B |\n|---|---|\n| 1 | 2 |\n::"),
    ("decision", "::decision[status=accepted date=2026-06-11]\nShip it.\n::"),
    ("diagram", "::diagram[type=architecture title=\"Flow\"]\nweb: Web\napi: API\nweb -> api: HTTPS\n::"),
    ("details", "::details[title=\"More\" open=true]\nHidden\n::"),
    ("divider", "::divider[label=SECTION]\n::"),
    ("embed", "::embed[src=\"https://example.com\" type=iframe title=\"Demo\"]\n::"),
    ("faq", "::faq\n- q=\"Fast?\" a=\"Yes.\"\n::"),
    ("figure", "::figure[src=/img/a.png alt=\"A\" caption=\"First\"]\n::"),
    ("footer", "::footer[copyright=\"© 2026\"]\n::"),
    ("form", "::form[submit=\"Send\"]\n- name text \"Your name\" required\n- email email \"Email\"\n::"),
    ("gallery", "::gallery[columns=2]\n- src=/img/a.png alt=\"A\" caption=\"First\"\n::"),
    ("hero-image", "::hero-image[src=/img/hero.png alt=\"Hero\"]\n::"),
    ("metric", "::metric[label=\"Tests\" value=42 trend=up unit=tests]\n::"),
    ("nav", "::nav[logo=\"Co\"]\n- Home /\n- Pricing /pricing\n::"),
    ("page", "::page[route=/ title=\"Home\"]\nBody\n::"),
    ("pricing-table", "::pricing-table\n| Plan | Price |\n|------|-------|\n| Free | $0 |\n::"),
    ("post-grid", "::post-grid[title=\"Posts\" subtitle=\"Latest\"]\n::"),
    ("product-grid", "::product-grid[cols=3]\n- Surf CLI /cli\n::"),
    ("gate", "::gate[title=\"Members\" action=/api/gate field=email submit=\"Enter\"]\n::"),
    ("progress", "::progress[source=deploy.progress]\n- Parse\n- Ship\n::"),
    ("quote", "::quote[by=\"Ada\" cite=\"Notes\"]\nAll that is gold.\n::"),
    ("site", "::site\nname: Co\naccent: #10b981\n::"),
    ("style", "::style\naccent: #2563eb\n::"),
    ("summary", "::summary\nOne source.\n::"),
    ("tabs", "::tabs\n::: tab[title=\"First\"]\nOne\n:::\n::"),
    ("tasks", "::tasks\n- [x] Done\n- [ ] Todo\n::"),
    ("testimonial", "::testimonial[author=\"Ada\" role=\"Engineer\" company=\"Co\"]\nGreat.\n::"),
    ("hero", "::hero\nheadline: Build once\nsubtitle: Render everywhere\nbadge: NEW\n::"),
    ("features", "::features\n- icon=bolt title=\"Fast\" body=\"Rust\"\n::"),
    ("steps", "::steps\n1. Write\n2. Ship\n::"),
    ("stats", "::stats\n- value=\"95\" label=\"Blocks\"\n::"),
    ("comparison", "::comparison[highlight=\"SurfDoc\"]\n| Feature | SurfDoc | Other |\n|---|---|---|\n| One source | Yes | No |\n::"),
    ("logo", "::logo[src=/img/logo.png alt=\"Logo\" size=48]\n::"),
    ("toc", "::toc[depth=2]\n::"),
    ("before-after", "::before-after\nbefore:\n- Old\nafter:\n- New\n::"),
    ("pipeline", "::pipeline\n- Parse\n- Render\n::"),
    ("section", "::section[headline=\"Pitch\" subtitle=\"Why\"]\nInner\n::"),
    ("product-card", "::product-card[badge=\"Popular\" badge-color=\"green\"]\n## Pro\nFor teams\n\n- Everything\n\n[Start](/pricing)\n::"),
    ("list", "::list[source=\"/api/tasks\" display=card preload=true]\n## {= title =}\n{= summary =}\n::"),
    ("board", "::board[source=\"/api/board\" preload=true]\ncolumns: To Do | Done\n### {= title =}\n::"),
    ("action", "::action[method=POST target=\"/api/ship\" label=\"Ship\" confirm=\"Sure?\"]\n::"),
    ("filter-bar", "::filter-bar[target=\"#tasks\"]\n- Status (select: All | Done)\n::"),
    ("search", "::search[source=\"/api/search\" placeholder=\"Search…\"]\n::"),
    ("dashboard", "::dashboard[source=\"/api/stats\" refresh=30]\n::"),
    ("chat-input", "::chat-input[action=send placeholder=\"Ask…\"]\n::"),
    ("feed", "::feed[source=\"/api/feed\" stream=feed_updated]\n::"),
    ("editor", "::editor[source=doc lang=surf preview=true]\n::"),
    ("split-pane", "::split-pane[ratio=50]\n::"),
    ("pane", "::split-pane[ratio=50 back-label=\"Chats\" back-action=closeConversation]\n:::pane[side=left]\n::::row[icon=knowledge href=#]\nSam Rose\n::::\n:::\n:::pane[side=right]\nThread\n:::\n::"),
    ("app", "::app[name=demo]\n::"),
    ("build", "::build[base=debian runtime=rust edition=2024]\n::"),
    ("database", "::database[name=main shared-auth=true volume-gb=1]\n::"),
    ("deploy", "::deploy[target=fly]\n::"),
    ("env", "::env[tier=prod]\nKEY: value\n::"),
    ("health", "::health[path=/healthz method=GET]\n::"),
    ("concurrency", "::concurrency[type=requests hard-limit=250]\n::"),
    ("cicd", "::cicd[provider=github]\n::"),
    ("smoke", "::smoke[script=scripts/smoke.sh]\n::"),
    ("domains", "::domains\n- example.com\n::"),
    ("crates", "::crates\n- serde\n::"),
    ("deploy-urls", "::deploy-urls\n- https://example.com\n::"),
    ("volumes", "::volumes\n- data /data 1\n::"),
    ("model", "::model[name=User]\n- id: uuid pk\n- email: string unique\n::"),
    ("route", "::route[method=GET path=/api/users returns=list(User)]\n::"),
    ("auth", "::auth[provider=email]\n::"),
    ("binding", "::binding[source=users target=list]\n::"),
    ("schema", "::schema[name=User]\n- id: uuid pk\n::"),
    ("use", "::use\n- serde\n::"),
    ("app-env", "::app-env\nKEY: value\n::"),
    ("app-deploy", "::app-deploy[region=sjc scale=1]\n::"),
    ("row", "::row[icon=doc href=\"/docs\" unread=true avatar=auto rtime=\"1:42 PM\" unread-count=3 trailing-label=\"Open\" trailing-action=open progress=0.42]\nTitle\nDescription\naction: Accept | invoke:contacts.accept\n::"),
    ("infocard", "::infocard[intent=success image=\"/img/a.png\"]\n# Card\nSubtitle\n\nSummary text.\n\n1. Step one\n\nVersion: 1.0\n::"),
    // Full shell shape (0.14): sidebar rows + divider + hub row (drives the
    // generated tab-bar), topbar, tab-content, and a RIGHT panel (drives
    // the drawer + FAB), so every responsive-chrome class is covered.
    ("app-shell", "::app-shell[layout=sidebar-main-panel height=600]\n:::sidebar[position=left width=240]\n::::toolbar\n- text[value=\"Surfspace\" size=22]\n::::\n::::row[icon=doc href=#]\nDocs\n::::\n::::row[icon=knowledge href=# unread=true]\nMessages\n::::\n::::divider\n::::\n::::row[icon=settings href=#]\nSettings\n::::\n:::\n:::toolbar\n- button[label=\"Search\" icon=search action=openSearch]\n- separator\n- button[label=\"Surfy\" icon=surfy-fin action=toggleSurfy]\n:::\n:::tab-content[tab=main]\nPane\n:::\n:::panel[position=right]\nSurfy body\n::::chat-input-simple[placeholder=\"Ask\" action=send]\n::::\n:::\n::"),
    ("sidebar", "::sidebar[position=left collapsible=true width=240]\n::"),
    ("panel", "::panel[position=bottom resizable=true height=160 desktop-only=true]\n::"),
    ("tab-bar", "::tab-bar[active=preview]\n- preview \"Preview\" {icon=eye unread=true}\n- edit \"Edit\"\n::"),
    ("tab-content", "::tab-content[tab=preview width=880 align=center]\nPane\n::"),
    ("toolbar", "::toolbar[title=\"Messages\" title-source=thread.display_name]\n- button[label=\"Run\" action=run style=primary toggled=true]\n- button[icon=filter action=open_filter]\n- button[label=\"cloudsurf\" avatar=\"C\" action=switch_workspace]\n- text[value=\"Surfspace\" size=22]\n- separator\n- spacer\n- badge[value=\"Live\" color=green]\n- dropdown[options=\"A|B\"]\n::"),
    ("drawer", "::drawer[name=filters position=right width=320 trigger=\"Filters\"]\nBody\n::"),
    ("modal", "::modal[name=confirm title=\"Confirm\" width=480 placement=centered dismissible=false]\nSure?\n::"),
    ("segmented-control", "::segmented-control[active=all size=compact action=filter]\n- all \"All\"\n- done \"Done\"\n::"),
    ("dropdown-select", "::dropdown-select[label=\"Sort\" icon=arrow selected=\"Newest\" align=right]\n- \"Newest\" description=\"Most recent\" icon=clock action=sort_newest\n- \"Oldest\"\n::"),
    ("command-palette", "::command-palette[trigger=cmd+k]\n- \"Deploy\" description=\"Ship\" action=deploy icon=paperplane group=Ops\n::"),
    ("code-editor", "::code-editor[lang=surf source=doc line-numbers=true]\n# Doc\n::"),
    ("block-editor", "::block-editor[source=doc]\n::"),
    ("terminal", "::terminal[shell=zsh cwd=~/code]\n::"),
    ("nav-tree", "::nav-tree[source=files on-select=open_file]\n::"),
    ("badge", "::badge[value=3 color=red]\n::"),
    ("suggestion-chips", "::suggestion-chips[source=ai.suggestions max=3 dismissible=true]\n::"),
    ("recipient-picker", "::recipient-picker[source=contacts mode=multi on-submit=\"invoke:messages.compose\"]\n::"),
    ("qr", "::qr[mode=show]\n::"),
    ("chat-thread", "::chat-thread[source=chat.thread on-action=run_action]\n- them[sender=\"Danny\" time=\"1:42 PM\" reactions=\"Love:2:mine|Wave\"] Tahoe update finished\n- them[sender=\"Surfy\"] Enrollment retried\n- own[time=\"1:44 PM\"] Yes, retry now\n::"),
    ("chat-input-simple", "::chat-input-simple[placeholder=\"Ask…\" action=send]\n::"),
    ("chip-input", "::chip-input[label=\"To:\" placeholder=\"Type a name…\" source=contacts on-change=\"invoke:messages.compose\"]\n- Danny Pappageorge\n::"),
    ("log-stream", "::log-stream[source=build.log tail=100]\n::"),
    ("problem-list", "::problem-list[source=diagnostics]\n::"),
];

/// Every class attribute in the HTML, as its list of tokens.
fn class_attributes(html: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("class=\"") {
        let after = &rest[start + 7..];
        let Some(end) = after.find('"') else { break };
        out.push(after[..end].split_whitespace().map(str::to_string).collect());
        rest = &after[end + 1..];
    }
    out
}

/// True when `.class` appears in the stylesheet at a selector boundary
/// (not merely as a prefix of a longer class name).
fn css_has_rule(css: &str, class: &str) -> bool {
    let needle = format!(".{class}");
    let mut from = 0;
    while let Some(pos) = css[from..].find(&needle) {
        let abs = from + pos;
        let after = css[abs + needle.len()..].chars().next();
        // Boundary: end of file, or any char that cannot continue a class
        // ident (letters, digits, - and _ continue it).
        let boundary = match after {
            None => true,
            Some(c) => !(c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        };
        if boundary {
            return true;
        }
        from = abs + needle.len();
    }
    false
}

/// The snippet table stays in lockstep with the registry: exactly one
/// snippet per implemented registry kind.
#[test]
fn css_coverage_snippet_table_matches_registry() {
    let registry: toml::Value =
        toml::from_str(include_str!("../spec/blocks.toml")).expect("blocks.toml parses");
    let implemented: BTreeSet<&str> = registry["blocks"]
        .as_table()
        .unwrap()
        .iter()
        .filter(|(_, b)| b["status"].as_str() == Some("implemented"))
        .map(|(name, _)| name.as_str())
        .collect();
    let snippets: BTreeSet<&str> = SNIPPETS.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        implemented, snippets,
        "snippet table must cover exactly the implemented registry kinds"
    );
}

/// Every surfdoc class emitted for any implemented kind is covered: a
/// direct rule in assets/surfdoc.css, a styled sibling in the same class
/// attribute (variant-tag pattern), or a justified allowlist entry.
#[test]
fn every_emitted_class_has_a_css_rule() {
    let css = surf_parse::SURFDOC_CSS;
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for (name, src) in SNIPPETS {
        let doc = surf_parse::parse(src).doc;
        let html = doc.to_html();
        for tokens in class_attributes(&html) {
            let any_styled = tokens.iter().any(|t| css_has_rule(css, t));
            for token in &tokens {
                if !token.starts_with("surfdoc-") {
                    continue;
                }
                let ok = css_has_rule(css, token)
                    || (any_styled && tokens.len() > 1)
                    || ALLOWLIST.contains(&token.as_str());
                if !ok {
                    missing.insert(format!("{name}: .{token}"));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "classes emitted with no rule in assets/surfdoc.css (and no styled \
         sibling / allowlist entry):\n  {}",
        missing.iter().cloned().collect::<Vec<_>>().join("\n  ")
    );
}

/// Toolbar overflow pin (R5): a crowded toolbar must scroll or wrap
/// instead of clipping — the app-shell sets overflow:hidden and the grid
/// track can shrink below the bar's natural width. The guard requires the
/// DESKTOP rule (before the 768px media query) to carry a horizontal
/// escape valve (overflow-x auto/scroll or flex-wrap wrap) plus a zero
/// min-width, and the grid placement rule to zero its min-width too —
/// mobile-only coverage inside the media query does not count.
#[test]
fn toolbar_overflow_never_clips_on_desktop() {
    let css = surf_parse::SURFDOC_CSS;

    // Top-level = braces balance to zero before the selector (a rule
    // inside @media sits one level deep).
    let depth_at = |idx: usize| {
        css[..idx].matches('{').count() as i64 - css[..idx].matches('}').count() as i64
    };

    // Rule body of the first `.surfdoc .surfdoc-toolbar {` — the desktop
    // rule; the mobile override lives later, inside a media query.
    let sel_idx = css
        .find(".surfdoc .surfdoc-toolbar {")
        .expect("toolbar rule exists");
    assert_eq!(
        depth_at(sel_idx),
        0,
        "the first toolbar rule must be top-level (desktop), not media-query-only"
    );
    let body_start = css[sel_idx..].find('{').unwrap() + sel_idx + 1;
    let body_end = css[body_start..].find('}').unwrap() + body_start;
    let body = &css[body_start..body_end];

    let scrolls = body.contains("overflow-x: auto") || body.contains("overflow-x: scroll");
    let wraps = body.contains("flex-wrap: wrap");
    assert!(
        scrolls || wraps,
        "desktop toolbar rule must scroll or wrap, not clip: {body}"
    );
    assert!(
        body.contains("min-width: 0"),
        "desktop toolbar rule needs min-width: 0 so the flex/grid track can shrink: {body}"
    );

    // Grid placement must also release the track (like tab-content does).
    let placement_idx = css
        .find(".surfdoc .surfdoc-layout-sidebar-main-panel > .surfdoc-toolbar {")
        .expect("toolbar grid placement rule exists");
    assert_eq!(depth_at(placement_idx), 0, "grid placement rule must be top-level");
    let p_start = css[placement_idx..].find('{').unwrap() + placement_idx + 1;
    let p_end = css[p_start..].find('}').unwrap() + p_start;
    assert!(
        css[p_start..p_end].contains("min-width: 0"),
        "toolbar grid placement needs min-width: 0: {}",
        &css[p_start..p_end]
    );
}

/// D3 (0.14.1): toolbars inside tab-content are content rows, not chrome
/// bars — the 880px detail column's breadcrumb bar must WRAP, never grow
/// a horizontal scroller (Brady's doc-detail screenshot: the crumb pushed
/// share/up off-track behind a scrollbar). The desktop chrome-bar rule
/// keeps its b1e5723 scroll escape valve (pinned above); this pin covers
/// the tab-content-scoped override: top-level (not media-query-only),
/// wraps, and resets the inherited overflow-x back to visible.
#[test]
fn tab_content_toolbar_wraps_instead_of_scrolling() {
    let css = surf_parse::SURFDOC_CSS;
    let depth_at = |idx: usize| {
        css[..idx].matches('{').count() as i64 - css[..idx].matches('}').count() as i64
    };
    let sel = ".surfdoc .surfdoc-tab-content .surfdoc-toolbar {";
    let idx = css.find(sel).expect("tab-content toolbar rule exists");
    assert_eq!(depth_at(idx), 0, "the first tab-content toolbar rule must be top-level");
    let start = idx + sel.len();
    let end = css[start..].find('}').unwrap() + start;
    let body = &css[start..end];
    assert!(
        body.contains("flex-wrap: wrap"),
        "tab-content toolbars must wrap, not scroll: {body}"
    );
    assert!(
        body.contains("overflow-x: visible"),
        "tab-content toolbars must reset the chrome bar's overflow-x: auto: {body}"
    );
    assert!(
        !body.contains("overflow-x: auto") && !body.contains("overflow-x: scroll"),
        "no horizontal scroller in the detail column: {body}"
    );
    // Long breadcrumb text shrinks and ellipsizes instead of forcing the
    // bar wide (mirrors the split-right F4 chain).
    let text_sel = ".surfdoc .surfdoc-tab-content .surfdoc-toolbar .surfdoc-toolbar-text {";
    let tidx = css.find(text_sel).expect("tab-content toolbar-text rule exists");
    let tstart = tidx + text_sel.len();
    let tend = css[tstart..].find('}').unwrap() + tstart;
    let tbody = &css[tstart..tend];
    assert!(
        tbody.contains("min-width: 0") && tbody.contains("text-overflow: ellipsis"),
        "crumb text must shrink + ellipsize: {tbody}"
    );
}

/// D5 + D8 (0.14.1): a toolbar inside a modal is a button/content row —
/// the modal-scoped reset must strip the chrome-bar treatment (the dark
/// full-width band + hairline behind "Create a blank doc instead" and the
/// filter dropdowns) and give the row real vertical padding (D4).
#[test]
fn modal_toolbar_is_content_not_chrome() {
    let css = surf_parse::SURFDOC_CSS;
    let sel = ".surfdoc .surfdoc-modal .surfdoc-toolbar {";
    let idx = css.find(sel).expect("modal-scoped toolbar rule exists");
    let start = idx + sel.len();
    let end = css[start..].find('}').unwrap() + start;
    let body = &css[start..end];
    assert!(body.contains("background: transparent"), "no chrome fill in a modal: {body}");
    assert!(body.contains("border-bottom: none"), "no chrome hairline in a modal: {body}");
    assert!(body.contains("height: auto"), "no fixed 48px bar height in a modal: {body}");
    assert!(body.contains("padding: 10px 0"), "modal button rows need vertical room (D4): {body}");
    assert!(body.contains("flex-wrap: wrap"), "modal toolbars wrap, never scroll: {body}");
}

/// D5 (0.14.1): the modal keeps a sheet-scale corner in BOTH declarations —
/// the base rule (Section 68) and the post-containment override (the
/// Section 76 grouped rule re-imposes var(--ws-control-radius, 10px), and
/// the live layer declares no radius at all) — with the overflow clip so
/// children cannot square the corners.
#[test]
fn modal_corner_radius_is_sheet_scale_in_both_declarations() {
    let css = surf_parse::SURFDOC_CSS;
    let sel = ".surfdoc .surfdoc-modal {";
    let mut bodies = Vec::new();
    let mut from = 0;
    while let Some(pos) = css[from..].find(sel) {
        let start = from + pos + sel.len();
        let end = css[start..].find('}').unwrap() + start;
        bodies.push(&css[start..end]);
        from = end;
    }
    let radiused: Vec<&&str> = bodies
        .iter()
        .filter(|b| b.contains("border-radius: 16px"))
        .collect();
    assert!(
        radiused.len() >= 2,
        "both modal radius declarations (base + post-containment override) \
         must carry the 16px sheet corner; found {} of {} rule bodies: {:?}",
        radiused.len(),
        bodies.len(),
        bodies
    );
    // The override (the last modal rule) also keeps the clip.
    let last = bodies.last().expect("at least one modal rule");
    assert!(
        last.contains("border-radius: 16px") && last.contains("overflow: hidden"),
        "the final modal rule must pin radius + clip so the containment \
         rule cannot square the sheet: {last}"
    );
}

/// D9 (0.14.1): the toolbar-dropdown pill release must carry
/// .surfdoc-dropdown-select specificity — as a plain 2-class rule it lost
/// min-width/margin to the later Section 72b base rule, which re-imposed a
/// 220px pill around a label-width trigger (clicks right of the text hit
/// the inert wrapper). And the trigger fills its pill (width: 100%), the
/// same shape as the panel-head release pinned above.
#[test]
fn toolbar_dropdown_trigger_fills_its_pill() {
    let css = surf_parse::SURFDOC_CSS;

    // Base rule keeps its geometry (in-content dropdowns byte-identical).
    let base_sel = ".surfdoc .surfdoc-dropdown-select {";
    let base_idx = css.find(base_sel).expect("base dropdown-select rule exists");
    let base_start = base_idx + base_sel.len();
    let base_end = css[base_start..].find('}').unwrap() + base_start;
    assert!(
        css[base_start..base_end].contains("min-width: 220px"),
        "base dropdown-select keeps min-width: 220px — the D9 fix is scoped: {}",
        &css[base_start..base_end]
    );

    // Pill rule carries 3-class specificity so the base rule can never
    // re-impose its min-width/margin regardless of source order.
    let pill_sel = ".surfdoc .surfdoc-dropdown-select.surfdoc-toolbar-dropdown {";
    let pill_idx = css.find(pill_sel).expect("specificity-hardened toolbar pill rule exists");
    let pill_start = pill_idx + pill_sel.len();
    let pill_end = css[pill_start..].find('}').unwrap() + pill_start;
    let pill = &css[pill_start..pill_end];
    assert!(pill.contains("min-width: 0"), "toolbar pill releases the 220px floor: {pill}");
    assert!(pill.contains("margin: 0"), "toolbar pill drops the content-flow margin: {pill}");

    // Trigger fills the pill — a width:auto button sizes to its label and
    // leaves a dead zone right of the text.
    let trig_sel = ".surfdoc .surfdoc-toolbar-dropdown .surfdoc-dropdown-trigger {";
    let trig_idx = css.find(trig_sel).expect("toolbar dropdown trigger rule exists");
    let trig_start = trig_idx + trig_sel.len();
    let trig_end = css[trig_start..].find('}').unwrap() + trig_start;
    let trig = &css[trig_start..trig_end];
    assert!(
        trig.contains("width: 100%"),
        "trigger must fill its pill so the whole control is clickable: {trig}"
    );
    assert!(
        !trig.contains("width: auto"),
        "width: auto re-opens the D9 dead zone: {trig}"
    );
}

/// Extract the body of the first media query whose prelude contains
/// `needle` — brace-balanced, so nested rules stay inside.
fn media_block(css: &str, needle: &str) -> String {
    let mut from = 0;
    while let Some(pos) = css[from..].find("@media") {
        let abs = from + pos;
        let brace = css[abs..].find('{').expect("media block opens") + abs;
        if css[abs..brace].contains(needle) {
            let mut depth = 0i64;
            for (i, c) in css[brace..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return css[brace + 1..brace + i].to_string();
                        }
                    }
                    _ => {}
                }
            }
        }
        from = brace;
    }
    panic!("no media query with prelude containing {needle:?}");
}

/// WP-R1 (0.14): the medium icon-rail block exists and every selector in
/// it that touches shell chrome is scoped under the app shell.
#[test]
fn medium_icon_rail_block_is_shell_scoped() {
    let body = media_block(surf_parse::SURFDOC_CSS, "max-width: 1023px");
    assert!(
        body.contains(".surfdoc .surfdoc-app-shell .surfdoc-sidebar"),
        "1023px block must scope the icon rail under .surfdoc-app-shell"
    );
    assert!(
        body.contains("min-width: 68px") && body.contains("max-width: 68px"),
        "icon rail needs min/max-width clamps — a bare `width` loses to the \
         renderer's inline style=\"width:NNNpx\" when the sidebar has a width= attr"
    );
    assert!(
        body.contains(".surfdoc-panel-right"),
        "1023px block must switch the right drawer to overlay mode"
    );
    assert!(
        !body.contains("position: fixed"),
        "medium overlay must be absolute, not fixed — Section 76 containment"
    );
}

/// WP-R1 (0.14): the small block hides the sidebar and shows the
/// generated floating tab-bar.
#[test]
fn small_block_swaps_sidebar_for_generated_tabbar() {
    let css = surf_parse::SURFDOC_CSS;
    // Two ≤767px blocks exist (legacy Section 74 + Section 74c); the
    // tab-bar lives in the one that mentions it.
    let mut found = false;
    let mut from = 0;
    while let Some(pos) = css[from..].find("max-width: 767px") {
        let abs = from + pos;
        let brace = css[abs..].find('{').unwrap() + abs;
        let mut depth = 0i64;
        let mut end = css.len();
        for (i, c) in css[brace..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = brace + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &css[brace + 1..end];
        if body.contains(".surfdoc .surfdoc-app-tabbar") {
            assert!(
                body.contains(".surfdoc .surfdoc-app-shell .surfdoc-sidebar { display: none; }"),
                "767px tab-bar block must hide the shell sidebar"
            );
            assert!(body.contains("display: flex"), "tab-bar must be shown as flex");
            found = true;
        }
        from = end;
    }
    assert!(found, "no 767px block styles .surfdoc-app-tabbar");
    // The shell grid must NOT collapse to a single-column template on small
    // screens: children are pinned to grid-column 2 (Section 56), so a `1fr`
    // override would strand them in an implicit auto column beside an empty
    // 1fr track. Column 1 auto-collapses when the sidebar hides instead.
    assert!(
        !css.contains(".surfdoc .surfdoc-layout-sidebar-main-panel { grid-template-columns: 1fr; }"),
        "small-screen single-column template override must stay removed"
    );
    // Outside the media queries the generated tab-bar and FAB stay hidden.
    assert!(css.contains(".surfdoc .surfdoc-app-tabbar { display: none;"));
    assert!(css.contains(".surfdoc .surfdoc-panel-fab {\n    display: none;"));
}

/// WP-R5 (0.14): the native topbar treatments are pinned to the DIRECT
/// app-shell toolbar child and carry no !important (the live layer must
/// be able to shrink them without a specificity war).
#[test]
fn native_topbar_treatments_are_scoped_without_important() {
    let css = surf_parse::SURFDOC_CSS;
    for action in ["openSearch", "toggleSurfy"] {
        let sel = format!(
            ".surfdoc .surfdoc-app-shell > .surfdoc-toolbar [data-action=\"{action}\"] {{"
        );
        let idx = css.find(&sel).unwrap_or_else(|| panic!("missing topbar rule for {action}"));
        let body_start = idx + sel.len();
        let body_end = css[body_start..].find('}').unwrap() + body_start;
        assert!(
            !css[body_start..body_end].contains("!important"),
            "{action} topbar rule must not use !important"
        );
    }
}

/// F1 (0.14 drawer polish): the tier dropdown inside the drawer head must
/// not inherit the content dropdown's 220px min-width — at 1280 the open
/// drawer's head row (fin + tier + spacer + Chats + ✕) otherwise overflows
/// the 360px inner width and pushes the close ✕ off-viewport (DOM probe:
/// ✕ right edge 1313 > 1280, head scrollWidth 392 > 360). The fix is a
/// panel-head-scoped release, NOT a change to the base rule: toolbar and
/// in-content dropdown-select sites keep their 220px geometry.
#[test]
fn panel_head_dropdown_releases_the_base_min_width() {
    let css = surf_parse::SURFDOC_CSS;

    // Base rule keeps its geometry (byte-identical renders elsewhere).
    let base_sel = ".surfdoc .surfdoc-dropdown-select {";
    let base_idx = css.find(base_sel).expect("base dropdown-select rule exists");
    let base_start = base_idx + base_sel.len();
    let base_end = css[base_start..].find('}').unwrap() + base_start;
    assert!(
        css[base_start..base_end].contains("min-width: 220px"),
        "base dropdown-select must keep min-width: 220px — the fix is scoped, \
         not global: {}",
        &css[base_start..base_end]
    );

    // Panel-head scope releases it so the head row fits in 360px. The
    // selector carries 3 classes, so it beats the base rule regardless of
    // source order.
    let head_sel = ".surfdoc .surfdoc-panel-head .surfdoc-dropdown-select {";
    let head_idx = css
        .find(head_sel)
        .expect("panel-head dropdown-select override rule exists");
    let head_start = head_idx + head_sel.len();
    let head_end = css[head_start..].find('}').unwrap() + head_start;
    assert!(
        css[head_start..head_end].contains("min-width: 0"),
        "panel-head dropdown-select must release the 220px min-width \
         (min-width: 0) so the drawer head row fits its 360px track: {}",
        &css[head_start..head_end]
    );
}

/// F2 (0.14 drawer polish): split-pane children are flex:1 — with the
/// default min-width:auto their min-content size (long thread lines, the
/// composer input+Send) forces the pane past its track, clipping the
/// roster chevrons at 390 and the thread text + Send at 900 (DOM probe:
/// right pane right edge 1001 > 900, left pane 418 > 390). The classic
/// fix: min-width: 0 on the panes so they shrink to their track and inner
/// content wraps/ellipsizes.
#[test]
fn split_pane_children_shrink_to_their_track() {
    let css = surf_parse::SURFDOC_CSS;
    let sel = ".surfdoc-split-left, .surfdoc-split-right {";
    let idx = css.find(sel).expect("split-pane children rule exists");
    let start = idx + sel.len();
    let end = css[start..].find('}').unwrap() + start;
    let body = &css[start..end];
    assert!(body.contains("flex: 1"), "split children stay flex: 1: {body}");
    assert!(
        body.contains("min-width: 0"),
        "split children need min-width: 0 — flex min-width:auto lets content \
         force the pane past its track and clip at 390/900: {body}"
    );
}

/// F3 (0.14 drawer polish): the tier dropdown OPTIONS popup at panel-head
/// placement needs its own explicit width — the F1 head-scoped min-width
/// release shrank the closed trigger AND left the absolutely-positioned
/// popup to shrink-to-fit its ~91px containing block (the collapsed
/// trigger wrapper), wrapping option descriptions to 11 lines (DOM probe:
/// popup w 91.1, options w 79.1). A min-width floor is NOT enough: the
/// live layer's `.surfdoc.surfdoc-live .surfdoc-dropdown-select.is-open
/// .surfdoc-dropdown-options` rule wins min-width at 5-class specificity
/// (its 100% resolves against the same tiny wrapper), so the crate pins
/// `width` — a property the live layer does not touch. 220px matches the
/// base content dropdown floor, and the popup is positioned, so it cannot
/// inflate the head row (F1 stays intact). Scoped under the panel head:
/// toolbar and in-content options popups stay byte-identical.
#[test]
fn panel_head_dropdown_options_popup_keeps_a_readable_width() {
    let css = surf_parse::SURFDOC_CSS;

    // Head-scoped popup rule pins an explicit width.
    let sel = ".surfdoc .surfdoc-panel-head .surfdoc-dropdown-options {";
    let idx = css.find(sel).expect("panel-head dropdown-options rule exists");
    let start = idx + sel.len();
    let end = css[start..].find('}').unwrap() + start;
    let body = &css[start..end];
    assert!(
        body.contains("width: 220px"),
        "panel-head options popup needs an explicit 220px width — min-width \
         alone loses to the live layer's 5-class min-width:100% and the \
         popup collapses to its ~91px containing block: {body}"
    );
    assert!(
        !body.contains("!important"),
        "panel-head options rule must stay !important-free: {body}"
    );

    // The shared toolbar-dropdown popup rule keeps its geometry (other
    // dropdown sites byte-identical).
    let base_sel = ".surfdoc .surfdoc-toolbar-dropdown .surfdoc-dropdown-options {";
    let base_idx = css.find(base_sel).expect("toolbar-dropdown options rule exists");
    let base_start = base_idx + base_sel.len();
    let base_end = css[base_start..].find('}').unwrap() + base_start;
    assert!(
        css[base_start..base_end].contains("min-width: max(100%, 180px)"),
        "shared toolbar-dropdown options floor must stay untouched — the \
         F3 fix is head-scoped: {}",
        &css[base_start..base_end]
    );
}

/// F4 (0.14 drawer polish): the split right-pane header title (messages
/// thread: "Danny Pappageorge — … · cloudsurf workspace") hard-cuts at
/// 390 full-plane — the toolbar-text span keeps min-width:auto, so it
/// refuses to shrink past its nowrap min-content and overflows the bar
/// (DOM probe: title w 364.8 > bar w 328, text-overflow clip). The fix
/// is the classic ellipsis chain ON THE TITLE element: min-width: 0 so
/// the flex item shrinks to its track, overflow hidden + text-overflow
/// ellipsis + nowrap. Scoped to the split right pane: every other
/// toolbar title keeps its current geometry.
#[test]
fn split_right_toolbar_title_ellipsizes_instead_of_hard_cutting() {
    let css = surf_parse::SURFDOC_CSS;
    let sel = ".surfdoc .surfdoc-split-right .surfdoc-toolbar .surfdoc-toolbar-text {";
    let idx = css.find(sel).expect("split-right toolbar-text rule exists");
    let start = idx + sel.len();
    let end = css[start..].find('}').unwrap() + start;
    let body = &css[start..end];
    for decl in [
        "min-width: 0",
        "overflow: hidden",
        "text-overflow: ellipsis",
        "white-space: nowrap",
    ] {
        assert!(
            body.contains(decl),
            "split-right toolbar title needs `{decl}` in its ellipsis chain: {body}"
        );
    }

    // The base toolbar-text rule stays untouched — other toolbar titles
    // keep their geometry.
    let base_sel = ".surfdoc .surfdoc-toolbar-text {";
    let base_idx = css.find(base_sel).expect("base toolbar-text rule exists");
    let base_start = base_idx + base_sel.len();
    let base_end = css[base_start..].find('}').unwrap() + base_start;
    let base_body = &css[base_start..base_end];
    assert!(
        !base_body.contains("text-overflow") && !base_body.contains("overflow: hidden"),
        "base toolbar-text must NOT gain the ellipsis chain — the F4 fix \
         is split-right-scoped: {base_body}"
    );
}

/// The allowlist stays honest: no entry may shadow a class that has since
/// gained a direct rule.
#[test]
fn css_allowlist_entries_are_still_ruleless() {
    for class in ALLOWLIST {
        assert!(
            !css_has_rule(surf_parse::SURFDOC_CSS, class),
            ".{class} now has a rule in assets/surfdoc.css — remove it from the allowlist"
        );
    }
}

// ------------------------------------------------------------------
// Size-class axis (0.18) — the CSS breakpoints ARE the Rust constants
// ------------------------------------------------------------------

/// The chrome layer may use exactly four breakpoint numbers, and all four
/// are derived from the two exported Rust constants. If someone changes
/// `SIZE_CLASS_TABLET_MIN`/`SIZE_CLASS_DESKTOP_MIN` without editing
/// `assets/surfdoc.css` (or the reverse), this fails.
#[test]
fn chrome_media_query_numbers_equal_the_exported_rust_consts() {
    let css = surf_parse::SURFDOC_CSS;
    let tablet_min = surf_parse::SIZE_CLASS_TABLET_MIN;
    let desktop_min = surf_parse::SIZE_CLASS_DESKTOP_MIN;
    assert_eq!(tablet_min, 768, "tablet breakpoint moved — update the CSS too");
    assert_eq!(desktop_min, 1024, "desktop breakpoint moved — update the CSS too");

    // The four chrome preludes, each spelled from the constants.
    for prelude in [
        format!("max-width: {}px", tablet_min - 1),
        format!("min-width: {tablet_min}px"),
        format!("max-width: {}px", desktop_min - 1),
        format!("min-width: {desktop_min}px"),
    ] {
        assert!(
            css.contains(&format!("@media ({prelude})"))
                || css.contains(&format!("@media ({prelude}) and")),
            "assets/surfdoc.css is missing a chrome media query for {prelude:?}"
        );
    }
}

/// Every media block whose prelude contains `needle`, concatenated. The
/// 0.18 axis reuses preludes that older sections already spell, so the
/// axis tests must look at ALL matching blocks, not just the first.
fn media_blocks_all(css: &str, needle: &str) -> String {
    let mut out = String::new();
    let mut from = 0;
    while let Some(pos) = css[from..].find("@media") {
        let abs = from + pos;
        let brace = css[abs..].find('{').expect("media block opens") + abs;
        if css[abs..brace].contains(needle) {
            let mut depth = 0i64;
            for (i, c) in css[brace..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            out.push_str(&css[brace + 1..brace + i]);
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        from = brace;
    }
    assert!(!out.is_empty(), "no media query with prelude containing {needle:?}");
    out
}

/// The Section 80 axis block selects the per-class width custom properties
/// at each boundary, and never writes an inline-losing bare shorthand at
/// the mobile tier only.
#[test]
fn size_class_width_vars_are_selected_at_both_boundaries() {
    let css = surf_parse::SURFDOC_CSS;
    for var in ["--sc-w-mobile", "--sc-w-tablet", "--sc-w-desktop"] {
        assert!(
            css.contains(&format!("var({var})")),
            "assets/surfdoc.css must consume {var}"
        );
    }
    let tablet = media_blocks_all(css, "min-width: 768px");
    assert!(
        tablet.contains("--sc-w-tablet"),
        "the 768px block must switch the per-class width to the tablet value"
    );
    let desktop = media_blocks_all(css, "min-width: 1024px");
    assert!(
        desktop.contains("--sc-w-desktop"),
        "the 1024px block must switch the per-class width to the desktop value"
    );
}

/// The class-conditional attributes actually gate something in every tier.
#[test]
fn class_conditional_attrs_are_gated_in_every_tier() {
    let css = surf_parse::SURFDOC_CSS;
    for (prelude, needle) in [
        ("max-width: 767px", "data-size-class~=\"mobile\""),
        ("min-width: 768px) and (max-width: 1023px", "data-size-class~=\"tablet\""),
        ("min-width: 1024px", "data-size-class~=\"desktop\""),
    ] {
        let body = media_blocks_all(css, prelude);
        assert!(
            body.contains(needle),
            "the {prelude} block must gate {needle}"
        );
    }
    assert!(
        css.contains("data-min-size-class"),
        "min-class= needs a gating rule"
    );
}

/// The content-block breakpoints are NOT part of this axis and must not
/// have been rewritten by the 0.18 freeze.
#[test]
fn content_block_breakpoints_are_untouched_by_the_size_class_axis() {
    let css = surf_parse::SURFDOC_CSS;
    for n in ["max-width: 480px", "max-width: 640px", "max-width: 720px"] {
        assert!(
            css.contains(n),
            "content-block query {n} disappeared — the 0.18 axis must not \
             reopen the visual canon"
        );
    }
}

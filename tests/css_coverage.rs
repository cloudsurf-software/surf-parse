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
];

/// One minimal source document per implemented registry kind
/// (spec/blocks.toml, status = "implemented"; registry currently has 98
/// implemented of 111 total). When a kind is added to the registry, the
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
    ("row", "::row[icon=doc href=\"/docs\" unread=true trailing-label=\"Open\" trailing-action=open]\nTitle\nDescription\naction: Accept | invoke:contacts.accept\n::"),
    ("infocard", "::infocard[intent=success image=\"/img/a.png\"]\n# Card\nSubtitle\n\nSummary text.\n\n1. Step one\n\nVersion: 1.0\n::"),
    ("app-shell", "::app-shell[layout=sidebar-main-panel height=600]\n::"),
    ("sidebar", "::sidebar[position=left collapsible=true width=240]\n::"),
    ("panel", "::panel[position=bottom resizable=true height=160 desktop-only=true]\n::"),
    ("tab-bar", "::tab-bar[active=preview]\n- preview \"Preview\" {icon=eye unread=true}\n- edit \"Edit\"\n::"),
    ("tab-content", "::tab-content[tab=preview width=880 align=center]\nPane\n::"),
    ("toolbar", "::toolbar[title=\"Messages\" title-source=thread.display_name]\n- button[label=\"Run\" action=run style=primary toggled=true]\n- text[value=\"Surfspace\" size=22]\n- separator\n- spacer\n- badge[value=\"Live\" color=green]\n- dropdown[options=\"A|B\"]\n::"),
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
    ("chat-thread", "::chat-thread[source=chat.thread on-action=run_action]\n::"),
    ("chat-input-simple", "::chat-input-simple[placeholder=\"Ask…\" action=send]\n::"),
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

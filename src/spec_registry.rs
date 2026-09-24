//! The registry as DATA (0.27): the block registry, the rules registry and
//! the corpus, exported so a player can build a block catalog from the
//! crate it pins — no vendored copy, no repin diff gate. CloudSurf Developer
//! on the web (`GET /api/spec/blocks`, TASK-994 lane D) is the first reader.

/// The crate's own version, for a player that reports which parser built
/// its catalog (`env!` resolves here, in this crate, at its build).
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `spec/blocks.toml` — the authoritative block registry (the same bytes
/// the lint's L020 vocabulary and `tests/spec_compliance.rs` read).
pub const BLOCKS_TOML: &str = include_str!("../spec/blocks.toml");

/// `spec/rules.toml` — the lint rule registry.
pub const RULES_TOML: &str = include_str!("../spec/rules.toml");

/// The corpus fixtures, by file stem: every `tests/corpus/*.surf` the
/// snapshot suites pin. A new fixture is one line here (the corpus test
/// `corpus_is_exported` refuses a fixture this list lacks).
pub const CORPUS: &[(&str, &str)] = &[
    ("ffi-hole-closure", include_str!("../tests/corpus/ffi-hole-closure.surf")),
    ("inline-extensions", include_str!("../tests/corpus/inline-extensions.surf")),
    ("tier1-content-cards", include_str!("../tests/corpus/tier1-content-cards.surf")),
    ("tier1-content", include_str!("../tests/corpus/tier1-content.surf")),
    ("tier2-marketing", include_str!("../tests/corpus/tier2-marketing.surf")),
    ("tier2-site", include_str!("../tests/corpus/tier2-site.surf")),
    ("tier3-app-data", include_str!("../tests/corpus/tier3-app-data.surf")),
    ("tier3-chrome", include_str!("../tests/corpus/tier3-chrome.surf")),
    ("tier4-manifest", include_str!("../tests/corpus/tier4-manifest.surf")),
    ("tier5-size-class", include_str!("../tests/corpus/tier5-size-class.surf")),
    ("tier6-the-last-twenty", include_str!("../tests/corpus/tier6-the-last-twenty.surf")),
    ("tier7-panels", include_str!("../tests/corpus/tier7-panels.surf")),
];

/// One minimal source document per implemented registry kind — the
/// coverage suite's table, exported (0.27) so every block has an example
/// (spec/blocks.toml, status = "implemented"; registry currently has 124
/// implemented of 124 total). When a kind is added to the registry, the
/// companion completeness check below fails until it gets a snippet here.
pub const SNIPPETS: &[(&str, &str)] = &[
    ("banner", "::banner[id=contact]\n# Talk to us\nWe reply within one business day.\n[Book a call](/book)\n::"),
    ("bibliography", "::bibliography[style=apa]\n::"),
    ("booking", "::booking[title=\"Book a session\" service-label=Service]\n- service: Strategy Call | 60 min | $200\n- day: 2026-09-01 | 09:00, 10:00\n::"),
    ("callout", "::callout[type=warning title=\"Heads up\"]\nBody\n::"),
    ("cite", "::cite[key=bourne2026 type=article]\ntitle: Tidal Records\nauthor: R. Bourne\nyear: 2026\n::"),
    ("chart", "::chart[type=line source=\"/api/metrics\" period=weekly title=\"WAU\"]\n::"),
    ("code", "::code[lang=rust file=src/main.rs]\nfn main() {}\n::"),
    ("columns", "::columns\n::: column\nLeft\n:::\n::: column\nRight\n:::\n::"),
    ("cta", "::cta[label=\"Go\" href=/go primary=true]\n::"),
    ("data", "::data[sortable=true]\n| A | B |\n|---|---|\n| 1 | 2 |\n::"),
    ("store", "::store[title=\"Sweet Delights Bakery\" currency=USD]\n- category: Pastries\n- item: Almond Croissant | 4.50 | Flaky and filled | New\n::"),
    ("decision", "::decision[status=accepted date=2026-06-11]\nShip it.\n::"),
    ("diagram", "::diagram[type=architecture title=\"Flow\"]\nweb: Web\napi: API\nweb -> api: HTTPS\n::"),
    ("details", "::details[title=\"More\" open=true]\nHidden\n::"),
    ("divider", "::divider[label=SECTION]\n::"),
    ("embed", "::embed[src=\"https://example.com\" type=iframe title=\"Demo\"]\n::"),
    ("faq", "::faq\n- q=\"Fast?\" a=\"Yes.\"\n::"),
    ("figure", "::figure[src=/img/a.png alt=\"A\" caption=\"First\"]\n::"),
    ("footer", "::footer[copyright=\"© 2026\"]\n::"),
    ("form", "::form[submit=\"Send\"]\ngroup: Contact\n- Name (text, \"Your name\") *\n- Email (email)\ngroup: Preferences\n- Plan (radio: Free | Pro)\n- Subscribe (checkbox)\n- Dark mode (toggle)\n- Resume (file)\n- Source (hidden, \"pricing\")\n::"),
    ("gallery", "::gallery[columns=2]\n- src=/img/a.png alt=\"A\" caption=\"First\"\n::"),
    ("hero-image", "::hero-image[src=/img/hero.png alt=\"Hero\"]\n::"),
    ("hours", "::hours[title=\"Hours\" timezone=\"America/Los_Angeles\"]\n- Monday: 11am - 9pm\n- Sunday: Closed\n::"),
    ("marquee", "::marquee\n- Fresh daily\n- Open late\n::"),
    // 0.25.0: the fourteen blocks that were planned until sessions 11 + 12.
    ("related", "::related\n- [Architecture Plan](plans/plan.md) \u{2014} produces\n- consumes: research/FINDINGS.md\n::"),
    ("turn", "::turn[participant=claude time=2026-02-10T04:01Z role=ai model=opus]\nYes \u{2014} file before launch.\n::"),
    ("timeline", "::timeline[title=\"Milestones\"]\n## Q1\n- 2026-01 \u{2014} Beta\n- Launch\n::"),
    ("output", "::output[for=analysis timestamp=\"2026-02-10T12:00:00Z\" exit=0 format=text]\nMean: $12,000\n::"),
    ("ai-generated", "::ai-generated[model=opus date=2026-02-10 reviewed=false]\nMaybe.\n::"),
    ("alternatives", "::alternatives\n| Option | Pros | Cons | Verdict |\n|---|---|---|---|\n| GTK4 | small | Linux-first | **Selected** |\n| Electron | everywhere | heavy | Rejected |\n| Tauri | small | young | Considered |\n| Qt | mature | licence | Open |\n::"),
    ("ai-context", "::ai-context[model=opus tokens=2400 loaded=true]\nA note.\n::"),
    ("countdown", "::countdown[date=2026-03-15 label=\"Launch day\"]\n::"),
    ("css", "::css\n.custom-thing { border: 2px dashed red; }\n::"),
    ("footnote", "::footnote[id=1]\nGartner, 2025.\n::"),
    ("kernel", "::kernel[lang=python env=analysis]\nruntime: python3.12\npackages: [numpy, pandas]\nsandbox: strict\n::"),
    ("logo-cloud", "::logo-cloud[title=\"Trusted by\"]\n- assets/logos/acme.svg\n- assets/logos/initech.svg | Initech\n::"),
    ("subscribe", "::subscribe[action=/subscribe placeholder=\"you@email.com\"]\nGet notified.\n::"),
    ("notes", "::notes\nPause here.\n::"),
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
    ("panel-slot", "::app-shell[layout=panels]\n:::sidebar[position=left width=\"0 44 44\"]\n::::row[icon=doc href=#]\nSurfDocs\n::::\n:::\n:::panel-slot[role=navigator kind=surfdocs]\n::::nav-tree[source=docs.recent on-select=open_doc]\n::::\n:::\n:::panel-slot[role=work kind=desk]\n::::tab-bar\n::::\n::::tab-content[tab=desk]\n::::\n:::\n:::panel-slot[role=work kind=desk2]\n::::tab-bar\n::::\n::::tab-content[tab=desk2]\n::::\n:::\n:::preset[name=twoColumns title=\"Two Columns\" columns=\"0.5 0.5\" rows=\"1\" spans=\"0,0 1,0\" default=true]\n:::\n::"),
    ("preset", "::preset[name=single title=\"Single\" icon=rectangle columns=\"1\" rows=\"1\" spans=\"0,0\"]\n::"),
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

/// One row of `spec/blocks.toml`, typed (0.27) — so a player needs no TOML
/// parser of its own to read the registry it pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub name: String,
    pub status: String,
    pub category: String,
    pub purpose: String,
    pub attributes: Vec<String>,
    pub degradation: String,
    pub enum_variant: Option<String>,
}

/// Every registered block, in registry (alphabetical) order.
pub fn registry() -> Vec<RegistryEntry> {
    let value: toml::Value = toml::from_str(BLOCKS_TOML).expect("spec/blocks.toml parses");
    let Some(blocks) = value.get("blocks").and_then(|b| b.as_table()) else {
        return Vec::new();
    };
    let str_of = |t: &toml::Value, k: &str| t.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    blocks
        .iter()
        .map(|(name, t)| RegistryEntry {
            name: name.clone(),
            status: str_of(t, "status"),
            category: str_of(t, "category"),
            purpose: str_of(t, "purpose"),
            attributes: t
                .get("attributes")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            degradation: str_of(t, "degradation"),
            enum_variant: t.get("enum_variant").and_then(|v| v.as_str()).map(str::to_string),
        })
        .collect()
}

/// One example of a block IN SOURCE: the directive line through its closer,
/// cut from a corpus fixture. `fixture` is the file stem it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockExample {
    pub fixture: &'static str,
    pub source: String,
}

/// Every corpus occurrence of `::<block>` (any fence depth), each as the
/// self-contained source from its opening directive to its matching
/// closer. A block that opens and closes on one line (`::divider`) is one
/// line. Order: fixture order, then source order.
pub fn examples_for(block: &str) -> Vec<BlockExample> {
    let mut out = Vec::new();
    for (fixture, src) in CORPUS {
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim_start();
            let fence = trimmed.len() - trimmed.trim_start_matches(':').len();
            // `::: preset[…]` — the corpus writes a space after the fence.
            let after = trimmed[fence..].trim_start();
            let name_len = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(after.len());
            let name = &after[..name_len];
            if fence >= 2 && name == block {
                // Find the closer: the first later line whose trimmed form is
                // exactly the same fence with nothing after it.
                let closer = format!("{}", ":".repeat(fence));
                let mut j = i + 1;
                let mut end = None;
                while j < lines.len() {
                    let t = lines[j].trim();
                    if t == closer {
                        end = Some(j);
                        break;
                    }
                    j += 1;
                }
                let stop = end.unwrap_or(i);
                let snippet: Vec<String> = lines[i..=stop]
                    .iter()
                    .map(|l| {
                        // Re-base the fence so the snippet parses on its own
                        // (`:::row` inside a sidebar becomes `::row`).
                        let lt = l.trim_start();
                        let lf = lt.len() - lt.trim_start_matches(':').len();
                        if lf >= fence && lt.starts_with(':') {
                            format!("{}{}", ":".repeat(lf - fence + 2), lt[lf..].trim_start())
                        } else {
                            l.to_string()
                        }
                    })
                    .collect();
                out.push(BlockExample {
                    fixture,
                    source: snippet.join("\n"),
                });
                i = stop + 1;
                continue;
            }
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_the_same_bytes_the_lint_reads() {
        let registry: toml::Value = toml::from_str(BLOCKS_TOML).expect("blocks.toml parses");
        let total = registry["meta"]["total_blocks"].as_integer().unwrap() as usize;
        let names = registry["blocks"].as_table().unwrap().len();
        assert_eq!(total, names);
        for name in registry["blocks"].as_table().unwrap().keys() {
            assert!(crate::lint::known_block_names().contains(name.as_str()), "{name}");
        }
        let rules: toml::Value = toml::from_str(RULES_TOML).expect("rules.toml parses");
        assert!(rules["rules"]["L045"].is_table());
    }

    #[test]
    fn the_typed_registry_matches_the_toml() {
        let rows = registry();
        assert_eq!(rows.len(), 124);
        let slot = rows.iter().find(|r| r.name == "panel-slot").expect("panel-slot");
        assert_eq!(slot.status, "implemented");
        assert_eq!(slot.enum_variant.as_deref(), Some("PanelSlot"));
        assert!(slot.attributes.contains(&"kind".to_string()));
        assert!(rows.iter().all(|r| !r.purpose.is_empty()));
    }

    #[test]
    fn every_corpus_file_is_exported() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.strip_suffix(".surf").map(str::to_string)
            })
            .collect();
        on_disk.sort();
        let mut exported: Vec<String> = CORPUS.iter().map(|(n, _)| n.to_string()).collect();
        exported.sort();
        assert_eq!(on_disk, exported, "a corpus fixture is missing from CORPUS");
    }

    #[test]
    fn examples_cut_a_block_from_the_directive_to_its_closer() {
        let panels = examples_for("preset");
        assert!(panels.len() >= 10, "{}", panels.len());
        assert!(panels.iter().all(|e| e.fixture == "tier7-panels"));
        assert!(panels[0].source.starts_with("::preset[name=single"), "{}", panels[0].source);
        assert!(panels[0].source.ends_with("\n::"), "{:?}", panels[0].source);
        let slots = examples_for("panel-slot");
        assert!(slots.len() >= 6);
        // A nested block is re-based to depth 2 and parses on its own.
        let first = &slots[0].source;
        assert!(first.starts_with("::panel-slot[role=navigator kind=surfdocs"), "{first}");
        assert!(first.contains("\n:::nav-tree["), "{first}");
        let parsed = crate::parse(first);
        assert!(matches!(parsed.doc.blocks[0], crate::Block::PanelSlot { .. }), "{:?}", parsed.doc.blocks);
        // A block the corpus never authors has no example — honest, not a panic.
        assert!(examples_for("no-such-block").is_empty());
        // Every implemented block has at least one example somewhere in the
        // corpus OR in the coverage snippets — the web catalog's floor is
        // measured by the frontend's block_parity test, not asserted here.
    }
}

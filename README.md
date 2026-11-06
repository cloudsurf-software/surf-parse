# surf-parse

The **SurfDoc reference implementation**: parser and renderers for SurfDoc, a plain-text document format whose typed `::` blocks a parser validates. One `.surf` file renders to HTML, PDF, slides, the terminal, and a native block tree that Swift and Kotlin receive through UniFFI.

SurfDoc uses `::directive` blocks to represent data tables, callouts, decisions, metrics, tasks, code, figures, FAQ, pricing tables, landing page sections, and full multi-page site structures. Every block is typed, validated, and renderable to HTML, LaTeX, Typst, PDF, slides, ANSI terminal output, or a native block tree exported through UniFFI to Swift and Kotlin. A `.surf` file without a renderer is still readable plain text.

This crate is the open half of an open-core split (since 0.10.0): it contains everything needed to parse, validate, lint, and render `.surf` documents — the complete format surface. Application-platform tooling built on top of the format (e.g. compiling an app description document into a running service) is out of scope here and not part of this crate's API.

## Usage

```rust
let result = surf_parse::parse("# Hello\n\n::callout[type=tip]\nThis is a tip.\n::\n");

// Render to HTML
let config = surf_parse::PageConfig::default();
let html = result.doc.to_html_page(&config);

// Render as an email (0.43.0): the HTML and text parts of one message
let footer = surf_parse::EmailFooter {
    postal_address: "1 Main St, Reno NV".into(),
    unsubscribe_url: "https://app.cloudsurf.com/n/u/token".into(),
    sender_name: Some("The Letter".into()),
    sent_with_cloudsurf: false,
};
let email = result.doc.to_email(&surf_parse::EmailProfile::new("https://example.com", footer));
assert!(email.html.contains("Unsubscribe") && email.text.contains("Unsubscribe"));
```

## Block Types (131 registered · 131 implemented)

The registry (`spec/blocks.toml`) is the authority: 131 directives, every one
implemented since 0.25.0 (130 `Block` variants — `::split-pane` and `::pane`
share one; `::panel-slot` and `::preset` joined in 0.27.0 with the `panels`
app-shell layout). The implemented variants:

**Core**: Callout, Code, Data, Decision, Details, Diagram, Figure, Footnote, Metric, Notes, Quote, Related, Summary, Tasks, Turn, Video
**Citations**: Cite, Bibliography
**Layout**: Columns, Divider, Section, Tabs
**Web**: Banner, BeforeAfter, Carousel, Comparison, Countdown, Cta, Embed, Faq, Features, Footer, Form, Gallery, Gate, Hero, HeroImage, Hours, Logo, LogoCloud, Marquee, Nav, Pipeline, PostGrid, PricingTable, ProductCard, ProductGrid, Site, Page, Stats, Steps, Style, Subscribe, Testimonial, Toc
**Data / annotations**: Alternatives, Timeline, Output, Kernel, AiGenerated, AiContext, Css
**App Description**: Action, Board, Booking, ChatInput, Dashboard, Feed, FilterBar, List, Search, Store
**Compound Widgets**: Chart, Editor, SplitPane
**Infrastructure Manifest**: App, Auth, Binding, Build, Cicd, Concurrency, Crates, Deploy, DeployUrls, Domains, Health, InfraDatabase, InfraEnv, Model, Route, Schema, Smoke, Use, Volumes
**App Format**: AppDeploy, AppEnv
**Stateful App (0.33.0)**: Compute, Flow, Picker, Schedule, When
**Compact Display**: Badge, InfoCard, Row
**App Shell / Interactive**: AppShell, BlockEditor, ChatInputSimple, ChatThread, ChipInput, CodeEditor, CommandPalette, Drawer, DropdownSelect, LogStream, Modal, NavTree, Panel, ProblemList, Progress, SegmentedControl, Sidebar, SuggestionChips, TabBar, TabContent, Terminal, Toolbar
**Messages / Contacts**: RecipientPicker, Qr
**Passthrough**: Unknown (unrecognized directives preserved)

## Features

- Inline text rendering: emphasis, links, inline code, lists and headings
- YAML front matter parsing
- 122 implemented block directives (every registered one) with attribute parsing
- Renderers: HTML (with embedded CSS), plain-text degradation, LaTeX, Typst, ANSI terminal, slides, native block tree (feature `native`), PDF (feature `pdf`)
- Email (0.43.0): `to_email(&doc, &EmailProfile)` returns an email's HTML part (inline styles from the house tokens, a 600 px table layout, absolute URLs, no scripts or forms, a caller-filled footer) and its text part
- Bindings: UniFFI (feature `uniffi`) and WASM (feature `wasm`)
- `surf-lint` CLI (feature `cli`) — format/style lint with auto-fixes
- SurfDocBuilder for programmatic document construction
- Round-trip serialization via `to_surf_source()`
- Structural validation diagnostics
- Multi-page site generation (`::site` + `::page` blocks)
- 20 built-in SVG icons (Lucide-based)
- 6 font presets with Google Fonts auto-import

## License

MIT

## Editor Support

- **VS Code**: [SurfDoc extension](https://marketplace.visualstudio.com/items?itemName=CloudSurf.surfdoc) — syntax highlighting, 49 snippets, file icons. Install: `ext install CloudSurf.surfdoc`

## Links

- [SurfDoc Specification](https://doc.surf/spec)
- [Repository](https://github.com/cloudsurf-software/surf-parse)

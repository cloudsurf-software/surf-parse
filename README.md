# surf-parse

The **SurfDoc reference implementation** — parser and renderers for the SurfDoc format, a typed document format with block directives for structured documents. Backward-compatible with Markdown.

SurfDoc uses `::directive` blocks to represent data tables, callouts, decisions, metrics, tasks, code, figures, FAQ, pricing tables, landing page sections, and full multi-page site structures. Every block is typed, validated, and renderable to HTML, markdown, LaTeX, Typst, PDF, slides, native block trees, or ANSI terminal output. A `.surf` file without a renderer is still readable plain text.

This crate is the open half of an open-core split (since 0.10.0): it contains everything needed to parse, validate, lint, and render `.surf` documents — the complete format surface. Application-platform tooling built on top of the format (e.g. compiling an app description document into a running service) is out of scope here and not part of this crate's API.

## Usage

```rust
let result = surf_parse::parse("# Hello\n\n::callout[type=tip]\nThis is a tip.\n::\n");

// Render to HTML
let config = surf_parse::PageConfig::default();
let html = result.doc.to_html_page(&config);
```

## Block Types (120 registered · 106 implemented)

The registry (`spec/blocks.toml`) is the authority: 120 directives, 106 of them
implemented (105 `Block` variants — `::split-pane` and `::pane` share one), 14
still planned. The implemented variants:

**Core**: Callout, Code, Data, Decision, Details, Diagram, Figure, Metric, Quote, Summary, Tasks
**Citations**: Cite, Bibliography
**Layout**: Columns, Divider, Section, Tabs
**Web**: Banner, BeforeAfter, Comparison, Cta, Embed, Faq, Features, Footer, Form, Gallery, Gate, Hero, HeroImage, Logo, Nav, Pipeline, PostGrid, PricingTable, ProductCard, ProductGrid, Site, Page, Stats, Steps, Style, Testimonial, Toc
**App Description**: Action, Board, Booking, ChatInput, Dashboard, Feed, FilterBar, List, Search, Store
**Compound Widgets**: Chart, Editor, SplitPane
**Infrastructure Manifest**: App, Auth, Binding, Build, Cicd, Concurrency, Crates, Deploy, DeployUrls, Domains, Health, InfraDatabase, InfraEnv, Model, Route, Schema, Smoke, Use, Volumes
**App Format**: AppDeploy, AppEnv
**Compact Display**: Badge, InfoCard, Row
**App Shell / Interactive**: AppShell, BlockEditor, ChatInputSimple, ChatThread, ChipInput, CodeEditor, CommandPalette, Drawer, DropdownSelect, LogStream, Modal, NavTree, Panel, ProblemList, Progress, SegmentedControl, Sidebar, SuggestionChips, TabBar, TabContent, Terminal, Toolbar
**Messages / Contacts**: RecipientPicker, Qr
**Passthrough**: Unknown (unrecognized directives preserved)

## Features

- CommonMark-compatible inline text rendering
- YAML front matter parsing
- 106 implemented block directives (of 120 registered) with attribute parsing
- Renderers: HTML (with embedded CSS), markdown degradation, LaTeX, Typst, ANSI terminal, slides, native block tree (feature `native`), PDF (feature `pdf`)
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

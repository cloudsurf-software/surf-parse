# surf-parse

Parser for the **SurfDoc** format — a typed document format with block directives for structured documents. Backward-compatible with Markdown.

SurfDoc uses `::directive` blocks to represent data tables, callouts, decisions, metrics, tasks, code, figures, FAQ, pricing tables, landing page sections, and full multi-page site structures. Every block is typed, validated, and renderable to HTML, markdown, or ANSI terminal output. A `.surf` file without a renderer is still readable plain text.

## Usage

```rust
let result = surf_parse::parse("# Hello\n\n::callout[type=tip]\nThis is a tip.\n::\n");

// Render to HTML
let config = surf_parse::PageConfig::default();
let html = result.doc.to_html_page(&config);
```

## Block Types (37)

**Core**: Callout, Code, Data, Decision, Details, Figure, Metric, Quote, Summary, Tasks
**Layout**: Columns, Divider, Section, Tabs
**Web**: Cta, Embed, Faq, Footer, Form, Gallery, HeroImage, Nav, PricingTable, Site, Page, Style, Testimonial
**Landing Page**: BeforeAfter, Comparison, Features, Hero, Logo, Pipeline, ProductCard, Stats, Steps, Toc
**Passthrough**: Unknown (unrecognized directives preserved)

## Features

- CommonMark-compatible inline text rendering
- YAML front matter parsing
- 37 typed block directives with attribute parsing
- 4 renderers: HTML (with embedded CSS), markdown degradation, ANSI terminal, PDF (feature-gated)
- SurfDocBuilder for programmatic document construction
- Round-trip serialization via `to_surf_source()`
- Validation with 21 diagnostic codes
- Multi-page site generation (`::site` + `::page` blocks)
- 20 built-in SVG icons (Lucide-based)
- 6 font presets with Google Fonts auto-import

## License

MIT

## Links

- [SurfDoc Specification](https://surfcontext.org)
- [Repository](https://github.com/cloudsurf-software/surf-parse)

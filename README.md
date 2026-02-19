# surf-parse

Parser for the **SurfDoc** format — a markdown superset with typed block directives for structured documents.

SurfDoc extends CommonMark with `::directive` blocks that represent data tables, callouts, decisions, metrics, tasks, code, figures, FAQ, pricing tables, landing page sections, and full multi-page site structures. Every block is typed, validated, and renderable to HTML, markdown, or ANSI terminal output.

## Usage

```rust
let result = surf_parse::parse("# Hello\n\n::callout[type=tip]\nThis is a tip.\n::\n");

// Render to HTML
let config = surf_parse::PageConfig::default();
let html = result.doc.to_html_page(&config);
```

## Block Types (35)

**Core**: Callout, Data, Code, Tasks, Decision, Metric, Summary, Figure
**Container**: Columns, Tabs
**Web**: Cta, HeroImage, Testimonial, Style, Faq, PricingTable, Site, Page, Nav, Embed, Form, Gallery, Footer
**Landing Page**: Hero, Features, Steps, Stats, Comparison, Logo, Toc
**Passthrough**: Unknown (unrecognized directives preserved)

## Features

- Full CommonMark support via pulldown-cmark
- YAML front matter parsing
- 35 typed block directives with attribute parsing
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

# surf-parse

Parser for the **SurfDoc** format — a markdown superset with typed block directives for structured documents.

SurfDoc extends CommonMark with `::directive` blocks that represent data tables, callouts, decisions, metrics, tasks, code, figures, FAQ, pricing tables, and full multi-page site structures. Every block is typed, validated, and renderable to HTML, markdown, or ANSI terminal output.

## Usage

```rust
let result = surf_parse::parse("# Hello\n\n::callout[type=tip]\nThis is a tip.\n::\n");

// Render to HTML
let config = surf_parse::PageConfig::default();
let html = result.doc.to_html_page(&config);
```

## Block Types (19)

**Core**: Callout, Data, Code, Tasks, Decision, Metric, Summary, Figure
**Container**: Columns, Tabs
**Web**: Cta, HeroImage, Testimonial, Style, Faq, PricingTable, Site, Page
**Passthrough**: Unknown (unrecognized directives preserved)

## Features

- Full CommonMark support via pulldown-cmark
- YAML front matter parsing
- 19 typed block directives with attribute parsing
- 3 renderers: HTML (with embedded CSS), markdown degradation, ANSI terminal
- Validation with 19 diagnostic codes
- Multi-page site generation (`::site` + `::page` blocks)

## License

MIT

## Links

- [SurfDoc Specification](https://surfcontext.org)
- [Repository](https://github.com/cloudsurf-software/surfcontext-cli)

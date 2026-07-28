//! WebAssembly bindings for client-side SurfDoc rendering.
//!
//! Exposes `render_surfdoc` and `parse_metadata` to JavaScript via wasm-bindgen.
//! Build with: `wasm-pack build --target web --features wasm --no-default-features`

use wasm_bindgen::prelude::*;

/// Render a SurfDoc source string to an HTML fragment.
///
/// Returns semantic HTML with `surfdoc-*` CSS classes, suitable for
/// injecting into a page that already loads surfdoc.css.
#[wasm_bindgen]
pub fn render_surfdoc(source: &str) -> String {
    let result = crate::parse::parse(source);
    result.doc.to_html_fragment()
}

/// Render a SurfDoc source string to a full HTML document (with page chrome).
///
/// Uses `to_html_page`, which embeds the full SURFDOC_CSS stylesheet so the
/// output is self-contained and styled. (`to_html` only emits body markup with
/// `surfdoc-*` classes and no stylesheet — that left the full render unstyled.)
#[wasm_bindgen]
pub fn render_surfdoc_full(source: &str) -> String {
    let result = crate::parse::parse(source);
    result.doc.to_html_page(&crate::PageConfig::default())
}

/// Parse a SurfDoc source string and return metadata as JSON.
///
/// Returns: `{"title": "...", "summary": "...", "doc_type": "...", "tags": [...]}`
#[wasm_bindgen]
pub fn parse_metadata(source: &str) -> String {
    let result = crate::parse::parse(source);

    let title = result
        .doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm.title.clone())
        .unwrap_or_default();

    // Extract summary from the first Summary block.
    let summary = result
        .doc
        .blocks
        .iter()
        .find_map(|b| {
            if let crate::types::Block::Summary { content, .. } = b {
                Some(content.clone())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let doc_type = result
        .doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm.doc_type.as_ref())
        .map(|dt| format!("{dt:?}").to_lowercase())
        .unwrap_or_default();

    let tags: Vec<String> = result
        .doc
        .front_matter
        .as_ref()
        .and_then(|fm| fm.tags.clone())
        .unwrap_or_default();

    serde_json::json!({
        "title": title,
        "summary": summary,
        "doc_type": doc_type,
        "tags": tags,
    })
    .to_string()
}

/// Run the full check pipeline (parse + validate + lint) over a SurfDoc
/// source string and return the surf-lint JSON envelope.
///
/// Same schema (`schema_version` 1) and construction as
/// `surf-lint check --format json` — both serialize
/// `surf_parse::lint::reports_to_json`. The per-file `path` key is omitted
/// because the input is in-memory. Returns:
///
/// `{"schema_version":1,"files":[{"diagnostics":[…],"error_count":N,
/// "warning_count":N,"info_count":N,"fixable_count":N}],"summary":{…}}`
///
/// Diagnostics carry spans (line numbers + byte offsets into the
/// CRLF-normalised source) and machine-applicable fix payloads when present.
#[wasm_bindgen]
pub fn check_json(input: &str) -> String {
    let report = crate::lint::check(input);
    crate::lint::reports_to_json(&[(None, &report)]).to_string()
}

/// Apply deterministic auto-fixes to a SurfDoc source string and return the
/// fixed source.
///
/// `include_suggested = false` applies only `Safe`-tier fixes;
/// `true` also applies `Suggested`-tier fixes (which includes all Safe
/// fixes). Iterates fix → re-check to a fixpoint (max 10 passes). CRLF input
/// is normalised to LF, matching `apply_fixes` / `parse`. Input with no
/// applicable fixes is returned unchanged (modulo that normalisation).
#[wasm_bindgen]
pub fn fix_source(input: &str, include_suggested: bool) -> String {
    let safety = if include_suggested {
        crate::error::FixSafety::Suggested
    } else {
        crate::error::FixSafety::Safe
    };
    crate::lint::apply_fixes(input, safety).source
}

/// Parse a SurfDoc source string and return any diagnostics as JSON.
///
/// Returns: `[{"severity": "warning", "message": "...", "line": 5}, ...]`
#[wasm_bindgen]
pub fn parse_diagnostics(source: &str) -> String {
    let result = crate::parse::parse(source);

    let diags: Vec<serde_json::Value> = result
        .diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "severity": format!("{:?}", d.severity).to_lowercase(),
                "message": d.message,
                "line": d.span.as_ref().map(|s| s.start_line),
            })
        })
        .collect();

    serde_json::Value::Array(diags).to_string()
}

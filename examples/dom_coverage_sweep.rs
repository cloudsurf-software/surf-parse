//! `dom_coverage_sweep` — measure the constructive DOM renderer's coverage
//! (TASK-1039 L4 / lane R, 2026-09-25): which registry block kinds and which
//! real documents the zero-sink `render_dom` path declines, and why.
//!
//! Two sweeps, both against `check_coverage` (the takeover gate the wasm's
//! `coverage_check_doc` / `render_doc` call):
//!
//! 1. **The registry** — every `spec/blocks.toml` kind through its coverage
//!    snippet (`spec_registry::SNIPPETS`) and its corpus examples
//!    (`examples_for`): a kind is COVERED when every one of its examples
//!    renders. Prints `covered/total` and the uncovered kinds with the first
//!    decline reason each.
//! 2. **A corpus of `.surf` files** — every path given on the command line is
//!    walked (a directory recursively, a file as itself); each document is
//!    parsed and gated; the decline REASONS are counted (`markdown:blockquote`
//!    · `steps` · `script-emitting:tab-bar` …) so the twins can land in the
//!    order the documents need them. Prints `rendered/total` and the histogram.
//!
//! ```text
//! cargo run --example dom_coverage_sweep --features dom -- ~/dev/corpus/plans ~/dev/corpus/docs
//! ```
//!
//! Read-only; never writes. The verdict line is the last line:
//! `SWEEP kinds <covered>/<total> docs <rendered>/<total>`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use surf_parse::render_dom::{check_coverage, RenderDomError};
use surf_parse::spec_registry::{examples_for, registry, SNIPPETS};

fn reason(err: &RenderDomError) -> String {
    match err {
        RenderDomError::Unimplemented(what) => what.clone(),
        RenderDomError::LimitExceeded(e) => format!("limit:{e}"),
    }
}

fn gate(source: &str) -> Result<(), String> {
    let doc = surf_parse::parse(source).doc;
    check_coverage(&doc).map_err(|e| reason(&e))
}

fn walk(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else { return };
        let mut kids: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        kids.sort();
        for k in kids {
            let name = k.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            walk(&k, out);
        }
    } else if path.extension().and_then(|e| e.to_str()) == Some("surf") {
        out.push(path.to_path_buf());
    }
}

fn main() {
    // ── 1. The registry ──
    let snippets: BTreeMap<&str, &str> = SNIPPETS.iter().copied().collect();
    let reg = registry();
    let mut covered = 0usize;
    let mut uncovered: Vec<(String, String)> = Vec::new();
    for entry in &reg {
        let mut sources: Vec<String> = Vec::new();
        if let Some(s) = snippets.get(entry.name.as_str()) {
            sources.push((*s).to_string());
        }
        for ex in examples_for(&entry.name) {
            sources.push(ex.source);
        }
        if sources.is_empty() {
            sources.push(format!("::{}\n::\n", entry.name));
        }
        let mut first_decline: Option<String> = None;
        for s in &sources {
            if let Err(why) = gate(s) {
                first_decline = Some(why);
                break;
            }
        }
        match first_decline {
            None => covered += 1,
            Some(why) => uncovered.push((entry.name.clone(), why)),
        }
    }
    println!("== registry: {covered}/{} kinds render through the DOM path", reg.len());
    let mut by_reason: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, why) in &uncovered {
        by_reason.entry(why.clone()).or_default().push(name.clone());
    }
    for (why, names) in &by_reason {
        println!("   {why:<32} ← {}", names.join(" "));
    }

    // ── 2. The documents ──
    let mut files = Vec::new();
    for arg in std::env::args().skip(1) {
        walk(Path::new(&arg), &mut files);
    }
    let mut rendered = 0usize;
    let mut histogram: BTreeMap<String, usize> = BTreeMap::new();
    let mut examples: BTreeMap<String, String> = BTreeMap::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        match gate(&src) {
            Ok(()) => rendered += 1,
            Err(why) => {
                *histogram.entry(why.clone()).or_insert(0) += 1;
                examples.entry(why).or_insert_with(|| f.display().to_string());
            }
        }
    }
    if !files.is_empty() {
        println!("== documents: {rendered}/{} render through the DOM path", files.len());
        let mut rows: Vec<(&String, &usize)> = histogram.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (why, n) in rows {
            println!("   {n:>5}  {why:<32} e.g. {}", examples.get(why).map(String::as_str).unwrap_or(""));
        }
    }
    println!("SWEEP kinds {covered}/{} docs {rendered}/{}", reg.len(), files.len());
}

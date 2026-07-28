//! E2E QA render of every SurfDoc doc-types-and-viz showcase example.
//!
//! Parses each `examples/showcase/*.surf`, renders it to the appropriate
//! target(s), writes the artifact into the plan's `rendered/` directory, and
//! asserts each artifact is non-empty and contains its expected markers.
//!
//! Run:  cargo run --example qa_render
//!       cargo run --example qa_render --features pdf   (also emits paper-ieee.pdf)

use std::fs;
use std::path::PathBuf;

use surf_parse::{extract_site, parse, render_site_page, PageConfig};

fn repo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn showcase(name: &str) -> String {
    let p = repo_dir().join("examples/showcase").join(name);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn out_dir() -> PathBuf {
    let d = repo_dir().join("plans/develop/2026-06-27-surfdoc-doc-types-and-viz/rendered");
    fs::create_dir_all(&d).expect("create rendered/ dir");
    d
}

fn write_artifact(name: &str, bytes: &[u8]) -> usize {
    let p = out_dir().join(name);
    fs::write(&p, bytes).unwrap_or_else(|e| panic!("write {}: {e}", p.display()));
    bytes.len()
}

struct Check {
    fails: usize,
}

impl Check {
    fn new() -> Self {
        Check { fails: 0 }
    }

    /// Assert `haystack` contains every marker; report PASS/FAIL line.
    fn artifact(&mut self, label: &str, file: &str, size: usize, haystack: &str, markers: &[&str]) {
        let mut missing = vec![];
        for m in markers {
            if !haystack.contains(m) {
                missing.push(*m);
            }
        }
        let nonempty = size > 0;
        let ok = nonempty && missing.is_empty();
        if !ok {
            self.fails += 1;
        }
        let matched: Vec<&str> = markers.iter().filter(|m| haystack.contains(**m)).copied().collect();
        println!(
            "[{}] {:<28} {:<22} {:>7} B  markers matched: {:?}{}",
            if ok { "PASS" } else { "FAIL" },
            label,
            file,
            size,
            matched,
            if missing.is_empty() {
                String::new()
            } else {
                format!("  MISSING: {missing:?}")
            }
        );
        if !nonempty {
            println!("       !! artifact is EMPTY");
        }
    }
}

fn main() {
    let mut chk = Check::new();
    println!("== SurfDoc doc-types-and-viz E2E QA render ==\n");

    // ---- diagrams (HTML) ----
    {
        let doc = parse(&showcase("diagrams-showcase.surf")).doc;
        let html = doc.to_html();
        let n = write_artifact("diagrams-showcase.html", html.as_bytes());
        chk.artifact(
            "diagrams",
            "diagrams-showcase.html",
            n,
            &html,
            &["<svg", "surfdoc-diagram-flowchart", "surfdoc-diagram-sequence",
              "surfdoc-diagram-gantt", "surfdoc-diagram-state", "surfdoc-diagram-mindmap"],
        );
    }

    // ---- charts (HTML) ----
    {
        let doc = parse(&showcase("charts-showcase.surf")).doc;
        let html = doc.to_html();
        let n = write_artifact("charts-showcase.html", html.as_bytes());
        chk.artifact(
            "charts",
            "charts-showcase.html",
            n,
            &html,
            &["<svg", "<polyline", "<rect", "<circle"],
        );
    }

    // ---- citations (HTML) ----
    {
        let doc = parse(&showcase("citations-showcase.surf")).doc;
        let html = doc.to_html();
        let n = write_artifact("citations-showcase.html", html.as_bytes());
        chk.artifact(
            "citations",
            "citations-showcase.html",
            n,
            &html,
            &["surfdoc-cite", "surfdoc-bibliography", "ref-smith2020", "References"],
        );
    }

    // ---- web (HTML, multipage site) ----
    {
        let src = showcase("web.surf");
        let doc = parse(&src).doc;
        let (site_opt, pages, _loose) = extract_site(&doc);
        let site = site_opt.unwrap_or_default();
        let nav_items: Vec<(String, String)> = pages
            .iter()
            .map(|p| {
                let title = p
                    .title
                    .clone()
                    .unwrap_or_else(|| surf_parse::humanize_route(&p.route));
                (p.route.clone(), title)
            })
            .collect();
        let mut html = String::new();
        for page in &pages {
            let config = PageConfig {
                source_path: "web.surf".to_string(),
                ..Default::default()
            };
            html.push_str(&render_site_page(page, &site, &nav_items, &config));
            html.push('\n');
        }
        let n = write_artifact("web.html", html.as_bytes());
        // both page routes present (nav hrefs) + prose from each page + site name.
        chk.artifact(
            "web (type: web)",
            "web.html",
            n,
            &html,
            &["href=\"/\"", "href=\"/pricing\"", "Build once", "per-seat pricing", "Surfspace"],
        );
        println!("       pages rendered: {} ({})", pages.len(),
            pages.iter().map(|p| p.route.as_str()).collect::<Vec<_>>().join(", "));

        // Core Chunk-1 deliverable: type:web === type:website (byte-identical).
        let as_website = src.replacen("type: web", "type: website", 1);
        let doc_w = parse(&as_website).doc;
        let (site_w, pages_w, _) = extract_site(&doc_w);
        let site_w = site_w.unwrap_or_default();
        let nav_w: Vec<(String, String)> = pages_w
            .iter()
            .map(|p| (p.route.clone(), p.title.clone().unwrap_or_else(|| surf_parse::humanize_route(&p.route))))
            .collect();
        let mut html_w = String::new();
        for page in &pages_w {
            let config = PageConfig { source_path: "web.surf".to_string(), ..Default::default() };
            html_w.push_str(&render_site_page(page, &site_w, &nav_w, &config));
            html_w.push('\n');
        }
        let web_eq_website = html == html_w;
        if !web_eq_website {
            chk.fails += 1;
        }
        println!(
            "[{}] equivalence: type:web byte-identical to type:website",
            if web_eq_website { "PASS" } else { "FAIL" }
        );
    }

    // ---- presentation (slides HTML) ----
    {
        let doc = parse(&showcase("presentation.surf")).doc;
        let html = doc.to_slides_html();
        let n = write_artifact("presentation.html", html.as_bytes());
        chk.artifact(
            "presentation (slides)",
            "presentation.html",
            n,
            &html,
            &["notes-pane", "slide title", "slide section", "slide two", "slide code", "<svg"],
        );
    }

    // ---- papers / reports (Typst + LaTeX) ----
    struct Academic {
        src: &'static str,
        stem: &'static str,
        typ_markers: &'static [&'static str],
        tex_markers: &'static [&'static str],
    }
    let academics = [
        Academic {
            src: "paper-ieee.surf",
            stem: "paper-ieee",
            typ_markers: &["columns: 2", "Deterministic"],
            tex_markers: &["IEEEtran", "abstract", "thebibliography"],
        },
        Academic {
            src: "report-mla.surf",
            stem: "report-mla",
            typ_markers: &["1.5em", "Works Cited"],
            tex_markers: &["doublespacing", "Works Cited"],
        },
        Academic {
            src: "report-apa.surf",
            stem: "report-apa",
            typ_markers: &["References"],
            tex_markers: &["doublespacing", "References"],
        },
        Academic {
            src: "report-chicago.surf",
            stem: "report-chicago",
            typ_markers: &["Bibliography"],
            tex_markers: &["doublespacing", "Bibliography"],
        },
    ];
    for a in &academics {
        let doc = parse(&showcase(a.src)).doc;
        let typ = doc.to_typst();
        let tex = doc.to_latex();
        let nt = write_artifact(&format!("{}.typ", a.stem), typ.as_bytes());
        let nx = write_artifact(&format!("{}.tex", a.stem), tex.as_bytes());
        chk.artifact(a.stem, &format!("{}.typ", a.stem), nt, &typ, a.typ_markers);
        chk.artifact(a.stem, &format!("{}.tex", a.stem), nx, &tex, a.tex_markers);
    }

    // ---- paper-ieee PDF (feature pdf) ----
    #[cfg(feature = "pdf")]
    {
        let doc = parse(&showcase("paper-ieee.surf")).doc;
        match doc.to_pdf(&surf_parse::PdfConfig::default()) {
            Ok(bytes) => {
                let n = write_artifact("paper-ieee.pdf", &bytes);
                let head = String::from_utf8_lossy(&bytes[..bytes.len().min(8)]).to_string();
                let ok = bytes.starts_with(b"%PDF-");
                if !ok {
                    chk.fails += 1;
                }
                println!(
                    "[{}] {:<28} {:<22} {:>7} B  header: {:?}",
                    if ok { "PASS" } else { "FAIL" },
                    "paper-ieee (pdf)",
                    "paper-ieee.pdf",
                    n,
                    head
                );
            }
            Err(e) => {
                chk.fails += 1;
                println!("[FAIL] paper-ieee (pdf): to_pdf error: {e:?}");
            }
        }
    }
    #[cfg(not(feature = "pdf"))]
    println!("[skip] paper-ieee (pdf): build with --features pdf to render PDF");

    // ---- determinism spot-check ----
    {
        let charts = parse(&showcase("charts-showcase.surf")).doc;
        let a = charts.to_html();
        let charts2 = parse(&showcase("charts-showcase.surf")).doc;
        let b = charts2.to_html();
        let charts_det = a == b;

        let paper = parse(&showcase("paper-ieee.surf")).doc;
        let c = paper.to_typst();
        let paper2 = parse(&showcase("paper-ieee.surf")).doc;
        let d = paper2.to_typst();
        let paper_det = c == d;

        if !charts_det {
            chk.fails += 1;
        }
        if !paper_det {
            chk.fails += 1;
        }
        println!(
            "\n[{}] determinism: charts-showcase HTML byte-identical x2",
            if charts_det { "PASS" } else { "FAIL" }
        );
        println!(
            "[{}] determinism: paper-ieee Typst byte-identical x2",
            if paper_det { "PASS" } else { "FAIL" }
        );
    }

    println!("\n== QA render complete: {} failure(s) ==", chk.fails);
    if chk.fails > 0 {
        std::process::exit(1);
    }
}

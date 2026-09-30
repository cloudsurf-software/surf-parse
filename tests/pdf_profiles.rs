//! The page profiles (surf-parse 0.31.0, TASK-1074 lane R): the resume
//! profile lays Ashley's V7 resume out on ONE US-Letter page in Inter with
//! the V7 margins; `to_pages` is the same compile as `to_pdf`; the generic
//! path is untouched.
#![cfg(feature = "pdf")]

use surf_parse::{page_count, to_pages, typst_source, Margins, PaperSize, PdfConfig};

const V7: &str = include_str!("fixtures/resume-ashley-yeghiayan-v7.surf");

fn route_default() -> PdfConfig {
    PdfConfig {
        paper_size: PaperSize::Letter,
        title: Some("Ashley Yeghiayan".into()),
        ..Default::default()
    }
}

#[test]
fn the_resume_template_selects_the_resume_profile_and_its_page() {
    let parsed = surf_parse::parse(V7);
    assert!(parsed.diagnostics.iter().all(|d| d.severity != surf_parse::error::Severity::Error), "{:?}", parsed.diagnostics);
    assert!(surf_parse::render_typst::is_resume(&parsed.doc));
    let cfg = parsed.doc.pdf_config(route_default());
    assert_eq!(cfg.paper_size, PaperSize::Letter);
    assert_eq!(cfg.margins, Margins::RESUME);
    let typst = parsed.doc.to_typst();
    // For the eyes: the markup as generated (never a gate).
    let _ = std::fs::write(std::env::temp_dir().join("surf-parse-resume-v7.typ"), &typst);
    assert!(typst.contains("#resume-head(["), "the head: {typst}");
    assert!(typst.contains("[Ashley Yeghiayan]"), "the name");
    assert!(typst.contains("resume-section([Clinical Experience])"), "a section title");
    assert!(typst.contains("#resume-entry([Ultrasound Technologist], [MedSmart Inc.], [Aug 2025 – Present]"), "an entry: {typst}");
    assert!(typst.contains("#resume-cols("), "the two-column bottom: {typst}");
    assert!(typst.contains("resume-section([Education & Credentials])"), "left column");
    assert!(typst.contains("resume-section([Skills])"), "right column");
    assert!(typst.contains("#resume-skill([Imaging], ["), "a skill group");
    assert!(!typst.contains("#h(1fr) SurfDoc"), "no running head on a resume: {typst}");
}

#[test]
fn ashley_v7_lays_out_on_one_letter_page_in_inter() {
    let parsed = surf_parse::parse(V7);
    let cfg = parsed.doc.pdf_config(route_default());
    let n = page_count(&parsed.doc, &cfg).expect("compiles");
    assert_eq!(n, 1, "the V7 resume is one page (the guide's 1052 of 1056 px)");
    let pdf = parsed.doc.to_pdf(&cfg).expect("pdf");
    assert!(pdf.starts_with(b"%PDF-"), "a PDF");
    assert!(pdf.len() > 10_000, "with the text and an embedded face: {} bytes", pdf.len());
    // The embedded font subset carries the face name.
    let hay = String::from_utf8_lossy(&pdf);
    assert!(hay.contains("Inter"), "Inter is embedded");
    // For the eyes: the page as rendered (never a gate; a write failure is fine).
    let _ = std::fs::write(std::env::temp_dir().join("surf-parse-resume-v7.pdf"), &pdf);
}

#[test]
fn to_pages_is_the_same_compile_as_to_pdf() {
    let parsed = surf_parse::parse(V7);
    let cfg = parsed.doc.pdf_config(route_default());
    let pages = to_pages(&parsed.doc, &cfg).expect("pages");
    assert_eq!(pages.len(), page_count(&parsed.doc, &cfg).unwrap());
    let svg = &pages[0];
    assert!(svg.starts_with("<svg"), "an svg: {}", &svg[..60.min(svg.len())]);
    // Letter in points: 612 × 792 (typst-svg prints the height as a float).
    let vb = svg.split("viewBox=\"").nth(1).and_then(|r| r.split('"').next()).expect("a viewBox");
    let nums: Vec<f64> = vb.split(' ').map(|n| n.parse().unwrap()).collect();
    assert_eq!(nums.len(), 4, "{vb}");
    assert!((nums[2] - 612.0).abs() < 0.01 && (nums[3] - 792.0).abs() < 0.01, "the Letter viewBox: {vb}");
    assert!(svg.contains("Ashley") || svg.contains("glyph"), "the page carries the text (as glyphs or text)");
}

#[test]
fn to_pdf_and_pages_gives_both_from_one_compile() {
    let parsed = surf_parse::parse(V7);
    let cfg = parsed.doc.pdf_config(route_default());
    let (pdf, pages) = surf_parse::to_pdf_and_pages(&parsed.doc, &cfg).expect("both");
    assert!(pdf.starts_with(b"%PDF-"));
    assert_eq!(pages.len(), 1);
    assert_eq!(pages, to_pages(&parsed.doc, &cfg).unwrap(), "the same pages as to_pages");
}

#[test]
fn a_two_page_resume_stays_two_pages_and_the_generic_layout_is_untouched() {
    // Eight entries of five bullets each overflow one page; the profile must
    // not clip, it must page.
    let mut long = String::from("---\ntitle: Long\nprofile: resume\n---\n\n# Someone Long\n\nA headline\n\na · b\n\n## Experience\n\n::steps\n");
    for i in 0..9 {
        long.push_str(&format!("### Role {i} — Org {i} {{time=\"2020 – 2021\"}}\nSomewhere · Full-time\n"));
        for j in 0..5 {
            long.push_str(&format!("- Bullet {j} of role {i}: a sentence long enough to wrap onto a second line at nine and three quarter points on a letter page.\n"));
        }
    }
    long.push_str("::\n");
    let parsed = surf_parse::parse(&long);
    let cfg = parsed.doc.pdf_config(PdfConfig::default());
    let n = page_count(&parsed.doc, &cfg).expect("compiles");
    assert!(n >= 2, "paged, not clipped: {n}");

    // A plain doc still renders through the generic template (the running head).
    let plain = surf_parse::parse("---\ntitle: Plain\n---\n\n# Plain\n\nHello.\n");
    let typst = plain.doc.to_typst();
    assert!(typst.contains("SurfDoc"), "the generic running head");
    assert!(!typst.contains("resume-head"), "no resume head");
    let cfg = plain.doc.pdf_config(PdfConfig::default());
    assert_eq!(cfg.paper_size, PaperSize::A4, "the route's default survives");
    assert_eq!(page_count(&plain.doc, &cfg).unwrap(), 1);
}

// ── 0.31.1 (2026-09-28): the paper the config asks for, and the resume's spacing ──

/// The generic template no longer pins `paper: "a4"`, so the PDF config's page override (the
/// route's Letter, or the doc's own `paper:`) is the paper the compile lays out — every generic
/// doc came out A4 on the web (MediaBox 595 × 842) while the pages JSON said Letter.
#[test]
fn a_generic_doc_lays_out_on_the_papers_the_config_asks_for() {
    let letter_from_route = surf_parse::parse("---\ntitle: Plain\n---\n\n# Plain\n\nA paragraph of words on the route's paper.\n");
    let cfg = letter_from_route.doc.pdf_config(route_default());
    assert_eq!(cfg.paper_size, PaperSize::Letter);
    let pages = to_pages(&letter_from_route.doc, &cfg).expect("pages");
    let (w, h) = view_box(&pages[0]);
    assert!((w - 612.0).abs() < 0.01 && (h - 792.0).abs() < 0.01, "the route's Letter, not the template's A4: {w} × {h}");

    let a4_from_the_doc = surf_parse::parse("---\ntitle: Plain\npaper: a4\n---\n\n# Plain\n\nWords on the doc's own paper.\n");
    let cfg = a4_from_the_doc.doc.pdf_config(route_default());
    assert_eq!(cfg.paper_size, PaperSize::A4, "the doc's own paper wins over the route");
    let pages = to_pages(&a4_from_the_doc.doc, &cfg).expect("pages");
    let (w, h) = view_box(&pages[0]);
    assert!((w - 595.28).abs() < 0.1 && (h - 841.89).abs() < 0.1, "A4 in points: {w} × {h}");
}

/// The resume's gaps are spacer blocks the template and the generator write (a block's weak
/// spacing collapsed in 0.31.0: sections butted, entries touched), at the V8 numbers plus the
/// two line edges; the pitch is the V8 line height.
#[test]
fn the_resume_spacing_is_written_as_spacers_at_the_v8_numbers() {
    let parsed = surf_parse::parse(V7);
    let typst = surf_parse::render_typst::to_typst(&parsed.doc);
    assert!(typst.contains("#gap(13.2pt)\n#resume-section(["), "a section gap before every section but the first: {typst}");
    assert!(typst.contains("#gap(12.2pt)\n#resume-entry(["), "an entry gap between entries");
    assert!(typst.contains("#gap(9.5pt)\n#resume-cert(["), "a credential gap");
    assert!(typst.contains("#gap(10.6pt)\n#resume-skill(["), "a skill-group gap");
    assert!(typst.contains("#resume-head(") && typst.contains("])\n#gap(10.4pt)\n"), "the head's gap");
    let template = include_str!("../assets/resume.typ");
    assert!(template.contains("#set par(justify: false, leading: 0.575em, spacing: 10.5pt)"), "the V8 pitch");
    assert!(template.contains("#set block(above: 0pt, below: 0pt)"), "no weak block spacing anywhere");
    assert!(!template.contains("weak: true") && !template.contains("#v("), "no weak spacing, no v(): {template}");
    assert!(!template.contains("paper:"), "the paper comes from the config, never the template");
    let n = page_count(&parsed.doc, &parsed.doc.pdf_config(route_default())).expect("compiles");
    assert_eq!(n, 1, "the V7 resume still lays out on ONE page at the V8 spacing");
}

fn view_box(svg: &str) -> (f64, f64) {
    let vb = svg.split("viewBox=\"").nth(1).and_then(|r| r.split('"').next()).expect("a viewBox");
    let nums: Vec<f64> = vb.split(' ').map(|n| n.parse().unwrap()).collect();
    (nums[2], nums[3])
}

// ── surf-parse 0.34.0: the running footer, the brand line, the callout card (TASK-1167) ─────────────────────────

/// A generic document long enough for three pages on US Letter.
fn three_pages() -> surf_parse::SurfDoc {
    let mut text = String::from("---\ntitle: A long report\n---\n\n# A long report\n\n");
    for i in 0..140 {
        text.push_str(&format!("Paragraph {i}: the quick brown fox jumps over the lazy dog, and the dog, being lazy, lets it, which is the whole of the story as far as anyone has ever cared to tell it.\n\n"));
    }
    let parsed = surf_parse::parse(&text);
    assert!(parsed.diagnostics.iter().all(|d| d.severity != surf_parse::error::Severity::Error), "{:?}", parsed.diagnostics);
    parsed.doc
}

#[test]
fn no_running_head_on_any_page() {
    let doc = three_pages();
    let cfg = doc.pdf_config(route_default());
    assert!(page_count(&doc, &cfg).expect("compiles") >= 3, "the fixture must span three pages");
    let src = typst_source(&doc, &cfg);
    assert!(!src.contains("header:"), "no page header is set anywhere in the source: {src}");
    assert!(!src.contains("#h(1fr) SurfDoc"), "the engine never names itself on a page");
    assert!(src.contains("#counter(page).display(\"1 / 1\", both: true)"), "the page counter stays: {src}");
    let footers = src.matches("#set page(footer:").count();
    assert_eq!(footers, 1, "ONE footer rule, the renderer's, before the template: {src}");
    assert!(src.find("#set page(footer:").unwrap() < src.find("#set text(font:").unwrap(), "the furniture precedes the template");
}

#[test]
fn brand_rides_in_every_pages_footer() {
    let doc = three_pages();
    let plain = doc.pdf_config(route_default());
    let branded = PdfConfig { brand: Some("Built with CloudSurf".into()), ..plain.clone() };
    let src = typst_source(&doc, &branded);
    assert!(src.contains("#link(\"https://cloudsurf.com\")[#\"Built with CloudSurf\"]"), "the words as a link, bottom-right: {src}");
    assert!(src.contains("#grid(columns: (1fr, auto, 1fr), align: (left, center, right)"), "the counter stays centred between equal columns");
    assert_eq!(src.matches("#set page(footer:").count(), 1, "one footer rule carries both");
    // The footer is page furniture: the page count does not move.
    assert_eq!(page_count(&doc, &branded).unwrap(), page_count(&doc, &plain).unwrap());
    // The PDF carries the link annotation on every page (Typst writes /URI uncompressed).
    let pdf = surf_parse::render_pdf::to_pdf(&doc, &branded).expect("compiles");
    let uris = pdf.windows(b"https://cloudsurf.com".len()).filter(|w| *w == b"https://cloudsurf.com").count();
    assert!(uris >= page_count(&doc, &branded).unwrap(), "one link per page: {uris} URIs");
}

#[test]
fn no_brand_means_no_footer_words() {
    let doc = three_pages();
    let cfg = doc.pdf_config(route_default());
    let src = typst_source(&doc, &cfg);
    assert!(!src.contains("cloudsurf.com"), "no brand, no words, no link: {src}");
    assert!(src.contains("#h(1fr) #counter(page).display(\"1 / 1\", both: true) #h(1fr)"), "the centred counter alone");
    let empty = PdfConfig { brand: Some("   ".into()), ..cfg };
    assert!(!typst_source(&doc, &empty).contains("cloudsurf.com"), "blank words are no brand");
}

#[test]
fn the_callout_card_has_no_stroke() {
    let parsed = surf_parse::parse("# Cards\n\n::callout[type=note title=\"Mirror\"]\nThe mirror is the sibling tree.\n::\n\n::callout[type=warning]\nMind the gap.\n::\n");
    assert!(parsed.diagnostics.iter().all(|d| d.severity != surf_parse::error::Severity::Error), "{:?}", parsed.diagnostics);
    let cfg = parsed.doc.pdf_config(route_default());
    let src = typst_source(&parsed.doc, &cfg);
    assert!(src.contains("#surfdoc-callout(\"note\""), "the note card is emitted: {src}");
    let def_start = src.find("#let surfdoc-callout").expect("the template's callout");
    let def = &src[def_start..];
    let def = &def[..def.find("\n#let ").unwrap_or(def.len())];
    assert!(!def.contains("stroke"), "no stroke on any side of the card: {def}");
    assert!(def.contains("radius: 6pt"), "the 6pt corners: {def}");
    assert_eq!(page_count(&parsed.doc, &cfg).unwrap(), 1);
}

#[test]
fn ashley_v7_still_lays_out_on_one_page_with_a_brand() {
    let parsed = surf_parse::parse(V7);
    let cfg = PdfConfig { brand: Some("Built with CloudSurf".into()), ..parsed.doc.pdf_config(route_default()) };
    assert_eq!(page_count(&parsed.doc, &cfg).expect("compiles"), 1, "the footer line sits in the margin: the V7 page holds");
    let src = typst_source(&parsed.doc, &cfg);
    assert!(src.contains("https://cloudsurf.com"), "a template-set document carries the words too (D-PDF-5)");
}

//! The page profiles (surf-parse 0.31.0, TASK-1074 lane R): the resume
//! profile lays Ashley's V7 resume out on ONE US-Letter page in Inter with
//! the V7 margins; `to_pages` is the same compile as `to_pdf`; the generic
//! path is untouched.
#![cfg(feature = "pdf")]

use surf_parse::{page_count, to_pages, Margins, PaperSize, PdfConfig};

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

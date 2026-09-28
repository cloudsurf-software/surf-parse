//! Render one `.surf` file to a PDF through the route's Letter config (the doc's own paper and margins win):
//! `cargo run --example render_pdf -- in.surf out.pdf` — the eyes-on loop for the page profiles.
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let input = args.get(1).expect("Usage: render_pdf <in.surf> <out.pdf>");
    let output = args.get(2).expect("Usage: render_pdf <in.surf> <out.pdf>");
    let source = std::fs::read_to_string(input).expect("read the .surf");
    let parsed = surf_parse::parse(&source);
    let base = surf_parse::PdfConfig { paper_size: surf_parse::PaperSize::Letter, ..Default::default() };
    let cfg = parsed.doc.pdf_config(base);
    let (pdf, pages) = surf_parse::to_pdf_and_pages(&parsed.doc, &cfg).expect("compile");
    std::fs::write(output, &pdf).expect("write the pdf");
    // The Typst source beside it, for the eyes.
    let _ = std::fs::write(format!("{output}.typ"), surf_parse::render_typst::to_typst(&parsed.doc));
    let vb = pages.first().and_then(|s| s.split("viewBox=\"").nth(1)).and_then(|r| r.split('"').next()).unwrap_or("");
    println!("{} pages · {} bytes · paper {:?} · page 1 viewBox {vb}", pages.len(), pdf.len(), cfg.paper_size);
}

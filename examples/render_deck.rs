//! Render a `.surf` file to a self-contained HTML presentation deck.
//!
//! Usage: `cargo run --example render_deck -- <input.surf> [output.html]`
//!
//! With no explicit `::deck`/`::slide` blocks, the document is auto-split into
//! slides on `#`/`##` heading boundaries (Presentation Mode).

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: render_deck <input.surf> [output.html]");
    let output = args
        .next()
        .unwrap_or_else(|| format!("{}.deck.html", input.trim_end_matches(".surf")));

    let src = std::fs::read_to_string(&input).expect("read input .surf");
    let doc = surf_parse::parse(&src).doc;
    let (config, slides) = surf_parse::extract_deck(&doc);

    let html = doc.to_slides_html();
    std::fs::write(&output, &html).expect("write output html");

    eprintln!(
        "Rendered {} slides (theme: {}) -> {}",
        slides.len(),
        config.theme_name(),
        output
    );
}

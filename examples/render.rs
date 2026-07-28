use std::env;
use std::fs;
use surf_parse::{parse, extract_site, render_site_page, PageConfig};

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: render <file.surf>");
    let route = args.get(2).map(|s| s.as_str()).unwrap_or("/");
    let content = fs::read_to_string(path).expect("Cannot read file");
    let result = parse(&content);
    let config = PageConfig {
        source_path: path.clone(),
        ..Default::default()
    };

    let (site_opt, pages, _loose) = extract_site(&result.doc);

    if pages.is_empty() {
        // Document mode — no ::page blocks
        let html = result.doc.to_html_page(&config);
        print!("{}", html);
    } else {
        // Site mode — render the requested page
        let site = site_opt.unwrap_or_default();
        let nav_items: Vec<(String, String)> = pages
            .iter()
            .map(|p| {
                let title = p.title.clone().unwrap_or_else(|| {
                    surf_parse::humanize_route(&p.route)
                });
                (p.route.clone(), title)
            })
            .collect();

        let page = pages.iter().find(|p| p.route == route).unwrap_or(&pages[0]);
        let html = render_site_page(page, &site, &nav_items, &config);
        print!("{}", html);
    }
}

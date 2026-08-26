//! Nested block addressing: every `id=` inside a `::page` must reach its OWN
//! block root — a side table keyed by span collapses when nested children share
//! a placeholder span.

#[test]
fn sibling_ids_inside_a_page_each_reach_their_own_root() {
    let src = "::page[route=\"/login\" title=\"Sign in\"]\n\n::hero[id=login-hero align=left]\n# Welcome back\n::\n\n::form[id=login-form submit=\"Sign in\" action=\"/login\"]\n- Email (email) *\n::\n\n::callout[id=login-note type=info title=\"Note\"]\nHi\n::\n\n::\n";
    let doc = surf_parse::parse(src).doc;
    let html = surf_parse::render_html::to_html(&doc);
    for id in ["login-hero", "login-form", "login-note"] {
        assert_eq!(
            html.matches(&format!("data-block-id=\"{id}\"")).count(),
            1,
            "{id} must appear exactly once:\n{html}"
        );
    }
}

#[test]
fn ids_in_two_pages_do_not_cross_talk() {
    let src = "::page[route=\"/\" title=\"Home\"]\n::hero[id=home-hero]\n# Home\n::\n::\n\n::page[route=\"/about\" title=\"About\"]\n::hero[id=about-hero]\n# About\n::\n::\n";
    let doc = surf_parse::parse(src).doc;
    let html = surf_parse::render_html::to_html(&doc);
    assert_eq!(html.matches("data-block-id=\"home-hero\"").count(), 1, "{html}");
    assert_eq!(html.matches("data-block-id=\"about-hero\"").count(), 1, "{html}");
}

#[test]
fn nested_child_spans_slice_the_source_at_the_directive() {
    let src = "# Title\n\n::page[route=\"/\" title=\"Home\"]\n\n::hero[id=home-hero]\n# Home\n::\n\n::section[id=s1]\n## Intro\n\n::callout[id=deep type=info]\nNested twice\n::\n::\n\n::\n";
    let doc = surf_parse::parse(src).doc;
    let mut seen = Vec::new();
    fn walk(blocks: &[surf_parse::Block], src: &str, seen: &mut Vec<(String, String)>) {
        for b in blocks {
            let sp = b.span();
            if sp.end_offset > sp.start_offset {
                let text = &src[sp.start_offset..sp.end_offset];
                seen.push((text.lines().next().unwrap_or("").to_string(), format!("{}-{}", sp.start_line, sp.end_line)));
            }
            match b {
                surf_parse::Block::Page { children, .. } => walk(children, src, seen),
                surf_parse::Block::Section { children, .. } => walk(children, src, seen),
                _ => {}
            }
        }
    }
    walk(&doc.blocks, src, &mut seen);
    let firsts: Vec<&str> = seen.iter().map(|(f, _)| f.as_str()).collect();
    assert!(firsts.contains(&"::page[route=\"/\" title=\"Home\"]"), "{seen:?}");
    assert!(firsts.contains(&"::hero[id=home-hero]"), "{seen:?}");
    assert!(firsts.contains(&"::section[id=s1]"), "{seen:?}");
    assert!(firsts.contains(&"::callout[id=deep type=info]"), "{seen:?}");
    let hero = seen.iter().find(|(f, _)| f.starts_with("::hero")).unwrap();
    assert_eq!(hero.1, "5-7", "{seen:?}");
    let deep = seen.iter().find(|(f, _)| f.starts_with("::callout")).unwrap();
    assert_eq!(deep.1, "12-14", "{seen:?}");
}

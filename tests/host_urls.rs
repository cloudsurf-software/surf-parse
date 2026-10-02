//! 0.37.2 — the URL allow-list passes what the HOST builds for itself and
//! still refuses the same thing from a document.
//!
//! The three inputs are the ones surf's own pages hand the renderer: a
//! template's image-slot marker in a `src`, the mail reading pane's
//! same-origin frame, and the "Add to Cursor" deeplink. Each has its
//! negative: without the host's guard — which is how every document is
//! rendered — the frame and the deeplink are dropped.

use surf_parse::{install_host_urls, Block, EmbedType, HostUrls, Span, SurfDoc};

const CURSOR: &str = "cursor://anysphere.cursor-deeplink/mcp/install?name=surfspace-cloudsurf&config=e30%3D";
const MARKER: &str = "«IMG: a wide photo of the venue or last year's crowd»";

fn fragment(blocks: Vec<Block>) -> String {
    SurfDoc { front_matter: None, blocks, source: String::new() }.to_html_fragment()
}

fn mail_frame() -> Block {
    Block::Embed {
        src: "/email/thr/body".to_string(),
        embed_type: Some(EmbedType::Generic),
        width: None,
        height: Some("480px".to_string()),
        title: Some("Message body".to_string()),
        span: Span::SYNTHETIC,
    }
}

fn cursor_cta() -> Block {
    Block::Cta { label: "Add to Cursor".to_string(), href: CURSOR.to_string(), primary: true, icon: None, span: Span::SYNTHETIC }
}

fn surf_host() -> HostUrls {
    HostUrls { same_origin_frame_prefixes: vec!["/email/".to_string()], link_schemes: vec!["cursor".to_string(), "vscode".to_string()] }
}

#[test]
fn an_image_slot_marker_rides_in_a_src_untouched() {
    let html = fragment(vec![Block::Figure { src: MARKER.to_string(), caption: None, alt: None, width: None, span: Span::SYNTHETIC }]);
    assert!(html.contains("«IMG: a wide photo of the venue"), "the marker survives to be filled: {html}");
    // from a document's source too — a marker is not a scheme to anyone
    let parsed = surf_parse::parse(&format!("::figure[src=\"{MARKER}\"]\n")).doc.to_html_fragment();
    assert!(parsed.contains("«IMG: a wide photo of the venue"), "{parsed}");
}

#[test]
fn the_host_frames_its_own_mail_body() {
    let _host = install_host_urls(surf_host());
    let html = fragment(vec![mail_frame()]);
    assert!(html.contains("<iframe class=\"surfdoc-embed-frame\" src=\"/email/thr/body\""), "{html}");
}

#[test]
fn a_document_cannot_frame_the_app_origin() {
    // no guard: this is every document render
    let built = fragment(vec![mail_frame()]);
    assert!(!built.contains("/email/thr/body"), "{built}");
    let parsed = surf_parse::parse("::embed[src=\"/email/thr/body\" height=480px]\n").doc.to_html_fragment();
    assert!(!parsed.contains("src=\"/email/thr/body\""), "{parsed}");
    // and with a host that allows /email/, a document still cannot reach another route through it
    let _host = install_host_urls(surf_host());
    let escaped = surf_parse::parse("::embed[src=\"/email/../settings/danger\" height=480px]\n").doc.to_html_fragment();
    assert!(!escaped.contains("settings/danger\""), "{escaped}");
}

#[test]
fn the_host_links_an_editor_deeplink() {
    let _host = install_host_urls(surf_host());
    let html = fragment(vec![cursor_cta()]);
    assert!(html.contains("href=\"cursor://anysphere.cursor-deeplink/mcp/install"), "{html}");
}

#[test]
fn a_document_cannot_link_an_app_scheme() {
    let built = fragment(vec![cursor_cta()]);
    assert!(!built.contains("cursor://"), "{built}");
    let parsed = surf_parse::parse(&format!("::cta[label=\"Add\" href=\"{CURSOR}\"]\n")).doc.to_html_fragment();
    assert!(!parsed.contains("cursor://"), "{parsed}");
}

#[test]
fn no_host_can_name_a_script_scheme() {
    let _host = install_host_urls(HostUrls { same_origin_frame_prefixes: vec![], link_schemes: vec!["javascript".to_string(), "data".to_string()] });
    let html = fragment(vec![Block::Cta { label: "x".to_string(), href: "javascript:alert(1)".to_string(), primary: false, icon: None, span: Span::SYNTHETIC }]);
    assert!(!html.contains("javascript:"), "{html}");
}

//! The email profile and the newsletter `::subscribe` block (0.43.0).
//!
//! A fixture issue (`tests/fixtures/newsletter-issue.surf`) renders to both
//! parts of an email and is pinned against `tests/snapshots/
//! newsletter-issue.email.{html,txt}.snap`. Update intentionally with:
//!   UPDATE_SNAPSHOTS=1 cargo test --test email

use std::fs;
use std::path::Path;

use surf_parse::{
    absolutize_url, email_disposition, parse, to_email, Block, EmailDisposition, EmailFooter,
    EmailParts, EmailProfile, SUBSCRIBE_DEFAULT_BUTTON, SUBSCRIBE_DEFAULT_CONSENT,
    SUBSCRIBE_FORM_NAME,
};

fn fixture() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/newsletter-issue.surf"))
        .expect("fixture")
}

fn footer(sent_with: bool) -> EmailFooter {
    EmailFooter {
        postal_address: "Example Press\n1 Main St\nReno, NV 89501".into(),
        unsubscribe_url: "https://app.cloudsurf.com/n/u/tok-abc".into(),
        sender_name: Some("The Example Letter".into()),
        sent_with_cloudsurf: sent_with,
    }
}

fn profile() -> EmailProfile {
    let mut p = EmailProfile::new("https://letter.example.com", footer(false));
    p.preheader = Some("Three things shipped, one essay read.".into());
    p.web_url = Some("/issues/3".into());
    p
}

fn render() -> EmailParts {
    to_email(&parse(&fixture()).doc, &profile())
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots").join(name);
    if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        fs::write(&path, actual).expect("write snapshot");
        return;
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing snapshot {} — run UPDATE_SNAPSHOTS=1 cargo test --test email", path.display()));
    assert_eq!(expected, actual, "snapshot drift for {name} — if intentional, regenerate with UPDATE_SNAPSHOTS=1");
}

// ── Snapshots ────────────────────────────────────────────────────────────

#[test]
fn issue_html_snapshot() {
    assert_snapshot("newsletter-issue.email.html.snap", &render().html);
}

#[test]
fn issue_text_snapshot() {
    assert_snapshot("newsletter-issue.email.txt.snap", &render().text);
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(render(), render());
    let doc = parse(&fixture()).doc;
    assert_eq!(doc.to_email(&profile()), render());
}

// ── The HTML part's shape ───────────────────────────────────────────────

#[test]
fn html_is_a_600px_table_layout_with_inline_styles_only() {
    let html = render().html;
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
    assert!(html.contains("<title>The Example Letter — Issue 3</title>"));
    assert!(html.contains("width=\"600\""));
    assert!(html.contains("max-width:600px"));
    assert!(html.contains("role=\"presentation\""));
    // No stylesheet, no class names: every element styles itself.
    assert!(!html.contains("<style"), "no <style> element");
    assert!(!html.contains("class="), "no class attributes");
    assert!(html.contains("<p style=\"margin:0 0 16px;"));
    // House tokens.
    assert!(html.contains("#2563eb"), "accent");
    assert!(html.contains("#f7f8fc"), "page");
    assert!(html.contains("#0f1422"), "text");
}

#[test]
fn html_carries_no_script_and_no_form() {
    let html = render().html.to_ascii_lowercase();
    for needle in ["<script", "alert(", "<form", "<input", "<button", "<textarea", "<select", "<iframe", "<video", "_honey"] {
        assert!(!html.contains(needle), "email HTML must not contain {needle}");
    }
}

#[test]
fn relative_urls_become_absolute() {
    let html = render().html;
    assert!(html.contains("src=\"https://letter.example.com/img/issue-3.png\""), "hero image");
    assert!(html.contains("href=\"https://letter.example.com/reading/essay\""), "markdown link");
    assert!(html.contains("href=\"https://letter.example.com/tour\""), "cta");
    assert!(html.contains("href=\"https://letter.example.com/issues/3\""), "web page");
    for attr in [" src=\"/", " href=\"/"] {
        assert!(!html.contains(attr), "no relative {attr}");
    }
}

#[test]
fn interactive_blocks_degrade_deterministically() {
    let html = render().html;
    // The form becomes a link to the issue's web page; the subscribe block
    // is dropped (its pitch never shows); the video becomes a link to its file.
    assert!(html.contains(">Open the form on the web</a>"));
    assert!(!html.contains("Get the next issue."));
    assert!(html.contains("<a href=\"https://media.example.com/clip.mp4\""));
    assert!(html.contains(">The studio, in one minute</a>"));

    // Without a web page the form is dropped too.
    let mut bare = profile();
    bare.web_url = None;
    let html = to_email(&parse(&fixture()).doc, &bare).html;
    assert!(!html.contains("Open the form"));
    assert!(!html.contains("View this email in your browser"));
}

#[test]
fn the_disposition_table() {
    let p = profile();
    let doc = parse("::subscribe\nHi.\n::\n\n::nav\n- [Home](/)\n::\n\n::features\n### A\nB\n::\n\n::embed[src=\"javascript:alert(1)\"]\n::\n").doc;
    let got: Vec<EmailDisposition> = doc.blocks.iter().map(|b| email_disposition(b, &p)).collect();
    assert_eq!(got[0], EmailDisposition::Drop, "subscribe");
    assert_eq!(got[1], EmailDisposition::Drop, "nav is chrome");
    assert_eq!(got[2], EmailDisposition::Degrade, "features degrade through markdown");
    assert_eq!(got[3], EmailDisposition::Drop, "an embed with no usable source");
}

#[test]
fn unsafe_links_lose_their_target_and_keep_their_text() {
    let doc = parse("Click [here](javascript:alert(1)) or ![x](data:image/png;base64,AAAA).\n").doc;
    let html = to_email(&doc, &profile()).html;
    assert!(!html.contains("javascript:"));
    assert!(!html.contains("data:image"));
    assert!(html.contains("Click here or"));
}

#[test]
fn preheader_is_hidden_text() {
    let html = render().html;
    assert!(html.contains("display:none;max-height:0;"));
    assert!(html.contains("Three things shipped, one essay read."));
}

// ── The footer rules ────────────────────────────────────────────────────

#[test]
fn footer_always_carries_the_address_and_the_unsubscribe_link() {
    let parts = render();
    assert!(parts.html.contains("<a href=\"https://app.cloudsurf.com/n/u/tok-abc\""));
    assert!(parts.html.contains(">Unsubscribe</a>"));
    assert!(parts.html.contains("Example Press<br>1 Main St<br>Reno, NV 89501"));
    assert!(parts.html.contains("You're receiving this because you subscribed to The Example Letter."));
    assert!(parts.text.contains("\n--\n"));
    assert!(parts.text.contains("Unsubscribe: https://app.cloudsurf.com/n/u/tok-abc\n"));
    assert!(parts.text.contains("Example Press\n1 Main St\nReno, NV 89501\n"));
}

#[test]
fn sent_with_cloudsurf_only_when_asked() {
    let doc = parse(&fixture()).doc;
    let off = to_email(&doc, &profile());
    assert!(!off.html.contains("Sent with"));
    assert!(!off.text.contains("Sent with"));

    let mut p = profile();
    p.footer = footer(true);
    let on = to_email(&doc, &p);
    assert!(on.html.contains("Sent with <a href=\"https://cloudsurf.com\""));
    assert!(on.text.ends_with("Sent with CloudSurf: https://cloudsurf.com\n"));
}

#[test]
fn a_relative_unsubscribe_link_joins_the_base() {
    let mut p = profile();
    p.footer.unsubscribe_url = "/n/u/tok-xyz".into();
    p.footer.sender_name = None;
    let parts = to_email(&parse("Hi.\n").doc, &p);
    assert!(parts.html.contains("href=\"https://letter.example.com/n/u/tok-xyz\""));
    assert!(parts.text.contains("Unsubscribe: https://letter.example.com/n/u/tok-xyz"));
    assert!(!parts.html.contains("You're receiving this"), "no reason line without a sender name");
}

// ── The text part ───────────────────────────────────────────────────────

#[test]
fn text_part_is_the_markdown_with_the_same_drops() {
    let text = render().text;
    assert!(text.starts_with("View this email in your browser: https://letter.example.com/issues/3\n\n"));
    assert!(text.contains("[one good essay](https://letter.example.com/reading/essay)"));
    assert!(text.contains("Open the form on the web: https://letter.example.com/issues/3"));
    assert!(text.contains("The studio, in one minute: https://media.example.com/clip.mp4"));
    assert!(!text.contains("Get the next issue."), "subscribe dropped");
    assert!(!text.contains("::"), "no directive markers");
    assert!(!text.contains("<script"));
}

#[test]
fn absolutize_is_public_and_total() {
    assert_eq!(absolutize_url("/x", "https://a.example/b/c").as_deref(), Some("https://a.example/x"));
    assert_eq!(absolutize_url("y", "https://a.example/b/c").as_deref(), Some("https://a.example/b/y"));
    assert_eq!(absolutize_url("vbscript:x", "https://a.example"), None);
}

// ── ::subscribe ─────────────────────────────────────────────────────────

#[test]
fn subscribe_parses_button_and_consent() {
    let doc = parse("::subscribe[button=\"Join the list\" consent=\"Monthly. No spam.\" placeholder=\"you@site.com\"]\nThe next issue, in your inbox.\n::\n").doc;
    match &doc.blocks[0] {
        Block::Subscribe { button, consent, placeholder, content, action, .. } => {
            assert_eq!(button.as_deref(), Some("Join the list"));
            assert_eq!(consent.as_deref(), Some("Monthly. No spam."));
            assert_eq!(placeholder.as_deref(), Some("you@site.com"));
            assert_eq!(content, "The next issue, in your inbox.");
            assert_eq!(action, &None);
        }
        other => panic!("expected Subscribe, got {other:?}"),
    }
    // Blank attributes read as absent.
    let doc = parse("::subscribe[button=\"\" consent=\" \"]\n::\n").doc;
    let Block::Subscribe { button, consent, .. } = &doc.blocks[0] else { panic!() };
    assert_eq!((button, consent), (&None, &None));
}

#[test]
fn subscribe_renders_as_a_newsletter_form() {
    let html = parse("::subscribe\nGet the next issue.\n::\n").doc.to_html_fragment();
    assert_eq!(SUBSCRIBE_FORM_NAME, "newsletter");
    // The host's form wiring keys on the exact class and on a form with no action.
    assert!(html.starts_with("<form") && html.contains("class=\"surfdoc-form\" data-form-name=\"newsletter\">"), "{html}");
    assert!(!html.contains(" action="), "no action: the host wires it like a ::form");
    assert!(html.contains("name=\"_honey\""), "the honey field ::form uses");
    assert!(html.contains("<input class=\"surfdoc-subscribe-email\" type=\"email\" name=\"email\""));
    assert!(html.contains(" required>"));
    assert!(html.contains(&format!(">{SUBSCRIBE_DEFAULT_BUTTON}</button>")));
    assert!(html.contains(&format!("<p class=\"surfdoc-subscribe-consent\">{SUBSCRIBE_DEFAULT_CONSENT}</p>")), "{html}");
    assert!(html.contains("<p class=\"surfdoc-subscribe-pitch\">Get the next issue.</p>"));
    assert!(html.ends_with("</div></form>"));
}

#[test]
fn subscribe_with_an_action_posts_there() {
    let html = parse("::subscribe[action=https://lists.example.com/join button=Join consent=\"See our [privacy note](/privacy).\"]\n::\n")
        .doc
        .to_html_fragment();
    assert!(html.contains("data-form-name=\"newsletter\" method=\"post\" action=\"https://lists.example.com/join\">"), "{html}");
    assert!(html.contains(">Join</button>"));
    assert!(html.contains("<a href=\"/privacy\" rel=\"noopener noreferrer\">privacy note</a>"), "the consent line takes inline markdown: {html}");
    // A script scheme is refused by the form-action rule.
    let html = parse("::subscribe[action=\"javascript:alert(1)\"]\n::\n").doc.to_html_fragment();
    assert!(!html.contains("javascript:") && !html.contains(" action="), "{html}");
}

#[test]
fn subscribe_round_trips_through_the_serializer() {
    let src = "::subscribe[placeholder=\"you@site.com\" button=\"Join\" consent=\"Monthly.\"]\nThe pitch.\n::\n";
    let doc = parse(src).doc;
    let again = parse(&doc.to_surf_source()).doc;
    let (Block::Subscribe { button: b1, consent: c1, content: p1, .. }, Block::Subscribe { button: b2, consent: c2, content: p2, .. }) =
        (&doc.blocks[0], &again.blocks[0])
    else {
        panic!("subscribe both times");
    };
    assert_eq!((b1, c1, p1), (b2, c2, p2));
}

#[test]
fn subscribe_degrades_to_a_link_in_markdown() {
    let md = parse("::subscribe[action=https://lists.example.com/join]\nGet the next issue.\n::\n").doc.to_markdown();
    assert_eq!(md.trim(), "[Get the next issue.](https://lists.example.com/join)");
}

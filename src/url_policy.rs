//! The one URL allow-list every renderer applies before it writes a URL taken
//! from a document into an attribute or a CSS `url()`.
//!
//! [`safe_url`] reads the scheme the way a browser would — ASCII whitespace
//! and control characters are ignored, the comparison is case-insensitive, a
//! colon that comes before any `/`, `?` or `#` makes a scheme — and returns
//! the INPUT UNCHANGED when its kind allows it, `None` otherwise. Callers
//! never emit a cleaned copy: an allowed URL is written byte-for-byte as the
//! author wrote it (still HTML-escaped by the caller).
//!
//! Kinds:
//! - [`UrlKind::Link`] — `http`, `https`, `mailto`, `tel`, protocol-relative,
//!   relative paths and fragments.
//! - [`UrlKind::Image`] — `http`, `https`, `data:image/…`, protocol-relative
//!   and relative paths.
//! - [`UrlKind::Frame`] — `https` only: never `http`, never
//!   protocol-relative, and never a relative path (a document on the app
//!   origin must not frame that origin's own signed-in routes).
//! - [`UrlKind::FormAction`] — `https` and relative paths (never
//!   protocol-relative: that is another origin).
//!
//! A scheme is `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )` (RFC 3986, and
//! what a browser's URL parser requires before it leaves the no-scheme
//! state). Text before a colon that is not one — a template's `«IMG: a photo
//! of the venue»` slot marker, `1:2` — is a relative reference, as it is to
//! the browser.
//!
//! # The host's door
//!
//! A HOST that renders its OWN blocks (an application's chrome built from
//! `Block` values, never a document's source) may widen two rules for the
//! lifetime of a guard, through [`install_host_urls`]: same-origin frames
//! under path prefixes it names, and link schemes it names (an editor's
//! install deeplink). Nothing a document can write installs it — there is
//! no front-matter key, no directive, no FFI or wasm export — and a host
//! must not hold the guard while it renders a document's blocks.

/// What the URL is about to be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UrlKind {
    /// `href` on an anchor.
    Link,
    /// `src` / `srcset` / `poster` / CSS `url()` of an image.
    Image,
    /// `src` of an `<iframe>`.
    Frame,
    /// `action` of a `<form>`.
    FormAction,
}

/// The shape of a URL once the browser has read its scheme.
#[derive(Debug, PartialEq, Eq)]
enum Shape {
    /// `//host…` (or a backslash spelling of it).
    ProtocolRelative,
    /// No scheme: a path, a query or a fragment (or empty).
    Relative,
    /// `scheme:…`, the scheme lower-cased; `rest` is what follows the colon
    /// with whitespace and controls removed and lower-cased.
    Scheme { scheme: String, rest: String },
}

fn ignored(c: char) -> bool {
    // ASCII whitespace and every C0 control plus DEL: a browser strips them
    // at the ends and drops tab / newline anywhere, so none of them may hide
    // a scheme from this reader.
    c.is_ascii_control() || c == ' '
}

/// What a host allows for the blocks it builds itself. See the module doc.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostUrls {
    /// Path prefixes (each starting with one `/`) under which a relative
    /// frame `src` is allowed: `"/email/"` lets the host frame its own
    /// `/email/<thread>/body`. A path with a `..` segment, a backslash or an
    /// encoded dot, slash or backslash is refused whatever its prefix.
    pub same_origin_frame_prefixes: Vec<String>,
    /// Extra link schemes, without the colon (`"cursor"`, `"vscode"`).
    /// `javascript`, `vbscript`, `data`, `file` and `blob` are refused even
    /// when named here.
    pub link_schemes: Vec<String>,
}

thread_local! {
    static HOST_URLS: std::cell::RefCell<Option<HostUrls>> = const { std::cell::RefCell::new(None) };
}

/// RAII guard of [`install_host_urls`]: dropping it puts back whatever was
/// installed before (or nothing).
pub struct HostUrlScope {
    previous: Option<HostUrls>,
}

impl Drop for HostUrlScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        HOST_URLS.with(|c| *c.borrow_mut() = previous);
    }
}

/// Install what the host allows for ITS OWN blocks on the current thread,
/// for the lifetime of the returned guard. Hold it only around a render of
/// blocks the host built; drop it before rendering a document.
pub fn install_host_urls(urls: HostUrls) -> HostUrlScope {
    let previous = HOST_URLS.with(|c| c.borrow_mut().replace(urls));
    HostUrlScope { previous }
}

/// Schemes no host may name.
const NEVER: [&str; 5] = ["javascript", "vbscript", "data", "file", "blob"];

fn host_allows_link_scheme(scheme: &str) -> bool {
    !NEVER.contains(&scheme)
        && HOST_URLS.with(|c| {
            c.borrow()
                .as_ref()
                .is_some_and(|h| h.link_schemes.iter().any(|s| s.eq_ignore_ascii_case(scheme)))
        })
}

fn host_allows_frame_path(raw: &str) -> bool {
    let cleaned: String = raw.chars().filter(|c| !ignored(*c)).collect();
    let lower = cleaned.to_ascii_lowercase();
    if !cleaned.starts_with('/')
        || lower.contains('\\')
        || lower.split(['/', '?', '#']).any(|seg| seg == "..")
        || ["%2e", "%2f", "%5c"].iter().any(|e| lower.contains(e))
    {
        return false;
    }
    HOST_URLS.with(|c| {
        c.borrow().as_ref().is_some_and(|h| {
            h.same_origin_frame_prefixes
                .iter()
                .any(|p| p.starts_with('/') && !p.starts_with("//") && cleaned.starts_with(p.as_str()))
        })
    })
}

fn is_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn shape(raw: &str) -> Shape {
    let cleaned: String = raw.chars().filter(|c| !ignored(*c)).collect();
    let mut chars = cleaned.chars();
    let first = chars.next();
    let second = chars.next();
    if matches!(first, Some('/') | Some('\\')) && matches!(second, Some('/') | Some('\\')) {
        return Shape::ProtocolRelative;
    }
    match cleaned.find([':', '/', '?', '#', '\\']) {
        Some(i) if cleaned.as_bytes()[i] == b':' && is_scheme(&cleaned[..i]) => Shape::Scheme {
            scheme: cleaned[..i].to_ascii_lowercase(),
            rest: cleaned[i + 1..].to_ascii_lowercase(),
        },
        _ => Shape::Relative,
    }
}

/// `Some(raw)` — unchanged — when `raw` is allowed for `kind`, else `None`.
pub(crate) fn safe_url(raw: &str, kind: UrlKind) -> Option<&str> {
    let ok = match (shape(raw), kind) {
        (Shape::Relative, UrlKind::Frame) => host_allows_frame_path(raw),
        (Shape::Relative, _) => true,
        (Shape::ProtocolRelative, UrlKind::Link | UrlKind::Image) => true,
        (Shape::ProtocolRelative, _) => false,
        (Shape::Scheme { scheme, rest }, kind) => match kind {
            UrlKind::Link => {
                matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel") || host_allows_link_scheme(&scheme)
            }
            UrlKind::Image => {
                matches!(scheme.as_str(), "http" | "https")
                    || (scheme == "data" && rest.starts_with("image/"))
            }
            UrlKind::Frame | UrlKind::FormAction => scheme == "https",
        },
    };
    ok.then_some(raw)
}

/// A link target: the input when allowed, else `#`.
pub(crate) fn link_href(raw: &str) -> &str {
    safe_url(raw, UrlKind::Link).unwrap_or("#")
}

/// An image / frame source: the input when allowed, else empty.
pub(crate) fn src_or_empty(raw: &str, kind: UrlKind) -> &str {
    safe_url(raw, kind).unwrap_or("")
}

/// A CSS value taken from a document (a colour, a gradient, a length):
/// `Some(raw)` — unchanged — unless it could write a `url()` or leave its
/// declaration. Refused: a `url(` / `image(` / `image-set(` / `cross-fade(` /
/// `element(` / `src(` / `expression(` function, a backslash escape (which
/// could spell one), a comment opener, and `;`, `{`, `}`. Whitespace and
/// controls are ignored and case folded before the check.
pub(crate) fn css_value(raw: &str) -> Option<&str> {
    let folded: String = raw.chars().filter(|c| !ignored(*c)).collect::<String>().to_ascii_lowercase();
    let refused = folded.contains(['\\', ';', '{', '}'])
        || ["url(", "image(", "image-set(", "cross-fade(", "element(", "src(", "expression(", "/*"]
            .iter()
            .any(|f| folded.contains(f));
    (!refused).then_some(raw)
}

/// The body of a CSS `url('…')`: the Image rule first (refused → empty),
/// then the characters that could leave the quoted string or the `url()`
/// percent-encoded (`'`, `(`, `)`, `\` and ASCII controls). The caller still
/// HTML-escapes the result for the attribute.
pub(crate) fn css_url(raw: &str) -> String {
    let Some(ok) = safe_url(raw, UrlKind::Image) else {
        return String::new();
    };
    let mut out = String::with_capacity(ok.len());
    for c in ok.chars() {
        match c {
            '\'' | '(' | ')' | '\\' => out.push_str(&format!("%{:02X}", c as u32)),
            c if c.is_ascii_control() => out.push_str(&format!("%{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [UrlKind; 4] = [UrlKind::Link, UrlKind::Image, UrlKind::Frame, UrlKind::FormAction];

    fn allowed(raw: &str) -> Vec<UrlKind> {
        KINDS.iter().copied().filter(|k| safe_url(raw, *k).is_some()).collect()
    }

    #[test]
    fn https_is_allowed_everywhere_and_unchanged() {
        for k in KINDS {
            assert_eq!(safe_url("https://example.com/a b", k), Some("https://example.com/a b"));
        }
    }

    #[test]
    fn http_is_link_and_image_only() {
        assert_eq!(allowed("http://example.com"), vec![UrlKind::Link, UrlKind::Image]);
    }

    #[test]
    fn mailto_and_tel_are_link_only() {
        assert_eq!(allowed("mailto:a@example.com"), vec![UrlKind::Link]);
        assert_eq!(allowed("tel:+15555550100"), vec![UrlKind::Link]);
    }

    #[test]
    fn relative_and_fragment_are_allowed_for_every_kind_but_frame() {
        for raw in ["/path", "/email/thread-id/body?q=a&b='q'", "page.html", "./a:b", "?q=1", "#top", ""] {
            assert_eq!(allowed(raw), vec![UrlKind::Link, UrlKind::Image, UrlKind::FormAction], "{raw}");
        }
    }

    #[test]
    fn protocol_relative_is_link_and_image_only() {
        for raw in ["//example.com/x", "/\\example.com", "\\\\example.com", " //example.com"] {
            assert_eq!(allowed(raw), vec![UrlKind::Link, UrlKind::Image], "{raw}");
        }
    }

    #[test]
    fn data_image_is_image_only_and_other_data_is_refused() {
        assert_eq!(allowed("data:image/png;base64,AAAA"), vec![UrlKind::Image]);
        assert_eq!(allowed("DATA:Image/svg+xml,<svg/>"), vec![UrlKind::Image]);
        assert!(allowed("data:text/html,<script>alert(1)</script>").is_empty());
        assert!(allowed("data:,hello").is_empty());
    }

    #[test]
    fn script_schemes_are_refused_for_every_kind() {
        for raw in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "java\tscript:alert(1)",
            "java\nscript:alert(1)",
            "java\rscript:alert(1)",
            "   javascript:alert(1)",
            "\u{0}javascript:alert(1)",
            "javascript\u{0}:alert(1)",
            "\u{1f}javascript:alert(1)",
            "vbscript:msgbox(1)",
            "VBScript:msgbox(1)",
            "file:///etc/passwd",
            "ftp://example.com",
            "blob:https://example.com/x",
            "x:y",
        ] {
            assert!(allowed(raw).is_empty(), "{raw:?} must be refused");
        }
    }

    #[test]
    fn colon_after_slash_query_or_hash_is_not_a_scheme() {
        assert_eq!(safe_url("/a/javascript:x", UrlKind::Link), Some("/a/javascript:x"));
        assert_eq!(safe_url("?next=javascript:x", UrlKind::Link), Some("?next=javascript:x"));
        assert_eq!(safe_url("#javascript:x", UrlKind::Link), Some("#javascript:x"));
    }

    #[test]
    fn entity_text_stays_literal() {
        // An unparsed entity is not a colon: the '#' makes this a relative
        // reference, and the caller's escape keeps the '&' literal.
        assert_eq!(safe_url("javascript&#58;alert(1)", UrlKind::Link), Some("javascript&#58;alert(1)"));
        // Once a parser has decoded it, it is an ordinary colon and refused.
        assert_eq!(safe_url("javascript\u{3a}alert(1)", UrlKind::Link), None);
    }

    #[test]
    fn helpers_fall_back() {
        assert_eq!(link_href("javascript:alert(1)"), "#");
        assert_eq!(link_href("/ok"), "/ok");
        assert_eq!(src_or_empty("javascript:alert(1)", UrlKind::Image), "");
        assert_eq!(src_or_empty("http://example.com", UrlKind::Frame), "");
        assert_eq!(src_or_empty("//example.com/x", UrlKind::Frame), "");
        assert_eq!(src_or_empty("/settings/danger", UrlKind::Frame), "");
        assert_eq!(src_or_empty("https://example.com/f", UrlKind::Frame), "https://example.com/f");
    }

    #[test]
    fn css_value_refuses_url_and_breakouts() {
        for raw in [
            "red;background-image:url(javascript:alert(1))",
            "URL(javascript:x)",
            "u r l(x)",
            "u\\72l(x)",
            "red}body{x:y",
            "expression(alert(1))",
            "image-set('a.png' 1x)",
            "ur/**/l(x)",
        ] {
            assert_eq!(css_value(raw), None, "{raw:?}");
        }
        for raw in ["#123456", "red", "rgb(1, 2, 3)", "var(--accent)", "linear-gradient(90deg, #000, #fff)", "100%", "480px", "calc(100% - 2rem)"] {
            assert_eq!(css_value(raw), Some(raw), "{raw:?}");
        }
    }

    #[test]
    fn css_url_refuses_and_encodes() {
        assert_eq!(css_url("javascript:alert(1)"), "");
        assert_eq!(css_url("/img/a.png"), "/img/a.png");
        assert_eq!(css_url("/a');x:url('b)"), "/a%27%29;x:url%28%27b%29");
        assert_eq!(css_url("/a\\b\nc"), "/a%5Cb%0Ac");
        assert_eq!(css_url("data:image/png;base64,AA"), "data:image/png;base64,AA");
    }

    #[test]
    fn text_before_a_colon_that_is_no_scheme_is_relative() {
        // A template's image-slot marker rides in a `src` until the slot is
        // filled; a browser reads it as a relative reference too.
        let marker = "«IMG: a wide photo of the venue or last year's crowd»";
        assert_eq!(safe_url(marker, UrlKind::Image), Some(marker));
        assert_eq!(src_or_empty(marker, UrlKind::Image), marker);
        assert_eq!(css_url(marker), "«IMG: a wide photo of the venue or last year%27s crowd»");
        assert_eq!(safe_url("«FILL: site | https://example.com»", UrlKind::Link), Some("«FILL: site | https://example.com»"));
        assert_eq!(safe_url("1:2", UrlKind::Link), Some("1:2"));
        // …and never a frame, and a real scheme behind such text is still read.
        assert_eq!(safe_url(marker, UrlKind::Frame), None);
        assert_eq!(safe_url("javascript:«IMG: x»", UrlKind::Image), None);
        assert_eq!(safe_url("java+script.x-1:y", UrlKind::Link), None);
    }

    fn host() -> HostUrls {
        HostUrls {
            same_origin_frame_prefixes: vec!["/email/".into()],
            link_schemes: vec!["cursor".into(), "VSCode".into(), "javascript".into(), "data".into()],
        }
    }

    #[test]
    fn without_a_host_the_door_is_shut() {
        assert_eq!(safe_url("/email/thr/body", UrlKind::Frame), None);
        assert_eq!(safe_url("cursor://anysphere.cursor-deeplink/mcp/install?name=a", UrlKind::Link), None);
        assert_eq!(safe_url("vscode:mcp/install?x", UrlKind::Link), None);
    }

    #[test]
    fn the_host_frames_its_own_prefix_and_nothing_else() {
        let _h = install_host_urls(host());
        assert_eq!(safe_url("/email/thr/body", UrlKind::Frame), Some("/email/thr/body"));
        assert_eq!(safe_url("/email/t%3Fx/body?images=1", UrlKind::Frame), Some("/email/t%3Fx/body?images=1"));
        for raw in [
            "/settings/danger",
            "/emailx/thr/body",
            "email/thr/body",
            "//email/thr/body",
            "/\\email/x",
            "/email/../settings/danger",
            "/email/thr/..",
            "/email/%2e%2e/settings",
            "/email/%2E%2e%2fsettings",
            "/email/a\\..\\b",
            "/email/a?next=/..",
            "http://example.com/email/",
            "",
        ] {
            assert_eq!(safe_url(raw, UrlKind::Frame), None, "{raw:?}");
        }
        // https frames are what they were.
        assert_eq!(safe_url("https://example.com/f", UrlKind::Frame), Some("https://example.com/f"));
    }

    #[test]
    fn the_host_names_link_schemes_but_never_a_script_one() {
        let _h = install_host_urls(host());
        let cursor = "cursor://anysphere.cursor-deeplink/mcp/install?name=a&config=e30%3D";
        assert_eq!(safe_url(cursor, UrlKind::Link), Some(cursor));
        assert_eq!(safe_url("vscode:mcp/install?%7B%7D", UrlKind::Link), Some("vscode:mcp/install?%7B%7D"));
        assert_eq!(safe_url("CURSOR://x", UrlKind::Link), Some("CURSOR://x"));
        // named by the host, refused all the same
        assert_eq!(safe_url("javascript:alert(1)", UrlKind::Link), None);
        assert_eq!(safe_url("data:text/html,x", UrlKind::Link), None);
        // a host link scheme is a LINK scheme only
        for k in [UrlKind::Image, UrlKind::Frame, UrlKind::FormAction] {
            assert_eq!(safe_url(cursor, k), None);
        }
        assert_eq!(safe_url("zed://x", UrlKind::Link), None);
    }

    #[test]
    fn the_guard_restores_what_was_there() {
        {
            let _outer = install_host_urls(HostUrls { link_schemes: vec!["cursor".into()], ..Default::default() });
            {
                let _inner = install_host_urls(HostUrls::default());
                assert_eq!(safe_url("cursor://x", UrlKind::Link), None);
            }
            assert_eq!(safe_url("cursor://x", UrlKind::Link), Some("cursor://x"));
        }
        assert_eq!(safe_url("cursor://x", UrlKind::Link), None);
    }
}

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

fn shape(raw: &str) -> Shape {
    let cleaned: String = raw.chars().filter(|c| !ignored(*c)).collect();
    let mut chars = cleaned.chars();
    let first = chars.next();
    let second = chars.next();
    if matches!(first, Some('/') | Some('\\')) && matches!(second, Some('/') | Some('\\')) {
        return Shape::ProtocolRelative;
    }
    match cleaned.find([':', '/', '?', '#', '\\']) {
        Some(i) if cleaned.as_bytes()[i] == b':' => Shape::Scheme {
            scheme: cleaned[..i].to_ascii_lowercase(),
            rest: cleaned[i + 1..].to_ascii_lowercase(),
        },
        _ => Shape::Relative,
    }
}

/// `Some(raw)` — unchanged — when `raw` is allowed for `kind`, else `None`.
pub(crate) fn safe_url(raw: &str, kind: UrlKind) -> Option<&str> {
    let ok = match (shape(raw), kind) {
        (Shape::Relative, UrlKind::Frame) => false,
        (Shape::Relative, _) => true,
        (Shape::ProtocolRelative, UrlKind::Link | UrlKind::Image) => true,
        (Shape::ProtocolRelative, _) => false,
        (Shape::Scheme { scheme, rest }, kind) => match kind {
            UrlKind::Link => matches!(scheme.as_str(), "http" | "https" | "mailto" | "tel"),
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
}

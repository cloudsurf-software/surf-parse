//! Generation slot markers (`«FILL: …»` / `«IMG: …»`) — render-time resolution.
//!
//! Mako's app generator marks brand-identity fields it cannot know as fill
//! slots (`«FILL: what it is | a realistic default»`) and references images
//! only as slots (`«IMG: description of the image»`). The markers stay RAW in
//! the committed `site/index.surf` so they remain editable; this module
//! resolves them in the FINAL rendered HTML only (called from
//! [`crate::render_html::render_site_page`] and
//! [`crate::render_html::render_site_single_file`]).
//!
//! The pass is a linear scanner over the rendered document with a small
//! context state machine (text / inside-tag / script-style raw text). The
//! output HTML is machine-generated and well-formed — `escape_html` bans raw
//! `<` in text and attribute values — so the classification is sound:
//!
//! - `«FILL: label | default»` → the default text (label when the default
//!   segment is missing or empty), in every context. `|` is the primary
//!   separator; when it is absent a spaced em-dash (`label — default`) is
//!   accepted as a fallback separator.
//! - `«IMG: description»` in **text** → `<img class="surfdoc-img-slot"
//!   src=[`IMG_SLOT_PLACEHOLDER_URI`] alt="description" loading="lazy">`.
//! - `«IMG: description»` **inside a tag** (`src="…"`, `style="…url('…')…"`)
//!   → the bare placeholder URI.
//! - `«IMG: description»` **inside `<script>`/`<style>` content** (JSON-LD
//!   etc.) → the description text only; never a tag.
//!
//! Unterminated or malformed markers are left verbatim.

/// Placeholder image for unresolved `«IMG:»` slots: a self-contained SVG data
/// URI (neutral surface, sun + mountains glyph).
///
/// Guaranteed free of `"`, `'`, `&`, and `<` so it splices verbatim into
/// `src="…"`, `url('…')`, and `url("…")` contexts. Style chunks may swap the
/// SVG body but MUST keep that guarantee (percent-encode everything).
pub const IMG_SLOT_PLACEHOLDER_URI: &str = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20600%20400%22%3E%3Crect%20width%3D%22600%22%20height%3D%22400%22%20fill%3D%22%23e5e9f0%22%2F%3E%3Ccircle%20cx%3D%22410%22%20cy%3D%22130%22%20r%3D%2242%22%20fill%3D%22%23c9d2e0%22%2F%3E%3Cpath%20d%3D%22M0%20400%20210%20180%20330%20310%20420%20230%20600%20400z%22%20fill%3D%22%23c9d2e0%22%2F%3E%3C%2Fsvg%3E";

/// Rendering context at the current scan position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// Regular element content (escaped text between tags).
    Text,
    /// Between a raw `<` and its closing `>` (tag name + attributes).
    Tag,
    /// Raw text content of a `<script>` or `<style>` element.
    Raw,
}

/// Which raw-text element the tag being scanned opens (if any).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawKind {
    None,
    Script,
    Style,
}

/// Resolve generation slot markers in rendered HTML. See the module docs for
/// the marker grammar and per-context behavior. Returns the input unchanged
/// (no allocation beyond the move) when it contains no `«`.
pub fn resolve_slot_markers(html: String) -> String {
    if !html.contains('\u{00AB}') {
        return html;
    }
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + 128);
    let mut seg = 0usize; // start of the pending verbatim slice
    let mut i = 0usize;
    let mut ctx = Ctx::Text;
    // What the tag currently being scanned opens once its `>` closes it.
    let mut pending = RawKind::None;
    // Which raw element we are inside (valid while ctx == Ctx::Raw).
    let mut raw_kind = RawKind::None;

    while i < len {
        match bytes[i] {
            b'<' if ctx == Ctx::Text => {
                pending = if tag_open(&html[i..], "script") {
                    RawKind::Script
                } else if tag_open(&html[i..], "style") {
                    RawKind::Style
                } else {
                    RawKind::None
                };
                ctx = Ctx::Tag;
                i += 1;
            }
            b'<' if ctx == Ctx::Raw => {
                // Only the matching close tag ends script/style raw text.
                let closes = match raw_kind {
                    RawKind::Script => tag_open(&html[i..], "/script"),
                    RawKind::Style => tag_open(&html[i..], "/style"),
                    RawKind::None => false,
                };
                if closes {
                    ctx = Ctx::Tag;
                    pending = RawKind::None;
                    raw_kind = RawKind::None;
                }
                i += 1;
            }
            b'>' if ctx == Ctx::Tag => {
                if pending == RawKind::None {
                    ctx = Ctx::Text;
                } else {
                    ctx = Ctx::Raw;
                    raw_kind = pending;
                    pending = RawKind::None;
                }
                i += 1;
            }
            // `«` is U+00AB = bytes C2 AB.
            0xC2 if i + 1 < len && bytes[i + 1] == 0xAB => {
                match parse_marker(&html, i, ctx) {
                    Some((end, replacement)) => {
                        out.push_str(&html[seg..i]);
                        out.push_str(&replacement);
                        i = end;
                        seg = end;
                    }
                    // Not a well-formed marker: leave the `«` verbatim and
                    // keep scanning after it (an inner marker may still parse).
                    None => i += 2,
                }
            }
            _ => i += 1,
        }
    }
    out.push_str(&html[seg..]);
    out
}

/// Does `rest` (which starts at a `<`) open the given tag name? Pass
/// `"/script"` / `"/style"` for close tags. Case-insensitive; the name must be
/// followed by whitespace, `>`, `/`, or end-of-input (so `<style>` matches but
/// `<styleguide>` does not).
fn tag_open(rest: &str, name: &str) -> bool {
    // Byte-wise: `rest` may contain multibyte chars right after the name
    // position, so `&str` slicing could split a char boundary.
    let r = &rest.as_bytes()[1..];
    let n = name.as_bytes();
    if r.len() < n.len() || !r[..n.len()].eq_ignore_ascii_case(n) {
        return false;
    }
    matches!(
        r.get(n.len()),
        None | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
    )
}

/// Try to parse a slot marker starting at byte `start` (which points at `«`).
/// Returns `(end_byte_index_past_the_closing_guillemet, replacement)` or
/// `None` when the text at `start` is not a well-formed, same-context marker.
fn parse_marker(html: &str, start: usize, ctx: Ctx) -> Option<(usize, String)> {
    let after = &html[start + 2..]; // past the 2-byte `«`
    let (is_fill, body) = if let Some(rest) = after.strip_prefix("FILL:") {
        (true, rest)
    } else if let Some(rest) = after.strip_prefix("IMG:") {
        (false, rest)
    } else {
        return None;
    };

    // Find the closing `»` (C2 BB). A raw `<`, `>`, or a nested `«` before it
    // means the marker crosses a context boundary or is malformed — leave it
    // verbatim rather than risk splicing across markup.
    let b = body.as_bytes();
    let mut j = 0usize;
    let close = loop {
        if j >= b.len() {
            return None;
        }
        match b[j] {
            b'<' | b'>' => return None,
            0xC2 if j + 1 < b.len() && b[j + 1] == 0xBB => break j,
            0xC2 if j + 1 < b.len() && b[j + 1] == 0xAB => return None,
            _ => j += 1,
        }
    };
    let content = &body[..close];
    // end = « (2 bytes) + "FILL:"/"IMG:" prefix + content + » (2 bytes)
    let end = start + 2 + (after.len() - body.len()) + close + 2;

    let replacement = if is_fill {
        // «FILL: label | default» — first `|` splits; a second `|` truncates
        // the default (trained-prompt grammar uses `|` as the sole separator).
        // Fallback: with NO `|` at all, a spaced em-dash (` — `) separates
        // label from default — models imitate the trained prompt's em-dashes
        // (`«FILL: brand tag — Skate or Die»` must render the default, not
        // the label + dash).
        let (label, default) = if content.contains('|') {
            let mut parts = content.split('|');
            (
                parts.next().unwrap_or("").trim(),
                parts.next().map(str::trim).unwrap_or(""),
            )
        } else {
            match content.split_once(" \u{2014} ") {
                Some((l, d)) => (l.trim(), d.trim()),
                None => (content.trim(), ""),
            }
        };
        if !default.is_empty() {
            default.to_string()
        } else {
            label.to_string()
        }
    } else {
        let desc = content.trim();
        match ctx {
            // Attribute position (src="…", style="…url('…')…"): the URI only.
            Ctx::Tag => IMG_SLOT_PLACEHOLDER_URI.to_string(),
            // Script/style content (JSON-LD, CSS): never emit a tag.
            Ctx::Raw => desc.to_string(),
            Ctx::Text => {
                // desc arrived through escape_html in any legit text context,
                // so `&`/`<`/`>` are already entity-escaped; only raw `"`
                // needs (re-)escaping for the alt attribute.
                let alt = desc.replace('"', "&quot;");
                format!(
                    "<img class=\"surfdoc-img-slot\" src=\"{IMG_SLOT_PLACEHOLDER_URI}\" alt=\"{alt}\" loading=\"lazy\">"
                )
            }
        }
    };
    Some((end, replacement))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(s: &str) -> String {
        resolve_slot_markers(s.to_string())
    }

    // 1. The plan's exact H1 case: FILL resolves to its default.
    #[test]
    fn fill_resolves_default() {
        let html = "<h1 class=\"surfdoc-hero-headline\">\u{ab}FILL: brand tag/headline | Skate or Die in One Piece\u{bb}</h1>";
        let out = resolve(html);
        assert_eq!(
            out,
            "<h1 class=\"surfdoc-hero-headline\">Skate or Die in One Piece</h1>"
        );
        assert!(!out.contains('\u{ab}') && !out.contains('\u{bb}'));
    }

    // 2. Missing or empty default falls back to the label.
    #[test]
    fn fill_missing_or_empty_default_uses_label() {
        assert_eq!(resolve("<p>\u{ab}FILL: phone\u{bb}</p>"), "<p>phone</p>");
        assert_eq!(resolve("<p>\u{ab}FILL: phone | \u{bb}</p>"), "<p>phone</p>");
        // FILL resolves inside attributes and <script> content too.
        assert_eq!(
            resolve("<meta content=\"\u{ab}FILL: tagline | Ride fast\u{bb}\">"),
            "<meta content=\"Ride fast\">"
        );
        assert_eq!(
            resolve("<script>{\"name\":\"\u{ab}FILL: name | Harbor\u{bb}\"}</script>"),
            "<script>{\"name\":\"Harbor\"}</script>"
        );
    }

    // 3. IMG in text context becomes a placeholder <img> carrying the
    //    description as alt text and the styling hook class.
    #[test]
    fn img_text_context_emits_placeholder_img() {
        let out = resolve("<p>\u{ab}IMG: storefront at dusk\u{bb}</p>");
        assert!(out.contains("<img class=\"surfdoc-img-slot\""));
        assert!(out.contains(&format!("src=\"{IMG_SLOT_PLACEHOLDER_URI}\"")));
        assert!(out.contains("alt=\"storefront at dusk\""));
        assert!(out.contains("loading=\"lazy\""));
        assert!(!out.contains('\u{ab}'));
    }

    // 4. IMG inside a tag (src attribute) splices the bare URI — no nested tag.
    #[test]
    fn img_attr_context_emits_uri_only() {
        let out = resolve("<img src=\"\u{ab}IMG: storefront\u{bb}\" alt=\"Storefront\">");
        assert_eq!(
            out,
            format!("<img src=\"{IMG_SLOT_PLACEHOLDER_URI}\" alt=\"Storefront\">")
        );
        assert_eq!(out.matches("<img").count(), 1, "no <img> nested inside a tag");
    }

    // 5. IMG inside a style attribute's url('…') — the hero cover case.
    #[test]
    fn img_style_url_context() {
        let out = resolve(
            "<section class=\"surfdoc-hero\" style=\"background-image:url('\u{ab}IMG: hero skateboard\u{bb}')\">",
        );
        assert!(out.contains(&format!("background-image:url('{IMG_SLOT_PLACEHOLDER_URI}')")));
        assert!(!out.contains('\u{ab}'));
        // The URI is safe for both quote styles by construction.
        assert!(!IMG_SLOT_PLACEHOLDER_URI.contains('\''));
        assert!(!IMG_SLOT_PLACEHOLDER_URI.contains('"'));
        assert!(!IMG_SLOT_PLACEHOLDER_URI.contains('&'));
        assert!(!IMG_SLOT_PLACEHOLDER_URI.contains('<'));
    }

    // 6. IMG inside <script> (JSON-LD) emits the description text only.
    #[test]
    fn img_script_context_emits_text_only() {
        let out = resolve(
            "<script type=\"application/ld+json\">{\"image\":\"\u{ab}IMG: shop photo\u{bb}\"}</script>",
        );
        assert!(out.contains("{\"image\":\"shop photo\"}"));
        assert!(!out.contains("surfdoc-img-slot"), "no tag inside script");
        // Same guarantee for <style> raw text.
        let css = resolve("<style>.x{/*\u{ab}IMG: swatch\u{bb}*/}</style>");
        assert!(css.contains("/*swatch*/"));
        assert!(!css.contains("<img"));
    }

    // 7. Unterminated / malformed markers stay verbatim; no panic.
    #[test]
    fn unterminated_or_malformed_markers_verbatim() {
        assert_eq!(resolve("<p>\u{ab}FILL: oops</p>"), "<p>\u{ab}FILL: oops</p>");
        assert_eq!(resolve("<p>\u{bb} alone</p>"), "<p>\u{bb} alone</p>");
        assert_eq!(resolve("<p>\u{ab}\u{ab}</p>"), "<p>\u{ab}\u{ab}</p>");
        assert_eq!(resolve("<p>\u{ab}NOTE: hi\u{bb}</p>"), "<p>\u{ab}NOTE: hi\u{bb}</p>");
        // Marker at EOF, cut mid-grammar.
        assert_eq!(resolve("tail \u{ab}IMG: x"), "tail \u{ab}IMG: x");
        assert_eq!(resolve("tail \u{ab}"), "tail \u{ab}");
        // A stray « before a real marker doesn't swallow it.
        assert_eq!(
            resolve("<p>\u{ab} and \u{ab}FILL: a | b\u{bb}</p>"),
            "<p>\u{ab} and b</p>"
        );
    }

    // 8. Multiple markers on one line all resolve; marker-free input takes the
    //    fast path unchanged.
    #[test]
    fn multiple_markers_and_fast_path() {
        let out = resolve("<p>\u{ab}FILL: a | A\u{bb} — \u{ab}FILL: b | B\u{bb}</p>");
        assert_eq!(out, "<p>A — B</p>");
        let plain = "<p>no markers, just prose &amp; entities</p>".to_string();
        assert_eq!(resolve_slot_markers(plain.clone()), plain);
        // FILL default with a second `|` truncates at it (grammar risk pin).
        assert_eq!(resolve("<p>\u{ab}FILL: a | B | C\u{bb}</p>"), "<p>B</p>");
    }

    // 9. p3-copy-sweep: spaced em-dash as a fallback FILL separator when the
    //    trained `|` is absent (models imitate the prompt's em-dashes).
    #[test]
    fn fill_emdash_separator_falls_back_to_default() {
        assert_eq!(
            resolve("<p>\u{ab}FILL: brand tag \u{2014} Skate or Die\u{bb}</p>"),
            "<p>Skate or Die</p>"
        );
        // Only the FIRST spaced em-dash splits.
        assert_eq!(
            resolve("<p>\u{ab}FILL: tagline \u{2014} Fast \u{2014} and loud\u{bb}</p>"),
            "<p>Fast \u{2014} and loud</p>"
        );
        // `|` still wins when present, even with an em-dash inside the default.
        assert_eq!(
            resolve("<p>\u{ab}FILL: tagline | Ride hard \u{2014} rest later\u{bb}</p>"),
            "<p>Ride hard \u{2014} rest later</p>"
        );
        // Em-dash with an empty right side falls back to the label.
        assert_eq!(
            resolve("<p>\u{ab}FILL: phone \u{2014} \u{bb}</p>"),
            "<p>phone</p>"
        );
        // Unspaced em-dash is NOT a separator (compound words stay whole).
        assert_eq!(
            resolve("<p>\u{ab}FILL: well\u{2014}known\u{bb}</p>"),
            "<p>well\u{2014}known</p>"
        );
    }
}

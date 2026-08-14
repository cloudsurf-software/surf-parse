//! Constructive DOM renderer — the SurfDoc zero-sink pilot (feature `dom`).
//!
//! Renders a parsed block tree directly into a DOM through the small
//! [`DomSink`] abstraction instead of producing an HTML string: elements are
//! created with `createElement`, text with `textContent`, attributes through
//! an allowlisted `setAttribute`. No `innerHTML` / `outerHTML` /
//! `insertAdjacentHTML` / `document.write` / `DOMParser` anywhere.
//!
//! Two sinks exist:
//! - [`NativeDom`] — a lightweight arena DOM used by tests (and by
//!   [`coverage_check`] as a dry-run target). Its serializer reproduces
//!   [`crate::render_html`]'s string conventions byte-for-byte, so the
//!   never-weaken identity corpus can compare `serialize(render_dom)` against
//!   `to_html_fragment` output exactly.
//! - `WebSysDom` (wasm32 only) — the browser sink used by the pilot runtime.
//!
//! # Coverage
//!
//! The pilot implements exactly the census of thelove222.doc.surf: `site`,
//! `page`, `hero`, `section`, `figure`, `callout`, `features`, `form`,
//! `banner`, `store`, `infocard`, `gallery`, `booking`, plus the markdown
//! subset those pages use (headings, paragraphs, bullet/ordered lists, links,
//! images, emphasis/strong, soft/hard breaks). Any other block kind or
//! markdown construct returns a typed [`RenderDomError::Unimplemented`] so
//! the takeover can decline the document and fall back to full navigation —
//! never a dead click.
//!
//! Script-emitting blocks (`store`, `booking`, `gallery` — gallery always
//! emits its lightbox script) render fine through the NATIVE sink (the
//! byte-identity corpus needs them), but are CONSTRUCTIVELY unimplemented:
//! creating a `<script>` element with text is itself a TrustedScript sink
//! under `require-trusted-types-for 'script'`, so [`check_coverage`] /
//! [`coverage_check`] decline them (`Unimplemented("script-emitting:…")`)
//! and the takeover falls back to full navigation (partial-coverage law).

use std::collections::HashMap;

use crate::render_html::{
    self, escape_markdown_in_slot_markers, slugify, split_explicit_anchor,
};
use crate::types::{Block, FormFieldType, RowState, SurfDoc};

/// Typed failure of the constructive DOM path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderDomError {
    /// The document contains a block kind or markdown construct outside the
    /// pilot coverage set. The payload names it (`"tabs"`, `"markdown:table"`,
    /// `"static-markup"` …).
    #[error("unimplemented for constructive DOM rendering: {0}")]
    Unimplemented(String),
}

fn unimpl<T>(what: impl Into<String>) -> Result<T, RenderDomError> {
    Err(RenderDomError::Unimplemented(what.into()))
}

/// Serialization style of an element's closer — `render_html` is not
/// consistent about void tags (Hero `<img …>` vs Figure/Gallery `<img … />`),
/// so the style is recorded per element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseStyle {
    /// `<tag …>children</tag>`
    Normal,
    /// `<tag …>` (HTML void style)
    Void,
    /// `<tag …/>` (no space)
    SelfClose,
    /// `<tag … />` (space before the slash)
    SelfCloseSpace,
}

/// Write-only DOM construction target.
///
/// The renderer computes, for every text node and attribute value, both the
/// `decoded` form (what the browser DOM should hold) and the `raw` form (the
/// exact bytes the string renderer would have emitted). Browser sinks use
/// `decoded`; the native test sink stores `raw` so serialization is a pure
/// concatenation and byte-compares against `render_html` output.
pub trait DomSink {
    type Node: Clone;
    /// Create a detached element. `svg` selects the SVG namespace in browser
    /// sinks. `close` is a serialization hint (ignored by browser sinks).
    fn create_element(&mut self, tag: &str, svg: bool, close: CloseStyle) -> Self::Node;
    /// Set an attribute. `value` is `None` for boolean/valueless attributes,
    /// else `(raw, decoded)`. Callers guarantee `name` passed
    /// [`attr_allowed`]; sinks may debug-assert it.
    fn set_attr(&mut self, el: &Self::Node, name: &str, value: Option<(&str, &str)>);
    fn append_child(&mut self, parent: &Self::Node, child: &Self::Node);
    /// Append a text node. `raw` is the exact serialized byte form, `decoded`
    /// the DOM text.
    fn append_text(&mut self, parent: &Self::Node, raw: &str, decoded: &str);
}

/// Attribute-name allowlist for the constructive DOM path. Every attribute
/// the covered renderers emit, plus the `data-*`/`aria-*` prefixes. `style`
/// is deliberately included (the Hero cover / Gallery item inline styles
/// would otherwise decline). `onerror` is deliberately EXCLUDED: event
/// handler content attributes are TrustedScript sinks —
/// `setAttribute('onerror')` throws under `require-trusted-types-for
/// 'script'`, panicking wasm mid-render. Image fallbacks ride the
/// `data-img-fallback` data attribute instead (the pilot shell's delegated
/// error listener performs the swap).
pub fn attr_allowed(name: &str) -> bool {
    if name.starts_with("data-") || name.starts_with("aria-") {
        return true;
    }
    matches!(
        name,
        "class" | "id" | "href" | "src" | "alt" | "title" | "role" | "style" | "tabindex"
            | "hidden" | "disabled" | "required" | "autofocus" | "autocomplete" | "rel"
            | "target" | "type" | "name" | "placeholder" | "rows" | "method" | "action"
            | "value" | "loading" | "width" | "height" | "start" | "open"
            // SVG presentation attributes used by the vendored icon set and
            // static widget markup.
            | "viewBox" | "xmlns" | "fill" | "stroke" | "stroke-width" | "stroke-linecap"
            | "stroke-linejoin" | "fill-rule" | "d" | "points" | "x" | "y" | "x1" | "y1"
            | "x2" | "y2" | "cx" | "cy" | "r" | "rx" | "ry" | "opacity" | "font-size"
            | "font-family" | "font-weight" | "text-anchor" | "transform"
    )
}

// ---------------------------------------------------------------------------
// Escaping helpers (two conventions, see module docs)
// ---------------------------------------------------------------------------

/// `render_html::escape_html` convention: `& < > "` escaped (NBSP untouched).
fn esc_markup(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            c => o.push(c),
        }
    }
    o
}

/// html5ever text-serializer convention (ammonia output): `& < >` and NBSP
/// escaped; quotes literal.
fn esc_cmark_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\u{a0}' => o.push_str("&nbsp;"),
            c => o.push(c),
        }
    }
    o
}

/// html5ever attribute-serializer convention (ammonia output): `& "` and NBSP
/// escaped; angle brackets literal.
fn esc_cmark_attr(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '"' => o.push_str("&quot;"),
            '\u{a0}' => o.push_str("&nbsp;"),
            c => o.push(c),
        }
    }
    o
}

/// pulldown-cmark `escape_href` percent-encoding, minus its `&`/`'` HTML
/// entity substitutions (those decode back to the raw character after the
/// sanitizer pass, so the net effect on the attribute VALUE is identity).
fn href_encode(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut o = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let safe = b < 0x80
            && matches!(b,
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'
                | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+'
                | b',' | b'-' | b'.' | b'/' | b':' | b';' | b'=' | b'?' | b'@' | b'_'
                | b'~' | b'^');
        if safe {
            o.push(b as char);
        } else {
            o.push('%');
            o.push(HEX[(b >> 4) as usize] as char);
            o.push(HEX[(b & 0xf) as usize] as char);
        }
    }
    o
}

/// ammonia's default URL-attribute keep rule: parseable absolute URL with an
/// allowlisted scheme, or a relative URL (`UrlRelative::PassThrough`).
fn url_kept(value: &str) -> bool {
    const SCHEMES: &[&str] = &[
        "bitcoin", "ftp", "ftps", "geo", "http", "https", "im", "irc", "ircs", "magnet",
        "mailto", "mms", "mx", "news", "nntp", "openpgp4fpr", "sip", "sms", "smsto", "ssh",
        "tel", "url", "webcal", "wtai", "xmpp",
    ];
    match ammonia::Url::parse(value) {
        Ok(url) => SCHEMES.contains(&url.scheme()),
        Err(url::ParseError::RelativeUrlWithoutBase) => true,
        Err(_) => false,
    }
}

// url::ParseError comes through ammonia's re-export chain; reference the
// crate ammonia itself re-exports so versions can never split.
use ammonia::url;

// ---------------------------------------------------------------------------
// Native arena DOM + byte-exact serializer (test sink)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum NNode {
    El {
        tag: String,
        close: CloseStyle,
        attrs: Vec<(String, Option<(String, String)>)>,
        children: Vec<usize>,
    },
    Text {
        raw: String,
        decoded: String,
    },
}

/// Lightweight arena DOM: the native [`DomSink`] used by tests and by
/// [`coverage_check`]'s dry run. [`NativeDom::serialize`] reproduces the
/// string renderer's output byte-for-byte.
#[derive(Debug, Default)]
pub struct NativeDom {
    nodes: Vec<NNode>,
}

/// Handle into a [`NativeDom`] arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeNode(usize);

impl NativeDom {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a detached container to render into (the "mount").
    pub fn create_root(&mut self) -> NativeNode {
        self.nodes.push(NNode::El {
            tag: String::new(),
            close: CloseStyle::Normal,
            attrs: Vec::new(),
            children: Vec::new(),
        });
        NativeNode(self.nodes.len() - 1)
    }

    /// Serialize the CHILDREN of `root` (the root container itself emits no
    /// tags), byte-compatible with `render_html`.
    pub fn serialize(&self, root: NativeNode) -> String {
        let mut out = String::new();
        if let NNode::El { children, .. } = &self.nodes[root.0] {
            for &c in children {
                self.serialize_node(c, &mut out);
            }
        }
        out
    }

    /// Concatenated decoded text of a subtree (what a browser DOM would
    /// report as `textContent`). Test/inspection helper.
    pub fn text_content(&self, node: NativeNode) -> String {
        let mut out = String::new();
        self.collect_text(node.0, &mut out);
        out
    }

    fn collect_text(&self, id: usize, out: &mut String) {
        match &self.nodes[id] {
            NNode::Text { decoded, .. } => out.push_str(decoded),
            NNode::El { children, .. } => {
                for &c in children {
                    self.collect_text(c, out);
                }
            }
        }
    }

    fn serialize_node(&self, id: usize, out: &mut String) {
        match &self.nodes[id] {
            NNode::Text { raw, .. } => out.push_str(raw),
            NNode::El { tag, close, attrs, children } => {
                out.push('<');
                out.push_str(tag);
                for (name, value) in attrs {
                    out.push(' ');
                    out.push_str(name);
                    if let Some((raw, _)) = value {
                        out.push_str("=\"");
                        out.push_str(raw);
                        out.push('"');
                    }
                }
                match close {
                    CloseStyle::Void => out.push('>'),
                    CloseStyle::SelfClose => out.push_str("/>"),
                    CloseStyle::SelfCloseSpace => out.push_str(" />"),
                    CloseStyle::Normal => {
                        out.push('>');
                        for &c in children {
                            self.serialize_node(c, out);
                        }
                        out.push_str("</");
                        out.push_str(tag);
                        out.push('>');
                    }
                }
            }
        }
    }
}

impl DomSink for NativeDom {
    type Node = NativeNode;

    fn create_element(&mut self, tag: &str, _svg: bool, close: CloseStyle) -> NativeNode {
        self.nodes.push(NNode::El {
            tag: tag.to_string(),
            close,
            attrs: Vec::new(),
            children: Vec::new(),
        });
        NativeNode(self.nodes.len() - 1)
    }

    fn set_attr(&mut self, el: &NativeNode, name: &str, value: Option<(&str, &str)>) {
        debug_assert!(attr_allowed(name), "attribute not allowlisted: {name}");
        if let NNode::El { attrs, .. } = &mut self.nodes[el.0] {
            attrs.push((
                name.to_string(),
                value.map(|(r, d)| (r.to_string(), d.to_string())),
            ));
        }
    }

    fn append_child(&mut self, parent: &NativeNode, child: &NativeNode) {
        let c = child.0;
        if let NNode::El { children, .. } = &mut self.nodes[parent.0] {
            children.push(c);
        }
    }

    fn append_text(&mut self, parent: &NativeNode, raw: &str, decoded: &str) {
        self.nodes.push(NNode::Text {
            raw: raw.to_string(),
            decoded: decoded.to_string(),
        });
        let id = self.nodes.len() - 1;
        if let NNode::El { children, .. } = &mut self.nodes[parent.0] {
            children.push(id);
        }
    }
}

// ---------------------------------------------------------------------------
// web-sys sink (wasm32 + dom only) — kept in a submodule so native builds
// never compile web-sys.
// ---------------------------------------------------------------------------

#[cfg(all(target_arch = "wasm32", feature = "dom"))]
pub mod websys {
    //! Browser [`DomSink`](super::DomSink) — constructive only:
    //! `create_element` / `create_element_ns`, allowlisted `set_attribute`,
    //! `append_child`, and `set_text_content` via `create_text_node`.

    use super::{attr_allowed, CloseStyle, DomSink};

    const SVG_NS: &str = "http://www.w3.org/2000/svg";

    /// [`DomSink`] over the live browser document.
    pub struct WebSysDom {
        doc: web_sys::Document,
    }

    impl WebSysDom {
        pub fn new(doc: web_sys::Document) -> Self {
            Self { doc }
        }
    }

    impl DomSink for WebSysDom {
        type Node = web_sys::Element;

        fn create_element(&mut self, tag: &str, svg: bool, _close: CloseStyle) -> Self::Node {
            if svg {
                self.doc
                    .create_element_ns(Some(SVG_NS), tag)
                    .expect("create_element_ns")
            } else {
                self.doc.create_element(tag).expect("create_element")
            }
        }

        fn set_attr(&mut self, el: &Self::Node, name: &str, value: Option<(&str, &str)>) {
            assert!(attr_allowed(name), "attribute not allowlisted: {name}");
            let decoded = value.map(|(_, d)| d).unwrap_or("");
            el.set_attribute(name, decoded).expect("set_attribute");
        }

        fn append_child(&mut self, parent: &Self::Node, child: &Self::Node) {
            parent.append_child(child).expect("append_child");
        }

        fn append_text(&mut self, parent: &Self::Node, _raw: &str, decoded: &str) {
            let t = self.doc.create_text_node(decoded);
            parent.append_child(&t).expect("append text");
        }
    }
}

// ---------------------------------------------------------------------------
// Build context: element stack with pending-text merging + prose-heading
// wiring (the DOM equivalent of render_html's wire_headings_and_toc pass 1)
// ---------------------------------------------------------------------------

struct Frame<N> {
    node: N,
    pend_raw: String,
    pend_dec: String,
}

struct Dom<'a, S: DomSink> {
    sink: &'a mut S,
    stack: Vec<Frame<S::Node>>,
    /// Global slug de-duplication, document order (matches the string pass).
    slug_counts: HashMap<String, u32>,
    /// Count of prose headings assigned so far (for the `section-N` fallback).
    headings_seen: usize,
    /// Raw (serialized-form) text accumulation for the currently open prose
    /// heading — the equivalent of `strip_tags(inner)`.
    heading_text: Option<String>,
    /// Whether the current element context is inside an `<svg>` subtree.
    svg_depth: usize,
}

impl<'a, S: DomSink> Dom<'a, S> {
    fn new(sink: &'a mut S, root: S::Node) -> Self {
        Dom {
            sink,
            stack: vec![Frame { node: root, pend_raw: String::new(), pend_dec: String::new() }],
            slug_counts: HashMap::new(),
            headings_seen: 0,
            heading_text: None,
            svg_depth: 0,
        }
    }

    fn flush_pending(&mut self) {
        let top = self.stack.last_mut().expect("stack");
        if top.pend_raw.is_empty() && top.pend_dec.is_empty() {
            return;
        }
        let raw = std::mem::take(&mut top.pend_raw);
        let dec = std::mem::take(&mut top.pend_dec);
        let node = top.node.clone();
        self.sink.append_text(&node, &raw, &dec);
    }

    fn open(&mut self, tag: &str, close: CloseStyle) {
        self.open_ns(tag, false, close);
    }

    fn open_ns(&mut self, tag: &str, svg: bool, close: CloseStyle) {
        self.flush_pending();
        let el = self.sink.create_element(tag, svg || self.svg_depth > 0, close);
        let parent = self.stack.last().expect("stack").node.clone();
        self.sink.append_child(&parent, &el);
        if svg {
            self.svg_depth += 1;
        }
        self.stack.push(Frame { node: el, pend_raw: String::new(), pend_dec: String::new() });
    }

    fn close(&mut self) {
        self.flush_pending();
        self.stack.pop().expect("unbalanced close");
    }

    fn close_svg(&mut self) {
        self.close();
        self.svg_depth = self.svg_depth.saturating_sub(1);
    }

    fn attr(&mut self, name: &str, value: AttrVal) {
        let (raw, dec) = match &value {
            AttrVal::Markup(v) => (esc_markup(v), v.to_string()),
            AttrVal::Cmark(v) => (esc_cmark_attr(v), v.to_string()),
            AttrVal::Exact { raw, decoded } => (raw.to_string(), decoded.to_string()),
        };
        let node = self.stack.last().expect("stack").node.clone();
        self.sink.set_attr(&node, name, Some((&raw, &dec)));
    }

    fn bool_attr(&mut self, name: &str) {
        let node = self.stack.last().expect("stack").node.clone();
        self.sink.set_attr(&node, name, None);
    }

    fn text_push(&mut self, raw: &str, dec: &str) {
        if let Some(h) = &mut self.heading_text {
            h.push_str(raw);
        }
        let top = self.stack.last_mut().expect("stack");
        top.pend_raw.push_str(raw);
        top.pend_dec.push_str(dec);
    }

    /// Text escaped with the `render_html::escape_html` convention.
    fn text_markup(&mut self, s: &str) {
        let raw = esc_markup(s);
        self.text_push(&raw, s);
    }

    /// Text escaped with the html5ever/ammonia convention (markdown path).
    fn text_cmark(&mut self, s: &str) {
        let raw = esc_cmark_text(s);
        self.text_push(&raw, s);
    }

    /// Text whose raw bytes equal its decoded form (structural newlines,
    /// rawtext script bodies).
    fn text_raw(&mut self, s: &str) {
        self.text_push(s, s);
    }

    /// Text with distinct raw/decoded forms (static markup with entities).
    fn text_exact(&mut self, raw: &str, dec: &str) {
        self.text_push(raw, dec);
    }

    // -- prose headings (class-less <hN>) — id wiring ----------------------

    fn open_prose_heading(&mut self, level: u8) {
        debug_assert!(self.heading_text.is_none(), "nested prose heading");
        self.open(&format!("h{level}"), CloseStyle::Normal);
        self.heading_text = Some(String::new());
    }

    fn close_prose_heading(&mut self) {
        // Explicit `{#slug}` anchor: must live entirely in the trailing text
        // run, which is still buffered in the open frame's pending text.
        let mut explicit: Option<String> = None;
        {
            let top = self.stack.last_mut().expect("stack");
            if let Some((prefix, slug)) = split_explicit_anchor(&top.pend_raw) {
                explicit = Some(slug.to_string());
                let removed = top.pend_raw.len() - prefix.len();
                let new_raw_len = top.pend_raw.len() - removed;
                let new_dec_len = top.pend_dec.len() - removed;
                top.pend_raw.truncate(new_raw_len);
                top.pend_dec.truncate(new_dec_len);
                if let Some(h) = &mut self.heading_text {
                    let l = h.len() - removed;
                    h.truncate(l);
                }
            }
        }
        let text_raw = self.heading_text.take().unwrap_or_default();
        let base = match &explicit {
            Some(s) => s.clone(),
            None => slugify(text_raw.trim()),
        };
        let slug = if base.is_empty() {
            format!("section-{}", self.headings_seen + 1)
        } else {
            let n = self.slug_counts.entry(base.clone()).or_insert(0);
            *n += 1;
            if *n == 1 { base } else { format!("{base}-{n}") }
        };
        self.headings_seen += 1;
        // The heading has no other attributes, so setting id now still makes
        // it the first (and only) attribute.
        self.attr("id", AttrVal::Markup(&slug));
        self.close();
    }
}

#[derive(Clone)]
enum AttrVal<'v> {
    /// Serializes as `escape_html(value)`.
    Markup(&'v str),
    /// Serializes with the html5ever attribute convention.
    Cmark(&'v str),
    /// Pre-computed raw/decoded pair.
    Exact { raw: &'v str, decoded: &'v str },
}

// ---------------------------------------------------------------------------
// Static trusted markup → sink calls
// ---------------------------------------------------------------------------

/// Feed a compile-time trusted markup constant (icon SVGs, widget scaffolds,
/// inline widget scripts) through the sink. This is a Rust tokenizer over
/// `&'static str` renderer-owned constants — untrusted content NEVER flows
/// through it (the signature enforces `'static`), so it is not an HTML
/// injection sink. `<script>`/`<style>` bodies are consumed as rawtext.
fn build_static<S: DomSink>(dom: &mut Dom<'_, S>, src: &'static str) -> Result<(), RenderDomError> {
    const VOID_TAGS: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source",
        "track", "wbr",
    ];
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let mut depth = 0usize; // container elements opened by THIS call
    let mut svg_stack: Vec<bool> = Vec::new(); // per open container: was it <svg>?
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if src[i..].starts_with("</") {
                let end = match src[i..].find('>') {
                    Some(e) => i + e,
                    None => return unimpl("static-markup"),
                };
                if depth == 0 {
                    return unimpl("static-markup");
                }
                if svg_stack.pop() == Some(true) {
                    dom.close_svg();
                } else {
                    dom.close();
                }
                depth -= 1;
                i = end + 1;
                continue;
            }
            // -- scan the full opening tag first ---------------------------
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                j += 1;
            }
            let tag = &src[i + 1..j];
            if tag.is_empty() {
                return unimpl("static-markup");
            }
            let mut attrs: Vec<(&str, Option<&str>)> = Vec::new();
            let close_kind: CloseStyle;
            loop {
                let mut ws = 0usize;
                while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() {
                    j += 1;
                    ws += 1;
                }
                if j >= bytes.len() {
                    return unimpl("static-markup");
                }
                if src[j..].starts_with("/>") {
                    close_kind = if ws > 0 { CloseStyle::SelfCloseSpace } else { CloseStyle::SelfClose };
                    j += 2;
                    break;
                }
                if bytes[j] == b'>' {
                    close_kind = if VOID_TAGS.contains(&tag) { CloseStyle::Void } else { CloseStyle::Normal };
                    j += 1;
                    break;
                }
                let an = j;
                while j < bytes.len()
                    && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-' || bytes[j] == b':')
                {
                    j += 1;
                }
                let name = &src[an..j];
                if name.is_empty() {
                    return unimpl("static-markup");
                }
                if j < bytes.len() && bytes[j] == b'=' {
                    if !src[j + 1..].starts_with('"') {
                        return unimpl("static-markup");
                    }
                    let vstart = j + 2;
                    let vend = match src[vstart..].find('"') {
                        Some(e) => vstart + e,
                        None => return unimpl("static-markup"),
                    };
                    attrs.push((name, Some(&src[vstart..vend])));
                    j = vend + 1;
                } else {
                    attrs.push((name, None));
                }
            }
            // -- emit -------------------------------------------------------
            let is_svg_root = tag == "svg";
            dom.open_ns(tag, is_svg_root, close_kind);
            for (name, value) in &attrs {
                match value {
                    Some(raw) => {
                        let decoded = decode_entities(raw)?;
                        dom.attr(name, AttrVal::Exact { raw, decoded: &decoded });
                    }
                    None => dom.bool_attr(name),
                }
            }
            match close_kind {
                CloseStyle::Normal => {
                    if tag == "script" || tag == "style" {
                        // rawtext: consume the body verbatim, no entity decode
                        let close_tag: &str = if tag == "script" { "</script>" } else { "</style>" };
                        let end = match src[j..].find(close_tag) {
                            Some(e) => j + e,
                            None => return unimpl("static-markup"),
                        };
                        let body = &src[j..end];
                        if !body.is_empty() {
                            dom.text_raw(body);
                        }
                        if is_svg_root {
                            dom.close_svg();
                        } else {
                            dom.close();
                        }
                        j = end + close_tag.len();
                    } else {
                        depth += 1;
                        svg_stack.push(is_svg_root);
                    }
                }
                _ => {
                    // leaf: no children
                    if is_svg_root {
                        dom.close_svg();
                    } else {
                        dom.close();
                    }
                }
            }
            i = j;
        } else {
            let end = src[i..].find('<').map(|e| i + e).unwrap_or(src.len());
            let raw = &src[i..end];
            let decoded = decode_entities(raw)?;
            dom.text_exact(raw, &decoded);
            i = end;
        }
    }
    if depth != 0 {
        return unimpl("static-markup");
    }
    Ok(())
}

/// Decode the entity references that appear in the renderer's static markup
/// constants. Unknown/bare `&` stays literal (matching HTML parsing).
fn decode_entities(s: &str) -> Result<String, RenderDomError> {
    if !s.contains('&') {
        return Ok(s.to_string());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let semi = match tail.find(';') {
            Some(e) if e <= 12 => e,
            _ => {
                out.push('&');
                rest = &tail[1..];
                continue;
            }
        };
        let ent = &tail[1..semi];
        let decoded: Option<char> = if let Some(num) = ent.strip_prefix('#') {
            let cp = if let Some(hex) = num.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                num.parse::<u32>().ok()
            };
            cp.and_then(char::from_u32)
        } else {
            match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "times" => Some('\u{d7}'),
                "lsaquo" => Some('\u{2039}'),
                "rsaquo" => Some('\u{203a}'),
                "laquo" => Some('\u{ab}'),
                "raquo" => Some('\u{bb}'),
                "middot" => Some('\u{b7}'),
                "hellip" => Some('\u{2026}'),
                "mdash" => Some('\u{2014}'),
                "ndash" => Some('\u{2013}'),
                "larr" => Some('\u{2190}'),
                "rarr" => Some('\u{2192}'),
                "copy" => Some('\u{a9}'),
                _ => None,
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

// ---------------------------------------------------------------------------
// Markdown (census subset) — pulldown-cmark events → DOM, replicating the
// `render_markdown` string pipeline (cmark → ammonia → cite splice) byte-wise
// ---------------------------------------------------------------------------

use pulldown_cmark::{Event, HeadingLevel, LinkType, Options as MdOptions, Parser, Tag, TagEnd};

fn md_options() -> MdOptions {
    let mut options = MdOptions::empty();
    options.insert(MdOptions::ENABLE_TABLES);
    options.insert(MdOptions::ENABLE_STRIKETHROUGH);
    options.insert(MdOptions::ENABLE_TASKLISTS);
    options
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Full markdown block content (`Block::Markdown`, callout bodies after
/// pre-escaping, …).
fn build_markdown<S: DomSink>(dom: &mut Dom<'_, S>, content: &str) -> Result<(), RenderDomError> {
    let events: Vec<Event> = Parser::new_ext(content, md_options()).collect();
    build_md_events(dom, &events, true)
}

/// `render_inline_markdown` equivalent: raw HTML is defused by pre-escaping
/// `& < >` before the cmark pass (slot-marker contents also protected).
fn build_inline_markdown<S: DomSink>(
    dom: &mut Dom<'_, S>,
    content: &str,
) -> Result<(), RenderDomError> {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let escaped = escape_markdown_in_slot_markers(&escaped);
    build_markdown(dom, &escaped)
}

/// `render_inline_markdown_phrasing` equivalent: a single-paragraph result is
/// spliced in without the `<p>` wrapper (or its trailing newline); anything
/// else renders in full.
fn build_phrasing<S: DomSink>(dom: &mut Dom<'_, S>, content: &str) -> Result<(), RenderDomError> {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let escaped = escape_markdown_in_slot_markers(&escaped);
    let events: Vec<Event> = Parser::new_ext(&escaped, md_options()).collect();
    let single_para = events.len() >= 2
        && matches!(events.first(), Some(Event::Start(Tag::Paragraph)))
        && matches!(events.last(), Some(Event::End(TagEnd::Paragraph)))
        && !events[1..events.len() - 1]
            .iter()
            .any(|e| matches!(e, Event::End(TagEnd::Paragraph)));
    if single_para {
        build_md_events(dom, &events[1..events.len() - 1], false)
    } else if !events.is_empty()
        && matches!(events.first(), Some(Event::Start(Tag::Paragraph)))
        && matches!(events.last(), Some(Event::End(TagEnd::Paragraph)))
    {
        // Multi-paragraph phrasing: the string renderer strip-splices the
        // outer <p></p> pair off a MULTI-block render, leaving unbalanced
        // markup no DOM can represent. Unreachable from parse() for covered
        // fields (probed), but decline rather than silently diverge when a
        // caller constructs such a block directly.
        return unimpl("phrasing:multi-paragraph");
    } else {
        build_md_events(dom, &events, true)
    }
}

/// `render_wrapped_phrasing_or_blocks` equivalent (see `render_html.rs`):
/// phrasing-only content is spliced into a `<p>` wrapper; anything containing
/// block-level markup builds inside a `<div>` instead, because an HTML parser
/// re-parsing `<p><ul>…</ul></p>` bytes auto-closes the `<p>` and hoists the
/// list out — the literal nesting this builder would otherwise produce can
/// never round-trip through the parser. Change both together.
fn build_wrapped_phrasing_or_blocks<S: DomSink>(
    dom: &mut Dom<'_, S>,
    class: Option<&str>,
    content: &str,
) -> Result<(), RenderDomError> {
    let escaped = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let escaped = escape_markdown_in_slot_markers(&escaped);
    let events: Vec<Event> = Parser::new_ext(&escaped, md_options()).collect();
    let single_para = events.len() >= 2
        && matches!(events.first(), Some(Event::Start(Tag::Paragraph)))
        && matches!(events.last(), Some(Event::End(TagEnd::Paragraph)))
        && !events[1..events.len() - 1]
            .iter()
            .any(|e| matches!(e, Event::End(TagEnd::Paragraph)));
    if single_para {
        dom.open("p", CloseStyle::Normal);
        if let Some(c) = class {
            dom.attr("class", AttrVal::Markup(c));
        }
        build_md_events(dom, &events[1..events.len() - 1], false)?;
        dom.close();
    } else {
        dom.open("div", CloseStyle::Normal);
        if let Some(c) = class {
            dom.attr("class", AttrVal::Markup(c));
        }
        build_md_events(dom, &events, true)?;
        dom.close();
    }
    Ok(())
}

/// Event walker. `at_newline` replicates pulldown's HTML writer `end_newline`
/// tracking so structural newlines land as the exact same text runs.
fn build_md_events<S: DomSink>(
    dom: &mut Dom<'_, S>,
    events: &[Event],
    initial_newline: bool,
) -> Result<(), RenderDomError> {
    let mut at_newline = initial_newline;
    let mut i = 0usize;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::Paragraph) => {
                if !at_newline {
                    dom.text_raw("\n");
                }
                dom.open("p", CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::Paragraph) => {
                dom.close();
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::Heading { level, id, classes, attrs }) => {
                if id.is_some() || !classes.is_empty() || !attrs.is_empty() {
                    return unimpl("markdown:heading-attrs");
                }
                if !at_newline {
                    dom.text_raw("\n");
                }
                dom.open_prose_heading(heading_level(*level));
                at_newline = false;
            }
            Event::End(TagEnd::Heading(_)) => {
                dom.close_prose_heading();
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::List(start)) => {
                if !at_newline {
                    dom.text_raw("\n");
                }
                match start {
                    None => dom.open("ul", CloseStyle::Normal),
                    Some(1) => dom.open("ol", CloseStyle::Normal),
                    Some(n) => {
                        dom.open("ol", CloseStyle::Normal);
                        dom.attr("start", AttrVal::Cmark(&n.to_string()));
                    }
                }
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::End(TagEnd::List(_)) => {
                dom.close();
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::Item) => {
                if !at_newline {
                    dom.text_raw("\n");
                }
                dom.open("li", CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::Item) => {
                dom.close();
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::Emphasis) => {
                dom.open("em", CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::Emphasis) => {
                dom.close();
                at_newline = false;
            }
            Event::Start(Tag::Strong) => {
                dom.open("strong", CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::Strong) => {
                dom.close();
                at_newline = false;
            }
            Event::Start(Tag::Link { link_type, dest_url, title, .. }) => {
                dom.open("a", CloseStyle::Normal);
                let href = if *link_type == LinkType::Email {
                    format!("mailto:{}", href_encode(dest_url))
                } else {
                    href_encode(dest_url)
                };
                if url_kept(&href) {
                    dom.attr("href", AttrVal::Cmark(&href));
                }
                if !title.is_empty() {
                    dom.attr("title", AttrVal::Cmark(title));
                }
                // ammonia's default link_rel, appended to every <a>.
                dom.attr("rel", AttrVal::Cmark("noopener noreferrer"));
                at_newline = false;
            }
            Event::End(TagEnd::Link) => {
                dom.close();
                at_newline = false;
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                // Collect the alt text exactly like pulldown's raw_text pass.
                let mut alt = String::new();
                let mut nest = 0usize;
                let mut j = i + 1;
                loop {
                    if j >= events.len() {
                        return unimpl("markdown:image");
                    }
                    match &events[j] {
                        Event::Start(Tag::Image { .. }) => nest += 1,
                        Event::End(TagEnd::Image) => {
                            if nest == 0 {
                                break;
                            }
                            nest -= 1;
                        }
                        Event::Text(t) | Event::Code(t) => alt.push_str(t),
                        Event::SoftBreak | Event::HardBreak => alt.push(' '),
                        Event::Start(Tag::Emphasis | Tag::Strong | Tag::Link { .. })
                        | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link) => {}
                        _ => return unimpl("markdown:image-alt"),
                    }
                    j += 1;
                }
                dom.open("img", CloseStyle::Void);
                let src = href_encode(dest_url);
                if url_kept(&src) {
                    dom.attr("src", AttrVal::Cmark(&src));
                }
                dom.attr("alt", AttrVal::Cmark(&alt));
                if !title.is_empty() {
                    dom.attr("title", AttrVal::Cmark(title));
                }
                dom.close();
                at_newline = false;
                i = j; // skip to End(Image)
            }
            Event::Text(t) => {
                if !t.is_empty() {
                    dom.text_cmark(t);
                    at_newline = t.ends_with('\n');
                }
            }
            Event::SoftBreak => {
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::HardBreak => {
                dom.open("br", CloseStyle::Void);
                dom.close();
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::BlockQuote(_)) => return unimpl("markdown:blockquote"),
            Event::Start(Tag::CodeBlock(_)) => return unimpl("markdown:code-block"),
            Event::Start(Tag::Table(_))
            | Event::Start(Tag::TableHead)
            | Event::Start(Tag::TableRow)
            | Event::Start(Tag::TableCell) => return unimpl("markdown:table"),
            Event::Start(Tag::Strikethrough) => return unimpl("markdown:strikethrough"),
            Event::Start(Tag::FootnoteDefinition(_)) | Event::FootnoteReference(_) => {
                return unimpl("markdown:footnote")
            }
            Event::Code(_) => return unimpl("markdown:code-span"),
            Event::Html(_) | Event::InlineHtml(_) | Event::Start(Tag::HtmlBlock) => {
                return unimpl("markdown:raw-html")
            }
            Event::Rule => return unimpl("markdown:rule"),
            Event::TaskListMarker(_) => return unimpl("markdown:tasklist"),
            other => return Err(RenderDomError::Unimplemented(format!(
                "markdown:{}",
                md_event_name(other)
            ))),
        }
        i += 1;
    }
    Ok(())
}

fn md_event_name(e: &Event) -> &'static str {
    match e {
        Event::Start(_) | Event::End(_) => "container",
        Event::InlineMath(_) | Event::DisplayMath(_) => "math",
        _ => "event",
    }
}

// ---------------------------------------------------------------------------
// Block arms (coverage set only) — each mirrors its render_html arm exactly
// ---------------------------------------------------------------------------

/// JSON string escaping used by the `::store`/`::booking` data islands —
/// ported verbatim from the render_html arms.
fn js_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '<' => o.push_str("\\u003c"),
            '&' => o.push_str("\\u0026"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn js_opt(v: &Option<String>) -> String {
    v.as_deref().map(js_str).unwrap_or_else(|| "null".to_string())
}

/// Serde tag name of a block (`"kind"`), used for typed decline messages.
fn block_kind(b: &Block) -> String {
    serde_json::to_value(b)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_block<S: DomSink>(dom: &mut Dom<'_, S>, block: &Block) -> Result<(), RenderDomError> {
    match block {
        Block::Markdown { content, .. } => build_markdown(dom, content)?,

        Block::Callout { callout_type, title, content, .. } => {
            let type_str = render_html::callout_type_str(*callout_type);
            let role = if matches!(callout_type, crate::types::CalloutType::Danger) {
                "alert"
            } else {
                "note"
            };
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup(&format!("surfdoc-callout surfdoc-callout-{type_str}")));
            dom.attr("role", AttrVal::Markup(role));
            build_static(dom, render_html::callout_icon_svg(*callout_type))?;
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-callout-body"));
            if let Some(t) = title {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-callout-title"));
                dom.text_markup(t);
                dom.close();
            }
            build_inline_markdown(dom, content)?;
            dom.close();
            dom.close();
        }

        Block::Figure { src, caption, alt, .. } => {
            let alt_attr = alt.as_deref().unwrap_or("");
            dom.open("figure", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-figure"));
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-figure-img"));
            dom.open("img", CloseStyle::SelfCloseSpace);
            dom.attr("src", AttrVal::Markup(src));
            dom.attr("alt", AttrVal::Markup(alt_attr));
            dom.attr("data-img-fallback", AttrVal::Markup("hide"));
            dom.close();
            dom.close();
            if let Some(c) = caption {
                dom.open("figcaption", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-figure-cap"));
                dom.text_markup(c);
                dom.close();
            }
            dom.close();
        }

        Block::Site { properties, domain, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-site"));
            dom.attr("aria-hidden", AttrVal::Markup("true"));
            if let Some(d) = domain {
                dom.attr("data-domain", AttrVal::Markup(d));
            }
            // NOTE: render_html double-escapes here — each key/value is
            // escape_html'd, joined, then the whole attribute is escaped
            // again. Markup() supplies the second pass.
            let pairs: Vec<String> = properties
                .iter()
                .map(|p| format!("{}={}", esc_markup(&p.key), esc_markup(&p.value)))
                .collect();
            dom.attr("data-properties", AttrVal::Markup(&pairs.join(";")));
            dom.close();
        }

        Block::Page { route, layout, title, children, .. } => {
            dom.open("section", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-page"));
            if let Some(l) = layout {
                dom.attr("data-layout", AttrVal::Markup(l));
            }
            match title {
                Some(t) => dom.attr("aria-label", AttrVal::Markup(t)),
                None => dom.attr("aria-label", AttrVal::Markup(&format!("Page: {route}"))),
            }
            for child in children {
                build_block(dom, child)?;
            }
            dom.close();
        }

        Block::Form { fields, submit_label, action, method, honeypot, .. } => {
            let btn_label = submit_label.as_deref().unwrap_or("Submit");
            dom.open("form", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-form"));
            if let Some(a) = action {
                let m = method.as_deref().unwrap_or("post");
                dom.attr("method", AttrVal::Markup(m));
                dom.attr("action", AttrVal::Markup(a));
            }
            if *honeypot {
                build_static(dom, render_html::FORM_HONEYPOT_HTML)?;
            }
            for field in fields {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-form-field"));
                dom.open("label", CloseStyle::Normal);
                dom.text_markup(&field.label);
                if field.required {
                    dom.text_raw(" ");
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("required"));
                    dom.text_raw("*");
                    dom.close();
                }
                dom.close();
                match field.field_type {
                    FormFieldType::Textarea => {
                        let ph = field.placeholder.as_deref().unwrap_or("");
                        dom.open("textarea", CloseStyle::Normal);
                        dom.attr("name", AttrVal::Markup(&field.name));
                        dom.attr("placeholder", AttrVal::Markup(ph));
                        dom.attr("rows", AttrVal::Markup("4"));
                        if field.required {
                            dom.bool_attr("required");
                        }
                        dom.close();
                    }
                    FormFieldType::Select => {
                        dom.open("select", CloseStyle::Normal);
                        dom.attr("name", AttrVal::Markup(&field.name));
                        if field.required {
                            dom.bool_attr("required");
                        }
                        dom.open("option", CloseStyle::Normal);
                        dom.attr("value", AttrVal::Markup(""));
                        dom.text_raw("Select...");
                        dom.close();
                        for opt in &field.options {
                            dom.open("option", CloseStyle::Normal);
                            dom.attr("value", AttrVal::Markup(opt));
                            dom.text_markup(opt);
                            dom.close();
                        }
                        dom.close();
                    }
                    _ => {
                        let input_type = match field.field_type {
                            FormFieldType::Email => "email",
                            FormFieldType::Tel => "tel",
                            FormFieldType::Date => "date",
                            FormFieldType::Number => "number",
                            FormFieldType::Password => "password",
                            _ => "text",
                        };
                        let ph = field.placeholder.as_deref().unwrap_or("");
                        dom.open("input", CloseStyle::SelfClose);
                        dom.attr("type", AttrVal::Markup(input_type));
                        dom.attr("name", AttrVal::Markup(&field.name));
                        dom.attr("placeholder", AttrVal::Markup(ph));
                        if field.required {
                            dom.bool_attr("required");
                        }
                        dom.close();
                    }
                }
                dom.close();
            }
            dom.open("button", CloseStyle::Normal);
            dom.attr("type", AttrVal::Markup("submit"));
            dom.attr("class", AttrVal::Markup("surfdoc-form-submit"));
            dom.text_markup(btn_label);
            dom.close();
            dom.close();
        }

        Block::Banner { headline, subtitle, buttons, id, .. } => {
            dom.open("section", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-banner"));
            if let Some(i) = id {
                dom.attr("id", AttrVal::Markup(i));
            }
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-banner-inner"));
            if let Some(h) = headline {
                dom.open("h2", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-banner-headline"));
                build_phrasing(dom, h)?;
                dom.close();
            }
            if let Some(s) = subtitle {
                dom.open("p", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-banner-subtitle"));
                build_phrasing(dom, s)?;
                dom.close();
            }
            if !buttons.is_empty() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-banner-actions"));
                for btn in buttons {
                    let cls = if btn.primary {
                        "surfdoc-banner-btn surfdoc-banner-btn-primary"
                    } else {
                        "surfdoc-banner-btn surfdoc-banner-btn-secondary"
                    };
                    dom.open("a", CloseStyle::Normal);
                    dom.attr("href", AttrVal::Markup(&btn.href));
                    dom.attr("class", AttrVal::Markup(cls));
                    if btn.external {
                        dom.attr("target", AttrVal::Markup("_blank"));
                        dom.attr("rel", AttrVal::Markup("noopener"));
                    }
                    dom.text_markup(&btn.label);
                    dom.close();
                }
                dom.close();
            }
            dom.close();
            dom.close();
        }

        Block::Hero {
            headline,
            subtitle,
            badge,
            align,
            image,
            image_alt,
            layout,
            transparent,
            buttons,
            ..
        } => {
            let cover = layout.as_deref() == Some("cover") && image.is_some();
            let stacked = layout.as_deref() == Some("stacked");
            let image_above = !cover && (stacked || align != "left");
            let image_side = !cover && !image_above;
            let align_cls = if align == "left" { " surfdoc-hero-left" } else { "" };
            let layout_cls = layout
                .as_deref()
                .map(|l| format!(" surfdoc-hero-{}", l))
                .unwrap_or_default();
            let transparent_cls = if *transparent { " surfdoc-hero-transparent" } else { "" };
            let alt = image_alt.as_deref().unwrap_or("");
            dom.open("section", CloseStyle::Normal);
            dom.attr(
                "class",
                AttrVal::Markup(&format!("surfdoc-hero{align_cls}{layout_cls}{transparent_cls}")),
            );
            if cover {
                // Byte-parity note: render_html escape_html's the src INSIDE
                // url('…') but leaves apostrophes alone (known flaw, pinned
                // by hostile fixtures). Markup() reproduces it exactly.
                dom.attr(
                    "style",
                    AttrVal::Markup(&format!(
                        "background-image:url('{}')",
                        image.as_deref().unwrap_or("")
                    )),
                );
            }
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-hero-inner"));
            if image_above {
                if let Some(img) = image {
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-hero-image"));
                    dom.open("img", CloseStyle::Void);
                    dom.attr("src", AttrVal::Markup(img));
                    dom.attr("alt", AttrVal::Markup(alt));
                    dom.attr("data-img-fallback", AttrVal::Markup("broken"));
                    dom.close();
                    dom.close();
                }
            }
            if let Some(b) = badge {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-hero-badge"));
                dom.text_markup(b);
                dom.close();
            }
            if let Some(h) = headline {
                dom.open("h1", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-hero-headline"));
                build_phrasing(dom, h)?;
                dom.close();
            }
            if let Some(s) = subtitle {
                build_wrapped_phrasing_or_blocks(dom, Some("surfdoc-hero-subtitle"), s)?;
            }
            if !buttons.is_empty() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-hero-actions"));
                for btn in buttons {
                    let cls = if btn.primary {
                        "surfdoc-hero-btn surfdoc-hero-btn-primary"
                    } else {
                        "surfdoc-hero-btn surfdoc-hero-btn-secondary"
                    };
                    dom.open("a", CloseStyle::Normal);
                    dom.attr("href", AttrVal::Markup(&btn.href));
                    dom.attr("class", AttrVal::Markup(cls));
                    if btn.external {
                        dom.attr("target", AttrVal::Markup("_blank"));
                        dom.attr("rel", AttrVal::Markup("noopener"));
                    }
                    dom.text_markup(&btn.label);
                    dom.close();
                }
                dom.close();
            }
            dom.close(); // .surfdoc-hero-inner
            if image_side {
                if let Some(img) = image {
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-hero-image-side"));
                    dom.open("img", CloseStyle::Void);
                    dom.attr("src", AttrVal::Markup(img));
                    dom.attr("alt", AttrVal::Markup(alt));
                    dom.attr("data-img-fallback", AttrVal::Markup("broken"));
                    dom.close();
                    dom.close();
                }
            }
            dom.close();
        }

        Block::Features { cards, cols, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-features"));
            if let Some(c) = cols {
                dom.attr("data-cols", AttrVal::Markup(&c.to_string()));
            }
            for card in cards {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-feature-card"));
                if let Some(icon) = &card.icon {
                    if let Some(svg) = crate::icons::get_icon(icon) {
                        dom.open("span", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-feature-icon"));
                        build_static(dom, svg)?;
                        dom.close();
                    }
                }
                dom.open("h3", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-feature-title"));
                build_phrasing(dom, &card.title)?;
                dom.close();
                if !card.body.is_empty() {
                    build_wrapped_phrasing_or_blocks(dom, Some("surfdoc-feature-body"), &card.body)?;
                }
                if let (Some(label), Some(href)) = (&card.link_label, &card.link_href) {
                    dom.open("a", CloseStyle::Normal);
                    dom.attr("href", AttrVal::Markup(href));
                    dom.attr("class", AttrVal::Markup("surfdoc-feature-link"));
                    dom.text_markup(label);
                    dom.text_raw(" \u{2192}");
                    dom.close();
                }
                dom.close();
            }
            dom.close();
        }

        Block::Section { bg, headline, subtitle, children, .. } => {
            let bg_cls = bg
                .as_ref()
                .map(|b| format!(" section-{}", esc_markup(b)))
                .unwrap_or_default();
            dom.open("section", CloseStyle::Normal);
            // bg is escape_html'd INTO the class string, then the attribute
            // is emitted verbatim — a single escaping pass overall.
            dom.attr(
                "class",
                AttrVal::Exact {
                    raw: &format!("surfdoc-section{bg_cls}"),
                    decoded: &format!(
                        "surfdoc-section{}",
                        bg.as_ref().map(|b| format!(" section-{b}")).unwrap_or_default()
                    ),
                },
            );
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-section-inner"));
            if headline.is_some() || subtitle.is_some() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-section-header"));
                if let Some(h) = headline {
                    // Class-less <h2>: participates in the prose-heading id
                    // pass exactly like a markdown heading.
                    dom.open_prose_heading(2);
                    build_phrasing(dom, h)?;
                    dom.close_prose_heading();
                }
                if let Some(s) = subtitle {
                    build_wrapped_phrasing_or_blocks(dom, None, s)?;
                }
                dom.close();
            }
            for child in children {
                build_block(dom, child)?;
            }
            dom.close();
            dom.close();
        }

        Block::Gallery { items, columns, .. } => {
            let cols = columns.unwrap_or(3);
            let categories: Vec<&str> = {
                let mut cats: Vec<&str> =
                    items.iter().filter_map(|i| i.category.as_deref()).collect();
                cats.sort();
                cats.dedup();
                cats
            };
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-gallery"));
            dom.attr("data-cols", AttrVal::Markup(&cols.to_string()));
            if !categories.is_empty() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-gallery-filters"));
                dom.open("button", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("filter-btn active"));
                dom.attr("data-filter", AttrVal::Markup("all"));
                dom.text_raw("All");
                dom.close();
                for cat in &categories {
                    dom.open("button", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("filter-btn"));
                    dom.attr("data-filter", AttrVal::Markup(cat));
                    dom.text_markup(cat);
                    dom.close();
                }
                dom.close();
            }
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-gallery-grid"));
            for (i, item) in items.iter().enumerate() {
                let alt = item.alt.as_deref().unwrap_or("");
                dom.open("figure", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-gallery-item"));
                dom.attr("data-index", AttrVal::Markup(&i.to_string()));
                dom.attr("style", AttrVal::Markup("cursor:pointer"));
                dom.attr("tabindex", AttrVal::Markup("0"));
                dom.attr("role", AttrVal::Markup("button"));
                dom.attr("aria-label", AttrVal::Markup("Open image in lightbox"));
                if let Some(c) = &item.category {
                    dom.attr("data-category", AttrVal::Markup(c));
                }
                dom.open("img", CloseStyle::SelfCloseSpace);
                dom.attr("src", AttrVal::Markup(&item.src));
                dom.attr("alt", AttrVal::Markup(alt));
                dom.attr("loading", AttrVal::Markup("lazy"));
                dom.attr("data-img-fallback", AttrVal::Markup("hide"));
                dom.close();
                if let Some(cap) = &item.caption {
                    dom.open("figcaption", CloseStyle::Normal);
                    dom.text_markup(cap);
                    dom.close();
                }
                dom.close();
            }
            dom.close();
            build_static(dom, render_html::GALLERY_LIGHTBOX_HTML)?;
            if !categories.is_empty() {
                build_static(dom, render_html::GALLERY_FILTER_JS)?;
            }
            build_static(dom, render_html::GALLERY_LIGHTBOX_JS)?;
            dom.close();
        }

        Block::Store { title, currency, items, .. } => {
            let cur = currency.as_deref().unwrap_or("$");
            let items_json = items
                .iter()
                .map(|it| {
                    format!(
                        "{{\"name\":{},\"price\":{},\"blurb\":{},\"badge\":{},\"category\":{}}}",
                        js_str(&it.name),
                        js_str(&it.price),
                        js_opt(&it.blurb),
                        js_opt(&it.badge),
                        js_opt(&it.category),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let data_json = format!("{{\"currency\":{},\"items\":[{items_json}]}}", js_str(cur));

            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-store"));
            dom.bool_attr("data-store");
            if let Some(t) = title {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-store-head"));
                dom.text_markup(t);
                dom.close();
            }
            build_static(dom, render_html::STORE_LAYOUT_HTML)?;
            build_static(dom, render_html::STORE_FORM_HTML)?;
            dom.open("script", CloseStyle::Normal);
            dom.attr("type", AttrVal::Markup("application/json"));
            dom.bool_attr("data-st-data");
            dom.text_raw(&data_json);
            dom.close();
            build_static(dom, render_html::STORE_WIDGET_JS)?;
            dom.close();
        }

        Block::Booking { title, service_label, services, days, .. } => {
            let services_json = services
                .iter()
                .map(|s| {
                    format!(
                        "{{\"name\":{},\"duration\":{},\"price\":{}}}",
                        js_str(&s.name),
                        js_opt(&s.duration),
                        js_opt(&s.price),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            let days_json = days
                .iter()
                .map(|d| {
                    let slots = d.slots.iter().map(|s| js_str(s)).collect::<Vec<_>>().join(",");
                    format!("{{\"date\":{},\"slots\":[{}]}}", js_str(&d.date), slots)
                })
                .collect::<Vec<_>>()
                .join(",");
            let data_json = format!("{{\"services\":[{services_json}],\"days\":[{days_json}]}}");

            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-booking"));
            dom.bool_attr("data-booking");
            if let Some(t) = title {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-booking-head"));
                dom.text_markup(t);
                dom.close();
            }
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-booking-grid"));
            if !services.is_empty() {
                let label = service_label.as_deref().unwrap_or("Service");
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-booking-col surfdoc-booking-svc-col"));
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-booking-label"));
                dom.text_markup(label);
                dom.close();
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-booking-services"));
                dom.bool_attr("data-bk-services");
                dom.attr("role", AttrVal::Markup("radiogroup"));
                dom.attr("aria-label", AttrVal::Markup(label));
                dom.close();
                dom.close();
            }
            build_static(dom, render_html::BOOKING_CAL_HTML)?;
            dom.close(); // .surfdoc-booking-grid
            build_static(dom, render_html::BOOKING_FORM_HTML)?;
            dom.open("script", CloseStyle::Normal);
            dom.attr("type", AttrVal::Markup("application/json"));
            dom.bool_attr("data-bk-data");
            dom.text_raw(&data_json);
            dom.close();
            build_static(dom, render_html::BOOKING_WIDGET_JS)?;
            dom.close();
        }

        Block::InfoCard { intent, title, subtitle, summary, image, facts, steps, state, .. } => {
            let state_class = match state {
                RowState::Loading => " surfdoc-infocard--loading",
                RowState::Empty => " surfdoc-infocard--empty",
                _ => "",
            };
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup(&format!("surfdoc-infocard{state_class}")));
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-infocard-header"));
            if let Some(img) = image {
                dom.open("img", CloseStyle::Void);
                dom.attr("class", AttrVal::Markup("surfdoc-infocard-image"));
                dom.attr("src", AttrVal::Markup(img));
                dom.attr("alt", AttrVal::Markup(title));
                dom.close();
            }
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-infocard-info"));
            dom.open("h3", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-infocard-title"));
            dom.text_markup(title);
            dom.close();
            if !subtitle.is_empty() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-infocard-subtitle"));
                dom.text_markup(subtitle);
                dom.close();
            }
            dom.open("span", CloseStyle::Normal);
            dom.attr(
                "class",
                AttrVal::Exact {
                    raw: &format!("surfdoc-infocard-badge {}", esc_markup(intent)),
                    decoded: &format!("surfdoc-infocard-badge {intent}"),
                },
            );
            dom.text_markup(intent);
            dom.close();
            dom.close(); // .surfdoc-infocard-info
            dom.close(); // .surfdoc-infocard-header
            if !summary.is_empty() {
                dom.open("p", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-infocard-summary"));
                dom.text_markup(summary);
                dom.close();
            }
            if !steps.is_empty() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-infocard-steps"));
                for (i, step) in steps.iter().enumerate() {
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-infocard-step"));
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-infocard-step-num"));
                    dom.text_raw(&(i + 1).to_string());
                    dom.close();
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-infocard-step-text"));
                    dom.text_markup(step);
                    dom.close();
                    dom.close();
                }
                dom.close();
            } else if !facts.is_empty() {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-infocard-facts"));
                for fact in facts {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-infocard-fact-label"));
                    dom.text_markup(&fact[0]);
                    dom.close();
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-infocard-fact-value"));
                    dom.text_markup(&fact[1]);
                    dom.close();
                }
                dom.close();
            }
            dom.close();
        }

        other => return unimpl(block_kind(other)),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Render a block slice into `root` through `sink`, mirroring
/// [`crate::render_html::to_html_fragment`] semantics: blocks joined by
/// newline text nodes, prose headings wired with anchor ids (the DOM
/// equivalent of `wire_headings_and_toc` pass 1 — pass 2 is unreachable
/// because `::toc` is outside the coverage set and declines).
///
/// On `Err`, the sink may hold a partial tree — run [`coverage_check`] first
/// when rendering into a live mount.
///
/// NOTE: this renderer itself does NOT enforce the script-emitting decline —
/// the native sink builds `store`/`booking`/`gallery` fine so the
/// byte-identity corpus can compare them. The constructive gate lives in
/// [`check_coverage_blocks`] (which live-mount callers must run first).
pub fn render_blocks_dom<S: DomSink>(
    sink: &mut S,
    root: &S::Node,
    blocks: &[Block],
) -> Result<(), RenderDomError> {
    if blocks.is_empty() {
        return Ok(());
    }
    let mut dom = Dom::new(sink, root.clone());
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            dom.text_raw("\n");
        }
        build_block(&mut dom, block)?;
    }
    dom.flush_pending();
    Ok(())
}

/// Render a whole document (fragment semantics — see [`render_blocks_dom`]).
pub fn render_doc_dom<S: DomSink>(
    sink: &mut S,
    root: &S::Node,
    doc: &SurfDoc,
) -> Result<(), RenderDomError> {
    render_blocks_dom(sink, root, &doc.blocks)
}

/// Serde tag of a block whose render emits executable `<script>` text —
/// constructively unimplemented (see the module Coverage docs): `store` and
/// `booking` emit their widget scripts (and JSON data-island scripts), and
/// `gallery` ALWAYS emits its lightbox script (plus the filter script when
/// categories exist).
fn script_emitting_kind(block: &Block) -> Option<&'static str> {
    match block {
        Block::Store { .. } => Some("store"),
        Block::Booking { .. } => Some("booking"),
        Block::Gallery { .. } => Some("gallery"),
        _ => None,
    }
}

/// First script-emitting block in `blocks`, recursing through the covered
/// container kinds (`page`, `section`). Uncovered containers decline in the
/// dry run anyway, so they need no recursion here.
fn find_script_emitter(blocks: &[Block]) -> Option<&'static str> {
    for block in blocks {
        if let Some(kind) = script_emitting_kind(block) {
            return Some(kind);
        }
        match block {
            Block::Page { children, .. } | Block::Section { children, .. } => {
                if let Some(kind) = find_script_emitter(children) {
                    return Some(kind);
                }
            }
            _ => {}
        }
    }
    None
}

/// Detailed coverage check over a block slice: declines script-emitting
/// blocks (typed `script-emitting:<kind>`), then dry-runs the constructive
/// renderer against the native arena sink and reports the first
/// unimplemented construct. This is the takeover gate the wasm glue calls
/// per route (Model-2 route-scoped rendering).
pub fn check_coverage_blocks(blocks: &[Block]) -> Result<(), RenderDomError> {
    if let Some(kind) = find_script_emitter(blocks) {
        return unimpl(format!("script-emitting:{kind}"));
    }
    let mut nd = NativeDom::new();
    let root = nd.create_root();
    render_blocks_dom(&mut nd, &root, blocks)
}

/// Detailed coverage check: [`check_coverage_blocks`] over the whole doc.
pub fn check_coverage(doc: &SurfDoc) -> Result<(), RenderDomError> {
    check_coverage_blocks(&doc.blocks)
}

/// `true` when every block kind AND every markdown construct in `doc` is
/// inside the pilot coverage set AND no block emits script text — the
/// takeover gate (partial coverage law: decline the whole doc, never a dead
/// click).
pub fn coverage_check(doc: &SurfDoc) -> bool {
    check_coverage(doc).is_ok()
}

/// Test/measurement helper: constructive render through the native sink,
/// serialized to a string (byte-comparable with `to_html_fragment`).
pub fn render_fragment_string(doc: &SurfDoc) -> Result<String, RenderDomError> {
    let mut nd = NativeDom::new();
    let root = nd.create_root();
    render_blocks_dom(&mut nd, &root, &doc.blocks)?;
    Ok(nd.serialize(root))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn render_str(src: &str) -> String {
        let doc = crate::parse(src).doc;
        render_fragment_string(&doc).expect("covered")
    }

    fn html_str(src: &str) -> String {
        let doc = crate::parse(src).doc;
        crate::render_html::to_html_fragment(&doc.blocks)
    }

    // -- serializer conventions (wp1-2) ------------------------------------

    #[test]
    fn escape_conventions_differ_by_mode() {
        assert_eq!(esc_markup("a\"b<c>&\u{a0}"), "a&quot;b&lt;c&gt;&amp;\u{a0}");
        assert_eq!(esc_cmark_text("a\"b<c>&\u{a0}"), "a\"b&lt;c&gt;&amp;&nbsp;");
        assert_eq!(esc_cmark_attr("a\"b<c>&\u{a0}"), "a&quot;b<c>&amp;&nbsp;");
    }

    #[test]
    fn serializer_preserves_attr_order_and_void_styles() {
        let mut nd = NativeDom::new();
        let root = nd.create_root();
        {
            let mut dom = Dom::new(&mut nd, root);
            dom.open("img", CloseStyle::SelfCloseSpace);
            dom.attr("src", AttrVal::Markup("x.png"));
            dom.attr("alt", AttrVal::Markup("a\"b"));
            dom.close();
            dom.open("img", CloseStyle::Void);
            dom.attr("src", AttrVal::Markup("y.png"));
            dom.close();
            dom.open("input", CloseStyle::SelfClose);
            dom.attr("type", AttrVal::Markup("text"));
            dom.bool_attr("required");
            dom.close();
            dom.flush_pending();
        }
        assert_eq!(
            nd.serialize(root),
            "<img src=\"x.png\" alt=\"a&quot;b\" /><img src=\"y.png\"><input type=\"text\" required/>"
        );
    }

    #[test]
    fn round_trip_hand_built_tree_with_newline_joins() {
        let mut nd = NativeDom::new();
        let root = nd.create_root();
        {
            let mut dom = Dom::new(&mut nd, root);
            dom.open("p", CloseStyle::Normal);
            dom.text_cmark("a & b");
            dom.close();
            dom.text_raw("\n");
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("x"));
            dom.text_markup("q\"q");
            dom.close();
            dom.flush_pending();
        }
        assert_eq!(nd.serialize(root), "<p>a &amp; b</p>\n<div class=\"x\">q&quot;q</div>");
        assert_eq!(nd.text_content(root), "a & b\nq\"q");
    }

    #[test]
    fn adjacent_text_runs_merge_into_one_node() {
        let mut nd = NativeDom::new();
        let root = nd.create_root();
        {
            let mut dom = Dom::new(&mut nd, root);
            dom.open("p", CloseStyle::Normal);
            dom.text_cmark("a");
            dom.text_raw("\n");
            dom.text_cmark("b");
            dom.close();
            dom.flush_pending();
        }
        // one <p> with exactly one text child
        assert_eq!(nd.serialize(root), "<p>a\nb</p>");
        assert_eq!(nd.nodes.len(), 3); // root + p + merged text
    }

    // -- static markup builder ----------------------------------------------

    #[test]
    fn static_markup_round_trips_icon_svg() {
        let svg = crate::render_html::callout_icon_svg(crate::types::CalloutType::Warning);
        let mut nd = NativeDom::new();
        let root = nd.create_root();
        {
            let mut dom = Dom::new(&mut nd, root);
            build_static(&mut dom, svg).expect("static svg");
            dom.flush_pending();
        }
        assert_eq!(nd.serialize(root), svg);
    }

    #[test]
    fn static_markup_round_trips_widget_scaffolds() {
        for chunk in [
            crate::render_html::GALLERY_LIGHTBOX_HTML,
            crate::render_html::GALLERY_FILTER_JS,
            crate::render_html::GALLERY_LIGHTBOX_JS,
            crate::render_html::STORE_LAYOUT_HTML,
            crate::render_html::STORE_FORM_HTML,
            crate::render_html::BOOKING_CAL_HTML,
            crate::render_html::BOOKING_FORM_HTML,
            crate::render_html::STORE_WIDGET_JS,
            crate::render_html::BOOKING_WIDGET_JS,
            crate::render_html::FORM_HONEYPOT_HTML,
        ] {
            let mut nd = NativeDom::new();
            let root = nd.create_root();
            {
                let mut dom = Dom::new(&mut nd, root);
                build_static(&mut dom, chunk).expect("static chunk");
                dom.flush_pending();
            }
            assert_eq!(nd.serialize(root), chunk, "chunk drift");
        }
    }

    /// Every icon in the library must round-trip through the static builder
    /// (regression: `ry` in the lock/image icons was missing from the
    /// attribute allowlist — the debug_assert only fires when an icon is
    /// actually exercised, so exercise them all).
    #[test]
    fn static_markup_round_trips_every_icon() {
        for name in crate::icons::available_icons() {
            let svg = crate::icons::get_icon(name).expect("listed icon exists");
            let mut nd = NativeDom::new();
            let root = nd.create_root();
            {
                let mut dom = Dom::new(&mut nd, root);
                build_static(&mut dom, svg).unwrap_or_else(|e| panic!("icon {name}: {e}"));
                dom.flush_pending();
            }
            assert_eq!(nd.serialize(root), svg, "icon {name} drifted");
        }
    }

    /// Features cards with icons (incl. the `ry`-bearing lock/image icons)
    /// stay byte-identical to the string renderer.
    #[test]
    fn features_with_icons_byte_identity() {
        let src = "::features[cols=2]\n### Secure {icon=lock}\nbody\n\n### Visual {icon=image}\nbody\n::\n";
        let doc = crate::parse(src).doc;
        match render_fragment_string(&doc) {
            Ok(rendered) => {
                // Guard against a vacuous pass: the icons must actually have
                // rendered (both carry the `ry` attribute this test pins).
                assert!(
                    rendered.contains("ry=\""),
                    "test no longer exercises an ry-bearing icon"
                );
                assert_eq!(
                    rendered,
                    crate::render_html::to_html_fragment(&doc.blocks),
                    "features icon drift"
                );
            }
            Err(e) => panic!("features with icons must be covered: {e}"),
        }
    }

    #[test]
    fn decode_entities_table() {
        assert_eq!(decode_entities("&times;&#8249;&#8250;&#10005; &amp;x").unwrap(), "\u{d7}\u{2039}\u{203a}\u{2715} &x");
        assert_eq!(decode_entities("a & b &unknown; c").unwrap(), "a & b &unknown; c");
    }

    // -- URL rules ----------------------------------------------------------

    #[test]
    fn href_encoding_matches_pulldown() {
        assert_eq!(href_encode("https://a/b 2.jpg"), "https://a/b%202.jpg");
        assert_eq!(href_encode("https://x.com/a?b=1&c='q'"), "https://x.com/a?b=1&c='q'");
        assert_eq!(href_encode("/x<y>[z]"), "/x%3Cy%3E%5Bz%5D");
    }

    #[test]
    fn url_scheme_filtering_matches_ammonia() {
        assert!(url_kept("https://x.com"));
        assert!(url_kept("mailto:a@b.c"));
        assert!(url_kept("/relative"));
        assert!(url_kept("relative.png"));
        assert!(!url_kept("javascript:alert(1)"));
        assert!(!url_kept("data:text/html,hi"));
    }

    // -- markdown census (wp1-5) ---------------------------------------------

    #[test]
    fn markdown_byte_identity_basics() {
        for src in [
            "plain text with \"quotes\" & 'apostrophes'",
            "# Head & \"Q\"\n\npara\n\n- a & b\n- [l](http://x)\n\n1. one\n2. two",
            "[ext](https://x.com/a?b=1&c=2) and [rel](/about) and [mail](mailto:a@b.c)",
            "[bad](javascript:alert(1)) and [data](data:text/html,hi)",
            "![alt \"q\"](img.png) and ![](https://a/b%202.jpg)",
            "**bold** *ital* and [t](https://x.com \"my title\")",
            "auto <https://x.com> link and email <a@b.c>",
            "- item one\n  - nested\n- item two",
            "- loose one\n\n- loose two",
            "hard  \nbreak",
            "text with \u{a0}nbsp and — dash · dot",
            "## Our Menu {#our-menu}",
            "# One\n\n# One\n\n## one",
            "5. five\n6. six",
        ] {
            assert_eq!(render_str(src), html_str(src), "markdown drift for {src:?}");
        }
    }

    #[test]
    fn markdown_declines_uncovered_constructs() {
        for src in [
            "a | b\n--- | ---\n1 | 2",          // table (needs pipes at line start? see below)
            "| a | b |\n| --- | --- |\n| 1 | 2 |",
            "~~strike~~",
            "`code span`",
            "```\nfenced\n```",
            "> quote",
            "<div>raw html</div>",
            "inline <b>html</b>",
            "---",
            "- [ ] task",
        ] {
            let doc = crate::parse(src).doc;
            // Only assert decline when the construct actually parses as
            // markdown (the table-ish first case may parse as plain text —
            // then identity must hold instead).
            match render_fragment_string(&doc) {
                Err(RenderDomError::Unimplemented(_)) => {}
                Ok(rendered) => assert_eq!(
                    rendered,
                    crate::render_html::to_html_fragment(&doc.blocks),
                    "covered but drifted: {src:?}"
                ),
            }
        }
    }

    #[test]
    fn slug_dedupe_and_explicit_anchors_match_string_pass() {
        let src = "# Alpha\n\n## Alpha\n\n## Alpha {#alpha}\n\n### \u{1f600}\n\n## Beta {#not valid}\n";
        assert_eq!(render_str(src), html_str(src));
    }

    // -- coverage (wp1-3) ----------------------------------------------------

    #[test]
    fn coverage_true_for_covered_kinds() {
        let src = "::site[domain=\"x.io\"]\nname: X\n::\n\n::page[route=\"/\" title=\"Home\"]\n# Hi\n::\n";
        let doc = crate::parse(src).doc;
        assert!(coverage_check(&doc), "{:?}", check_coverage(&doc));
    }

    /// Script-emitting blocks are constructively unimplemented (typed
    /// decline) even though the native sink can still render them for the
    /// byte-identity corpus.
    #[test]
    fn coverage_declines_script_emitting_kinds() {
        for (src, want) in [
            (
                "::store[currency=\"$\"]\n- Print | $25\n::\n",
                "script-emitting:store",
            ),
            (
                "::booking\n### Tattoo\n- 2026-09-01: 10:00\n::\n",
                "script-emitting:booking",
            ),
            (
                "::gallery\n- x.png\n::\n",
                "script-emitting:gallery",
            ),
            // Nested inside a covered container: still found.
            (
                "::section\n::gallery\n- x.png\n::\n::\n",
                "script-emitting:gallery",
            ),
        ] {
            let doc = crate::parse(src).doc;
            assert!(!coverage_check(&doc), "must decline: {src:?}");
            match check_coverage(&doc) {
                Err(RenderDomError::Unimplemented(k)) => {
                    assert_eq!(k, want, "typed decline for {src:?}");
                }
                other => panic!("expected script-emitting decline, got {other:?}"),
            }
            // Native identity path still renders (or the fixture doesn't
            // parse to the expected kind — then the decline above is wrong).
            let rendered = render_fragment_string(&doc).expect("native sink renders");
            assert_eq!(rendered, crate::render_html::to_html_fragment(&doc.blocks));
        }
    }

    /// Image fallbacks are the `data-img-fallback` data attribute — never an
    /// inline `onerror` handler (TrustedScript sink), and `onerror` stays off
    /// the allowlist.
    #[test]
    fn image_fallback_is_data_attribute_not_onerror() {
        assert!(!attr_allowed("onerror"), "onerror must stay off the allowlist");
        assert!(attr_allowed("data-img-fallback"));
        let src = "::figure[src=\"a.png\" alt=\"a\"]\n::\n";
        let rendered = render_str(src);
        assert!(rendered.contains("data-img-fallback=\"hide\""), "{rendered}");
        assert!(!rendered.contains("onerror"), "{rendered}");
        assert_eq!(rendered, html_str(src), "figure fallback parity");
    }

    #[test]
    fn coverage_declines_uncovered_kind() {
        let doc = crate::parse("::tabs\n== Tab A\ncontent\n::\n").doc;
        assert!(!coverage_check(&doc));
        match check_coverage(&doc) {
            Err(RenderDomError::Unimplemented(k)) => {
                assert!(!k.is_empty(), "kind name should be reported");
            }
            other => panic!("expected decline, got {other:?}"),
        }
    }
}

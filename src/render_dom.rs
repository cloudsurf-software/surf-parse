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
//! The pilot implements the census of thelove222.doc.surf: `site`, `page`,
//! `hero`, `section`, `figure`, `callout`, `features`, `form`, `banner`,
//! `store`, `infocard`, `gallery`, `booking`, plus the markdown subset those
//! pages use (headings, paragraphs, bullet/ordered lists, links, images,
//! emphasis/strong, soft/hard breaks); 0.19 adds the Surfspace web-shell
//! chrome — `app-shell` (with the generated small-screen tab-bar),
//! `sidebar`, `panel` (including the right-panel Surfy drawer anatomy),
//! `tab-bar`, `toolbar` (all six item kinds), `modal` and
//! `dropdown-select`, plus the leaf kinds those surfaces compose with —
//! `style`, `summary`, `divider`, `search`, `segmented-control`,
//! `chat-input-simple`, `chip-input`, `chat-thread`, `embed`, `code`,
//! `data`, `metric`, `progress` and `pricing-table`. Any other block kind or
//! markdown construct returns a typed [`RenderDomError::Unimplemented`] so
//! the takeover can decline the document and fall back to full navigation —
//! never a dead click.
//!
//! `chart` and `diagram` are DECLINED rather than covered: both hand
//! `render_block` a pre-serialized SVG string from `crate::chart::render_svg`
//! / `crate::diagram::render_svg`, and feeding an owned string through
//! [`build_static`] would break its `&'static str` non-injection boundary.
//! They cover once those models emit direct sink calls.
//!
//! Script-emitting blocks (`store`, `booking`, `gallery` — gallery always
//! emits its lightbox script — plus every `tab-bar` and any `app-shell`
//! with a direct right-panel child) render fine through the NATIVE sink (the
//! byte-identity corpus needs them), but are CONSTRUCTIVELY unimplemented:
//! creating a `<script>` element with text is itself a TrustedScript sink
//! under `require-trusted-types-for 'script'`, so [`check_coverage`] /
//! [`coverage_check`] decline them (`Unimplemented("script-emitting:…")`)
//! and the takeover falls back to full navigation (partial-coverage law).

use std::collections::HashMap;

use crate::render_html::{
    self, escape_markdown_in_slot_markers, slugify, split_explicit_anchor,
};
use crate::limits::ParseLimits;
use crate::render_html::chart_type_str;
use crate::types::{Block, FormFieldType, PerClass, RowState, SizeClass, SurfDoc, DATA_PREVIEW_ROWS, DATA_WIDE_COLS};

/// Typed failure of the constructive DOM path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderDomError {
    /// The document contains a block kind or markdown construct outside the
    /// pilot coverage set. The payload names it (`"tabs"`, `"markdown:table"`,
    /// `"static-markup"` …).
    #[error("unimplemented for constructive DOM rendering: {0}")]
    Unimplemented(String),
    /// The document blew past a [`ParseLimits`] bound (spec §4.4). Like
    /// [`RenderDomError::Unimplemented`] this is a decline, not a sanitize:
    /// the takeover refuses the whole document and falls back to full
    /// navigation, and the server refuses to publish it at all.
    #[error("parse bounds exceeded: {0}")]
    LimitExceeded(#[from] crate::limits::LimitExceeded),
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
            // 0.19 leaf coverage: `<meter min max>` (render_html.rs:2896),
            // `<progress max>` (render_html.rs:6015) and `<th scope="col">`
            // (render_html.rs:2726). Nothing else was widened — every name
            // here is emitted by an arm in this file.
            | "max" | "min" | "scope"
            // SVG presentation attributes used by the vendored icon set and
            // static widget markup.
            | "viewBox" | "xmlns" | "fill" | "stroke" | "stroke-width" | "stroke-linecap"
            | "stroke-linejoin" | "fill-rule" | "d" | "points" | "x" | "y" | "x1" | "y1"
            | "x2" | "y2" | "cx" | "cy" | "r" | "rx" | "ry" | "opacity" | "font-size"
            | "font-family" | "font-weight" | "text-anchor" | "transform"
            // 0.19 `::diagram` / `::chart` static SVG: the arrowhead `<marker>`
            // in `defs` and the `marker-end` reference on flow edges. Measured
            // 2026-08-26 by scanning `to_html_fragment` over the web-shell
            // fixtures that carry diagrams and charts — every name here is
            // emitted by `crate::diagram` or `crate::chart`, and all six are
            // geometry/paint, never script.
            | "marker-end" | "markerWidth" | "markerHeight" | "orient" | "refX" | "refY"
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
    /// Generic addressing attributes (`data-block-id` / `aria-label`) waiting
    /// for the current block's root element — set by [`build_block`], consumed
    /// by the very next [`Dom::open_ns`]. Mirrors
    /// `render_html::inject_root_attrs`, which splices them ahead of the
    /// renderer's own attributes.
    root_attrs: Option<Vec<(&'static str, String)>>,
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
            root_attrs: None,
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
        if let Some(attrs) = self.root_attrs.take() {
            for (name, value) in attrs {
                self.attr(name, AttrVal::Markup(&value));
            }
        }
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
    build_markup(dom, src, false)
}

/// Feed markup the renderer GENERATED this call (`::diagram` / `::chart`
/// static SVG) through the sink, proven byte-identical first.
///
/// The `'static` bound on [`build_static`] is what makes that tokenizer safe:
/// it can never see author text. Generated SVG interpolates escaped author
/// labels, so it cannot carry that bound. The proof is supplied instead of
/// assumed — the markup is tokenized into a scratch [`NativeDom`], serialized
/// back, and compared to the source. Only on an exact match is it replayed
/// into the caller's sink; ANY divergence (a construct the tokenizer reads
/// differently than an HTML parser would) declines as `kind` rather than
/// building a tree that disagrees with the string renderer.
///
/// The replay is still `create_element` / `set_attribute` only — no string
/// ever reaches a browser parser — and in this mode the tokenizer additionally
/// refuses `<script>` / `<style>` tags and any attribute outside
/// [`attr_allowed`].
fn build_verified_markup<S: DomSink>(
    dom: &mut Dom<'_, S>,
    src: &str,
    kind: &str,
) -> Result<(), RenderDomError> {
    let mut probe_dom = NativeDom::new();
    let probe_root = probe_dom.create_root();
    {
        let mut probe = Dom::new(&mut probe_dom, probe_root);
        if build_markup(&mut probe, src, true).is_err() {
            return unimpl(kind);
        }
        probe.flush_pending();
    }
    if probe_dom.serialize(probe_root) != src {
        return unimpl(kind);
    }
    match build_markup(dom, src, true) {
        Ok(()) => Ok(()),
        Err(_) => unimpl(kind),
    }
}

/// Shared tokenizer behind [`build_static`] and [`build_verified_markup`].
/// `guarded` turns on the checks that only matter for generated markup:
/// no rawtext (`<script>`/`<style>`) elements, and every attribute name must
/// pass [`attr_allowed`].
fn build_markup<S: DomSink>(
    dom: &mut Dom<'_, S>,
    src: &str,
    guarded: bool,
) -> Result<(), RenderDomError> {
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
            // Generated markup never gets a rawtext element: creating a
            // <script> (or <style>) and giving it text is the TrustedScript
            // sink the constructive path exists to avoid.
            if guarded && (tag == "script" || tag == "style") {
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
                if guarded && !attr_allowed(name) {
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
                "rsquo" => Some('\u{2019}'),
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
    // GFM table cursor: `<th>` while the head row is open, `<td>` after.
    // Markdown tables cannot nest, so one flag pair is enough.
    let mut in_table_head = false;
    let mut table_body_open = false;
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

            // Fenced/indented code block. `pulldown_cmark::html` writes the
            // fence info as `class="language-…"`, but the `ammonia::clean`
            // pass in `render_html::render_markdown` strips `class` from
            // `<code>`, so the surviving bytes are a bare `<pre><code>` in
            // both cases and the language is deliberately NOT emitted here.
            Event::Start(Tag::CodeBlock(_)) => {
                if !at_newline {
                    dom.text_raw("\n");
                }
                dom.open("pre", CloseStyle::Normal);
                dom.open("code", CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::CodeBlock) => {
                dom.close();
                dom.close();
                dom.text_raw("\n");
                at_newline = true;
            }

            // GFM table. `render_html::render_markdown` splices the responsive
            // scroll container in with a post-sanitize string replace of
            // `<table>` / `</table>`, so the wrap `<div>` opens immediately
            // before the table and closes immediately after it — the trailing
            // newline `pulldown_cmark` writes stays OUTSIDE the div. Column
            // alignment is dropped on purpose: pulldown emits it as an inline
            // `style` on each cell and `ammonia` strips `style` from `<th>` /
            // `<td>`, so no alignment survives into the string renderer's
            // bytes either.
            Event::Start(Tag::Table(_)) => {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-table-wrap"));
                dom.open("table", CloseStyle::Normal);
                in_table_head = false;
                table_body_open = false;
                at_newline = false;
            }
            Event::End(TagEnd::Table) => {
                // `</tbody>` — opened by End(TableHead). A table without a
                // head row cannot come out of the GFM parser, but the close
                // is guarded so a future event shape can never unbalance the
                // element stack (the no-panic sweep covers this file).
                if table_body_open {
                    dom.close();
                    table_body_open = false;
                }
                dom.close(); // </table>
                dom.close(); // </div>
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::TableHead) => {
                dom.open("thead", CloseStyle::Normal);
                dom.open("tr", CloseStyle::Normal);
                in_table_head = true;
                at_newline = false;
            }
            Event::End(TagEnd::TableHead) => {
                dom.close(); // </tr>
                dom.close(); // </thead>
                dom.open("tbody", CloseStyle::Normal);
                dom.text_raw("\n");
                in_table_head = false;
                table_body_open = true;
                at_newline = true;
            }
            Event::Start(Tag::TableRow) => {
                dom.open("tr", CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::TableRow) => {
                dom.close();
                dom.text_raw("\n");
                at_newline = true;
            }
            Event::Start(Tag::TableCell) => {
                dom.open(if in_table_head { "th" } else { "td" }, CloseStyle::Normal);
                at_newline = false;
            }
            Event::End(TagEnd::TableCell) => {
                dom.close();
                at_newline = false;
            }
            Event::Start(Tag::Strikethrough) => return unimpl("markdown:strikethrough"),
            Event::Start(Tag::FootnoteDefinition(_)) | Event::FootnoteReference(_) => {
                return unimpl("markdown:footnote")
            }
            // `pulldown_cmark::html` writes `<code>` + body-escaped text; the
            // ammonia pass keeps `<code>` verbatim, so the raw bytes are the
            // html5ever text convention (`esc_cmark_text`) — same as any other
            // inline text run.
            Event::Code(t) => {
                dom.open("code", CloseStyle::Normal);
                if !t.is_empty() {
                    dom.text_cmark(t);
                }
                dom.close();
                at_newline = false;
            }
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
/// Emit the `data-cols*` attribute set for a per-size-class column count.
/// Byte-for-byte the same output as `render_html::per_class_cols_attr`.
fn emit_cols_attrs<S: DomSink>(dom: &mut Dom<'_, S>, c: &PerClass<u32>) {
    match c.as_uniform() {
        Some(v) => dom.attr("data-cols", AttrVal::Markup(&v.to_string())),
        None => {
            dom.attr("data-cols", AttrVal::Markup(&c.mobile.to_string()));
            dom.attr("data-cols-tablet", AttrVal::Markup(&c.tablet.to_string()));
            dom.attr("data-cols-desktop", AttrVal::Markup(&c.desktop.to_string()));
        }
    }
}

fn block_kind(b: &Block) -> String {
    serde_json::to_value(b)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_block<S: DomSink>(dom: &mut Dom<'_, S>, block: &Block) -> Result<(), RenderDomError> {
    dom.root_attrs = crate::block_meta::root_attrs(block);
    let built = build_block_inner(dom, block);
    // A block that opened no element (an empty render) must not leak its
    // attributes onto the next block's root.
    dom.root_attrs = None;
    built
}

fn build_block_inner<S: DomSink>(dom: &mut Dom<'_, S>, block: &Block) -> Result<(), RenderDomError> {
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
            // Mirror of `render_html::render_form_fields_html` — a run of
            // fields sharing a `group` value is wrapped in one fieldset.
            let mut open_group: Option<&str> = None;
            for field in fields {
                let group = field.group.as_deref();
                if group != open_group {
                    if open_group.is_some() {
                        dom.close();
                    }
                    if let Some(name) = group {
                        dom.open("fieldset", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-form-group"));
                        dom.open("legend", CloseStyle::Normal);
                        dom.text_markup(name);
                        dom.close();
                    }
                    open_group = group;
                }
                // Mirror of `render_html::render_form_field_html` — the
                // byte-identity suite pins the two together.
                if field.field_type == FormFieldType::Hidden {
                    dom.open("input", CloseStyle::SelfClose);
                    dom.attr("type", AttrVal::Markup("hidden"));
                    dom.attr("name", AttrVal::Markup(&field.name));
                    dom.attr(
                        "value",
                        AttrVal::Markup(field.placeholder.as_deref().unwrap_or("")),
                    );
                    dom.close();
                    continue;
                }
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
                    FormFieldType::Radio if !field.options.is_empty() => {
                        dom.open("div", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-form-options"));
                        for opt in &field.options {
                            dom.open("label", CloseStyle::Normal);
                            dom.attr("class", AttrVal::Markup("surfdoc-form-option"));
                            dom.open("input", CloseStyle::SelfClose);
                            dom.attr("type", AttrVal::Markup("radio"));
                            dom.attr("name", AttrVal::Markup(&field.name));
                            dom.attr("value", AttrVal::Markup(opt));
                            if field.required {
                                dom.bool_attr("required");
                            }
                            dom.close();
                            dom.text_markup(opt);
                            dom.close();
                        }
                        dom.close();
                    }
                    FormFieldType::Checkbox
                    | FormFieldType::Radio
                    | FormFieldType::Toggle
                    | FormFieldType::File => {
                        let (input_type, role) = match field.field_type {
                            FormFieldType::Radio => ("radio", None),
                            FormFieldType::File => ("file", None),
                            FormFieldType::Toggle => ("checkbox", Some("switch")),
                            _ => ("checkbox", None),
                        };
                        dom.open("input", CloseStyle::SelfClose);
                        dom.attr("type", AttrVal::Markup(input_type));
                        dom.attr("name", AttrVal::Markup(&field.name));
                        if let Some(r) = role {
                            dom.attr("role", AttrVal::Markup(r));
                        }
                        if field.required {
                            dom.bool_attr("required");
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
            if open_group.is_some() {
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
            // Mirror render_html's per-class emission exactly, or the
            // byte-identity suite breaks.
            if let Some(c) = cols {
                emit_cols_attrs(dom, c);
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
            let cols = columns.unwrap_or_else(|| PerClass::uniform(3));
            let categories: Vec<&str> = {
                let mut cats: Vec<&str> =
                    items.iter().filter_map(|i| i.category.as_deref()).collect();
                cats.sort();
                cats.dedup();
                cats
            };
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-gallery"));
            emit_cols_attrs(dom, &cols);
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

        // Mirror of the `render_html` Row arm (render_html.rs:5268). The
        // highest-census block in the /next shell (517 uses). Link rows
        // demote their controls to `role="button"` spans — interactive
        // content is invalid inside `<a>` — exactly as the string renderer
        // does; the class names and `data-action` values are unchanged so
        // runtime dispatch is identical in both backends.
        Block::Row {
            icon,
            title,
            description,
            href,
            state,
            unread,
            avatar,
            rtime,
            unread_count,
            trailing_label,
            trailing_action,
            action,
            progress,
            actions,
            ..
        } => {
            let state_class = match state {
                RowState::Loading => " surfdoc-row--loading",
                RowState::Empty => " surfdoc-row--empty",
                // 0.19.0: mirrors render_html — active state is source truth.
                RowState::Active => " is-active",
                _ => "",
            };
            let is_link = href.is_some();
            let tag = if is_link { "a" } else { "div" };
            dom.open(tag, CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup(&format!("surfdoc-row{state_class}")));
            if let Some(h) = href {
                dom.attr("href", AttrVal::Markup(h));
            }
            if let Some(a) = action {
                dom.attr("data-action", AttrVal::Markup(a));
            }
            // Same emission order as the string arm: aria-current LAST.
            if matches!(state, RowState::Active) {
                dom.attr("aria-current", AttrVal::Markup("page"));
            }

            // Lead slot: `avatar=` swaps the icon slot for a circular
            // initials avatar; "group" falls back to the users glyph.
            match avatar {
                Some(a) if a == "group" => {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr(
                        "class",
                        AttrVal::Markup("surfdoc-row-avatar surfdoc-row-avatar-group"),
                    );
                    build_static(dom, crate::icons::get_icon("users").unwrap_or(""))?;
                    dom.close();
                }
                Some(a) => {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-row-avatar"));
                    dom.text_markup(a);
                    dom.close();
                }
                None => {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-row-icon"));
                    build_static(dom, icon_svg(icon))?;
                    dom.close();
                }
            }

            dom.open("span", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-row-body"));
            dom.open("span", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-row-title"));
            dom.text_markup(title);
            dom.close();
            dom.open("span", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-row-desc"));
            dom.text_markup(description);
            dom.close();
            // D4 (0.14.1) usage bar: the fraction is clamped at parse and the
            // percent renders with at most one decimal (42 not 42.0).
            if let Some(p) = progress {
                let pct = ((p * 1000.0).round() / 10.0).clamp(0.0, 100.0);
                let pct = format!("{pct:.1}");
                let pct = pct.strip_suffix(".0").unwrap_or(&pct);
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-row-progress"));
                dom.attr("role", AttrVal::Markup("progressbar"));
                dom.attr("aria-valuemin", AttrVal::Markup("0"));
                dom.attr("aria-valuemax", AttrVal::Markup("100"));
                dom.attr("aria-valuenow", AttrVal::Markup(pct));
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-row-progress-fill"));
                dom.attr("style", AttrVal::Markup(&format!("width:{pct}%")));
                dom.close();
                dom.close();
            }
            dom.close();

            if let Some(t) = rtime {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-row-time"));
                dom.text_markup(t);
                dom.close();
            }

            // Unread: right-side dot only — an unread COUNT renders a pill
            // instead of the dot (never both).
            match unread_count {
                Some(c) => {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-row-badge"));
                    dom.attr("aria-label", AttrVal::Markup(&format!("{c} unread")));
                    dom.text_markup(&c.to_string());
                    dom.close();
                }
                None if *unread => build_static(dom, UNREAD_DOT_HTML)?,
                None => {}
            }

            if trailing_label.is_some() || trailing_action.is_some() {
                let label = trailing_label
                    .as_deref()
                    .or(trailing_action.as_deref())
                    .unwrap_or_default();
                if is_link {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("role", AttrVal::Markup("button"));
                    dom.attr("class", AttrVal::Markup("surfdoc-row-trailing"));
                } else {
                    dom.open("button", CloseStyle::Normal);
                    dom.attr("type", AttrVal::Markup("button"));
                    dom.attr("class", AttrVal::Markup("surfdoc-row-trailing"));
                }
                if let Some(a) = trailing_action {
                    dom.attr("data-action", AttrVal::Markup(a));
                }
                dom.text_markup(label);
                dom.close();
            }

            // Per-row actions replace the chevron affordance (0.12).
            if actions.is_empty() {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-row-arrow"));
                build_static(dom, ROW_ARROW_HTML)?;
                dom.close();
            } else {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-row-actions"));
                for a in actions {
                    if is_link {
                        dom.open("span", CloseStyle::Normal);
                        dom.attr("role", AttrVal::Markup("button"));
                        dom.attr("class", AttrVal::Markup("surfdoc-row-action"));
                    } else {
                        dom.open("button", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-row-action"));
                    }
                    dom.attr("data-action", AttrVal::Markup(&a.action));
                    dom.text_markup(&a.label);
                    dom.close();
                }
                dom.close();
            }

            dom.close();
        }

        // Mirror of the `render_html` SplitPane arm (render_html.rs:4949).
        // Two-plane layout: authored pane children render through the same
        // chrome-children path as sidebar/panel bodies. Empty split-panes
        // keep the historical two empty divs byte for byte.
        Block::SplitPane { ratio, back_label, back_action, left, right, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-split-pane"));
            dom.attr("data-ratio", AttrVal::Markup(ratio));
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-split-left"));
            build_chrome_children(dom, left)?;
            dom.close();
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-split-right"));
            if back_label.is_some() || back_action.is_some() {
                let label = back_label.as_deref().unwrap_or("Back");
                dom.open("button", CloseStyle::Normal);
                dom.attr("type", AttrVal::Markup("button"));
                dom.attr("class", AttrVal::Markup("surfdoc-split-back"));
                if let Some(a) = back_action {
                    dom.attr("data-action", AttrVal::Markup(a));
                }
                dom.text_markup(label);
                dom.close();
            }
            build_chrome_children(dom, right)?;
            dom.close();
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

        // ── Interactive / application chrome (C1) ─────────────────
        // Mirrors of the `render_html` arms named in each comment; the
        // attribute ORDER below is the string renderer's emission order and
        // is what the byte-identity corpus pins.

        // render_html.rs:5463 (AppShell)
        Block::AppShell { layout, adaptive, height, children, .. } => {
            let has_right_panel = children
                .iter()
                .any(|c| matches!(c, Block::Panel { position, .. } if position == "right"));
            dom.open("div", CloseStyle::Normal);
            dom.attr(
                "class",
                AttrVal::Markup(&format!("surfdoc-app-shell surfdoc-layout-{}", layout.as_str())),
            );
            if let Some(a) = adaptive {
                dom.attr("data-adaptive-mobile", AttrVal::Markup(a.mobile.as_str()));
                dom.attr("data-adaptive-tablet", AttrVal::Markup(a.tablet.as_str()));
                dom.attr("data-adaptive-desktop", AttrVal::Markup(a.desktop.as_str()));
            }
            if has_right_panel {
                dom.attr("data-panel-open", AttrVal::Markup("false"));
            }
            if children.iter().any(contains_split_pane) {
                dom.attr("data-thread", AttrVal::Markup("closed"));
            }
            if let Some(h) = height {
                dom.attr(
                    "style",
                    AttrVal::Markup(&format!("min-height:{h}px;max-height:{h}px")),
                );
            }
            build_chrome_children(dom, children)?;
            build_app_tabbar(dom, children)?;
            if has_right_panel {
                dom.open("button", CloseStyle::Normal);
                dom.attr("type", AttrVal::Markup("button"));
                dom.attr("class", AttrVal::Markup("surfdoc-panel-fab"));
                dom.attr("aria-label", AttrVal::Markup("Toggle Surfy"));
                dom.attr("aria-expanded", AttrVal::Markup("false"));
                dom.attr("aria-controls", AttrVal::Markup("surfdoc-panel-right"));
                build_static(dom, icon_svg("surfy-fin"))?;
                dom.close();
            }
            dom.close();
            // 0.19.0: the drawer-toggle behavior is RUNTIME-OWNED (spec
            // web-runtime-v1 §2 — script-emitting behavior moves to the
            // versioned runtime at P3/P4). Both backends emit markup + state
            // attributes only; no `<script>` follows the shell, so a
            // right-panel shell is constructively coverable.
        }

        // render_html.rs:5538 (Sidebar)
        Block::Sidebar { position, collapsible, width, classes, min_class, children, .. } => {
            let mut style = String::new();
            let mut width_attr: Option<String> = None;
            if let Some(w) = width {
                let (decls, attr) = per_class_px_parts("width", w);
                style = decls.join(";");
                width_attr = attr;
            }
            dom.open("aside", CloseStyle::Normal);
            dom.attr(
                "class",
                AttrVal::Markup(&format!("surfdoc-sidebar surfdoc-sidebar-{position}")),
            );
            dom.attr("data-collapsible", AttrVal::Markup(&collapsible.to_string()));
            if let Some(a) = &width_attr {
                dom.attr("data-size-class-width", AttrVal::Markup(a));
            }
            emit_size_class_attrs(dom, classes, min_class);
            if width.is_some() {
                dom.attr("style", AttrVal::Markup(&style));
            }
            build_chrome_children(dom, children)?;
            dom.close();
        }

        // render_html.rs:5556 (Panel; the `right` arm is the Surfy drawer)
        Block::Panel { position, resizable, height, desktop_only, classes, min_class, children, .. } => {
            let style = height.map(|h| format!("height:{h}px"));
            if position == "right" {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-panel surfdoc-panel-right"));
                dom.attr("id", AttrVal::Markup("surfdoc-panel-right"));
                dom.attr("role", AttrVal::Markup("complementary"));
                dom.attr("aria-hidden", AttrVal::Markup("true"));
                dom.attr("data-resizable", AttrVal::Markup(&resizable.to_string()));
                dom.attr("data-desktop-only", AttrVal::Markup(&desktop_only.to_string()));
                emit_size_class_attrs(dom, classes, min_class);
                if let Some(s) = &style {
                    dom.attr("style", AttrVal::Markup(s));
                }
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-panel-inner"));
                build_surfy_panel_children(dom, children)?;
                dom.close();
                dom.close();
            } else {
                dom.open("div", CloseStyle::Normal);
                dom.attr(
                    "class",
                    AttrVal::Markup(&format!("surfdoc-panel surfdoc-panel-{position}")),
                );
                dom.attr("data-resizable", AttrVal::Markup(&resizable.to_string()));
                dom.attr("data-desktop-only", AttrVal::Markup(&desktop_only.to_string()));
                emit_size_class_attrs(dom, classes, min_class);
                if let Some(s) = &style {
                    dom.attr("style", AttrVal::Markup(s));
                }
                build_chrome_children(dom, children)?;
                dom.close();
            }
        }

        // render_html.rs:5588 (TabBar)
        Block::TabBar { active, items, .. } => {
            dom.open("nav", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-tab-bar"));
            dom.attr("role", AttrVal::Markup("tablist"));
            for item in items {
                let is_active = active.as_ref().is_some_and(|a| a == &item.id);
                dom.open("button", CloseStyle::Normal);
                dom.attr("role", AttrVal::Markup("tab"));
                dom.attr("data-tab", AttrVal::Markup(&item.id));
                if let Some(i) = &item.icon {
                    dom.attr("data-icon", AttrVal::Markup(i));
                }
                if let Some(r) = &item.role {
                    dom.attr("data-role", AttrVal::Markup(r));
                }
                if is_active {
                    dom.attr("class", AttrVal::Markup("active"));
                }
                dom.attr("aria-selected", AttrVal::Markup(&is_active.to_string()));
                dom.text_markup(&item.label);
                if item.unread {
                    build_static(dom, UNREAD_DOT_HTML)?;
                }
                dom.close();
            }
            dom.close();
            // Script text: NATIVE-sink only — `script_emitting_kind` declines
            // every tab-bar constructively.
            build_static(dom, TAB_BAR_JS)?;
        }

        // render_html.rs:5660 (Toolbar)
        Block::Toolbar { title, title_source, items, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-toolbar"));
            if let Some(s) = title_source {
                dom.attr("data-title-source", AttrVal::Markup(s));
            }
            if let Some(t) = title {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-toolbar-title"));
                dom.text_markup(t);
                dom.close();
            }
            build_toolbar_items(dom, items)?;
            dom.close();
        }

        // render_html.rs:5695 (Modal)
        Block::Modal { name, title, width, placement, dismissible, children, .. } => {
            dom.open("dialog", CloseStyle::Normal);
            dom.bool_attr("open");
            dom.attr("class", AttrVal::Markup("surfdoc-modal"));
            dom.attr("data-name", AttrVal::Markup(name));
            dom.attr("data-placement", AttrVal::Markup(placement));
            if !*dismissible {
                dom.attr("data-dismissible", AttrVal::Markup("false"));
            }
            if let Some(w) = width {
                dom.attr("style", AttrVal::Markup(&format!("width:{w}px")));
            }
            let heading = title.as_deref().unwrap_or(name);
            dom.open("header", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-modal-header"));
            dom.open("strong", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-modal-title"));
            dom.text_markup(heading);
            dom.close();
            build_static(dom, MODAL_CLOSE_HTML)?;
            dom.close();
            build_chrome_children(dom, children)?;
            dom.close();
        }

        // render_html.rs:5722 (DropdownSelect)
        Block::DropdownSelect { label, icon, selected, align, options, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-dropdown-select"));
            dom.attr("data-align", AttrVal::Markup(align));
            dom.open("button", CloseStyle::Normal);
            dom.attr("type", AttrVal::Markup("button"));
            dom.attr("class", AttrVal::Markup("surfdoc-dropdown-trigger"));
            if let Some(i) = icon {
                dom.attr("data-icon", AttrVal::Markup(i));
            }
            if let Some(l) = label {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-dropdown-label"));
                dom.text_markup(l);
                dom.close();
            }
            if let Some(s) = selected {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-dropdown-selected"));
                dom.text_markup(s);
                dom.close();
            }
            build_static(dom, DROPDOWN_CARET_HTML)?;
            dom.close();
            build_dropdown_options(dom, selected, options)?;
            dom.close();
        }

        // -- C2 leaf coverage --------------------------------------------
        // Every arm below mirrors its `render_html` counterpart
        // byte-for-byte (class names, attribute order, whitespace framing).
        // Line references are into `src/render_html.rs`.

        // render_html.rs:2708
        Block::Data { headers, rows, caption, total, .. } => {
            // 0.19.2 preview contract — mirrors render_html.rs:2708 byte for
            // byte: class order (wrap, preview, wide), then `data-rows`,
            // then `data-cols`; the count line is the LAST child of the wrap.
            let row_count = rows.len();
            let col_count = rows
                .iter()
                .map(|r| r.len())
                .max()
                .unwrap_or(0)
                .max(headers.len());
            let preview = row_count > DATA_PREVIEW_ROWS;
            let mut wrap_class = String::from("surfdoc-table-wrap");
            if preview {
                wrap_class.push_str(" surfdoc-table-preview");
            }
            if col_count >= DATA_WIDE_COLS {
                wrap_class.push_str(" surfdoc-table-wide");
            }
            let shown = if preview { DATA_PREVIEW_ROWS } else { row_count };
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup(&wrap_class));
            if preview {
                dom.attr("data-rows", AttrVal::Markup(&row_count.to_string()));
                dom.attr("data-cols", AttrVal::Markup(&col_count.to_string()));
            }
            dom.open("table", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-data"));
            if let Some(c) = caption {
                dom.open("caption", CloseStyle::Normal);
                build_cell_markdown(dom, c)?;
                dom.close();
            }
            if !headers.is_empty() {
                dom.open("thead", CloseStyle::Normal);
                dom.open("tr", CloseStyle::Normal);
                for h in headers {
                    dom.open("th", CloseStyle::Normal);
                    dom.attr("scope", AttrVal::Markup("col"));
                    dom.attr("aria-sort", AttrVal::Markup("none"));
                    build_cell_markdown(dom, h)?;
                    dom.close();
                }
                dom.close();
                dom.close();
            }
            dom.open("tbody", CloseStyle::Normal);
            for row in rows.iter().take(shown) {
                dom.open("tr", CloseStyle::Normal);
                for cell in row {
                    build_data_cell(dom, cell)?;
                }
                dom.close();
            }
            dom.close();
            if !total.is_empty() {
                dom.open("tfoot", CloseStyle::Normal);
                dom.open("tr", CloseStyle::Normal);
                for cell in total {
                    build_data_cell(dom, cell)?;
                }
                dom.close();
                dom.close();
            }
            dom.close();
            if preview {
                dom.open("p", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-table-more"));
                dom.text_markup(&format!("{row_count} rows \u{b7} open as spreadsheet"));
                dom.close();
            }
            dom.close();
        }

        // render_html.rs:2760
        Block::Code { lang, file, highlight, content, .. } => {
            dom.open("figure", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-code"));
            let file_span = file.is_some();
            let lang_span = lang.is_some();
            if file_span || lang_span {
                dom.open("figcaption", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-code-head"));
                if let Some(f) = file {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-code-file"));
                    dom.text_markup(f);
                    dom.close();
                }
                if let Some(l) = lang {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-code-lang"));
                    dom.text_markup(&l.to_uppercase());
                    dom.close();
                }
                dom.close();
            }
            dom.open("pre", CloseStyle::Normal);
            if let Some(l) = lang {
                dom.attr("aria-label", AttrVal::Markup(&format!("{l} code")));
                dom.attr("data-lang", AttrVal::Markup(l));
            }
            dom.open("code", CloseStyle::Normal);
            if let Some(l) = lang {
                dom.attr("class", AttrVal::Markup(&format!("language-{l}")));
            }
            build_code_body(dom, content, highlight);
            dom.close();
            dom.close();
            dom.close();
        }

        // render_html.rs:2856
        Block::Metric { label, value, trend, unit, min, max, .. } => {
            let trend_text = match trend {
                Some(crate::types::Trend::Up) => ", trending up",
                Some(crate::types::Trend::Down) => ", trending down",
                Some(crate::types::Trend::Flat) => ", flat",
                None => "",
            };
            let unit_text = match unit {
                Some(u) => format!(" {u}"),
                None => String::new(),
            };
            let aria_label = format!("{label}: {value}{unit_text}{trend_text}");
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-metric-row"));
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-metric"));
            dom.attr("role", AttrVal::Markup("group"));
            dom.attr("aria-label", AttrVal::Markup(&aria_label));
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-metric-value"));
            dom.text_markup(value);
            if let Some(u) = unit {
                // The string arm prefixes the unit span with a literal space.
                dom.text_raw(" ");
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-metric-unit"));
                dom.text_markup(u);
                dom.close();
            }
            dom.close();
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-metric-label"));
            dom.text_markup(label);
            if let Some(t) = trend {
                let (cls, glyph) = match t {
                    crate::types::Trend::Up => ("surfdoc-trend surfdoc-trend--up", "\u{25B2}"),
                    crate::types::Trend::Down => ("surfdoc-trend surfdoc-trend--down", "\u{25BC}"),
                    crate::types::Trend::Flat => ("surfdoc-trend surfdoc-trend--flat", "\u{2192}"),
                };
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup(cls));
                dom.text_markup(glyph);
                dom.close();
            }
            dom.close();
            if let (Some(max_n), Some(value_n)) =
                (max.as_deref().and_then(parse_number), parse_number(value))
            {
                let min_n = min.as_deref().and_then(parse_number).unwrap_or(0.0);
                dom.open("meter", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-metric-meter"));
                dom.attr("min", AttrVal::Markup(&fmt_number(min_n)));
                dom.attr("max", AttrVal::Markup(&fmt_number(max_n)));
                dom.attr("value", AttrVal::Markup(&fmt_number(value_n)));
                dom.text_markup(value);
                dom.close();
            }
            dom.close();
            dom.close();
        }

        // render_html.rs:2918. NOTE: the string arm wraps
        // `render_inline_markdown` (which always emits its own `<p>`) in a
        // second `<p>` — a `<p>`-in-`<p>` shape an HTML parser auto-closes.
        // The constructive tree mirrors the bytes exactly; the divergence is
        // a render_html defect, pinned by the corpus parser-stability test.
        Block::Summary { content, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-summary"));
            dom.attr("role", AttrVal::Markup("doc-abstract"));
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-summary-label"));
            dom.text_markup("Summary");
            dom.close();
            build_wrapped_phrasing_or_blocks(dom, None, content)?;
            dom.close();
        }

        // render_html.rs:3172
        Block::Style { properties, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-style"));
            dom.attr("aria-hidden", AttrVal::Markup("true"));
            // Double-escaped exactly as the string arm: each pair is escaped,
            // joined, then the whole attribute value escaped again.
            let pairs: Vec<String> = properties
                .iter()
                .map(|p| format!("{}={}", esc_markup(&p.key), esc_markup(&p.value)))
                .collect();
            dom.attr("data-properties", AttrVal::Markup(&pairs.join(";")));
            dom.close();
        }

        // render_html.rs:3197
        Block::PricingTable { headers, rows, highlight, current, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-pricing"));
            dom.attr("aria-label", AttrVal::Markup("Pricing"));
            for row in rows {
                if row.is_empty() {
                    continue;
                }
                let raw_name = row[0].trim();
                let (name, featured) = if raw_name.len() >= 4
                    && raw_name.starts_with("**")
                    && raw_name.ends_with("**")
                {
                    (raw_name[2..raw_name.len() - 2].trim(), true)
                } else {
                    (raw_name, false)
                };
                let matches_tier = |want: &Option<String>| {
                    want.as_deref().is_some_and(|w| w.trim().eq_ignore_ascii_case(name))
                };
                let is_highlight = matches_tier(highlight);
                let is_current = matches_tier(current);
                let tier_cls = if featured || is_highlight {
                    "surfdoc-tier surfdoc-tier-featured"
                } else {
                    "surfdoc-tier"
                };
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup(tier_cls));
                if is_highlight {
                    dom.bool_attr("data-highlight");
                }
                if is_current {
                    dom.bool_attr("data-current");
                }
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-tier-name"));
                dom.text_markup(name);
                dom.close();
                if let Some(price) = row.get(1) {
                    let price = price.trim();
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-tier-price"));
                    if let Some(slash) = price.find('/') {
                        let (amount, suffix) = price.split_at(slash);
                        dom.text_markup(amount.trim());
                        dom.open("span", CloseStyle::Normal);
                        dom.text_markup(suffix);
                        dom.close();
                    } else {
                        dom.text_markup(price);
                    }
                    dom.close();
                }
                if row.len() > 2 {
                    dom.open("ul", CloseStyle::Normal);
                    for (i, cell) in row.iter().enumerate().skip(2) {
                        let val = cell.trim();
                        if val.is_empty() {
                            continue;
                        }
                        let header = headers.get(i).map(|h| h.trim()).unwrap_or("");
                        let bullet = if header.is_empty() {
                            val.to_string()
                        } else {
                            format!("{} {}", val, header.to_lowercase())
                        };
                        dom.open("li", CloseStyle::Normal);
                        build_cell_markdown(dom, &bullet)?;
                        dom.close();
                    }
                    dom.close();
                }
                let price_l = row.get(1).map(|p| p.trim().to_lowercase()).unwrap_or_default();
                let is_free = price_l.is_empty()
                    || price_l.contains("free")
                    || price_l.starts_with("$0")
                    || price_l == "0";
                let cta_label = if is_free { "Get started" } else { "Subscribe" };
                let cta_cls = if featured {
                    "surfdoc-tier-cta surfdoc-tier-cta-primary"
                } else {
                    "surfdoc-tier-cta surfdoc-tier-cta-secondary"
                };
                dom.open("a", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup(cta_cls));
                dom.attr("href", AttrVal::Markup("#"));
                dom.text_markup(cta_label);
                dom.close();
                dom.close();
            }
            dom.close();
        }

        // render_html.rs:3377
        Block::Embed { src, embed_type, title, width, height, .. } => {
            use crate::types::EmbedType;
            let is_generic = matches!(embed_type, None | Some(EmbedType::Generic));
            if is_generic && !src.is_empty() && height.is_some() {
                let h = height.as_deref().unwrap();
                let w = width.as_deref().unwrap_or("100%");
                dom.open("iframe", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-embed-frame"));
                dom.attr("src", AttrVal::Markup(src));
                if let Some(t) = title {
                    dom.attr("title", AttrVal::Markup(t));
                }
                dom.attr("style", AttrVal::Markup(&format!("width:{w};height:{h};border:0")));
                dom.attr("loading", AttrVal::Markup("lazy"));
                dom.close();
            } else {
                let title_text = title.as_deref().unwrap_or(src.as_str());
                let type_label = match embed_type {
                    Some(EmbedType::Map) => "map",
                    Some(EmbedType::Video) => "video",
                    Some(EmbedType::Audio) => "audio",
                    _ => "embed",
                };
                let icon = match embed_type {
                    Some(EmbedType::Video) => EMBED_ICON_VIDEO,
                    Some(EmbedType::Audio) => EMBED_ICON_AUDIO,
                    Some(EmbedType::Map) => EMBED_ICON_MAP,
                    _ => EMBED_ICON_GENERIC,
                };
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-embed"));
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-embed-icon"));
                build_static(dom, icon)?;
                dom.close();
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-embed-body"));
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-embed-title"));
                dom.text_markup(title_text);
                dom.close();
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-embed-src"));
                dom.text_markup(src);
                dom.text_raw(" \u{b7} ");
                dom.text_markup(type_label);
                dom.close();
                dom.close();
                dom.close();
            }
        }

        // render_html.rs:4064
        Block::Divider { label, .. } => match label {
            Some(text) => {
                dom.open("div", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-divider"));
                dom.attr("role", AttrVal::Markup("separator"));
                dom.open("span", CloseStyle::Normal);
                dom.text_markup(text);
                dom.close();
                dom.close();
            }
            None => {
                dom.open("hr", CloseStyle::SelfCloseSpace);
                dom.attr("class", AttrVal::Markup("surfdoc-divider-plain"));
                dom.close();
            }
        },

        // render_html.rs:4626
        Block::Search { source, placeholder, .. } => {
            let ph = placeholder.as_deref().unwrap_or("Search...");
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-search"));
            dom.attr("data-surf-source", AttrVal::Markup(source));
            dom.open("input", CloseStyle::SelfClose);
            dom.attr("type", AttrVal::Markup("search"));
            dom.attr("placeholder", AttrVal::Markup(ph));
            dom.attr("aria-label", AttrVal::Markup(ph));
            dom.attr("autocomplete", AttrVal::Markup("off"));
            dom.close();
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-search-results"));
            dom.attr("aria-live", AttrVal::Markup("polite"));
            dom.close();
            dom.close();
        }

        // render_html.rs:4904 / 2950 — both build their body by handing a
        // pre-serialized SVG STRING back from `crate::chart::render_svg` /
        // `crate::diagram::render_svg`. Neither emits any script text, so
        // both are constructible; the owned SVG goes through
        // `build_verified_markup`, which proves the tokenization byte-exact
        // against a scratch arena before a node reaches the sink rather than
        // relying on the `&'static str` bound `build_static` carries.
        Block::Chart { chart_type, source, period, title, data, .. } => {
            let type_str = chart_type_str(*chart_type);
            match data {
                // Inline data → a real deterministic SVG chart.
                Some(d) => {
                    let svg = crate::chart::render_svg(*chart_type, d, title.as_deref());
                    dom.open("figure", CloseStyle::Normal);
                    dom.attr(
                        "class",
                        AttrVal::Markup(&format!("surfdoc-chart surfdoc-chart-{type_str}")),
                    );
                    dom.attr("data-chart-type", AttrVal::Markup(type_str));
                    if let Some(t) = title {
                        dom.open("figcaption", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-chart-cap"));
                        dom.text_markup(t);
                        dom.close();
                    }
                    build_verified_markup(dom, &svg, "static-svg:chart")?;
                    dom.close();
                }
                // No inline data → the live-data mount point / static preview.
                None => {
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-chart surfdoc-chart-preview"));
                    dom.attr("data-chart-type", AttrVal::Markup(type_str));
                    dom.attr("data-surf-source", AttrVal::Markup(source));
                    if let Some(p) = period {
                        dom.attr("data-period", AttrVal::Markup(p));
                    }
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-chart-header"));
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-chart-type-label"));
                    dom.text_markup(&format!("{type_str} chart"));
                    dom.close();
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-chart-source-label"));
                    dom.text_markup(source);
                    dom.close();
                    dom.close();
                    build_static(dom, CHART_PREVIEW_SVG_HTML)?;
                    dom.close();
                }
            }
        }

        Block::Diagram { diagram_type, title, content, .. } => {
            // Mermaid-syntax bodies (sniffed, or explicit `type=mermaid`)
            // translate to the native DSL first; the prose fallback always
            // shows the AUTHOR'S source, never the translation.
            let translated = crate::mermaid_compat::translate(diagram_type, content);
            let (eff_type, eff_content) = match &translated {
                Some(t) => (t.diagram_type, t.content.as_str()),
                None => (diagram_type.as_str(), content.as_str()),
            };
            // `svg` is the rendered body when the source parsed; `None` is the
            // prose fallback (malformed DSL, or an empty/unknown type) — a
            // diagram NEVER fails the render.
            let (class, svg) = match crate::diagram::chart_alias(eff_type) {
                // Chart-alias types (pie/donut/radar/xychart) forward the body
                // to the `::chart` pipeline — same pipe-delimited table.
                Some(chart_type) => match crate::blocks::parse_chart_data(eff_content) {
                    Some(data) => (
                        format!("surfdoc-diagram surfdoc-diagram-{eff_type}"),
                        Some(crate::chart::render_svg(chart_type, &data, title.as_deref())),
                    ),
                    None => ("surfdoc-diagram surfdoc-diagram-fallback".to_string(), None),
                },
                None => match crate::diagram::parse_diagram_source(eff_type, eff_content) {
                    Ok(model) => (
                        format!("surfdoc-diagram surfdoc-diagram-{eff_type}"),
                        Some(crate::diagram::render_svg(&model, title.as_deref())),
                    ),
                    Err(_) => ("surfdoc-diagram surfdoc-diagram-fallback".to_string(), None),
                },
            };
            dom.open("figure", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup(&class));
            if let Some(t) = title {
                dom.open("figcaption", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-diagram-cap"));
                dom.text_markup(t);
                dom.close();
            }
            match svg {
                Some(svg) => build_verified_markup(dom, &svg, "static-svg:diagram")?,
                None => {
                    dom.open("pre", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-diagram-src"));
                    dom.text_markup(content);
                    dom.close();
                }
            }
            dom.close();
        }

        // render_html.rs:5627
        Block::SegmentedControl { active, size, action, segments, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-segmented-control"));
            dom.attr("role", AttrVal::Markup("radiogroup"));
            dom.attr("data-size", AttrVal::Markup(size));
            if let Some(a) = action {
                dom.attr("data-action", AttrVal::Markup(a));
            }
            // Single-select invariant: first id match wins, duplicates lose.
            let active_idx = segments
                .iter()
                .position(|seg| active.as_ref().is_some_and(|a| a == &seg.id));
            for (i, seg) in segments.iter().enumerate() {
                let is_active = active_idx == Some(i);
                let cls = if is_active {
                    "surfdoc-segment is-active"
                } else {
                    "surfdoc-segment"
                };
                dom.open("button", CloseStyle::Normal);
                dom.attr("type", AttrVal::Markup("button"));
                dom.attr("role", AttrVal::Markup("radio"));
                dom.attr("class", AttrVal::Markup(cls));
                dom.attr("data-id", AttrVal::Markup(&seg.id));
                dom.attr("aria-checked", AttrVal::Markup(if is_active { "true" } else { "false" }));
                dom.text_markup(&seg.label);
                dom.close();
            }
            dom.close();
        }

        // render_html.rs:5871
        Block::ChatThread { source, on_react, on_doc_open, messages, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-chat-thread"));
            if let Some(s) = source {
                dom.attr("data-source", AttrVal::Markup(s));
            }
            if let Some(a) = on_react {
                dom.attr("data-on-react", AttrVal::Markup(a));
            }
            if let Some(a) = on_doc_open {
                dom.attr("data-on-doc-open", AttrVal::Markup(a));
            }
            if messages.is_empty() {
                build_static(dom, CHAT_THREAD_SAMPLE_HTML)?;
            } else {
                let distinct_incoming: std::collections::BTreeSet<&str> = messages
                    .iter()
                    .filter(|m| m.side != "own")
                    .filter_map(|m| m.sender.as_deref())
                    .collect();
                let is_group = distinct_incoming.len() >= 2;
                for m in messages {
                    let own = m.side == "own";
                    let side = if own { "own" } else { "them" };
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup(&format!("surfdoc-chat-msg-row surfdoc-chat-msg-row-{side}")));
                    if let Some(sender) = &m.sender {
                        let surfy = sender == "Surfy";
                        if !own && (is_group || surfy) {
                            let cls = if surfy {
                                "surfdoc-chat-sender surfdoc-chat-sender-surfy"
                            } else {
                                "surfdoc-chat-sender"
                            };
                            dom.open("span", CloseStyle::Normal);
                            dom.attr("class", AttrVal::Markup(cls));
                            dom.text_markup(sender);
                            dom.close();
                        }
                    }
                    dom.open("div", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup(&format!("surfdoc-chat-bubble surfdoc-chat-bubble-{side}")));
                    dom.text_markup(&m.text);
                    if let Some(t) = &m.timestamp {
                        dom.open("span", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-chat-time"));
                        dom.text_markup(t);
                        dom.close();
                    }
                    dom.close();
                    if !m.reactions.is_empty() {
                        dom.open("div", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-chat-reacts"));
                        for r in &m.reactions {
                            let cls = if r.mine {
                                "surfdoc-chat-react-pill surfdoc-chat-react-pill-mine"
                            } else {
                                "surfdoc-chat-react-pill"
                            };
                            dom.open("span", CloseStyle::Normal);
                            dom.attr("class", AttrVal::Markup(cls));
                            dom.text_markup(&r.label);
                            if let Some(c) = r.count {
                                dom.open("span", CloseStyle::Normal);
                                dom.attr("class", AttrVal::Markup("surfdoc-chat-react-count"));
                                dom.text_markup(&c.to_string());
                                dom.close();
                            }
                            dom.close();
                        }
                        dom.close();
                    }
                    dom.close();
                }
            }
            dom.close();
        }

        // render_html.rs:5955
        Block::ChipInput { label, placeholder, source, on_change, chips, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-chip-input"));
            if let Some(s) = source {
                dom.attr("data-source", AttrVal::Markup(s));
            }
            if let Some(a) = on_change {
                dom.attr("data-on-change", AttrVal::Markup(a));
            }
            if let Some(l) = label {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-chip-input-label"));
                dom.text_markup(l);
                dom.close();
            }
            for chip in chips {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-chip-input-chip"));
                dom.text_markup(chip);
                dom.open("button", CloseStyle::Normal);
                dom.attr("type", AttrVal::Markup("button"));
                dom.attr("class", AttrVal::Markup("surfdoc-chip-input-remove"));
                dom.attr("aria-label", AttrVal::Markup(&format!("Remove {chip}")));
                build_static(dom, CHIP_REMOVE_ICON_HTML)?;
                dom.close();
                dom.close();
            }
            dom.open("input", CloseStyle::Void);
            dom.attr("type", AttrVal::Markup("text"));
            dom.attr("class", AttrVal::Markup("surfdoc-chip-input-field"));
            dom.attr("placeholder", AttrVal::Markup(placeholder.as_deref().unwrap_or("")));
            dom.close();
            dom.close();
        }

        // render_html.rs:5989
        Block::ChatInputSimple { placeholder, action, .. } => {
            dom.open("div", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-chat-input"));
            dom.open("input", CloseStyle::Void);
            dom.attr("type", AttrVal::Markup("text"));
            dom.attr("placeholder", AttrVal::Markup(placeholder.as_deref().unwrap_or("")));
            dom.close();
            dom.open("button", CloseStyle::Normal);
            if let Some(a) = action {
                dom.attr("data-action", AttrVal::Markup(a));
            }
            dom.text_markup("Send");
            dom.close();
            dom.close();
        }

        // render_html.rs:6001
        Block::Progress { steps, value, max, .. } => {
            if let Some(value_n) = value.as_deref().and_then(parse_number) {
                let max_n = max.as_deref().and_then(parse_number).unwrap_or(100.0);
                let pct = if max_n > 0.0 {
                    (value_n / max_n * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                };
                dom.open("progress", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-progress-bar"));
                dom.attr("value", AttrVal::Markup(&fmt_number(value_n)));
                dom.attr("max", AttrVal::Markup(&fmt_number(max_n)));
                dom.attr("aria-label", AttrVal::Markup("Progress"));
                dom.text_markup(&fmt_number(pct.round()));
                dom.text_raw("%");
                dom.close();
            } else {
                dom.open("ol", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-progress"));
                for step in steps {
                    dom.open("li", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup(&format!("surfdoc-step-{}", step.status)));
                    dom.text_markup(&step.label);
                    dom.close();
                }
                dom.close();
            }
        }

        other => return unimpl(block_kind(other)),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Leaf helpers (C2) — mirrors of the private `render_html` free functions the
// leaf arms compose with. Each names its counterpart; the byte-identity tests
// are what keep the two in step.
// ---------------------------------------------------------------------------

/// `::embed` link-card icons — mirrors of the four `&'static str` literals in
/// the `render_html::render_block` embed arm (render_html.rs:3411-3427).
const EMBED_ICON_VIDEO: &str = "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"currentColor\" aria-hidden=\"true\"><path d=\"M8 5v14l11-7z\"/></svg>";
const EMBED_ICON_AUDIO: &str = "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><path d=\"M9 18V5l12-2v13\"/><circle cx=\"6\" cy=\"18\" r=\"3\"/><circle cx=\"18\" cy=\"16\" r=\"3\"/></svg>";
const EMBED_ICON_MAP: &str = "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><polygon points=\"1 6 1 22 8 18 16 22 23 18 23 2 16 6 8 2 1 6\"/><line x1=\"8\" y1=\"2\" x2=\"8\" y2=\"18\"/><line x1=\"16\" y1=\"6\" x2=\"16\" y2=\"22\"/></svg>";
const EMBED_ICON_GENERIC: &str = "<svg width=\"20\" height=\"20\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\"><rect x=\"3\" y=\"3\" width=\"7\" height=\"7\"/><rect x=\"14\" y=\"3\" width=\"7\" height=\"7\"/><rect x=\"14\" y=\"14\" width=\"7\" height=\"7\"/><rect x=\"3\" y=\"14\" width=\"7\" height=\"7\"/></svg>";

/// `::chip-input` chip dismiss glyph (render_html.rs:5969).
const CHIP_REMOVE_ICON_HTML: &str = "<svg width=\"10\" height=\"10\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2.5\" stroke-linecap=\"round\" aria-hidden=\"true\"><line x1=\"18\" y1=\"6\" x2=\"6\" y2=\"18\"/><line x1=\"6\" y1=\"6\" x2=\"18\" y2=\"18\"/></svg>";

/// The pre-0.17 two-message sample preview an attrs-only `::chat-thread`
/// keeps (render_html.rs:5886) — inner children only; the wrapper div
/// carries the block's own data attributes.
const CHAT_THREAD_SAMPLE_HTML: &str = "<div class=\"surfdoc-chat-msg surfdoc-chat-msg-user\">How do I add a new task?</div><div class=\"surfdoc-chat-msg surfdoc-chat-msg-assistant\">Click <strong>+ New Task</strong> in the board toolbar, fill in the title, and assign an owner \u{2014} it&rsquo;ll appear in the Todo column instantly.</div>";

/// Mirror of the private `render_html::parse_number` (render_html.rs:6212).
fn parse_number(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix(['$', '\u{20ac}', '\u{a3}', '\u{a5}'])
        .unwrap_or(trimmed);
    let stripped = stripped.strip_suffix('%').unwrap_or(stripped);
    let cleaned: String = stripped.chars().filter(|c| *c != ',' && *c != '_').collect();
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Mirror of the private `render_html::fmt_number` (render_html.rs:6222).
fn fmt_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Mirror of the private `render_html::is_numeric_cell` (render_html.rs:6232).
fn is_numeric_cell(cell: &str) -> bool {
    let trimmed = cell.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut s = trimmed;
    if let Some(rest) = s.strip_prefix(['$', '\u{20ac}', '\u{a3}', '\u{a5}']) {
        s = rest;
    }
    let s = s.strip_suffix('%').unwrap_or(s);
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let cleaned: String = s.chars().filter(|c| *c != ',').collect();
    if cleaned.is_empty() {
        return false;
    }
    cleaned.chars().any(|c| c.is_ascii_digit()) && cleaned.parse::<f64>().is_ok()
}

/// One `<td>` of a `::data` table (render_html.rs:2733 / 2748).
fn build_data_cell<S: DomSink>(dom: &mut Dom<'_, S>, cell: &str) -> Result<(), RenderDomError> {
    dom.open("td", CloseStyle::Normal);
    if is_numeric_cell(cell) {
        dom.attr("class", AttrVal::Markup("num"));
    }
    build_cell_markdown(dom, cell)?;
    dom.close();
    Ok(())
}

/// Mirror of the private `render_html::render_code_with_highlights`
/// (render_html.rs:6261).
fn build_code_body<S: DomSink>(dom: &mut Dom<'_, S>, content: &str, highlight: &[String]) {
    let mut lines_to_hl: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for spec in highlight {
        let spec = spec.trim();
        if let Some((start, end)) = spec.split_once('-') {
            if let (Ok(s), Ok(e)) = (start.trim().parse::<usize>(), end.trim().parse::<usize>()) {
                let (lo, hi) = if s <= e { (s, e) } else { (e, s) };
                for n in lo..=hi {
                    lines_to_hl.insert(n);
                }
            }
        } else if let Ok(n) = spec.parse::<usize>() {
            lines_to_hl.insert(n);
        }
    }
    if lines_to_hl.is_empty() {
        dom.text_markup(content);
        return;
    }
    for (idx, line) in content.split('\n').enumerate() {
        if idx > 0 {
            dom.text_raw("\n");
        }
        if lines_to_hl.contains(&(idx + 1)) {
            dom.open("span", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-code-hl"));
            dom.text_markup(line);
            dom.close();
        } else {
            dom.text_markup(line);
        }
    }
}

/// Constructive mirror of the private `render_html::render_cell_inline_markdown`
/// (render_html.rs:1539) — the hand-rolled table/bullet inline scanner that
/// understands `[text](url)`, `**bold**`, `*italic*` and treats `«…»`
/// generation slot markers as atomic. Each `try_parse_*` counterpart is split
/// into a pure SCAN (which may fail without emitting) and an emit step, so a
/// rejected candidate leaves no partial nodes behind.
fn build_cell_markdown<S: DomSink>(
    dom: &mut Dom<'_, S>,
    input: &str,
) -> Result<(), RenderDomError> {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0usize;
    while i < len {
        // «…» generation slot marker: atomic, never scanned for markdown.
        let marker_end = if chars[i] == '\u{ab}' {
            (i + 1..len).find(|&j| chars[j] == '\u{bb}')
        } else {
            None
        };
        if let Some(close) = marker_end {
            let marker: String = chars[i..=close].iter().collect();
            dom.text_markup(&marker);
            i = close + 1;
            continue;
        }
        let link = if chars[i] == '[' { scan_cell_link(&chars, i) } else { None };
        if let Some((text, url, advance)) = link {
            dom.open("a", CloseStyle::Normal);
            if url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("mailto:")
                || url.starts_with('/')
            {
                dom.attr("href", AttrVal::Markup(&url));
            } else {
                dom.attr("href", AttrVal::Markup(&format!("/wiki/{url}")));
            }
            dom.text_markup(&text);
            dom.close();
            i += advance;
            continue;
        }
        let bold = if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            scan_cell_bold(&chars, i)
        } else {
            None
        };
        if let Some((inner, advance)) = bold {
            dom.open("strong", CloseStyle::Normal);
            build_cell_markdown(dom, &inner)?;
            dom.close();
            i += advance;
            continue;
        }
        let italic = if chars[i] == '*' && (i + 1 >= len || chars[i + 1] != '*') {
            scan_cell_italic(&chars, i)
        } else {
            None
        };
        if let Some((inner, advance)) = italic {
            dom.open("em", CloseStyle::Normal);
            build_cell_markdown(dom, &inner)?;
            dom.close();
            i += advance;
            continue;
        }
        let mut one = String::new();
        one.push(chars[i]);
        dom.text_markup(&one);
        i += 1;
    }
    Ok(())
}

/// Scan half of `render_html::try_parse_link` (render_html.rs:1611).
fn scan_cell_link(chars: &[char], pos: usize) -> Option<(String, String, usize)> {
    let len = chars.len();
    let mut j = pos + 1;
    let mut depth = 1;
    while j < len && depth > 0 {
        if chars[j] == '[' {
            depth += 1;
        }
        if chars[j] == ']' {
            depth -= 1;
        }
        j += 1;
    }
    if depth != 0 {
        return None;
    }
    let close_bracket = j - 1;
    if j >= len || chars[j] != '(' {
        return None;
    }
    let paren_start = j + 1;
    let mut k = paren_start;
    let mut paren_depth = 1;
    while k < len && paren_depth > 0 {
        if chars[k] == '(' {
            paren_depth += 1;
        }
        if chars[k] == ')' {
            paren_depth -= 1;
        }
        k += 1;
    }
    if paren_depth != 0 {
        return None;
    }
    let close_paren = k - 1;
    let text: String = chars[pos + 1..close_bracket].iter().collect();
    let url: String = chars[paren_start..close_paren].iter().collect();
    if text.is_empty() || url.is_empty() {
        return None;
    }
    Some((text, url, k - pos))
}

/// Scan half of `render_html::try_parse_bold` (render_html.rs:1661).
fn scan_cell_bold(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let len = chars.len();
    let start = pos + 2;
    let mut j = start;
    while j + 1 < len {
        if chars[j] == '*' && chars[j + 1] == '*' {
            if j == start {
                return None;
            }
            return Some((chars[start..j].iter().collect(), j + 2 - pos));
        }
        j += 1;
    }
    None
}

/// Scan half of `render_html::try_parse_italic` (render_html.rs:1682).
fn scan_cell_italic(chars: &[char], pos: usize) -> Option<(String, usize)> {
    let len = chars.len();
    let start = pos + 1;
    let mut j = start;
    while j < len {
        if chars[j] == '*' && (j + 1 >= len || chars[j + 1] != '*') {
            if j == start {
                return None;
            }
            return Some((chars[start..j].iter().collect(), j + 1 - pos));
        }
        if chars[j] == '*' && j + 1 < len && chars[j + 1] == '*' {
            return None;
        }
        j += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Chrome helpers (C1) — mirrors of the `render_html` free functions the
// app-shell family composes with. Each one names its counterpart; the
// byte-identity tests are what keep the two in step.
// ---------------------------------------------------------------------------

/// Circle fallback for unmapped row/toolbar icon names. Mirror of the
/// private `render_html::ICON_FALLBACK_CIRCLE`; the unknown-icon identity
/// test below fails the moment the two drift.
const ICON_FALLBACK_CIRCLE: &str = "<svg width=\"18\" height=\"18\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\"><circle cx=\"12\" cy=\"12\" r=\"10\"/></svg>";

/// Mirror of `render_html::row_icon_svg` (and of the toolbar/panel icon
/// lookups, which use the same registry-then-fallback rule).
fn icon_svg(name: &str) -> &'static str {
    crate::icons::get_icon(name).unwrap_or(ICON_FALLBACK_CIRCLE)
}

/// `<span class="surfdoc-unread-dot" …>` — the right-side dot shared by the
/// tab-bar items and the generated app tab-bar.
const UNREAD_DOT_HTML: &str =
    "<span class=\"surfdoc-unread-dot\" aria-label=\"Unread\"></span>";

/// `::chart` static preview body — the `&'static str` SVG scaffold the
/// no-inline-data branch of the `render_html` Chart arm pushes verbatim
/// (render_html.rs:4941-4950): axes, tick labels, a sample polyline pair and
/// the head dot. Renderer-owned, no author text.
const CHART_PREVIEW_SVG_HTML: &str = "<svg class=\"surfdoc-chart-svg\" viewBox=\"0 0 320 120\" xmlns=\"http://www.w3.org/2000/svg\" aria-label=\"Sample chart preview\"><line x1=\"32\" y1=\"8\" x2=\"32\" y2=\"100\" stroke=\"currentColor\" stroke-width=\"1\" opacity=\"0.25\"/><line x1=\"32\" y1=\"100\" x2=\"312\" y2=\"100\" stroke=\"currentColor\" stroke-width=\"1\" opacity=\"0.25\"/><text x=\"28\" y=\"104\" font-size=\"9\" fill=\"currentColor\" opacity=\"0.4\" text-anchor=\"end\">0</text><text x=\"28\" y=\"70\" font-size=\"9\" fill=\"currentColor\" opacity=\"0.4\" text-anchor=\"end\">50</text><text x=\"28\" y=\"36\" font-size=\"9\" fill=\"currentColor\" opacity=\"0.4\" text-anchor=\"end\">100</text><polyline points=\"40,80 80,60 120,70 160,40 200,55 240,30 280,45 310,20\" fill=\"none\" stroke=\"var(--accent)\" stroke-width=\"2\" stroke-linejoin=\"round\"/><polyline points=\"40,80 80,60 120,70 160,40 200,55 240,30 280,45 310,20 310,100 40,100\" fill=\"var(--accent)\" opacity=\"0.08\" stroke=\"none\"/><circle cx=\"310\" cy=\"20\" r=\"3\" fill=\"var(--accent)\"/></svg>";

/// `::row` chevron affordance — the `arrow_svg` literal in the `render_html`
/// Row arm (render_html.rs:5298). Replaced by per-row action buttons when
/// the row carries any.
const ROW_ARROW_HTML: &str = "<svg width=\"14\" height=\"14\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\"><polyline points=\"9 18 15 12 9 6\"/></svg>";

/// Modal header close control (render_html.rs:5695 arm).
const MODAL_CLOSE_HTML: &str =
    "<button type=\"button\" class=\"surfdoc-modal-close\" aria-label=\"Close\">&#10005;</button>";

/// Dropdown trigger caret (shared by the block, the toolbar item and the
/// drawer tier switcher).
const DROPDOWN_CARET_HTML: &str =
    "<span class=\"surfdoc-dropdown-caret\" aria-hidden=\"true\">&#9662;</span>";

/// Surfy drawer grounding chip (render_html::render_surfy_panel_children).
const PANEL_GROUNDING_HTML: &str = "<div class=\"surfdoc-panel-grounding\" hidden><span class=\"surfdoc-grounding-label\"></span><button type=\"button\" class=\"surfdoc-grounding-clear\" data-action=\"clearSurfyGrounding\" aria-label=\"Clear grounding\">&#10005;</button></div>";

/// Mirror of the private `render_html::contains_split_pane` — drives the
/// app-shell's `data-thread="closed"` stamp.
fn contains_split_pane(block: &Block) -> bool {
    match block {
        Block::SplitPane { .. } => true,
        Block::Section { children, .. }
        | Block::AppShell { children, .. }
        | Block::Sidebar { children, .. }
        | Block::Panel { children, .. }
        | Block::TabContent { children, .. }
        | Block::Drawer { children, .. }
        | Block::Modal { children, .. } => children.iter().any(contains_split_pane),
        _ => false,
    }
}

/// Mirror of the private `render_html::initially_active_tab`.
fn initially_active_tab(children: &[Block]) -> Option<&str> {
    let tabs: Vec<&str> = children
        .iter()
        .filter_map(|b| match b {
            Block::TabContent { tab, .. } => Some(tab.as_str()),
            _ => None,
        })
        .collect();
    if tabs.is_empty() || tabs.contains(&"preview") {
        return None;
    }
    let bar_active = children.iter().find_map(|b| match b {
        Block::TabBar { active, .. } => active.as_deref(),
        _ => None,
    });
    if let Some(a) = bar_active
        && tabs.contains(&a)
    {
        return Some(a);
    }
    Some(tabs[0])
}

/// Mirror of `render_html::size_class_attrs` — emits nothing when neither
/// attribute was authored.
fn emit_size_class_attrs<S: DomSink>(
    dom: &mut Dom<'_, S>,
    classes: &Option<Vec<SizeClass>>,
    min_class: &Option<SizeClass>,
) {
    if let Some(cs) = classes {
        let joined: Vec<&str> = cs.iter().map(|c| c.as_str()).collect();
        dom.attr("data-size-class", AttrVal::Markup(&joined.join(" ")));
    }
    if let Some(m) = min_class {
        dom.attr("data-min-size-class", AttrVal::Markup(m.as_str()));
    }
}

/// Mirror of `render_html::per_class_px` — (style declarations, optional
/// `data-size-class-width` value).
fn per_class_px_parts(prop: &str, w: &PerClass<u32>) -> (Vec<String>, Option<String>) {
    match w.as_uniform() {
        Some(v) => (vec![format!("{prop}:{v}px")], None),
        None => (
            vec![
                format!("--sc-w-mobile:{}px", w.mobile),
                format!("--sc-w-tablet:{}px", w.tablet),
                format!("--sc-w-desktop:{}px", w.desktop),
            ],
            Some(format!("{} {} {}", w.mobile, w.tablet, w.desktop)),
        ),
    }
}

/// Mirror of `render_html::render_chrome_children` — children of a chrome
/// container, with the initially visible tab-content pane marked. NOTE: the
/// active pane goes through `build_tab_content` directly, exactly like the
/// string renderer, so it carries NO block-addressing root attributes.
fn build_chrome_children<S: DomSink>(
    dom: &mut Dom<'_, S>,
    children: &[Block],
) -> Result<(), RenderDomError> {
    let active_tab = initially_active_tab(children);
    let mut activated = false;
    for child in children {
        match child {
            Block::TabContent { tab, .. } if !activated && active_tab == Some(tab.as_str()) => {
                build_tab_content(dom, child, true)?;
                activated = true;
            }
            _ => build_block(dom, child)?,
        }
    }
    Ok(())
}

/// Mirror of `render_html::render_tab_content`.
fn build_tab_content<S: DomSink>(
    dom: &mut Dom<'_, S>,
    block: &Block,
    active: bool,
) -> Result<(), RenderDomError> {
    let Block::TabContent { tab, width, align, classes, min_class, children, .. } = block else {
        return Ok(());
    };
    let class = if active {
        "surfdoc-tab-content active"
    } else {
        "surfdoc-tab-content"
    };
    let mut styles: Vec<String> = Vec::new();
    let mut width_attr: Option<String> = None;
    if let Some(w) = width {
        let (decls, attr) = per_class_px_parts("max-width", w);
        styles.extend(decls);
        width_attr = attr;
    }
    if align.as_deref() == Some("center") {
        styles.push("margin-left:auto".to_string());
        styles.push("margin-right:auto".to_string());
        styles.push("width:100%".to_string());
    }
    dom.open("div", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup(class));
    dom.attr("data-tab", AttrVal::Markup(tab));
    dom.attr("role", AttrVal::Markup("tabpanel"));
    if let Some(a) = &width_attr {
        dom.attr("data-size-class-width", AttrVal::Markup(a));
    }
    emit_size_class_attrs(dom, classes, min_class);
    if !styles.is_empty() {
        dom.attr("style", AttrVal::Markup(&styles.join(";")));
    }
    for child in children {
        build_block(dom, child)?;
    }
    dom.close();
    Ok(())
}

/// Mirror of `render_html::render_dropdown_options`.
fn build_dropdown_options<S: DomSink>(
    dom: &mut Dom<'_, S>,
    selected: &Option<String>,
    options: &[crate::types::DropdownOption],
) -> Result<(), RenderDomError> {
    dom.open("ul", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup("surfdoc-dropdown-options"));
    for opt in options {
        let sel_class = if selected.as_deref() == Some(opt.label.as_str()) {
            "surfdoc-dropdown-option is-selected"
        } else {
            "surfdoc-dropdown-option"
        };
        dom.open("li", CloseStyle::Normal);
        dom.attr("class", AttrVal::Markup(sel_class));
        if let Some(a) = &opt.action {
            dom.attr("data-action", AttrVal::Markup(a));
        }
        if let Some(i) = &opt.icon {
            dom.attr("data-icon", AttrVal::Markup(i));
        }
        dom.open("span", CloseStyle::Normal);
        dom.attr("class", AttrVal::Markup("surfdoc-dropdown-option-label"));
        dom.text_markup(&opt.label);
        dom.close();
        if let Some(d) = &opt.description {
            dom.open("span", CloseStyle::Normal);
            dom.attr("class", AttrVal::Markup("surfdoc-dropdown-option-desc"));
            dom.text_markup(d);
            dom.close();
        }
        dom.close();
    }
    dom.close();
    Ok(())
}

/// Mirror of `render_html::render_toolbar_items` — buttons, separators,
/// spacers, badges, dropdowns and text, without the toolbar wrapper.
fn build_toolbar_items<S: DomSink>(
    dom: &mut Dom<'_, S>,
    items: &[crate::types::ToolbarItem],
) -> Result<(), RenderDomError> {
    use crate::types::ToolbarItem;
    for item in items {
        match item {
            ToolbarItem::Button { label, action, icon, style, toggled, avatar, aria_label, .. } => {
                let mut cls = match style {
                    Some(s) => format!(" surfdoc-toolbar-btn-{s}"),
                    None => String::new(),
                };
                if *toggled {
                    cls.push_str(" surfdoc-toolbar-btn--toggled");
                }
                let glyph = icon.as_deref().map(icon_svg);
                let label_text = label.as_deref().unwrap_or("");
                let icon_only = glyph.is_some() && label_text.is_empty() && avatar.is_none();
                if icon_only {
                    cls.push_str(" surfdoc-toolbar-btn--icon");
                }
                dom.open("button", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup(&format!("surfdoc-toolbar-btn{cls}")));
                if let Some(a) = action {
                    dom.attr("data-action", AttrVal::Markup(a));
                }
                if *toggled {
                    dom.attr("aria-pressed", AttrVal::Markup("true"));
                }
                if icon_only {
                    let aria = aria_label
                        .as_deref()
                        .or(action.as_deref())
                        .or(icon.as_deref())
                        .unwrap_or_default();
                    dom.attr("aria-label", AttrVal::Markup(aria));
                } else if label_text.is_empty() && avatar.is_some() {
                    // A label-less avatar chip has no accessible name either
                    // (the initial badge is aria-hidden).
                    let name = aria_label
                        .as_deref()
                        .or(action.as_deref())
                        .or(icon.as_deref())
                        .or(avatar.as_deref())
                        .unwrap_or_default();
                    dom.attr("aria-label", AttrVal::Markup(name));
                }
                if !icon_only
                    && let Some(a) = avatar
                {
                    dom.open("span", CloseStyle::Normal);
                    dom.attr("class", AttrVal::Markup("surfdoc-toolbar-avatar"));
                    dom.attr("aria-hidden", AttrVal::Markup("true"));
                    dom.text_markup(a);
                    dom.close();
                }
                if let Some(g) = glyph {
                    build_static(dom, g)?;
                }
                if !icon_only {
                    dom.text_markup(label_text);
                }
                dom.close();
            }
            ToolbarItem::Separator => {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-toolbar-separator"));
                dom.close();
            }
            ToolbarItem::Spacer => {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-toolbar-spacer"));
                dom.close();
            }
            ToolbarItem::Badge { value, color } => {
                let cls = match color {
                    Some(c) => format!("surfdoc-badge surfdoc-badge-{c}"),
                    None => "surfdoc-badge".to_string(),
                };
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup(&cls));
                dom.text_markup(value);
                dom.close();
            }
            ToolbarItem::Dropdown { label, options, action } => {
                dom.open("div", CloseStyle::Normal);
                dom.attr(
                    "class",
                    AttrVal::Markup("surfdoc-dropdown-select surfdoc-toolbar-dropdown"),
                );
                dom.attr("data-align", AttrVal::Markup("start"));
                dom.open("button", CloseStyle::Normal);
                dom.attr("type", AttrVal::Markup("button"));
                dom.attr("class", AttrVal::Markup("surfdoc-dropdown-trigger"));
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-dropdown-label"));
                dom.text_markup(label);
                dom.close();
                build_static(dom, DROPDOWN_CARET_HTML)?;
                dom.close();
                dom.open("ul", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-dropdown-options"));
                if let Some(opts) = options {
                    for opt in opts.split('|').filter(|o| !o.is_empty()) {
                        dom.open("li", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-dropdown-option"));
                        if let Some(a) = action {
                            dom.attr("data-action", AttrVal::Markup(a));
                        }
                        dom.open("span", CloseStyle::Normal);
                        dom.attr("class", AttrVal::Markup("surfdoc-dropdown-option-label"));
                        dom.text_markup(opt);
                        dom.close();
                        dom.close();
                    }
                }
                dom.close();
                dom.close();
            }
            ToolbarItem::Text { value, size, .. } => {
                dom.open("span", CloseStyle::Normal);
                dom.attr("class", AttrVal::Markup("surfdoc-toolbar-text"));
                if let Some(s) = size {
                    dom.attr("style", AttrVal::Markup(&format!("font-size:{s}px")));
                }
                dom.text_markup(value);
                dom.close();
            }
        }
    }
    Ok(())
}

/// One generated tab-bar item, read straight off a sidebar `row`:
/// (icon, title, unread, href, action).
type TabbarRow<'a> = (&'a str, &'a str, bool, Option<&'a str>, Option<&'a str>);

/// Mirror of `render_html::render_app_tabbar` — the small-screen tab-bar
/// generated from the shell sidebar's nav rows (ruling R-A).
fn build_app_tabbar<S: DomSink>(
    dom: &mut Dom<'_, S>,
    children: &[Block],
) -> Result<(), RenderDomError> {
    let side_children = children.iter().find_map(|c| match c {
        Block::Sidebar { children, .. } => Some(children),
        _ => None,
    });
    let Some(side_children) = side_children else {
        return Ok(());
    };
    let mut rows: Vec<TabbarRow<'_>> = Vec::new();
    for child in side_children {
        match child {
            Block::Divider { .. } => break,
            Block::Row { icon, title, unread, href, action, .. } => {
                rows.push((icon, title, *unread, href.as_deref(), action.as_deref()));
            }
            _ => {}
        }
    }
    if rows.is_empty() {
        return Ok(());
    }
    let active_tab = initially_active_tab(children);
    let active_idx = rows
        .iter()
        .position(|(_, title, _, _, _)| active_tab == Some(slugify(title).as_str()))
        .unwrap_or(0);
    dom.open("nav", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup("surfdoc-app-tabbar"));
    dom.attr("aria-label", AttrVal::Markup("Primary"));
    for (i, (icon, title, unread, href, action)) in rows.iter().enumerate() {
        let is_active = i == active_idx;
        let cls = if is_active {
            "surfdoc-app-tabbar-item is-active"
        } else {
            "surfdoc-app-tabbar-item"
        };
        dom.open("button", CloseStyle::Normal);
        dom.attr("type", AttrVal::Markup("button"));
        dom.attr("class", AttrVal::Markup(cls));
        dom.attr("data-tab", AttrVal::Markup(&slugify(title)));
        if let Some(h) = href {
            dom.attr("data-href", AttrVal::Markup(h));
        }
        if let Some(a) = action {
            dom.attr("data-action", AttrVal::Markup(a));
        }
        dom.attr(
            "aria-current",
            AttrVal::Markup(if is_active { "page" } else { "false" }),
        );
        dom.open("span", CloseStyle::Normal);
        dom.attr("class", AttrVal::Markup("surfdoc-app-tabbar-icon"));
        build_static(dom, icon_svg(icon))?;
        if *unread {
            build_static(dom, UNREAD_DOT_HTML)?;
        }
        dom.close();
        dom.open("span", CloseStyle::Normal);
        dom.attr("class", AttrVal::Markup("surfdoc-app-tabbar-label"));
        dom.text_markup(title);
        dom.close();
        dom.close();
    }
    dom.close();
    Ok(())
}

/// Mirror of `render_html::render_panel_tier_dropdown` — the drawer head
/// tier switcher (selected value only, toolbar-dropdown clothing).
fn build_panel_tier_dropdown<S: DomSink>(
    dom: &mut Dom<'_, S>,
    block: &Block,
) -> Result<(), RenderDomError> {
    let Block::DropdownSelect { label, selected, options, .. } = block else {
        return Ok(());
    };
    let title = selected.as_deref().or(label.as_deref()).unwrap_or("");
    dom.open("div", CloseStyle::Normal);
    dom.attr(
        "class",
        AttrVal::Markup("surfdoc-dropdown-select surfdoc-toolbar-dropdown"),
    );
    dom.attr("data-align", AttrVal::Markup("start"));
    dom.open("button", CloseStyle::Normal);
    dom.attr("type", AttrVal::Markup("button"));
    dom.attr("class", AttrVal::Markup("surfdoc-dropdown-trigger"));
    dom.open("span", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup("surfdoc-dropdown-selected"));
    dom.text_markup(title);
    dom.close();
    build_static(dom, DROPDOWN_CARET_HTML)?;
    dom.close();
    build_dropdown_options(dom, selected, options)?;
    dom.close();
    Ok(())
}

/// Mirror of `render_html::render_surfy_panel_children` — the right panel's
/// ruled drawer anatomy (head row, grounding chip, body, composer last).
fn build_surfy_panel_children<S: DomSink>(
    dom: &mut Dom<'_, S>,
    children: &[Block],
) -> Result<(), RenderDomError> {
    let mut head_dropdown: Option<&Block> = None;
    let mut head_toolbar: Option<&[crate::types::ToolbarItem]> = None;
    let mut composer: Option<(Option<&str>, Option<&str>)> = None;
    let mut body: Vec<&Block> = Vec::new();
    for child in children {
        match child {
            Block::DropdownSelect { .. } if head_dropdown.is_none() => {
                head_dropdown = Some(child);
            }
            Block::Toolbar { items, .. } if head_toolbar.is_none() => {
                head_toolbar = Some(items);
            }
            Block::ChatInputSimple { placeholder, action, .. } if composer.is_none() => {
                composer = Some((placeholder.as_deref(), action.as_deref()));
            }
            _ => body.push(child),
        }
    }

    dom.open("div", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup("surfdoc-panel-head"));
    dom.open("span", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup("surfdoc-panel-fin"));
    dom.attr("aria-hidden", AttrVal::Markup("true"));
    build_static(dom, icon_svg("surfy-fin"))?;
    dom.close();
    if let Some(dd) = head_dropdown {
        build_panel_tier_dropdown(dom, dd)?;
    }
    if let Some(items) = head_toolbar {
        build_toolbar_items(dom, items)?;
    }
    dom.close();

    build_static(dom, PANEL_GROUNDING_HTML)?;

    dom.open("div", CloseStyle::Normal);
    dom.attr("class", AttrVal::Markup("surfdoc-panel-body"));
    let active_tab = initially_active_tab(children);
    let mut activated = false;
    for child in body {
        match child {
            Block::TabContent { tab, .. } if !activated && active_tab == Some(tab.as_str()) => {
                build_tab_content(dom, child, true)?;
                activated = true;
            }
            _ => build_block(dom, child)?,
        }
    }
    dom.close();

    if let Some((placeholder, action)) = composer {
        let ph = placeholder.unwrap_or("");
        dom.open("div", CloseStyle::Normal);
        dom.attr("class", AttrVal::Markup("surfdoc-chat-input"));
        dom.open("button", CloseStyle::Normal);
        dom.attr("type", AttrVal::Markup("button"));
        dom.attr("class", AttrVal::Markup("surfdoc-chat-attach"));
        dom.attr("data-action", AttrVal::Markup("attachToSurfy"));
        dom.attr("aria-label", AttrVal::Markup("Attach"));
        build_static(dom, icon_svg("plus"))?;
        dom.close();
        dom.open("input", CloseStyle::Void);
        dom.attr("type", AttrVal::Markup("text"));
        dom.attr("placeholder", AttrVal::Markup(ph));
        dom.close();
        dom.open("button", CloseStyle::Normal);
        if let Some(a) = action {
            dom.attr("data-action", AttrVal::Markup(a));
        }
        dom.text_markup("Send");
        dom.close();
        dom.close();
    }
    Ok(())
}

/// The tab-bar switch script, verbatim from the `render_html` TabBar arm
/// (render_html.rs:5623). Emitted through the NATIVE sink only: creating a
/// `<script>` element with text is a TrustedScript sink, so the coverage
/// gate declines any doc that carries a tab-bar.
const TAB_BAR_JS: &str = r#"<script>document.querySelectorAll('.surfdoc-tab-bar').forEach(bar=>{bar.querySelectorAll('[role="tab"]').forEach(btn=>{btn.onclick=()=>{const tab=btn.dataset.tab;const parent=bar.parentElement||document;bar.querySelectorAll('[role="tab"]').forEach(b=>{b.classList.remove('active');b.setAttribute('aria-selected','false')});btn.classList.add('active');btn.setAttribute('aria-selected','true');parent.querySelectorAll('.surfdoc-tab-content').forEach(tc=>{if(tc.dataset.tab===tab){tc.classList.add('active');tc.style.display='block'}else{tc.classList.remove('active');tc.style.display='none'}})}})})</script>"#;

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
        // Chrome (C1 → 0.19.0): every tab-bar emits its switch script. The
        // app-shell right-panel drawer no longer does — its toggle behavior
        // is runtime-owned from 0.19.0 (markup + state attributes only), so
        // a right-panel shell is coverable.
        Block::TabBar { .. } => Some("tab-bar"),
        _ => None,
    }
}

/// First script-emitting block in `blocks`, recursing through EVERY covered
/// container kind. The list must stay in step with the container arms in
/// `build_block_inner`: a container that renders its children but is missing
/// here hides a `<script>`-emitting descendant from the gate, and the doc
/// would be reported constructible while the string renderer emits script
/// text (`::gallery` inside a `::tab-content` did exactly that). Uncovered
/// containers decline in the dry run anyway.
fn find_script_emitter(blocks: &[Block]) -> Option<&'static str> {
    for block in blocks {
        if let Some(kind) = script_emitting_kind(block) {
            return Some(kind);
        }
        match block {
            Block::Page { children, .. }
            | Block::Section { children, .. }
            | Block::AppShell { children, .. }
            | Block::Sidebar { children, .. }
            | Block::Panel { children, .. }
            | Block::TabContent { children, .. }
            | Block::Drawer { children, .. }
            | Block::Modal { children, .. } => {
                if let Some(kind) = find_script_emitter(children) {
                    return Some(kind);
                }
            }
            Block::SplitPane { left, right, .. } => {
                if let Some(kind) = find_script_emitter(left).or_else(|| find_script_emitter(right))
                {
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

/// [`check_coverage_blocks`] with the DoS bounds applied first
/// (spec/web-runtime-v1.surf §4.4). Bounds run BEFORE the dry run, so a
/// hostile tree is refused without being walked into an arena DOM.
///
/// The bound-free [`check_coverage_blocks`] is unchanged and still the gate
/// for callers that bounded their input elsewhere; live-mount callers on
/// untrusted source should prefer this one, or
/// [`check_source_coverage`] when they still hold the source text.
pub fn check_coverage_blocks_with_limits(
    blocks: &[Block],
    limits: &ParseLimits,
) -> Result<(), RenderDomError> {
    limits.check_blocks(blocks)?;
    check_coverage_blocks(blocks)
}

/// [`check_coverage_blocks_with_limits`] over a whole document.
pub fn check_coverage_with_limits(
    doc: &SurfDoc,
    limits: &ParseLimits,
) -> Result<(), RenderDomError> {
    check_coverage_blocks_with_limits(&doc.blocks, limits)
}

/// Source-taking gate: the `max_source_bytes` bound is the only one that can
/// be checked before parsing, so this is the entry point the publish path and
/// the client takeover share. Order is bytes → parse → depth/count →
/// coverage dry run, and the parsed document is handed back so neither side
/// parses twice.
///
/// Returns the same typed decline as the block-level gates: bounds surface as
/// [`RenderDomError::LimitExceeded`], uncovered constructs as
/// [`RenderDomError::Unimplemented`].
pub fn check_source_coverage(
    source: &str,
    limits: &ParseLimits,
) -> Result<SurfDoc, RenderDomError> {
    limits.check_source_bytes(source)?;
    let doc = crate::parse(source).doc;
    check_coverage_with_limits(&doc, limits)?;
    Ok(doc)
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
                // The renderer applies no DoS bounds — those live on the
                // coverage/source gates — so this arm is unreachable.
                Err(e @ RenderDomError::LimitExceeded(_)) => {
                    panic!("render path must not enforce bounds: {e}")
                }
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

    // -- chrome coverage (C1) ------------------------------------------------

    /// Full shell shape: sidebar nav rows + divider + hub row (the generated
    /// tab-bar's source), topbar, tab-content pane, right panel.
    const FULL_SHELL: &str = "::app-shell[layout=sidebar-main-panel height=600]\n:::sidebar[position=left width=240]\n::::toolbar\n- text[value=\"Surfspace\" size=22]\n::::\n::::row[icon=doc href=#]\nDocs\n::::\n::::row[icon=knowledge href=# unread=true]\nMessages\n::::\n::::divider\n::::\n::::row[icon=settings href=#]\nSettings\n::::\n:::\n:::toolbar\n- button[label=\"Search\" icon=search action=openSearch]\n:::\n:::tab-content[tab=main]\nPane\n:::\n::";

    /// Shell whose children are all inside this round's coverage set AND
    /// which carries the right panel (drawer anatomy + FAB + toggle script).
    const RIGHT_PANEL_SHELL: &str = "::app-shell[layout=sidebar-main-panel height=720]\n:::sidebar[position=left width=240]\n::::toolbar\n- text[value=\"Surfspace\" size=22]\n::::\n:::\n:::toolbar\n- spacer\n- button[label=\"Surfy\" icon=surfy-fin action=toggleSurfy]\n:::\n:::panel[name=surfy position=right width=360]\n::::dropdown-select[name=surfy-tier icon=surfy-fin label=\"Surfy Standard\" selected=\"Standard\" action=switchTier]\n- \"Standard\" description=\"Balanced quality and speed\" action=switchTierStandard\n- \"Max\" description=\"Highest quality\" action=switchTierMax\n::::\n::::toolbar\n- spacer\n- button[label=\"Chats\" action=openSurfyChats]\n::::\n::::chat-input-simple[placeholder=\"Ask Surfy\" action=sendToSurfy]\n::::\n:::\n::";

    /// Every chrome kind in the coverage set, rendered through the
    /// constructive path, byte-compared with the string renderer.
    #[test]
    fn chrome_kinds_byte_identity() {
        // (source, a class the CHROME arm — not a markdown fallback — must
        // have emitted; the pair keeps a mis-parsed fixture from passing
        // vacuously).
        for (src, needle) in [
            // sidebar — uniform width, then the 0.18 per-class width +
            // size-class hooks.
            (
                "::sidebar[position=left collapsible=true width=240]\n::",
                "surfdoc-sidebar-left\" data-collapsible=\"true\" style=\"width:240px\"",
            ),
            (
                "::sidebar[position=left width=\"0 72 260\"]\n:::toolbar\n- text[value=\"Surfspace\" size=22]\n:::\n::",
                "data-size-class-width=\"0 72 260\"",
            ),
            // panel — bottom/left keep the generic shape.
            (
                "::panel[position=bottom resizable=true height=160 desktop-only=true]\n::",
                "surfdoc-panel-bottom\" data-resizable=\"true\" data-desktop-only=\"true\"",
            ),
            (
                "::panel[position=left]\n:::toolbar\n- spacer\n:::\n::",
                "surfdoc-toolbar-spacer",
            ),
            // toolbar — every item kind.
            (
                "::toolbar[title=\"Messages\" title-source=thread.display_name]\n- button[label=\"Run\" action=run style=primary toggled=true]\n- button[icon=filter action=open_filter]\n- button[label=\"cloudsurf\" avatar=\"C\" action=switch_workspace]\n- button[avatar=\"C\" action=openHub]\n- text[value=\"Surfspace\" size=22]\n- separator\n- spacer\n- badge[value=\"Live\" color=green]\n- badge[value=\"3\"]\n- dropdown[options=\"A|B\" label=\"Sort\"]\n::",
                "surfdoc-toolbar-btn--icon",
            ),
            // toolbar text that must survive markup escaping.
            (
                "::toolbar[title=\"Docs & <files>\"]\n- text[value=\"a & b\"]\n::",
                "Docs &amp; &lt;files&gt;",
            ),
            // modal.
            (
                "::modal[name=confirm title=\"Confirm\" width=480 placement=centered dismissible=false]\n:::toolbar\n- button[label=\"OK\" action=ok]\n:::\n::",
                "<dialog open class=\"surfdoc-modal\" data-name=\"confirm\"",
            ),
            (
                "::modal[name=hub placement=anchored]\n::",
                "surfdoc-modal-close",
            ),
            // dropdown-select.
            (
                "::dropdown-select[label=\"Sort\" icon=arrow selected=\"Newest\" align=right]\n- \"Newest\" description=\"Most recent\" icon=clock action=sort_newest\n- \"Oldest\"\n::",
                "surfdoc-dropdown-option is-selected",
            ),
            // app-shell without a right panel or nav rows (no script, no
            // generated tab-bar).
            (
                "::app-shell[layout=sidebar-main-panel height=600]\n:::sidebar[position=left width=240]\n::::toolbar\n- text[value=\"Surfspace\" size=22]\n::::\n:::\n:::toolbar\n- button[label=\"Search\" icon=search action=openSearch]\n:::\n::",
                "surfdoc-app-shell surfdoc-layout-sidebar-main-panel",
            ),
            (
                "::app-shell[layout=tabs]\n:::toolbar\n- spacer\n:::\n::",
                "surfdoc-layout-tabs",
            ),
        ] {
            let rendered = render_str(src);
            assert!(rendered.contains(needle), "{needle} missing from {rendered}");
            assert_eq!(rendered, html_str(src), "chrome drift for {src:?}");
            assert!(
                coverage_check(&crate::parse(src).doc),
                "chrome fixture must be covered: {src:?}"
            );
        }
    }

    /// 0.19.0 regression (TASK-267): a shell with a direct right panel no
    /// longer emits the drawer-toggle script — behavior is runtime-owned —
    /// so it COVERS, renders byte-identically to `render_html`, and carries
    /// no `<script>` at all. This is the gate that arms constructive
    /// navigation on the composed /next shell (which always carries the
    /// Surfy right panel). Pre-fix code declines this doc with
    /// `script-emitting:app-shell` — this test FAILS there.
    #[test]
    fn right_panel_shell_covers_without_script() {
        let doc = crate::parse(RIGHT_PANEL_SHELL).doc;
        assert!(coverage_check(&doc), "right-panel shell must cover at 0.19.0");
        let rendered = render_fragment_string(&doc).expect("constructive render");
        assert!(!rendered.contains("<script>"), "no script text anywhere");
        assert!(rendered.contains("data-panel-open=\"false\""), "state contract stays");
        assert!(rendered.contains("surfdoc-panel-fab"), "FAB markup stays");
        assert_eq!(
            rendered,
            crate::render_html::to_html_fragment(&doc.blocks),
            "right-panel shell byte identity"
        );
    }

    /// Every tab-bar (switch script) emits script TEXT: constructively
    /// declined with a typed kind, while the native sink still matches the
    /// string renderer byte for byte — the same split store/booking/gallery
    /// already live under.
    #[test]
    fn script_emitting_chrome_declines_but_native_matches() {
        for (src, want) in [
            (
                "::tab-bar[active=preview]\n- preview \"Preview\" {icon=eye unread=true}\n- edit \"Edit\"\n::",
                "script-emitting:tab-bar",
            ),
        ] {
            let doc = crate::parse(src).doc;
            let rendered = render_fragment_string(&doc).expect("native sink renders");
            assert!(rendered.contains("<script>"), "fixture must emit script text");
            assert_eq!(
                rendered,
                crate::render_html::to_html_fragment(&doc.blocks),
                "script-emitting chrome drift: {src:?}"
            );
            assert!(!coverage_check(&doc), "must decline: {src:?}");
            match check_coverage(&doc) {
                Err(RenderDomError::Unimplemented(k)) => assert_eq!(k, want),
                other => panic!("expected script-emitting decline, got {other:?}"),
            }
        }
    }

    /// The script-emitter search recurses through the chrome containers, so a
    /// gallery buried in a shell still declines the whole document.
    #[test]
    fn nested_script_emitter_found_through_chrome_containers() {
        for src in [
            "::app-shell[layout=sidebar-main-panel]\n:::sidebar[position=left]\n::::gallery\n- x.png\n::::\n:::\n::",
            "::modal[name=m]\n:::gallery\n- x.png\n:::\n::",
            "::app-shell[layout=sidebar-main-panel]\n:::panel[position=bottom]\n::::store[currency=\"$\"]\n- Print | $25\n::::\n:::\n::",
        ] {
            match check_coverage(&crate::parse(src).doc) {
                Err(RenderDomError::Unimplemented(k)) => {
                    assert!(k.starts_with("script-emitting:"), "got {k} for {src:?}");
                }
                other => panic!("expected nested decline, got {other:?}"),
            }
        }
    }

    /// The small-screen tab-bar generated from the sidebar's nav rows
    /// (ruling R-A). The shell's `row` children are outside this round's
    /// coverage set, so the byte comparison is scoped to the generated nav —
    /// taken from the string renderer's own output.
    #[test]
    fn generated_app_tabbar_byte_identity() {
        let doc = crate::parse(FULL_SHELL).doc;
        let Block::AppShell { children, .. } = &doc.blocks[0] else {
            panic!("fixture must start with the app-shell");
        };
        let mut nd = NativeDom::new();
        let root = nd.create_root();
        {
            let mut dom = Dom::new(&mut nd, root);
            build_app_tabbar(&mut dom, children).expect("tab-bar builds");
            dom.flush_pending();
        }
        let got = nd.serialize(root);
        let html = crate::render_html::to_html_fragment(&doc.blocks);
        let start = html
            .find("<nav class=\"surfdoc-app-tabbar\"")
            .expect("string renderer emits the generated tab-bar");
        let end = start
            + html[start..].find("</nav>").expect("generated tab-bar closes")
            + "</nav>".len();
        assert_eq!(got, &html[start..end], "generated tab-bar drift");
        assert!(got.contains("surfdoc-unread-dot"), "fixture must exercise the dot");
        assert!(got.contains("is-active"), "fixture must exercise the active item");
    }

    /// Unknown icon names keep the circle fallback — the one glyph mirrored
    /// from `render_html` rather than resolved through the registry, so this
    /// is the drift guard on that copy.
    #[test]
    fn unknown_chrome_icon_falls_back_identically() {
        let src = "::toolbar\n- button[icon=no-such-icon-name action=x]\n::";
        let rendered = render_str(src);
        assert!(
            rendered.contains("<circle cx=\"12\" cy=\"12\" r=\"10\"/>"),
            "fallback circle expected: {rendered}"
        );
        assert_eq!(rendered, html_str(src), "icon fallback drift");
    }


    // -- leaf coverage (C2) --------------------------------------------------

    /// Every leaf kind this round covers, in the variants that exercise each
    /// optional branch of its `render_html` arm. `(source, marker class the
    /// LEAF arm — not a markdown fallback — must produce)`.
    const LEAF_CASES: &[(&str, &str)] = &[
        // ::style — double-escaped `data-properties` pair list.
        ("::style\naccent: #2E8AD8\nheading-font: surf-display\n::\n", "surfdoc-style"),
        ("::style\naccent: \"#fff\" & <x>\n::\n", "surfdoc-style"),
        // ::summary — see the p-in-p note on the arm.
        ("::summary\nA short abstract with **bold** and a [link](https://example.org).\n::\n", "surfdoc-summary"),
        ("::summary\nQuotes \" and <angles> & amps.\n::\n", "surfdoc-summary"),
        // ::divider — labelled and plain (the plain form is an `<hr … />`).
        ("::divider\n::\n", "surfdoc-divider-plain"),
        ("::divider[label=\"Yesterday & <b>\"]\n::\n", "surfdoc-divider"),
        // ::search — explicit and defaulted placeholder (the default also
        // fills `aria-label`).
        ("::search[source=\"docs\" placeholder=\"Find a doc\"]\n::\n", "surfdoc-search"),
        ("::search[source=\"a&b\"]\n::\n", "surfdoc-search"),
        // ::segmented-control — active pill, and the duplicate-id
        // single-select invariant (first match wins).
        ("::segmented-control[name=v action=setView active=list size=compact]\n- list \"List\"\n- board \"Board\"\n::\n", "surfdoc-segment is-active"),
        ("::segmented-control[active=a size=md]\n- a \"One\"\n- a \"Two\"\n::\n", "surfdoc-segment"),
        // ::chat-input-simple — with and without `action=`.
        ("::chat-input-simple[placeholder=\"Message\" action=\"send\"]\n::\n", "surfdoc-chat-input"),
        ("::chat-input-simple\n::\n", "surfdoc-chat-input"),
        // ::chip-input — chips + dismiss glyph, and the bare shape.
        ("::chip-input[label=\"To:\" placeholder=\"Add\" source=\"people\" on-change=\"chg\"]\n- Ada\n- Grace\n::\n", "surfdoc-chip-input-chip"),
        ("::chip-input\n::\n", "surfdoc-chip-input"),
        // ::chat-thread — the attrs-only sample preview, authored messages
        // (incl. reactions + the group sender lead) and the named Surfy lead.
        ("::chat-thread[source=\"t1\" on-react=\"r\" on-doc-open=\"d\"]\n::\n", "surfdoc-chat-msg-assistant"),
        ("::chat-thread\n- them[sender=\"Danny\" time=\"1:42 PM\"] Update finished\n- own[time=\"1:44 PM\"] Yes \" & <ok>\n- them[sender=\"Ada\" time=\"2:00 PM\" reactions=\"Like:2:mine|Wow\"] Hi\n::\n", "surfdoc-chat-react-pill-mine"),
        ("::chat-thread\n- them[sender=\"Surfy\" time=\"9:00\"] Hello\n::\n", "surfdoc-chat-sender-surfy"),
        // ::embed — the iframe branch and all four link-card icons.
        ("::embed[src=\"https://example.org/x\" height=\"400\" title=\"Policy\"]\n::\n", "surfdoc-embed-frame"),
        ("::embed[src=\"https://example.org/x\" type=\"video\" title=\"A clip\"]\n::\n", "surfdoc-embed-icon"),
        ("::embed[src=\"https://a.example/x\" type=\"audio\"]\n::\n", "surfdoc-embed-icon"),
        ("::embed[src=\"https://maps.example/x?a=1&b=2\" type=\"map\"]\n::\n", "surfdoc-embed-icon"),
        ("::embed[src=\"https://g.example/x\"]\n::\n", "surfdoc-embed-icon"),
        // ::code — head (file + lang badge), highlight spans, bare body.
        ("::code[lang=\"rust\" file=\"src/main.rs\"]\nfn main() {}\n::\n", "surfdoc-code-head"),
        ("::code[lang=\"rust\" highlight=\"2,4-5\"]\nline1 <a>\nline2\nline3\nline4\nline5\n::\n", "surfdoc-code-hl"),
        ("::code\nplain & <text>\n::\n", "surfdoc-code"),
        // ::data — plain grid, then caption + tfoot total + numeric cells +
        // cell-level inline markdown (bold, wiki link).
        ("::data\nName | Count\nAda | 3\nGrace | 12\n::\n", "surfdoc-data"),
        ("::data[caption=\"Q3 & Q4\"]\nName | Amount\n**Ada** | $1,204.50\n[Doc](guide) | -12%\ntotal: All | $1,204.50\n::\n", "surfdoc-table-wrap"),
        // ::data 0.19.2 preview contract — 21 rows (capped tbody + count
        // line + data-rows/data-cols) and an eight-column wide wrap.
        ("::data\nName | Count\nr1 | 1\nr2 | 2\nr3 | 3\nr4 | 4\nr5 | 5\nr6 | 6\nr7 | 7\nr8 | 8\nr9 | 9\nr10 | 10\nr11 | 11\nr12 | 12\nr13 | 13\nr14 | 14\nr15 | 15\nr16 | 16\nr17 | 17\nr18 | 18\nr19 | 19\nr20 | 20\nr21 | 21\ntotal: All | 231\n::\n", "surfdoc-table-preview"),
        ("::data\nc1 | c2 | c3 | c4 | c5 | c6 | c7 | c8\n1 | 2 | 3 | 4 | 5 | 6 | 7 | 8\n::\n", "surfdoc-table-wide"),
        // ::metric — plain, trend arrows, unit span, and the gauge <meter>.
        ("::metric[label=\"Docs\" value=\"1,204\"]\n::\n", "surfdoc-metric"),
        ("::metric[label=\"Uptime\" value=\"98\" unit=\"%\" trend=\"up\" max=\"100\"]\n::\n", "surfdoc-metric-meter"),
        ("::metric[label=\"Errors\" value=\"3\" trend=\"down\" unit=\"per day\"]\n::\n", "surfdoc-trend--down"),
        ("::metric[label=\"Flat\" value=\"7\" trend=\"flat\" min=\"1\" max=\"10\"]\n::\n", "surfdoc-trend--flat"),
        // ::progress — numeric <progress> bar and the step list.
        ("::progress[value=\"40\" max=\"80\"]\n::\n", "surfdoc-progress-bar"),
        ("::progress\n- done: Draft\n- active: Review\n- pending: Ship\n::\n", "surfdoc-progress"),
        // ::pricing-table — featured/highlight/current marks, `/seat` price
        // suffix span, feature bullets, free vs paid CTA label.
        ("::pricing-table[highlight=\"Team\" current=\"Free\"]\nTier | Price | Seats | Docs\nFree | Free | 1 |\n**Team** | $6.99/seat | 25 | Unlimited\nMax | $200 | 100 | Unlimited\n::\n", "surfdoc-tier-featured"),
    ];

    /// Byte identity, per leaf kind, against the string renderer.
    #[test]
    fn leaf_kinds_byte_identity() {
        for (src, marker) in LEAF_CASES {
            let expected = html_str(src);
            assert!(
                expected.contains(marker),
                "fixture does not reach the leaf arm ({marker}): {expected}"
            );
            let doc = crate::parse(src).doc;
            let rendered =
                render_fragment_string(&doc).unwrap_or_else(|e| panic!("{src:?} declined: {e}"));
            assert_eq!(rendered, expected, "leaf drift for {src:?}");
            assert!(coverage_check(&doc), "leaf kind must pass coverage: {src:?}");
        }
    }

    /// Numeric character references in the mirrored chrome constants must
    /// survive the constructive path with BOTH forms intact: the serialized
    /// bytes keep `&#NNNN;` (byte identity with the string renderer) while the
    /// DOM text node holds the decoded glyph (what a browser would show).
    #[test]
    fn numeric_entities_keep_raw_bytes_and_decoded_text() {
        for (html, glyph) in [
            (MODAL_CLOSE_HTML, '\u{2715}'),   // &#10005; — modal close
            (DROPDOWN_CARET_HTML, '\u{25BE}'), // &#9662;  — dropdown caret
        ] {
            let mut nd = NativeDom::new();
            let root = nd.create_root();
            {
                let mut dom = Dom::new(&mut nd, root);
                build_static(&mut dom, html).expect("static chunk");
                dom.flush_pending();
            }
            assert_eq!(nd.serialize(root), html, "raw byte drift");
            assert_eq!(
                nd.text_content(root),
                glyph.to_string(),
                "decoded DOM text drift"
            );
        }
        // The same pair, reached through the real block arms.
        let modal = render_str("::modal[name=m title=\"T\"]\nbody\n::");
        assert!(modal.contains("&#10005;"), "{modal}");
        let dropdown = render_str("::dropdown-select[name=d label=\"L\"]\n- \"A\"\n::");
        assert!(dropdown.contains("&#9662;"), "{dropdown}");
    }

    /// `::chart` and `::diagram` build their bodies from a pre-serialized SVG
    /// STRING (`crate::chart::render_svg` / `crate::diagram::render_svg`).
    /// Neither emits script text, so as of 0.19 both are COVERED: the owned
    /// SVG goes through `build_verified_markup`, which proves the
    /// tokenization byte-exact against a scratch arena before any node
    /// reaches the sink. This test replaces the 0.18 `static-svg:*` decline
    /// pins — the four sources that used to decline must now cover AND
    /// render byte-identically to `render_html`.
    #[test]
    fn chart_and_diagram_are_covered_and_byte_identical() {
        for src in [
            "::chart type=\"line\" source=\"m\"\n::\n",
            "::chart type=\"bar\"\nA | 1\nB | 2\n::\n",
            "::diagram type=\"flow\"\nA -> B\n::\n",
            "::diagram type=\"pie\"\nA | 1\n::\n",
            // Prose fallback: an unusable body must still build (a diagram
            // never fails the render).
            "::diagram type=\"flow\"\n(((\n::\n",
        ] {
            let doc = crate::parse(src).doc;
            if let Err(e) = check_coverage(&doc) {
                panic!("expected coverage for {src:?}, got {e:?}");
            }
            assert!(coverage_check(&doc), "{src:?}");
            assert_eq!(
                render_fragment_string(&doc).expect("native sink renders"),
                doc.to_html_fragment(),
                "{src:?}: constructive DOM drifted from render_html"
            );
        }
    }

    /// The verified-markup gate is what makes the non-`'static` SVG path
    /// safe: markup the tokenizer cannot reproduce byte-for-byte, or that
    /// carries a rawtext element or an attribute outside `attr_allowed`,
    /// declines under the caller's typed kind instead of building a tree
    /// that disagrees with the string renderer.
    #[test]
    fn verified_markup_declines_what_it_cannot_reproduce() {
        for bad in [
            "<script>alert(1)</script>",
            "<style>a{}</style>",
            "<svg onload=\"x\"></svg>",
            "<svg><g></svg>",
            "<svg>",
            "</svg>",
        ] {
            let mut nd = NativeDom::new();
            let root = nd.create_root();
            let mut dom = Dom::new(&mut nd, root);
            match build_verified_markup(&mut dom, bad, "static-svg:diagram") {
                Err(RenderDomError::Unimplemented(k)) => {
                    assert_eq!(k, "static-svg:diagram", "{bad:?}")
                }
                other => panic!("expected typed decline for {bad:?}, got {other:?}"),
            }
        }
        // The happy path still round-trips.
        let good = "<svg viewBox=\"0 0 2 2\"><rect x=\"0\" y=\"0\"/><text>a&amp;b</text></svg>";
        let mut nd = NativeDom::new();
        let root = nd.create_root();
        {
            let mut dom = Dom::new(&mut nd, root);
            build_verified_markup(&mut dom, good, "static-svg:diagram").expect("covered");
            dom.flush_pending();
        }
        assert_eq!(nd.serialize(root), good);
    }

    /// The allowlist widened by exactly three names in 0.18 and six more for
    /// the 0.19 `::diagram` / `::chart` SVG — each one emitted by a real arm.
    /// Anything else stays off until a grep proves emission.
    #[test]
    fn allowlist_widened_only_for_emitted_leaf_attributes() {
        for name in ["max", "min", "scope"] {
            assert!(attr_allowed(name), "{name} must be allowlisted");
        }
        // Emission proof: the three names reach real markup.
        let meter = render_str("::metric[label=\"L\" value=\"5\" min=\"1\" max=\"10\"]\n::\n");
        assert!(meter.contains("<meter class=\"surfdoc-metric-meter\" min=\"1\" max=\"10\""), "{meter}");
        let bar = render_str("::progress[value=\"40\" max=\"80\"]\n::\n");
        assert!(bar.contains("max=\"80\""), "{bar}");
        let table = render_str("::data\nName | Count\nAda | 3\n::\n");
        assert!(table.contains("<th scope=\"col\" aria-sort=\"none\">"), "{table}");

        // 0.19: the six SVG marker names, and their emission proof — the
        // vendored web-shell sources that actually carry `::diagram` /
        // `::chart` (the same four scanned on 2026-08-26 to derive the list).
        // Rendering them here is what keeps the allowlist honest: drop an arm
        // that emits one of these and the proof fails.
        let mut svg_markup = String::new();
        for rel in [
            "tests/fixtures/web-shell/surfaces/deploy-lane.surf",
            "tests/fixtures/web-shell/surfaces/tasks.surf",
            "tests/fixtures/web-shell/surfaces/workspace-settings.surf",
            "tests/fixtures/web-shell/hostile/rawtext.surf",
        ] {
            let path = format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"));
            let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            svg_markup.push_str(&crate::parse(&src).doc.to_html_fragment());
        }
        for name in ["marker-end", "markerWidth", "markerHeight", "orient", "refX", "refY"] {
            assert!(attr_allowed(name), "{name} must be allowlisted");
            assert!(
                svg_markup.contains(&format!("{name}=\"")),
                "{name} is allowlisted but no arm emits it"
            );
        }
        // Neighbours that were NOT widened.
        for name in [
            "onerror", "onclick", "srcdoc", "sandbox", "form", "list", "step", "pattern",
            "content", "http-equiv", "integrity", "nonce", "allow", "referrerpolicy",
        ] {
            assert!(!attr_allowed(name), "{name} must stay off the allowlist");
        }
    }
}

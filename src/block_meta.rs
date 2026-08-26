//! Block metadata side table — the authored `id=` and `label=` of any block.
//!
//! ## Why a side table
//!
//! `id=` and `label=` are tolerated on *every* block kind (0.18.1): a template
//! file stamps `id=` on each block so a later editing API can address one
//! block instead of rewriting the page. Carrying them in the AST would mean
//! either a new field on all 109 `Block` variants (the drift guards forbid
//! widening every variant) or a field on `SurfDoc` (64 struct-literal
//! construction sites, plus a serde shape change for every JSON/WASM
//! consumer). Both are worse than a side table.
//!
//! So: [`record`] is called once per directive from
//! [`crate::blocks::resolve_block`] — the single funnel every top-level
//! (`parse.rs`) and nested (`parse_page_children`) block passes through — and
//! the renderers read it back by [`crate::types::Span`], the one identity a
//! typed block keeps.
//!
//! ## Staleness
//!
//! The table is thread-local and holds exactly one document: [`begin`] clears
//! it at the top of every [`crate::parse::parse`]. Renderers that hold the
//! source ([`activate_for`]) compare a hash of it against the hash recorded at
//! parse time and read the table only on a match — so rendering a document
//! parsed *before* some other document on the same thread emits no `id`/`label`
//! rather than the wrong one. Fragment renderers, which get a block slice and
//! no source, read the live table best-effort.
//!
//! ## The `label=` gate
//!
//! Six implemented directives already spend `label=` on their own semantics —
//! `::metric`, `::cta`, `::divider`, `::action`, `::dropdown-select` and
//! `::chip-input` (`::countdown` too, still `planned`). On those, `label=` is
//! the metric's caption or the button's text, NOT an accessibility name, so it
//! must not also become `aria-label` on the block root. The gate is the
//! registry itself — a directive is label-typed exactly when
//! `spec/blocks.toml` lists `label` among its `attributes` — so a future row
//! that adopts `label=` is covered without touching this file. The decision is
//! made once, at record time, where the directive name is still in hand.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::types::{AttrValue, Attrs, Block, Span};

/// The authored addressing attributes of one block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockMeta {
    /// `id=` — emitted as `data-block-id` on the block root.
    pub id: Option<String>,
    /// `label=` — emitted as `aria-label` on the block root. Always `None` for
    /// the directives that spend `label=` on their own semantics (see the
    /// module docs).
    pub label: Option<String>,
}

impl BlockMeta {
    /// `true` when neither attribute was authored (nothing to emit).
    pub fn is_empty(&self) -> bool {
        self.id.is_none() && self.label.is_none()
    }
}

/// Span identity: byte extent, which is unique within one document.
type Key = (usize, usize);

#[derive(Default)]
struct Table {
    /// Hash of the source the entries were parsed from.
    source_key: u64,
    /// The (line-ending-normalised) source itself — what nested-child span
    /// derivation reads (see [`content_start`]).
    source: String,
    /// Whether lookups currently resolve (see [`activate_for`]).
    active: bool,
    entries: BTreeMap<Key, (Span, BlockMeta)>,
}

thread_local! {
    static TABLE: RefCell<Table> = RefCell::new(Table {
        source_key: 0,
        source: String::new(),
        active: true,
        entries: BTreeMap::new(),
    });
}

fn source_hash(source: &str) -> u64 {
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    h.finish()
}

fn key(span: Span) -> Key {
    (span.start_offset, span.end_offset)
}

/// Start a new document: drop the previous document's entries and pin the
/// table to `source`. Called from [`crate::parse::parse`] before resolution.
pub fn begin(source: &str) {
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        t.entries.clear();
        t.source_key = source_hash(source);
        t.source = source.to_string();
        t.active = true;
    });
}

/// Where a container's content starts inside the document being parsed:
/// `(byte offset, 1-based line)` of the first line after `parent`'s opener.
///
/// Nested children (`parse_page_children`) are parsed from the container's
/// content string, which knows nothing about its position in the document.
/// This anchors them: `parent` must be a real span into the source the table
/// was pinned to (it starts with `::` and lies inside it). Anything else —
/// a hand-built block, a foreign source — returns `None` and the caller
/// keeps the placeholder zero span, exactly as before 0.18.1.
///
/// Why it matters: the table is keyed by span, so two nested siblings that
/// shared the placeholder span would collapse onto one entry and the last
/// `id=` recorded would be emitted on every one of them.
pub(crate) fn content_start(parent: Span) -> Option<(usize, usize)> {
    TABLE.with(|t| {
        let t = t.borrow();
        let src = t.source.as_str();
        if parent.end_offset <= parent.start_offset || parent.end_offset > src.len() {
            return None;
        }
        let head = src.get(parent.start_offset..parent.end_offset)?;
        if !head.starts_with("::") {
            return None;
        }
        let nl = head.find('\n')?;
        Some((parent.start_offset + nl + 1, parent.start_line + 1))
    })
}

/// Record the addressing attributes of one directive. No-op when neither is
/// present, so the common block costs two `BTreeMap` probes and nothing else.
pub fn record(span: Span, name: &str, attrs: &Attrs) {
    let id = attr_text(attrs, "id");
    let label = if label_is_typed(name) {
        None
    } else {
        attr_text(attrs, "label")
    };
    if id.is_none() && label.is_none() {
        return;
    }
    TABLE.with(|t| {
        t.borrow_mut()
            .entries
            .insert(key(span), (span, BlockMeta { id, label }));
    });
}

/// The recorded metadata for `span`, if the table is active and holds it.
pub fn lookup(span: Span) -> Option<BlockMeta> {
    TABLE.with(|t| {
        let t = t.borrow();
        if !t.active {
            return None;
        }
        t.entries.get(&key(span)).map(|(_, meta)| meta.clone())
    })
}

/// Root attributes to emit for `block`, in emission order, already gated.
/// `None` when the block carried neither attribute.
pub fn root_attrs(block: &Block) -> Option<Vec<(&'static str, String)>> {
    let meta = lookup(block.span())?;
    let mut out = Vec::with_capacity(2);
    if let Some(id) = meta.id {
        out.push(("data-block-id", id));
    }
    if let Some(label) = meta.label {
        out.push(("aria-label", label));
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Every recorded entry in span order — the FFI's view of the table.
pub fn snapshot() -> Vec<(Span, BlockMeta)> {
    TABLE.with(|t| {
        let t = t.borrow();
        if !t.active {
            return Vec::new();
        }
        t.entries.values().cloned().collect()
    })
}

/// RAII guard restoring the previous active flag — see [`activate_for`].
pub struct MetaScope {
    previous: bool,
}

impl Drop for MetaScope {
    fn drop(&mut self) {
        let previous = self.previous;
        TABLE.with(|t| t.borrow_mut().active = previous);
    }
}

/// Resolve lookups for the lifetime of the guard only when the table was
/// filled by parsing `source`. Renderers that hold a whole document call this
/// at their entrypoint (the [`crate::citation`] context precedent).
pub fn activate_for(source: &str) -> MetaScope {
    TABLE.with(|t| {
        let mut t = t.borrow_mut();
        let previous = t.active;
        t.active = t.source_key == source_hash(source);
        MetaScope { previous }
    })
}

fn attr_text(attrs: &Attrs, name: &str) -> Option<String> {
    let raw = match attrs.get(name)? {
        AttrValue::String(s) => s.clone(),
        AttrValue::Number(n) => n.to_string(),
        AttrValue::Bool(b) => b.to_string(),
        AttrValue::Null => return None,
    };
    if raw.trim().is_empty() { None } else { Some(raw) }
}

/// `true` when `spec/blocks.toml` gives this directive a typed `label=`.
fn label_is_typed(name: &str) -> bool {
    crate::lint::blocks_with_typed_label().contains(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_label_directives_come_from_the_registry() {
        for name in ["metric", "cta", "divider", "action", "dropdown-select", "chip-input"] {
            assert!(label_is_typed(name), "{name} spends label= on its own semantics");
        }
        for name in ["callout", "hero", "form", "banner", "store"] {
            assert!(!label_is_typed(name), "{name} has no typed label=");
        }
    }

    #[test]
    fn record_and_lookup_round_trip() {
        begin("::banner[id=announce label=\"Site notice\"]\nhi\n::\n");
        let span = Span { start_line: 1, end_line: 3, start_offset: 0, end_offset: 40 };
        let mut attrs = Attrs::new();
        attrs.insert("id".into(), AttrValue::String("announce".into()));
        attrs.insert("label".into(), AttrValue::String("Site notice".into()));
        record(span, "banner", &attrs);
        let meta = lookup(span).expect("recorded");
        assert_eq!(meta.id.as_deref(), Some("announce"));
        assert_eq!(meta.label.as_deref(), Some("Site notice"));
    }

    #[test]
    fn typed_label_block_records_no_aria_label() {
        begin("::metric[label=Tests value=694]\n");
        let span = Span { start_line: 1, end_line: 1, start_offset: 0, end_offset: 32 };
        let mut attrs = Attrs::new();
        attrs.insert("label".into(), AttrValue::String("Tests".into()));
        record(span, "metric", &attrs);
        assert_eq!(lookup(span), None, "label= alone on ::metric records nothing");
    }

    #[test]
    fn a_new_parse_drops_the_previous_document() {
        begin("first");
        let span = Span { start_line: 1, end_line: 1, start_offset: 0, end_offset: 5 };
        let mut attrs = Attrs::new();
        attrs.insert("id".into(), AttrValue::String("a".into()));
        record(span, "callout", &attrs);
        assert!(lookup(span).is_some());
        begin("second");
        assert_eq!(lookup(span), None);
    }

    #[test]
    fn activate_for_declines_a_foreign_source() {
        begin("mine");
        let span = Span { start_line: 1, end_line: 1, start_offset: 0, end_offset: 4 };
        let mut attrs = Attrs::new();
        attrs.insert("id".into(), AttrValue::String("a".into()));
        record(span, "callout", &attrs);
        {
            let _scope = activate_for("theirs");
            assert_eq!(lookup(span), None, "a doc parsed elsewhere reads nothing");
        }
        let _scope = activate_for("mine");
        assert!(lookup(span).is_some(), "the parsed source resolves again");
    }
}

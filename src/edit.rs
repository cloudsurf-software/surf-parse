//! Block edits by id over the SOURCE (0.28.0) — the editing API the block
//! addressing of 0.18.1 was built for.
//!
//! ## The model
//!
//! A site (or any document) is edited one block at a time, by the block's
//! authored `id=` — never by rewriting the whole page. Every function here
//! is a pure function over the source string: it parses once, finds the
//! block's byte span through [`crate::block_meta`] (the span identity every
//! typed block keeps) and splices the source. Nothing is re-rendered, nothing
//! outside the addressed block moves, and an unknown id is an ERROR — never a
//! silent no-op, because the caller (a Surfy tool, a kit) is about to record
//! the result as a new version of the document.
//!
//! ## Addressing
//!
//! Ids are unique WITHIN A PAGE (lint L043), not across a site: the home page
//! and the about page may both have a `hero`. So every verb takes an optional
//! `route`: with one, the id resolves inside that `::page`; without one the id
//! must resolve to exactly one block in the whole document (an id that lives
//! on two pages is [`EditError::AmbiguousId`] until a route is passed).
//! Top-level blocks (`::nav`, `::footer`, a loose `::section`) have no route.
//!
//! ## What a "block" is here
//!
//! Every directive the parser typed — top-level or nested inside a container
//! (`::page`, `::section`, `::app-shell`, …) — with its span in the
//! line-ending-normalised source. Loose Markdown between directives carries
//! no `id=` and is never an edit target (a loose paragraph stays loose by
//! design; to change it, edit the directive block around it or wrap it in
//! one). A top-level Markdown block is listed as kind `markdown` with no id
//! so a caller can see it; inside a container the parser gives loose text a
//! placeholder span, so it is not listed at all.
//!
//! ## Stamping
//!
//! [`stamp_ids`] gives every directive block that has no `id=` a stable
//! `b-<kind>-<n>` id ONCE (n = the block's ordinal among its kind on its
//! page; an authored id always wins and is never rewritten). Templates
//! already stamp their own ids; this is for imported or hand-written sites.
//! `::site` and `::page` are never stamped — a site is addressed as the
//! document, a page by its route.
//!
//! ## Round trip
//!
//! An edit keeps everything it did not touch byte-identical (modulo the
//! CRLF → LF normalisation `parse` itself performs), so a versioned document
//! diffs as the one change the person asked for.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::types::{Block, Span};

/// One addressable block: where it is, what it is, and how it is addressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRef {
    /// The directive name (`hero`, `section`, `page`, …); `markdown` for
    /// loose text between directives.
    pub name: String,
    /// The authored `id=` (or the one [`stamp_ids`] gave it); `None` when
    /// the block has no address yet.
    pub id: Option<String>,
    /// The authored `label=` when the directive does not spend it on its own
    /// semantics (see `block_meta`).
    pub label: Option<String>,
    /// The `::page` route this block lives under; `None` for a top-level block.
    pub route: Option<String>,
    /// Nesting depth: 0 = top-level, 1 = a page's child, …
    pub depth: u32,
    /// 1-based first line of the block.
    pub start_line: usize,
    /// 1-based last line of the block (inclusive).
    pub end_line: usize,
    /// 0-based byte offset of the block's first character (normalised source).
    pub start_offset: usize,
    /// 0-based byte offset past the block's last character.
    pub end_offset: usize,
}

impl BlockRef {
    fn span(&self) -> Span {
        Span {
            start_line: self.start_line,
            end_line: self.end_line,
            start_offset: self.start_offset,
            end_offset: self.end_offset,
        }
    }
}

/// Why an edit was refused. Every variant is a caller error the caller can
/// act on; none of them leaves a partially edited source behind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("no block with id `{id}`{scope} — list_blocks names the ids this document has")]
    UnknownId { id: String, scope: String },
    #[error(
        "block id `{id}` is on {count} pages ({routes}) — pass the page's route to say which one"
    )]
    AmbiguousId {
        id: String,
        count: usize,
        routes: String,
    },
    #[error("no `::page` at route `{route}` — the routes are: {routes}")]
    UnknownRoute { route: String, routes: String },
    #[error("the document has no `::site` block to edit")]
    NoSiteBlock,
    #[error("block `{id}` (`::{name}`) has no text line to set — replace the block instead")]
    NoTextLine { id: String, name: String },
    #[error(
        "invalid block id `{id}` — letters, digits, `-`, `_`, `.` and `:`, starting with a letter"
    )]
    InvalidId { id: String },
    #[error("the new source must be exactly ONE block (got {count})")]
    NotOneBlock { count: usize },
    #[error("a block cannot be moved before itself")]
    MoveOntoSelf,
    #[error("`{a}` and `{b}` are not on the same page — a block moves within its page")]
    DifferentPages { a: String, b: String },
    #[error("invalid attribute key `{key}` — letters, digits, `-` and `_`")]
    InvalidKey { key: String },
    #[error("unknown edit op `{op}` — one of replace, insert_after, remove, move, set_attr, set_site_key, set_text, stamp_ids")]
    UnknownOp { op: String },
    #[error("edit op `{op}` is missing `{field}`")]
    MissingField { op: String, field: String },
    #[error("invalid edit op JSON: {message}")]
    InvalidOp { message: String },
}

/// `parse` normalises CRLF to LF before it records any span; the edits do
/// the same so every offset lands on the bytes it was recorded against.
fn normalise(source: &str) -> String {
    source.replace("\r\n", "\n")
}

/// The directive name written at `offset` (`::hero[…` → `hero`), or `None`
/// when the bytes there are not a directive opener (loose Markdown).
fn directive_name_at(source: &str, offset: usize) -> Option<String> {
    let rest = source.get(offset..)?;
    let rest = rest.strip_prefix("::")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

fn children_of(block: &Block) -> &[Block] {
    match block {
        Block::Page { children, .. }
        | Block::Slide { children, .. }
        | Block::Section { children, .. }
        | Block::App { children, .. }
        | Block::AppShell { children, .. }
        | Block::Sidebar { children, .. }
        | Block::Drawer { children, .. }
        | Block::Modal { children, .. } => children,
        _ => &[],
    }
}

fn is_real_span(span: Span, source_len: usize) -> bool {
    span.end_offset > span.start_offset && span.end_offset <= source_len
}

/// Every block of `source` in document order, with its id where it has one.
/// Nested blocks follow their container (depth-first), so the list reads
/// the way the page does.
pub fn list_blocks(source: &str) -> Vec<BlockRef> {
    let source = normalise(source);
    let parsed = crate::parse::parse(&source);
    let meta: BTreeMap<(usize, usize), (Option<String>, Option<String>)> =
        crate::block_meta::snapshot()
            .into_iter()
            .map(|(span, m)| ((span.start_offset, span.end_offset), (m.id, m.label)))
            .collect();
    let mut out = Vec::new();
    for block in &parsed.doc.blocks {
        walk(&source, block, None, 0, &meta, &mut out);
    }
    out
}

fn walk(
    source: &str,
    block: &Block,
    route: Option<&str>,
    depth: u32,
    meta: &BTreeMap<(usize, usize), (Option<String>, Option<String>)>,
    out: &mut Vec<BlockRef>,
) {
    let span = block.span();
    if !is_real_span(span, source.len()) {
        // A hand-built or placeholder-span block (nested under a container
        // whose span the parser could not anchor): unaddressable, skipped.
        return;
    }
    let name = match block {
        Block::Markdown { .. } => "markdown".to_string(),
        _ => directive_name_at(source, span.start_offset).unwrap_or_else(|| "block".to_string()),
    };
    let (id, label) = meta
        .get(&(span.start_offset, span.end_offset))
        .cloned()
        .unwrap_or((None, None));
    let own_route = match block {
        Block::Page { route, .. } => Some(route.as_str()),
        _ => route,
    };
    out.push(BlockRef {
        name,
        id,
        label,
        route: route.map(str::to_string),
        depth,
        start_line: span.start_line,
        end_line: span.end_line,
        start_offset: span.start_offset,
        end_offset: span.end_offset,
    });
    for child in children_of(block) {
        walk(source, child, own_route, depth + 1, meta, out);
    }
}

/// The routes of the document's pages, in order.
pub fn routes(source: &str) -> Vec<String> {
    list_blocks(source)
        .into_iter()
        .filter(|b| b.name == "page" && b.depth == 0)
        .filter_map(|b| page_route(b.start_offset, source))
        .collect()
}

/// The `route=` of the `::page` opener at `offset`.
fn page_route(offset: usize, source: &str) -> Option<String> {
    let source = normalise(source);
    let line = source.get(offset..)?.lines().next()?;
    let open = line.find('[')?;
    let close = line.rfind(']')?;
    let attrs = crate::attrs::parse_attrs(&line[open..=close]).ok()?;
    match attrs.get("route") {
        Some(crate::types::AttrValue::String(r)) => Some(r.clone()),
        _ => None,
    }
}

fn scope_words(route: Option<&str>) -> String {
    match route {
        Some(r) => format!(" on page `{r}`"),
        None => String::new(),
    }
}

/// Resolve `id` (optionally inside `route`) to exactly one block.
fn find(source: &str, route: Option<&str>, id: &str) -> Result<BlockRef, EditError> {
    let all = list_blocks(source);
    if let Some(route) = route {
        let known = routes(source);
        if !known.iter().any(|r| r == route) {
            return Err(EditError::UnknownRoute {
                route: route.to_string(),
                routes: if known.is_empty() {
                    "(none)".to_string()
                } else {
                    known.join(", ")
                },
            });
        }
    }
    let hits: Vec<&BlockRef> = all
        .iter()
        .filter(|b| b.id.as_deref() == Some(id))
        .filter(|b| route.is_none() || b.route.as_deref() == route)
        .collect();
    match hits.len() {
        0 => Err(EditError::UnknownId {
            id: id.to_string(),
            scope: scope_words(route),
        }),
        1 => Ok(hits[0].clone()),
        _ if route.is_some() => Ok(hits[0].clone()),
        n => {
            let mut pages: Vec<String> = hits
                .iter()
                .map(|b| b.route.clone().unwrap_or_else(|| "(top level)".to_string()))
                .collect();
            pages.dedup();
            Err(EditError::AmbiguousId {
                id: id.to_string(),
                count: n,
                routes: pages.join(", "),
            })
        }
    }
}

/// `true` for an id this module will write: letters, digits, `-`, `_`, `.`,
/// `:`, starting with a letter — what `id=` tolerates unquoted and what
/// `data-block-id` carries unchanged.
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

fn check_id(id: &str) -> Result<(), EditError> {
    if is_valid_id(id) {
        Ok(())
    } else {
        Err(EditError::InvalidId { id: id.to_string() })
    }
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// The one block `fragment` holds, trimmed, or the count it holds instead.
fn one_block(fragment: &str) -> Result<String, EditError> {
    let fragment = normalise(fragment);
    let trimmed = fragment.trim();
    let parsed = crate::parse::parse(trimmed);
    let count = parsed.doc.blocks.len();
    if count != 1 {
        return Err(EditError::NotOneBlock { count });
    }
    Ok(trimmed.to_string())
}

/// The opener line of the block at `start_offset` and its byte range.
fn opener(source: &str, start_offset: usize) -> (usize, usize) {
    let rest = &source[start_offset..];
    let len = rest.find('\n').unwrap_or(rest.len());
    (start_offset, start_offset + len)
}

/// Rewrite one directive opener so `key=value` is set (added or replaced),
/// every other attribute kept in its authored order and spelling.
fn opener_with_attr(line: &str, key: &str, value: &str) -> String {
    let (head, bracket) = match line.find('[') {
        Some(open) if line.trim_end().ends_with(']') => {
            let close = line.rfind(']').unwrap_or(line.len());
            (&line[..open], Some(&line[open + 1..close]))
        }
        _ => (line.trim_end(), None),
    };
    let rendered = render_attr(key, value);
    let mut tokens: Vec<String> = bracket.map(split_attr_tokens).unwrap_or_default();
    let mut replaced = false;
    for token in tokens.iter_mut() {
        let name = token.split('=').next().unwrap_or("");
        if name == key {
            *token = rendered.clone();
            replaced = true;
        }
    }
    if !replaced {
        tokens.push(rendered);
    }
    format!("{head}[{}]", tokens.join(" "))
}

/// `key=value`, quoted when the value needs it (whitespace, `]`, `"`, or
/// empty), `"` escaped as `\"` the way `parse_attrs` reads it back.
fn render_attr(key: &str, value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, ']' | '"' | '[' | '='));
    if needs_quotes {
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{key}=\"{escaped}\"")
    } else {
        format!("{key}={value}")
    }
}

/// Split a bracket's inside into its attribute tokens, quotes respected.
fn split_attr_tokens(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '\\' if in_quotes => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Whether the opener line already carries an `id=`.
fn opener_has_id(line: &str) -> bool {
    match (line.find('['), line.rfind(']')) {
        (Some(open), Some(close)) if close > open => split_attr_tokens(&line[open + 1..close])
            .iter()
            .any(|t| t.split('=').next() == Some("id")),
        _ => false,
    }
}

/// Splice `replacement` over `[start, end)` of `source`.
fn splice(source: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(source.len() + replacement.len());
    out.push_str(&source[..start]);
    out.push_str(replacement);
    out.push_str(&source[end..]);
    out
}

/// Replace the block `id` with `new_source` (exactly one block). The new
/// block keeps the old address: when it carries no `id=` of its own and the
/// old one was a directive with an id, the same id is stamped on it, so a
/// second edit can find it again.
pub fn replace_block(
    source: &str,
    route: Option<&str>,
    id: &str,
    new_source: &str,
) -> Result<String, EditError> {
    let source = normalise(source);
    let target = find(&source, route, id)?;
    let mut fragment = one_block(new_source)?;
    if fragment.starts_with("::") && !opener_has_id(fragment.lines().next().unwrap_or("")) {
        let (o_start, o_end) = opener(&fragment, 0);
        let line = fragment[o_start..o_end].to_string();
        fragment = splice(&fragment, o_start, o_end, &opener_with_attr(&line, "id", id));
    }
    Ok(splice(
        &source,
        target.start_offset,
        target.end_offset,
        &fragment,
    ))
}

/// Insert `new_source` (exactly one block) right after the block `id`, as
/// its next sibling. A directive without an `id=` is stamped with the next
/// free `b-<kind>-<n>` on that page so it is addressable at once.
pub fn insert_after(
    source: &str,
    route: Option<&str>,
    id: &str,
    new_source: &str,
) -> Result<String, EditError> {
    let source = normalise(source);
    let target = find(&source, route, id)?;
    let mut fragment = one_block(new_source)?;
    if let Some(name) = directive_name_at(&fragment, 0)
        && !opener_has_id(fragment.lines().next().unwrap_or(""))
        && !matches!(name.as_str(), "site" | "page")
    {
        let taken = ids_on_page(&list_blocks(&source), target.route.as_deref());
        let fresh = next_free_id(&name, &taken);
        let (o_start, o_end) = opener(&fragment, 0);
        let line = fragment[o_start..o_end].to_string();
        fragment = splice(&fragment, o_start, o_end, &opener_with_attr(&line, "id", &fresh));
    }
    let insertion = format!("\n\n{fragment}");
    Ok(splice(
        &source,
        target.end_offset,
        target.end_offset,
        &insertion,
    ))
}

/// Remove the block `id` and the blank line that separated it, leaving one
/// blank line between its neighbours.
pub fn remove_block(source: &str, route: Option<&str>, id: &str) -> Result<String, EditError> {
    let source = normalise(source);
    let target = find(&source, route, id)?;
    let before = source[..target.start_offset].trim_end_matches('\n');
    let after = source[target.end_offset..].trim_start_matches('\n');
    let mut out = String::with_capacity(source.len());
    out.push_str(before);
    if !before.is_empty() && !after.is_empty() {
        out.push_str("\n\n");
    } else if !before.is_empty() {
        out.push('\n');
    }
    out.push_str(after);
    Ok(out)
}

/// Move the block `id` so it sits right before the block `before`, on the
/// same page.
pub fn move_block(
    source: &str,
    route: Option<&str>,
    id: &str,
    before: &str,
) -> Result<String, EditError> {
    let source = normalise(source);
    if id == before {
        return Err(EditError::MoveOntoSelf);
    }
    let a = find(&source, route, id)?;
    let b = find(&source, route, before)?;
    if a.route != b.route {
        return Err(EditError::DifferentPages {
            a: id.to_string(),
            b: before.to_string(),
        });
    }
    let text = source[a.start_offset..a.end_offset].to_string();
    let without = remove_block(&source, route, id)?;
    let b_again = find(&without, route, before)?;
    let insertion = format!("{text}\n\n");
    Ok(splice(
        &without,
        b_again.start_offset,
        b_again.start_offset,
        &insertion,
    ))
}

/// Set `key=value` on the block `id`'s directive line (bracket attributes).
/// For the `::site` body keys (`accent:`, `name:` …) use [`set_site_key`].
pub fn set_attr(
    source: &str,
    route: Option<&str>,
    id: &str,
    key: &str,
    value: &str,
) -> Result<String, EditError> {
    if !is_valid_key(key) {
        return Err(EditError::InvalidKey {
            key: key.to_string(),
        });
    }
    if key == "id" {
        check_id(value)?;
    }
    let source = normalise(source);
    let target = find(&source, route, id)?;
    let (o_start, o_end) = opener(&source, target.start_offset);
    let line = source[o_start..o_end].to_string();
    Ok(splice(
        &source,
        o_start,
        o_end,
        &opener_with_attr(&line, key, value),
    ))
}

/// Set one `key: value` line in the top-level `::site` block's body (the
/// accent, the name, the description, …): replaced in place when the key is
/// there, appended before the closing `::` when it is not.
pub fn set_site_key(source: &str, key: &str, value: &str) -> Result<String, EditError> {
    if !is_valid_key(key) {
        return Err(EditError::InvalidKey {
            key: key.to_string(),
        });
    }
    let source = normalise(source);
    let site = list_blocks(&source)
        .into_iter()
        .find(|b| b.name == "site" && b.depth == 0)
        .ok_or(EditError::NoSiteBlock)?;
    let body = &source[site.start_offset..site.end_offset];
    let lines: Vec<&str> = body.split('\n').collect();
    let one_line: String = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let wanted = format!("{key}: {one_line}");
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 1);
    let mut replaced = false;
    for (i, line) in lines.iter().enumerate() {
        let is_closer = i == lines.len() - 1 && line.trim() == "::";
        if !replaced
            && i > 0
            && !is_closer
            && line
                .split_once(':')
                .map(|(k, _)| k.trim() == key)
                .unwrap_or(false)
        {
            out.push(wanted.clone());
            replaced = true;
            continue;
        }
        if is_closer && !replaced {
            out.push(wanted.clone());
            replaced = true;
        }
        out.push((*line).to_string());
    }
    if !replaced {
        // A `::site` with no closing line (a leaf form): append the key.
        out.push(wanted);
    }
    Ok(splice(
        &source,
        site.start_offset,
        site.end_offset,
        &out.join("\n"),
    ))
}

/// Set the block's first text line — the headline of a `::hero`, the first
/// paragraph of a `::section`, the caption of a card. A Markdown heading
/// prefix (`# `, `## `) is kept; the rest of the line is `text`. A leaf
/// directive with no content line is refused.
pub fn set_text(
    source: &str,
    route: Option<&str>,
    id: &str,
    text: &str,
) -> Result<String, EditError> {
    let source = normalise(source);
    let target = find(&source, route, id)?;
    let block = &source[target.start_offset..target.end_offset];
    let mut offset = target.start_offset;
    let mut first = true;
    for line in block.split('\n') {
        let len = line.len();
        if first {
            first = false;
        } else {
            let trimmed = line.trim();
            let is_closer = trimmed == "::";
            let is_directive = trimmed.starts_with("::");
            let is_body_key = trimmed
                .split_once(':')
                .map(|(k, v)| {
                    !k.is_empty()
                        && k.chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                        && v.starts_with(' ')
                })
                .unwrap_or(false);
            if !trimmed.is_empty() && !is_closer && !is_directive && !is_body_key {
                let indent_len = line.len() - line.trim_start().len();
                let indent = &line[..indent_len];
                let content = line.trim_start();
                let heading_len = content
                    .find(|c: char| c != '#')
                    .filter(|&n| n > 0 && content[n..].starts_with(' '))
                    .map(|n| n + 1)
                    .unwrap_or(0);
                let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let replacement = format!("{indent}{}{one_line}", &content[..heading_len]);
                return Ok(splice(&source, offset, offset + len, &replacement));
            }
        }
        offset += len + 1;
    }
    Err(EditError::NoTextLine {
        id: id.to_string(),
        name: target.name,
    })
}

fn ids_on_page(all: &[BlockRef], route: Option<&str>) -> BTreeSet<String> {
    all.iter()
        .filter(|b| b.route.as_deref() == route)
        .filter_map(|b| b.id.clone())
        .collect()
}

fn next_free_id(kind: &str, taken: &BTreeSet<String>) -> String {
    let mut n = 1usize;
    loop {
        let candidate = format!("b-{kind}-{n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Give every directive block without an `id=` a stable `b-<kind>-<n>` id,
/// once. Authored ids win and are untouched; `::site` and `::page` are never
/// stamped. Idempotent: a stamped document stamps to itself.
pub fn stamp_ids(source: &str) -> String {
    let source = normalise(source);
    let all = list_blocks(&source);
    // Work from the END so an earlier stamp never moves a later offset.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut taken_by_route: BTreeMap<Option<String>, BTreeSet<String>> = BTreeMap::new();
    for b in &all {
        if let Some(id) = &b.id {
            taken_by_route
                .entry(b.route.clone())
                .or_default()
                .insert(id.clone());
        }
    }
    for b in &all {
        if b.id.is_some() || b.name == "markdown" || matches!(b.name.as_str(), "site" | "page") {
            continue;
        }
        if directive_name_at(&source, b.start_offset).is_none() {
            continue;
        }
        let taken = taken_by_route.entry(b.route.clone()).or_default();
        let fresh = next_free_id(&b.name, taken);
        taken.insert(fresh.clone());
        let (o_start, o_end) = opener(&source, b.start_offset);
        let line = source[o_start..o_end].to_string();
        edits.push((o_start, o_end, opener_with_attr(&line, "id", &fresh)));
    }
    edits.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out = source;
    for (start, end, line) in edits {
        out = splice(&out, start, end, &line);
    }
    out
}

/// One edit as data — the shape the FFI, the wasm module and a JSON tool
/// surface carry. `op` picks the verb; the other fields are what that verb
/// reads (the rest are ignored).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EditOp {
    pub op: String,
    #[serde(default)]
    pub route: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    /// `replace` / `insert_after`: the new block's source.
    #[serde(default)]
    pub source: Option<String>,
    /// `move`: the id the block goes before.
    #[serde(default)]
    pub before: Option<String>,
    /// `set_attr` / `set_site_key`: the key.
    #[serde(default)]
    pub key: Option<String>,
    /// `set_attr` / `set_site_key`: the value.
    #[serde(default)]
    pub value: Option<String>,
    /// `set_text`: the text.
    #[serde(default)]
    pub text: Option<String>,
}

fn need<'a>(op: &EditOp, field: &str, value: &'a Option<String>) -> Result<&'a str, EditError> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| EditError::MissingField {
            op: op.op.clone(),
            field: field.to_string(),
        })
}

/// Apply one [`EditOp`] to `source`.
pub fn apply(source: &str, op: &EditOp) -> Result<String, EditError> {
    let route = op.route.as_deref().map(str::trim).filter(|s| !s.is_empty());
    match op.op.trim() {
        "replace" => replace_block(
            source,
            route,
            need(op, "id", &op.id)?,
            need(op, "source", &op.source)?,
        ),
        "insert_after" => insert_after(
            source,
            route,
            need(op, "id", &op.id)?,
            need(op, "source", &op.source)?,
        ),
        "remove" => remove_block(source, route, need(op, "id", &op.id)?),
        "move" => move_block(
            source,
            route,
            need(op, "id", &op.id)?,
            need(op, "before", &op.before)?,
        ),
        "set_attr" => set_attr(
            source,
            route,
            need(op, "id", &op.id)?,
            need(op, "key", &op.key)?,
            op.value.as_deref().unwrap_or(""),
        ),
        "set_site_key" => set_site_key(
            source,
            need(op, "key", &op.key)?,
            op.value.as_deref().unwrap_or(""),
        ),
        "set_text" => set_text(
            source,
            route,
            need(op, "id", &op.id)?,
            need(op, "text", &op.text)?,
        ),
        "stamp_ids" => Ok(stamp_ids(source)),
        other => Err(EditError::UnknownOp {
            op: other.to_string(),
        }),
    }
}

/// [`apply`] over a JSON-encoded [`EditOp`] — the one entry the FFI and the
/// wasm module export.
pub fn apply_json(source: &str, op_json: &str) -> Result<String, EditError> {
    let op: EditOp = serde_json::from_str(op_json).map_err(|e| EditError::InvalidOp {
        message: e.to_string(),
    })?;
    apply(source, &op)
}

/// [`list_blocks`] as a JSON array (the FFI / wasm twin).
pub fn list_blocks_json(source: &str) -> String {
    serde_json::to_string(&list_blocks(source)).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "::site\nname: Bens Construction Co\naccent: #f97316\ntheme: light\n::\n\n::nav[id=nav]\n- [Home](/)\n- [About](/about)\n::\n\n::page[route=\"/\" title=\"Home\"]\n::hero[id=hero badge=\"New\" align=left]\n# Every tool, on the truck\nWe track what we own.\n::\n\nA loose paragraph between blocks.\n\n::features[id=features columns=3]\n- **Fast** — in seconds\n- **Clear** — one list\n::\n::\n\n::page[route=\"/about\" title=\"About\"]\n::hero[id=hero]\n# About us\n::\n::\n";

    fn ids(source: &str) -> Vec<(String, Option<String>, Option<String>, u32)> {
        list_blocks(source)
            .into_iter()
            .map(|b| (b.name, b.id, b.route, b.depth))
            .collect()
    }

    #[test]
    fn lists_every_block_with_its_route_and_depth() {
        let got = ids(SITE);
        assert_eq!(
            got,
            vec![
                ("site".into(), None, None, 0),
                ("nav".into(), Some("nav".into()), None, 0),
                ("page".into(), None, None, 0),
                ("hero".into(), Some("hero".into()), Some("/".into()), 1),
                ("features".into(), Some("features".into()), Some("/".into()), 1),
                ("page".into(), None, None, 0),
                ("hero".into(), Some("hero".into()), Some("/about".into()), 1),
            ]
        );
        assert_eq!(routes(SITE), vec!["/", "/about"]);
    }

    #[test]
    fn an_id_on_two_pages_needs_a_route() {
        let err = set_text(SITE, None, "hero", "x").unwrap_err();
        assert!(matches!(err, EditError::AmbiguousId { count: 2, .. }), "{err}");
        let out = set_text(SITE, Some("/about"), "hero", "About Bens").unwrap();
        assert!(out.contains("# About Bens"));
        assert!(out.contains("# Every tool, on the truck"), "the home hero is untouched");
    }

    #[test]
    fn an_unknown_id_or_route_is_an_error_never_a_no_op() {
        let err = remove_block(SITE, None, "nope").unwrap_err();
        assert!(matches!(err, EditError::UnknownId { .. }), "{err}");
        let err = remove_block(SITE, Some("/pricing"), "hero").unwrap_err();
        assert!(matches!(err, EditError::UnknownRoute { .. }), "{err}");
        assert!(err.to_string().contains("/about"));
    }

    #[test]
    fn set_text_keeps_the_heading_prefix_and_everything_else() {
        let out = set_text(SITE, Some("/"), "hero", "Every tool, accounted for").unwrap();
        assert!(out.contains("# Every tool, accounted for\nWe track what we own."));
        let (a, b) = (SITE.lines().count(), out.lines().count());
        assert_eq!(a, b, "one line changed, none added");
        // Only one line differs.
        let diff = SITE.lines().zip(out.lines()).filter(|(x, y)| x != y).count();
        assert_eq!(diff, 1);
    }

    #[test]
    fn set_text_refuses_a_leaf_without_a_text_line() {
        let src = "::page[route=\"/\"]\n::divider[id=d]\n::\n";
        let err = set_text(src, None, "d", "x").unwrap_err();
        assert!(matches!(err, EditError::NoTextLine { .. }), "{err}");
    }

    #[test]
    fn set_attr_adds_or_replaces_in_authored_order() {
        let out = set_attr(SITE, Some("/"), "hero", "align", "center").unwrap();
        assert!(out.contains("::hero[id=hero badge=\"New\" align=center]"));
        let out = set_attr(SITE, Some("/"), "hero", "image", "/assets/truck.jpg").unwrap();
        assert!(out.contains("::hero[id=hero badge=\"New\" align=left image=/assets/truck.jpg]"));
        let out = set_attr(SITE, None, "nav", "cta-label", "Book a demo").unwrap();
        assert!(out.contains("::nav[id=nav cta-label=\"Book a demo\"]"));
        // A quoted value with a quote inside survives the round trip.
        let out = set_attr(SITE, Some("/"), "hero", "badge", "the \"new\" one").unwrap();
        assert!(out.contains("badge=\"the \\\"new\\\" one\""));
        let opener = out.lines().find(|l| l.starts_with("::hero[")).unwrap();
        let attrs = crate::attrs::parse_attrs(&opener[opener.find('[').unwrap()..]).unwrap();
        assert_eq!(
            attrs.get("badge"),
            Some(&crate::types::AttrValue::String("the \"new\" one".into()))
        );
    }

    #[test]
    fn set_attr_refuses_a_bad_key_or_a_bad_id_value() {
        assert!(matches!(
            set_attr(SITE, None, "nav", "bad key", "x").unwrap_err(),
            EditError::InvalidKey { .. }
        ));
        assert!(matches!(
            set_attr(SITE, None, "nav", "id", "1abc").unwrap_err(),
            EditError::InvalidId { .. }
        ));
    }

    #[test]
    fn set_site_key_replaces_or_appends_inside_the_site_body() {
        let out = set_site_key(SITE, "accent", "#3b82f6").unwrap();
        assert!(out.contains("::site\nname: Bens Construction Co\naccent: #3b82f6\ntheme: light\n::"));
        assert_eq!(out.lines().count(), SITE.lines().count());
        let out = set_site_key(SITE, "tagline", "Tools, tracked.").unwrap();
        assert!(out.contains("theme: light\ntagline: Tools, tracked.\n::"));
        let err = set_site_key("::page[route=\"/\"]\n::", "accent", "#000000").unwrap_err();
        assert_eq!(err, EditError::NoSiteBlock);
    }

    #[test]
    fn replace_keeps_the_address_when_the_new_block_has_none() {
        let out = replace_block(
            SITE,
            Some("/"),
            "hero",
            "::hero[align=center]\n# A new hero\n::",
        )
        .unwrap();
        assert!(out.contains("::hero[align=center id=hero]\n# A new hero\n::"));
        assert!(!out.contains("Every tool, on the truck"));
        assert!(out.contains("# About us"), "the other page is untouched");
        // An explicit id on the new block wins.
        let out = replace_block(SITE, Some("/"), "hero", "::hero[id=banner]\n# B\n::").unwrap();
        assert!(out.contains("::hero[id=banner]\n# B\n::"));
    }

    #[test]
    fn replace_refuses_more_or_less_than_one_block() {
        let err = replace_block(SITE, Some("/"), "hero", "::hero\n# A\n::\n\n::cta[label=Go href=/]").unwrap_err();
        assert!(matches!(err, EditError::NotOneBlock { count: 2 }), "{err}");
    }

    #[test]
    fn insert_after_stamps_a_fresh_id_on_the_page() {
        let out = insert_after(SITE, Some("/"), "hero", "::cta[label=\"Get a quote\" href=/contact]").unwrap();
        assert!(out.contains("::\n\n::cta[label=\"Get a quote\" href=/contact id=b-cta-1]\n\nA loose paragraph"));
        let listed = list_blocks(&out);
        let cta = listed.iter().find(|b| b.name == "cta").unwrap();
        assert_eq!(cta.route.as_deref(), Some("/"));
        assert_eq!(cta.id.as_deref(), Some("b-cta-1"));
    }

    #[test]
    fn remove_leaves_one_blank_line_between_the_neighbours() {
        let out = remove_block(SITE, Some("/"), "features").unwrap();
        assert!(!out.contains("::features"));
        assert!(out.contains("A loose paragraph between blocks.\n\n::\n\n::page[route=\"/about\""));
        let out = remove_block(SITE, None, "nav").unwrap();
        assert!(out.contains("::\n\n::page[route=\"/\" title=\"Home\"]"));
        assert!(!out.contains("::nav"));
    }

    #[test]
    fn move_puts_a_block_before_another_on_the_same_page() {
        let out = move_block(SITE, Some("/"), "features", "hero").unwrap();
        let listed: Vec<String> = list_blocks(&out)
            .into_iter()
            .filter(|b| b.route.as_deref() == Some("/"))
            .map(|b| b.name)
            .collect();
        assert_eq!(listed, vec!["features", "hero"]);
        assert!(matches!(
            move_block(SITE, Some("/"), "hero", "hero").unwrap_err(),
            EditError::MoveOntoSelf
        ));
        assert!(matches!(
            move_block(SITE, None, "nav", "features").unwrap_err(),
            EditError::DifferentPages { .. }
        ));
    }

    #[test]
    fn stamp_ids_addresses_every_unlabelled_directive_once() {
        let src = "::site\nname: X\n::\n\n::nav\n- [Home](/)\n::\n\n::page[route=\"/\"]\n::hero\n# A\n::\n\n::features[columns=3]\n- a\n::\n\n::hero[id=hero]\n# B\n::\n\n::cta[label=Go href=/]\n::\n";
        let out = stamp_ids(src);
        assert!(out.contains("::nav[id=b-nav-1]"));
        assert!(out.contains("::hero[id=b-hero-1]\n# A"));
        assert!(out.contains("::features[columns=3 id=b-features-1]"));
        assert!(out.contains("::hero[id=hero]\n# B"), "an authored id is untouched");
        assert!(out.contains("::cta[label=Go href=/ id=b-cta-1]"));
        assert!(out.starts_with("::site\nname: X\n::"), "site is never stamped");
        assert!(out.contains("::page[route=\"/\"]\n"), "page is never stamped");
        assert_eq!(stamp_ids(&out), out, "idempotent");
        // The lint agrees: no duplicate ids.
        let report = crate::lint::check(&out);
        assert!(
            !report.diagnostics.iter().any(|d| d.code.as_deref() == Some("L043")),
            "{:?}",
            report.diagnostics
        );
    }

    #[test]
    fn stamped_ids_skip_an_authored_collision() {
        let src = "::page[route=\"/\"]\n::cta[id=b-cta-1 label=A href=/]\n::cta[label=B href=/]\n::\n";
        let out = stamp_ids(src);
        assert!(out.contains("::cta[label=B href=/ id=b-cta-2]"));
    }

    #[test]
    fn crlf_input_edits_on_normalised_offsets() {
        let crlf = SITE.replace('\n', "\r\n");
        let out = set_text(&crlf, Some("/"), "hero", "Windows wrote this").unwrap();
        assert!(out.contains("# Windows wrote this\nWe track"));
        assert!(!out.contains('\r'));
    }

    #[test]
    fn apply_json_routes_every_verb_and_names_a_bad_one() {
        let out = apply_json(SITE, r#"{"op":"set_text","route":"/","id":"hero","text":"Hi"}"#).unwrap();
        assert!(out.contains("# Hi"));
        let out = apply_json(SITE, r##"{"op":"set_site_key","key":"accent","value":"#000000"}"##).unwrap();
        assert!(out.contains("accent: #000000"));
        let out = apply_json(SITE, r#"{"op":"stamp_ids"}"#).unwrap();
        assert_eq!(out, stamp_ids(SITE));
        let err = apply_json(SITE, r#"{"op":"remove"}"#).unwrap_err();
        assert!(matches!(err, EditError::MissingField { .. }), "{err}");
        let err = apply_json(SITE, r#"{"op":"explode","id":"hero"}"#).unwrap_err();
        assert!(matches!(err, EditError::UnknownOp { .. }), "{err}");
        let err = apply_json(SITE, "not json").unwrap_err();
        assert!(matches!(err, EditError::InvalidOp { .. }), "{err}");
        let listed: Vec<BlockRef> = serde_json::from_str(&list_blocks_json(SITE)).unwrap();
        assert_eq!(listed.len(), 7);
    }

    #[test]
    fn an_edit_round_trips_through_the_parser_unchanged_elsewhere() {
        let out = set_text(SITE, Some("/"), "hero", "Every tool, accounted for").unwrap();
        let before = crate::parse::parse(SITE);
        let after = crate::parse::parse(&out);
        assert_eq!(before.doc.blocks.len(), after.doc.blocks.len());
        assert!(after.diagnostics.iter().all(|d| d.severity != crate::error::Severity::Error));
    }
}

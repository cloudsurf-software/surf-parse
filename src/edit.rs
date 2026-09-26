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
//! ## Text-anchored edits (0.29.0)
//!
//! A person quotes what they SEE, not an id. [`find_text`] turns a phrase
//! into the places it occurs (block · slot · exact or normalised) and
//! [`resolve`] applies the ONE ambiguity policy — exact once, else
//! normalised once, else the current route's one, else never guess.
//! [`replace_text`] replaces the phrase inside its line, the markup around
//! it kept, in the scope of a block, a page (loose Markdown included) or the
//! document. [`list_blocks`] carries each block's first visible `text` and a
//! `count` of its repeated items so a client can show an index of a page.
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
    /// The block's first visible text (markup stripped, ≤ 120 chars): a
    /// `::site`'s name, a `::page`'s title, a hero's headline, a card's
    /// title — the words an index shows beside the id. `None` for a block
    /// with no text of its own.
    #[serde(default)]
    pub text: Option<String>,
    /// How many repeated things the block holds — list items, cards,
    /// questions, steps, table rows, child blocks; `None` when none.
    #[serde(default)]
    pub count: Option<usize>,
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
    #[error("block id `{id}` appears {count} times on page `{route}` — give the blocks distinct ids (stamp_ids) and say which")]
    DuplicateId {
        id: String,
        route: String,
        count: usize,
    },
    #[error("`{find}` was not found{scope} — find_text lists where a phrase is; quote it as the page shows it")]
    TextNotFound { find: String, scope: String },
    #[error("`{find}` is in {count} places{scope}: {candidates} — say which (its block id), or quote more of the text")]
    AmbiguousText {
        find: String,
        count: usize,
        scope: String,
        candidates: String,
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
    #[error("unknown edit op `{op}` — one of replace, insert_after, remove, move, set_attr, set_site_key, set_text, replace_text, stamp_ids")]
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
    let bare = out.clone();
    for b in out.iter_mut() {
        b.text = block_text_n(&source, &bare, b);
        b.count = block_count_n(&source, &bare, b);
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
        text: None,
        count: None,
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
    // With a route, the page's blocks AND the site-wide (top-level) ones are in
    // scope — a `nav` or a `footer` is found whatever page the person is on.
    let hits: Vec<&BlockRef> = all
        .iter()
        .filter(|b| b.id.as_deref() == Some(id))
        .filter(|b| route.is_none() || b.route.as_deref() == route || b.route.is_none())
        .collect();
    match hits.len() {
        0 => Err(EditError::UnknownId {
            id: id.to_string(),
            scope: scope_words(route),
        }),
        1 => Ok(hits[0].clone()),
        n if route.is_some() && hits.iter().all(|b| b.route == hits[0].route) => Err(EditError::DuplicateId {
            id: id.to_string(),
            route: hits[0].route.clone().unwrap_or_else(|| "(top level)".to_string()),
            count: n,
        }),
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
/// directive with no content line is refused. This is sugar for the one
/// common case; to change a PHRASE anywhere in a block (a nav item, a
/// second card, a button label, a FAQ answer) use [`replace_text`], which
/// keeps the markup around the phrase.
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
    /// `replace_text`: the text as the page shows it (the anchor).
    #[serde(default)]
    pub find: Option<String>,
    /// `replace_text`: the new text (one line; empty removes the phrase).
    #[serde(default)]
    pub replace: Option<String>,
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
        "replace_text" => replace_text(
            source,
            route,
            op.id.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            need(op, "find", &op.find)?,
            op.replace.as_deref().unwrap_or(""),
        )
        .map(|r| r.source),
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

// ═══════════════════════════════════════════════════════════════════════
// Text-anchored edits (0.29.0) — the block is found BEFORE the model runs
// ═══════════════════════════════════════════════════════════════════════
//
// A person looking at a rendered page says "change 'About Bens' to 'About
// Ben's Construction'". Nothing in that sentence is a block id. [`find_text`]
// turns the quoted phrase into the places it occurs — each with the block it
// sits in, the SLOT inside that block (a headline, the second item, the
// `label` attribute, a table cell), and whether the bytes there are the
// phrase verbatim — and [`resolve`] applies ONE policy to those hits: an
// exact match once wins, else a normalised match once, else the current
// route's one, else never guess (name the candidates). [`replace_text`] is
// the edit that follows: the old text is the anchor, the match is replaced
// INSIDE its line so the markup around it (a bullet, `[label](href)`, a
// heading prefix, `{#anchor}`, the `[attrs]` line, the value side of
// `key: value`) survives, and a page-scoped call reaches loose Markdown
// that has no id at all.
//
// One implementation, three callers: a client resolving a pill before send
// (over the source it already holds), a server tool mid-turn, and the write
// itself. The FFI and wasm twins carry `find_text` as JSON.

/// One place a phrase was found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextHit {
    /// The block's id — `None` for loose Markdown under a page or a block
    /// nobody stamped yet.
    pub id: Option<String>,
    /// The directive name (`hero`, `features`, …); `markdown` for loose text.
    pub kind: String,
    /// The `::page` route the text lives under; `None` for a site-wide block.
    pub route: Option<String>,
    /// Where inside the block: `headline` · `subtitle` · `body(n)` · `item(n)`
    /// · `attr(key)` · `cell(r,c)`.
    pub slot: String,
    /// 1-based line of the (normalised) source.
    pub line: usize,
    /// 1-based character column on that line where the match starts.
    pub col: usize,
    /// The match's length in characters.
    pub len: usize,
    /// Byte range of the match in the normalised source.
    pub start_offset: usize,
    pub end_offset: usize,
    /// `true` when the bytes there are the query verbatim (trailing
    /// punctuation of the query aside); `false` for a normalised match.
    pub exact: bool,
    /// The line's visible text (markup stripped, ≤ 120 chars) — or the
    /// attribute's value for an `attr(key)` hit.
    pub snippet: String,
}

impl TextHit {
    /// `hero › headline`, `features › item 2`, `cta › label`, `markdown › body 1`
    /// — the words a composer pill or a candidate chip shows.
    pub fn label(&self) -> String {
        format!("{} › {}", self.id.as_deref().unwrap_or(&self.kind), slot_words(&self.slot))
    }
}

/// `item(2)` → `item 2`, `attr(label)` → `label`, `cell(2,3)` → `row 2 col 3`.
pub fn slot_words(slot: &str) -> String {
    if let Some(inner) = slot.strip_prefix("attr(").and_then(|s| s.strip_suffix(')')) {
        return inner.to_string();
    }
    if let Some(inner) = slot.strip_prefix("cell(").and_then(|s| s.strip_suffix(')')) {
        if let Some((r, c)) = inner.split_once(',') {
            return format!("row {r} col {c}");
        }
    }
    match slot.split_once('(') {
        Some((word, rest)) => format!("{word} {}", rest.trim_end_matches(')')),
        None => slot.to_string(),
    }
}

/// What the ONE ambiguity policy decided for a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// The phrase occurs verbatim exactly once.
    Exact(TextHit),
    /// No verbatim occurrence; a normalised one (case, whitespace, quotes,
    /// trailing punctuation, emphasis markers, an ellipsis) exactly once.
    Normalized(TextHit),
    /// Several occurrences, one of them on the route the person is looking at.
    CurrentRoute(TextHit),
    /// Several occurrences and the route does not decide — never guess: the
    /// candidates, for the person to pick from.
    Ambiguous(Vec<TextHit>),
    /// Nothing matched, even normalised.
    None,
}

impl Resolved {
    /// The policy word the JSON twin and the server's tool answer carry.
    pub fn policy(&self) -> &'static str {
        match self {
            Resolved::Exact(_) => "exact",
            Resolved::Normalized(_) => "normalized",
            Resolved::CurrentRoute(_) => "current_route",
            Resolved::Ambiguous(_) => "ambiguous",
            Resolved::None => "none",
        }
    }

    /// The one hit the policy picked, when it picked one.
    pub fn pick(&self) -> Option<&TextHit> {
        match self {
            Resolved::Exact(h) | Resolved::Normalized(h) | Resolved::CurrentRoute(h) => Some(h),
            Resolved::Ambiguous(_) | Resolved::None => None,
        }
    }
}

/// The result of a [`replace_text`]: the new source and the one hit it
/// replaced, with the text before and after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextReplaced {
    pub source: String,
    pub hit: TextHit,
    pub before: String,
    pub after: String,
}

// ── the normaliser ─────────────────────────────────────────────────────

/// A line in its normalised view, every normalised character mapped back to
/// the raw bytes it came from (so a match's raw span is exact).
struct NormLine {
    chars: Vec<char>,
    /// Per normalised char: the raw byte range within the line.
    spans: Vec<(usize, usize)>,
}

/// The normalising rules, one each (the table the tests pin):
///   1. case — every character lower-cased;
///   2. whitespace — a run of any whitespace is one space;
///   3. quotes — curly single / double quotes read as straight ones;
///   4. emphasis — `*` and `` ` `` are transparent (rendered text has none);
///   5. trailing punctuation — `. , ! ? ; :` at the END of the query are dropped
///      (applied to the query only, see [`normalise_query`]);
///   6. ellipsis — `…` or `...` in the MIDDLE of the query stands for anything
///      on the same line (see [`Query`]);
///   7. invisible markup — a link's `(href)` and a trailing `{…}` are skipped
///      (the page never shows them).
fn normalise_line(line: &str) -> NormLine {
    let mut chars = Vec::with_capacity(line.len());
    let mut spans = Vec::with_capacity(line.len());
    // Rule 7: a link's `(href)` and a trailing `{primary}` / `{#anchor}` are
    // invisible on the page, so they are invisible here too.
    let mut skip_until = 0usize;
    for (i, c) in line.char_indices() {
        let end = i + c.len_utf8();
        if i < skip_until {
            continue;
        }
        if c == ']' && line[end..].starts_with('(') {
            if let Some(close) = line[end..].find(')') {
                skip_until = end + close + 1;
                continue;
            }
        }
        if c == '{'
            && let Some(close) = line[end..].find('}')
            && line[end + close + 1..].trim().is_empty()
        {
            skip_until = end + close + 1;
            continue;
        }
        if c.is_whitespace() {
            if chars.last() == Some(&' ') {
                continue;
            }
            chars.push(' ');
            spans.push((i, end));
            continue;
        }
        if matches!(c, '*' | '`') {
            continue;
        }
        let mapped = match c {
            '‘' | '’' | '‚' | '‛' | '´' => '\'',
            '“' | '”' | '„' | '‟' => '"',
            other => other,
        };
        for lc in mapped.to_lowercase() {
            chars.push(lc);
            spans.push((i, end));
        }
    }
    NormLine { chars, spans }
}

/// The query in its normalised view: the line rules, trimmed, trailing
/// punctuation dropped, and split at an ellipsis into a head and a tail.
struct Query {
    head: Vec<char>,
    tail: Option<Vec<char>>,
}

fn trim_spaces(chars: &[char]) -> &[char] {
    let start = chars.iter().position(|c| *c != ' ').unwrap_or(chars.len());
    let end = chars.iter().rposition(|c| *c != ' ').map(|i| i + 1).unwrap_or(start);
    &chars[start..end.max(start)]
}

fn strip_trailing_punct(chars: &[char]) -> &[char] {
    let end = chars
        .iter()
        .rposition(|c| !matches!(c, '.' | ',' | '!' | '?' | ';' | ':'))
        .map(|i| i + 1)
        .unwrap_or(0);
    &chars[..end]
}

fn normalise_query(query: &str) -> Option<Query> {
    let norm = normalise_line(query);
    let chars = trim_spaces(strip_trailing_punct(trim_spaces(&norm.chars))).to_vec();
    if chars.is_empty() {
        return None;
    }
    // An ellipsis in the middle: `…` or `...` with something on both sides.
    let ellipsis = chars
        .iter()
        .position(|c| *c == '…')
        .map(|i| (i, 1))
        .or_else(|| chars.windows(3).position(|w| w == ['.', '.', '.']).map(|i| (i, 3)));
    if let Some((at, width)) = ellipsis {
        let head = trim_spaces(&chars[..at]).to_vec();
        let tail = trim_spaces(&chars[at + width..]).to_vec();
        if !head.is_empty() && !tail.is_empty() {
            return Some(Query { head, tail: Some(tail) });
        }
    }
    Some(Query { head: chars, tail: None })
}

/// The trailing-punctuation-free, trimmed raw query — what an exact hit is
/// compared against besides the query itself.
fn raw_query_variants(query: &str) -> [String; 2] {
    let trimmed = query.trim();
    let stripped = trimmed.trim_end_matches(|c: char| matches!(c, '.' | ',' | '!' | '?' | ';' | ':')).trim_end();
    [trimmed.to_string(), stripped.to_string()]
}

fn find_chars(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Every match of `q` on one raw line, as raw byte ranges within the line.
fn matches_in_line(raw: &str, norm: &NormLine, q: &Query) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = find_chars(&norm.chars, &q.head, from) {
        let head_end = p + q.head.len();
        let end_idx = match &q.tail {
            None => head_end,
            Some(tail) => match find_chars(&norm.chars, tail, head_end) {
                Some(t) => t + tail.len(),
                None => break,
            },
        };
        let (start, end) = (norm.spans[p].0, norm.spans[end_idx - 1].1);
        out.push(balance_markers(raw, start, end));
        from = p + 1;
    }
    out
}

/// A normalised match can start or end inside an emphasis run (`**Fast**`
/// matched as `Fast`): a span that holds an odd number of a marker is
/// widened over the marker just after it (else just before it), so the
/// replacement never leaves half a `**` behind.
fn balance_markers(raw: &str, mut start: usize, mut end: usize) -> (usize, usize) {
    for marker in ["**", "*", "`"] {
        for _ in 0..2 {
            let inside = raw[start..end].matches(marker).count();
            if inside % 2 == 0 {
                break;
            }
            if raw[end..].starts_with(marker) {
                end += marker.len();
            } else if raw[..start].ends_with(marker) {
                start -= marker.len();
            } else {
                break;
            }
        }
    }
    (start, end)
}

// ── visible text ───────────────────────────────────────────────────────

/// A source line as the page shows it: the heading prefix, the bullet, the
/// emphasis markers, the link syntax (`[label](href)` → `label`) and a
/// trailing `{…}` gone, whitespace collapsed.
pub fn visible_text(line: &str) -> String {
    let mut t = line.trim();
    // A table row reads as its cells.
    if t.starts_with('|') && !is_table_separator(t) {
        return t
            .trim_matches('|')
            .split('|')
            .map(visible_text)
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
    }
    // Heading / bullet / numbered / quote prefixes.
    if let Some(n) = t.find(|c: char| c != '#')
        && n > 0
        && t[n..].starts_with(' ')
    {
        t = t[n + 1..].trim_start();
    }
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")).or_else(|| t.strip_prefix("> ")) {
        t = rest.trim_start();
    } else if let Some(dot) = t.find(". ")
        && dot > 0
        && t[..dot].chars().all(|c| c.is_ascii_digit())
    {
        t = t[dot + 2..].trim_start();
    }
    // A trailing `{primary}` / `{#anchor}` / `{.class}`.
    let mut s = t.to_string();
    loop {
        let open = if s.ends_with('}') { s.rfind('{') } else { None };
        match open {
            Some(open) => {
                s.truncate(open);
                s = s.trim_end().to_string();
            }
            None => break,
        }
    }
    // `[label](href)` and `![alt](src)` → the label.
    let mut out = String::with_capacity(s.len());
    let mut rest = s.as_str();
    while let Some(open) = rest.find('[') {
        let (before, after) = rest.split_at(open);
        if let Some(close) = after.find("](")
            && let Some(paren) = after[close..].find(')')
        {
            let bang = before.ends_with('!');
            out.push_str(if bang { &before[..before.len() - 1] } else { before });
            out.push_str(&after[1..close]);
            rest = &after[close + paren + 1..];
        } else {
            out.push_str(before);
            out.push('[');
            rest = &after[1..];
        }
    }
    out.push_str(rest);
    let out: String = out.chars().filter(|c| !matches!(c, '*' | '`')).collect();
    let words: Vec<&str> = out.split_whitespace().collect();
    words.join(" ")
}

fn clip(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    s = s.trim_end().to_string();
    s.push('…');
    s
}

// ── the block's own lines and their slots ──────────────────────────────

/// A block kind whose `##` / `###` headings each start an item (a card, a
/// question, a step) rather than titling the block.
fn item_headed(kind: &str) -> bool {
    matches!(
        kind,
        "faq" | "features" | "steps" | "tabs" | "timeline" | "accordion" | "pricing-table" | "testimonials" | "stats" | "team" | "cards"
    )
}

/// An attribute whose value is words the page shows (`label`, `title`,
/// `badge`, `tagline`, `name`, `price` …) — not an id, a route, a link, a
/// colour or a layout token.
fn text_attr(key: &str) -> bool {
    !matches!(
        key,
        "id" | "route" | "href" | "src" | "icon" | "image" | "columns" | "cols" | "align" | "layout" | "theme" | "class"
            | "width" | "height" | "accent" | "color" | "colour" | "primary" | "currency" | "style" | "template" | "type"
    ) && !key.ends_with("-href")
        && !key.ends_with("-src")
        && !key.ends_with("-id")
        && !key.ends_with("-icon")
        && !key.ends_with("-image")
}

/// One attribute value on an opener or a `key: value` line: its key and its
/// raw byte range within the LINE.
struct AttrSpan {
    key: String,
    start: usize,
    end: usize,
}

/// The `key=value` tokens of a directive opener with their value ranges,
/// quotes respected (a quoted value's range excludes the quotes).
fn opener_attr_spans(line: &str) -> Vec<AttrSpan> {
    let mut out = Vec::new();
    let Some(open) = line.find('[') else { return out };
    let Some(close) = line.rfind(']') else { return out };
    if close <= open {
        return out;
    }
    let inner = &line[open + 1..close];
    let base = open + 1;
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = inner[key_start..i].to_string();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            if i < bytes.len() && bytes[i] == b'"' {
                let vstart = i + 1;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                out.push(AttrSpan { key, start: base + vstart, end: base + i.min(bytes.len()) });
                i += 1;
            } else {
                let vstart = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                out.push(AttrSpan { key, start: base + vstart, end: base + i });
            }
        }
    }
    out
}

/// `key: value` with an identifier key and a space after the colon — the
/// `::site` body's lines, a card's `price: …`. Returns the key and the
/// value's byte range within the line.
fn body_key_line(line: &str) -> Option<(String, usize, usize)> {
    let indent = line.len() - line.trim_start().len();
    let t = &line[indent..];
    let (k, v) = t.split_once(':')?;
    if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) || !v.starts_with(' ') {
        return None;
    }
    let vstart = indent + k.len() + 1 + (v.len() - v.trim_start().len());
    Some((k.to_string(), vstart, line.len()))
}

/// One of a block's own lines (not the closer, not a child block's line)
/// with the slot it fills.
struct LineSlot {
    /// Absolute byte range of the line (no newline).
    start: usize,
    end: usize,
    /// The slot for text hits on the line; empty on an opener (attrs only).
    slot: String,
    /// The line's visible text.
    visible: String,
    /// Attribute values on the line (an opener, or a `key: value` line).
    attrs: Vec<AttrSpan>,
    /// Table cells: absolute byte ranges, 1-based column order.
    cells: Vec<(usize, usize)>,
}

fn is_table_separator(t: &str) -> bool {
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn is_list_item(t: &str) -> bool {
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    if let Some(dot) = t.find(". ")
        && dot > 0
        && t[..dot].chars().all(|c| c.is_ascii_digit())
    {
        return true;
    }
    false
}

fn heading_level(t: &str) -> usize {
    match t.find(|c: char| c != '#') {
        Some(n) if n > 0 && t[n..].starts_with(' ') => n,
        _ => 0,
    }
}

/// The block's own lines with their slots. Lines inside a child block's
/// span belong to the child and are skipped here.
fn line_slots(source: &str, all: &[BlockRef], b: &BlockRef) -> Vec<LineSlot> {
    let children: Vec<(usize, usize)> = all
        .iter()
        .filter(|c| c.start_offset >= b.start_offset && c.end_offset <= b.end_offset && (c.start_offset, c.end_offset) != (b.start_offset, b.end_offset) && c.depth > b.depth)
        .map(|c| (c.start_offset, c.end_offset))
        .collect();
    let kind = b.name.as_str();
    let items_by_heading = item_headed(kind);
    let mut out = Vec::new();
    let mut off = b.start_offset;
    let mut first = true;
    let mut headline_seen = false;
    let mut subtitle_seen = false;
    let mut item = 0usize;
    let mut heading_item_open = false;
    let mut list_item = 0usize;
    let mut body = 0usize;
    let mut row = 0usize;
    for line in source[b.start_offset..b.end_offset].split('\n') {
        let (start, end) = (off, off + line.len());
        off = end + 1;
        let in_child = children.iter().any(|(s, e)| start >= *s && start < *e);
        let t = line.trim();
        if first {
            first = false;
            if t.starts_with("::") {
                out.push(LineSlot { start, end, slot: String::new(), visible: String::new(), attrs: opener_attr_spans(line).into_iter().map(|a| AttrSpan { key: a.key, start: start + a.start, end: start + a.end }).collect(), cells: Vec::new() });
                continue;
            }
        }
        if in_child || t.is_empty() || t == "::" || t.starts_with("::") {
            continue;
        }
        if let Some((key, vs, ve)) = body_key_line(line) {
            out.push(LineSlot { start, end, slot: format!("attr({key})"), visible: line[vs..ve].trim().to_string(), attrs: vec![AttrSpan { key, start: start + vs, end: start + ve }], cells: Vec::new() });
            continue;
        }
        let level = heading_level(t);
        let slot = if t.starts_with('|') {
            if is_table_separator(t) {
                continue;
            }
            row += 1;
            format!("cell({row},0)")
        } else if level > 0 {
            if items_by_heading && level >= 2 {
                item += 1;
                heading_item_open = true;
                format!("item({item})")
            } else if !headline_seen {
                headline_seen = true;
                "headline".to_string()
            } else {
                "subtitle".to_string()
            }
        } else if is_list_item(t) {
            if heading_item_open {
                format!("item({item})")
            } else {
                list_item += 1;
                format!("item({list_item})")
            }
        } else if heading_item_open {
            format!("item({item})")
        } else if kind == "hero" && headline_seen && !subtitle_seen {
            subtitle_seen = true;
            "subtitle".to_string()
        } else {
            body += 1;
            format!("body({body})")
        };
        let cells = if slot.starts_with("cell(") {
            let indent = line.len() - line.trim_start().len();
            let mut cells = Vec::new();
            let mut cell_start = None;
            let bytes = line.as_bytes();
            let mut i = indent;
            while i < bytes.len() {
                if bytes[i] == b'|' && (i == 0 || bytes[i - 1] != b'\\') {
                    if let Some(cs) = cell_start.take() {
                        cells.push((start + cs, start + i));
                    }
                    cell_start = Some(i + 1);
                }
                i += 1;
            }
            if let Some(cs) = cell_start
                && cs < line.len()
                && !line[cs..].trim().is_empty()
            {
                cells.push((start + cs, start + line.len()));
            }
            cells
        } else {
            Vec::new()
        };
        out.push(LineSlot { start, end, slot, visible: visible_text(line), attrs: Vec::new(), cells });
    }
    out
}

/// The first visible text of a block, ≤ 120 chars, markup stripped — for a
/// `::site` its name, for a `::page` its title.
pub fn block_text(source: &str, all: &[BlockRef], b: &BlockRef) -> Option<String> {
    block_text_n(&normalise(source), all, b)
}

fn block_text_n(source: &str, all: &[BlockRef], b: &BlockRef) -> Option<String> {
    let slots = line_slots(source, all, b);
    if matches!(b.name.as_str(), "site" | "page") {
        let opener = source[b.start_offset..b.end_offset].lines().next().unwrap_or("");
        for key in ["title", "name"] {
            if let Some(a) = opener_attr_spans(opener).into_iter().find(|a| a.key == key) {
                return Some(clip(opener[a.start..a.end].trim(), 120)).filter(|s| !s.is_empty());
            }
            if let Some(l) = slots.iter().find(|l| l.slot == format!("attr({key})")) {
                return Some(clip(&l.visible, 120)).filter(|s| !s.is_empty());
            }
        }
        return None;
    }
    if b.name == "markdown" {
        let first = source[b.start_offset..b.end_offset].lines().find(|l| !l.trim().is_empty())?;
        return Some(clip(&visible_text(first), 120)).filter(|s| !s.is_empty());
    }
    let from_line = slots.iter().find(|l| !l.slot.is_empty() && !l.slot.starts_with("attr(") && !l.visible.is_empty()).map(|l| l.visible.clone());
    let from_attr = || {
        let opener = source[b.start_offset..b.end_offset].lines().next().unwrap_or("");
        ["title", "label", "headline", "name", "text", "brand"]
            .iter()
            .find_map(|key| opener_attr_spans(opener).into_iter().find(|a| &a.key == key).map(|a| opener[a.start..a.end].trim().to_string()))
            .filter(|s| !s.is_empty())
    };
    from_line.or_else(from_attr).map(|s| clip(&s, 120))
}

/// How many repeated things a block holds — list items, heading-started
/// items (cards, questions, steps), table rows (the header aside), or child
/// blocks; `None` for a block with none.
pub fn block_count(source: &str, all: &[BlockRef], b: &BlockRef) -> Option<usize> {
    block_count_n(&normalise(source), all, b)
}

fn block_count_n(source: &str, all: &[BlockRef], b: &BlockRef) -> Option<usize> {
    let slots = line_slots(source, all, b);
    let items = slots.iter().filter_map(|l| l.slot.strip_prefix("item(")).filter_map(|s| s.trim_end_matches(')').parse::<usize>().ok()).max().unwrap_or(0);
    if items > 0 {
        return Some(items);
    }
    let rows = slots.iter().filter(|l| l.slot.starts_with("cell(")).count();
    if rows > 1 {
        return Some(rows - 1);
    }
    let children = all.iter().filter(|c| c.start_offset >= b.start_offset && c.end_offset <= b.end_offset && c.depth == b.depth + 1).count();
    if children > 0 { Some(children) } else { None }
}

// ── find_text ──────────────────────────────────────────────────────────

/// The deepest listed block whose span holds `offset`.
fn block_at(all: &[BlockRef], offset: usize) -> Option<usize> {
    all.iter()
        .enumerate()
        .filter(|(_, b)| b.start_offset <= offset && offset < b.end_offset)
        .max_by_key(|(_, b)| b.start_offset)
        .map(|(i, _)| i)
}

/// Every place `query` occurs in `source`, in document order. With `route`,
/// only that page's blocks and the site-wide (top-level) ones are searched —
/// what `read_page` shows for a route. Each hit says which block, which slot,
/// and whether it is the phrase verbatim; [`resolve`] applies the policy.
pub fn find_text(source: &str, query: &str, route: Option<&str>) -> Vec<TextHit> {
    let source = normalise(source);
    let Some(q) = normalise_query(query) else { return Vec::new() };
    let raw_variants = raw_query_variants(query);
    // The query's trailing punctuation, when the page has it too, is part of the match.
    let trailing = raw_variants[0][raw_variants[1].len()..].to_string();
    let all = list_blocks(&source);
    let mut slot_cache: BTreeMap<usize, Vec<LineSlot>> = BTreeMap::new();
    let mut page_routes: BTreeMap<usize, Option<String>> = BTreeMap::new();
    let mut out = Vec::new();
    let mut off = 0usize;
    for (line_ix, raw) in source.split('\n').enumerate() {
        let line_start = off;
        off += raw.len() + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let norm = normalise_line(raw);
        for (ls, mut le) in matches_in_line(raw, &norm, &q) {
            if !trailing.is_empty() && raw[le..].starts_with(&trailing) {
                le += trailing.len();
            }
            let abs = line_start + ls;
            let Some(bi) = block_at(&all, abs) else { continue };
            let b = &all[bi];
            let is_page = b.name == "page";
            let b_route = if is_page {
                page_routes.entry(bi).or_insert_with(|| page_route(b.start_offset, &source)).clone()
            } else {
                b.route.clone()
            };
            if let Some(r) = route
                && b_route.as_deref() != Some(r)
                && b_route.is_some()
            {
                continue;
            }
            let slots = slot_cache.entry(bi).or_insert_with(|| line_slots(&source, &all, b));
            let Some(ls_row) = slots.iter().find(|l| l.start <= abs && abs < l.end) else { continue };
            // On an opener or a `key: value` line the match must sit inside a value.
            let attr = ls_row.attrs.iter().find(|a| a.start <= abs && abs < a.end && text_attr(&a.key));
            let (slot, snippet) = if let Some(a) = attr {
                (format!("attr({})", a.key), clip(source[a.start..a.end].trim(), 120))
            } else if ls_row.slot.is_empty() || ls_row.slot.starts_with("attr(") {
                continue;
            } else if ls_row.slot.starts_with("cell(") {
                let cell = ls_row.cells.iter().position(|(s, e)| *s <= abs && abs < *e);
                let col = cell.map(|c| c + 1).unwrap_or(0);
                let row = ls_row.slot.trim_start_matches("cell(").split(',').next().unwrap_or("0");
                let words = cell.map(|c| visible_text(&source[ls_row.cells[c].0..ls_row.cells[c].1])).unwrap_or_else(|| ls_row.visible.clone());
                (format!("cell({row},{col})"), clip(&words, 120))
            } else {
                (ls_row.slot.clone(), clip(&ls_row.visible, 120))
            };
            let matched = &source[abs..line_start + le];
            let exact = raw_variants.iter().any(|v| !v.is_empty() && v == matched);
            let (kind, id) = if is_page && attr.is_none() {
                ("markdown".to_string(), None)
            } else {
                (b.name.clone(), b.id.clone())
            };
            out.push(TextHit {
                id,
                kind,
                route: b_route,
                slot,
                line: line_ix + 1,
                col: raw[..ls].chars().count() + 1,
                len: matched.chars().count(),
                start_offset: abs,
                end_offset: line_start + le,
                exact,
                snippet,
            });
        }
    }
    out
}

/// The ONE ambiguity policy over a query's hits, everywhere:
/// 1. exactly one verbatim hit → it;
/// 2. none verbatim, exactly one normalised → it;
/// 3. several, and exactly one of them on `current_route` → it;
/// 4. still several → never guess: the candidates;
/// 5. none → nothing.
pub fn resolve(hits: &[TextHit], current_route: Option<&str>) -> Resolved {
    let on_route = |set: &[&TextHit]| -> Option<TextHit> {
        let cr = current_route?;
        let mine: Vec<&&TextHit> = set.iter().filter(|h| h.route.as_deref() == Some(cr)).collect();
        (mine.len() == 1).then(|| (*mine[0]).clone())
    };
    let exact: Vec<&TextHit> = hits.iter().filter(|h| h.exact).collect();
    match exact.len() {
        1 => return Resolved::Exact(exact[0].clone()),
        n if n > 1 => {
            return match on_route(&exact) {
                Some(h) => Resolved::CurrentRoute(h),
                None => Resolved::Ambiguous(exact.into_iter().cloned().collect()),
            };
        }
        _ => {}
    }
    let all: Vec<&TextHit> = hits.iter().collect();
    match all.len() {
        0 => Resolved::None,
        1 => Resolved::Normalized(all[0].clone()),
        _ => match on_route(&all) {
            Some(h) => Resolved::CurrentRoute(h),
            None => Resolved::Ambiguous(hits.to_vec()),
        },
    }
}

/// [`find_text`] over the whole document + [`resolve`] against
/// `current_route`, as JSON: `{"hits":[…],"policy":"exact|normalized|current_route|ambiguous|none","pick":hit|null}`
/// — the FFI / wasm twin and the server tool's answer.
pub fn find_text_json(source: &str, query: &str, current_route: Option<&str>) -> String {
    let hits = find_text(source, query, None);
    let resolved = resolve(&hits, current_route);
    serde_json::json!({
        "hits": hits,
        "policy": resolved.policy(),
        "pick": resolved.pick(),
    })
    .to_string()
}

fn candidates_words(hits: &[TextHit]) -> String {
    hits.iter()
        .map(|h| format!("{} ({})", h.label(), h.route.as_deref().unwrap_or("site-wide")))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Replace `find` with `replace` INSIDE the line it sits on — the markup
/// around it kept. The scope is the block `id` (with `route` when the id is
/// on several pages), else the page `route` (loose Markdown included), else
/// the whole document; within the scope `find` must occur exactly once
/// (verbatim first, else normalised), or the call is refused with the
/// candidates named. `replace` is written as one line.
pub fn replace_text(
    source: &str,
    route: Option<&str>,
    id: Option<&str>,
    find: &str,
    replace: &str,
) -> Result<TextReplaced, EditError> {
    let source = normalise(source);
    let (hits, scope) = match id {
        Some(id) => {
            let target = self::find(&source, route, id)?;
            let hits: Vec<TextHit> = find_text(&source, find, None)
                .into_iter()
                .filter(|h| h.start_offset >= target.start_offset && h.end_offset <= target.end_offset)
                .collect();
            (hits, format!(" in block `{id}`"))
        }
        None => match route {
            Some(r) => {
                let known = routes(&source);
                if !known.iter().any(|k| k == r) {
                    return Err(EditError::UnknownRoute {
                        route: r.to_string(),
                        routes: if known.is_empty() { "(none)".to_string() } else { known.join(", ") },
                    });
                }
                let hits: Vec<TextHit> = find_text(&source, find, None).into_iter().filter(|h| h.route.as_deref() == Some(r)).collect();
                (hits, format!(" on page `{r}`"))
            }
            None => (find_text(&source, find, None), String::new()),
        },
    };
    let exact: Vec<&TextHit> = hits.iter().filter(|h| h.exact).collect();
    let hit = match exact.len() {
        1 => exact[0].clone(),
        n if n > 1 => {
            return Err(EditError::AmbiguousText {
                find: find.trim().to_string(),
                count: n,
                scope,
                candidates: candidates_words(&exact.into_iter().cloned().collect::<Vec<_>>()),
            });
        }
        _ => match hits.len() {
            0 => {
                return Err(EditError::TextNotFound {
                    find: find.trim().to_string(),
                    scope,
                });
            }
            1 => hits[0].clone(),
            n => {
                return Err(EditError::AmbiguousText {
                    find: find.trim().to_string(),
                    count: n,
                    scope,
                    candidates: candidates_words(&hits),
                });
            }
        },
    };
    let one_line: String = replace.split_whitespace().collect::<Vec<_>>().join(" ");
    let before = source[hit.start_offset..hit.end_offset].to_string();
    let out = splice(&source, hit.start_offset, hit.end_offset, &one_line);
    Ok(TextReplaced {
        source: out,
        hit,
        before,
        after: one_line,
    })
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

    // ── Text-anchored edits (0.29.0) ────────────────────────────────────

    /// A site with the shapes a person quotes: nav items, a hero with
    /// buttons, a loose paragraph, heading-started cards and questions, a
    /// hero headline with an anchor, curly quotes and emphasis, a table, an
    /// attribute label — and "Learn more" in three places, "Get a quote" on
    /// two pages.
    const TEXT: &str = "::site\nname: Bens Construction Co\naccent: #f97316\n::\n\n::nav[id=nav]\n- [Home](/)\n- [About](/about)\n::\n\n::footer[id=footer tagline=\"Tools, tracked.\"]\n© 2026 Bens Construction Co · [Learn more](/about)\n::\n\n::page[route=\"/\" title=\"Home\"]\n::hero[id=hero]\n# Every tool, on the truck\nWe track what we own.\n[Learn more](/about){primary}\n[Get a quote](/contact)\n::\n\nA loose paragraph between blocks stays loose.\n\n::features[id=features columns=3]\n### Fast\nA tool is checked out in seconds. [Learn more](/features)\n### Clear\nOne list for the whole crew.\n### Honest\nWhat is missing shows up **red**.\n::\n\n::faq[id=faq]\n### Do you deliver?\nYes — same day within the county.\n### Can I rent?\nWeekly and monthly rates.\n::\n::\n\n::page[route=\"/about\" title=\"About\"]\n::hero[id=hero]\n# About Bens {#about}\nFamily-run since 1998, it’s a **Bens** tradition.\n::\n\n::section[id=story]\n## The story\nOne truck, one list, no lost drills.\n\n| Year | Trucks |\n|------|--------|\n| 1998 | 1 |\n| 2026 | 12 |\n::\n\n::cta[id=cta label=\"Get a quote\" href=\"/contact\"]\n::\n::\n";

    fn one(hits: Vec<TextHit>) -> TextHit {
        assert_eq!(hits.len(), 1, "expected one hit, got {hits:?}");
        hits.into_iter().next().unwrap()
    }

    fn changed_lines(a: &str, b: &str) -> usize {
        assert_eq!(a.lines().count(), b.lines().count(), "a text edit never adds or removes a line");
        a.lines().zip(b.lines()).filter(|(x, y)| x != y).count()
    }

    #[test]
    fn exact_hit_is_the_phrase_verbatim() {
        let h = one(find_text(TEXT, "About Bens", None));
        assert!(h.exact);
        assert_eq!((h.id.as_deref(), h.kind.as_str(), h.route.as_deref(), h.slot.as_str()), (Some("hero"), "hero", Some("/about"), "headline"));
        assert_eq!(h.snippet, "About Bens", "the anchor is not visible text");
        assert_eq!(h.line, 44);
        assert_eq!(h.col, 3);
        assert_eq!(h.len, 10);
        assert_eq!(&TEXT.replace("\r\n", "\n")[h.start_offset..h.end_offset], "About Bens");
    }

    #[test]
    fn normaliser_rule_1_case() {
        let h = one(find_text(TEXT, "about bens", None));
        assert!(!h.exact);
        assert_eq!(h.slot, "headline");
    }

    #[test]
    fn normaliser_rule_2_whitespace_runs() {
        let h = one(find_text(TEXT, "We   track  what\twe own", None));
        assert!(!h.exact);
        assert_eq!((h.id.as_deref(), h.slot.as_str()), (Some("hero"), "subtitle"));
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "We track what we own");
    }

    #[test]
    fn normaliser_rule_3_quotes() {
        let h = one(find_text(TEXT, "it's a Bens tradition", None));
        assert!(!h.exact);
        assert_eq!(h.slot, "subtitle");
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "it’s a **Bens** tradition");
        // And the other way: a curly quote in the query finds straight text.
        let h = one(find_text("::page[route=\"/\"]\n::hero[id=h]\n# Ben's Tools\n::\n::\n", "Ben’s Tools", None));
        assert!(!h.exact);
    }

    #[test]
    fn normaliser_rule_4_emphasis_is_transparent_and_a_span_stays_balanced() {
        let h = one(find_text(TEXT, "shows up red", None));
        assert!(!h.exact);
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "shows up **red**", "the closing ** is pulled in");
        let h = one(find_text(TEXT, "Bens tradition", None));
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "**Bens** tradition", "the opening ** is pulled in");
        let out = replace_text(TEXT, None, None, "Bens tradition", "family tradition").unwrap();
        assert!(out.source.contains("Family-run since 1998, it’s a family tradition."));
        assert!(!out.source.contains("** tradition") && !out.source.contains("a ** "), "no half a marker left behind");
    }

    #[test]
    fn normaliser_rule_5_trailing_punctuation_of_the_query() {
        let h = one(find_text(TEXT, "About Bens.", None));
        assert!(h.exact, "the period is the person's, not the page's");
        let h = one(find_text(TEXT, "  Do you deliver?!  ", None));
        assert!(h.exact);
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "Do you deliver");
    }

    #[test]
    fn normaliser_rule_6_ellipsis_in_the_middle() {
        let h = one(find_text(TEXT, "Every tool … truck", None));
        assert!(!h.exact);
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "Every tool, on the truck");
        let h = one(find_text(TEXT, "one truck ... drills", None));
        assert_eq!(&TEXT[h.start_offset..h.end_offset], "One truck, one list, no lost drills");
        assert!(find_text(TEXT, "Every tool … nowhere", None).is_empty(), "the tail must follow on the same line");
    }

    #[test]
    fn normaliser_rule_7_hrefs_and_brace_suffixes_are_invisible() {
        // `about` is in two hrefs and one `{#about}`; only the visible words hit.
        let hits = find_text(TEXT, "about", None);
        let where_: Vec<String> = hits.iter().map(|h| h.label()).collect();
        assert_eq!(where_, vec!["nav › item 2", "page › title", "hero › headline"], "{hits:?}");
        assert!(find_text(TEXT, "/contact", None).is_empty());
        assert!(find_text(TEXT, "primary", None).is_empty());
    }

    #[test]
    fn an_empty_or_punctuation_only_query_finds_nothing() {
        assert!(find_text(TEXT, "", None).is_empty());
        assert!(find_text(TEXT, " ... ", None).is_empty());
        assert!(find_text(TEXT, "?!", None).is_empty());
    }

    #[test]
    fn slots_name_where_in_the_block_the_text_sits() {
        let slot = |q: &str| {
            let h = one(find_text(TEXT, q, None).into_iter().filter(|h| h.kind != "page").collect());
            (h.id.clone().unwrap_or(h.kind.clone()), h.slot.clone(), h.snippet.clone())
        };
        assert_eq!(slot("Every tool, on the truck"), ("hero".into(), "headline".into(), "Every tool, on the truck".into()));
        assert_eq!(slot("We track what we own"), ("hero".into(), "subtitle".into(), "We track what we own.".into()));
        assert_eq!(slot("[Get a quote](/contact)"), ("hero".into(), "body(2)".into(), "Get a quote".into()));
        assert_eq!(slot("Home"), ("nav".into(), "item(1)".into(), "Home".into()));
        assert_eq!(slot("Clear"), ("features".into(), "item(2)".into(), "Clear".into()));
        assert_eq!(slot("whole crew"), ("features".into(), "item(2)".into(), "One list for the whole crew.".into()));
        assert_eq!(slot("Can I rent"), ("faq".into(), "item(2)".into(), "Can I rent?".into()));
        assert_eq!(slot("Weekly and monthly"), ("faq".into(), "item(2)".into(), "Weekly and monthly rates.".into()));
        assert_eq!(slot("The story"), ("story".into(), "headline".into(), "The story".into()));
        assert_eq!(slot("no lost drills"), ("story".into(), "body(1)".into(), "One truck, one list, no lost drills.".into()));
        assert_eq!(slot("Trucks"), ("story".into(), "cell(1,2)".into(), "Trucks".into()));
        assert_eq!(visible_text("| Year | Trucks |"), "Year · Trucks");
        assert_eq!(slot("Tools, tracked"), ("footer".into(), "attr(tagline)".into(), "Tools, tracked.".into()));
        assert_eq!(slot("Bens Construction Co ·"), ("footer".into(), "body(1)".into(), "© 2026 Bens Construction Co · Learn more".into()));
        let site = find_text(TEXT, "Bens Construction Co", None).into_iter().find(|h| h.kind == "site").unwrap();
        assert_eq!((site.slot.as_str(), site.snippet.as_str()), ("attr(name)", "Bens Construction Co"));
    }

    #[test]
    fn a_loose_paragraph_under_a_page_is_markdown_with_no_id_on_that_route() {
        let h = one(find_text(TEXT, "stays loose", None));
        assert_eq!(h.label(), "markdown › body 1");
        assert_eq!((h.id, h.kind.as_str(), h.route.as_deref(), h.slot.as_str()), (None, "markdown", Some("/"), "body(1)"));
    }

    #[test]
    fn a_page_title_is_an_attr_hit_on_the_page() {
        let h = one(find_text(TEXT, "Home", None).into_iter().filter(|h| h.kind == "page").collect());
        assert_eq!((h.id, h.route.as_deref(), h.slot.as_str()), (None, Some("/"), "attr(title)"));
    }

    #[test]
    fn a_directive_name_or_key_never_hits() {
        assert!(find_text(TEXT, "features", None).is_empty(), "{:?}", find_text(TEXT, "features", None));
        assert!(find_text(TEXT, "columns", None).is_empty());
        assert!(find_text(TEXT, "accent", None).is_empty(), "the key of a key: value line is not text");
        assert!(find_text(TEXT, "#f97316", None).is_empty(), "a colour is not text");
        assert!(find_text(TEXT, "cta", None).is_empty(), "an id is not text");
    }

    #[test]
    fn a_route_narrows_the_search_to_that_page_plus_the_site_wide_blocks() {
        let on_about: Vec<String> = find_text(TEXT, "Learn more", Some("/about")).iter().map(|h| h.label()).collect();
        assert_eq!(on_about, vec!["footer › body 1"]);
        let on_home: Vec<String> = find_text(TEXT, "Learn more", Some("/")).iter().map(|h| h.label()).collect();
        assert_eq!(on_home, vec!["footer › body 1", "hero › body 1", "features › item 1"]);
        let all = find_text(TEXT, "Get a quote", None);
        assert_eq!(all.len(), 2);
        assert_eq!(find_text(TEXT, "Get a quote", Some("/about")).len(), 1);
    }

    #[test]
    fn policy_1_exact_once() {
        let hits = find_text(TEXT, "About Bens", None);
        assert!(matches!(resolve(&hits, Some("/")), Resolved::Exact(ref h) if h.slot == "headline"));
        assert_eq!(resolve(&hits, None).policy(), "exact");
    }

    #[test]
    fn policy_2_normalised_once() {
        let hits = find_text(TEXT, "about bens", None);
        assert!(matches!(resolve(&hits, None), Resolved::Normalized(_)));
    }

    #[test]
    fn policy_3_the_current_route_decides_between_pages() {
        let hits = find_text(TEXT, "Get a quote", None);
        assert_eq!(hits.len(), 2);
        match resolve(&hits, Some("/about")) {
            Resolved::CurrentRoute(h) => assert_eq!(h.label(), "cta › label"),
            other => panic!("{other:?}"),
        }
        match resolve(&hits, Some("/")) {
            Resolved::CurrentRoute(h) => assert_eq!(h.label(), "hero › body 2"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn policy_4_never_guesses_and_names_the_candidates() {
        let hits = find_text(TEXT, "Learn more", None);
        match resolve(&hits, Some("/")) {
            Resolved::Ambiguous(c) => {
                let labels: Vec<String> = c.iter().map(|h| h.label()).collect();
                assert_eq!(labels, vec!["footer › body 1", "hero › body 1", "features › item 1"]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(resolve(&hits, Some("/pricing")), Resolved::Ambiguous(_)));
        assert!(matches!(resolve(&hits, None), Resolved::Ambiguous(_)));
        // Several exact and several normalised: the exact ones are the candidates.
        let src = "::page[route=\"/\"]\n::hero[id=a]\n# Fast\n::\n::hero[id=b]\n# Fast\n::\n::hero[id=c]\n# fast\n::\n::\n";
        match resolve(&find_text(src, "Fast", None), None) {
            Resolved::Ambiguous(c) => assert_eq!(c.len(), 2),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn policy_5_nothing() {
        let hits = find_text(TEXT, "flux capacitor", None);
        assert!(hits.is_empty());
        assert_eq!(resolve(&hits, Some("/")), Resolved::None);
        assert_eq!(Resolved::None.policy(), "none");
        assert!(Resolved::None.pick().is_none());
    }

    #[test]
    fn find_text_json_carries_hits_policy_and_pick() {
        let v: serde_json::Value = serde_json::from_str(&find_text_json(TEXT, "Get a quote", Some("/about"))).unwrap();
        assert_eq!(v["policy"], "current_route");
        assert_eq!(v["hits"].as_array().unwrap().len(), 2);
        assert_eq!(v["pick"]["id"], "cta");
        assert_eq!(v["pick"]["slot"], "attr(label)");
        let v: serde_json::Value = serde_json::from_str(&find_text_json(TEXT, "Learn more", Some("/"))).unwrap();
        assert_eq!(v["policy"], "ambiguous");
        assert!(v["pick"].is_null());
        let v: serde_json::Value = serde_json::from_str(&find_text_json(TEXT, "nope", None)).unwrap();
        assert_eq!(v["policy"], "none");
        assert_eq!(v["hits"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn slot_words_read_like_a_pill() {
        assert_eq!(slot_words("headline"), "headline");
        assert_eq!(slot_words("item(2)"), "item 2");
        assert_eq!(slot_words("body(1)"), "body 1");
        assert_eq!(slot_words("attr(label)"), "label");
        assert_eq!(slot_words("cell(2,3)"), "row 2 col 3");
    }

    // replace_text — the eight shapes, each markup-preserving.

    #[test]
    fn replace_text_in_a_nav_item_keeps_the_link() {
        let out = replace_text(TEXT, None, Some("nav"), "About", "Our story").unwrap();
        assert!(out.source.contains("- [Our story](/about)\n"));
        assert_eq!((out.before.as_str(), out.after.as_str(), out.hit.slot.as_str()), ("About", "Our story", "item(2)"));
        assert_eq!(changed_lines(TEXT, &out.source), 1);
    }

    #[test]
    fn replace_text_in_a_second_card_title_keeps_the_heading() {
        let out = replace_text(TEXT, Some("/"), Some("features"), "Clear", "Crystal clear").unwrap();
        assert!(out.source.contains("### Crystal clear\nOne list"));
        assert_eq!(out.hit.label(), "features › item 2");
    }

    #[test]
    fn replace_text_in_a_faq_answer() {
        let out = replace_text(TEXT, Some("/"), Some("faq"), "same day within the county", "next day, county-wide").unwrap();
        assert!(out.source.contains("Yes — next day, county-wide.\n"));
        assert_eq!(out.hit.label(), "faq › item 1");
    }

    #[test]
    fn replace_text_in_a_button_label_keeps_href_and_style() {
        let out = replace_text(TEXT, Some("/"), Some("hero"), "Learn more", "See the work").unwrap();
        assert!(out.source.contains("[See the work](/about){primary}\n"));
        assert!(out.source.contains("[Learn more](/features)"), "the card's link is untouched");
        assert!(out.source.contains("· [Learn more](/about)\n"), "the footer's link is untouched");
    }

    #[test]
    fn replace_text_in_a_headline_keeps_the_anchor() {
        let out = replace_text(TEXT, Some("/about"), Some("hero"), "About Bens", "About Ben's Construction").unwrap();
        assert!(out.source.contains("# About Ben's Construction {#about}\n"));
        assert!(out.source.contains("- [About](/about)"), "the nav item is untouched");
    }

    #[test]
    fn replace_text_reaches_a_loose_paragraph_through_the_page_scope() {
        let out = replace_text(TEXT, Some("/"), None, "stays loose", "is still loose").unwrap();
        assert!(out.source.contains("A loose paragraph between blocks is still loose.\n"));
        assert_eq!((out.hit.id, out.hit.kind.as_str()), (None, "markdown"));
    }

    #[test]
    fn replace_text_in_an_attribute_value_keeps_the_quotes() {
        let out = replace_text(TEXT, Some("/about"), Some("cta"), "Get a quote", "Book a visit").unwrap();
        assert!(out.source.contains("::cta[id=cta label=\"Book a visit\" href=\"/contact\"]"));
        assert!(out.source.contains("[Get a quote](/contact)"), "the home hero's button is untouched");
        let opener = out.source.lines().find(|l| l.starts_with("::cta[")).unwrap();
        let attrs = crate::attrs::parse_attrs(&opener[opener.find('[').unwrap()..]).unwrap();
        assert_eq!(attrs.get("label"), Some(&crate::types::AttrValue::String("Book a visit".into())));
    }

    #[test]
    fn replace_text_in_a_table_cell() {
        let out = replace_text(TEXT, Some("/about"), Some("story"), "Trucks", "Fleet").unwrap();
        assert!(out.source.contains("| Year | Fleet |\n"));
        assert_eq!(out.hit.slot, "cell(1,2)");
    }

    #[test]
    fn replace_text_refuses_an_ambiguous_or_missing_phrase_and_names_the_scope() {
        let err = replace_text(TEXT, None, None, "Learn more", "x").unwrap_err();
        match &err {
            EditError::AmbiguousText { count, candidates, scope, .. } => {
                assert_eq!(*count, 3);
                assert!(candidates.contains("hero › body 1 (/)") && candidates.contains("footer › body 1 (site-wide)"), "{candidates}");
                assert_eq!(scope, "");
            }
            other => panic!("{other}"),
        }
        let err = replace_text(TEXT, Some("/"), None, "Learn more", "x").unwrap_err();
        assert!(matches!(err, EditError::AmbiguousText { count: 2, .. }), "{err}");
        assert!(err.to_string().contains("on page `/`"));
        let err = replace_text(TEXT, Some("/"), Some("faq"), "Learn more", "x").unwrap_err();
        assert!(matches!(err, EditError::TextNotFound { .. }), "{err}");
        assert!(err.to_string().contains("in block `faq`"));
        let err = replace_text(TEXT, Some("/pricing"), None, "x", "y").unwrap_err();
        assert!(matches!(err, EditError::UnknownRoute { .. }), "{err}");
        let err = replace_text(TEXT, None, Some("nope"), "x", "y").unwrap_err();
        assert!(matches!(err, EditError::UnknownId { .. }), "{err}");
    }

    #[test]
    fn replace_text_prefers_the_verbatim_hit_inside_the_scope() {
        // "red" is verbatim in card 3 and normalised nowhere else in the block.
        let out = replace_text(TEXT, Some("/"), Some("features"), "red", "in red").unwrap();
        assert!(out.source.contains("shows up **in red**."));
        // Site-wide, "Bens" (verbatim ×4) is ambiguous; "bens tradition" (normalised once) is not.
        assert!(matches!(replace_text(TEXT, None, None, "Bens", "x").unwrap_err(), EditError::AmbiguousText { .. }));
        assert!(replace_text(TEXT, None, None, "bens tradition", "x").is_ok());
    }

    #[test]
    fn replace_text_writes_one_line_and_may_remove_the_phrase() {
        let out = replace_text(TEXT, Some("/"), Some("hero"), "We track what we own.", "We track\nwhat we\n  own, daily.").unwrap();
        assert!(out.source.contains("# Every tool, on the truck\nWe track what we own, daily.\n"));
        assert_eq!(changed_lines(TEXT, &out.source), 1);
        let out = replace_text(TEXT, Some("/"), Some("hero"), ", on the truck", "").unwrap();
        assert!(out.source.contains("# Every tool\n"));
        let parsed = crate::parse::parse(&out.source);
        assert!(parsed.diagnostics.iter().all(|d| d.severity != crate::error::Severity::Error));
    }

    #[test]
    fn apply_json_carries_replace_text() {
        let out = apply_json(TEXT, r#"{"op":"replace_text","route":"/about","id":"hero","find":"About Bens","replace":"About us"}"#).unwrap();
        assert!(out.contains("# About us {#about}"));
        let out = apply_json(TEXT, r#"{"op":"replace_text","route":"/","find":"stays loose","replace":"is loose"}"#).unwrap();
        assert!(out.contains("blocks is loose."));
        let err = apply_json(TEXT, r#"{"op":"replace_text","id":"hero"}"#).unwrap_err();
        assert!(matches!(err, EditError::MissingField { ref field, .. } if field == "find"), "{err}");
        let out = apply_json(TEXT, r#"{"op":"replace_text","route":"/","id":"hero","find":", on the truck"}"#).unwrap();
        assert!(out.contains("# Every tool\n"), "no replace = the phrase goes");
        assert!(EditError::UnknownOp { op: "x".into() }.to_string().contains("replace_text"));
    }

    // The find quirks (P3).

    #[test]
    fn a_same_page_duplicate_id_is_refused_even_with_a_route() {
        let src = "::page[route=\"/\"]\n::hero[id=hero]\n# A\n::\n::hero[id=hero]\n# B\n::\n::\n";
        let err = set_text(src, Some("/"), "hero", "x").unwrap_err();
        assert!(matches!(err, EditError::DuplicateId { count: 2, .. }), "{err}");
        assert!(err.to_string().contains("on page `/`"));
    }

    #[test]
    fn a_site_wide_block_is_found_whatever_route_is_passed() {
        let out = set_attr(TEXT, Some("/about"), "nav", "cta-label", "Call").unwrap();
        assert!(out.contains("::nav[id=nav cta-label=Call]"));
        let out = set_text(TEXT, Some("/"), "footer", "© 2026 Bens").unwrap();
        assert!(out.contains("© 2026 Bens\n::"));
        // A page's own block still wins over nothing: the about hero with route /about.
        assert!(set_text(TEXT, Some("/about"), "hero", "x").is_ok());
        assert!(matches!(set_text(TEXT, None, "hero", "x").unwrap_err(), EditError::AmbiguousId { .. }));
    }

    // list_blocks' text + count (P4).

    #[test]
    fn list_blocks_carries_each_blocks_first_visible_text_and_count() {
        let got: Vec<(String, Option<String>, Option<usize>)> = list_blocks(TEXT).into_iter().map(|b| (b.id.unwrap_or(b.name), b.text, b.count)).collect();
        assert_eq!(
            got,
            vec![
                ("site".into(), Some("Bens Construction Co".into()), None),
                ("nav".into(), Some("Home".into()), Some(2)),
                ("footer".into(), Some("© 2026 Bens Construction Co · Learn more".into()), None),
                ("page".into(), Some("Home".into()), Some(3)),
                ("hero".into(), Some("Every tool, on the truck".into()), None),
                ("features".into(), Some("Fast".into()), Some(3)),
                ("faq".into(), Some("Do you deliver?".into()), Some(2)),
                ("page".into(), Some("About".into()), Some(3)),
                ("hero".into(), Some("About Bens".into()), None),
                ("story".into(), Some("The story".into()), Some(2)),
                ("cta".into(), Some("Get a quote".into()), None),
            ]
        );
    }

    #[test]
    fn block_text_is_clipped_and_markup_free() {
        let long = format!("::page[route=\"/\"]\n::hero[id=h]\n# {}\n::\n::\n", "word ".repeat(40));
        let hero = list_blocks(&long).into_iter().find(|b| b.name == "hero").unwrap();
        let text = hero.text.unwrap();
        assert!(text.chars().count() <= 120);
        assert!(text.ends_with('…'));
        assert_eq!(visible_text("- **Fast** — [a link](/x){primary}"), "Fast — a link");
        assert_eq!(visible_text("## The story {#story}"), "The story");
        assert_eq!(visible_text("3. `code` here"), "code here");
        assert_eq!(visible_text("![alt text](/img.png)"), "alt text");
    }

    #[test]
    fn block_ref_json_grows_only() {
        let listed: Vec<BlockRef> = serde_json::from_str(&list_blocks_json(TEXT)).unwrap();
        assert_eq!(listed.len(), 11);
        assert_eq!(listed[1].text.as_deref(), Some("Home"));
        assert_eq!(listed[1].count, Some(2));
        // A 0.28.0 listing (no text, no count) still decodes.
        let old = r#"[{"name":"hero","id":"hero","label":null,"route":"/","depth":1,"start_line":1,"end_line":3,"start_offset":0,"end_offset":10}]"#;
        let listed: Vec<BlockRef> = serde_json::from_str(old).unwrap();
        assert_eq!(listed[0].text, None);
        assert_eq!(listed[0].count, None);
    }

    #[test]
    fn a_text_edit_round_trips_through_the_parser() {
        for (route, id, find, replace) in [
            (Some("/"), Some("hero"), "Every tool, on the truck", "Every tool, accounted for"),
            (Some("/"), None, "stays loose", "is loose"),
            (Some("/about"), Some("story"), "Trucks", "Fleet"),
            (None, Some("nav"), "About", "Story"),
        ] {
            let out = replace_text(TEXT, route, id, find, replace).unwrap();
            let before = crate::parse::parse(TEXT);
            let after = crate::parse::parse(&out.source);
            assert_eq!(before.doc.blocks.len(), after.doc.blocks.len(), "{find}");
            assert!(after.diagnostics.iter().all(|d| d.severity != crate::error::Severity::Error), "{find}: {:?}", after.diagnostics);
            assert_eq!(list_blocks(TEXT).len(), list_blocks(&out.source).len(), "{find}");
        }
    }

    #[test]
    fn crlf_text_edits_land_on_normalised_offsets() {
        let crlf = TEXT.replace('\n', "\r\n");
        let out = replace_text(&crlf, Some("/about"), Some("hero"), "About Bens", "About us").unwrap();
        assert!(out.source.contains("# About us {#about}\n"));
        assert!(!out.source.contains('\r'));
        let h = one(find_text(&crlf, "About Bens", None));
        assert_eq!(h.line, 44);
    }
}

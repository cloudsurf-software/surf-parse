//! Citation & bibliography engine (CSL-style, deterministic).
//!
//! This module is the load-bearing deliverable of Chunk 4: a *pure*,
//! deterministic formatting engine that produces correct in-text citations and
//! reference-list entries for the academic styles carried by [`Format`]
//! (MLA, APA 7, Chicago author-date, IEEE, plus ACM ≈ IEEE numbering and the
//! generic `article` paper style ≈ APA author-date).
//!
//! The data model ([`Reference`], [`Author`], [`RefType`]) is consumed both by
//! the `::cite` block (a stored definition) and by Chunk 6 (paper/report
//! rendering). The formatting functions ([`format_in_text`],
//! [`format_reference`], [`reference_list`], [`reference_list_keyed`]) are pure:
//! same input → byte-identical output, no time / randomness / hashmap-iteration.
//!
//! Inline citations (`[@key]`, `[@key, p. 12]`, `[@key1; @key2]`) are scanned by
//! [`crate::inline::find_inline_cites`] into [`CiteRef`] values, resolved against
//! a [`CiteContext`] (the active style + the document's references + IEEE
//! numbering). Renderers install a context via [`install_context`] and read it
//! through [`with_active`] / [`substitute_text_cites`].

use std::collections::{BTreeMap, BTreeSet};
use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::types::{Block, Format};

// ───────────────────────────────────────────────────────────────────────────
// Data model
// ───────────────────────────────────────────────────────────────────────────

/// A single author/editor name, split into family (surname) + optional given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Author {
    /// Surname / family name (e.g. "Smith").
    pub family: String,
    /// Given name(s) as authored (e.g. "John", "Mary Jane"). `None` for
    /// mononyms / organisations.
    pub given: Option<String>,
}

/// Source category for a [`Reference`]; selects per-style entry shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RefType {
    /// Journal / periodical article (default).
    #[default]
    Article,
    /// Whole book.
    Book,
    /// Chapter in an edited book.
    Chapter,
    /// Web page / online resource.
    Web,
    /// Technical report / white paper.
    Report,
    /// Conference / proceedings paper.
    Conference,
}

impl RefType {
    /// Parse a `type=` value (case-insensitive). Unknown → `Article`.
    pub fn from_str_lossy(s: &str) -> RefType {
        match s.trim().to_ascii_lowercase().as_str() {
            "book" => RefType::Book,
            "chapter" | "incollection" | "bookchapter" => RefType::Chapter,
            "web" | "website" | "online" | "webpage" => RefType::Web,
            "report" | "techreport" | "whitepaper" => RefType::Report,
            "conference" | "proceedings" | "inproceedings" | "paper" => RefType::Conference,
            _ => RefType::Article,
        }
    }
}

/// A bibliographic reference. Optional fields use `Option`; the `key` is the
/// stable id used by inline `[@key]` citations and bibliography anchors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Reference {
    /// Citation key / id (e.g. `smith2020`).
    pub key: String,
    /// Source category (article/book/web/…).
    pub ref_type: RefType,
    /// Authors in listed order.
    pub authors: Vec<Author>,
    /// Editors (for edited books / chapters).
    pub editors: Vec<Author>,
    /// Title of the work.
    pub title: Option<String>,
    /// Container: journal / book / website / proceedings name.
    pub container: Option<String>,
    /// Publisher (books / reports).
    pub publisher: Option<String>,
    /// Year of publication (kept as a string to preserve `n.d.`/`2020a`).
    pub year: Option<String>,
    /// Volume number.
    pub volume: Option<String>,
    /// Issue number.
    pub issue: Option<String>,
    /// Page range (e.g. `45-67`).
    pub pages: Option<String>,
    /// URL.
    pub url: Option<String>,
    /// DOI (without the `https://doi.org/` prefix).
    pub doi: Option<String>,
    /// Access date (ISO `YYYY-MM-DD`) for web sources.
    pub accessed: Option<String>,
    /// Edition (e.g. `2nd`).
    pub edition: Option<String>,
}

// ───────────────────────────────────────────────────────────────────────────
// Inline citation references (produced by inline.rs)
// ───────────────────────────────────────────────────────────────────────────

/// One key inside an inline citation, with an optional locator (`p. 12`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiteItem {
    /// Reference key (matches [`Reference::key`]).
    pub key: String,
    /// Locator text exactly as authored (e.g. `p. 12`, `pp. 3-4`, `sec. 2`).
    pub locator: Option<String>,
}

/// A single inline citation group: `[@a]` is one item, `[@a; @b]` is two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiteRef {
    /// One or more cited items, in authored order.
    pub items: Vec<CiteItem>,
}

// ───────────────────────────────────────────────────────────────────────────
// Author parsing helpers
// ───────────────────────────────────────────────────────────────────────────

fn opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Parse a single author. Accepts `Last, First` (preferred) or `First Last`.
/// A single token becomes a family-only name (mononym / organisation).
pub fn parse_author(s: &str) -> Author {
    let s = s.trim();
    if let Some((last, first)) = s.split_once(',') {
        Author {
            family: last.trim().to_string(),
            given: opt(first),
        }
    } else if let Some(idx) = s.rfind(' ') {
        let (given, family) = s.split_at(idx);
        Author {
            family: family.trim().to_string(),
            given: opt(given),
        }
    } else {
        Author {
            family: s.to_string(),
            given: None,
        }
    }
}

/// Parse a `;`-separated author list (`Last, First; Last2, First2`).
pub fn parse_authors(s: &str) -> Vec<Author> {
    s.split(';')
        .map(parse_author)
        .filter(|a| !a.family.is_empty())
        .collect()
}

/// Initials from a given name: `John` → `J.`, `Mary Jane` → `M. J.`.
fn initials(given: &Option<String>) -> String {
    match given {
        None => String::new(),
        Some(g) => g
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .map(|c| format!("{}.", c.to_uppercase()))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// `Family` (no given) or `Family, Given` (inverted form).
fn inverted(a: &Author) -> String {
    match &a.given {
        Some(g) => format!("{}, {}", a.family, g),
        None => a.family.clone(),
    }
}

/// `Family` (no given) or `Given Family` (natural form).
fn natural(a: &Author) -> String {
    match &a.given {
        Some(g) => format!("{} {}", g, a.family),
        None => a.family.clone(),
    }
}

/// Strip a leading page-locator prefix (`p.`/`pp.`/`page`/`pages`) so styles
/// that want a bare number in-text (MLA, Chicago) render `12` not `p. 12`.
fn strip_page_prefix(s: &str) -> String {
    let t = s.trim();
    for p in ["pp.", "pages", "page", "pg.", "p."] {
        if let Some(rest) = t.strip_prefix(p) {
            return rest.trim().to_string();
        }
    }
    t.to_string()
}

/// MLA access-date format: `2021-05-01` → `1 May 2021`. Falls back to the input
/// when it is not an ISO date.
fn fmt_date_mla(iso: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August",
        "September", "October", "November", "December",
    ];
    let parts: Vec<&str> = iso.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i32>(),
            parts[1].parse::<usize>(),
            parts[2].parse::<i32>(),
        ) {
            if (1..=12).contains(&m) {
                return format!("{} {} {}", d, MONTHS[m - 1], y);
            }
        }
        // fall through to raw on out-of-range month
    }
    iso.to_string()
}

// ───────────────────────────────────────────────────────────────────────────
// Style helpers
// ───────────────────────────────────────────────────────────────────────────

/// True for numbered styles (IEEE / ACM): in-text `[n]`, reference list in
/// citation order with `[n]` prefixes.
pub fn is_numbered(style: Format) -> bool {
    matches!(style, Format::Ieee | Format::Acm)
}

/// The bibliography section heading for a style.
pub fn bibliography_heading(style: Format) -> &'static str {
    match style {
        Format::Mla => "Works Cited",
        Format::Chicago => "Bibliography",
        // APA / IEEE / ACM / generic article
        _ => "References",
    }
}

/// The active style: the document's `format:` if set, else APA author-date.
pub fn active_style(format: Option<Format>) -> Format {
    format.unwrap_or(Format::Apa)
}

// ── in-text author label ────────────────────────────────────────────────────

/// In-text name label for author-date / author-page styles.
/// 1 → `Smith`, 2 → `Smith & Jones` (APA) / `Smith and Jones` (MLA, Chicago),
/// 3+ → `Smith et al.`.
fn intext_names(r: &Reference, style: Format) -> String {
    let fams: Vec<&str> = r.authors.iter().map(|a| a.family.as_str()).collect();
    let amp = matches!(style, Format::Apa);
    match fams.len() {
        0 => r.title.clone().unwrap_or_else(|| r.key.clone()),
        1 => fams[0].to_string(),
        2 => {
            if amp {
                format!("{} & {}", fams[0], fams[1])
            } else {
                format!("{} and {}", fams[0], fams[1])
            }
        }
        _ => format!("{} et al.", fams[0]),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// In-text citation formatting
// ───────────────────────────────────────────────────────────────────────────

/// Format a complete inline citation group per `style`.
///
/// Pure. `numbers` maps reference keys → IEEE/ACM citation numbers and is only
/// consulted for numbered styles. Unknown keys degrade gracefully (the key text
/// for author styles, `[0]` for numbered styles).
///
/// Examples (APA): `[@smith2020]` → `(Smith, 2020)`,
/// `[@smith2020, p. 12]` → `(Smith, 2020, p. 12)`,
/// `[@a; @b]` → `(A, 2020; B, 2019)`. (IEEE): `[1], [2]`.
pub fn format_in_text(
    refs: &[Reference],
    cite: &CiteRef,
    style: Format,
    numbers: &BTreeMap<String, usize>,
) -> String {
    if is_numbered(style) {
        let parts: Vec<String> = cite
            .items
            .iter()
            .map(|it| {
                let n = numbers.get(&it.key).copied().unwrap_or(0);
                match &it.locator {
                    Some(loc) => format!("[{}, {}]", n, loc),
                    None => format!("[{}]", n),
                }
            })
            .collect();
        return parts.join(", ");
    }

    let find = |key: &str| refs.iter().find(|r| r.key == key);
    let parts: Vec<String> = cite
        .items
        .iter()
        .map(|it| {
            let r = find(&it.key);
            let names = r
                .map(|r| intext_names(r, style))
                .unwrap_or_else(|| it.key.clone());
            match style {
                Format::Mla => {
                    // (Author page)
                    match &it.locator {
                        Some(loc) => format!("{} {}", names, strip_page_prefix(loc)),
                        None => names,
                    }
                }
                Format::Chicago => {
                    // (Author Year, page)
                    let year = r
                        .and_then(|r| r.year.clone())
                        .unwrap_or_else(|| "n.d.".to_string());
                    match &it.locator {
                        Some(loc) => format!("{} {}, {}", names, year, strip_page_prefix(loc)),
                        None => format!("{} {}", names, year),
                    }
                }
                // APA + generic article: (Author, Year, locator)
                _ => {
                    let year = r
                        .and_then(|r| r.year.clone())
                        .unwrap_or_else(|| "n.d.".to_string());
                    match &it.locator {
                        Some(loc) => format!("{}, {}, {}", names, year, loc),
                        None => format!("{}, {}", names, year),
                    }
                }
            }
        })
        .collect();
    format!("({})", parts.join("; "))
}

// ───────────────────────────────────────────────────────────────────────────
// Reference-list entry formatting (per style)
// ───────────────────────────────────────────────────────────────────────────

// Italics use markdown `*...*`; HTML renderers convert to `<em>`, text renderers
// can keep or strip them. This keeps the pure functions renderer-agnostic.

fn join_apa(names: &[String]) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        2 => format!("{}, & {}", names[0], names[1]),
        _ => {
            let (last, rest) = names.split_last().unwrap();
            format!("{}, & {}", rest.join(", "), last)
        }
    }
}

fn join_and(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        2 => format!("{}, and {}", parts[0], parts[1]),
        _ => {
            let (last, rest) = parts.split_last().unwrap();
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

fn apa_authors(authors: &[Author]) -> String {
    let names: Vec<String> = authors
        .iter()
        .map(|a| {
            let ini = initials(&a.given);
            if ini.is_empty() {
                a.family.clone()
            } else {
                format!("{}, {}", a.family, ini)
            }
        })
        .collect();
    join_apa(&names)
}

fn mla_authors(authors: &[Author]) -> String {
    match authors.len() {
        0 => String::new(),
        1 => inverted(&authors[0]),
        2 => format!("{}, and {}", inverted(&authors[0]), natural(&authors[1])),
        _ => format!("{}, et al.", inverted(&authors[0])),
    }
}

fn chicago_authors(authors: &[Author]) -> String {
    match authors.len() {
        0 => String::new(),
        1 => inverted(&authors[0]),
        _ => {
            let mut parts = vec![inverted(&authors[0])];
            for a in &authors[1..] {
                parts.push(natural(a));
            }
            join_and(&parts)
        }
    }
}

fn ieee_authors(authors: &[Author]) -> String {
    let names: Vec<String> = authors
        .iter()
        .map(|a| {
            let ini = initials(&a.given);
            if ini.is_empty() {
                a.family.clone()
            } else {
                format!("{} {}", ini, a.family)
            }
        })
        .collect();
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => {
            let (last, rest) = names.split_last().unwrap();
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

fn apa_reference(r: &Reference) -> String {
    let authors = apa_authors(&r.authors);
    let year = r.year.clone().unwrap_or_else(|| "n.d.".to_string());
    let head = if authors.is_empty() {
        format!("({}).", year)
    } else {
        format!("{} ({}).", authors, year)
    };
    let title = r.title.clone().unwrap_or_default();
    match r.ref_type {
        RefType::Book => {
            let mut s = format!("{} *{}*.", head, title);
            if let Some(p) = &r.publisher {
                s.push_str(&format!(" {}.", p));
            }
            s
        }
        _ => {
            let mut s = head;
            if !title.is_empty() {
                s.push_str(&format!(" {}.", title));
            }
            if let Some(c) = &r.container {
                s.push_str(&format!(" *{}*", c));
                if let Some(v) = &r.volume {
                    s.push_str(&format!(", *{}*", v));
                    if let Some(i) = &r.issue {
                        s.push_str(&format!("({})", i));
                    }
                }
                if let Some(pg) = &r.pages {
                    s.push_str(&format!(", {}", pg));
                }
                s.push('.');
            }
            if let Some(d) = &r.doi {
                s.push_str(&format!(" https://doi.org/{}", d));
            } else if let Some(u) = &r.url {
                s.push_str(&format!(" {}", u));
            }
            s
        }
    }
}

fn mla_reference(r: &Reference) -> String {
    let authors = mla_authors(&r.authors);
    let mut s = String::new();
    if !authors.is_empty() {
        // Avoid a double period after "et al." (which already ends in '.').
        if authors.ends_with('.') {
            s.push_str(&format!("{} ", authors));
        } else {
            s.push_str(&format!("{}. ", authors));
        }
    }
    let title = r.title.clone().unwrap_or_default();
    match r.ref_type {
        RefType::Book => {
            s.push_str(&format!("*{}*.", title));
            if let Some(p) = &r.publisher {
                s.push_str(&format!(" {},", p));
            }
            if let Some(y) = &r.year {
                s.push_str(&format!(" {}.", y));
            }
        }
        _ => {
            if !title.is_empty() {
                s.push_str(&format!("\"{}.\" ", title));
            }
            if let Some(c) = &r.container {
                s.push_str(&format!("*{}*", c));
                let mut parts: Vec<String> = Vec::new();
                if let Some(v) = &r.volume {
                    parts.push(format!("vol. {}", v));
                }
                if let Some(i) = &r.issue {
                    parts.push(format!("no. {}", i));
                }
                if let Some(y) = &r.year {
                    parts.push(y.clone());
                }
                if let Some(pg) = &r.pages {
                    parts.push(format!("pp. {}", pg));
                }
                if matches!(r.ref_type, RefType::Web) {
                    if let Some(u) = &r.url {
                        parts.push(u.clone());
                    }
                }
                for p in parts {
                    s.push_str(&format!(", {}", p));
                }
                s.push('.');
            }
            if let Some(acc) = &r.accessed {
                s.push_str(&format!(" Accessed {}.", fmt_date_mla(acc)));
            }
        }
    }
    s
}

fn chicago_reference(r: &Reference) -> String {
    let authors = chicago_authors(&r.authors);
    let year = r.year.clone().unwrap_or_else(|| "n.d.".to_string());
    let head = if authors.is_empty() {
        format!("{}.", year)
    } else {
        format!("{}. {}.", authors, year)
    };
    let title = r.title.clone().unwrap_or_default();
    match r.ref_type {
        RefType::Book => {
            let mut s = format!("{} *{}*.", head, title);
            if let Some(p) = &r.publisher {
                s.push_str(&format!(" {}.", p));
            }
            s
        }
        _ => {
            let mut s = format!("{} \"{}.\"", head, title);
            if let Some(c) = &r.container {
                s.push_str(&format!(" *{}*", c));
                let has_vol = r.volume.is_some();
                if let Some(v) = &r.volume {
                    s.push_str(&format!(" {}", v));
                }
                if let Some(i) = &r.issue {
                    s.push_str(&format!(" ({})", i));
                }
                if let Some(pg) = &r.pages {
                    if has_vol {
                        s.push_str(&format!(": {}", pg));
                    } else {
                        s.push_str(&format!(", {}", pg));
                    }
                }
                s.push('.');
            }
            if let Some(u) = &r.url {
                s.push_str(&format!(" {}.", u));
            }
            s
        }
    }
}

fn ieee_reference(r: &Reference, number: usize) -> String {
    let prefix = format!("[{}] ", number);
    let authors = ieee_authors(&r.authors);
    let title = r.title.clone().unwrap_or_default();
    match r.ref_type {
        RefType::Book => {
            let mut s = format!("{}{}, *{}*.", prefix, authors, title);
            if let Some(p) = &r.publisher {
                s.push_str(&format!(" {},", p));
            }
            if let Some(y) = &r.year {
                s.push_str(&format!(" {}.", y));
            }
            s
        }
        RefType::Conference => {
            let mut s = format!("{}{}, \"{},\"", prefix, authors, title);
            if let Some(c) = &r.container {
                s.push_str(&format!(" in *{}*", c));
            }
            if let Some(y) = &r.year {
                s.push_str(&format!(", {}", y));
            }
            if let Some(pg) = &r.pages {
                s.push_str(&format!(", pp. {}", pg));
            }
            s.push('.');
            s
        }
        RefType::Web => {
            let mut s = format!("{}{}, \"{},\"", prefix, authors, title);
            if let Some(c) = &r.container {
                s.push_str(&format!(" *{}*", c));
            }
            if let Some(y) = &r.year {
                s.push_str(&format!(", {}", y));
            }
            s.push_str(". [Online]. Available: ");
            if let Some(u) = &r.url {
                s.push_str(u);
            } else if let Some(d) = &r.doi {
                s.push_str(&format!("https://doi.org/{}", d));
            }
            s
        }
        _ => {
            let mut s = format!("{}{}, \"{},\"", prefix, authors, title);
            if let Some(c) = &r.container {
                s.push_str(&format!(" *{}*", c));
            }
            if let Some(v) = &r.volume {
                s.push_str(&format!(", vol. {}", v));
            }
            if let Some(i) = &r.issue {
                s.push_str(&format!(", no. {}", i));
            }
            if let Some(pg) = &r.pages {
                s.push_str(&format!(", pp. {}", pg));
            }
            if let Some(y) = &r.year {
                s.push_str(&format!(", {}", y));
            }
            s.push('.');
            s
        }
    }
}

/// Format ONE reference-list entry for `style`. `number` is the IEEE/ACM entry
/// number (ignored by author styles). Italics use markdown `*...*`.
pub fn format_reference(r: &Reference, style: Format, number: Option<usize>) -> String {
    match style {
        Format::Mla => mla_reference(r),
        Format::Chicago => chicago_reference(r),
        Format::Ieee | Format::Acm => ieee_reference(r, number.unwrap_or(0)),
        // APA + generic article paper style
        _ => apa_reference(r),
    }
}

/// Sort key for author styles: (first-author family ↓case, year, title ↓case).
fn sort_key(r: &Reference) -> (String, String, String) {
    let fam = r
        .authors
        .first()
        .map(|a| a.family.to_lowercase())
        .unwrap_or_else(|| r.title.clone().unwrap_or_default().to_lowercase());
    (
        fam,
        r.year.clone().unwrap_or_default(),
        r.title.clone().unwrap_or_default().to_lowercase(),
    )
}

/// Build the full reference list as `(key, formatted_entry)` pairs in display
/// order. Numbered styles (IEEE/ACM) keep the *input order* (the caller passes
/// references in citation order) and number `1..`; author styles sort
/// alphabetically. Deterministic.
pub fn reference_list_keyed(refs: &[Reference], style: Format) -> Vec<(String, String)> {
    if is_numbered(style) {
        refs.iter()
            .enumerate()
            .map(|(i, r)| (r.key.clone(), format_reference(r, style, Some(i + 1))))
            .collect()
    } else {
        let mut sorted: Vec<&Reference> = refs.iter().collect();
        sorted.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
        sorted
            .iter()
            .map(|r| (r.key.clone(), format_reference(r, style, None)))
            .collect()
    }
}

/// Build the full reference list as formatted strings in display order.
pub fn reference_list(refs: &[Reference], style: Format) -> Vec<String> {
    reference_list_keyed(refs, style)
        .into_iter()
        .map(|(_, s)| s)
        .collect()
}

// ───────────────────────────────────────────────────────────────────────────
// Document context: collect references + citation order + numbering
// ───────────────────────────────────────────────────────────────────────────

/// Resolved citation context for a document: the active style, every defined
/// reference (definition order), and the IEEE/ACM number assigned to each key.
#[derive(Debug, Clone)]
pub struct CiteContext {
    /// Active citation style.
    pub style: Format,
    /// All references defined via `::cite`, in definition order.
    pub references: Vec<Reference>,
    /// Key → citation number (cited keys first in citation order, then any
    /// defined-but-uncited references in definition order). Used by IEEE/ACM.
    pub numbers: BTreeMap<String, usize>,
}

/// Child-block accessor for the container variants citations can nest inside.
fn children_of(b: &Block) -> Option<&[Block]> {
    match b {
        Block::Page { children, .. }
        | Block::Section { children, .. }
        | Block::Slide { children, .. }
        | Block::App { children, .. }
        | Block::AppShell { children, .. }
        | Block::Sidebar { children, .. }
        | Block::Panel { children, .. }
        | Block::TabContent { children, .. }
        | Block::Drawer { children, .. }
        | Block::Modal { children, .. } => Some(children),
        _ => None,
    }
}

/// Prose text of a block that may contain inline `[@key]` citations.
fn cite_text_of(b: &Block) -> Option<&str> {
    match b {
        Block::Markdown { content, .. }
        | Block::Callout { content, .. }
        | Block::Summary { content, .. }
        | Block::Quote { content, .. }
        | Block::Section { content, .. }
        | Block::Details { content, .. } => Some(content),
        _ => None,
    }
}

fn collect_refs_rec(blocks: &[Block], out: &mut Vec<Reference>) {
    for b in blocks {
        if let Block::Cite { reference, .. } = b {
            out.push(reference.clone());
        }
        if let Some(children) = children_of(b) {
            collect_refs_rec(children, out);
        }
    }
}

/// Collect every `::cite`-defined reference in document order (recursing into
/// container blocks).
pub fn collect_references(blocks: &[Block]) -> Vec<Reference> {
    let mut out = Vec::new();
    collect_refs_rec(blocks, &mut out);
    out
}

fn collect_keys_rec(blocks: &[Block], order: &mut Vec<String>, seen: &mut BTreeSet<String>) {
    for b in blocks {
        if let Some(text) = cite_text_of(b) {
            for (_, _, cr) in crate::inline::find_inline_cites(text) {
                for it in cr.items {
                    if seen.insert(it.key.clone()) {
                        order.push(it.key);
                    }
                }
            }
        }
        if let Some(children) = children_of(b) {
            collect_keys_rec(children, order, seen);
        }
    }
}

/// Collect cited reference keys in first-appearance (citation) order.
pub fn collect_cited_keys(blocks: &[Block]) -> Vec<String> {
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    collect_keys_rec(blocks, &mut order, &mut seen);
    order
}

/// Assign citation numbers: cited keys first (citation order), then any
/// defined-but-uncited references in definition order. Deterministic.
fn assign_numbers(refs: &[Reference], order: &[String]) -> BTreeMap<String, usize> {
    let mut nums = BTreeMap::new();
    let mut n = 1usize;
    for k in order {
        if refs.iter().any(|r| &r.key == k) && !nums.contains_key(k) {
            nums.insert(k.clone(), n);
            n += 1;
        }
    }
    for r in refs {
        if !nums.contains_key(&r.key) {
            nums.insert(r.key.clone(), n);
            n += 1;
        }
    }
    nums
}

/// Build a [`CiteContext`] from a document's blocks and its `format:`.
pub fn build_context(blocks: &[Block], format: Option<Format>) -> CiteContext {
    let references = collect_references(blocks);
    let order = collect_cited_keys(blocks);
    let numbers = assign_numbers(&references, &order);
    CiteContext {
        style: active_style(format),
        references,
        numbers,
    }
}

/// References ordered for the bibliography display: number order for numbered
/// styles, definition order otherwise (the keyed list re-sorts author styles).
pub fn ordered_references(ctx: &CiteContext) -> Vec<Reference> {
    if is_numbered(ctx.style) {
        let mut v = ctx.references.clone();
        v.sort_by_key(|r| ctx.numbers.get(&r.key).copied().unwrap_or(usize::MAX));
        v
    } else {
        ctx.references.clone()
    }
}

// ───────────────────────────────────────────────────────────────────────────
// Ambient context for renderers (thread-local, deterministic per render)
// ───────────────────────────────────────────────────────────────────────────

thread_local! {
    static ACTIVE: RefCell<Option<CiteContext>> = const { RefCell::new(None) };
}

/// RAII guard that clears the ambient [`CiteContext`] when dropped.
pub struct CiteScope {
    _private: (),
}

impl Drop for CiteScope {
    fn drop(&mut self) {
        ACTIVE.with(|c| *c.borrow_mut() = None);
    }
}

/// Install `ctx` as the ambient context for the current thread for the lifetime
/// of the returned guard. Renderers call this at the top of their entrypoint so
/// nested `render_block` calls can resolve inline cites + the bibliography
/// without threading the context through every signature.
pub fn install_context(ctx: CiteContext) -> CiteScope {
    ACTIVE.with(|c| *c.borrow_mut() = Some(ctx));
    CiteScope { _private: () }
}

/// Run `f` with a reference to the ambient context (if any).
pub fn with_active<R>(f: impl FnOnce(Option<&CiteContext>) -> R) -> R {
    ACTIVE.with(|c| f(c.borrow().as_ref()))
}

/// Replace inline `[@key]` citations in `text` with their formatted in-text
/// strings using the ambient context. No-op when there is no context, no
/// references, or no cites. Used by text-oriented renderers (markdown/native).
pub fn substitute_text_cites(text: &str) -> String {
    with_active(|ctx| match ctx {
        Some(ctx) if !ctx.references.is_empty() => {
            let cites = crate::inline::find_inline_cites(text);
            if cites.is_empty() {
                return text.to_string();
            }
            let mut out = String::with_capacity(text.len());
            let mut last = 0;
            for (s, e, cr) in cites {
                out.push_str(&text[last..s]);
                out.push_str(&format_in_text(&ctx.references, &cr, ctx.style, &ctx.numbers));
                last = e;
            }
            out.push_str(&text[last..]);
            out
        }
        _ => text.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> Vec<Reference> {
        vec![
            Reference {
                key: "smith2020".into(),
                ref_type: RefType::Article,
                authors: parse_authors("Smith, John"),
                title: Some("Deep Learning for Climate Models".into()),
                container: Some("Journal of Climate AI".into()),
                year: Some("2020".into()),
                volume: Some("12".into()),
                issue: Some("3".into()),
                pages: Some("45-67".into()),
                doi: Some("10.1000/jcai.2020.45".into()),
                ..Default::default()
            },
            Reference {
                key: "jones2019".into(),
                ref_type: RefType::Book,
                authors: parse_authors("Jones, Alice; Brown, Bob"),
                title: Some("Foundations of Data Science".into()),
                publisher: Some("MIT Press".into()),
                year: Some("2019".into()),
                ..Default::default()
            },
            Reference {
                key: "lee2021".into(),
                ref_type: RefType::Web,
                authors: parse_authors("Lee, Carol"),
                title: Some("Understanding Transformers".into()),
                container: Some("AI Weekly".into()),
                year: Some("2021".into()),
                url: Some("https://aiweekly.example/transformers".into()),
                accessed: Some("2021-05-01".into()),
                ..Default::default()
            },
            Reference {
                key: "garcia2022".into(),
                ref_type: RefType::Conference,
                authors: parse_authors("Garcia, David; Patel, Esha; Wong, Fang"),
                title: Some("Scalable Inference".into()),
                container: Some("Proceedings of NeurIPS".into()),
                year: Some("2022".into()),
                pages: Some("100-110".into()),
                ..Default::default()
            },
        ]
    }

    fn cite1(key: &str) -> CiteRef {
        CiteRef {
            items: vec![CiteItem {
                key: key.into(),
                locator: None,
            }],
        }
    }

    // ── L2: author parsing ──────────────────────────────────────────────────

    #[test]
    fn parse_author_inverted_and_natural() {
        assert_eq!(parse_author("Smith, John"), Author { family: "Smith".into(), given: Some("John".into()) });
        assert_eq!(parse_author("John Smith"), Author { family: "Smith".into(), given: Some("John".into()) });
        assert_eq!(parse_author("Plato"), Author { family: "Plato".into(), given: None });
    }

    #[test]
    fn parse_authors_semicolon() {
        let a = parse_authors("Jones, Alice; Brown, Bob");
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].family, "Jones");
        assert_eq!(a[1].family, "Brown");
    }

    #[test]
    fn initials_multiword() {
        assert_eq!(initials(&Some("John".into())), "J.");
        assert_eq!(initials(&Some("Mary Jane".into())), "M. J.");
        assert_eq!(initials(&None), "");
    }

    // ── L6 golden: APA ──────────────────────────────────────────────────────

    #[test]
    fn apa_in_text_and_list() {
        let refs = fixtures();
        let nums = BTreeMap::new();
        assert_eq!(format_in_text(&refs, &cite1("smith2020"), Format::Apa, &nums), "(Smith, 2020)");
        assert_eq!(
            format_in_text(&refs, &CiteRef { items: vec![CiteItem { key: "smith2020".into(), locator: Some("p. 12".into()) }] }, Format::Apa, &nums),
            "(Smith, 2020, p. 12)"
        );
        assert_eq!(format_in_text(&refs, &cite1("jones2019"), Format::Apa, &nums), "(Jones & Brown, 2019)");
        assert_eq!(format_in_text(&refs, &cite1("garcia2022"), Format::Apa, &nums), "(Garcia et al., 2022)");
        assert_eq!(
            format_in_text(&refs, &CiteRef { items: vec![CiteItem { key: "smith2020".into(), locator: None }, CiteItem { key: "jones2019".into(), locator: None }] }, Format::Apa, &nums),
            "(Smith, 2020; Jones & Brown, 2019)"
        );

        let list = reference_list(&refs, Format::Apa);
        // Alphabetical: Garcia, Jones, Lee, Smith
        assert_eq!(list[0], "Garcia, D., Patel, E., & Wong, F. (2022). Scalable Inference. *Proceedings of NeurIPS*, 100-110.");
        assert_eq!(list[1], "Jones, A., & Brown, B. (2019). *Foundations of Data Science*. MIT Press.");
        assert_eq!(list[2], "Lee, C. (2021). Understanding Transformers. *AI Weekly*. https://aiweekly.example/transformers");
        assert_eq!(list[3], "Smith, J. (2020). Deep Learning for Climate Models. *Journal of Climate AI*, *12*(3), 45-67. https://doi.org/10.1000/jcai.2020.45");
    }

    // ── L6 golden: MLA ──────────────────────────────────────────────────────

    #[test]
    fn mla_in_text_and_list() {
        let refs = fixtures();
        let nums = BTreeMap::new();
        assert_eq!(format_in_text(&refs, &cite1("smith2020"), Format::Mla, &nums), "(Smith)");
        assert_eq!(
            format_in_text(&refs, &CiteRef { items: vec![CiteItem { key: "smith2020".into(), locator: Some("p. 12".into()) }] }, Format::Mla, &nums),
            "(Smith 12)"
        );
        assert_eq!(format_in_text(&refs, &cite1("jones2019"), Format::Mla, &nums), "(Jones and Brown)");
        assert_eq!(format_in_text(&refs, &cite1("garcia2022"), Format::Mla, &nums), "(Garcia et al.)");

        let list = reference_list(&refs, Format::Mla);
        assert_eq!(list[0], "Garcia, David, et al. \"Scalable Inference.\" *Proceedings of NeurIPS*, 2022, pp. 100-110.");
        assert_eq!(list[1], "Jones, Alice, and Bob Brown. *Foundations of Data Science*. MIT Press, 2019.");
        assert_eq!(list[2], "Lee, Carol. \"Understanding Transformers.\" *AI Weekly*, 2021, https://aiweekly.example/transformers. Accessed 1 May 2021.");
        assert_eq!(list[3], "Smith, John. \"Deep Learning for Climate Models.\" *Journal of Climate AI*, vol. 12, no. 3, 2020, pp. 45-67.");
    }

    // ── L6 golden: Chicago author-date ──────────────────────────────────────

    #[test]
    fn chicago_in_text_and_list() {
        let refs = fixtures();
        let nums = BTreeMap::new();
        assert_eq!(format_in_text(&refs, &cite1("smith2020"), Format::Chicago, &nums), "(Smith 2020)");
        assert_eq!(
            format_in_text(&refs, &CiteRef { items: vec![CiteItem { key: "smith2020".into(), locator: Some("p. 12".into()) }] }, Format::Chicago, &nums),
            "(Smith 2020, 12)"
        );
        assert_eq!(format_in_text(&refs, &cite1("jones2019"), Format::Chicago, &nums), "(Jones and Brown 2019)");

        let list = reference_list(&refs, Format::Chicago);
        assert_eq!(list[0], "Garcia, David, Esha Patel, and Fang Wong. 2022. \"Scalable Inference.\" *Proceedings of NeurIPS*, 100-110.");
        assert_eq!(list[1], "Jones, Alice, and Bob Brown. 2019. *Foundations of Data Science*. MIT Press.");
        assert_eq!(list[2], "Lee, Carol. 2021. \"Understanding Transformers.\" *AI Weekly*. https://aiweekly.example/transformers.");
        assert_eq!(list[3], "Smith, John. 2020. \"Deep Learning for Climate Models.\" *Journal of Climate AI* 12 (3): 45-67.");
    }

    // ── L6 golden: IEEE (numbered, citation order) ──────────────────────────

    #[test]
    fn ieee_in_text_and_list() {
        let refs = fixtures();
        let mut nums = BTreeMap::new();
        nums.insert("smith2020".to_string(), 1);
        nums.insert("jones2019".to_string(), 2);
        nums.insert("garcia2022".to_string(), 3);
        nums.insert("lee2021".to_string(), 4);

        assert_eq!(format_in_text(&refs, &cite1("smith2020"), Format::Ieee, &nums), "[1]");
        assert_eq!(
            format_in_text(&refs, &CiteRef { items: vec![CiteItem { key: "smith2020".into(), locator: None }, CiteItem { key: "jones2019".into(), locator: None }] }, Format::Ieee, &nums),
            "[1], [2]"
        );
        assert_eq!(
            format_in_text(&refs, &CiteRef { items: vec![CiteItem { key: "smith2020".into(), locator: Some("p. 12".into()) }] }, Format::Ieee, &nums),
            "[1, p. 12]"
        );

        // Reference list in citation order: smith, jones, garcia, lee
        let ordered = vec![refs[0].clone(), refs[1].clone(), refs[3].clone(), refs[2].clone()];
        let list = reference_list(&ordered, Format::Ieee);
        assert_eq!(list[0], "[1] J. Smith, \"Deep Learning for Climate Models,\" *Journal of Climate AI*, vol. 12, no. 3, pp. 45-67, 2020.");
        assert_eq!(list[1], "[2] A. Jones and B. Brown, *Foundations of Data Science*. MIT Press, 2019.");
        assert_eq!(list[2], "[3] D. Garcia, E. Patel, and F. Wong, \"Scalable Inference,\" in *Proceedings of NeurIPS*, 2022, pp. 100-110.");
        assert_eq!(list[3], "[4] C. Lee, \"Understanding Transformers,\" *AI Weekly*, 2021. [Online]. Available: https://aiweekly.example/transformers");
    }

    // ── L6 determinism ──────────────────────────────────────────────────────

    #[test]
    fn reference_list_is_deterministic() {
        let refs = fixtures();
        for style in [Format::Apa, Format::Mla, Format::Chicago, Format::Ieee] {
            assert_eq!(reference_list(&refs, style), reference_list(&refs, style));
        }
    }

    #[test]
    fn bibliography_headings() {
        assert_eq!(bibliography_heading(Format::Mla), "Works Cited");
        assert_eq!(bibliography_heading(Format::Apa), "References");
        assert_eq!(bibliography_heading(Format::Chicago), "Bibliography");
        assert_eq!(bibliography_heading(Format::Ieee), "References");
    }

    #[test]
    fn active_style_defaults_to_apa() {
        assert_eq!(active_style(None), Format::Apa);
        assert_eq!(active_style(Some(Format::Ieee)), Format::Ieee);
    }
}

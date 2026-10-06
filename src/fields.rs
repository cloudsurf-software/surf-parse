//! Blocks as named fields — read any block as fields and lists, write them
//! back, and never hand block source to the caller.
//!
//! Why this exists: a model that writes SurfDoc source gets the fences, the
//! attribute quoting and the body grammar wrong some of the time. A model that
//! only ever says "set `headline` of `b-hero-1` to these words" cannot. So the
//! caller sees names and text; the grammar stays in the parser and the
//! serializer, which already agree with each other.
//!
//! How it works: every typed [`Block`] serializes to JSON by field name. The
//! field rows in `spec/blocks.toml` (the one registry) say which members a
//! kind exposes, under what name, and with what limit. A write mutates the
//! JSON, builds the block back, serializes that one block at its authored
//! fence, carries over every authored opener token the serializer did not
//! write (`id=` first of all), splices the result over the old span, and then
//! checks its own work by parsing the result. A write that does not parse back
//! to the intended block is refused, not returned.

use crate::edit::{self, BlockRef};
use crate::types::Block;
use serde::Deserialize;
use serde_json::{json, Map, Value};

/// One exposed field of a kind (or of a list item).
#[derive(Debug, Clone, Deserialize)]
pub struct FieldRow {
    pub name: String,
    /// The typed block's own member name, when it differs from `name`.
    #[serde(default)]
    pub member: Option<String>,
    pub kind: String,
    #[serde(default)]
    pub max: Option<usize>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Option<Vec<String>>,
    /// A field that only means something beside another (a link's label and
    /// its target): the two are set together or not at all.
    #[serde(default)]
    pub with: Option<String>,
}

impl FieldRow {
    fn member(&self) -> &str {
        self.member.as_deref().unwrap_or(&self.name)
    }
    fn json(&self) -> Value {
        let mut o = json!({"name": self.name, "kind": self.kind});
        if let Some(m) = self.max {
            o["max"] = json!(m);
        }
        if self.required {
            o["required"] = json!(true);
        }
        if let Some(c) = &self.choices {
            o["choices"] = json!(c);
        }
        o
    }
}

/// What one item of a list is made of: named fields, or the block's own
/// column headers (a table kind), known only once a block is read.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ItemRow {
    Columns(String),
    Fields(Vec<FieldRow>),
}

/// One exposed list of a kind.
#[derive(Debug, Clone, Deserialize)]
pub struct ListRow {
    pub name: String,
    #[serde(default)]
    pub member: Option<String>,
    #[serde(default)]
    pub min: usize,
    pub max: usize,
    /// `string` (each item is one string) · `pair` (a two-string array);
    /// absent = an object with named members.
    #[serde(default)]
    pub shape: Option<String>,
    /// Members a new item needs that the schema does not expose.
    #[serde(default)]
    pub blank: Option<toml::Value>,
    pub item: ItemRow,
}

impl ListRow {
    fn member(&self) -> &str {
        self.member.as_deref().unwrap_or(&self.name)
    }
    fn is_table(&self) -> bool {
        matches!(self.item, ItemRow::Columns(_))
    }
    /// The item's fields — for a table, resolved from the block's headers.
    fn item_fields(&self, block: &Value) -> Vec<FieldRow> {
        match &self.item {
            ItemRow::Fields(f) => f.clone(),
            ItemRow::Columns(_) => headers(block)
                .iter()
                .map(|h| FieldRow {
                    name: h.clone(),
                    member: None,
                    kind: if h.to_lowercase().contains("price") { "price" } else { "text" }.to_string(),
                    max: Some(if h.to_lowercase().contains("price") { 24 } else { 80 }),
                    required: false,
                    choices: None,
                    with: None,
                })
                .collect(),
        }
    }
    fn json(&self, block: Option<&Value>) -> Value {
        let item = match (&self.item, block) {
            (ItemRow::Columns(_), None) => json!("columns"),
            (_, b) => Value::Array(self.item_fields(b.unwrap_or(&Value::Null)).iter().map(FieldRow::json).collect()),
        };
        json!({"name": self.name, "min": self.min, "max": self.max, "item": item})
    }
}

/// The field schema of one block kind, read from the registry.
#[derive(Debug, Clone)]
pub struct KindSchema {
    pub kind: String,
    pub purpose: String,
    pub fields: Vec<FieldRow>,
    pub lists: Vec<ListRow>,
    /// Members that hold the block's raw authored body. They follow the
    /// source, not the typed fields, so the write check leaves them out.
    pub raw: Vec<String>,
    /// The admission gate's verdict, recorded in the registry.
    pub admitted: bool,
}

impl KindSchema {
    fn json(&self) -> Value {
        json!({"kind": self.kind, "purpose": self.purpose, "admitted": self.admitted,
               "fields": self.fields.iter().map(FieldRow::json).collect::<Vec<_>>(),
               "lists": self.lists.iter().map(|l| l.json(None)).collect::<Vec<_>>()})
    }
}

/// Every kind that carries field rows in `spec/blocks.toml`, in name order.
pub fn kinds() -> Vec<KindSchema> {
    let value: toml::Value = toml::from_str(crate::spec_registry::BLOCKS_TOML).expect("spec/blocks.toml parses");
    let Some(blocks) = value.get("blocks").and_then(|b| b.as_table()) else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|(_, t)| t.get("fields").is_some() || t.get("lists").is_some())
        .map(|(name, t)| {
            let rows = |k: &str| t.get(k).cloned().unwrap_or(toml::Value::Array(Vec::new()));
            KindSchema {
                kind: name.clone(),
                purpose: t.get("purpose").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                fields: rows("fields").try_into().expect("field rows are well formed"),
                lists: rows("lists").try_into().expect("list rows are well formed"),
                raw: rows("fields_raw").try_into().unwrap_or_default(),
                admitted: t.get("fields_admitted").and_then(|v| v.as_bool()).unwrap_or(false),
            }
        })
        .collect()
}

/// The field schema of one kind, `None` when the registry has no rows for it.
pub fn schema(kind: &str) -> Option<KindSchema> {
    kinds().into_iter().find(|k| k.kind == kind)
}

/// Why one op (or one field of it) was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub field: Option<String>,
    pub reason: String,
}

fn no(reason: impl Into<String>) -> Refusal {
    Refusal { field: None, reason: reason.into() }
}
fn no_field(field: &str, reason: impl Into<String>) -> Refusal {
    Refusal { field: Some(field.to_string()), reason: reason.into() }
}

fn headers(block: &Value) -> Vec<String> {
    block["headers"].as_array().map(|a| a.iter().filter_map(|h| h.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// The document's typed blocks as one JSON tree.
fn tree(source: &str) -> Value {
    serde_json::to_value(&crate::parse(source).doc.blocks).unwrap_or(Value::Null)
}

/// The typed block that starts at `start`, wherever it is nested.
fn locate(v: &Value, start: usize) -> Option<&Value> {
    match v {
        Value::Array(a) => a.iter().find_map(|x| locate(x, start)),
        Value::Object(o) => {
            if o.contains_key("kind") && o.get("span").and_then(|s| s["start_offset"].as_u64()) == Some(start as u64) {
                return Some(v);
            }
            o.values().find_map(|x| locate(x, start))
        }
        _ => None,
    }
}

fn strip(v: &Value, raw: &[String]) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(|x| strip(x, raw)).collect()),
        Value::Object(o) => Value::Object(
            o.iter().filter(|(k, _)| *k != "span" && !raw.contains(k)).map(|(k, x)| (k.clone(), strip(x, raw))).collect(),
        ),
        // An empty string and an absent value are one thing to a field reader.
        Value::String(s) if s.is_empty() => Value::Null,
        other => other.clone(),
    }
}

/// A slot marker (`«FILL: …»`, `«IMG: …»`) is a value like any other: it is
/// allowed wherever a link or an image is, so a template survives a write.
fn is_slot(s: &str) -> bool {
    s.trim_start().starts_with('«')
}

/// Check one value against its field row BEFORE anything is written.
fn check(f: &FieldRow, v: &Value, table_cell: bool) -> Result<(), String> {
    if v.is_null() {
        return if f.required { Err("required, cannot be cleared".into()) } else { Ok(()) };
    }
    if f.kind == "flag" {
        return if v.is_boolean() { Ok(()) } else { Err("a flag takes true or false".into()) };
    }
    let Some(s) = v.as_str() else { return Err("takes text".into()) };
    if let Some(max) = f.max {
        if s.chars().count() > max {
            return Err(format!("longer than {max} characters"));
        }
    }
    if f.kind != "richtext" && s.contains('\n') {
        return Err("a one-line field cannot hold a newline".into());
    }
    if s.lines().any(|l| l.trim_start().starts_with("::")) {
        return Err("a line that opens or closes a block (::)".into());
    }
    if table_cell && s.contains('|') {
        return Err("a table cell cannot hold a |".into());
    }
    if f.required && s.trim().is_empty() {
        return Err("required, cannot be empty".into());
    }
    if let Some(choices) = &f.choices {
        if !choices.iter().any(|c| c == s) {
            return Err(format!("not one of: {}", choices.join(", ")));
        }
    }
    let link = ["/", "#", "https://", "http://", "mailto:", "tel:"];
    if f.kind == "link" && !is_slot(s) && !link.iter().any(|p| s.starts_with(p)) {
        return Err("not a link (/route, #anchor, https://, http://, mailto: or tel:)".into());
    }
    Ok(())
}

/// Put a checked value into a member, clearing in the member's own type: a
/// plain string clears to empty, a flag to false, an optional to null.
fn put(o: &mut Value, f: &FieldRow, v: &Value) {
    let cur = &o[f.member()];
    let new = match (v, cur) {
        (Value::Null, Value::String(_)) => json!(""),
        (Value::Null, Value::Bool(_)) => json!(false),
        _ => v.clone(),
    };
    o[f.member()] = new;
}

/// Refuse half a pair: a field named `with` another is set only beside it.
fn pairs(o: &Value, rows: &[FieldRow], prefix: &str) -> Result<(), Refusal> {
    for f in rows {
        let Some(other) = f.with.as_ref().and_then(|w| rows.iter().find(|r| &r.name == w)) else { continue };
        if !shown(&o[f.member()]).is_null() && shown(&o[other.member()]).is_null() {
            return Err(no_field(&format!("{prefix}{}", f.name), format!("needs {} too", other.name)));
        }
    }
    Ok(())
}

fn shown(v: &Value) -> Value {
    match v {
        Value::String(s) if s.is_empty() => Value::Null,
        other => other.clone(),
    }
}

fn set_fields(block: &mut Value, rows: &[FieldRow], fields: &Value) -> Result<(), Refusal> {
    let Some(map) = fields.as_object() else { return Ok(()) };
    for (name, v) in map {
        let f = rows.iter().find(|f| &f.name == name).ok_or_else(|| no_field(name, "no such field on this kind"))?;
        check(f, v, false).map_err(|r| no_field(name, r))?;
        put(block, f, v);
    }
    pairs(block, rows, "")
}

/// One raw list item as named fields.
fn item_out(list: &ListRow, block: &Value, raw: &Value) -> Value {
    let fields = list.item_fields(block);
    let mut o = Map::new();
    for (i, f) in fields.iter().enumerate() {
        let v = match (list.is_table() || list.shape.as_deref() == Some("pair"), list.shape.as_deref()) {
            (true, _) => raw.get(i).cloned().unwrap_or(Value::Null),
            (_, Some("string")) => raw.clone(),
            _ => raw.get(f.member()).cloned().unwrap_or(Value::Null),
        };
        o.insert(f.name.clone(), shown(&v));
    }
    Value::Object(o)
}

/// A new object item with every exposed member blank in its own type ("" for
/// text, false for a flag, the first choice) and the hidden members the
/// registry names; `null_at` makes that one member null instead.
fn blank_item(list: &ListRow, rows: &[FieldRow], null_at: Option<&str>) -> Map<String, Value> {
    let mut o = Map::new();
    for f in rows {
        let blank = match (f.kind.as_str(), &f.choices) {
            ("flag", _) => json!(false),
            (_, Some(c)) => json!(c[0]),
            _ if null_at == Some(f.member()) => Value::Null,
            _ => json!(""),
        };
        o.insert(f.member().to_string(), blank);
    }
    let hidden = list.blank.as_ref().and_then(|b| serde_json::to_value(b).ok());
    for (k, v) in hidden.as_ref().and_then(|h| h.as_object()).into_iter().flatten() {
        o.insert(k.clone(), v.clone());
    }
    o
}

/// Write named fields into one raw list item (an empty one when `raw` is null).
fn item_in(list: &ListRow, block: &Value, raw: &mut Value, fields: &Value) -> Result<(), Refusal> {
    let rows = list.item_fields(block);
    let positional = list.is_table() || list.shape.as_deref() == Some("pair");
    if raw.is_null() {
        *raw = match (positional, list.shape.as_deref()) {
            (true, _) => Value::Array(vec![json!(""); rows.len()]),
            (_, Some("string")) => json!(""),
            _ => {
                let mut o = blank_item(list, &rows, None);
                // A member the typed item holds as an Option is blank as null, not
                // "": an empty string would be written as `{icon=}` or `[]()`.
                for f in &rows {
                    if matches!(f.kind.as_str(), "flag") || f.choices.is_some() {
                        continue;
                    }
                    let mut probe = block.clone();
                    probe[list.member()] = Value::Array(vec![Value::Object(blank_item(list, &rows, Some(f.member())))]);
                    if build(&mut probe, &[list.clone()]).is_ok() {
                        o.insert(f.member().to_string(), Value::Null);
                    }
                }
                Value::Object(o)
            }
        };
    }
    let Some(map) = fields.as_object() else { return Ok(()) };
    for (name, v) in map {
        let label = format!("{}.{}", list.name, name);
        let (i, f) = rows.iter().enumerate().find(|(_, f)| &f.name == name).ok_or_else(|| no_field(&label, "no such field on this list"))?;
        check(f, v, list.is_table()).map_err(|r| no_field(&label, r))?;
        let text = if v.is_null() { json!("") } else { v.clone() };
        match (positional, list.shape.as_deref()) {
            (true, _) => raw[i] = text,
            (_, Some("string")) => *raw = text,
            _ => put(raw, f, v),
        }
    }
    if positional || list.shape.is_some() {
        return Ok(());
    }
    pairs(raw, &rows, &format!("{}.", list.name))
}

/// Build the block back from JSON. A new object item may lack a member the
/// schema does not expose; serde names it, and it is filled with an empty
/// list (the only hidden non-optional shape in the admitted kinds).
fn build(intended: &mut Value, lists: &[ListRow]) -> Result<Block, String> {
    for _ in 0..8 {
        match serde_json::from_value::<Block>(intended.clone()) {
            Ok(b) => return Ok(b),
            Err(e) => {
                let msg = e.to_string();
                let Some(name) = msg.strip_prefix("missing field `").and_then(|r| r.split('`').next()) else { return Err(msg) };
                let mut filled = false;
                for l in lists {
                    for item in intended[l.member()].as_array_mut().into_iter().flatten() {
                        if let Some(o) = item.as_object_mut() {
                            if !o.contains_key(name) {
                                o.insert(name.to_string(), json!([]));
                                filled = true;
                            }
                        }
                    }
                }
                if !filled {
                    return Err(msg);
                }
            }
        }
    }
    Err("the block could not be built".into())
}

/// `(fence, name, attribute tokens, tail)` of a directive opener line.
fn opener_parts(line: &str) -> (String, String, Vec<String>, String) {
    let colons = line.chars().take_while(|c| *c == ':').count();
    let rest = &line[colons..];
    let name_len = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')).unwrap_or(rest.len());
    let after = &rest[name_len..];
    let (tokens, tail) = match (after.starts_with('['), after.rfind(']')) {
        (true, Some(close)) => (edit::split_attr_tokens(&after[1..close]), after[close + 1..].to_string()),
        _ => (Vec::new(), after.to_string()),
    };
    (line[..colons].to_string(), rest[..name_len].to_string(), tokens, tail)
}

fn token_key(t: &str) -> &str {
    t.split('=').next().unwrap_or(t)
}

/// The serializer writes the attributes it knows; everything else the author
/// put on the opener (`id=`, an `icon=`, a default it elides) is carried over
/// in authored order. A token is NOT carried when the intended block says its
/// member is unset — that is a field the caller just cleared.
fn carry(old: &str, new: &str, intended: &Value, minimal: bool) -> String {
    let (_, _, old_tokens, _) = opener_parts(old);
    let (fence, name, mut tokens, tail) = opener_parts(new);
    for t in old_tokens {
        let key = token_key(&t).to_string();
        if tokens.iter().any(|n| token_key(n) == key) {
            continue;
        }
        let cleared = matches!(intended.get(key.replace('-', "_")), Some(Value::Null) | Some(Value::Bool(false)));
        let emptied = intended.get(key.replace('-', "_")).and_then(|v| v.as_str()) == Some("");
        if key == "id" {
            tokens.insert(0, t);
        } else if minimal && intended.get(key.replace('-', "_")).is_some() {
            // Second pass: the serializer said this member some other way
            // (a body line, a heading), and the authored token would win over it.
        } else if !cleared && !emptied {
            tokens.push(t);
        }
    }
    if tokens.is_empty() {
        format!("{fence}{name}{tail}")
    } else {
        format!("{fence}{name}[{}]{tail}", tokens.join(" "))
    }
}

/// THE write path: put `intended` where block `b` is, and check the result.
fn write_block(source: &str, b: &BlockRef, intended: &mut Value, sch: &KindSchema) -> Result<String, String> {
    let block = build(intended, &sch.lists)?;
    // The typed block is the intention from here on: members the caller did
    // not name (a flag's default, a hidden option) now carry their values.
    *intended = serde_json::to_value(&block).map_err(|e| e.to_string())?;
    // Keep every authored opener token first; when that does not parse back
    // (the token shadows what the serializer wrote in the body), keep only
    // the tokens the typed block does not hold.
    write_pass(source, b, &block, intended, sch, false).or_else(|first| {
        write_pass(source, b, &block, intended, sch, true).map_err(|second| if second == first { first } else { format!("{first} · and without the authored attributes: {second}") })
    })
}

fn write_pass(source: &str, b: &BlockRef, block: &Block, intended: &Value, sch: &KindSchema, minimal: bool) -> Result<String, String> {
    let old = &source[b.start_offset..b.end_offset];
    let old_open = old.lines().next().unwrap_or("");
    let colons = old_open.chars().take_while(|c| *c == ':').count();
    let text = crate::builder::serialize_block_at(block, colons.saturating_sub(2));
    let text = text.trim_matches('\n');
    if !text.starts_with("::") {
        return Err("the serializer wrote no directive for this block".into());
    }
    let open_end = text.find('\n').unwrap_or(text.len());
    let mut out = carry(old_open, &text[..open_end], intended, minimal);
    out.push_str(&text[open_end..]);
    if old.ends_with('\n') {
        out.push('\n');
    }
    let result = edit::splice(source, b.start_offset, b.end_offset, &out);
    let after = tree(&result);
    let got = locate(&after, b.start_offset).ok_or("the written block did not parse back")?;
    let (got, want) = (strip(got, &sch.raw), strip(intended, &sch.raw));
    if got != want {
        // Name the members that moved, so a refusal says where the serializer
        // and the parser disagree instead of only that they do.
        let moved: Vec<String> = want
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, v)| got.get(k.as_str()) != Some(*v))
            .map(|(k, v)| format!("{k}: wrote {v}, read {}", got.get(k.as_str()).unwrap_or(&Value::Null)))
            .collect();
        let mut words = moved.join("; ");
        if words.chars().count() > 300 {
            words = words.chars().take(300).collect::<String>() + "…";
        }
        return Err(format!("the written block does not parse back to the intended fields ({words})"));
    }
    let id_kept = edit::list_blocks(&result).iter().any(|n| n.start_offset == b.start_offset && n.id == b.id);
    if !id_kept {
        return Err("the block's id was lost".into());
    }
    Ok(result)
}

/// One block in its read form: every schema field with its current text.
fn block_out(b: &BlockRef, v: &Value) -> Value {
    let Some(sch) = schema(&b.name).filter(|s| s.admitted) else {
        return json!({"id": b.id, "kind": b.name, "admitted": false});
    };
    let fields: Vec<Value> = sch.fields.iter().map(|f| { let mut o = f.json(); o["value"] = shown(&v[f.member()]); o }).collect();
    let lists: Vec<Value> = sch.lists.iter().map(|l| {
        let mut o = l.json(Some(v));
        o["items"] = Value::Array(v[l.member()].as_array().into_iter().flatten().map(|raw| item_out(l, v, raw)).collect());
        o
    }).collect();
    json!({"id": b.id, "kind": b.name, "admitted": true, "fields": fields, "lists": lists})
}

fn page_of<'a>(t: &'a Value, route: &str) -> Option<&'a Value> {
    t.as_array()?.iter().find(|p| p["kind"] == "Page" && p["route"] == route)
}

/// The site's identity and its pages.
pub fn pages(source: &str) -> Value {
    let t = tree(source);
    let mut site = json!({"name": null, "description": null, "accent": null, "theme": null});
    let mut list = Vec::new();
    for b in t.as_array().into_iter().flatten() {
        if b["kind"] == "Site" {
            for p in b["properties"].as_array().into_iter().flatten() {
                if let Some(k) = p["key"].as_str().filter(|k| site.get(*k).is_some()) {
                    site[k] = p["value"].clone();
                }
            }
        } else if b["kind"] == "Page" {
            let n = b["children"].as_array().map(|c| c.iter().filter(|x| x["kind"] != "Markdown").count()).unwrap_or(0);
            list.push(json!({"route": b["route"], "title": b["title"], "blocks": n}));
        }
    }
    json!({"site": site, "pages": list})
}

/// Each block of one page, in order, as fields. `route` `None` reads a
/// document that has no pages.
pub fn read(source: &str, route: Option<&str>) -> Result<Value, String> {
    let t = tree(source);
    let refs = edit::list_blocks(source);
    let (children, title) = match route {
        Some(r) => {
            let p = page_of(&t, r).ok_or_else(|| format!("no page at route {r}"))?;
            (p["children"].clone(), p["title"].clone())
        }
        None => (t.clone(), Value::Null),
    };
    let mut blocks = Vec::new();
    for c in children.as_array().into_iter().flatten() {
        if c["kind"] == "Markdown" {
            let text = c["content"].as_str().unwrap_or("").trim();
            if !text.is_empty() {
                blocks.push(json!({"id": null, "kind": "text", "text": text}));
            }
        } else if let Some(b) = refs.iter().find(|b| Some(b.start_offset as u64) == c["span"]["start_offset"].as_u64()) {
            blocks.push(block_out(b, c));
        }
    }
    Ok(json!({"route": route, "title": title, "blocks": blocks}))
}

fn s<'a>(op: &'a Value, key: &str) -> Option<&'a str> {
    op[key].as_str()
}
fn need<'a>(op: &'a Value, key: &str) -> Result<&'a str, Refusal> {
    s(op, key).ok_or_else(|| no(format!("the op needs `{key}`")))
}
fn idx(op: &Value, key: &str) -> Result<usize, Refusal> {
    op[key].as_u64().map(|n| n as usize).ok_or_else(|| no(format!("the op needs a number `{key}`")))
}
fn ed(e: edit::EditError) -> Refusal {
    no(e.to_string())
}

/// Find a block by id and hand back its schema and its typed JSON.
fn target(src: &str, op: &Value) -> Result<(BlockRef, KindSchema, Value), Refusal> {
    let b = edit::find(src, s(op, "route"), need(op, "block")?).map_err(ed)?;
    let sch = schema(&b.name).filter(|k| k.admitted).ok_or_else(|| no(format!("kind {} is not admitted for field writes", b.name)))?;
    let v = locate(&tree(src), b.start_offset).cloned().ok_or_else(|| no("the block did not parse"))?;
    Ok((b, sch, v))
}

fn list_of<'a>(sch: &'a KindSchema, op: &Value) -> Result<&'a ListRow, Refusal> {
    let name = need(op, "list")?;
    sch.lists.iter().find(|l| l.name == name).ok_or_else(|| no_field(name, "no such list on this kind"))
}

/// A new block of `kind` as source, filled from `fields` and `lists`.
fn fresh(kind: &str, op: &Value, carry_from: Option<(&KindSchema, &Value)>, id: &str) -> Result<String, Refusal> {
    let sch = schema(kind).filter(|k| k.admitted).ok_or_else(|| no(format!("kind {kind} is not admitted for field writes")))?;
    let stub = format!("::{kind}[id={id}]\n::\n");
    let mut v = tree(&stub).get(0).cloned().filter(|v| v["kind"] != "Unknown").ok_or_else(|| no(format!("kind {kind} has no typed block")))?;
    if let Some(cols) = op["columns"].as_array() {
        v["headers"] = Value::Array(cols.clone());
    } else if let Some(first) = op["lists"]["rows"].get(0).and_then(|r| r.as_object()).filter(|_| sch.lists.iter().any(ListRow::is_table)) {
        v["headers"] = Value::Array(first.keys().map(|k| json!(k)).collect());
    }
    if let Some((old_sch, old)) = carry_from {
        for f in &sch.fields {
            if let Some(o) = old_sch.fields.iter().find(|o| o.name == f.name) {
                let cur = shown(&old[o.member()]);
                if check(f, &cur, false).is_ok() && !cur.is_null() {
                    put(&mut v, f, &cur);
                }
            }
        }
    }
    set_fields(&mut v, &sch.fields, &op["fields"])?;
    for (name, items) in op["lists"].as_object().into_iter().flatten() {
        let l = sch.lists.iter().find(|l| &l.name == name).ok_or_else(|| no_field(name, "no such list on this kind"))?;
        let mut raws = Vec::new();
        for it in items.as_array().into_iter().flatten() {
            let mut raw = Value::Null;
            item_in(l, &v, &mut raw, it)?;
            raws.push(raw);
        }
        if raws.len() > l.max {
            return Err(no_field(name, format!("more than {} items", l.max)));
        }
        v[l.member()] = Value::Array(raws);
    }
    let b = edit::list_blocks(&stub).into_iter().next().ok_or_else(|| no("the stub did not parse"))?;
    write_block(&stub, &b, &mut v, &sch).map_err(no)
}

fn page_ref(src: &str, route: &str) -> Result<BlockRef, Refusal> {
    let start = page_of(&tree(src), route).and_then(|p| p["span"]["start_offset"].as_u64());
    edit::list_blocks(src).into_iter().find(|b| b.name == "page" && Some(b.start_offset as u64) == start)
        .ok_or_else(|| no(format!("no page at route {route}")))
}

fn one_line(v: &str, what: &str) -> Result<(), Refusal> {
    if v.contains('\n') || v.contains("::") { Err(no_field(what, "one line, and no block fence (::)")) } else { Ok(()) }
}

/// Apply one op to `src`; the touched block's id comes back when there is one.
fn apply_one(src: &str, op: &Value) -> Result<(String, Option<String>), Refusal> {
    let route = s(op, "route");
    match need(op, "op")? {
        name @ ("set_fields" | "set_item" | "add_item" | "remove_item" | "move_item") => {
            let (b, sch, mut v) = target(src, op)?;
            if name == "set_fields" {
                set_fields(&mut v, &sch.fields, &op["fields"])?;
            } else {
                let l = list_of(&sch, op)?;
                let snapshot = v.clone();
                let items = v[l.member()].as_array_mut().ok_or_else(|| no("the list did not parse"))?;
                let at = |key: &str, len: usize| idx(op, key).and_then(|i| if i < len { Ok(i) } else { Err(no(format!("`{key}` is past the end of the list ({len} items)"))) });
                match name {
                    "set_item" => { let i = at("index", items.len())?; item_in(l, &snapshot, &mut items[i], &op["fields"])? }
                    "add_item" => {
                        if items.len() >= l.max { return Err(no_field(&l.name, format!("already holds {} items, the most allowed", l.max))); }
                        let mut raw = Value::Null;
                        item_in(l, &snapshot, &mut raw, &op["item"])?;
                        let pos = op["at"].as_u64().map(|n| (n as usize).min(items.len())).unwrap_or(items.len());
                        items.insert(pos, raw);
                    }
                    "remove_item" => {
                        if items.len() <= l.min { return Err(no_field(&l.name, format!("cannot hold fewer than {} items", l.min))); }
                        let i = at("index", items.len())?; items.remove(i);
                    }
                    _ => { let from = at("from", items.len())?; let to = at("to", items.len())?; let it = items.remove(from); items.insert(to, it); }
                }
            }
            Ok((write_block(src, &b, &mut v, &sch).map_err(no)?, b.id))
        }
        "add_block" => {
            let r = route.ok_or_else(|| no("the op needs `route`"))?;
            let page = page_ref(src, r)?;
            let kind = need(op, "kind")?;
            let id = edit::next_free_id(kind, &edit::ids_on_page(&edit::list_blocks(src), Some(r)));
            let text = fresh(kind, op, None, &id)?;
            let out = match s(op, "after") {
                Some(after) => edit::insert_after(src, route, after, &text).map_err(ed)?,
                None => {
                    let open_end = src[page.start_offset..].find('\n').map(|n| page.start_offset + n + 1).unwrap_or(src.len());
                    edit::splice(src, open_end, open_end, &format!("{text}\n"))
                }
            };
            Ok((out, Some(id)))
        }
        "swap_block" => {
            let (b, sch, v) = target(src, op)?;
            let id = b.id.clone().unwrap_or_default();
            let text = fresh(need(op, "kind")?, op, Some((&sch, &v)), &id)?;
            Ok((edit::replace_block(src, route, &id, &text).map_err(ed)?, b.id))
        }
        "remove_block" => Ok((edit::remove_block(src, route, need(op, "block")?).map_err(ed)?, None)),
        "move_block" => {
            let id = need(op, "block")?;
            let out = match s(op, "before") {
                Some(before) => edit::move_block(src, route, id, before).map_err(ed)?,
                None => {
                    let b = edit::find(src, route, id).map_err(ed)?;
                    let r = b.route.clone().ok_or_else(|| no("the block is not on a page"))?;
                    let text = src[b.start_offset..b.end_offset].trim_end_matches('\n').to_string();
                    let cut = edit::remove_block(src, route, id).map_err(ed)?;
                    let page = page_ref(&cut, &r)?;
                    let body_end = cut[..page.end_offset].trim_end_matches('\n').rfind('\n').map(|n| n + 1).unwrap_or(page.end_offset);
                    edit::splice(&cut, body_end, body_end, &format!("\n{text}\n"))
                }
            };
            Ok((out, Some(id.to_string())))
        }
        "add_page" => {
            let r = need(op, "route")?;
            let title = need(op, "title")?;
            one_line(title, "title")?;
            if !r.starts_with('/') || r.contains(char::is_whitespace) || r.contains(']') || edit::routes(src).iter().any(|x| x == r) {
                return Err(no_field("route", "a new page needs a fresh /route with no spaces"));
            }
            let mut body = String::new();
            for (n, blk) in op["blocks"].as_array().into_iter().flatten().enumerate() {
                let kind = blk["kind"].as_str().ok_or_else(|| no("every block needs a `kind`"))?;
                if kind == "text" {
                    let rich = FieldRow { name: "body".into(), member: None, kind: "richtext".into(), max: Some(4000), required: false, choices: None, with: None };
                    check(&rich, &blk["fields"]["body"], false).map_err(|e| no_field("body", e))?;
                    if let Some(h) = blk["fields"]["heading"].as_str() {
                        one_line(h, "heading")?;
                        body.push_str(&format!("# {h}\n\n"));
                    }
                    body.push_str(&format!("{}\n\n", blk["fields"]["body"].as_str().unwrap_or("").trim()));
                } else {
                    body.push_str(&format!("{}\n", fresh(kind, blk, None, &format!("b-{kind}-{}", n + 1))?));
                }
            }
            let page = format!("::page[route={r} title=\"{}\"]\n\n{}::\n", title.replace('"', "'"), body);
            let at = match s(op, "after") {
                Some(after) => page_ref(src, after)?.end_offset,
                None => edit::list_blocks(src).iter().filter(|b| b.name == "page" && b.depth == 0).map(|b| b.end_offset).max().unwrap_or(src.len()),
            };
            let lead = if src[..at].ends_with('\n') { "\n" } else { "\n\n" };
            Ok((edit::splice(src, at, at, &format!("{lead}{page}")), None))
        }
        "move_page" => {
            let p = page_ref(src, need(op, "route")?)?;
            let text = src[p.start_offset..p.end_offset].trim_end_matches('\n').to_string();
            let cut = edit::splice(src, p.start_offset, p.end_offset, "");
            let at = match s(op, "before") {
                Some(before) => page_ref(&cut, before)?.start_offset,
                None => cut.trim_end_matches('\n').len() + 1,
            };
            let at = at.min(cut.len());
            let lead = if cut[..at].ends_with('\n') || at == 0 { "" } else { "\n\n" };
            Ok((edit::splice(&cut, at, at, &format!("{lead}{text}\n\n")), None))
        }
        "set_page" => {
            let p = page_ref(src, need(op, "route")?)?;
            let title = need(op, "title")?;
            one_line(title, "title")?;
            let end = src[p.start_offset..].find('\n').map(|n| p.start_offset + n).unwrap_or(src.len());
            let line = edit::opener_with_attr(&src[p.start_offset..end], "title", title);
            Ok((edit::splice(src, p.start_offset, end, &line), None))
        }
        "set_site" => {
            let (key, value) = (need(op, "key")?, need(op, "value")?);
            if !["name", "description", "accent", "theme"].contains(&key) {
                return Err(no_field(key, "not one of: name, description, accent, theme"));
            }
            one_line(value, key)?;
            Ok((edit::set_site_key(src, key, value).map_err(ed)?, None))
        }
        "rename_everywhere" => {
            let (find, replace) = (need(op, "find")?, need(op, "replace")?);
            one_line(find, "find")?;
            one_line(replace, "replace")?;
            if find.trim().is_empty() || !src.contains(find) {
                return Err(no_field("find", "not found in the document"));
            }
            Ok((src.replace(find, replace), None))
        }
        "replace_text" => {
            one_line(need(op, "replace")?, "replace")?;
            let done = edit::replace_text(src, route, s(op, "block"), need(op, "find")?, need(op, "replace")?).map_err(ed)?;
            Ok((done.source, s(op, "block").map(str::to_string)))
        }
        other => Err(no(format!("unknown op `{other}`"))),
    }
}

/// Every op or none: the first refusal returns the input unchanged.
pub fn apply(source: &str, ops: &[Value]) -> Value {
    let mut src = source.replace("\r\n", "\n");
    let mut touched = Vec::new();
    for (n, op) in ops.iter().enumerate() {
        match apply_one(&src, op) {
            Ok((next, id)) => {
                if let Some(id) = id {
                    let route = edit::list_blocks(&next).iter().find(|b| b.id.as_deref() == Some(&id) && (op["route"].is_null() || b.route.as_deref() == op["route"].as_str())).and_then(|b| b.route.clone());
                    touched.push(json!({"route": route, "id": id}));
                }
                src = next;
            }
            Err(r) => {
                return json!({"ok": false, "source": source, "touched": [],
                              "refused": [{"op": n, "field": r.field, "reason": r.reason}]});
            }
        }
    }
    json!({"ok": true, "source": src, "touched": touched, "refused": []})
}

fn sentinel(f: &FieldRow, cur: &Value) -> Value {
    match f.kind.as_str() {
        "flag" => json!(!cur.as_bool().unwrap_or(false)),
        "link" => json!("/zq-sentinel"),
        "image" => json!("/img/zq-sentinel.jpg"),
        "price" => json!("$12.34"),
        "choice" => json!(f.choices.iter().flatten().find(|c| Some(c.as_str()) != cur.as_str()).cloned().unwrap_or_default()),
        _ => json!("Zq sentinel"),
    }
}

/// Admission by round trip (ruling D-AH-4): per kind, over every instance in
/// `sources`, the no-change write must parse back to the same typed block and
/// render the same HTML, and every field must survive set-then-read-back.
pub fn admit(sources: &[(String, String)]) -> Value {
    let mut rows = Vec::new();
    let (mut admitted, mut refused) = (Vec::new(), Vec::new());
    for sch in kinds() {
        let (mut instances, mut round, mut render) = (0, 0, 0);
        let mut why: Vec<String> = Vec::new();
        let mut field_ok: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        for (name, raw) in sources {
            let src = raw.replace("\r\n", "\n");
            let t = tree(&src);
            let html = crate::parse(&src).doc.to_html_fragment();
            for b in edit::list_blocks(&src).iter().filter(|b| b.name == sch.kind) {
                let Some(v) = locate(&t, b.start_offset) else { continue };
                instances += 1;
                let at = format!("{name}:{}", b.start_line);
                match write_block(&src, b, &mut v.clone(), &sch) {
                    Ok(out) => {
                        round += 1;
                        if crate::parse(&out).doc.to_html_fragment() == html { render += 1 } else { why.push(format!("{at}: renders differently after a no-change write")) }
                    }
                    Err(e) => { why.push(format!("{at}: {e}")); continue }
                }
                let mut probe = |label: String, mutated: Value| {
                    let ok = write_block(&src, b, &mut mutated.clone(), &sch);
                    if let Err(e) = &ok { why.push(format!("{at}: field {label}: {e}")) }
                    let slot = field_ok.entry(label).or_insert(true);
                    *slot = *slot && ok.is_ok();
                };
                for f in &sch.fields {
                    let mut m = v.clone();
                    m[f.member()] = sentinel(f, &v[f.member()]);
                    if let Some(o) = f.with.as_ref().and_then(|w| sch.fields.iter().find(|r| &r.name == w)).filter(|o| shown(&v[o.member()]).is_null()) {
                        m[o.member()] = sentinel(o, &Value::Null);
                    }
                    probe(f.name.clone(), m);
                }
                for l in &sch.lists {
                    let Some(first) = v[l.member()].get(0) else { continue };
                    let rows = l.item_fields(v);
                    for f in &rows {
                        let mut m = v.clone();
                        let shown_now = item_out(l, v, first);
                        let mut set = json!({ f.name.clone(): sentinel(f, &shown_now[&f.name]) });
                        if let Some(o) = f.with.as_ref().and_then(|w| rows.iter().find(|r| &r.name == w)).filter(|o| shown_now[&o.name].is_null()) {
                            set[&o.name] = sentinel(o, &Value::Null);
                        }
                        let mut raw = first.clone();
                        if item_in(l, v, &mut raw, &set).is_ok() {
                            m[l.member()][0] = raw;
                            probe(if l.is_table() { format!("{}.*", l.name) } else { format!("{}.{}", l.name, f.name) }, m);
                        }
                    }
                }
            }
        }
        if instances == 0 {
            why.push("no instance in the corpus".into());
        }
        let ok = instances > 0 && round == instances && render == instances && field_ok.values().all(|v| *v);
        why.truncate(6);
        if ok { admitted.push(json!(sch.kind)) } else { refused.push(json!({"kind": sch.kind, "why": why.join(" · ")})) }
        rows.push(json!({"kind": sch.kind, "instances": instances, "round_trip": round, "render_same": render,
            "fields": field_ok.iter().map(|(n, v)| json!({"name": n, "set_read_back": v})).collect::<Vec<_>>(),
            "admitted": ok, "refused": why}));
    }
    json!({"kinds": rows, "admitted": admitted, "refused": refused})
}

/// One request of the `surf-fields` contract, as JSON in and JSON out.
pub fn handle(req: &Value) -> Result<Value, String> {
    let source = || req["source"].as_str().ok_or_else(|| "the request needs `source`".to_string());
    match req["cmd"].as_str() {
        Some("kinds") => Ok(json!({"crate_version": crate::spec_registry::CRATE_VERSION,
                                   "kinds": kinds().iter().map(KindSchema::json).collect::<Vec<_>>()})),
        Some("stamp") => Ok(json!({"source": edit::stamp_ids(source()?)})),
        Some("pages") => Ok(pages(source()?)),
        Some("read") => read(source()?, req["route"].as_str()),
        Some("apply") => Ok(apply(source()?, req["ops"].as_array().ok_or("the request needs `ops`")?)),
        Some("admit") => {
            let list = req["sources"].as_array().ok_or("the request needs `sources`")?;
            let sources: Vec<(String, String)> = list.iter().map(|x| (x["name"].as_str().unwrap_or("").to_string(), x["source"].as_str().unwrap_or("").to_string())).collect();
            Ok(admit(&sources))
        }
        Some(other) => Err(format!("unknown cmd `{other}`")),
        None => Err("the request needs `cmd`".into()),
    }
}

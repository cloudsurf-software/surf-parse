//! Native diagram DSL (`::diagram`) — parsing and SVG rendering.
//!
//! Two diagram kinds are supported:
//! - `architecture` — boxes-and-arrows system diagrams (nodes + edges)
//! - `erd` — entity-relationship diagrams (entities + relations)
//!
//! The DSL is line-oriented: blank lines are ignored and every other line is
//! exactly one statement. Malformed input returns a [`DiagramError`], which
//! renderers use to trigger the preformatted prose fallback — a diagram must
//! NEVER fail a render.
//!
//! DETERMINISM: [`render_svg`] is a pure function of the model. Layout uses
//! only declaration-ordered `Vec`s (no `HashMap` iteration), no randomness and
//! no time, all geometry is integer arithmetic, so output is byte-stable
//! across runs — consumers pin exact substrings in tests.

use crate::render_html::escape_html;

// ------------------------------------------------------------------
// Model types
// ------------------------------------------------------------------

/// A DSL parse failure. Carries the offending line for future diagnostics;
/// in v1 it is used only to trigger the prose fallback at render time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagramError {
    /// 1-based line number of the offending statement (0 = whole block,
    /// e.g. an unknown diagram type).
    pub(crate) line: usize,
    /// Human-readable description of what went wrong.
    pub(crate) message: String,
}

/// A node in an architecture diagram (`id: Label text`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchNode {
    pub(crate) id: String,
    pub(crate) label: String,
}

/// An edge in an architecture diagram (`a -> b`, `a <-> b`, `a -> b: Label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: Option<String>,
    pub(crate) bidirectional: bool,
}

/// One field row of an ERD entity, with its `pk`/`fk`/`unique` modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErdField {
    pub(crate) name: String,
    pub(crate) pk: bool,
    pub(crate) fk: bool,
    pub(crate) unique: bool,
}

/// An entity in an ERD (`name: field1, field2 pk, ...`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErdEntity {
    pub(crate) name: String,
    pub(crate) fields: Vec<ErdField>,
}

/// One side of an ERD relation: `1` (one) or `*` (many).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cardinality {
    One,
    Many,
}

impl Cardinality {
    /// Display glyph drawn near the relation endpoint.
    fn glyph(self) -> &'static str {
        match self {
            Cardinality::One => "1",
            Cardinality::Many => "\u{2217}", // ∗
        }
    }
}

/// A relation in an ERD (`a 1--* b`, optionally `: label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErdRelation {
    pub(crate) from: String,
    pub(crate) from_card: Cardinality,
    pub(crate) to: String,
    pub(crate) to_card: Cardinality,
    pub(crate) label: Option<String>,
}

// ── flowchart ──────────────────────────────────────────────────────

/// Node shape for a flowchart node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowShape {
    /// Default rectangle (process step).
    Box,
    /// Diamond (decision / branch).
    Diamond,
    /// Rounded stadium (start/end terminator).
    Rounded,
}

/// A flowchart node (`id: Label` or `id [shape]: Label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowNode {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) shape: FlowShape,
}

/// A flowchart edge (`a -> b`, optional `: Label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: Option<String>,
}

// ── sequence ───────────────────────────────────────────────────────

/// A sequence-diagram actor/participant column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SeqActor {
    pub(crate) id: String,
    pub(crate) label: String,
}

/// A timeline event in a sequence diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SeqEvent {
    /// A message arrow. `dashed` = async/return (`-->`), else sync (`->`).
    Message {
        from: String,
        to: String,
        label: Option<String>,
        dashed: bool,
    },
    /// Start an activation bar on an actor's lifeline.
    Activate(String),
    /// End the most recent activation bar on an actor's lifeline.
    Deactivate(String),
}

// ── gantt ──────────────────────────────────────────────────────────

/// One bar of a Gantt chart (`Label: start, duration`), optionally in a
/// section. `start` is a numeric unit or a day-number (when dates are used).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GanttTask {
    pub(crate) section: Option<String>,
    pub(crate) label: String,
    pub(crate) start: i64,
    pub(crate) duration: i64,
}

// ── state ──────────────────────────────────────────────────────────

/// A state-machine state. `pseudo` marks the synthetic initial/final markers
/// derived from `[*]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateNode {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) initial: bool,
    pub(crate) final_: bool,
}

/// A state transition (`A -> B`, optional `: Label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateTransition {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: Option<String>,
}

// ── mindmap ────────────────────────────────────────────────────────

/// A mindmap node; `depth` is its indentation level (0 = root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MindNode {
    pub(crate) label: String,
    pub(crate) depth: usize,
}

/// Parsed diagram body, per diagram kind. Declaration order is preserved in
/// every `Vec` — layout and SVG output depend on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DiagramModel {
    Architecture {
        nodes: Vec<ArchNode>,
        edges: Vec<ArchEdge>,
    },
    Erd {
        entities: Vec<ErdEntity>,
        relations: Vec<ErdRelation>,
    },
    Flowchart {
        nodes: Vec<FlowNode>,
        edges: Vec<FlowEdge>,
    },
    Sequence {
        actors: Vec<SeqActor>,
        events: Vec<SeqEvent>,
    },
    Gantt {
        tasks: Vec<GanttTask>,
        /// True when `start` values are day-numbers from ISO dates (axis ticks
        /// render as dates); false for plain numeric units.
        dated: bool,
    },
    State {
        nodes: Vec<StateNode>,
        transitions: Vec<StateTransition>,
    },
    Mindmap {
        nodes: Vec<MindNode>,
    },
}

// ------------------------------------------------------------------
// DSL parsing
// ------------------------------------------------------------------

/// Parse a diagram body into a [`DiagramModel`].
///
/// `diagram_type` is the (already lowercased) `type` attribute value. Unknown
/// or empty types return `Err` so the renderer degrades to prose.
pub(crate) fn parse_diagram_source(
    diagram_type: &str,
    content: &str,
) -> Result<DiagramModel, DiagramError> {
    match diagram_type {
        "architecture" => parse_architecture(content),
        "erd" => parse_erd(content),
        "flowchart" => parse_flowchart(content),
        "sequence" => parse_sequence(content),
        "gantt" => parse_gantt(content),
        "state" => parse_state(content),
        "mindmap" => parse_mindmap(content),
        other => Err(DiagramError {
            line: 0,
            message: format!("unknown diagram type \"{other}\""),
        }),
    }
}

/// True for characters allowed in node/entity ids: `[A-Za-z0-9_-]`.
fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Split a leading id off `s`, returning `(id, rest)`.
///
/// Ids are greedy over `[A-Za-z0-9_-]`, with one backoff: a trailing `-`
/// followed by `>` belongs to a glued `->` arrow, not the id (`a-> b`).
fn split_leading_id(s: &str) -> (&str, &str) {
    let mut end = s.find(|c: char| !is_id_char(c)).unwrap_or(s.len());
    while end > 0 && s[..end].ends_with('-') && s[end..].starts_with('>') {
        end -= 1;
    }
    (&s[..end], &s[end..])
}

/// Shorthand for building a [`DiagramError`] at a 1-based line number.
fn err(line: usize, message: impl Into<String>) -> DiagramError {
    DiagramError {
        line,
        message: message.into(),
    }
}

/// Parse an optional `: label` suffix after an edge/relation target id.
///
/// Returns `Ok(None)` for an empty remainder, `Ok(Some(label))` for a `:`
/// suffix, and `Err` for any other trailing text.
fn parse_label_suffix(rest: &str, line_no: usize) -> Result<Option<String>, DiagramError> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok(None);
    }
    match rest.strip_prefix(':') {
        Some(label) => Ok(Some(label.trim().to_string())),
        None => Err(err(line_no, format!("expected `: label` or end of line, found \"{rest}\""))),
    }
}

/// Parse an `architecture` body: node lines (`id: Label`) and edge lines
/// (`a -> b`, `a <-> b`, optional `: Label`). Ids referenced in edges without
/// a node line are auto-declared with label = id.
fn parse_architecture(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<ArchNode> = Vec::new();
    let mut edges: Vec<ArchEdge> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let (id, rest) = split_leading_id(line);
        if id.is_empty() {
            return Err(err(line_no, format!("expected node or edge, found \"{line}\"")));
        }
        let rest = rest.trim_start();

        // Classifier: `:` right after the id makes it a node; an arrow makes
        // it an edge. Node labels are free text, so `a: Uses -> b` is a node.
        if let Some(label) = rest.strip_prefix(':') {
            let label = label.trim();
            nodes.push(ArchNode {
                id: id.to_string(),
                // Empty label falls back to the id so every box has text.
                label: if label.is_empty() { id.to_string() } else { label.to_string() },
            });
            continue;
        }

        let (bidirectional, after_arrow) = if let Some(after) = rest.strip_prefix("<->") {
            (true, after)
        } else if let Some(after) = rest.strip_prefix("->") {
            (false, after)
        } else {
            return Err(err(
                line_no,
                format!("expected `:`, `->` or `<->` after \"{id}\""),
            ));
        };

        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target id after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;

        edges.push(ArchEdge {
            from: id.to_string(),
            to: to.to_string(),
            label,
            bidirectional,
        });
    }

    // Auto-declare edge endpoints that never had a node line (label = id),
    // in first-reference order so layout stays deterministic.
    for edge in &edges {
        for id in [&edge.from, &edge.to] {
            if !nodes.iter().any(|n| &n.id == id) {
                nodes.push(ArchNode {
                    id: id.clone(),
                    label: id.clone(),
                });
            }
        }
    }

    Ok(DiagramModel::Architecture { nodes, edges })
}

/// Strip a leading cardinality token (`1--1`, `1--*`, `*--1`, `*--*`) off `s`.
fn strip_cardinality(s: &str) -> Option<(Cardinality, Cardinality, &str)> {
    let card = |c: char| match c {
        '1' => Some(Cardinality::One),
        '*' => Some(Cardinality::Many),
        _ => None,
    };
    let mut chars = s.chars();
    let from = card(chars.next()?)?;
    let rest = chars.as_str().strip_prefix("--")?;
    let mut chars = rest.chars();
    let to = card(chars.next()?)?;
    Some((from, to, chars.as_str()))
}

/// Parse an `erd` body: entity lines (`name: field1, field2 pk, ...`) and
/// relation lines (`a 1--* b`, optional `: label`). Entities referenced in
/// relations but never declared are auto-declared with no fields.
fn parse_erd(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut entities: Vec<ErdEntity> = Vec::new();
    let mut relations: Vec<ErdRelation> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let (name, rest) = split_leading_id(line);
        if name.is_empty() {
            return Err(err(line_no, format!("expected entity or relation, found \"{line}\"")));
        }
        let rest = rest.trim_start();

        // Classifier: `:` right after the name makes it an entity; a
        // `<1|*>--<1|*>` cardinality token makes it a relation.
        if let Some(field_list) = rest.strip_prefix(':') {
            entities.push(ErdEntity {
                name: name.to_string(),
                fields: parse_erd_fields(field_list, line_no)?,
            });
            continue;
        }

        let Some((from_card, to_card, after_card)) = strip_cardinality(rest) else {
            return Err(err(
                line_no,
                format!("expected `:` or cardinality (`1--*` etc.) after \"{name}\""),
            ));
        };

        let (to, after_to) = split_leading_id(after_card.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target entity after cardinality"));
        }
        let label = parse_label_suffix(after_to, line_no)?;

        relations.push(ErdRelation {
            from: name.to_string(),
            from_card,
            to: to.to_string(),
            to_card,
            label,
        });
    }

    // Auto-declare relation endpoints that never had an entity line, in
    // first-reference order so layout stays deterministic.
    for rel in &relations {
        for name in [&rel.from, &rel.to] {
            if !entities.iter().any(|e| &e.name == name) {
                entities.push(ErdEntity {
                    name: name.clone(),
                    fields: Vec::new(),
                });
            }
        }
    }

    Ok(DiagramModel::Erd { entities, relations })
}

/// Parse the comma-separated field list of an entity line. Each field is a
/// name followed by optional whitespace-separated modifiers from
/// {`pk`, `fk`, `unique`}.
fn parse_erd_fields(list: &str, line_no: usize) -> Result<Vec<ErdField>, DiagramError> {
    let mut fields = Vec::new();

    for segment in list.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue; // tolerate trailing/duplicate commas
        }
        let mut tokens = segment.split_whitespace();
        let name = tokens.next().expect("non-empty segment has a token");
        let mut field = ErdField {
            name: name.to_string(),
            pk: false,
            fk: false,
            unique: false,
        };
        for modifier in tokens {
            match modifier {
                "pk" => field.pk = true,
                "fk" => field.fk = true,
                "unique" => field.unique = true,
                other => {
                    return Err(err(
                        line_no,
                        format!("unknown field modifier \"{other}\" (expected pk, fk or unique)"),
                    ));
                }
            }
        }
        fields.push(field);
    }

    Ok(fields)
}

// ------------------------------------------------------------------
// DSL parsing — flowchart
// ------------------------------------------------------------------

/// Parse a `flowchart` body.
///
/// DSL (line-oriented, top-down layout):
/// - node:  `id: Label`            — default box (process)
/// - node:  `id [diamond]: Label`  — decision; shapes: `box`, `diamond`,
///   `decision` (=diamond), `rounded`, `terminator` (=rounded), `round`
/// - edge:  `a -> b`  or  `a -> b: Label`
///
/// Edge endpoints with no node line are auto-declared (label = id, box shape)
/// in first-reference order, so layout stays deterministic.
///
/// Example:
/// ```text
/// start [terminator]: Start
/// check [diamond]: Valid?
/// save: Persist
/// start -> check
/// check -> save: yes
/// check -> start: no
/// ```
fn parse_flowchart(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<FlowNode> = Vec::new();
    let mut edges: Vec<FlowEdge> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let (id, rest) = split_leading_id(line);
        if id.is_empty() {
            return Err(err(line_no, format!("expected node or edge, found \"{line}\"")));
        }
        let rest = rest.trim_start();

        // Optional `[shape]` between id and `:` makes this a node declaration.
        if let Some(after_bracket) = rest.strip_prefix('[') {
            let Some(close) = after_bracket.find(']') else {
                return Err(err(line_no, "expected `]` to close shape token"));
            };
            let shape_tok = after_bracket[..close].trim();
            let shape = parse_flow_shape(shape_tok, line_no)?;
            let after = after_bracket[close + 1..].trim_start();
            let Some(label) = after.strip_prefix(':') else {
                return Err(err(line_no, "expected `: Label` after `[shape]`"));
            };
            let label = label.trim();
            push_flow_node(
                &mut nodes,
                id,
                if label.is_empty() { id } else { label },
                shape,
            );
            continue;
        }

        // `:` right after the id makes it a plain box node.
        if let Some(label) = rest.strip_prefix(':') {
            let label = label.trim();
            push_flow_node(
                &mut nodes,
                id,
                if label.is_empty() { id } else { label },
                FlowShape::Box,
            );
            continue;
        }

        // Otherwise it must be an edge.
        let Some(after_arrow) = rest.strip_prefix("->") else {
            return Err(err(line_no, format!("expected `:`, `[shape]` or `->` after \"{id}\"")));
        };
        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target id after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;
        edges.push(FlowEdge {
            from: id.to_string(),
            to: to.to_string(),
            label,
        });
    }

    for edge in &edges {
        for id in [&edge.from, &edge.to] {
            if !nodes.iter().any(|n| &n.id == id) {
                nodes.push(FlowNode {
                    id: id.clone(),
                    label: id.clone(),
                    shape: FlowShape::Box,
                });
            }
        }
    }

    Ok(DiagramModel::Flowchart { nodes, edges })
}

/// Map a shape keyword to a [`FlowShape`].
fn parse_flow_shape(tok: &str, line_no: usize) -> Result<FlowShape, DiagramError> {
    match tok {
        "box" | "" | "process" | "rect" => Ok(FlowShape::Box),
        "diamond" | "decision" => Ok(FlowShape::Diamond),
        "rounded" | "round" | "terminator" | "stadium" => Ok(FlowShape::Rounded),
        other => Err(err(
            line_no,
            format!("unknown node shape \"{other}\" (expected box, diamond, rounded)"),
        )),
    }
}

/// Insert a flowchart node, or update the shape/label of an existing
/// auto-declared one (re-declaration wins, keeping first-reference order).
fn push_flow_node(nodes: &mut Vec<FlowNode>, id: &str, label: &str, shape: FlowShape) {
    if let Some(existing) = nodes.iter_mut().find(|n| n.id == id) {
        existing.label = label.to_string();
        existing.shape = shape;
    } else {
        nodes.push(FlowNode {
            id: id.to_string(),
            label: label.to_string(),
            shape,
        });
    }
}

// ------------------------------------------------------------------
// DSL parsing — sequence
// ------------------------------------------------------------------

/// Parse a `sequence` body.
///
/// DSL (line-oriented, top-down timeline):
/// - actor:   `actor id: Display Name`  (alias: `participant`)
/// - message: `a -> b: text`   — solid (sync)
/// - message: `a --> b: text`  — dashed (async / return)
/// - lifebar: `activate id` / `deactivate id`
///
/// Actors used in messages without an `actor` line are auto-declared in
/// first-reference order.
///
/// Example:
/// ```text
/// actor user: User
/// actor api: API
/// user -> api: POST /login
/// activate api
/// api --> user: 200 OK
/// deactivate api
/// ```
fn parse_sequence(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut actors: Vec<SeqActor> = Vec::new();
    let mut events: Vec<SeqEvent> = Vec::new();

    let declare = |actors: &mut Vec<SeqActor>, id: &str| {
        if !actors.iter().any(|a| a.id == id) {
            actors.push(SeqActor {
                id: id.to_string(),
                label: id.to_string(),
            });
        }
    };

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        // Keyword lines: `actor`/`participant id: Label`, `activate id`,
        // `deactivate id`.
        if let Some(rest) = strip_keyword(line, "actor").or_else(|| strip_keyword(line, "participant")) {
            let (id, after) = split_leading_id(rest.trim_start());
            if id.is_empty() {
                return Err(err(line_no, "expected actor id"));
            }
            let label = match after.trim().strip_prefix(':') {
                Some(l) if !l.trim().is_empty() => l.trim().to_string(),
                _ => id.to_string(),
            };
            if let Some(a) = actors.iter_mut().find(|a| a.id == id) {
                a.label = label;
            } else {
                actors.push(SeqActor { id: id.to_string(), label });
            }
            continue;
        }
        if let Some(rest) = strip_keyword(line, "activate") {
            let (id, _) = split_leading_id(rest.trim());
            if id.is_empty() {
                return Err(err(line_no, "expected actor id after `activate`"));
            }
            declare(&mut actors, id);
            events.push(SeqEvent::Activate(id.to_string()));
            continue;
        }
        if let Some(rest) = strip_keyword(line, "deactivate") {
            let (id, _) = split_leading_id(rest.trim());
            if id.is_empty() {
                return Err(err(line_no, "expected actor id after `deactivate`"));
            }
            declare(&mut actors, id);
            events.push(SeqEvent::Deactivate(id.to_string()));
            continue;
        }

        // Message line: `a -> b: msg` or `a --> b: msg`.
        let (from, rest) = split_leading_id(line);
        if from.is_empty() {
            return Err(err(line_no, format!("expected actor or message, found \"{line}\"")));
        }
        let rest = rest.trim_start();
        let (dashed, after_arrow) = if let Some(a) = rest.strip_prefix("-->") {
            (true, a)
        } else if let Some(a) = rest.strip_prefix("->") {
            (false, a)
        } else {
            return Err(err(line_no, format!("expected `->` or `-->` after \"{from}\"")));
        };
        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target actor after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;
        declare(&mut actors, from);
        declare(&mut actors, to);
        events.push(SeqEvent::Message {
            from: from.to_string(),
            to: to.to_string(),
            label,
            dashed,
        });
    }

    Ok(DiagramModel::Sequence { actors, events })
}

/// Strip a leading whitespace-delimited keyword (e.g. `actor`) returning the
/// remainder, or `None` if `line` does not start with `keyword` + space.
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if rest.starts_with(char::is_whitespace) {
        Some(rest)
    } else {
        None
    }
}

// ------------------------------------------------------------------
// DSL parsing — gantt
// ------------------------------------------------------------------

/// Parse a `gantt` body.
///
/// DSL (line-oriented, left-to-right time axis):
/// - section: `section Name`           — groups following tasks (optional)
/// - task:    `Label: start, duration` — numeric units, or
/// - task:    `Label: 2026-01-05, 7`   — ISO start date + duration in days
///
/// All tasks must use the same convention (numeric or dated); mixing is a
/// parse error. `Label` may contain spaces (it ends at the last `:`).
///
/// Example:
/// ```text
/// section Planning
/// Research: 2026-01-01, 5
/// Design: 2026-01-06, 3
/// section Build
/// Implementation: 2026-01-09, 10
/// ```
fn parse_gantt(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut tasks: Vec<GanttTask> = Vec::new();
    let mut section: Option<String> = None;
    let mut dated: Option<bool> = None;

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = strip_keyword(line, "section") {
            let name = rest.trim();
            section = if name.is_empty() { None } else { Some(name.to_string()) };
            continue;
        }

        // `Label: start, duration` — split label at the LAST colon so labels
        // may contain colons; here we split at the first colon for the label
        // and parse the remainder as `start, duration`.
        let Some((label, spec)) = line.split_once(':') else {
            return Err(err(line_no, format!("expected `Label: start, duration`, found \"{line}\"")));
        };
        let label = label.trim();
        if label.is_empty() {
            return Err(err(line_no, "task label must not be empty"));
        }
        let Some((start_s, dur_s)) = spec.split_once(',') else {
            return Err(err(line_no, "expected `start, duration`"));
        };
        let (start, is_date) = parse_gantt_value(start_s.trim(), line_no)?;
        // Duration is always a numeric count of units/days.
        let duration: i64 = dur_s
            .trim()
            .parse()
            .map_err(|_| err(line_no, format!("invalid duration \"{}\"", dur_s.trim())))?;
        if duration < 0 {
            return Err(err(line_no, "duration must be non-negative"));
        }
        match dated {
            None => dated = Some(is_date),
            Some(d) if d != is_date => {
                return Err(err(line_no, "cannot mix numeric and date start values"));
            }
            _ => {}
        }
        tasks.push(GanttTask {
            section: section.clone(),
            label: label.to_string(),
            start,
            duration,
        });
    }

    Ok(DiagramModel::Gantt {
        tasks,
        dated: dated.unwrap_or(false),
    })
}

/// Parse a Gantt start value: either a plain integer (numeric units) or an
/// ISO `YYYY-MM-DD` date (converted to a day-number). Returns `(value,
/// is_date)`.
fn parse_gantt_value(s: &str, line_no: usize) -> Result<(i64, bool), DiagramError> {
    if let Ok(n) = s.parse::<i64>() {
        return Ok((n, false));
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<i64>(),
            parts[2].parse::<i64>(),
        ) {
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return Ok((days_from_civil(y, m, d), true));
            }
        }
    }
    Err(err(line_no, format!("invalid start value \"{s}\" (expected integer or YYYY-MM-DD)")))
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// algorithm). Deterministic integer arithmetic, no external date crate.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Inverse of [`days_from_civil`]: `(year, month, day)` for a day-number.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ------------------------------------------------------------------
// DSL parsing — state
// ------------------------------------------------------------------

/// Parse a `state` body.
///
/// DSL (line-oriented, top-down layout):
/// - state:      `id: Label`        — optional; defaults label = id
/// - transition: `a -> b`  or  `a -> b: event`
/// - `[*]` is the special initial (as a source) / final (as a target) marker.
///
/// States used in transitions without a state line are auto-declared.
///
/// Example:
/// ```text
/// [*] -> Idle
/// Idle -> Running: start
/// Running -> Idle: stop
/// Running -> [*]
/// ```
fn parse_state(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<StateNode> = Vec::new();
    let mut transitions: Vec<StateTransition> = Vec::new();

    // `[*]` maps to a synthetic node id; a `[*]` as a source means initial,
    // as a target means final. We use distinct ids so both can coexist.
    const INITIAL: &str = "__initial__";
    const FINAL: &str = "__final__";

    let ensure = |nodes: &mut Vec<StateNode>, id: &str| {
        if !nodes.iter().any(|n| n.id == id) {
            let (initial, final_) = (id == INITIAL, id == FINAL);
            nodes.push(StateNode {
                id: id.to_string(),
                label: if initial || final_ { String::new() } else { id.to_string() },
                initial,
                final_,
            });
        }
    };

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        // `[*]` token (source position).
        if let Some(rest) = line.strip_prefix("[*]") {
            let rest = rest.trim_start();
            let Some(after_arrow) = rest.strip_prefix("->") else {
                return Err(err(line_no, "expected `->` after `[*]`"));
            };
            let (to, after_to) = parse_state_target(after_arrow.trim_start());
            if to.is_empty() {
                return Err(err(line_no, "expected target state after `[*] ->`"));
            }
            let label = parse_label_suffix(after_to, line_no)?;
            let to = if to == "[*]" { FINAL } else { to };
            ensure(&mut nodes, INITIAL);
            ensure(&mut nodes, to);
            transitions.push(StateTransition {
                from: INITIAL.to_string(),
                to: to.to_string(),
                label,
            });
            continue;
        }

        let (id, rest) = split_leading_id(line);
        if id.is_empty() {
            return Err(err(line_no, format!("expected state or transition, found \"{line}\"")));
        }
        let rest = rest.trim_start();

        // `:` makes it a state declaration.
        if let Some(label) = rest.strip_prefix(':') {
            let label = label.trim();
            ensure(&mut nodes, id);
            if let Some(n) = nodes.iter_mut().find(|n| n.id == id) {
                n.label = if label.is_empty() { id.to_string() } else { label.to_string() };
            }
            continue;
        }

        let Some(after_arrow) = rest.strip_prefix("->") else {
            return Err(err(line_no, format!("expected `:` or `->` after \"{id}\"")));
        };
        let (to_raw, after_to) = parse_state_target(after_arrow.trim_start());
        if to_raw.is_empty() {
            return Err(err(line_no, "expected target state after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;
        let to = if to_raw == "[*]" { FINAL } else { to_raw };
        ensure(&mut nodes, id);
        ensure(&mut nodes, to);
        transitions.push(StateTransition {
            from: id.to_string(),
            to: to.to_string(),
            label,
        });
    }

    Ok(DiagramModel::State { nodes, transitions })
}

/// Split a transition target which may be `[*]` or a normal id, returning
/// `(target, rest)`. `[*]` is returned verbatim for the caller to remap.
fn parse_state_target(s: &str) -> (&str, &str) {
    if let Some(rest) = s.strip_prefix("[*]") {
        ("[*]", rest)
    } else {
        split_leading_id(s)
    }
}

// ------------------------------------------------------------------
// DSL parsing — mindmap
// ------------------------------------------------------------------

/// Parse a `mindmap` body.
///
/// DSL (indentation defines hierarchy; 2 spaces or 1 tab per level):
/// ```text
/// Root Topic
///   Branch A
///     Leaf A1
///     Leaf A2
///   Branch B
/// ```
/// The first non-blank line is the root (depth 0). Each node's label is the
/// trimmed line text; depth comes from leading whitespace.
fn parse_mindmap(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<MindNode> = Vec::new();

    for raw in content.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        // Count leading whitespace as indent columns (tab = 2).
        let mut indent = 0usize;
        for c in raw.chars() {
            match c {
                ' ' => indent += 1,
                '\t' => indent += 2,
                _ => break,
            }
        }
        let depth = indent / 2;
        nodes.push(MindNode {
            label: raw.trim().to_string(),
            depth,
        });
    }

    Ok(DiagramModel::Mindmap { nodes })
}

// ------------------------------------------------------------------
// SVG rendering — shared geometry
// ------------------------------------------------------------------

/// Approximate character width in px at the 13px UI font — no text measuring
/// is available, so boxes are sized by label length.
const CHAR_W: i64 = 8;
/// Outer canvas margin.
const MARGIN: i64 = 20;

/// A placed box: top-left corner + size, all integer px.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

impl Rect {
    fn cx(&self) -> i64 {
        self.x + self.w / 2
    }
    fn cy(&self) -> i64 {
        self.y + self.h / 2
    }
}

/// Open the root `<svg>` element with computed bounds plus an optional
/// `<title>` child (present when the block has a `title` attribute).
fn svg_open(w: i64, h: i64, title: Option<&str>) -> String {
    let title_el = match title {
        Some(t) => format!("<title>{}</title>", escape_html(t)),
        None => String::new(),
    };
    format!(
        "<svg class=\"surfdoc-diagram-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" font-family=\"system-ui, sans-serif\" font-size=\"13\">{title_el}"
    )
}

/// Render a [`DiagramModel`] as deterministic inline SVG.
///
/// `title` is the block's `title` attribute; when present it becomes the
/// SVG `<title>` child for accessibility.
pub(crate) fn render_svg(model: &DiagramModel, title: Option<&str>) -> String {
    match model {
        DiagramModel::Architecture { nodes, edges } => render_architecture_svg(nodes, edges, title),
        DiagramModel::Erd { entities, relations } => render_erd_svg(entities, relations, title),
        DiagramModel::Flowchart { nodes, edges } => render_flowchart_svg(nodes, edges, title),
        DiagramModel::Sequence { actors, events } => render_sequence_svg(actors, events, title),
        DiagramModel::Gantt { tasks, dated } => render_gantt_svg(tasks, *dated, title),
        DiagramModel::State { nodes, transitions } => render_state_svg(nodes, transitions, title),
        DiagramModel::Mindmap { nodes } => render_mindmap_svg(nodes, title),
    }
}

/// Reusable arrowhead marker `<defs>` (id `surfdoc-arrow`). Same geometry as
/// the architecture renderer so all diagram arrows look identical.
fn arrow_defs() -> &'static str {
    "<defs><marker id=\"surfdoc-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"#64748b\"/></marker></defs>"
}

/// Longest-path layering over an arbitrary node count + index edge list.
/// Mirrors [`arch_layers`]: cycle-closing edges (in declaration order) and
/// self-loops are dropped so the relaxation always terminates.
fn longest_path_layers(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(f, t) in edges {
        if f >= n || t >= n || f == t || reachable(&adj, t, f) {
            continue;
        }
        adj[f].push(t);
    }
    let mut layer = vec![0usize; n];
    for _ in 0..n {
        for (f, outs) in adj.iter().enumerate() {
            for &t in outs {
                if layer[t] < layer[f] + 1 {
                    layer[t] = layer[f] + 1;
                }
            }
        }
    }
    layer
}

/// A top-down layered placement: each node placed in a row by its layer,
/// centered left-to-right within the canvas in declaration order.
struct Placed {
    rects: Vec<Rect>,
    w: i64,
    h: i64,
}

/// Place `widths.len()` uniform-height nodes top-down by `longest_path_layers`.
fn layered_layout(widths: &[i64], node_h: i64, row_gap: i64, hgap: i64, edges: &[(usize, usize)]) -> Placed {
    let n = widths.len();
    let layer = longest_path_layers(n, edges);
    let n_rows = layer.iter().map(|l| l + 1).max().unwrap_or(1);
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); n_rows];
    for (i, &l) in layer.iter().enumerate() {
        rows[l].push(i);
    }
    let row_width = |row: &[usize]| -> i64 {
        if row.is_empty() {
            0
        } else {
            row.iter().map(|&i| widths[i]).sum::<i64>() + hgap * (row.len() as i64 - 1)
        }
    };
    let max_row_w = rows.iter().map(|r| row_width(r)).max().unwrap_or(0);
    let mut rects = vec![Rect { x: 0, y: 0, w: 0, h: 0 }; n];
    for (r, row) in rows.iter().enumerate() {
        let y = MARGIN + r as i64 * (node_h + row_gap);
        let mut x = MARGIN + (max_row_w - row_width(row)) / 2;
        for &i in row {
            rects[i] = Rect { x, y, w: widths[i], h: node_h };
            x += widths[i] + hgap;
        }
    }
    let total_w = (MARGIN * 2 + max_row_w).max(2 * MARGIN);
    let total_h = (MARGIN * 2 + n_rows as i64 * (node_h + row_gap) - row_gap).max(2 * MARGIN + node_h);
    Placed { rects, w: total_w, h: total_h }
}

/// Choose the two connection points between two boxes for a top-down layout:
/// bottom→top when descending, top→bottom when ascending, side→side same row.
fn vert_edge_points(a: &Rect, b: &Rect) -> (i64, i64, i64, i64) {
    if b.cy() > a.cy() {
        (a.cx(), a.y + a.h, b.cx(), b.y)
    } else if b.cy() < a.cy() {
        (a.cx(), a.y, b.cx(), b.y + b.h)
    } else if b.cx() >= a.cx() {
        (a.x + a.w, a.cy(), b.x, b.cy())
    } else {
        (a.x, a.cy(), b.x + b.w, b.cy())
    }
}

// ------------------------------------------------------------------
// SVG rendering — architecture
// ------------------------------------------------------------------

/// Node box height.
const NODE_H: i64 = 40;
/// Vertical gap between nodes in a column.
const NODE_VGAP: i64 = 24;
/// Horizontal gap between columns.
const COL_GAP: i64 = 60;

/// Is `to` reachable from `from` in the adjacency list? Iterative DFS so deep
/// chains never blow the stack.
fn reachable(adj: &[Vec<usize>], from: usize, to: usize) -> bool {
    let mut seen = vec![false; adj.len()];
    let mut stack = vec![from];
    while let Some(n) = stack.pop() {
        if n == to {
            return true;
        }
        if seen[n] {
            continue;
        }
        seen[n] = true;
        for &m in &adj[n] {
            if !seen[m] {
                stack.push(m);
            }
        }
    }
    false
}

/// Longest-path layering from roots. Cycles are broken by declaration order
/// (an edge that would close a cycle is ignored for layout — never panics or
/// loops), self-loops are skipped.
fn arch_layers(nodes: &[ArchNode], edges: &[ArchEdge]) -> Vec<usize> {
    let index_of = |id: &str| nodes.iter().position(|n| n.id == id);

    // Build a DAG in edge-declaration order, dropping cycle-closing edges.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for edge in edges {
        let (Some(f), Some(t)) = (index_of(&edge.from), index_of(&edge.to)) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        if f == t || reachable(&adj, t, f) {
            continue;
        }
        adj[f].push(t);
    }

    // Longest path from roots: n relaxation passes converge on a DAG.
    let mut layer = vec![0usize; nodes.len()];
    for _ in 0..nodes.len() {
        for (f, outs) in adj.iter().enumerate() {
            for &t in outs {
                if layer[t] < layer[f] + 1 {
                    layer[t] = layer[f] + 1;
                }
            }
        }
    }
    layer
}

/// Box width for a label: ~8px per char + padding, min 80.
fn label_width(label: &str) -> i64 {
    (label.chars().count() as i64 * CHAR_W + 24).max(80)
}

fn render_architecture_svg(nodes: &[ArchNode], edges: &[ArchEdge], title: Option<&str>) -> String {
    let layer = arch_layers(nodes, edges);
    let n_cols = layer.iter().map(|l| l + 1).max().unwrap_or(1);

    // Group node indices by layer (declaration order within a column).
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); n_cols];
    for (i, &l) in layer.iter().enumerate() {
        columns[l].push(i);
    }

    // Column widths and x offsets.
    let col_w: Vec<i64> = columns
        .iter()
        .map(|col| {
            col.iter()
                .map(|&i| label_width(&nodes[i].label))
                .max()
                .unwrap_or(80)
        })
        .collect();
    let mut col_x = Vec::with_capacity(n_cols);
    let mut x = MARGIN;
    for w in &col_w {
        col_x.push(x);
        x += w + COL_GAP;
    }

    // Place each node, centered within its column.
    let mut rects: Vec<Rect> = vec![
        Rect { x: 0, y: 0, w: 0, h: 0 };
        nodes.len()
    ];
    let mut max_rows = 0usize;
    for (c, col) in columns.iter().enumerate() {
        max_rows = max_rows.max(col.len());
        for (row, &i) in col.iter().enumerate() {
            let w = label_width(&nodes[i].label);
            rects[i] = Rect {
                x: col_x[c] + (col_w[c] - w) / 2,
                y: MARGIN + row as i64 * (NODE_H + NODE_VGAP),
                w,
                h: NODE_H,
            };
        }
    }

    let total_w = if n_cols > 0 { x - COL_GAP + MARGIN } else { 2 * MARGIN };
    let total_h = MARGIN * 2 + (max_rows.max(1) as i64) * (NODE_H + NODE_VGAP) - NODE_VGAP;

    let mut svg = svg_open(total_w, total_h, title);

    // Arrowhead marker. `orient="auto-start-reverse"` lets the same marker
    // serve both ends of a `<->` edge.
    svg.push_str(
        "<defs><marker id=\"surfdoc-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"#64748b\"/></marker></defs>",
    );

    // Edges first so node boxes paint over the line ends.
    for edge in edges {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == edge.from),
            nodes.iter().position(|n| n.id == edge.to),
        ) else {
            continue;
        };
        let (a, b) = (&rects[f], &rects[t]);

        // Connect facing sides: left/right across columns, top/bottom within one.
        let (x1, y1, x2, y2) = if b.cx() > a.cx() {
            (a.x + a.w, a.cy(), b.x, b.cy())
        } else if b.cx() < a.cx() {
            (a.x, a.cy(), b.x + b.w, b.cy())
        } else if b.cy() >= a.cy() {
            (a.cx(), a.y + a.h, b.cx(), b.y)
        } else {
            (a.cx(), a.y, b.cx(), b.y + b.h)
        };

        let marker_start = if edge.bidirectional {
            " marker-start=\"url(#surfdoc-arrow)\""
        } else {
            ""
        };
        svg.push_str(&format!(
            "<line class=\"surfdoc-diagram-edge\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#64748b\" stroke-width=\"1.5\" fill=\"none\" marker-end=\"url(#surfdoc-arrow)\"{marker_start}/>"
        ));

        if let Some(label) = &edge.label {
            svg.push_str(&format!(
                "<text class=\"surfdoc-diagram-edge-label\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
                (x1 + x2) / 2,
                (y1 + y2) / 2 - 5,
                escape_html(label),
            ));
        }
    }

    // Node boxes with centered labels.
    for (i, node) in nodes.iter().enumerate() {
        let r = &rects[i];
        svg.push_str(&format!(
            "<g class=\"surfdoc-diagram-node\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\" fill=\"#f8fafc\" stroke=\"#64748b\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"currentColor\">{}</text></g>",
            r.x,
            r.y,
            r.w,
            r.h,
            r.cx(),
            r.cy() + 4, // optical baseline centering
            escape_html(&node.label),
        ));
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// SVG rendering — ERD
// ------------------------------------------------------------------

/// Entity title bar height.
const ERD_TITLE_H: i64 = 28;
/// Height of one field row.
const ERD_ROW_H: i64 = 22;
/// Gap between grid cells (room for relation lines + glyphs).
const ERD_GAP: i64 = 70;
/// Entities per grid row.
const ERD_PER_ROW: usize = 3;

/// Modifier badge text for a field (`PK FK UNQ`), empty when unmodified.
fn erd_badges(field: &ErdField) -> String {
    let mut badges: Vec<&str> = Vec::new();
    if field.pk {
        badges.push("PK");
    }
    if field.fk {
        badges.push("FK");
    }
    if field.unique {
        badges.push("UNQ");
    }
    badges.join(" ")
}

/// Box width for an entity: widest of name / field rows, min 120.
fn entity_width(entity: &ErdEntity) -> i64 {
    let mut chars = entity.name.chars().count();
    for field in &entity.fields {
        let badges = erd_badges(field);
        let row = field.name.chars().count()
            + if badges.is_empty() { 0 } else { badges.chars().count() + 3 };
        chars = chars.max(row);
    }
    (chars as i64 * CHAR_W + 24).max(120)
}

/// Box height for an entity: title bar + one row per field.
fn entity_height(entity: &ErdEntity) -> i64 {
    ERD_TITLE_H + entity.fields.len() as i64 * ERD_ROW_H
}

fn render_erd_svg(entities: &[ErdEntity], relations: &[ErdRelation], title: Option<&str>) -> String {
    // Uniform grid, ERD_PER_ROW entities per row: cell size from the largest
    // entity so positions stay simple and deterministic.
    let cell_w = entities.iter().map(entity_width).max().unwrap_or(120) + ERD_GAP;
    let cell_h = entities.iter().map(entity_height).max().unwrap_or(ERD_TITLE_H) + ERD_GAP;

    let rects: Vec<Rect> = entities
        .iter()
        .enumerate()
        .map(|(i, e)| Rect {
            x: MARGIN + (i % ERD_PER_ROW) as i64 * cell_w,
            y: MARGIN + (i / ERD_PER_ROW) as i64 * cell_h,
            w: entity_width(e),
            h: entity_height(e),
        })
        .collect();

    let n_cols = entities.len().min(ERD_PER_ROW).max(1);
    let n_rows = entities.len().div_ceil(ERD_PER_ROW).max(1);
    let total_w = MARGIN * 2 + n_cols as i64 * cell_w - ERD_GAP;
    let total_h = MARGIN * 2 + n_rows as i64 * cell_h - ERD_GAP;

    let mut svg = svg_open(total_w, total_h, title);

    // Relation lines first so entity boxes paint over the line ends.
    for rel in relations {
        let (Some(f), Some(t)) = (
            entities.iter().position(|e| e.name == rel.from),
            entities.iter().position(|e| e.name == rel.to),
        ) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let (a, b) = (&rects[f], &rects[t]);

        // Connect the facing borders along the dominant axis.
        let (dx, dy) = (b.cx() - a.cx(), b.cy() - a.cy());
        let (x1, y1, x2, y2) = if dx.abs() >= dy.abs() {
            if dx >= 0 {
                (a.x + a.w, a.cy(), b.x, b.cy())
            } else {
                (a.x, a.cy(), b.x + b.w, b.cy())
            }
        } else if dy >= 0 {
            (a.cx(), a.y + a.h, b.cx(), b.y)
        } else {
            (a.cx(), a.y, b.cx(), b.y + b.h)
        };

        svg.push_str(&format!(
            "<line class=\"surfdoc-diagram-relation\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#64748b\" stroke-width=\"1.5\" fill=\"none\"/>"
        ));

        // Cardinality glyphs ~1/8 in from each endpoint, nudged off the line.
        svg.push_str(&format!(
            "<text class=\"surfdoc-diagram-card\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
            x1 + (x2 - x1) / 8,
            y1 + (y2 - y1) / 8 - 5,
            rel.from_card.glyph(),
        ));
        svg.push_str(&format!(
            "<text class=\"surfdoc-diagram-card\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
            x2 - (x2 - x1) / 8,
            y2 - (y2 - y1) / 8 - 5,
            rel.to_card.glyph(),
        ));

        if let Some(label) = &rel.label {
            svg.push_str(&format!(
                "<text class=\"surfdoc-diagram-relation-label\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
                (x1 + x2) / 2,
                (y1 + y2) / 2 - 6,
                escape_html(label),
            ));
        }
    }

    // Entity tables: outer box, title bar, one row per field with a
    // right-aligned modifier badge.
    for (i, entity) in entities.iter().enumerate() {
        let r = &rects[i];
        svg.push_str(&format!("<g class=\"surfdoc-diagram-entity\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"#ffffff\" stroke=\"#64748b\"/>", r.x, r.y, r.w, r.h));
        svg.push_str(&format!(
            "<rect class=\"surfdoc-diagram-entity-title\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{ERD_TITLE_H}\" rx=\"4\" fill=\"#e2e8f0\" stroke=\"#64748b\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-weight=\"bold\" fill=\"currentColor\">{}</text>",
            r.x,
            r.y,
            r.w,
            r.cx(),
            r.y + ERD_TITLE_H / 2 + 4,
            escape_html(&entity.name),
        ));
        for (row, field) in entity.fields.iter().enumerate() {
            let row_y = r.y + ERD_TITLE_H + row as i64 * ERD_ROW_H + ERD_ROW_H / 2 + 4;
            svg.push_str(&format!(
                "<text class=\"surfdoc-diagram-field\" x=\"{}\" y=\"{row_y}\" font-size=\"12\" fill=\"currentColor\">{}</text>",
                r.x + 8,
                escape_html(&field.name),
            ));
            let badges = erd_badges(field);
            if !badges.is_empty() {
                svg.push_str(&format!(
                    "<text class=\"surfdoc-diagram-badge\" x=\"{}\" y=\"{row_y}\" text-anchor=\"end\" font-size=\"10\" fill=\"#64748b\">{badges}</text>",
                    r.x + r.w - 8,
                ));
            }
        }
        svg.push_str("</g>");
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// SVG rendering — flowchart
// ------------------------------------------------------------------

/// Uniform flowchart node height.
const FLOW_NODE_H: i64 = 44;
/// Vertical gap between flowchart rows.
const FLOW_ROW_GAP: i64 = 50;
/// Horizontal gap between nodes in a row.
const FLOW_HGAP: i64 = 36;

/// Bounding-box width for a flowchart node (diamonds/rounded need padding).
fn flow_node_width(node: &FlowNode) -> i64 {
    let base = label_width(&node.label);
    match node.shape {
        FlowShape::Box => base,
        FlowShape::Rounded => base + 20,
        FlowShape::Diamond => base + 36,
    }
}

fn render_flowchart_svg(nodes: &[FlowNode], edges: &[FlowEdge], title: Option<&str>) -> String {
    let widths: Vec<i64> = nodes.iter().map(flow_node_width).collect();
    let edge_idx: Vec<(usize, usize)> = edges
        .iter()
        .filter_map(|e| {
            Some((
                nodes.iter().position(|n| n.id == e.from)?,
                nodes.iter().position(|n| n.id == e.to)?,
            ))
        })
        .collect();
    let placed = layered_layout(&widths, FLOW_NODE_H, FLOW_ROW_GAP, FLOW_HGAP, &edge_idx);

    let mut svg = svg_open(placed.w, placed.h, title);
    svg.push_str(arrow_defs());

    for edge in edges {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == edge.from),
            nodes.iter().position(|n| n.id == edge.to),
        ) else {
            continue;
        };
        let (a, b) = (&placed.rects[f], &placed.rects[t]);
        let (x1, y1, x2, y2) = vert_edge_points(a, b);
        svg.push_str(&format!(
            "<line class=\"surfdoc-diagram-edge\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#64748b\" stroke-width=\"1.5\" fill=\"none\" marker-end=\"url(#surfdoc-arrow)\"/>"
        ));
        if let Some(label) = &edge.label {
            svg.push_str(&format!(
                "<text class=\"surfdoc-diagram-edge-label\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
                (x1 + x2) / 2,
                (y1 + y2) / 2 - 5,
                escape_html(label),
            ));
        }
    }

    for (i, node) in nodes.iter().enumerate() {
        let r = &placed.rects[i];
        let shape = match node.shape {
            FlowShape::Box => format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"#f8fafc\" stroke=\"#64748b\"/>",
                r.x, r.y, r.w, r.h
            ),
            FlowShape::Rounded => format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"#f8fafc\" stroke=\"#64748b\"/>",
                r.x,
                r.y,
                r.w,
                r.h,
                r.h / 2
            ),
            FlowShape::Diamond => format!(
                "<polygon points=\"{},{} {},{} {},{} {},{}\" fill=\"#f8fafc\" stroke=\"#64748b\"/>",
                r.cx(),
                r.y,
                r.x + r.w,
                r.cy(),
                r.cx(),
                r.y + r.h,
                r.x,
                r.cy(),
            ),
        };
        svg.push_str(&format!(
            "<g class=\"surfdoc-diagram-node\">{shape}<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"currentColor\">{}</text></g>",
            r.cx(),
            r.cy() + 4,
            escape_html(&node.label),
        ));
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// SVG rendering — sequence
// ------------------------------------------------------------------

/// Actor header box height.
const SEQ_ACTOR_H: i64 = 30;
/// Vertical spacing between timeline event rows.
const SEQ_MSG_GAP: i64 = 38;
/// Horizontal gap between actor columns.
const SEQ_COL_GAP: i64 = 60;
/// Width of an activation bar.
const SEQ_ACT_W: i64 = 10;

fn render_sequence_svg(actors: &[SeqActor], events: &[SeqEvent], title: Option<&str>) -> String {
    let n = actors.len();
    let col_w = actors.iter().map(|a| label_width(&a.label)).max().unwrap_or(80);
    let spacing = col_w + SEQ_COL_GAP;
    let cx = |k: usize| MARGIN + col_w / 2 + k as i64 * spacing;
    let idx = |id: &str| actors.iter().position(|a| a.id == id);

    let lifeline_top = MARGIN + SEQ_ACTOR_H;
    let ey = |i: i64| lifeline_top + (i + 1) * SEQ_MSG_GAP;
    let n_events = events.len() as i64;
    let bottom = lifeline_top + (n_events + 1) * SEQ_MSG_GAP;

    let total_w = if n > 0 {
        MARGIN * 2 + n as i64 * col_w + (n as i64 - 1) * SEQ_COL_GAP
    } else {
        2 * MARGIN
    };
    let total_h = bottom + MARGIN;

    let mut svg = svg_open(total_w, total_h, title);
    svg.push_str(arrow_defs());

    // Lifelines.
    for k in 0..n {
        svg.push_str(&format!(
            "<line class=\"surfdoc-diagram-lifeline\" x1=\"{}\" y1=\"{lifeline_top}\" x2=\"{}\" y2=\"{bottom}\" stroke=\"#cbd5e1\" stroke-width=\"1\" stroke-dasharray=\"4 4\"/>",
            cx(k),
            cx(k),
        ));
    }

    // Activation bars (computed from activate/deactivate events).
    let mut stacks: Vec<Vec<i64>> = vec![Vec::new(); n];
    let mut bars: Vec<(usize, i64, i64)> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        let y = ey(i as i64);
        match ev {
            SeqEvent::Activate(id) => {
                if let Some(k) = idx(id) {
                    stacks[k].push(y);
                }
            }
            SeqEvent::Deactivate(id) => {
                if let Some(k) = idx(id) {
                    if let Some(start) = stacks[k].pop() {
                        bars.push((k, start, y));
                    }
                }
            }
            SeqEvent::Message { .. } => {}
        }
    }
    for (k, stack) in stacks.iter().enumerate() {
        for &start in stack {
            bars.push((k, start, bottom));
        }
    }
    for (k, y1, y2) in &bars {
        svg.push_str(&format!(
            "<rect class=\"surfdoc-diagram-activation\" x=\"{}\" y=\"{y1}\" width=\"{SEQ_ACT_W}\" height=\"{}\" fill=\"#e2e8f0\" stroke=\"#64748b\"/>",
            cx(*k) - SEQ_ACT_W / 2,
            (y2 - y1).max(1),
        ));
    }

    // Messages.
    for (i, ev) in events.iter().enumerate() {
        if let SeqEvent::Message { from, to, label, dashed } = ev {
            let (Some(a), Some(b)) = (idx(from), idx(to)) else {
                continue;
            };
            let y = ey(i as i64);
            let dash = if *dashed { " stroke-dasharray=\"6 4\"" } else { "" };
            if a == b {
                let x = cx(a);
                svg.push_str(&format!(
                    "<path class=\"surfdoc-diagram-msg\" d=\"M{x} {y} L{} {y} L{} {} L{} {}\" stroke=\"#64748b\" stroke-width=\"1.5\" fill=\"none\"{dash} marker-end=\"url(#surfdoc-arrow)\"/>",
                    x + 30,
                    x + 30,
                    y + 14,
                    x + 4,
                    y + 14,
                ));
                if let Some(l) = label {
                    svg.push_str(&format!(
                        "<text class=\"surfdoc-diagram-msg-label\" x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#64748b\">{}</text>",
                        x + 36,
                        y + 4,
                        escape_html(l),
                    ));
                }
            } else {
                let (x1, x2) = (cx(a), cx(b));
                svg.push_str(&format!(
                    "<line class=\"surfdoc-diagram-msg\" x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"#64748b\" stroke-width=\"1.5\"{dash} marker-end=\"url(#surfdoc-arrow)\"/>"
                ));
                if let Some(l) = label {
                    svg.push_str(&format!(
                        "<text class=\"surfdoc-diagram-msg-label\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
                        (x1 + x2) / 2,
                        y - 5,
                        escape_html(l),
                    ));
                }
            }
        }
    }

    // Actor header boxes (drawn last so they sit above lifelines).
    for (k, actor) in actors.iter().enumerate() {
        let w = label_width(&actor.label);
        svg.push_str(&format!(
            "<g class=\"surfdoc-diagram-actor\"><rect x=\"{}\" y=\"{MARGIN}\" width=\"{w}\" height=\"{SEQ_ACTOR_H}\" rx=\"4\" fill=\"#f8fafc\" stroke=\"#64748b\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"currentColor\">{}</text></g>",
            cx(k) - w / 2,
            cx(k),
            MARGIN + SEQ_ACTOR_H / 2 + 4,
            escape_html(&actor.label),
        ));
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// SVG rendering — gantt
// ------------------------------------------------------------------

/// Height of one Gantt row (task or section header).
const GANTT_ROW_H: i64 = 26;
/// Height of a task bar within its row.
const GANTT_BAR_H: i64 = 16;
/// Top offset leaving room for the axis tick labels.
const GANTT_TOP: i64 = MARGIN + 22;

/// Count layout rows (section headers + tasks) for a task list.
fn gantt_row_count(tasks: &[GanttTask]) -> i64 {
    let mut rows = 0i64;
    let mut prev: Option<&str> = None;
    for (i, t) in tasks.iter().enumerate() {
        let sec = t.section.as_deref();
        if i == 0 || sec != prev {
            if sec.is_some() {
                rows += 1;
            }
            prev = sec;
        }
        rows += 1;
    }
    rows
}

fn render_gantt_svg(tasks: &[GanttTask], dated: bool, title: Option<&str>) -> String {
    if tasks.is_empty() {
        let mut svg = svg_open(2 * MARGIN, 2 * MARGIN, title);
        svg.push_str("</svg>");
        return svg;
    }

    let t0 = tasks.iter().map(|t| t.start).min().unwrap_or(0);
    let t1 = tasks.iter().map(|t| t.start + t.duration).max().unwrap_or(0);
    let span = (t1 - t0).max(1);
    let unit_w = (600 / span).clamp(6, 40);
    let label_col_w = tasks.iter().map(|t| label_width(&t.label)).max().unwrap_or(80);
    let chart_x = MARGIN + label_col_w + 12;
    let chart_w = span * unit_w;

    let n_rows = gantt_row_count(tasks);
    let bottom = GANTT_TOP + n_rows * GANTT_ROW_H;
    let total_w = chart_x + chart_w + MARGIN;
    let total_h = bottom + MARGIN;

    let mut svg = svg_open(total_w, total_h, title);

    // Axis gridlines + tick labels.
    let stride = (span / 8).max(1);
    let mut tick = t0;
    while tick <= t1 {
        let x = chart_x + (tick - t0) * unit_w;
        svg.push_str(&format!(
            "<line class=\"surfdoc-diagram-grid\" x1=\"{x}\" y1=\"{GANTT_TOP}\" x2=\"{x}\" y2=\"{bottom}\" stroke=\"#e2e8f0\" stroke-width=\"1\"/>"
        ));
        let label = if dated {
            let (y, m, d) = civil_from_days(tick);
            format!("{y:04}-{m:02}-{d:02}")
        } else {
            tick.to_string()
        };
        svg.push_str(&format!(
            "<text class=\"surfdoc-diagram-tick\" x=\"{x}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" fill=\"#64748b\">{}</text>",
            GANTT_TOP - 6,
            escape_html(&label),
        ));
        tick += stride;
    }

    // Rows: section headers + task bars.
    let mut y = GANTT_TOP;
    let mut prev: Option<&str> = None;
    for (i, t) in tasks.iter().enumerate() {
        let sec = t.section.as_deref();
        if i == 0 || sec != prev {
            if let Some(s) = sec {
                svg.push_str(&format!(
                    "<text class=\"surfdoc-diagram-section\" x=\"{MARGIN}\" y=\"{}\" font-size=\"12\" font-weight=\"bold\" fill=\"currentColor\">{}</text>",
                    y + GANTT_ROW_H / 2 + 4,
                    escape_html(s),
                ));
                y += GANTT_ROW_H;
            }
            prev = sec;
        }
        svg.push_str(&format!(
            "<text class=\"surfdoc-diagram-task\" x=\"{MARGIN}\" y=\"{}\" font-size=\"12\" fill=\"currentColor\">{}</text>",
            y + GANTT_ROW_H / 2 + 4,
            escape_html(&t.label),
        ));
        let bx = chart_x + (t.start - t0) * unit_w;
        let bw = (t.duration * unit_w).max(2);
        svg.push_str(&format!(
            "<rect class=\"surfdoc-diagram-bar\" x=\"{bx}\" y=\"{}\" width=\"{bw}\" height=\"{GANTT_BAR_H}\" rx=\"3\" fill=\"#94a3b8\" stroke=\"#64748b\"/>",
            y + (GANTT_ROW_H - GANTT_BAR_H) / 2,
        ));
        y += GANTT_ROW_H;
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// SVG rendering — state
// ------------------------------------------------------------------

/// Uniform state node height.
const STATE_NODE_H: i64 = 40;

fn render_state_svg(nodes: &[StateNode], transitions: &[StateTransition], title: Option<&str>) -> String {
    let widths: Vec<i64> = nodes
        .iter()
        .map(|n| if n.initial || n.final_ { 24 } else { label_width(&n.label) })
        .collect();
    let edge_idx: Vec<(usize, usize)> = transitions
        .iter()
        .filter_map(|tr| {
            Some((
                nodes.iter().position(|n| n.id == tr.from)?,
                nodes.iter().position(|n| n.id == tr.to)?,
            ))
        })
        .collect();
    let placed = layered_layout(&widths, STATE_NODE_H, 50, 40, &edge_idx);

    let mut svg = svg_open(placed.w, placed.h, title);
    svg.push_str(arrow_defs());

    for tr in transitions {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == tr.from),
            nodes.iter().position(|n| n.id == tr.to),
        ) else {
            continue;
        };
        let (a, b) = (&placed.rects[f], &placed.rects[t]);
        let (x1, y1, x2, y2) = vert_edge_points(a, b);
        svg.push_str(&format!(
            "<line class=\"surfdoc-diagram-transition\" x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#64748b\" stroke-width=\"1.5\" fill=\"none\" marker-end=\"url(#surfdoc-arrow)\"/>"
        ));
        if let Some(label) = &tr.label {
            svg.push_str(&format!(
                "<text class=\"surfdoc-diagram-transition-label\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#64748b\">{}</text>",
                (x1 + x2) / 2,
                (y1 + y2) / 2 - 5,
                escape_html(label),
            ));
        }
    }

    for (i, node) in nodes.iter().enumerate() {
        let r = &placed.rects[i];
        if node.initial {
            svg.push_str(&format!(
                "<circle class=\"surfdoc-diagram-initial\" cx=\"{}\" cy=\"{}\" r=\"8\" fill=\"#64748b\" stroke=\"#64748b\"/>",
                r.cx(),
                r.cy(),
            ));
        } else if node.final_ {
            svg.push_str(&format!(
                "<g class=\"surfdoc-diagram-final\"><circle cx=\"{0}\" cy=\"{1}\" r=\"10\" fill=\"none\" stroke=\"#64748b\"/><circle cx=\"{0}\" cy=\"{1}\" r=\"5\" fill=\"#64748b\"/></g>",
                r.cx(),
                r.cy(),
            ));
        } else {
            svg.push_str(&format!(
                "<g class=\"surfdoc-diagram-state\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"12\" fill=\"#f8fafc\" stroke=\"#64748b\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"currentColor\">{}</text></g>",
                r.x,
                r.y,
                r.w,
                r.h,
                r.cx(),
                r.cy() + 4,
                escape_html(&node.label),
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// SVG rendering — mindmap
// ------------------------------------------------------------------

/// Mindmap node box height.
const MIND_NODE_H: i64 = 30;
/// Vertical gap between mindmap leaves.
const MIND_VGAP: i64 = 16;
/// Horizontal gap between mindmap depth columns.
const MIND_COL_GAP: i64 = 40;

/// Direct children of node `i` (subsequent nodes one level deeper, up to the
/// next sibling-or-shallower node).
fn mind_children(nodes: &[MindNode], i: usize) -> Vec<usize> {
    let d = nodes[i].depth;
    let mut out = Vec::new();
    let mut j = i + 1;
    while j < nodes.len() && nodes[j].depth > d {
        if nodes[j].depth == d + 1 {
            out.push(j);
        }
        j += 1;
    }
    out
}

/// Assign each node a center-y: leaves stack sequentially, parents center on
/// their children. Returns this node's center-y.
fn assign_mind_y(nodes: &[MindNode], i: usize, leaf: &mut i64, ys: &mut [i64]) -> i64 {
    let kids = mind_children(nodes, i);
    let cy = if kids.is_empty() {
        let y = MARGIN + *leaf * (MIND_NODE_H + MIND_VGAP) + MIND_NODE_H / 2;
        *leaf += 1;
        y
    } else {
        let mut sum = 0;
        for &k in &kids {
            sum += assign_mind_y(nodes, k, leaf, ys);
        }
        sum / kids.len() as i64
    };
    ys[i] = cy;
    cy
}

fn render_mindmap_svg(nodes: &[MindNode], title: Option<&str>) -> String {
    if nodes.is_empty() {
        let mut svg = svg_open(2 * MARGIN, 2 * MARGIN, title);
        svg.push_str("</svg>");
        return svg;
    }

    let max_depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
    let mut col_w = vec![0i64; max_depth + 1];
    for n in nodes {
        let w = label_width(&n.label);
        if w > col_w[n.depth] {
            col_w[n.depth] = w;
        }
    }
    let mut col_x = vec![0i64; max_depth + 1];
    let mut x = MARGIN;
    for d in 0..=max_depth {
        col_x[d] = x;
        x += col_w[d] + MIND_COL_GAP;
    }

    let mut ys = vec![0i64; nodes.len()];
    let mut leaf = 0i64;
    for i in 0..nodes.len() {
        if nodes[i].depth == 0 {
            assign_mind_y(nodes, i, &mut leaf, &mut ys);
        }
    }

    let total_w = x - MIND_COL_GAP + MARGIN;
    let total_h = (MARGIN * 2 + leaf * (MIND_NODE_H + MIND_VGAP) - MIND_VGAP).max(2 * MARGIN);

    let mut svg = svg_open(total_w, total_h, title);

    // Branch connectors (parent right edge → child left edge).
    for i in 0..nodes.len() {
        let pd = nodes[i].depth;
        let px2 = col_x[pd] + col_w[pd];
        let py = ys[i];
        for c in mind_children(nodes, i) {
            let cd = nodes[c].depth;
            svg.push_str(&format!(
                "<line class=\"surfdoc-diagram-branch\" x1=\"{px2}\" y1=\"{py}\" x2=\"{}\" y2=\"{}\" stroke=\"#94a3b8\" stroke-width=\"1.5\" fill=\"none\"/>",
                col_x[cd],
                ys[c],
            ));
        }
    }

    // Node boxes; root (depth 0) gets a filled accent.
    for (i, node) in nodes.iter().enumerate() {
        let d = node.depth;
        let w = col_w[d];
        let nx = col_x[d];
        let ny = ys[i] - MIND_NODE_H / 2;
        let fill = if d == 0 { "#e2e8f0" } else { "#f8fafc" };
        svg.push_str(&format!(
            "<g class=\"surfdoc-diagram-mind-node\"><rect x=\"{nx}\" y=\"{ny}\" width=\"{w}\" height=\"{MIND_NODE_H}\" rx=\"6\" fill=\"{fill}\" stroke=\"#64748b\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"currentColor\">{}</text></g>",
            nx + w / 2,
            ys[i] + 4,
            escape_html(&node.label),
        ));
    }

    svg.push_str("</svg>");
    svg
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn arch(content: &str) -> DiagramModel {
        parse_diagram_source("architecture", content).expect("architecture should parse")
    }

    fn erd(content: &str) -> DiagramModel {
        parse_diagram_source("erd", content).expect("erd should parse")
    }

    // ── DSL parsing: architecture ───────────────────────────────────

    #[test]
    fn arch_nodes_and_edges() {
        let DiagramModel::Architecture { nodes, edges } = arch(
            "web: Web Frontend\napi: API Server\n\nweb -> api: HTTPS",
        ) else {
            panic!("expected Architecture model");
        };
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].id, "web");
        assert_eq!(nodes[0].label, "Web Frontend");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from, "web");
        assert_eq!(edges[0].to, "api");
        assert_eq!(edges[0].label.as_deref(), Some("HTTPS"));
        assert!(!edges[0].bidirectional);
    }

    #[test]
    fn arch_auto_declares_edge_ids() {
        let DiagramModel::Architecture { nodes, edges } = arch("a -> b\nb -> c") else {
            panic!("expected Architecture model");
        };
        // a, b, c all auto-declared with label = id, in first-reference order.
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
        assert!(nodes.iter().all(|n| n.id == n.label));
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn arch_bidirectional_edge() {
        let DiagramModel::Architecture { edges, .. } = arch("a <-> b") else {
            panic!("expected Architecture model");
        };
        assert!(edges[0].bidirectional);
        assert_eq!(edges[0].label, None);
    }

    #[test]
    fn arch_node_label_may_contain_arrow_text() {
        // `:` immediately after the id wins the classification — free label text.
        let DiagramModel::Architecture { nodes, edges } = arch("a: Sends -> downstream") else {
            panic!("expected Architecture model");
        };
        assert_eq!(nodes[0].label, "Sends -> downstream");
        assert!(edges.is_empty());
    }

    #[test]
    fn arch_malformed_line_reports_line_number() {
        let e = parse_diagram_source("architecture", "a: Fine\n\nnot a statement at all")
            .expect_err("junk line must fail");
        assert_eq!(e.line, 3);
    }

    #[test]
    fn arch_missing_edge_target_fails() {
        let e = parse_diagram_source("architecture", "a ->").expect_err("dangling arrow");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn arch_junk_after_edge_target_fails() {
        let e = parse_diagram_source("architecture", "a -> b junk").expect_err("junk suffix");
        assert_eq!(e.line, 1);
    }

    // ── DSL parsing: erd ────────────────────────────────────────────

    #[test]
    fn erd_entities_and_modifiers() {
        let DiagramModel::Erd { entities, .. } = erd(
            "users: id pk, email unique, org_id fk, name",
        ) else {
            panic!("expected Erd model");
        };
        assert_eq!(entities.len(), 1);
        let fields = &entities[0].fields;
        assert_eq!(fields.len(), 4);
        assert!(fields[0].pk);
        assert!(fields[1].unique);
        assert!(fields[2].fk);
        assert!(!fields[3].pk && !fields[3].fk && !fields[3].unique);
    }

    #[test]
    fn erd_relations_and_cardinalities() {
        let DiagramModel::Erd { relations, .. } = erd(
            "users 1--* orders: places\norders *--* products\nusers 1--1 profiles\norders *--1 stores",
        ) else {
            panic!("expected Erd model");
        };
        assert_eq!(relations.len(), 4);
        assert_eq!(relations[0].from_card, Cardinality::One);
        assert_eq!(relations[0].to_card, Cardinality::Many);
        assert_eq!(relations[0].label.as_deref(), Some("places"));
        assert_eq!(relations[1].from_card, Cardinality::Many);
        assert_eq!(relations[1].to_card, Cardinality::Many);
        assert_eq!(relations[2].to_card, Cardinality::One);
        assert_eq!(relations[3].from_card, Cardinality::Many);
        assert_eq!(relations[3].to_card, Cardinality::One);
    }

    #[test]
    fn erd_auto_declares_relation_entities() {
        let DiagramModel::Erd { entities, .. } = erd("users 1--* orders") else {
            panic!("expected Erd model");
        };
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["users", "orders"]);
        assert!(entities.iter().all(|e| e.fields.is_empty()));
    }

    #[test]
    fn erd_classifier_colon_beats_relation() {
        // A field list may mention dashes/stars; `:` first means entity.
        let DiagramModel::Erd { entities, relations } = erd("a: x, y pk") else {
            panic!("expected Erd model");
        };
        assert_eq!(entities.len(), 1);
        assert!(relations.is_empty());
    }

    #[test]
    fn erd_unknown_modifier_reports_line_number() {
        let e = parse_diagram_source("erd", "users: id pk\norders: id primary")
            .expect_err("bad modifier");
        assert_eq!(e.line, 2);
        assert!(e.message.contains("primary"));
    }

    #[test]
    fn erd_malformed_relation_token_fails() {
        let e = parse_diagram_source("erd", "a 1-* b").expect_err("bad cardinality");
        assert_eq!(e.line, 1);
    }

    // ── Type dispatch ───────────────────────────────────────────────

    #[test]
    fn unknown_type_is_an_error() {
        assert!(parse_diagram_source("venn", "a -> b").is_err());
        assert!(parse_diagram_source("", "a -> b").is_err());
    }

    // ── SVG rendering ───────────────────────────────────────────────

    #[test]
    fn arch_svg_structure() {
        let model = arch("web: Web\napi: API\nweb -> api: HTTPS\napi <-> web");
        let svg = render_svg(&model, Some("System"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("viewBox=\"0 0 "));
        assert!(svg.contains("role=\"img\""));
        assert!(svg.contains("<title>System</title>"));
        assert!(svg.contains("surfdoc-diagram-node"));
        assert!(svg.contains("marker id=\"surfdoc-arrow\""));
        assert!(svg.contains("marker-end=\"url(#surfdoc-arrow)\""));
        // Bidirectional edge gets a start marker too.
        assert!(svg.contains("marker-start=\"url(#surfdoc-arrow)\""));
        // Edge label at midpoint.
        assert!(svg.contains(">HTTPS</text>"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn arch_svg_no_title_omits_title_element() {
        let svg = render_svg(&arch("a -> b"), None);
        assert!(!svg.contains("<title>"));
    }

    #[test]
    fn arch_svg_cycle_never_loops() {
        // a -> b -> c -> a closes a cycle; layering must terminate.
        let svg = render_svg(&arch("a -> b\nb -> c\nc -> a"), None);
        assert!(svg.contains("surfdoc-diagram-node"));
    }

    #[test]
    fn erd_svg_structure() {
        let model = erd("users: id pk, email unique\norders: id pk, user_id fk\nusers 1--* orders: places");
        let svg = render_svg(&model, Some("Data Model"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Data Model</title>"));
        assert!(svg.contains("surfdoc-diagram-entity"));
        assert!(svg.contains(">users</text>"));
        assert!(svg.contains(">PK</text>"));
        assert!(svg.contains(">FK</text>"));
        assert!(svg.contains(">UNQ</text>"));
        // Cardinality glyphs: 1 and ∗.
        assert!(svg.contains(">1</text>"));
        assert!(svg.contains(">\u{2217}</text>"));
        assert!(svg.contains(">places</text>"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn svg_is_deterministic() {
        let arch_model = arch("a: Alpha\nb: Beta\nc: Gamma\na -> b\nb -> c: link\na <-> c");
        assert_eq!(render_svg(&arch_model, Some("t")), render_svg(&arch_model, Some("t")));

        let erd_model = erd("u: id pk\no: id pk, u_id fk\nu 1--* o");
        assert_eq!(render_svg(&erd_model, None), render_svg(&erd_model, None));
    }

    #[test]
    fn svg_escapes_user_labels() {
        let model = arch("a: <script>alert(1)</script>\na -> b: <img onerror=x>");
        let svg = render_svg(&model, Some("<script>t</script>"));
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(svg.contains("&lt;img onerror=x&gt;"));
        assert!(svg.contains("<title>&lt;script&gt;t&lt;/script&gt;</title>"));
    }

    #[test]
    fn erd_svg_escapes_entity_and_field_names() {
        // Ids can't contain `<`, but field names ride through split_whitespace —
        // the renderer must escape everything user-derived anyway.
        let DiagramModel::Erd { mut entities, relations } = erd("users: id pk") else {
            panic!("expected Erd model");
        };
        entities[0].name = "<b>users</b>".to_string();
        entities[0].fields[0].name = "<i>id</i>".to_string();
        let svg = render_svg(&DiagramModel::Erd { entities, relations }, None);
        assert!(!svg.contains("<b>"));
        assert!(svg.contains("&lt;b&gt;users&lt;/b&gt;"));
        assert!(svg.contains("&lt;i&gt;id&lt;/i&gt;"));
    }

    #[test]
    fn empty_body_parses_to_empty_model() {
        let DiagramModel::Architecture { nodes, edges } = arch("") else {
            panic!("expected Architecture model");
        };
        assert!(nodes.is_empty() && edges.is_empty());
        // And still renders a (trivial) svg without panicking.
        let svg = render_svg(&DiagramModel::Architecture { nodes, edges }, None);
        assert!(svg.starts_with("<svg"));
    }

    // ── flowchart ───────────────────────────────────────────────────

    fn flow(content: &str) -> DiagramModel {
        parse_diagram_source("flowchart", content).expect("flowchart should parse")
    }

    #[test]
    fn flowchart_parses_shapes_and_edges() {
        let DiagramModel::Flowchart { nodes, edges } = flow(
            "start [terminator]: Start\ncheck [diamond]: Valid?\nsave: Persist\nstart -> check\ncheck -> save: yes\ncheck -> start: no",
        ) else {
            panic!("expected Flowchart");
        };
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].shape, FlowShape::Rounded);
        assert_eq!(nodes[1].shape, FlowShape::Diamond);
        assert_eq!(nodes[1].label, "Valid?");
        assert_eq!(nodes[2].shape, FlowShape::Box);
        assert_eq!(edges.len(), 3);
        assert_eq!(edges[1].label.as_deref(), Some("yes"));
    }

    #[test]
    fn flowchart_auto_declares_edge_ids() {
        let DiagramModel::Flowchart { nodes, .. } = flow("a -> b\nb -> c") else {
            panic!("expected Flowchart");
        };
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn flowchart_svg_structure_and_determinism() {
        let model = flow("start [rounded]: Start\nd [diamond]: OK?\nend: Done\nstart -> d\nd -> end: yes");
        let svg = render_svg(&model, Some("Flow"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Flow</title>"));
        assert!(svg.contains("surfdoc-diagram-node"));
        assert!(svg.contains("<polygon")); // diamond
        assert!(svg.contains("marker-end=\"url(#surfdoc-arrow)\""));
        assert!(svg.contains(">yes</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Flow")), render_svg(&model, Some("Flow")));
    }

    // ── sequence ────────────────────────────────────────────────────

    fn seq(content: &str) -> DiagramModel {
        parse_diagram_source("sequence", content).expect("sequence should parse")
    }

    #[test]
    fn sequence_parses_actors_messages_activation() {
        let DiagramModel::Sequence { actors, events } = seq(
            "actor user: User\nactor api: API\nuser -> api: POST\nactivate api\napi --> user: 200\ndeactivate api",
        ) else {
            panic!("expected Sequence");
        };
        assert_eq!(actors.len(), 2);
        assert_eq!(actors[0].label, "User");
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], SeqEvent::Message { dashed: false, .. }));
        assert!(matches!(&events[1], SeqEvent::Activate(a) if a == "api"));
        assert!(matches!(&events[2], SeqEvent::Message { dashed: true, .. }));
        assert!(matches!(&events[3], SeqEvent::Deactivate(a) if a == "api"));
    }

    #[test]
    fn sequence_auto_declares_actors() {
        let DiagramModel::Sequence { actors, .. } = seq("a -> b: hi\nb --> a: bye") else {
            panic!("expected Sequence");
        };
        let ids: Vec<&str> = actors.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn sequence_svg_structure_and_determinism() {
        let model = seq("actor u: User\nactor s: Server\nu -> s: req\nactivate s\ns --> u: resp\ndeactivate s");
        let svg = render_svg(&model, Some("Seq"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Seq</title>"));
        assert!(svg.contains("surfdoc-diagram-lifeline"));
        assert!(svg.contains("surfdoc-diagram-actor"));
        assert!(svg.contains("surfdoc-diagram-activation"));
        assert!(svg.contains("stroke-dasharray=\"6 4\"")); // dashed return message
        assert!(svg.contains(">req</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, None), render_svg(&model, None));
    }

    // ── gantt ───────────────────────────────────────────────────────

    fn gantt(content: &str) -> DiagramModel {
        parse_diagram_source("gantt", content).expect("gantt should parse")
    }

    #[test]
    fn gantt_parses_numeric_and_sections() {
        let DiagramModel::Gantt { tasks, dated } = gantt(
            "section Plan\nResearch: 0, 3\nDesign: 3, 2\nsection Build\nImpl: 5, 4",
        ) else {
            panic!("expected Gantt");
        };
        assert!(!dated);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].section.as_deref(), Some("Plan"));
        assert_eq!(tasks[0].start, 0);
        assert_eq!(tasks[0].duration, 3);
        assert_eq!(tasks[2].section.as_deref(), Some("Build"));
    }

    #[test]
    fn gantt_parses_dates() {
        let DiagramModel::Gantt { tasks, dated } = gantt("A: 2026-01-01, 5\nB: 2026-01-06, 3") else {
            panic!("expected Gantt");
        };
        assert!(dated);
        // Day-numbers are contiguous: B starts 5 days after A.
        assert_eq!(tasks[1].start - tasks[0].start, 5);
    }

    #[test]
    fn gantt_rejects_mixed_units() {
        assert!(parse_diagram_source("gantt", "A: 0, 5\nB: 2026-01-06, 3").is_err());
    }

    #[test]
    fn gantt_svg_structure_and_determinism() {
        let model = gantt("section Plan\nResearch: 0, 3\nDesign: 3, 2");
        let svg = render_svg(&model, Some("Plan"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Plan</title>"));
        assert!(svg.contains("surfdoc-diagram-bar"));
        assert!(svg.contains("surfdoc-diagram-grid"));
        assert!(svg.contains("surfdoc-diagram-section"));
        assert!(svg.contains(">Research</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Plan")), render_svg(&model, Some("Plan")));
    }

    #[test]
    fn gantt_date_roundtrip() {
        // days_from_civil ∘ civil_from_days is the identity.
        let d = days_from_civil(2026, 6, 27);
        assert_eq!(civil_from_days(d), (2026, 6, 27));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    // ── state ───────────────────────────────────────────────────────

    fn state(content: &str) -> DiagramModel {
        parse_diagram_source("state", content).expect("state should parse")
    }

    #[test]
    fn state_parses_initial_final_and_transitions() {
        let DiagramModel::State { nodes, transitions } = state(
            "[*] -> Idle\nIdle -> Running: start\nRunning -> Idle: stop\nRunning -> [*]",
        ) else {
            panic!("expected State");
        };
        assert!(nodes.iter().any(|n| n.initial));
        assert!(nodes.iter().any(|n| n.final_));
        assert!(nodes.iter().any(|n| n.id == "Idle" && !n.initial && !n.final_));
        assert_eq!(transitions.len(), 4);
        assert_eq!(transitions[1].label.as_deref(), Some("start"));
    }

    #[test]
    fn state_svg_structure_and_determinism() {
        let model = state("[*] -> Idle\nIdle -> Done: go\nDone -> [*]");
        let svg = render_svg(&model, Some("SM"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>SM</title>"));
        assert!(svg.contains("surfdoc-diagram-initial"));
        assert!(svg.contains("surfdoc-diagram-final"));
        assert!(svg.contains("surfdoc-diagram-state"));
        assert!(svg.contains(">go</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("SM")), render_svg(&model, Some("SM")));
    }

    // ── mindmap ─────────────────────────────────────────────────────

    fn mind(content: &str) -> DiagramModel {
        parse_diagram_source("mindmap", content).expect("mindmap should parse")
    }

    #[test]
    fn mindmap_parses_depth_from_indent() {
        let DiagramModel::Mindmap { nodes } = mind("Root\n  Branch A\n    Leaf A1\n  Branch B") else {
            panic!("expected Mindmap");
        };
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].depth, 0);
        assert_eq!(nodes[0].label, "Root");
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[2].depth, 2);
        assert_eq!(nodes[3].depth, 1);
    }

    #[test]
    fn mindmap_svg_structure_and_determinism() {
        let model = mind("Product\n  Web\n    Landing\n    Pricing\n  Mobile\n  API");
        let svg = render_svg(&model, Some("Map"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Map</title>"));
        assert!(svg.contains("surfdoc-diagram-mind-node"));
        assert!(svg.contains("surfdoc-diagram-branch"));
        assert!(svg.contains(">Landing</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Map")), render_svg(&model, Some("Map")));
    }

    #[test]
    fn new_kinds_escape_user_text() {
        let svg = render_svg(&flow("a: <script>x</script>\na -> b"), None);
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&lt;script&gt;"));
    }

    #[test]
    fn empty_new_kinds_render_without_panic() {
        for kind in ["flowchart", "sequence", "gantt", "state", "mindmap"] {
            let model = parse_diagram_source(kind, "").expect("empty body parses");
            let svg = render_svg(&model, None);
            assert!(svg.starts_with("<svg"), "{kind} should render an svg");
            assert!(svg.ends_with("</svg>"));
        }
    }
}

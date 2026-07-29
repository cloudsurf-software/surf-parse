//! Native diagram DSL (`::diagram`) — parsing, geometry layout and SVG
//! rendering.
//!
//! Seventeen diagram kinds are supported: `architecture`, `erd`, `flowchart`,
//! `sequence`, `gantt`, `state`, `mindmap`, `class`, `timeline`, `journey`,
//! `quadrant`, `kanban`, `usecase`, `gitgraph`, `c4`, `requirement` and
//! `sankey`. Four further chart-alias types (`pie`, `donut`, `radar`,
//! `xychart`) are recognized by [`chart_alias`] and render through the
//! `::chart` pipeline instead of this module's geometry scenes. Bodies
//! written in mermaid syntax are translated to this DSL up front by
//! [`crate::mermaid_compat`].
//!
//! The DSL is line-oriented: blank lines are ignored and every other line is
//! exactly one statement. Malformed input returns a [`DiagramError`], which
//! renderers use to trigger the preformatted prose fallback — a diagram must
//! NEVER fail a render.
//!
//! Rendering is staged: [`build_scene`] lays a parsed model out into a typed
//! geometry scene (shapes + semantic paint roles, see
//! [`crate::diagram_scene`]), and [`emit_svg`] serializes that scene to SVG.
//! The same scene crosses the FFI so native clients draw identical layouts
//! from typed shapes.
//!
//! DETERMINISM: [`render_svg`] is a pure function of the model. Layout uses
//! only declaration-ordered `Vec`s (no `HashMap` iteration), no randomness and
//! no time, all geometry is integer arithmetic, so output is byte-stable
//! across runs — consumers pin exact substrings in tests.

use crate::diagram_scene::{NativeAnchor, NativeMarker, NativePoint, NativeRole, NativeShape};
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

// ── class ──────────────────────────────────────────────────────────

/// The connector kind of a UML class relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClassRelationKind {
    /// `A -> B` — plain association (arrowhead at B).
    Association,
    /// `A *-> B` — composition (filled diamond at A).
    Composition,
    /// `A o-> B` — aggregation (hollow diamond at A).
    Aggregation,
    /// `A ^-> B` — inheritance (hollow triangle at B).
    Inheritance,
}

/// One member row of a class: optional visibility sigil (`+`/`-`/`#`),
/// name, and whether it is a method (declared with a trailing `()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassMember {
    pub(crate) name: String,
    pub(crate) visibility: Option<char>,
    pub(crate) method: bool,
}

/// A class box (`Name: member, member, ...`). A bare `enum` or `trait`
/// first member sets the stereotype instead of declaring a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassBox {
    pub(crate) name: String,
    pub(crate) stereotype: Option<String>,
    pub(crate) fields: Vec<ClassMember>,
    pub(crate) methods: Vec<ClassMember>,
}

/// A class relation (`A -> B`, `A *-> B`, `A o-> B`, `A ^-> B`, optional
/// `: label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassRelation {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) kind: ClassRelationKind,
    pub(crate) label: Option<String>,
}

// ── timeline ───────────────────────────────────────────────────────

/// One event on a timeline spine. `marker` is the raw date/number text shown
/// next to the label (`None` in ordered, unnumbered mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimelineEvent {
    pub(crate) marker: Option<String>,
    pub(crate) label: String,
}

// ── journey ────────────────────────────────────────────────────────

/// One task of a user journey, with its 1..=5 satisfaction score and the
/// section (lane) it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JourneyTask {
    pub(crate) section: Option<String>,
    pub(crate) label: String,
    pub(crate) score: i64,
}

// ── quadrant ───────────────────────────────────────────────────────

/// A labeled point on a quadrant chart. Coordinates are stored per-mille
/// (0..=1000) so layout stays integer arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuadPoint {
    pub(crate) label: String,
    pub(crate) x_mil: i64,
    pub(crate) y_mil: i64,
}

// ── kanban ─────────────────────────────────────────────────────────

/// One kanban column: header name plus its cards in declaration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KanbanColumn {
    pub(crate) name: String,
    pub(crate) cards: Vec<String>,
}

// ── usecase ────────────────────────────────────────────────────────

/// An actor in a use-case diagram (drawn as a stick figure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UcActor {
    pub(crate) id: String,
    pub(crate) label: String,
}

/// A use case (drawn as an ellipse inside the system boundary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UcCase {
    pub(crate) id: String,
    pub(crate) label: String,
}

/// The kind of a use-case edge: plain association (`->`) or a dashed
/// `«include»`/`«extend»` dependency (`^->` with a mandatory label).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UcEdgeKind {
    Association,
    Include,
    Extend,
}

/// An edge of a use-case diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UcEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) kind: UcEdgeKind,
}

// ── gitgraph ───────────────────────────────────────────────────────

/// One commit on a gitgraph lane, in commit order. `merge_from` is the
/// index of the source-branch tip commit when this commit was created by a
/// `merge` statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitCommit {
    /// Index into the branch list (the lane this commit sits on).
    pub(crate) branch: usize,
    pub(crate) label: Option<String>,
    pub(crate) merge_from: Option<usize>,
}

// ── c4 ─────────────────────────────────────────────────────────────

/// The kind of a C4 node (drawn as a styled architecture box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum C4Kind {
    Person,
    System,
    Container,
}

/// A node of a C4 context/container diagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct C4Node {
    pub(crate) id: String,
    pub(crate) label: String,
    /// Technology annotation (containers only, `container id: Label: tech`).
    pub(crate) tech: Option<String>,
    pub(crate) kind: C4Kind,
    /// `[ext]` marker — drawn muted (a system outside the team's scope).
    pub(crate) external: bool,
    /// Index of the enclosing boundary, if the node was declared inside one.
    pub(crate) boundary: Option<usize>,
}

/// An edge of a C4 diagram (`a -> b`, optional `: label`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct C4Edge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: Option<String>,
}

// ── requirement ────────────────────────────────────────────────────

/// The relation kind of a requirement-diagram edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReqEdgeKind {
    Satisfies,
    Verifies,
    Refines,
    Traces,
    Contains,
    Derives,
}

impl ReqEdgeKind {
    /// The DSL keyword / edge-label word for this kind.
    fn word(self) -> &'static str {
        match self {
            ReqEdgeKind::Satisfies => "satisfies",
            ReqEdgeKind::Verifies => "verifies",
            ReqEdgeKind::Refines => "refines",
            ReqEdgeKind::Traces => "traces",
            ReqEdgeKind::Contains => "contains",
            ReqEdgeKind::Derives => "derives",
        }
    }
}

/// A requirement-diagram node: a «requirement» (with optional body text) or
/// a design «element» that satisfies/verifies requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReqNode {
    pub(crate) id: String,
    pub(crate) label: String,
    /// Requirement body text (`requirement id: Label: text`).
    pub(crate) text: Option<String>,
    /// True for `requirement` nodes, false for `element` nodes.
    pub(crate) requirement: bool,
}

/// A requirement-diagram edge (`a -> b: satisfies`, kind mandatory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReqEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) kind: ReqEdgeKind,
}

// ── sankey ─────────────────────────────────────────────────────────

/// One flow of a sankey diagram (`Source -> Target: value`). Values are
/// stored in centi-units (`value × 100`, rounded) so layout stays integer
/// arithmetic. Node names are free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SankeyFlow {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) value_cs: i64,
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
    Class {
        classes: Vec<ClassBox>,
        relations: Vec<ClassRelation>,
    },
    Timeline {
        events: Vec<TimelineEvent>,
    },
    Journey {
        tasks: Vec<JourneyTask>,
    },
    Quadrant {
        /// `x-axis Low --> High` labels (left, right), when declared.
        x_axis: Option<(String, String)>,
        /// `y-axis Low --> High` labels (bottom, top), when declared.
        y_axis: Option<(String, String)>,
        /// Quadrant labels 1..=4 (1 = top-right, 2 = top-left, 3 =
        /// bottom-left, 4 = bottom-right), each optional.
        labels: Vec<Option<String>>,
        points: Vec<QuadPoint>,
    },
    Kanban {
        columns: Vec<KanbanColumn>,
    },
    UseCase {
        actors: Vec<UcActor>,
        cases: Vec<UcCase>,
        edges: Vec<UcEdge>,
    },
    GitGraph {
        /// Branch names in first-reference order (lane order); index 0 is
        /// always `main`.
        branches: Vec<String>,
        commits: Vec<GitCommit>,
    },
    C4 {
        nodes: Vec<C4Node>,
        /// Boundary names in declaration order.
        boundaries: Vec<String>,
        edges: Vec<C4Edge>,
    },
    Requirement {
        nodes: Vec<ReqNode>,
        edges: Vec<ReqEdge>,
    },
    Sankey {
        /// Node names in first-reference order.
        nodes: Vec<String>,
        flows: Vec<SankeyFlow>,
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
        "class" => parse_class(content),
        "timeline" => parse_timeline(content),
        "journey" => parse_journey(content),
        "quadrant" => parse_quadrant(content),
        "kanban" => parse_kanban(content),
        "usecase" => parse_usecase(content),
        "gitgraph" => parse_gitgraph(content),
        "c4" => parse_c4(content),
        "requirement" => parse_requirement(content),
        "sankey" => parse_sankey(content),
        other => Err(DiagramError {
            line: 0,
            message: format!("unknown diagram type \"{other}\""),
        }),
    }
}

/// Map a `::diagram` chart-alias type to the `::chart` type it renders as.
///
/// `pie`, `donut` and `radar` forward directly; `xychart` is an alias for a
/// `line` chart (the general xy-series form — bodies with one or many series
/// both render as lines). Returns `None` for every native geometry type so
/// the caller falls through to [`parse_diagram_source`].
pub(crate) fn chart_alias(diagram_type: &str) -> Option<crate::types::ChartType> {
    match diagram_type {
        "pie" => Some(crate::types::ChartType::Pie),
        "donut" => Some(crate::types::ChartType::Donut),
        "radar" => Some(crate::types::ChartType::Radar),
        "xychart" => Some(crate::types::ChartType::Line),
        _ => None,
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
        if duration > GANTT_VALUE_MAX {
            return Err(err(line_no, format!("duration \"{}\" is out of range", dur_s.trim())));
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

/// Layout-safe bound for gantt starts and durations (units/days). Values
/// beyond this would overflow the integer pixel arithmetic in the scene
/// stage; a bound this generous (±1 billion units ≈ 2.7 million years of
/// days) rejects only pathological input.
const GANTT_VALUE_MAX: i64 = 1_000_000_000;

/// Parse a Gantt start value: either a plain integer (numeric units) or an
/// ISO `YYYY-MM-DD` date (converted to a day-number). Returns `(value,
/// is_date)`.
fn parse_gantt_value(s: &str, line_no: usize) -> Result<(i64, bool), DiagramError> {
    if let Ok(n) = s.parse::<i64>() {
        if !(-GANTT_VALUE_MAX..=GANTT_VALUE_MAX).contains(&n) {
            return Err(err(line_no, format!("start value \"{s}\" is out of range")));
        }
        return Ok((n, false));
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<i64>(),
            parts[2].parse::<i64>(),
        ) {
            // Years are bounded before any day-number arithmetic runs.
            if (0..=9999).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d) {
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
// DSL parsing — class
// ------------------------------------------------------------------

/// Parse a `class` body (UML class diagram).
///
/// DSL (line-oriented, grid layout like `erd`):
/// - class:    `Name: member, member, ...` — members take an optional
///   leading visibility sigil (`+` public, `-` private, `#` protected) and a
///   trailing `()` marks a method. A bare `enum` or `trait` first member
///   sets the stereotype (rendered `«enum»`/`«trait»` above the name).
/// - relation: `A -> B` (association), `A *-> B` (composition), `A o-> B`
///   (aggregation), `A ^-> B` (inheritance), each with an optional `: label`.
///
/// Ids referenced in relations without a class line are auto-declared as
/// empty classes in first-reference order, so layout stays deterministic.
///
/// Example:
/// ```text
/// User: +id, -email, +save()
/// Role: enum, Admin, Member
/// Admin ^-> User
/// User *-> Role: has
/// ```
fn parse_class(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut classes: Vec<ClassBox> = Vec::new();
    let mut relations: Vec<ClassRelation> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let (name, rest) = split_leading_id(line);
        if name.is_empty() {
            return Err(err(line_no, format!("expected class or relation, found \"{line}\"")));
        }
        let rest = rest.trim_start();

        // Classifier: `:` right after the name makes it a class declaration;
        // a relation arrow makes it a relation.
        if let Some(member_list) = rest.strip_prefix(':') {
            classes.push(parse_class_box(name, member_list, line_no)?);
            continue;
        }

        let (kind, after_arrow) = if let Some(a) = rest.strip_prefix("*->") {
            (ClassRelationKind::Composition, a)
        } else if let Some(a) = rest.strip_prefix("o->") {
            (ClassRelationKind::Aggregation, a)
        } else if let Some(a) = rest.strip_prefix("^->") {
            (ClassRelationKind::Inheritance, a)
        } else if let Some(a) = rest.strip_prefix("->") {
            (ClassRelationKind::Association, a)
        } else {
            return Err(err(
                line_no,
                format!("expected `:` or a relation arrow (`->`, `*->`, `o->`, `^->`) after \"{name}\""),
            ));
        };

        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target class after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;

        relations.push(ClassRelation {
            from: name.to_string(),
            to: to.to_string(),
            kind,
            label,
        });
    }

    // Auto-declare relation endpoints that never had a class line, in
    // first-reference order so layout stays deterministic.
    for rel in &relations {
        for name in [&rel.from, &rel.to] {
            if !classes.iter().any(|c| &c.name == name) {
                classes.push(ClassBox {
                    name: name.clone(),
                    stereotype: None,
                    fields: Vec::new(),
                    methods: Vec::new(),
                });
            }
        }
    }

    Ok(DiagramModel::Class { classes, relations })
}

/// Parse the comma-separated member list of a class line into a [`ClassBox`].
fn parse_class_box(name: &str, list: &str, line_no: usize) -> Result<ClassBox, DiagramError> {
    let mut class = ClassBox {
        name: name.to_string(),
        stereotype: None,
        fields: Vec::new(),
        methods: Vec::new(),
    };

    let mut first = true;
    for segment in list.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue; // tolerate trailing/duplicate commas
        }
        // A bare `enum`/`trait` first member is the stereotype, not a member.
        if first && (segment == "enum" || segment == "trait") {
            class.stereotype = Some(segment.to_string());
            first = false;
            continue;
        }
        first = false;

        let (visibility, rest) = match segment.chars().next() {
            Some(v @ ('+' | '-' | '#')) => (Some(v), segment[1..].trim_start()),
            _ => (None, segment),
        };
        let (bare, method) = match rest.strip_suffix("()") {
            Some(b) => (b.trim_end(), true),
            None => (rest, false),
        };
        if bare.is_empty() {
            return Err(err(line_no, format!("empty member name in \"{segment}\"")));
        }

        let member = ClassMember {
            name: bare.to_string(),
            visibility,
            method,
        };
        if method {
            class.methods.push(member);
        } else {
            class.fields.push(member);
        }
    }

    Ok(class)
}

// ------------------------------------------------------------------
// DSL parsing — timeline
// ------------------------------------------------------------------

/// Parse a `timeline` body.
///
/// DSL (line-oriented, left-to-right spine; one event per line):
/// - dated event:   `2026-01: Label` — marker is an integer, `YYYY-MM` or
///   `YYYY-MM-DD`
/// - ordered event: `Label` — no marker, events run in declaration order
///
/// The two modes cannot mix: either every event carries a marker or none
/// does. Markers follow the gantt convention — all-numeric or all-ISO,
/// never both. A line whose leading `text:` does not parse as a marker is
/// treated as a plain ordered label (so labels may contain colons).
///
/// Example:
/// ```text
/// 2026-01: Kickoff
/// 2026-03: Private beta
/// 2026-06-15: Launch
/// ```
fn parse_timeline(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut events: Vec<TimelineEvent> = Vec::new();
    // Marker convention across the whole body: None until the first event,
    // then Some(is_date) — same no-mix law as gantt start values.
    let mut dated: Option<bool> = None;
    let mut marked: Option<bool> = None;

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        // A line is a dated event only when the text before the first `:`
        // parses as a marker; anything else is an ordered label.
        let parsed = line
            .split_once(':')
            .and_then(|(m, rest)| Some((m.trim(), timeline_marker_value(m.trim())?, rest.trim())));
        let (marker, is_date) = match parsed {
            Some((marker, is_date, label)) => {
                if label.is_empty() {
                    return Err(err(line_no, "event label must not be empty"));
                }
                events.push(TimelineEvent {
                    marker: Some(marker.to_string()),
                    label: label.to_string(),
                });
                (true, Some(is_date))
            }
            None => {
                events.push(TimelineEvent {
                    marker: None,
                    label: line.to_string(),
                });
                (false, None)
            }
        };

        match marked {
            None => marked = Some(marker),
            Some(m) if m != marker => {
                return Err(err(line_no, "cannot mix dated and unmarked events"));
            }
            _ => {}
        }
        if let Some(is_date) = is_date {
            match dated {
                None => dated = Some(is_date),
                Some(d) if d != is_date => {
                    return Err(err(line_no, "cannot mix numeric and date markers"));
                }
                _ => {}
            }
        }
    }

    Ok(DiagramModel::Timeline { events })
}

/// Classify a candidate timeline marker: `Some(is_date)` for an integer
/// (`false`), a `YYYY-MM` or `YYYY-MM-DD` date (`true`); `None` otherwise.
fn timeline_marker_value(s: &str) -> Option<bool> {
    if s.parse::<i64>().is_ok() {
        return Some(false);
    }
    let parts: Vec<&str> = s.split('-').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let _year = parts[0].parse::<i64>().ok()?;
    let m = parts[1].parse::<i64>().ok()?;
    let d = if parts.len() == 3 { parts[2].parse::<i64>().ok()? } else { 1 };
    if (1..=12).contains(&m) && (1..=31).contains(&d) {
        Some(true)
    } else {
        None
    }
}

// ------------------------------------------------------------------
// DSL parsing — journey
// ------------------------------------------------------------------

/// Parse a `journey` body.
///
/// DSL (line-oriented, left-to-right lanes):
/// - section: `section Name`  — groups following tasks into a lane (optional)
/// - task:    `Label: score`  — score is an integer 1..=5 (1 = worst,
///   5 = best), drawn as a dot at the score's height on the band
///
/// Example:
/// ```text
/// section Onboarding
/// Sign up: 3
/// Verify email: 2
/// section Daily use
/// Open app: 5
/// ```
fn parse_journey(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut tasks: Vec<JourneyTask> = Vec::new();
    let mut section: Option<String> = None;

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

        let Some((label, score_s)) = line.split_once(':') else {
            return Err(err(line_no, format!("expected `Label: score`, found \"{line}\"")));
        };
        let label = label.trim();
        if label.is_empty() {
            return Err(err(line_no, "task label must not be empty"));
        }
        let score: i64 = score_s
            .trim()
            .parse()
            .map_err(|_| err(line_no, format!("invalid score \"{}\"", score_s.trim())))?;
        if !(1..=5).contains(&score) {
            return Err(err(line_no, "score must be between 1 and 5"));
        }
        tasks.push(JourneyTask {
            section: section.clone(),
            label: label.to_string(),
            score,
        });
    }

    Ok(DiagramModel::Journey { tasks })
}

// ------------------------------------------------------------------
// DSL parsing — quadrant
// ------------------------------------------------------------------

/// Parse a `quadrant` body.
///
/// DSL (line-oriented, 2×2 chart; every statement optional except points):
/// - x axis:   `x-axis Low --> High`   — left / right end labels
/// - y axis:   `y-axis Low --> High`   — bottom / top end labels
/// - quadrant: `quadrant-1: Label` … `quadrant-4: Label` — 1 = top-right,
///   2 = top-left, 3 = bottom-left, 4 = bottom-right (optional)
/// - point:    `Name: 0.3, 0.7`        — coordinates in 0..=1 (x, y)
///
/// Example:
/// ```text
/// x-axis Low effort --> High effort
/// y-axis Low impact --> High impact
/// quadrant-1: Quick wins
/// Task A: 0.3, 0.7
/// ```
fn parse_quadrant(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut x_axis: Option<(String, String)> = None;
    let mut y_axis: Option<(String, String)> = None;
    let mut labels: Vec<Option<String>> = vec![None, None, None, None];
    let mut points: Vec<QuadPoint> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = strip_keyword(line, "x-axis") {
            x_axis = Some(parse_axis_labels(rest, line_no)?);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "y-axis") {
            y_axis = Some(parse_axis_labels(rest, line_no)?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("quadrant-") {
            let (n_s, after) = rest.split_at(rest.find(':').unwrap_or(rest.len()));
            let n: usize = n_s
                .trim()
                .parse()
                .map_err(|_| err(line_no, format!("invalid quadrant number \"{}\"", n_s.trim())))?;
            if !(1..=4).contains(&n) {
                return Err(err(line_no, "quadrant number must be 1..=4"));
            }
            let Some(label) = after.strip_prefix(':') else {
                return Err(err(line_no, "expected `: Label` after quadrant number"));
            };
            labels[n - 1] = Some(label.trim().to_string());
            continue;
        }

        // Point line: `Name: x, y`.
        let Some((label, coords)) = line.split_once(':') else {
            return Err(err(line_no, format!("expected `Name: x, y`, found \"{line}\"")));
        };
        let label = label.trim();
        if label.is_empty() {
            return Err(err(line_no, "point name must not be empty"));
        }
        let Some((x_s, y_s)) = coords.split_once(',') else {
            return Err(err(line_no, "expected `x, y` coordinates"));
        };
        points.push(QuadPoint {
            label: label.to_string(),
            x_mil: parse_unit_coord(x_s.trim(), line_no)?,
            y_mil: parse_unit_coord(y_s.trim(), line_no)?,
        });
    }

    Ok(DiagramModel::Quadrant { x_axis, y_axis, labels, points })
}

/// Parse the `Low --> High` remainder of an axis line into its two labels.
fn parse_axis_labels(rest: &str, line_no: usize) -> Result<(String, String), DiagramError> {
    let Some((low, high)) = rest.split_once("-->") else {
        return Err(err(line_no, "expected `Low --> High` axis labels"));
    };
    let (low, high) = (low.trim(), high.trim());
    if low.is_empty() || high.is_empty() {
        return Err(err(line_no, "axis labels must not be empty"));
    }
    Ok((low.to_string(), high.to_string()))
}

/// Parse a 0..=1 coordinate into per-mille (0..=1000) integer units.
fn parse_unit_coord(s: &str, line_no: usize) -> Result<i64, DiagramError> {
    let v: f64 = s
        .parse()
        .map_err(|_| err(line_no, format!("invalid coordinate \"{s}\"")))?;
    if !(0.0..=1.0).contains(&v) {
        return Err(err(line_no, format!("coordinate \"{s}\" must be between 0 and 1")));
    }
    Ok((v * 1000.0).round() as i64)
}

// ------------------------------------------------------------------
// DSL parsing — kanban
// ------------------------------------------------------------------

/// Parse a `kanban` body.
///
/// DSL (columns left-to-right; indentation marks cards):
/// - column: `column Name`  or a flush-left `Name:` header line
/// - card:   any indented line — belongs to the most recent column
///
/// Example:
/// ```text
/// column To do
///   Write spec
///   Review API
/// Doing:
///   Build parser
/// Done:
///   Ship v1
/// ```
fn parse_kanban(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut columns: Vec<KanbanColumn> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        if raw.trim().is_empty() {
            continue;
        }

        // Indented line = card in the current column.
        if raw.starts_with(' ') || raw.starts_with('\t') {
            let Some(col) = columns.last_mut() else {
                return Err(err(line_no, "card before any column header"));
            };
            col.cards.push(raw.trim().to_string());
            continue;
        }

        let line = raw.trim();
        if let Some(rest) = strip_keyword(line, "column") {
            let name = rest.trim();
            if name.is_empty() {
                return Err(err(line_no, "column name must not be empty"));
            }
            columns.push(KanbanColumn { name: name.to_string(), cards: Vec::new() });
            continue;
        }
        if let Some(name) = line.strip_suffix(':') {
            let name = name.trim();
            if name.is_empty() {
                return Err(err(line_no, "column name must not be empty"));
            }
            columns.push(KanbanColumn { name: name.to_string(), cards: Vec::new() });
            continue;
        }

        return Err(err(
            line_no,
            format!("expected `column Name`, `Name:` or an indented card, found \"{line}\""),
        ));
    }

    Ok(DiagramModel::Kanban { columns })
}

// ------------------------------------------------------------------
// DSL parsing — usecase
// ------------------------------------------------------------------

/// Parse a `usecase` body (UML use-case diagram).
///
/// DSL (line-oriented; actors left, use cases inside the system boundary):
/// - actor:    `actor id`  or  `actor id: Label`
/// - use case: `usecase id: Label` (label defaults to the id)
/// - edge:     `actor -> id`             — association (plain line)
/// - edge:     `id ^-> id: includes`     — dashed `«include»` dependency
/// - edge:     `id ^-> id: extends`      — dashed `«extend»` dependency
///
/// Undeclared edge endpoints are auto-declared: `->` sources become actors
/// and targets use cases; both `^->` endpoints become use cases.
///
/// Example:
/// ```text
/// actor customer: Customer
/// usecase browse: Browse catalog
/// usecase pay: Enter payment
/// customer -> browse
/// browse ^-> pay: includes
/// ```
fn parse_usecase(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut actors: Vec<UcActor> = Vec::new();
    let mut cases: Vec<UcCase> = Vec::new();
    let mut edges: Vec<UcEdge> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = strip_keyword(line, "actor") {
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
                actors.push(UcActor { id: id.to_string(), label });
            }
            continue;
        }
        if let Some(rest) = strip_keyword(line, "usecase") {
            let (id, after) = split_leading_id(rest.trim_start());
            if id.is_empty() {
                return Err(err(line_no, "expected use-case id"));
            }
            let label = match after.trim().strip_prefix(':') {
                Some(l) if !l.trim().is_empty() => l.trim().to_string(),
                _ => id.to_string(),
            };
            if let Some(c) = cases.iter_mut().find(|c| c.id == id) {
                c.label = label;
            } else {
                cases.push(UcCase { id: id.to_string(), label });
            }
            continue;
        }

        // Edge line: `a -> b` or `a ^-> b: includes|extends`.
        let (from, rest) = split_leading_id(line);
        if from.is_empty() {
            return Err(err(line_no, format!("expected actor, usecase or edge, found \"{line}\"")));
        }
        let rest = rest.trim_start();
        let (dependency, after_arrow) = if let Some(a) = rest.strip_prefix("^->") {
            (true, a)
        } else if let Some(a) = rest.strip_prefix("->") {
            (false, a)
        } else {
            return Err(err(line_no, format!("expected `->` or `^->` after \"{from}\"")));
        };
        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target id after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;

        let kind = if dependency {
            match label.as_deref() {
                Some("includes") | Some("include") => UcEdgeKind::Include,
                Some("extends") | Some("extend") => UcEdgeKind::Extend,
                _ => {
                    return Err(err(
                        line_no,
                        "expected `: includes` or `: extends` after `^->` edge",
                    ));
                }
            }
        } else {
            if label.is_some() {
                return Err(err(line_no, "association edges (`->`) take no label"));
            }
            UcEdgeKind::Association
        };

        // Auto-declare unknown endpoints: `->` sources are actors and
        // targets use cases; both `^->` endpoints are use cases.
        let known = |actors: &[UcActor], cases: &[UcCase], id: &str| {
            actors.iter().any(|a| a.id == id) || cases.iter().any(|c| c.id == id)
        };
        if !known(&actors, &cases, from) {
            if dependency {
                cases.push(UcCase { id: from.to_string(), label: from.to_string() });
            } else {
                actors.push(UcActor { id: from.to_string(), label: from.to_string() });
            }
        }
        if !known(&actors, &cases, to) {
            cases.push(UcCase { id: to.to_string(), label: to.to_string() });
        }

        edges.push(UcEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        });
    }

    Ok(DiagramModel::UseCase { actors, cases, edges })
}

// ------------------------------------------------------------------
// DSL parsing — gitgraph
// ------------------------------------------------------------------

/// Parse a `gitgraph` body.
///
/// DSL (line-oriented, one statement per line; commits run left-to-right):
/// - `commit`  or  `commit: label`      — a commit on the current branch
/// - `branch name`                      — create `name` and switch to it
/// - `checkout name`                    — switch to an existing branch
///   (unknown names are auto-declared, first-reference order)
/// - `merge name`  or  `merge name: label` — merge `name`'s tip into the
///   current branch as a new commit
///
/// The current branch starts as `main` (lane 0). Each branch is a
/// horizontal lane; merges draw a connector between lanes.
///
/// Example:
/// ```text
/// commit: init
/// branch feature
/// commit: draft
/// checkout main
/// merge feature: ship
/// ```
fn parse_gitgraph(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut branches: Vec<String> = vec!["main".to_string()];
    let mut commits: Vec<GitCommit> = Vec::new();
    // Tip commit index per branch (None until the branch has a commit).
    let mut tips: Vec<Option<usize>> = vec![None];
    let mut current = 0usize;

    let ensure = |branches: &mut Vec<String>, tips: &mut Vec<Option<usize>>, name: &str| {
        match branches.iter().position(|b| b == name) {
            Some(i) => i,
            None => {
                branches.push(name.to_string());
                tips.push(None);
                branches.len() - 1
            }
        }
    };

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if line == "commit" || line.starts_with("commit:") || strip_keyword(line, "commit").is_some() {
            let rest = line.strip_prefix("commit").expect("prefix checked").trim_start();
            let label = parse_label_suffix(rest, line_no)?;
            commits.push(GitCommit { branch: current, label, merge_from: None });
            tips[current] = Some(commits.len() - 1);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "branch") {
            let name = rest.trim();
            if name.is_empty() {
                return Err(err(line_no, "expected branch name after `branch`"));
            }
            current = ensure(&mut branches, &mut tips, name);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "checkout").or_else(|| strip_keyword(line, "switch")) {
            let name = rest.trim();
            if name.is_empty() {
                return Err(err(line_no, "expected branch name after `checkout`"));
            }
            current = ensure(&mut branches, &mut tips, name);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "merge") {
            let rest = rest.trim_start();
            let (name, after) = match rest.split_once(':') {
                Some((n, l)) => (n.trim(), Some(l.trim().to_string())),
                None => (rest.trim(), None),
            };
            if name.is_empty() {
                return Err(err(line_no, "expected branch name after `merge`"));
            }
            let source = ensure(&mut branches, &mut tips, name);
            // Merging a branch with no commits degrades to a plain commit.
            let merge_from = tips[source];
            commits.push(GitCommit { branch: current, label: after, merge_from });
            tips[current] = Some(commits.len() - 1);
            continue;
        }

        return Err(err(
            line_no,
            format!("expected `commit`, `branch`, `checkout` or `merge`, found \"{line}\""),
        ));
    }

    Ok(DiagramModel::GitGraph { branches, commits })
}

// ------------------------------------------------------------------
// DSL parsing — c4
// ------------------------------------------------------------------

/// Parse a `c4` body (a styled C4 context/container profile).
///
/// DSL (line-oriented; one nesting level of boundaries):
/// - person:    `person id: Label`
/// - system:    `system id: Label`  — append ` [ext]` for an external system
/// - container: `container id: Label`  or  `container id: Label: tech`
/// - boundary:  `boundary Name {` … `}` — groups the nodes declared inside
///   in a dashed rect (no nested boundaries)
/// - edge:      `a -> b`  or  `a -> b: label`
///
/// The ` [ext]` suffix is accepted on any node kind. Container labels split
/// at their first inner colon (`Label: tech`), so labels themselves must not
/// contain colons. Edge endpoints never declared are auto-declared as
/// systems in first-reference order.
///
/// Example:
/// ```text
/// person user: Customer
/// boundary Platform {
///   container api: API App: Rust
///   container db: Database: Postgres
/// }
/// system mail: Mail Provider [ext]
/// user -> api: Uses
/// api -> db: SQL
/// api -> mail: SMTP
/// ```
fn parse_c4(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<C4Node> = Vec::new();
    let mut boundaries: Vec<String> = Vec::new();
    let mut edges: Vec<C4Edge> = Vec::new();
    let mut open_boundary: Option<usize> = None;

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = strip_keyword(line, "boundary") {
            if open_boundary.is_some() {
                return Err(err(line_no, "boundaries cannot nest"));
            }
            let Some(name) = rest.trim().strip_suffix('{') else {
                return Err(err(line_no, "expected `{` at end of boundary line"));
            };
            let name = name.trim();
            if name.is_empty() {
                return Err(err(line_no, "boundary name must not be empty"));
            }
            boundaries.push(name.to_string());
            open_boundary = Some(boundaries.len() - 1);
            continue;
        }
        if line == "}" {
            if open_boundary.take().is_none() {
                return Err(err(line_no, "`}` without an open boundary"));
            }
            continue;
        }

        // Node declaration lines: `person|system|container id: Label…`.
        let kind = if strip_keyword(line, "person").is_some() {
            Some(C4Kind::Person)
        } else if strip_keyword(line, "system").is_some() {
            Some(C4Kind::System)
        } else if strip_keyword(line, "container").is_some() {
            Some(C4Kind::Container)
        } else {
            None
        };
        if let Some(kind) = kind {
            let rest = line.split_once(char::is_whitespace).expect("keyword matched").1;
            let (id, after) = split_leading_id(rest.trim_start());
            if id.is_empty() {
                return Err(err(line_no, "expected node id"));
            }
            let Some(label_part) = after.trim_start().strip_prefix(':') else {
                return Err(err(line_no, format!("expected `: Label` after \"{id}\"")));
            };
            let mut label_part = label_part.trim().to_string();
            let external = match label_part.strip_suffix("[ext]") {
                Some(stripped) => {
                    label_part = stripped.trim_end().to_string();
                    true
                }
                None => false,
            };
            // Containers take an optional `: tech` annotation after the label.
            let (label, tech) = if kind == C4Kind::Container {
                match label_part.split_once(':') {
                    Some((l, t)) => (l.trim().to_string(), Some(t.trim().to_string())),
                    None => (label_part.clone(), None),
                }
            } else {
                (label_part.clone(), None)
            };
            let label = if label.is_empty() { id.to_string() } else { label };
            nodes.push(C4Node {
                id: id.to_string(),
                label,
                tech: tech.filter(|t| !t.is_empty()),
                kind,
                external,
                boundary: open_boundary,
            });
            continue;
        }

        // Edge line: `a -> b`, optional `: label`.
        let (from, rest) = split_leading_id(line);
        if from.is_empty() {
            return Err(err(line_no, format!("expected node or edge, found \"{line}\"")));
        }
        let Some(after_arrow) = rest.trim_start().strip_prefix("->") else {
            return Err(err(line_no, format!("expected `->` after \"{from}\"")));
        };
        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target id after arrow"));
        }
        let label = parse_label_suffix(after_to, line_no)?;
        edges.push(C4Edge {
            from: from.to_string(),
            to: to.to_string(),
            label,
        });
    }

    // Auto-declare edge endpoints without a node line as top-level systems,
    // in first-reference order so layout stays deterministic.
    for edge in &edges {
        for id in [&edge.from, &edge.to] {
            if !nodes.iter().any(|n| &n.id == id) {
                nodes.push(C4Node {
                    id: id.clone(),
                    label: id.clone(),
                    tech: None,
                    kind: C4Kind::System,
                    external: false,
                    boundary: None,
                });
            }
        }
    }

    Ok(DiagramModel::C4 { nodes, boundaries, edges })
}

// ------------------------------------------------------------------
// DSL parsing — requirement
// ------------------------------------------------------------------

/// Parse a `requirement` body (SysML-flavoured requirement diagram).
///
/// DSL (line-oriented, grid layout like `erd`):
/// - requirement: `requirement id: Label`  or  `requirement id: Label: text`
/// - element:     `element id: Label`
/// - edge:        `a -> b: kind` — kind is one of `satisfies`, `verifies`,
///   `refines`, `traces`, `contains`, `derives` (mandatory)
///
/// Requirement labels split at their first inner colon (`Label: text`), so
/// labels themselves must not contain colons. Edge endpoints never declared
/// are auto-declared as elements in first-reference order.
///
/// Example:
/// ```text
/// requirement r1: Fast render: SVG in under 5ms
/// element parser: surf-parse
/// parser -> r1: satisfies
/// ```
fn parse_requirement(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<ReqNode> = Vec::new();
    let mut edges: Vec<ReqEdge> = Vec::new();

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let requirement = if strip_keyword(line, "requirement").is_some() {
            Some(true)
        } else if strip_keyword(line, "element").is_some() {
            Some(false)
        } else {
            None
        };
        if let Some(requirement) = requirement {
            let rest = line.split_once(char::is_whitespace).expect("keyword matched").1;
            let (id, after) = split_leading_id(rest.trim_start());
            if id.is_empty() {
                return Err(err(line_no, "expected node id"));
            }
            let Some(label_part) = after.trim_start().strip_prefix(':') else {
                return Err(err(line_no, format!("expected `: Label` after \"{id}\"")));
            };
            let label_part = label_part.trim();
            // Requirements take an optional `: text` body after the label.
            let (label, text) = if requirement {
                match label_part.split_once(':') {
                    Some((l, t)) => (l.trim().to_string(), Some(t.trim().to_string())),
                    None => (label_part.to_string(), None),
                }
            } else {
                (label_part.to_string(), None)
            };
            let label = if label.is_empty() { id.to_string() } else { label };
            nodes.push(ReqNode {
                id: id.to_string(),
                label,
                text: text.filter(|t| !t.is_empty()),
                requirement,
            });
            continue;
        }

        // Edge line: `a -> b: kind` (kind mandatory).
        let (from, rest) = split_leading_id(line);
        if from.is_empty() {
            return Err(err(line_no, format!("expected node or edge, found \"{line}\"")));
        }
        let Some(after_arrow) = rest.trim_start().strip_prefix("->") else {
            return Err(err(line_no, format!("expected `->` after \"{from}\"")));
        };
        let (to, after_to) = split_leading_id(after_arrow.trim_start());
        if to.is_empty() {
            return Err(err(line_no, "expected target id after arrow"));
        }
        let kind = match parse_label_suffix(after_to, line_no)?.as_deref() {
            Some("satisfies") => ReqEdgeKind::Satisfies,
            Some("verifies") => ReqEdgeKind::Verifies,
            Some("refines") => ReqEdgeKind::Refines,
            Some("traces") => ReqEdgeKind::Traces,
            Some("contains") => ReqEdgeKind::Contains,
            Some("derives") => ReqEdgeKind::Derives,
            Some(other) => {
                return Err(err(
                    line_no,
                    format!("unknown relation \"{other}\" (expected satisfies, verifies, refines, traces, contains or derives)"),
                ));
            }
            None => {
                return Err(err(
                    line_no,
                    "expected a relation kind after `->` edge (e.g. `: satisfies`)",
                ));
            }
        };
        edges.push(ReqEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        });
    }

    // Auto-declare edge endpoints without a node line as elements, in
    // first-reference order so layout stays deterministic.
    for edge in &edges {
        for id in [&edge.from, &edge.to] {
            if !nodes.iter().any(|n| &n.id == id) {
                nodes.push(ReqNode {
                    id: id.clone(),
                    label: id.clone(),
                    text: None,
                    requirement: false,
                });
            }
        }
    }

    Ok(DiagramModel::Requirement { nodes, edges })
}

// ------------------------------------------------------------------
// DSL parsing — sankey
// ------------------------------------------------------------------

/// Parse a `sankey` body.
///
/// DSL (one flow per line, columns computed from the flow graph):
/// - flow: `Source -> Target: value` — value is a positive number; node
///   names are free text (spaces allowed) and are trimmed
///
/// Values are stored ×100 (centi-units, rounded) so all layout stays
/// integer arithmetic. Nodes appear in first-reference order.
///
/// Example:
/// ```text
/// Wind -> Grid: 40
/// Solar -> Grid: 30
/// Grid -> Homes: 50
/// Grid -> Industry: 20
/// ```
fn parse_sankey(content: &str) -> Result<DiagramModel, DiagramError> {
    let mut nodes: Vec<String> = Vec::new();
    let mut flows: Vec<SankeyFlow> = Vec::new();

    let declare = |nodes: &mut Vec<String>, name: &str| {
        if !nodes.iter().any(|n| n == name) {
            nodes.push(name.to_string());
        }
    };

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let Some((from, rest)) = line.split_once("->") else {
            return Err(err(line_no, format!("expected `Source -> Target: value`, found \"{line}\"")));
        };
        let from = from.trim();
        if from.is_empty() {
            return Err(err(line_no, "source name must not be empty"));
        }
        let Some((to, value_s)) = rest.split_once(':') else {
            return Err(err(line_no, "expected `: value` after target"));
        };
        let to = to.trim();
        if to.is_empty() {
            return Err(err(line_no, "target name must not be empty"));
        }
        let value: f64 = value_s
            .trim()
            .parse()
            .map_err(|_| err(line_no, format!("invalid value \"{}\"", value_s.trim())))?;
        if !value.is_finite() || value <= 0.0 {
            return Err(err(line_no, "value must be a positive number"));
        }
        let value_cs = (value * 100.0).round() as i64;
        if value_cs <= 0 {
            return Err(err(line_no, "value must be a positive number"));
        }
        declare(&mut nodes, from);
        declare(&mut nodes, to);
        flows.push(SankeyFlow {
            from: from.to_string(),
            to: to.to_string(),
            value_cs,
        });
    }

    Ok(DiagramModel::Sankey { nodes, flows })
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
/// Two stages: [`build_scene`] computes the typed geometry scene, then
/// [`emit_svg`] serializes it. `title` is the block's `title` attribute;
/// when present it becomes the SVG `<title>` child for accessibility.
pub(crate) fn render_svg(model: &DiagramModel, title: Option<&str>) -> String {
    emit_svg(&build_scene(model), title)
}

/// Build the FFI-facing geometry scene for a diagram body, or `None` when
/// the body fails to parse (callers keep the raw DSL as their fallback).
/// Used by the native renderer so clients draw diagrams from typed shapes
/// instead of re-parsing the DSL.
///
/// The same mermaid translation and chart-alias routing the HTML renderer
/// applies happen here, so native clients get scenes for mermaid bodies and
/// for pie/donut/radar/xychart alias blocks too. `title` mirrors the SVG:
/// chart scenes draw an on-canvas heading, geometry scenes do not.
#[cfg(feature = "native")]
pub(crate) fn native_scene(
    diagram_type: &str,
    content: &str,
    title: Option<&str>,
) -> Option<crate::diagram_scene::NativeDiagramScene> {
    let translated = crate::mermaid_compat::translate(diagram_type, content);
    let (eff_type, eff_content) = match &translated {
        Some(t) => (t.diagram_type, t.content.as_str()),
        None => (diagram_type, content),
    };
    if let Some(chart_type) = chart_alias(eff_type) {
        let data = crate::blocks::parse_chart_data(eff_content)?;
        return Some(crate::chart::build_scene(chart_type, &data, title));
    }
    let model = parse_diagram_source(eff_type, eff_content).ok()?;
    let scene = build_scene(&model);
    Some(crate::diagram_scene::NativeDiagramScene {
        width: scene.w as f64,
        height: scene.h as f64,
        shapes: scene
            .items
            .into_iter()
            .filter_map(|item| match item {
                SvgItem::Shape { shape, .. } => Some(shape),
                _ => None,
            })
            .collect(),
    })
}

// ------------------------------------------------------------------
// Scene assembly
// ------------------------------------------------------------------

/// SVG-only presentation details attached to a scene shape: the CSS class
/// hook, whether the element carries an explicit `fill="none"`, and the
/// exact dash pattern. These stay out of the FFI scene — native clients
/// style from the shape's roles alone.
#[derive(Debug, Clone, Copy, Default)]
struct Chrome {
    class: Option<&'static str>,
    fill_none: bool,
    dash: Option<&'static str>,
}

impl Chrome {
    /// Chrome carrying only a CSS class.
    fn class(class: &'static str) -> Self {
        Chrome {
            class: Some(class),
            ..Default::default()
        }
    }
}

/// One entry of a built scene: a shape (with its SVG chrome) or a piece of
/// SVG-only structure (marker `<defs>`, `<g>` grouping).
#[derive(Debug, Clone)]
enum SvgItem {
    /// The shared arrowhead `<defs>` block.
    ArrowDefs,
    /// The UML marker `<defs>` block (arrow + diamonds + triangle).
    ClassDefs,
    GroupOpen(&'static str),
    GroupClose,
    Shape { shape: NativeShape, chrome: Chrome },
}

/// A fully laid-out diagram: canvas size + ordered paint items. The FFI
/// scene is this minus the SVG-only items and chrome.
struct SceneBuild {
    w: i64,
    h: i64,
    items: Vec<SvgItem>,
}

impl SceneBuild {
    fn new(w: i64, h: i64) -> Self {
        SceneBuild {
            w,
            h,
            items: Vec::new(),
        }
    }
    fn push(&mut self, chrome: Chrome, shape: NativeShape) {
        self.items.push(SvgItem::Shape { shape, chrome });
    }
    fn open_group(&mut self, class: &'static str) {
        self.items.push(SvgItem::GroupOpen(class));
    }
    fn close_group(&mut self) {
        self.items.push(SvgItem::GroupClose);
    }
}

/// Lay a parsed model out into a scene. Pure and deterministic: all
/// coordinates come from integer arithmetic, widened to `f64` at the edge.
fn build_scene(model: &DiagramModel) -> SceneBuild {
    match model {
        DiagramModel::Architecture { nodes, edges } => scene_architecture(nodes, edges),
        DiagramModel::Erd { entities, relations } => scene_erd(entities, relations),
        DiagramModel::Flowchart { nodes, edges } => scene_flowchart(nodes, edges),
        DiagramModel::Sequence { actors, events } => scene_sequence(actors, events),
        DiagramModel::Gantt { tasks, dated } => scene_gantt(tasks, *dated),
        DiagramModel::State { nodes, transitions } => scene_state(nodes, transitions),
        DiagramModel::Mindmap { nodes } => scene_mindmap(nodes),
        DiagramModel::Class { classes, relations } => scene_class(classes, relations),
        DiagramModel::Timeline { events } => scene_timeline(events),
        DiagramModel::Journey { tasks } => scene_journey(tasks),
        DiagramModel::Quadrant { x_axis, y_axis, labels, points } => {
            scene_quadrant(x_axis.as_ref(), y_axis.as_ref(), labels, points)
        }
        DiagramModel::Kanban { columns } => scene_kanban(columns),
        DiagramModel::UseCase { actors, cases, edges } => scene_usecase(actors, cases, edges),
        DiagramModel::GitGraph { branches, commits } => scene_gitgraph(branches, commits),
        DiagramModel::C4 { nodes, boundaries, edges } => scene_c4(nodes, boundaries, edges),
        DiagramModel::Requirement { nodes, edges } => scene_requirement(nodes, edges),
        DiagramModel::Sankey { nodes, flows } => scene_sankey(nodes, flows),
    }
}

/// A scene point from integer layout coordinates.
fn pt(x: i64, y: i64) -> NativePoint {
    NativePoint {
        x: x as f64,
        y: y as f64,
    }
}

/// A rectangle shape from integer layout coordinates (stroke width 1).
fn rect_at(x: i64, y: i64, w: i64, h: i64, corner: i64, fill: NativeRole, stroke: NativeRole) -> NativeShape {
    NativeShape::Rect {
        x: x as f64,
        y: y as f64,
        w: w as f64,
        h: h as f64,
        corner: corner as f64,
        fill,
        stroke,
        stroke_width: 1.0,
    }
}

/// A straight two-point line shape from integer layout coordinates.
#[allow(clippy::too_many_arguments)]
fn line2(
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
    stroke: NativeRole,
    stroke_width: f64,
    dashed: bool,
    marker_start: NativeMarker,
    marker_end: NativeMarker,
) -> NativeShape {
    NativeShape::Line {
        points: vec![pt(x1, y1), pt(x2, y2)],
        stroke,
        stroke_width,
        dashed,
        marker_start,
        marker_end,
    }
}

/// A text label shape from integer layout coordinates (non-mono).
fn text_at(x: i64, y: i64, text: &str, role: NativeRole, size: i64, bold: bool, anchor: NativeAnchor) -> NativeShape {
    NativeShape::Label {
        x: x as f64,
        y: y as f64,
        text: text.to_string(),
        role,
        size: size as f64,
        bold,
        mono: false,
        anchor,
    }
}

/// A circle shape from integer layout coordinates. `fill: None` = hollow,
/// `stroke: None` = no outline.
fn circle_at(cx: i64, cy: i64, r: i64, fill: Option<NativeRole>, stroke: Option<NativeRole>) -> NativeShape {
    NativeShape::Ellipse {
        cx: cx as f64,
        cy: cy as f64,
        rx: r as f64,
        ry: r as f64,
        fill,
        stroke,
    }
}

// ------------------------------------------------------------------
// Scene → SVG serialization
// ------------------------------------------------------------------

/// Reusable arrowhead marker `<defs>` (id `surfdoc-arrow`). One geometry
/// for every diagram kind so all arrows look identical;
/// `orient="auto-start-reverse"` lets the same marker serve both line ends.
const ARROW_DEFS: &str = "<defs><marker id=\"surfdoc-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"#64748b\"/></marker></defs>";

/// UML marker `<defs>` for `class` diagrams: the shared arrowhead plus the
/// composition (filled diamond), aggregation (hollow diamond) and
/// inheritance (hollow triangle) markers.
const CLASS_DEFS: &str = "<defs><marker id=\"surfdoc-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"#64748b\"/></marker><marker id=\"surfdoc-diamond\" viewBox=\"0 0 12 8\" refX=\"1\" refY=\"4\" markerWidth=\"12\" markerHeight=\"8\" orient=\"auto-start-reverse\"><path d=\"M1,4 L6,1 L11,4 L6,7 z\" fill=\"#64748b\"/></marker><marker id=\"surfdoc-diamond-open\" viewBox=\"0 0 12 8\" refX=\"1\" refY=\"4\" markerWidth=\"12\" markerHeight=\"8\" orient=\"auto-start-reverse\"><path d=\"M1,4 L6,1 L11,4 L6,7 z\" fill=\"#ffffff\" stroke=\"#64748b\"/></marker><marker id=\"surfdoc-triangle-open\" viewBox=\"0 0 12 12\" refX=\"11\" refY=\"6\" markerWidth=\"10\" markerHeight=\"10\" orient=\"auto-start-reverse\"><path d=\"M1,1 L11,6 L1,11 z\" fill=\"#ffffff\" stroke=\"#64748b\"/></marker></defs>";

/// Fixed SVG color for a paint role (the reference palette; native clients
/// substitute their own theme tokens for the same roles).
fn role_color(role: NativeRole) -> &'static str {
    match role {
        NativeRole::Surface => "#f8fafc",
        NativeRole::SurfaceAlt => "#e2e8f0",
        NativeRole::Accent => "#2563eb",
        NativeRole::AccentSoft => "#cbd5e1",
        NativeRole::Stroke => "#64748b",
        NativeRole::Muted => "#94a3b8",
        NativeRole::TextPrimary => "currentColor",
        NativeRole::TextSecondary => "#64748b",
        NativeRole::OnAccent => "#ffffff",
    }
}

/// SVG marker element id for a line-end marker kind (`None` unreachable —
/// callers skip absent markers).
fn marker_ref(marker: NativeMarker) -> &'static str {
    match marker {
        NativeMarker::None => "",
        NativeMarker::Arrow => "surfdoc-arrow",
        NativeMarker::Diamond => "surfdoc-diamond",
        NativeMarker::DiamondOpen => "surfdoc-diamond-open",
        NativeMarker::TriangleOpen => "surfdoc-triangle-open",
    }
}

/// Format a scene number for SVG output: whole values print as integers
/// (all layout math is integer, keeping historical byte-stable output),
/// fractional values (stroke widths) print minimally (`1.5`).
fn fnum(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// ` class="…"` attribute for a shape's chrome, or empty.
fn class_attr(chrome: &Chrome) -> String {
    match chrome.class {
        Some(c) => format!(" class=\"{c}\""),
        None => String::new(),
    }
}

/// Serialize a built scene as deterministic inline SVG.
fn emit_svg(scene: &SceneBuild, title: Option<&str>) -> String {
    let mut svg = svg_open(scene.w, scene.h, title);
    for item in &scene.items {
        match item {
            SvgItem::ArrowDefs => svg.push_str(ARROW_DEFS),
            SvgItem::ClassDefs => svg.push_str(CLASS_DEFS),
            SvgItem::GroupOpen(class) => {
                svg.push_str("<g class=\"");
                svg.push_str(class);
                svg.push_str("\">");
            }
            SvgItem::GroupClose => svg.push_str("</g>"),
            SvgItem::Shape { shape, chrome } => emit_shape(&mut svg, shape, chrome),
        }
    }
    svg.push_str("</svg>");
    svg
}

/// Serialize one scene shape. Attribute order and formatting are frozen —
/// consumers pin exact substrings of the output.
fn emit_shape(svg: &mut String, shape: &NativeShape, chrome: &Chrome) {
    match shape {
        NativeShape::Rect {
            x,
            y,
            w,
            h,
            corner,
            fill,
            stroke,
            stroke_width,
        } => {
            let rx = if *corner != 0.0 {
                format!(" rx=\"{}\"", fnum(*corner))
            } else {
                String::new()
            };
            let sw = if *stroke_width != 1.0 {
                format!(" stroke-width=\"{}\"", fnum(*stroke_width))
            } else {
                String::new()
            };
            svg.push_str(&format!(
                "<rect{} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} fill=\"{}\" stroke=\"{}\"{sw}/>",
                class_attr(chrome),
                fnum(*x),
                fnum(*y),
                fnum(*w),
                fnum(*h),
                role_color(*fill),
                role_color(*stroke),
            ));
        }

        NativeShape::Line {
            points,
            stroke,
            stroke_width,
            dashed: _, // SVG uses the chrome's exact dash pattern
            marker_start,
            marker_end,
        } => {
            let mut tail = format!(
                " stroke=\"{}\" stroke-width=\"{}\"",
                role_color(*stroke),
                fnum(*stroke_width),
            );
            if chrome.fill_none {
                tail.push_str(" fill=\"none\"");
            }
            if let Some(dash) = chrome.dash {
                tail.push_str(&format!(" stroke-dasharray=\"{dash}\""));
            }
            if *marker_end != NativeMarker::None {
                tail.push_str(&format!(" marker-end=\"url(#{})\"", marker_ref(*marker_end)));
            }
            if *marker_start != NativeMarker::None {
                tail.push_str(&format!(" marker-start=\"url(#{})\"", marker_ref(*marker_start)));
            }
            if points.len() == 2 {
                svg.push_str(&format!(
                    "<line{} x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{tail}/>",
                    class_attr(chrome),
                    fnum(points[0].x),
                    fnum(points[0].y),
                    fnum(points[1].x),
                    fnum(points[1].y),
                ));
            } else {
                let mut d = String::new();
                for (i, p) in points.iter().enumerate() {
                    if i == 0 {
                        d.push_str(&format!("M{} {}", fnum(p.x), fnum(p.y)));
                    } else {
                        d.push_str(&format!(" L{} {}", fnum(p.x), fnum(p.y)));
                    }
                }
                svg.push_str(&format!("<path{} d=\"{d}\"{tail}/>", class_attr(chrome)));
            }
        }

        NativeShape::Polygon { points, fill, stroke } => {
            let pts = points
                .iter()
                .map(|p| format!("{},{}", fnum(p.x), fnum(p.y)))
                .collect::<Vec<_>>()
                .join(" ");
            svg.push_str(&format!(
                "<polygon{} points=\"{pts}\" fill=\"{}\" stroke=\"{}\"/>",
                class_attr(chrome),
                role_color(*fill),
                role_color(*stroke),
            ));
        }

        NativeShape::Ellipse {
            cx,
            cy,
            rx,
            ry,
            fill,
            stroke,
        } => {
            let fill_attr = match fill {
                Some(role) => format!(" fill=\"{}\"", role_color(*role)),
                None => " fill=\"none\"".to_string(),
            };
            let stroke_attr = match stroke {
                Some(role) => format!(" stroke=\"{}\"", role_color(*role)),
                None => String::new(),
            };
            if rx == ry {
                svg.push_str(&format!(
                    "<circle{} cx=\"{}\" cy=\"{}\" r=\"{}\"{fill_attr}{stroke_attr}/>",
                    class_attr(chrome),
                    fnum(*cx),
                    fnum(*cy),
                    fnum(*rx),
                ));
            } else {
                svg.push_str(&format!(
                    "<ellipse{} cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{fill_attr}{stroke_attr}/>",
                    class_attr(chrome),
                    fnum(*cx),
                    fnum(*cy),
                    fnum(*rx),
                    fnum(*ry),
                ));
            }
        }

        NativeShape::Label {
            x,
            y,
            text,
            role,
            size,
            bold,
            mono,
            anchor,
        } => {
            let anchor_attr = match anchor {
                NativeAnchor::Start => "",
                NativeAnchor::Middle => " text-anchor=\"middle\"",
                NativeAnchor::End => " text-anchor=\"end\"",
            };
            let size_attr = if *size != 13.0 {
                format!(" font-size=\"{}\"", fnum(*size))
            } else {
                String::new()
            };
            let weight_attr = if *bold { " font-weight=\"bold\"" } else { "" };
            let mono_attr = if *mono {
                " font-family=\"ui-monospace, monospace\""
            } else {
                ""
            };
            svg.push_str(&format!(
                "<text{} x=\"{}\" y=\"{}\"{anchor_attr}{size_attr}{weight_attr}{mono_attr} fill=\"{}\">{}</text>",
                class_attr(chrome),
                fnum(*x),
                fnum(*y),
                role_color(*role),
                escape_html(text),
            ));
        }
    }
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

fn scene_architecture(nodes: &[ArchNode], edges: &[ArchEdge]) -> SceneBuild {
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

    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ArrowDefs);

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
            NativeMarker::Arrow
        } else {
            NativeMarker::None
        };
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-edge"),
                fill_none: true,
                dash: None,
            },
            line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, marker_start, NativeMarker::Arrow),
        );

        if let Some(label) = &edge.label {
            sc.push(
                Chrome::class("surfdoc-diagram-edge-label"),
                text_at(
                    (x1 + x2) / 2,
                    (y1 + y2) / 2 - 5,
                    label,
                    NativeRole::TextSecondary,
                    11,
                    false,
                    NativeAnchor::Middle,
                ),
            );
        }
    }

    // Node boxes with centered labels.
    for (i, node) in nodes.iter().enumerate() {
        let r = &rects[i];
        sc.open_group("surfdoc-diagram-node");
        sc.push(
            Chrome::default(),
            rect_at(r.x, r.y, r.w, r.h, 8, NativeRole::Surface, NativeRole::Stroke),
        );
        sc.push(
            Chrome::default(),
            text_at(
                r.cx(),
                r.cy() + 4, // optical baseline centering
                &node.label,
                NativeRole::TextPrimary,
                13,
                false,
                NativeAnchor::Middle,
            ),
        );
        sc.close_group();
    }

    sc
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

/// Place `n` boxes of per-index width/height on a uniform grid, `per_row`
/// per row, with `gap` between cells. Cell size comes from the largest box
/// so positions stay simple and deterministic. Returns the placed rects and
/// the total canvas size. Shared by the `erd` and `class` layouts.
fn grid_layout(sizes: &[(i64, i64)], per_row: usize, gap: i64, min_h: i64) -> (Vec<Rect>, i64, i64) {
    let cell_w = sizes.iter().map(|s| s.0).max().unwrap_or(120) + gap;
    let cell_h = sizes.iter().map(|s| s.1).max().unwrap_or(min_h) + gap;

    let rects: Vec<Rect> = sizes
        .iter()
        .enumerate()
        .map(|(i, &(w, h))| Rect {
            x: MARGIN + (i % per_row) as i64 * cell_w,
            y: MARGIN + (i / per_row) as i64 * cell_h,
            w,
            h,
        })
        .collect();

    let n_cols = sizes.len().min(per_row).max(1);
    let n_rows = sizes.len().div_ceil(per_row).max(1);
    let total_w = MARGIN * 2 + n_cols as i64 * cell_w - gap;
    let total_h = MARGIN * 2 + n_rows as i64 * cell_h - gap;
    (rects, total_w, total_h)
}

/// Connection points on the facing borders of two grid-placed boxes, along
/// the dominant axis. Shared by the `erd` and `class` relation lines.
fn facing_edge_points(a: &Rect, b: &Rect) -> (i64, i64, i64, i64) {
    let (dx, dy) = (b.cx() - a.cx(), b.cy() - a.cy());
    if dx.abs() >= dy.abs() {
        if dx >= 0 {
            (a.x + a.w, a.cy(), b.x, b.cy())
        } else {
            (a.x, a.cy(), b.x + b.w, b.cy())
        }
    } else if dy >= 0 {
        (a.cx(), a.y + a.h, b.cx(), b.y)
    } else {
        (a.cx(), a.y, b.cx(), b.y + b.h)
    }
}

fn scene_erd(entities: &[ErdEntity], relations: &[ErdRelation]) -> SceneBuild {
    // Uniform grid, ERD_PER_ROW entities per row.
    let sizes: Vec<(i64, i64)> = entities
        .iter()
        .map(|e| (entity_width(e), entity_height(e)))
        .collect();
    let (rects, total_w, total_h) = grid_layout(&sizes, ERD_PER_ROW, ERD_GAP, ERD_TITLE_H);

    let mut sc = SceneBuild::new(total_w, total_h);

    // Relation lines first so entity boxes paint over the line ends.
    for rel in relations {
        let (Some(f), Some(t)) = (
            entities.iter().position(|e| e.name == rel.from),
            entities.iter().position(|e| e.name == rel.to),
        ) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let (x1, y1, x2, y2) = facing_edge_points(&rects[f], &rects[t]);

        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-relation"),
                fill_none: true,
                dash: None,
            },
            line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::None),
        );

        // Cardinality glyphs ~1/8 in from each endpoint, nudged off the line.
        sc.push(
            Chrome::class("surfdoc-diagram-card"),
            text_at(
                x1 + (x2 - x1) / 8,
                y1 + (y2 - y1) / 8 - 5,
                rel.from_card.glyph(),
                NativeRole::TextSecondary,
                11,
                false,
                NativeAnchor::Middle,
            ),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-card"),
            text_at(
                x2 - (x2 - x1) / 8,
                y2 - (y2 - y1) / 8 - 5,
                rel.to_card.glyph(),
                NativeRole::TextSecondary,
                11,
                false,
                NativeAnchor::Middle,
            ),
        );

        if let Some(label) = &rel.label {
            sc.push(
                Chrome::class("surfdoc-diagram-relation-label"),
                text_at(
                    (x1 + x2) / 2,
                    (y1 + y2) / 2 - 6,
                    label,
                    NativeRole::TextSecondary,
                    11,
                    false,
                    NativeAnchor::Middle,
                ),
            );
        }
    }

    // Entity tables: outer box, title bar, one row per field with a
    // right-aligned modifier badge.
    for (i, entity) in entities.iter().enumerate() {
        let r = &rects[i];
        sc.open_group("surfdoc-diagram-entity");
        sc.push(
            Chrome::default(),
            rect_at(r.x, r.y, r.w, r.h, 4, NativeRole::OnAccent, NativeRole::Stroke),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-entity-title"),
            rect_at(r.x, r.y, r.w, ERD_TITLE_H, 4, NativeRole::SurfaceAlt, NativeRole::Stroke),
        );
        sc.push(
            Chrome::default(),
            text_at(
                r.cx(),
                r.y + ERD_TITLE_H / 2 + 4,
                &entity.name,
                NativeRole::TextPrimary,
                13,
                true,
                NativeAnchor::Middle,
            ),
        );
        for (row, field) in entity.fields.iter().enumerate() {
            let row_y = r.y + ERD_TITLE_H + row as i64 * ERD_ROW_H + ERD_ROW_H / 2 + 4;
            sc.push(
                Chrome::class("surfdoc-diagram-field"),
                text_at(r.x + 8, row_y, &field.name, NativeRole::TextPrimary, 12, false, NativeAnchor::Start),
            );
            let badges = erd_badges(field);
            if !badges.is_empty() {
                sc.push(
                    Chrome::class("surfdoc-diagram-badge"),
                    text_at(r.x + r.w - 8, row_y, &badges, NativeRole::TextSecondary, 10, false, NativeAnchor::End),
                );
            }
        }
        sc.close_group();
    }

    sc
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

fn scene_flowchart(nodes: &[FlowNode], edges: &[FlowEdge]) -> SceneBuild {
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

    let mut sc = SceneBuild::new(placed.w, placed.h);
    sc.items.push(SvgItem::ArrowDefs);

    for edge in edges {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == edge.from),
            nodes.iter().position(|n| n.id == edge.to),
        ) else {
            continue;
        };
        let (a, b) = (&placed.rects[f], &placed.rects[t]);
        let (mut x1, y1, mut x2, y2) = vert_edge_points(a, b);
        // Opposite edges between the same two nodes trace the same vertical
        // segment; nudge each member of the pair sideways (mirrors state).
        let vertical = x1 == x2;
        let reverse = f != t
            && edges
                .iter()
                .any(|o| o.from == edge.to && o.to == edge.from);
        let dx = if vertical && reverse {
            if f < t {
                -10
            } else {
                10
            }
        } else {
            0
        };
        x1 += dx;
        x2 += dx;
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-edge"),
                fill_none: true,
                dash: None,
            },
            line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::Arrow),
        );
        if let Some(label) = &edge.label {
            // Vertical edges get their label beside the line (a centered
            // label would sit on the stroke); paired edges label outward.
            let (lx, ly, anchor) = if vertical {
                if dx < 0 {
                    (x1 - 6, (y1 + y2) / 2 + 4, NativeAnchor::End)
                } else {
                    (x1 + 6, (y1 + y2) / 2 + 4, NativeAnchor::Start)
                }
            } else {
                ((x1 + x2) / 2, (y1 + y2) / 2 - 5, NativeAnchor::Middle)
            };
            sc.push(
                Chrome::class("surfdoc-diagram-edge-label"),
                text_at(lx, ly, label, NativeRole::TextSecondary, 11, false, anchor),
            );
        }
    }

    for (i, node) in nodes.iter().enumerate() {
        let r = &placed.rects[i];
        sc.open_group("surfdoc-diagram-node");
        let shape = match node.shape {
            FlowShape::Box => rect_at(r.x, r.y, r.w, r.h, 4, NativeRole::Surface, NativeRole::Stroke),
            FlowShape::Rounded => rect_at(r.x, r.y, r.w, r.h, r.h / 2, NativeRole::Surface, NativeRole::Stroke),
            FlowShape::Diamond => NativeShape::Polygon {
                points: vec![
                    pt(r.cx(), r.y),
                    pt(r.x + r.w, r.cy()),
                    pt(r.cx(), r.y + r.h),
                    pt(r.x, r.cy()),
                ],
                fill: NativeRole::Surface,
                stroke: NativeRole::Stroke,
            },
        };
        sc.push(Chrome::default(), shape);
        sc.push(
            Chrome::default(),
            text_at(r.cx(), r.cy() + 4, &node.label, NativeRole::TextPrimary, 13, false, NativeAnchor::Middle),
        );
        sc.close_group();
    }

    sc
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

fn scene_sequence(actors: &[SeqActor], events: &[SeqEvent]) -> SceneBuild {
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

    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ArrowDefs);

    // Lifelines.
    for k in 0..n {
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-lifeline"),
                fill_none: false,
                dash: Some("4 4"),
            },
            line2(cx(k), lifeline_top, cx(k), bottom, NativeRole::AccentSoft, 1.0, true, NativeMarker::None, NativeMarker::None),
        );
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
        sc.push(
            Chrome::class("surfdoc-diagram-activation"),
            rect_at(
                cx(*k) - SEQ_ACT_W / 2,
                *y1,
                SEQ_ACT_W,
                (y2 - y1).max(1),
                0,
                NativeRole::SurfaceAlt,
                NativeRole::Stroke,
            ),
        );
    }

    // Messages.
    for (i, ev) in events.iter().enumerate() {
        if let SeqEvent::Message { from, to, label, dashed } = ev {
            let (Some(a), Some(b)) = (idx(from), idx(to)) else {
                continue;
            };
            let y = ey(i as i64);
            let dash = if *dashed { Some("6 4") } else { None };
            if a == b {
                // Self-message: a small out-and-back loop.
                let x = cx(a);
                sc.push(
                    Chrome {
                        class: Some("surfdoc-diagram-msg"),
                        fill_none: true,
                        dash,
                    },
                    NativeShape::Line {
                        points: vec![pt(x, y), pt(x + 30, y), pt(x + 30, y + 14), pt(x + 4, y + 14)],
                        stroke: NativeRole::Stroke,
                        stroke_width: 1.5,
                        dashed: *dashed,
                        marker_start: NativeMarker::None,
                        marker_end: NativeMarker::Arrow,
                    },
                );
                if let Some(l) = label {
                    sc.push(
                        Chrome::class("surfdoc-diagram-msg-label"),
                        text_at(x + 36, y + 4, l, NativeRole::TextSecondary, 11, false, NativeAnchor::Start),
                    );
                }
            } else {
                let (x1, x2) = (cx(a), cx(b));
                sc.push(
                    Chrome {
                        class: Some("surfdoc-diagram-msg"),
                        fill_none: false,
                        dash,
                    },
                    line2(x1, y, x2, y, NativeRole::Stroke, 1.5, *dashed, NativeMarker::None, NativeMarker::Arrow),
                );
                if let Some(l) = label {
                    sc.push(
                        Chrome::class("surfdoc-diagram-msg-label"),
                        text_at((x1 + x2) / 2, y - 5, l, NativeRole::TextSecondary, 11, false, NativeAnchor::Middle),
                    );
                }
            }
        }
    }

    // Actor header boxes (drawn last so they sit above lifelines).
    for (k, actor) in actors.iter().enumerate() {
        let w = label_width(&actor.label);
        sc.open_group("surfdoc-diagram-actor");
        sc.push(
            Chrome::default(),
            rect_at(cx(k) - w / 2, MARGIN, w, SEQ_ACTOR_H, 4, NativeRole::Surface, NativeRole::Stroke),
        );
        sc.push(
            Chrome::default(),
            text_at(
                cx(k),
                MARGIN + SEQ_ACTOR_H / 2 + 4,
                &actor.label,
                NativeRole::TextPrimary,
                13,
                false,
                NativeAnchor::Middle,
            ),
        );
        sc.close_group();
    }

    sc
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

fn scene_gantt(tasks: &[GanttTask], dated: bool) -> SceneBuild {
    if tasks.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
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
    // Dated tick labels (YYYY-MM-DD, centered on their gridline) are ~60px
    // wide; pad the right edge so the last label never clips at the canvas
    // bound. Numeric ticks are narrow enough for the plain margin.
    let total_w = chart_x + chart_w + MARGIN + if dated { 28 } else { 0 };
    let total_h = bottom + MARGIN;

    let mut sc = SceneBuild::new(total_w, total_h);

    // Axis gridlines + tick labels. Dated labels (YYYY-MM-DD) are ~62px
    // wide, so the tick stride must also guarantee at least 72px between
    // gridlines — otherwise short spans put a colliding label on every line.
    let min_dated = if dated { (72 + unit_w - 1) / unit_w } else { 1 };
    let stride = (span / 8).max(min_dated).max(1);
    let mut tick = t0;
    while tick <= t1 {
        let x = chart_x + (tick - t0) * unit_w;
        sc.push(
            Chrome::class("surfdoc-diagram-grid"),
            line2(x, GANTT_TOP, x, bottom, NativeRole::SurfaceAlt, 1.0, false, NativeMarker::None, NativeMarker::None),
        );
        let label = if dated {
            let (y, m, d) = civil_from_days(tick);
            format!("{y:04}-{m:02}-{d:02}")
        } else {
            tick.to_string()
        };
        sc.push(
            Chrome::class("surfdoc-diagram-tick"),
            text_at(x, GANTT_TOP - 6, &label, NativeRole::TextSecondary, 10, false, NativeAnchor::Middle),
        );
        tick += stride;
    }

    // Rows: section headers + task bars.
    let mut y = GANTT_TOP;
    let mut prev: Option<&str> = None;
    for (i, t) in tasks.iter().enumerate() {
        let sec = t.section.as_deref();
        if i == 0 || sec != prev {
            if let Some(s) = sec {
                sc.push(
                    Chrome::class("surfdoc-diagram-section"),
                    text_at(MARGIN, y + GANTT_ROW_H / 2 + 4, s, NativeRole::TextPrimary, 12, true, NativeAnchor::Start),
                );
                y += GANTT_ROW_H;
            }
            prev = sec;
        }
        sc.push(
            Chrome::class("surfdoc-diagram-task"),
            text_at(MARGIN, y + GANTT_ROW_H / 2 + 4, &t.label, NativeRole::TextPrimary, 12, false, NativeAnchor::Start),
        );
        let bx = chart_x + (t.start - t0) * unit_w;
        let bw = (t.duration * unit_w).max(2);
        sc.push(
            Chrome::class("surfdoc-diagram-bar"),
            rect_at(
                bx,
                y + (GANTT_ROW_H - GANTT_BAR_H) / 2,
                bw,
                GANTT_BAR_H,
                3,
                NativeRole::Muted,
                NativeRole::Stroke,
            ),
        );
        y += GANTT_ROW_H;
    }

    sc
}

// ------------------------------------------------------------------
// SVG rendering — state
// ------------------------------------------------------------------

/// Uniform state node height.
const STATE_NODE_H: i64 = 40;

fn scene_state(nodes: &[StateNode], transitions: &[StateTransition]) -> SceneBuild {
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

    let mut sc = SceneBuild::new(placed.w, placed.h);
    sc.items.push(SvgItem::ArrowDefs);

    for tr in transitions {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == tr.from),
            nodes.iter().position(|n| n.id == tr.to),
        ) else {
            continue;
        };
        let (a, b) = (&placed.rects[f], &placed.rects[t]);
        let (mut x1, y1, mut x2, y2) = vert_edge_points(a, b);
        // An opposite transition between the same two states traces the
        // exact same vertical segment; nudge each member of the pair
        // sideways so both arrows (and both labels) stay distinguishable.
        let vertical = x1 == x2;
        let reverse = f != t
            && transitions
                .iter()
                .any(|o| o.from == tr.to && o.to == tr.from);
        let dx = if vertical && reverse {
            if f < t {
                -10
            } else {
                10
            }
        } else {
            0
        };
        x1 += dx;
        x2 += dx;
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-transition"),
                fill_none: true,
                dash: None,
            },
            line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::Arrow),
        );
        if let Some(label) = &tr.label {
            // Vertical edges get their label beside the line (a centered
            // label would sit on the stroke); paired edges label outward.
            let (lx, ly, anchor) = if vertical {
                if dx < 0 {
                    (x1 - 6, (y1 + y2) / 2 + 4, NativeAnchor::End)
                } else {
                    (x1 + 6, (y1 + y2) / 2 + 4, NativeAnchor::Start)
                }
            } else {
                ((x1 + x2) / 2, (y1 + y2) / 2 - 5, NativeAnchor::Middle)
            };
            sc.push(
                Chrome::class("surfdoc-diagram-transition-label"),
                text_at(lx, ly, label, NativeRole::TextSecondary, 11, false, anchor),
            );
        }
    }

    for (i, node) in nodes.iter().enumerate() {
        let r = &placed.rects[i];
        if node.initial {
            sc.push(
                Chrome::class("surfdoc-diagram-initial"),
                circle_at(r.cx(), r.cy(), 8, Some(NativeRole::Stroke), Some(NativeRole::Stroke)),
            );
        } else if node.final_ {
            sc.open_group("surfdoc-diagram-final");
            sc.push(
                Chrome::default(),
                circle_at(r.cx(), r.cy(), 10, None, Some(NativeRole::Stroke)),
            );
            sc.push(
                Chrome::default(),
                circle_at(r.cx(), r.cy(), 5, Some(NativeRole::Stroke), None),
            );
            sc.close_group();
        } else {
            sc.open_group("surfdoc-diagram-state");
            sc.push(
                Chrome::default(),
                rect_at(r.x, r.y, r.w, r.h, 12, NativeRole::Surface, NativeRole::Stroke),
            );
            sc.push(
                Chrome::default(),
                text_at(r.cx(), r.cy() + 4, &node.label, NativeRole::TextPrimary, 13, false, NativeAnchor::Middle),
            );
            sc.close_group();
        }
    }

    sc
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

fn scene_mindmap(nodes: &[MindNode]) -> SceneBuild {
    if nodes.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
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

    let mut sc = SceneBuild::new(total_w, total_h);

    // Branch connectors (parent right edge → child left edge).
    for i in 0..nodes.len() {
        let pd = nodes[i].depth;
        let px2 = col_x[pd] + col_w[pd];
        let py = ys[i];
        for c in mind_children(nodes, i) {
            let cd = nodes[c].depth;
            sc.push(
                Chrome {
                    class: Some("surfdoc-diagram-branch"),
                    fill_none: true,
                    dash: None,
                },
                line2(px2, py, col_x[cd], ys[c], NativeRole::Muted, 1.5, false, NativeMarker::None, NativeMarker::None),
            );
        }
    }

    // Node boxes; root (depth 0) gets a filled accent.
    for (i, node) in nodes.iter().enumerate() {
        let d = node.depth;
        let w = col_w[d];
        let nx = col_x[d];
        let ny = ys[i] - MIND_NODE_H / 2;
        let fill = if d == 0 { NativeRole::SurfaceAlt } else { NativeRole::Surface };
        sc.open_group("surfdoc-diagram-mind-node");
        sc.push(Chrome::default(), rect_at(nx, ny, w, MIND_NODE_H, 6, fill, NativeRole::Stroke));
        sc.push(
            Chrome::default(),
            text_at(nx + w / 2, ys[i] + 4, &node.label, NativeRole::TextPrimary, 13, false, NativeAnchor::Middle),
        );
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — class
// ------------------------------------------------------------------

/// Extra title-bar height when a class has a stereotype line.
const CLASS_STEREO_H: i64 = 14;

/// Display text for a class member: visibility sigil + name, with `()`
/// re-appended for methods.
fn class_member_text(member: &ClassMember) -> String {
    let vis = member.visibility.map(String::from).unwrap_or_default();
    let parens = if member.method { "()" } else { "" };
    format!("{vis}{}{parens}", member.name)
}

/// Title-bar height for a class box (taller when a stereotype is shown).
fn class_title_h(class: &ClassBox) -> i64 {
    if class.stereotype.is_some() {
        ERD_TITLE_H + CLASS_STEREO_H
    } else {
        ERD_TITLE_H
    }
}

/// Box width for a class: widest of name / `«stereotype»` / member rows,
/// min 120 (same sizing rule as ERD entities).
fn class_width(class: &ClassBox) -> i64 {
    let mut chars = class.name.chars().count();
    if let Some(st) = &class.stereotype {
        chars = chars.max(st.chars().count() + 2); // « »
    }
    for member in class.fields.iter().chain(&class.methods) {
        chars = chars.max(class_member_text(member).chars().count());
    }
    (chars as i64 * CHAR_W + 24).max(120)
}

/// Box height for a class: title bar + one row per field and method.
fn class_height(class: &ClassBox) -> i64 {
    class_title_h(class) + (class.fields.len() + class.methods.len()) as i64 * ERD_ROW_H
}

fn scene_class(classes: &[ClassBox], relations: &[ClassRelation]) -> SceneBuild {
    // Same uniform grid as ERD, reusing its cell gap and row metrics.
    let sizes: Vec<(i64, i64)> = classes
        .iter()
        .map(|c| (class_width(c), class_height(c)))
        .collect();
    let (rects, total_w, total_h) = grid_layout(&sizes, ERD_PER_ROW, ERD_GAP, ERD_TITLE_H);

    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ClassDefs);

    // Relation lines first so class boxes paint over the line ends.
    for rel in relations {
        let (Some(f), Some(t)) = (
            classes.iter().position(|c| c.name == rel.from),
            classes.iter().position(|c| c.name == rel.to),
        ) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let (x1, y1, x2, y2) = facing_edge_points(&rects[f], &rects[t]);

        // UML end markers: composition/aggregation decorate the source (A),
        // association/inheritance decorate the target (B).
        let (marker_start, marker_end) = match rel.kind {
            ClassRelationKind::Association => (NativeMarker::None, NativeMarker::Arrow),
            ClassRelationKind::Composition => (NativeMarker::Diamond, NativeMarker::None),
            ClassRelationKind::Aggregation => (NativeMarker::DiamondOpen, NativeMarker::None),
            ClassRelationKind::Inheritance => (NativeMarker::None, NativeMarker::TriangleOpen),
        };
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-relation"),
                fill_none: true,
                dash: None,
            },
            line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, marker_start, marker_end),
        );

        if let Some(label) = &rel.label {
            sc.push(
                Chrome::class("surfdoc-diagram-relation-label"),
                text_at(
                    (x1 + x2) / 2,
                    (y1 + y2) / 2 - 6,
                    label,
                    NativeRole::TextSecondary,
                    11,
                    false,
                    NativeAnchor::Middle,
                ),
            );
        }
    }

    // Class boxes: outer box, title bar (stereotype + name), field rows,
    // separator, method rows — the classic three compartments.
    for (i, class) in classes.iter().enumerate() {
        let r = &rects[i];
        let title_h = class_title_h(class);
        sc.open_group("surfdoc-diagram-class");
        sc.push(
            Chrome::default(),
            rect_at(r.x, r.y, r.w, r.h, 4, NativeRole::OnAccent, NativeRole::Stroke),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-class-title"),
            rect_at(r.x, r.y, r.w, title_h, 4, NativeRole::SurfaceAlt, NativeRole::Stroke),
        );
        if let Some(st) = &class.stereotype {
            sc.push(
                Chrome::default(),
                text_at(
                    r.cx(),
                    r.y + 16,
                    &format!("\u{ab}{st}\u{bb}"),
                    NativeRole::TextSecondary,
                    10,
                    false,
                    NativeAnchor::Middle,
                ),
            );
            sc.push(
                Chrome::default(),
                text_at(r.cx(), r.y + 33, &class.name, NativeRole::TextPrimary, 13, true, NativeAnchor::Middle),
            );
        } else {
            sc.push(
                Chrome::default(),
                text_at(
                    r.cx(),
                    r.y + ERD_TITLE_H / 2 + 4,
                    &class.name,
                    NativeRole::TextPrimary,
                    13,
                    true,
                    NativeAnchor::Middle,
                ),
            );
        }

        let mut row_y = r.y + title_h;
        for member in &class.fields {
            sc.push(
                Chrome::class("surfdoc-diagram-member"),
                text_at(
                    r.x + 8,
                    row_y + ERD_ROW_H / 2 + 4,
                    &class_member_text(member),
                    NativeRole::TextPrimary,
                    12,
                    false,
                    NativeAnchor::Start,
                ),
            );
            row_y += ERD_ROW_H;
        }
        if !class.methods.is_empty() {
            sc.push(
                Chrome::class("surfdoc-diagram-class-sep"),
                line2(r.x, row_y, r.x + r.w, row_y, NativeRole::Stroke, 1.0, false, NativeMarker::None, NativeMarker::None),
            );
            for member in &class.methods {
                sc.push(
                    Chrome::class("surfdoc-diagram-member"),
                    text_at(
                        r.x + 8,
                        row_y + ERD_ROW_H / 2 + 4,
                        &class_member_text(member),
                        NativeRole::TextPrimary,
                        12,
                        false,
                        NativeAnchor::Start,
                    ),
                );
                row_y += ERD_ROW_H;
            }
        }
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — timeline
// ------------------------------------------------------------------

/// Length of the stem connecting an event dot to its text block.
const TL_STEM: i64 = 24;
/// Vertical space reserved for an event's text block (marker + label rows).
const TL_TEXT_ZONE: i64 = 40;
/// Horizontal gap between event slots.
const TL_GAP: i64 = 12;

fn scene_timeline(events: &[TimelineEvent]) -> SceneBuild {
    if events.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    // Uniform slot width fits the widest label or marker.
    let slot = events
        .iter()
        .map(|e| {
            let lw = label_width(&e.label);
            match &e.marker {
                Some(m) => lw.max(label_width(m)),
                None => lw,
            }
        })
        .max()
        .unwrap_or(80);

    let n = events.len() as i64;
    let mid = MARGIN + TL_TEXT_ZONE + TL_STEM;
    let total_w = MARGIN * 2 + n * slot + (n - 1) * TL_GAP + 24;
    let total_h = mid + TL_STEM + TL_TEXT_ZONE + MARGIN;

    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ArrowDefs);

    // The spine, running left-to-right with an arrowhead.
    sc.push(
        Chrome {
            class: Some("surfdoc-diagram-spine"),
            fill_none: false,
            dash: None,
        },
        line2(MARGIN, mid, total_w - MARGIN, mid, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::Arrow),
    );

    // Events alternate above (even index) / below (odd index) the spine.
    for (i, event) in events.iter().enumerate() {
        let cx = MARGIN + slot / 2 + i as i64 * (slot + TL_GAP);
        let above = i % 2 == 0;
        let tip = if above { mid - TL_STEM } else { mid + TL_STEM };

        sc.open_group("surfdoc-diagram-event");
        sc.push(
            Chrome::class("surfdoc-diagram-stem"),
            line2(cx, mid, cx, tip, NativeRole::AccentSoft, 1.0, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::default(),
            circle_at(cx, mid, 5, Some(NativeRole::Accent), None),
        );

        // Text rows: the row nearest the spine holds the marker when there
        // is one (dates hug the axis), the label sits one row further out.
        let (near_y, far_y) = if above {
            (tip - 6, tip - 22)
        } else {
            (tip + 14, tip + 30)
        };
        match &event.marker {
            Some(marker) => {
                sc.push(
                    Chrome::class("surfdoc-diagram-event-marker"),
                    text_at(cx, near_y, marker, NativeRole::TextSecondary, 11, true, NativeAnchor::Middle),
                );
                sc.push(
                    Chrome::default(),
                    text_at(cx, far_y, &event.label, NativeRole::TextPrimary, 12, false, NativeAnchor::Middle),
                );
            }
            None => {
                sc.push(
                    Chrome::default(),
                    text_at(cx, near_y, &event.label, NativeRole::TextPrimary, 12, false, NativeAnchor::Middle),
                );
            }
        }
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — journey
// ------------------------------------------------------------------

/// Height of one score row of the journey band (scores 1..=5).
const JR_ROW_H: i64 = 22;
/// Horizontal gap between task columns.
const JR_COL_GAP: i64 = 8;
/// Vertical space above the band for section (lane) headers.
const JR_HEADER_ZONE: i64 = 18;

fn scene_journey(tasks: &[JourneyTask]) -> SceneBuild {
    if tasks.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    let band_top = MARGIN + JR_HEADER_ZONE;
    let band_h = 5 * JR_ROW_H;
    let band_bottom = band_top + band_h;
    let chart_x = MARGIN + 16;

    // Column x positions (per-task widths, declaration order).
    let widths: Vec<i64> = tasks.iter().map(|t| label_width(&t.label)).collect();
    let mut xs = Vec::with_capacity(tasks.len());
    let mut x = chart_x;
    for w in &widths {
        xs.push(x);
        x += w + JR_COL_GAP;
    }
    let chart_end = x - JR_COL_GAP;

    let total_w = chart_end + MARGIN;
    let total_h = band_bottom + 24 + MARGIN;
    let mut sc = SceneBuild::new(total_w, total_h);

    // Lanes: consecutive tasks sharing a section form one lane; fills
    // alternate so adjacent lanes read apart.
    let mut lane_start = 0usize;
    let mut lane_no = 0usize;
    for i in 0..=tasks.len() {
        let boundary = i == tasks.len() || (i > 0 && tasks[i].section != tasks[i - 1].section);
        if !boundary {
            continue;
        }
        let (first, last) = (lane_start, i - 1);
        let lx = xs[first];
        let lw = xs[last] + widths[last] - lx;
        let fill = if lane_no % 2 == 0 { NativeRole::Surface } else { NativeRole::OnAccent };
        sc.push(
            Chrome::class("surfdoc-diagram-lane"),
            rect_at(lx, band_top, lw, band_h, 0, fill, fill),
        );
        if let Some(name) = tasks[first].section.as_deref() {
            sc.push(
                Chrome::class("surfdoc-diagram-section"),
                text_at(lx, band_top - 6, name, NativeRole::TextPrimary, 12, true, NativeAnchor::Start),
            );
        }
        lane_start = i;
        lane_no += 1;
    }

    // Score gridlines + axis ticks (5 at the top down to 1 at the bottom).
    for score in 1..=5i64 {
        let row_y = band_top + (5 - score) * JR_ROW_H + JR_ROW_H / 2;
        sc.push(
            Chrome::class("surfdoc-diagram-grid"),
            line2(chart_x, row_y, chart_end, row_y, NativeRole::SurfaceAlt, 1.0, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-tick"),
            text_at(chart_x - 6, row_y + 4, &score.to_string(), NativeRole::TextSecondary, 10, false, NativeAnchor::End),
        );
    }

    // Dot centers, one per task.
    let dot = |i: usize| -> (i64, i64) {
        let cx = xs[i] + widths[i] / 2;
        let cy = band_top + (5 - tasks[i].score) * JR_ROW_H + JR_ROW_H / 2;
        (cx, cy)
    };

    // A connecting line makes the mood curve readable at a glance.
    if tasks.len() > 1 {
        let points: Vec<NativePoint> = (0..tasks.len())
            .map(|i| {
                let (cx, cy) = dot(i);
                pt(cx, cy)
            })
            .collect();
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-journey-line"),
                fill_none: true,
                dash: None,
            },
            NativeShape::Line {
                points,
                stroke: NativeRole::Muted,
                stroke_width: 1.5,
                dashed: false,
                marker_start: NativeMarker::None,
                marker_end: NativeMarker::None,
            },
        );
    }

    // Score dots + task labels beneath the band.
    for i in 0..tasks.len() {
        let (cx, cy) = dot(i);
        sc.open_group("surfdoc-diagram-task");
        sc.push(Chrome::default(), circle_at(cx, cy, 6, Some(NativeRole::Accent), None));
        sc.push(
            Chrome::default(),
            text_at(cx, band_bottom + 16, &tasks[i].label, NativeRole::TextPrimary, 11, false, NativeAnchor::Middle),
        );
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — quadrant
// ------------------------------------------------------------------

/// Side length of the square quadrant chart area.
const QC_SIZE: i64 = 360;

fn scene_quadrant(
    x_axis: Option<&(String, String)>,
    y_axis: Option<&(String, String)>,
    labels: &[Option<String>],
    points: &[QuadPoint],
) -> SceneBuild {
    // Left gutter fits the y-axis end labels (drawn beside the frame).
    let gutter = y_axis
        .map(|(low, high)| {
            let chars = low.chars().count().max(high.chars().count()) as i64;
            chars * 7 + 12
        })
        .unwrap_or(16);

    let chart_x = MARGIN + gutter;
    let chart_y = MARGIN;
    let total_w = chart_x + QC_SIZE + MARGIN;
    let total_h = chart_y + QC_SIZE + 24 + MARGIN;

    let mut sc = SceneBuild::new(total_w, total_h);

    // Frame + center gridlines.
    sc.push(
        Chrome::class("surfdoc-diagram-frame"),
        rect_at(chart_x, chart_y, QC_SIZE, QC_SIZE, 0, NativeRole::Surface, NativeRole::Stroke),
    );
    sc.push(
        Chrome::class("surfdoc-diagram-grid"),
        line2(chart_x + QC_SIZE / 2, chart_y, chart_x + QC_SIZE / 2, chart_y + QC_SIZE, NativeRole::AccentSoft, 1.0, false, NativeMarker::None, NativeMarker::None),
    );
    sc.push(
        Chrome::class("surfdoc-diagram-grid"),
        line2(chart_x, chart_y + QC_SIZE / 2, chart_x + QC_SIZE, chart_y + QC_SIZE / 2, NativeRole::AccentSoft, 1.0, false, NativeMarker::None, NativeMarker::None),
    );

    // Quadrant labels: 1 = top-right, 2 = top-left, 3 = bottom-left,
    // 4 = bottom-right, near the top of each cell.
    let quad_pos = [
        (chart_x + QC_SIZE * 3 / 4, chart_y + 20),
        (chart_x + QC_SIZE / 4, chart_y + 20),
        (chart_x + QC_SIZE / 4, chart_y + QC_SIZE / 2 + 20),
        (chart_x + QC_SIZE * 3 / 4, chart_y + QC_SIZE / 2 + 20),
    ];
    for (label, &(qx, qy)) in labels.iter().zip(&quad_pos) {
        if let Some(label) = label {
            sc.push(
                Chrome::class("surfdoc-diagram-quadrant-label"),
                text_at(qx, qy, label, NativeRole::TextSecondary, 11, false, NativeAnchor::Middle),
            );
        }
    }

    // Axis end labels: x along the bottom, y beside the left edge.
    if let Some((low, high)) = x_axis {
        sc.push(
            Chrome::class("surfdoc-diagram-axis-label"),
            text_at(chart_x, chart_y + QC_SIZE + 16, low, NativeRole::TextSecondary, 11, false, NativeAnchor::Start),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-axis-label"),
            text_at(chart_x + QC_SIZE, chart_y + QC_SIZE + 16, high, NativeRole::TextSecondary, 11, false, NativeAnchor::End),
        );
    }
    if let Some((low, high)) = y_axis {
        sc.push(
            Chrome::class("surfdoc-diagram-axis-label"),
            text_at(chart_x - 6, chart_y + QC_SIZE - 4, low, NativeRole::TextSecondary, 11, false, NativeAnchor::End),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-axis-label"),
            text_at(chart_x - 6, chart_y + 12, high, NativeRole::TextSecondary, 11, false, NativeAnchor::End),
        );
    }

    // Labeled points (per-mille coordinates; y grows upward in the DSL).
    for point in points {
        let px = chart_x + point.x_mil * QC_SIZE / 1000;
        let py = chart_y + QC_SIZE - point.y_mil * QC_SIZE / 1000;
        // Labels sit above their point, except near the top of a cell where
        // they would collide with the quadrant label strip (or the frame
        // edge) — those flip below the point.
        let cell_top = if point.y_mil >= 500 { chart_y } else { chart_y + QC_SIZE / 2 };
        let ly = if py - cell_top < 44 { py + 18 } else { py - 9 };
        sc.open_group("surfdoc-diagram-point");
        sc.push(Chrome::default(), circle_at(px, py, 5, Some(NativeRole::Accent), None));
        sc.push(
            Chrome::default(),
            text_at(px, ly, &point.label, NativeRole::TextPrimary, 11, false, NativeAnchor::Middle),
        );
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — kanban
// ------------------------------------------------------------------

/// Column header height.
const KB_HEADER_H: i64 = 30;
/// Card height.
const KB_CARD_H: i64 = 26;
/// Vertical gap between cards (and around the card stack).
const KB_CARD_GAP: i64 = 8;
/// Horizontal gap between columns.
const KB_COL_GAP: i64 = 16;
/// Cards shown per column before the overflow note takes over.
const KB_MAX_CARDS: usize = 8;

fn scene_kanban(columns: &[KanbanColumn]) -> SceneBuild {
    if columns.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    // Uniform column height fits the tallest visible card stack.
    let rows = |col: &KanbanColumn| -> i64 {
        let shown = col.cards.len().min(KB_MAX_CARDS) as i64;
        shown + i64::from(col.cards.len() > KB_MAX_CARDS)
    };
    let max_rows = columns.iter().map(rows).max().unwrap_or(0).max(1);
    let col_h = KB_HEADER_H + max_rows * (KB_CARD_H + KB_CARD_GAP) + KB_CARD_GAP;

    let widths: Vec<i64> = columns
        .iter()
        .map(|col| {
            let mut w = label_width(&col.name);
            for card in &col.cards {
                w = w.max(label_width(card) + 16);
            }
            w.max(140)
        })
        .collect();

    let total_w = MARGIN * 2 + widths.iter().sum::<i64>() + KB_COL_GAP * (columns.len() as i64 - 1);
    let total_h = MARGIN * 2 + col_h;
    let mut sc = SceneBuild::new(total_w, total_h);

    let mut x = MARGIN;
    for (col, &w) in columns.iter().zip(&widths) {
        sc.open_group("surfdoc-diagram-column");
        sc.push(
            Chrome::default(),
            rect_at(x, MARGIN, w, col_h, 6, NativeRole::Surface, NativeRole::Stroke),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-column-title"),
            rect_at(x, MARGIN, w, KB_HEADER_H, 6, NativeRole::SurfaceAlt, NativeRole::Stroke),
        );
        sc.push(
            Chrome::default(),
            text_at(x + w / 2, MARGIN + KB_HEADER_H / 2 + 4, &col.name, NativeRole::TextPrimary, 13, true, NativeAnchor::Middle),
        );

        let shown = col.cards.len().min(KB_MAX_CARDS);
        for (j, card) in col.cards.iter().take(shown).enumerate() {
            let card_y = MARGIN + KB_HEADER_H + KB_CARD_GAP + j as i64 * (KB_CARD_H + KB_CARD_GAP);
            sc.push(
                Chrome::class("surfdoc-diagram-card"),
                rect_at(x + 8, card_y, w - 16, KB_CARD_H, 4, NativeRole::OnAccent, NativeRole::Stroke),
            );
            sc.push(
                Chrome::default(),
                text_at(x + 16, card_y + KB_CARD_H / 2 + 4, card, NativeRole::TextPrimary, 12, false, NativeAnchor::Start),
            );
        }
        if col.cards.len() > KB_MAX_CARDS {
            let note_y = MARGIN + KB_HEADER_H + KB_CARD_GAP + shown as i64 * (KB_CARD_H + KB_CARD_GAP) + 12;
            sc.push(
                Chrome::class("surfdoc-diagram-more"),
                text_at(
                    x + w / 2,
                    note_y,
                    &format!("+{} more", col.cards.len() - KB_MAX_CARDS),
                    NativeRole::TextSecondary,
                    11,
                    false,
                    NativeAnchor::Middle,
                ),
            );
        }
        sc.close_group();
        x += w + KB_COL_GAP;
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — usecase
// ------------------------------------------------------------------

/// Stick-figure height (head + body + legs), excluding the name label.
const UC_FIGURE_H: i64 = 56;
/// Vertical slot per actor (figure + name + gap).
const UC_ACTOR_SLOT: i64 = 96;
/// Use-case ellipse vertical radius.
const UC_ELLIPSE_RY: i64 = 22;
/// Vertical slot per use case inside the boundary.
const UC_CASE_SLOT: i64 = 2 * UC_ELLIPSE_RY + 20;

fn scene_usecase(actors: &[UcActor], cases: &[UcCase], edges: &[UcEdge]) -> SceneBuild {
    if actors.is_empty() && cases.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    // Actor column on the left, sized by the widest actor name.
    let actor_col_w = actors.iter().map(|a| label_width(&a.label)).max().unwrap_or(80);
    let ax = MARGIN + actor_col_w / 2;
    let actor_top = |k: usize| MARGIN + k as i64 * UC_ACTOR_SLOT;

    // System boundary on the right, ellipses stacked inside.
    let ellipse_rx = |c: &UcCase| label_width(&c.label) / 2 + 8;
    let max_rx = cases.iter().map(ellipse_rx).max().unwrap_or(40);
    let boundary_x = if actors.is_empty() { MARGIN } else { MARGIN + actor_col_w + 50 };
    let boundary_w = 2 * max_rx + 48;
    let boundary_h = cases.len() as i64 * UC_CASE_SLOT + 20;
    let case_cx = boundary_x + boundary_w / 2;
    let case_cy = |j: usize| MARGIN + 20 + UC_ELLIPSE_RY + j as i64 * UC_CASE_SLOT;

    // Bounding boxes for edge routing (actors and ellipses alike), so
    // `facing_edge_points` picks sensible connection points for any pair.
    let bbox = |id: &str| -> Option<Rect> {
        if let Some(k) = actors.iter().position(|a| a.id == id) {
            let top = actor_top(k);
            return Some(Rect { x: ax - 12, y: top, w: 24, h: UC_FIGURE_H });
        }
        let j = cases.iter().position(|c| c.id == id)?;
        let rx = ellipse_rx(&cases[j]);
        Some(Rect {
            x: case_cx - rx,
            y: case_cy(j) - UC_ELLIPSE_RY,
            w: 2 * rx,
            h: 2 * UC_ELLIPSE_RY,
        })
    };

    let actors_bottom = MARGIN + actors.len() as i64 * UC_ACTOR_SLOT;
    let boundary_bottom = MARGIN + boundary_h;
    let total_w = if cases.is_empty() {
        MARGIN + actor_col_w + MARGIN
    } else {
        boundary_x + boundary_w + MARGIN
    };
    let total_h = actors_bottom.max(boundary_bottom) + MARGIN;

    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ArrowDefs);

    // System boundary behind everything else.
    if !cases.is_empty() {
        sc.push(
            Chrome::class("surfdoc-diagram-boundary"),
            rect_at(boundary_x, MARGIN, boundary_w, boundary_h, 0, NativeRole::OnAccent, NativeRole::Stroke),
        );
    }

    // Edges next, so figures and ellipses paint over the line ends.
    for edge in edges {
        let (Some(a), Some(b)) = (bbox(&edge.from), bbox(&edge.to)) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let (x1, y1, x2, y2) = facing_edge_points(&a, &b);
        match edge.kind {
            UcEdgeKind::Association => {
                sc.push(
                    Chrome {
                        class: Some("surfdoc-diagram-assoc"),
                        fill_none: true,
                        dash: None,
                    },
                    line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::None),
                );
            }
            UcEdgeKind::Include | UcEdgeKind::Extend => {
                sc.push(
                    Chrome {
                        class: Some("surfdoc-diagram-uc-rel"),
                        fill_none: true,
                        dash: Some("6 4"),
                    },
                    line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, true, NativeMarker::None, NativeMarker::Arrow),
                );
                let word = if edge.kind == UcEdgeKind::Include { "include" } else { "extend" };
                sc.push(
                    Chrome::class("surfdoc-diagram-uc-rel-label"),
                    text_at(
                        (x1 + x2) / 2 + 8,
                        (y1 + y2) / 2 + 4,
                        &format!("\u{ab}{word}\u{bb}"),
                        NativeRole::TextSecondary,
                        11,
                        false,
                        NativeAnchor::Start,
                    ),
                );
            }
        }
    }

    // Use-case ellipses with centered labels.
    for (j, case) in cases.iter().enumerate() {
        let cy = case_cy(j);
        let rx = ellipse_rx(case);
        sc.open_group("surfdoc-diagram-usecase");
        sc.push(
            Chrome::default(),
            NativeShape::Ellipse {
                cx: case_cx as f64,
                cy: cy as f64,
                rx: rx as f64,
                ry: UC_ELLIPSE_RY as f64,
                fill: Some(NativeRole::Surface),
                stroke: Some(NativeRole::Stroke),
            },
        );
        sc.push(
            Chrome::default(),
            text_at(case_cx, cy + 4, &case.label, NativeRole::TextPrimary, 13, false, NativeAnchor::Middle),
        );
        sc.close_group();
    }

    // Actor stick figures (head, body, arms, legs) with the name beneath.
    for (k, actor) in actors.iter().enumerate() {
        let top = actor_top(k);
        sc.open_group("surfdoc-diagram-actor-figure");
        sc.push(Chrome::default(), circle_at(ax, top + 8, 8, None, Some(NativeRole::Stroke)));
        sc.push(
            Chrome::default(),
            line2(ax, top + 16, ax, top + 38, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::default(),
            line2(ax - 12, top + 22, ax + 12, top + 22, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::default(),
            line2(ax, top + 38, ax - 10, top + UC_FIGURE_H, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::default(),
            line2(ax, top + 38, ax + 10, top + UC_FIGURE_H, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::default(),
            text_at(ax, top + UC_FIGURE_H + 16, &actor.label, NativeRole::TextPrimary, 12, false, NativeAnchor::Middle),
        );
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — gitgraph
// ------------------------------------------------------------------

/// Vertical spacing between branch lanes.
const GG_LANE_GAP: i64 = 44;
/// Horizontal spacing between consecutive commits.
const GG_STEP: i64 = 44;
/// Commit dot radius.
const GG_DOT_R: i64 = 6;

/// Dot fill role for a branch lane. Lane 0 (`main`) is the accent; further
/// lanes cycle a fixed role sequence so adjacent lanes read apart.
fn gitgraph_lane_role(lane: usize) -> NativeRole {
    const ROLES: [NativeRole; 4] = [
        NativeRole::Accent,
        NativeRole::Stroke,
        NativeRole::Muted,
        NativeRole::AccentSoft,
    ];
    ROLES[lane % ROLES.len()]
}

fn scene_gitgraph(branches: &[String], commits: &[GitCommit]) -> SceneBuild {
    if commits.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    // Left gutter fits the widest branch name.
    let name_col = branches
        .iter()
        .map(|b| b.chars().count() as i64 * CHAR_W)
        .max()
        .unwrap_or(32)
        .max(32);
    let chart_x = MARGIN + name_col + 16;
    let lane_y = |lane: usize| MARGIN + 10 + lane as i64 * GG_LANE_GAP;
    let commit_x = |j: usize| chart_x + j as i64 * GG_STEP + GG_STEP / 2;

    let n = commits.len() as i64;
    let total_w = chart_x + n * GG_STEP + MARGIN;
    // +12: commit labels alternate between two rows below the dot (labels
    // wider than GG_STEP would otherwise run together), so the bottom lane
    // needs room for the lower label row.
    let total_h = MARGIN * 2 + 10 + branches.len() as i64 * GG_LANE_GAP + 12;

    let mut sc = SceneBuild::new(total_w, total_h);

    // Lane lines + branch names.
    for (lane, name) in branches.iter().enumerate() {
        let y = lane_y(lane);
        sc.push(
            Chrome::class("surfdoc-diagram-lane"),
            line2(chart_x, y, chart_x + n * GG_STEP, y, NativeRole::SurfaceAlt, 1.5, false, NativeMarker::None, NativeMarker::None),
        );
        sc.push(
            Chrome::class("surfdoc-diagram-branch-label"),
            text_at(MARGIN, y + 4, name, NativeRole::TextPrimary, 12, true, NativeAnchor::Start),
        );
    }

    // Merge connectors beneath the dots.
    for (j, commit) in commits.iter().enumerate() {
        if let Some(src) = commit.merge_from {
            sc.push(
                Chrome {
                    class: Some("surfdoc-diagram-merge"),
                    fill_none: true,
                    dash: None,
                },
                line2(
                    commit_x(src),
                    lane_y(commits[src].branch),
                    commit_x(j),
                    lane_y(commit.branch),
                    NativeRole::Stroke,
                    1.5,
                    false,
                    NativeMarker::None,
                    NativeMarker::None,
                ),
            );
        }
    }

    // Commit dots + labels.
    for (j, commit) in commits.iter().enumerate() {
        let (cx, cy) = (commit_x(j), lane_y(commit.branch));
        sc.open_group("surfdoc-diagram-commit");
        sc.push(
            Chrome::default(),
            circle_at(cx, cy, GG_DOT_R, Some(gitgraph_lane_role(commit.branch)), Some(NativeRole::Stroke)),
        );
        if let Some(label) = &commit.label {
            // Alternate labels between two rows: adjacent labels wider than
            // GG_STEP would collide on a single baseline.
            let dy = if j % 2 == 0 { 14 } else { 26 };
            sc.push(
                Chrome::class("surfdoc-diagram-commit-label"),
                text_at(cx, cy + GG_DOT_R + dy, label, NativeRole::TextSecondary, 10, false, NativeAnchor::Middle),
            );
        }
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — c4
// ------------------------------------------------------------------

/// Uniform C4 node heights per kind (person is taller for the head circle;
/// containers with a tech line get an extra text row).
const C4_NODE_H: i64 = 44;
const C4_PERSON_H: i64 = 60;
const C4_TECH_H: i64 = 16;
/// Vertical gap between stacked nodes in a cluster.
const C4_VGAP: i64 = 22;
/// Horizontal gap between clusters.
const C4_CLUSTER_GAP: i64 = 56;
/// Boundary padding: sides / bottom, and top (room for the name).
const C4_PAD: i64 = 14;
const C4_PAD_TOP: i64 = 30;

/// Height of one C4 node box.
fn c4_node_height(node: &C4Node) -> i64 {
    match node.kind {
        C4Kind::Person => C4_PERSON_H,
        C4Kind::Container if node.tech.is_some() => C4_NODE_H + C4_TECH_H,
        _ => C4_NODE_H,
    }
}

/// Width of one C4 node box (fits label and tech annotation).
fn c4_node_width(node: &C4Node) -> i64 {
    let mut w = label_width(&node.label);
    if let Some(tech) = &node.tech {
        w = w.max(tech.chars().count() as i64 * 7 + 32);
    }
    w
}

fn scene_c4(nodes: &[C4Node], boundaries: &[String], edges: &[C4Edge]) -> SceneBuild {
    if nodes.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    // Clusters, left-to-right: each boundary is one cluster; consecutive
    // top-level nodes (declaration order) form an unnamed cluster between
    // them. Nodes stack vertically within a cluster.
    struct Cluster {
        boundary: Option<usize>,
        members: Vec<usize>,
    }
    let mut clusters: Vec<Cluster> = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let matches_last = clusters
            .last()
            .is_some_and(|c| c.boundary == node.boundary);
        if matches_last {
            clusters.last_mut().expect("checked above").members.push(i);
        } else {
            clusters.push(Cluster {
                boundary: node.boundary,
                members: vec![i],
            });
        }
    }

    // Place nodes: per cluster, a uniform-width vertical stack.
    let mut rects: Vec<Rect> = vec![Rect { x: 0, y: 0, w: 0, h: 0 }; nodes.len()];
    let mut boundary_rects: Vec<(usize, Rect)> = Vec::new();
    let mut x = MARGIN;
    let mut max_bottom = MARGIN;
    for cluster in &clusters {
        let bounded = cluster.boundary.is_some();
        let col_w = cluster
            .members
            .iter()
            .map(|&i| c4_node_width(&nodes[i]))
            .max()
            .unwrap_or(80);
        let (node_x, mut y) = if bounded {
            (x + C4_PAD, MARGIN + C4_PAD_TOP)
        } else {
            (x, MARGIN)
        };
        for &i in &cluster.members {
            let h = c4_node_height(&nodes[i]);
            rects[i] = Rect { x: node_x, y, w: col_w, h };
            y += h + C4_VGAP;
        }
        let stack_bottom = y - C4_VGAP;
        if let Some(b) = cluster.boundary {
            let rect = Rect {
                x,
                y: MARGIN,
                w: col_w + 2 * C4_PAD,
                h: stack_bottom + C4_PAD - MARGIN,
            };
            boundary_rects.push((b, rect));
            x += rect.w + C4_CLUSTER_GAP;
            max_bottom = max_bottom.max(rect.y + rect.h);
        } else {
            x += col_w + C4_CLUSTER_GAP;
            max_bottom = max_bottom.max(stack_bottom);
        }
    }

    let total_w = x - C4_CLUSTER_GAP + MARGIN;
    let total_h = max_bottom + MARGIN;
    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ArrowDefs);

    // Boundaries first: dashed closed outlines with the name at top-left.
    // Drawn as polylines (not rects) so the dash survives the FFI scene.
    for (b, r) in &boundary_rects {
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-boundary"),
                fill_none: true,
                dash: Some("6 4"),
            },
            NativeShape::Line {
                points: vec![
                    pt(r.x, r.y),
                    pt(r.x + r.w, r.y),
                    pt(r.x + r.w, r.y + r.h),
                    pt(r.x, r.y + r.h),
                    pt(r.x, r.y),
                ],
                stroke: NativeRole::Stroke,
                stroke_width: 1.0,
                dashed: true,
                marker_start: NativeMarker::None,
                marker_end: NativeMarker::None,
            },
        );
        sc.push(
            Chrome::class("surfdoc-diagram-boundary-label"),
            text_at(r.x + 10, r.y + 18, &boundaries[*b], NativeRole::TextSecondary, 11, true, NativeAnchor::Start),
        );
    }

    // Edges next so node boxes paint over the line ends.
    for edge in edges {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == edge.from),
            nodes.iter().position(|n| n.id == edge.to),
        ) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let (x1, y1, x2, y2) = facing_edge_points(&rects[f], &rects[t]);
        sc.push(
            Chrome {
                class: Some("surfdoc-diagram-edge"),
                fill_none: true,
                dash: None,
            },
            line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, false, NativeMarker::None, NativeMarker::Arrow),
        );
        if let Some(label) = &edge.label {
            // Stacked nodes produce short vertical edges; a centered label
            // would sit on the stroke and the box border — set it beside
            // the line instead.
            let (lx, ly, anchor) = if x1 == x2 {
                (x1 + 8, (y1 + y2) / 2 + 4, NativeAnchor::Start)
            } else {
                ((x1 + x2) / 2, (y1 + y2) / 2 - 5, NativeAnchor::Middle)
            };
            sc.push(
                Chrome::class("surfdoc-diagram-edge-label"),
                text_at(lx, ly, label, NativeRole::TextSecondary, 11, false, anchor),
            );
        }
    }

    // Node boxes. `[ext]` nodes are muted (alt fill, secondary text).
    for (i, node) in nodes.iter().enumerate() {
        let r = &rects[i];
        let fill = if node.external { NativeRole::SurfaceAlt } else { NativeRole::Surface };
        let text_role = if node.external { NativeRole::TextSecondary } else { NativeRole::TextPrimary };
        sc.open_group("surfdoc-diagram-node");
        match node.kind {
            C4Kind::Person => {
                // Rounded body below a head circle.
                sc.push(
                    Chrome::default(),
                    rect_at(r.x, r.y + 16, r.w, r.h - 16, 8, fill, NativeRole::Stroke),
                );
                sc.push(
                    Chrome::default(),
                    circle_at(r.cx(), r.y + 10, 9, Some(fill), Some(NativeRole::Stroke)),
                );
                sc.push(
                    Chrome::default(),
                    text_at(r.cx(), r.y + 16 + (r.h - 16) / 2 + 8, &node.label, text_role, 13, false, NativeAnchor::Middle),
                );
            }
            C4Kind::System | C4Kind::Container => {
                sc.push(
                    Chrome::default(),
                    rect_at(r.x, r.y, r.w, r.h, 4, fill, NativeRole::Stroke),
                );
                match &node.tech {
                    Some(tech) => {
                        sc.push(
                            Chrome::default(),
                            text_at(r.cx(), r.y + C4_NODE_H / 2 + 2, &node.label, text_role, 13, false, NativeAnchor::Middle),
                        );
                        sc.push(
                            Chrome::class("surfdoc-diagram-tech"),
                            text_at(r.cx(), r.y + C4_NODE_H / 2 + 18, &format!("[{tech}]"), NativeRole::TextSecondary, 10, false, NativeAnchor::Middle),
                        );
                    }
                    None => {
                        sc.push(
                            Chrome::default(),
                            text_at(r.cx(), r.cy() + 4, &node.label, text_role, 13, false, NativeAnchor::Middle),
                        );
                    }
                }
            }
        }
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — requirement
// ------------------------------------------------------------------

/// Title zone of a requirement/element box (stereotype line + name line).
const REQ_TITLE_H: i64 = ERD_TITLE_H + CLASS_STEREO_H;

/// Stereotype text for a requirement node (`«requirement»`/`«element»`).
fn req_stereotype(node: &ReqNode) -> &'static str {
    if node.requirement {
        "\u{ab}requirement\u{bb}"
    } else {
        "\u{ab}element\u{bb}"
    }
}

/// Box width for a requirement node: widest of stereotype / label / text.
fn req_width(node: &ReqNode) -> i64 {
    let mut chars = node.label.chars().count().max(13); // «requirement»
    if let Some(text) = &node.text {
        chars = chars.max(text.chars().count());
    }
    (chars as i64 * CHAR_W + 24).max(140)
}

/// Box height for a requirement node: title zone + one row of body text.
fn req_height(node: &ReqNode) -> i64 {
    REQ_TITLE_H + if node.text.is_some() { ERD_ROW_H } else { 0 }
}

fn scene_requirement(nodes: &[ReqNode], edges: &[ReqEdge]) -> SceneBuild {
    // Same uniform grid as ERD/class, reusing its cell gap.
    let sizes: Vec<(i64, i64)> = nodes.iter().map(|n| (req_width(n), req_height(n))).collect();
    let (rects, total_w, total_h) = grid_layout(&sizes, ERD_PER_ROW, ERD_GAP, REQ_TITLE_H);

    let mut sc = SceneBuild::new(total_w, total_h);
    sc.items.push(SvgItem::ArrowDefs);

    // Dashed relation edges first so boxes paint over the line ends.
    for edge in edges {
        let (Some(f), Some(t)) = (
            nodes.iter().position(|n| n.id == edge.from),
            nodes.iter().position(|n| n.id == edge.to),
        ) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let (a, b) = (rects[f], rects[t]);
        // A straight line between non-adjacent boxes in the same grid row
        // would tunnel under the box between them and bury its label —
        // detour those edges below the row instead.
        let blocked = a.y == b.y
            && rects.iter().enumerate().any(|(k, r)| {
                k != f && k != t && r.y == a.y && r.x > a.x.min(b.x) && r.x < a.x.max(b.x)
            });
        let chrome = Chrome {
            class: Some("surfdoc-diagram-relation"),
            fill_none: true,
            dash: Some("6 4"),
        };
        let (label_x, label_y);
        if blocked {
            let row_bottom = rects
                .iter()
                .filter(|r| r.y == a.y)
                .map(|r| r.y + r.h)
                .max()
                .unwrap_or(a.y + a.h);
            let dip = row_bottom + 24;
            sc.h = sc.h.max(dip + MARGIN);
            sc.push(
                chrome,
                NativeShape::Line {
                    points: vec![
                        pt(a.cx(), a.y + a.h),
                        pt(a.cx(), dip),
                        pt(b.cx(), dip),
                        pt(b.cx(), b.y + b.h),
                    ],
                    stroke: NativeRole::Stroke,
                    stroke_width: 1.5,
                    dashed: true,
                    marker_start: NativeMarker::None,
                    marker_end: NativeMarker::Arrow,
                },
            );
            label_x = (a.cx() + b.cx()) / 2;
            label_y = dip - 6;
        } else {
            let (x1, y1, x2, y2) = facing_edge_points(&a, &b);
            sc.push(
                chrome,
                line2(x1, y1, x2, y2, NativeRole::Stroke, 1.5, true, NativeMarker::None, NativeMarker::Arrow),
            );
            label_x = (x1 + x2) / 2;
            label_y = (y1 + y2) / 2 - 6;
        }
        sc.push(
            Chrome::class("surfdoc-diagram-relation-label"),
            text_at(
                label_x,
                label_y,
                &format!("\u{ab}{}\u{bb}", edge.kind.word()),
                NativeRole::TextSecondary,
                11,
                false,
                NativeAnchor::Middle,
            ),
        );
    }

    // Requirement/element boxes: stereotype line, bold name, optional text.
    for (i, node) in nodes.iter().enumerate() {
        let r = &rects[i];
        sc.open_group("surfdoc-diagram-requirement");
        sc.push(
            Chrome::default(),
            rect_at(r.x, r.y, r.w, r.h, 4, NativeRole::OnAccent, NativeRole::Stroke),
        );
        sc.push(
            Chrome::default(),
            text_at(r.cx(), r.y + 16, req_stereotype(node), NativeRole::TextSecondary, 10, false, NativeAnchor::Middle),
        );
        sc.push(
            Chrome::default(),
            text_at(r.cx(), r.y + 33, &node.label, NativeRole::TextPrimary, 13, true, NativeAnchor::Middle),
        );
        if let Some(text) = &node.text {
            sc.push(
                Chrome::class("surfdoc-diagram-req-text"),
                text_at(r.x + 8, r.y + REQ_TITLE_H + ERD_ROW_H / 2 + 4, text, NativeRole::TextSecondary, 11, false, NativeAnchor::Start),
            );
        }
        sc.close_group();
    }

    sc
}

// ------------------------------------------------------------------
// Scene assembly — sankey
// ------------------------------------------------------------------

/// Node bar width.
const SK_BAR_W: i64 = 14;
/// Horizontal gap between node columns (the flow band region).
const SK_COL_GAP: i64 = 140;
/// Vertical gap between bars in a column.
const SK_VGAP: i64 = 18;
/// Pixel budget for the tallest column's summed throughput.
const SK_MAX_H: i64 = 220;

fn scene_sankey(nodes: &[String], flows: &[SankeyFlow]) -> SceneBuild {
    if nodes.is_empty() || flows.is_empty() {
        return SceneBuild::new(2 * MARGIN, 2 * MARGIN);
    }

    let idx = |name: &str| nodes.iter().position(|n| n == name);
    let edge_idx: Vec<(usize, usize)> = flows
        .iter()
        .filter_map(|f| Some((idx(&f.from)?, idx(&f.to)?)))
        .collect();

    // Column per node from longest-path layering over the flow graph.
    let layer = longest_path_layers(nodes.len(), &edge_idx);
    let n_cols = layer.iter().map(|l| l + 1).max().unwrap_or(1);
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); n_cols];
    for (i, &l) in layer.iter().enumerate() {
        columns[l].push(i);
    }

    // Node throughput (centi-units): max of inbound and outbound sums.
    // Widened to i128 so pathological magnitudes (a value parse saturates
    // at i64::MAX centi-units) can neither overflow the sums nor the pixel
    // scaling below; every px result is bounded by SK_MAX_H.
    let thr: Vec<i128> = (0..nodes.len())
        .map(|i| {
            let inflow: i128 =
                flows.iter().filter(|f| idx(&f.to) == Some(i)).map(|f| f.value_cs as i128).sum();
            let outflow: i128 =
                flows.iter().filter(|f| idx(&f.from) == Some(i)).map(|f| f.value_cs as i128).sum();
            inflow.max(outflow).max(1)
        })
        .collect();

    // One global scale: the busiest column's summed throughput maps to
    // SK_MAX_H pixels, so flow heights always match bar segments.
    let max_col_sum = columns
        .iter()
        .map(|col| col.iter().map(|&i| thr[i]).sum::<i128>())
        .max()
        .unwrap_or(1)
        .max(1);
    let px = |cs: i128| (cs * SK_MAX_H as i128 / max_col_sum) as i64;

    // Gutters for the outer labels (first column left, last column right).
    let gutter = |col: &[usize]| {
        col.iter()
            .map(|&i| nodes[i].chars().count() as i64 * 7 + 10)
            .max()
            .unwrap_or(10)
    };
    let gutter_l = gutter(&columns[0]);
    let gutter_r = gutter(columns.last().expect("n_cols >= 1"));

    // Column pixel heights → vertical centering.
    let col_px_h: Vec<i64> = columns
        .iter()
        .map(|col| {
            col.iter().map(|&i| px(thr[i])).sum::<i64>() + SK_VGAP * (col.len() as i64 - 1)
        })
        .collect();
    let max_col_px = col_px_h.iter().copied().max().unwrap_or(SK_MAX_H).max(1);

    // Bar rects (top-left + size), centered per column.
    let mut bars: Vec<Rect> = vec![Rect { x: 0, y: 0, w: 0, h: 0 }; nodes.len()];
    for (c, col) in columns.iter().enumerate() {
        let x = MARGIN + gutter_l + c as i64 * (SK_BAR_W + SK_COL_GAP);
        let mut y = MARGIN + 8 + (max_col_px - col_px_h[c]) / 2;
        for &i in col {
            bars[i] = Rect { x, y, w: SK_BAR_W, h: px(thr[i]) };
            y += px(thr[i]) + SK_VGAP;
        }
    }

    let total_w = MARGIN * 2 + gutter_l + n_cols as i64 * SK_BAR_W + (n_cols as i64 - 1) * SK_COL_GAP + gutter_r;
    let total_h = MARGIN * 2 + 16 + max_col_px;
    let mut sc = SceneBuild::new(total_w, total_h);

    // Flow bands first (straight-edged trapezoids), in declaration order;
    // per-node offsets keep bands stacked without overlap.
    let mut out_off: Vec<i64> = vec![0; nodes.len()];
    let mut in_off: Vec<i64> = vec![0; nodes.len()];
    for flow in flows {
        let (Some(f), Some(t)) = (idx(&flow.from), idx(&flow.to)) else {
            continue; // unreachable: endpoints are auto-declared at parse time
        };
        let h = px(flow.value_cs as i128).max(1);
        let (a, b) = (&bars[f], &bars[t]);
        let (y1, y2) = (a.y + out_off[f], b.y + in_off[t]);
        sc.push(
            Chrome::class("surfdoc-diagram-flow"),
            NativeShape::Polygon {
                points: vec![
                    pt(a.x + a.w, y1),
                    pt(b.x, y2),
                    pt(b.x, y2 + h),
                    pt(a.x + a.w, y1 + h),
                ],
                fill: NativeRole::AccentSoft,
                stroke: NativeRole::AccentSoft,
            },
        );
        out_off[f] += h;
        in_off[t] += h;
    }

    // Node bars + labels: first column labels sit left of the bar, the last
    // column right of it, and middle columns above it (clear of the bands).
    for (i, name) in nodes.iter().enumerate() {
        let r = &bars[i];
        sc.open_group("surfdoc-diagram-sankey-node");
        sc.push(
            Chrome::default(),
            rect_at(r.x, r.y, r.w, r.h, 0, NativeRole::Accent, NativeRole::Stroke),
        );
        let label = if layer[i] == 0 {
            text_at(r.x - 6, r.cy() + 4, name, NativeRole::TextPrimary, 12, false, NativeAnchor::End)
        } else if layer[i] == n_cols - 1 {
            text_at(r.x + r.w + 6, r.cy() + 4, name, NativeRole::TextPrimary, 12, false, NativeAnchor::Start)
        } else {
            text_at(r.cx(), r.y - 6, name, NativeRole::TextPrimary, 12, false, NativeAnchor::Middle)
        };
        sc.push(Chrome::default(), label);
        sc.close_group();
    }

    sc
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
    fn gantt_extreme_values_are_rejected_not_overflowed() {
        // Starts/durations beyond the layout-safe bound are parse errors
        // (prose fallback), never arithmetic overflow in the scene stage.
        for body in [
            "A: 9223372036854775807, 0",
            "A: 0, 9223372036854775807",
            "A: -9223372036854775808, 3",
            "A: 99999999999999-01-01, 3",
        ] {
            assert!(
                parse_diagram_source("gantt", body).is_err(),
                "{body:?} must be rejected"
            );
        }
        // Values at the bound still parse and render.
        let model = gantt("A: 1000000000, 1000000000\nB: -1000000000, 1");
        let svg = render_svg(&model, None);
        assert!(svg.starts_with("<svg"));
        let model = gantt("A: 9999-12-31, 5");
        assert!(render_svg(&model, None).starts_with("<svg"));
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
        // Stage-2 kinds escape through the same emitter.
        for (kind, body) in [
            ("timeline", "<b>Event</b>"),
            ("journey", "<b>Task</b>: 3"),
            ("quadrant", "<b>P</b>: 0.5, 0.5"),
            ("kanban", "column <b>Col</b>\n  <b>Card</b>"),
            ("usecase", "usecase a: <b>Case</b>"),
        ] {
            let model = parse_diagram_source(kind, body).expect("body parses");
            let svg = render_svg(&model, None);
            assert!(!svg.contains("<b>"), "{kind} must escape labels");
            assert!(svg.contains("&lt;b&gt;"), "{kind} must escape labels");
        }
    }

    #[test]
    fn empty_new_kinds_render_without_panic() {
        for kind in [
            "flowchart", "sequence", "gantt", "state", "mindmap", "class",
            "timeline", "journey", "quadrant", "kanban", "usecase",
            "gitgraph", "c4", "requirement", "sankey",
        ] {
            let model = parse_diagram_source(kind, "").expect("empty body parses");
            let svg = render_svg(&model, None);
            assert!(svg.starts_with("<svg"), "{kind} should render an svg");
            assert!(svg.ends_with("</svg>"));
        }
    }

    // ── class ───────────────────────────────────────────────────────

    fn class(content: &str) -> DiagramModel {
        parse_diagram_source("class", content).expect("class should parse")
    }

    #[test]
    fn class_parses_members_visibility_and_methods() {
        let DiagramModel::Class { classes, .. } = class(
            "User: +id, -email, #org_id, name, save(), +delete(), -hash_password()",
        ) else {
            panic!("expected Class");
        };
        assert_eq!(classes.len(), 1);
        let c = &classes[0];
        assert_eq!(c.name, "User");
        assert_eq!(c.stereotype, None);
        // Fields and methods split into their compartments.
        let field_names: Vec<&str> = c.fields.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(field_names, vec!["id", "email", "org_id", "name"]);
        assert_eq!(c.fields[0].visibility, Some('+'));
        assert_eq!(c.fields[1].visibility, Some('-'));
        assert_eq!(c.fields[2].visibility, Some('#'));
        assert_eq!(c.fields[3].visibility, None);
        let method_names: Vec<&str> = c.methods.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(method_names, vec!["save", "delete", "hash_password"]);
        assert!(c.methods.iter().all(|m| m.method));
        assert_eq!(c.methods[0].visibility, None);
        assert_eq!(c.methods[1].visibility, Some('+'));
    }

    #[test]
    fn class_parses_stereotypes() {
        let DiagramModel::Class { classes, .. } = class(
            "Color: enum, Red, Green, Blue\nShape: trait, +area()\nPlain: enum_like",
        ) else {
            panic!("expected Class");
        };
        assert_eq!(classes[0].stereotype.as_deref(), Some("enum"));
        // Stereotype token is not a member.
        assert_eq!(classes[0].fields.len(), 3);
        assert_eq!(classes[1].stereotype.as_deref(), Some("trait"));
        assert_eq!(classes[1].methods.len(), 1);
        // Only a bare `enum`/`trait` FIRST member is a stereotype.
        assert_eq!(classes[2].stereotype, None);
        assert_eq!(classes[2].fields[0].name, "enum_like");
        // Non-first `enum` stays a member.
        let DiagramModel::Class { classes, .. } = class("K: kind, enum") else {
            panic!("expected Class");
        };
        assert_eq!(classes[0].stereotype, None);
        assert_eq!(classes[0].fields.len(), 2);
    }

    #[test]
    fn class_parses_all_relation_kinds() {
        let DiagramModel::Class { classes, relations } = class(
            "A -> B: uses\nC *-> D\nE o-> F: pool\nG ^-> H",
        ) else {
            panic!("expected Class");
        };
        assert_eq!(relations.len(), 4);
        assert_eq!(relations[0].kind, ClassRelationKind::Association);
        assert_eq!(relations[0].label.as_deref(), Some("uses"));
        assert_eq!(relations[1].kind, ClassRelationKind::Composition);
        assert_eq!(relations[1].label, None);
        assert_eq!(relations[2].kind, ClassRelationKind::Aggregation);
        assert_eq!(relations[2].label.as_deref(), Some("pool"));
        assert_eq!(relations[3].kind, ClassRelationKind::Inheritance);
        // Unknown ids auto-declare empty classes in first-reference order.
        let names: Vec<&str> = classes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B", "C", "D", "E", "F", "G", "H"]);
        assert!(classes.iter().all(|c| c.fields.is_empty() && c.methods.is_empty()));
    }

    #[test]
    fn class_malformed_lines_report_line_numbers() {
        // No `:` and no arrow.
        let e = parse_diagram_source("class", "A: id\nB junk").expect_err("junk line");
        assert_eq!(e.line, 2);
        // Dangling arrow.
        let e = parse_diagram_source("class", "A ->").expect_err("dangling arrow");
        assert_eq!(e.line, 1);
        // Junk after relation target.
        let e = parse_diagram_source("class", "A -> B junk").expect_err("junk suffix");
        assert_eq!(e.line, 1);
        // Visibility sigil with no name.
        let e = parse_diagram_source("class", "A: +").expect_err("empty member");
        assert_eq!(e.line, 1);
        // Bare parens with no name.
        let e = parse_diagram_source("class", "A: +()").expect_err("empty method");
        assert_eq!(e.line, 1);
        // Non-id start.
        let e = parse_diagram_source("class", "-> B").expect_err("missing source");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn class_svg_structure_and_determinism() {
        let model = class(
            "User: +id, -email, +save()\nRole: enum, Admin, Member\nProfile: bio\nTeam: +name\nAdmin ^-> User\nUser *-> Profile: owns\nTeam o-> User: members\nUser -> Role",
        );
        let svg = render_svg(&model, Some("Domain"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Domain</title>"));
        // Three-compartment boxes.
        assert!(svg.contains("surfdoc-diagram-class"));
        assert!(svg.contains("surfdoc-diagram-class-title"));
        assert!(svg.contains("surfdoc-diagram-class-sep"));
        assert!(svg.contains("surfdoc-diagram-member"));
        // Stereotype above the name.
        assert!(svg.contains(">\u{ab}enum\u{bb}</text>"));
        assert!(svg.contains(">Role</text>"));
        // Members keep visibility sigils; methods re-gain `()`.
        assert!(svg.contains(">+id</text>"));
        assert!(svg.contains(">-email</text>"));
        assert!(svg.contains(">+save()</text>"));
        // UML edge markers from the class defs block.
        assert!(svg.contains("marker id=\"surfdoc-diamond\""));
        assert!(svg.contains("marker id=\"surfdoc-diamond-open\""));
        assert!(svg.contains("marker id=\"surfdoc-triangle-open\""));
        assert!(svg.contains("marker-end=\"url(#surfdoc-triangle-open)\"")); // inheritance
        assert!(svg.contains("marker-start=\"url(#surfdoc-diamond)\"")); // composition
        assert!(svg.contains("marker-start=\"url(#surfdoc-diamond-open)\"")); // aggregation
        assert!(svg.contains("marker-end=\"url(#surfdoc-arrow)\"")); // association
        // Relation label.
        assert!(svg.contains(">owns</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Domain")), render_svg(&model, Some("Domain")));
    }

    #[test]
    fn class_svg_escapes_user_text() {
        let svg = render_svg(&class("A: <b>x</b>\nA -> B: <i>l</i>"), None);
        assert!(!svg.contains("<b>"));
        assert!(svg.contains("&lt;b&gt;x&lt;/b&gt;"));
        assert!(svg.contains("&lt;i&gt;l&lt;/i&gt;"));
    }

    // ── timeline ────────────────────────────────────────────────────

    fn timeline(content: &str) -> DiagramModel {
        parse_diagram_source("timeline", content).expect("timeline should parse")
    }

    #[test]
    fn timeline_parses_dated_events() {
        let DiagramModel::Timeline { events } = timeline(
            "2026-01: Kickoff\n2026-03: Private beta\n2026-06-15: Launch",
        ) else {
            panic!("expected Timeline");
        };
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].marker.as_deref(), Some("2026-01"));
        assert_eq!(events[0].label, "Kickoff");
        assert_eq!(events[2].marker.as_deref(), Some("2026-06-15"));
        assert_eq!(events[2].label, "Launch");
    }

    #[test]
    fn timeline_parses_numeric_and_ordered_events() {
        // Numeric markers.
        let DiagramModel::Timeline { events } = timeline("1: First\n2: Second") else {
            panic!("expected Timeline");
        };
        assert_eq!(events[0].marker.as_deref(), Some("1"));
        // Ordered mode: no markers; labels may contain colons.
        let DiagramModel::Timeline { events } = timeline("Idea\nPrototype\nNote: colon stays") else {
            panic!("expected Timeline");
        };
        assert!(events.iter().all(|e| e.marker.is_none()));
        assert_eq!(events[2].label, "Note: colon stays");
    }

    #[test]
    fn timeline_rejects_mixed_modes() {
        // Dated + unmarked.
        let e = parse_diagram_source("timeline", "2026-01: Kickoff\nJust a label")
            .expect_err("mixed marked/unmarked");
        assert_eq!(e.line, 2);
        // Numeric + date markers (gantt no-mix law).
        let e = parse_diagram_source("timeline", "1: First\n2026-01: Second")
            .expect_err("mixed numeric/date");
        assert_eq!(e.line, 2);
        // Marker with empty label.
        let e = parse_diagram_source("timeline", "2026-01:").expect_err("empty label");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn timeline_svg_structure_and_determinism() {
        let model = timeline("2026-01: Kickoff\n2026-03: Beta\n2026-06: Launch");
        let svg = render_svg(&model, Some("Roadmap"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Roadmap</title>"));
        assert!(svg.contains("surfdoc-diagram-spine"));
        assert!(svg.contains("surfdoc-diagram-event"));
        assert!(svg.contains("surfdoc-diagram-stem"));
        assert!(svg.contains("marker-end=\"url(#surfdoc-arrow)\"")); // spine arrowhead
        assert!(svg.contains(">2026-01</text>"));
        assert!(svg.contains(">Kickoff</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Roadmap")), render_svg(&model, Some("Roadmap")));
    }

    #[test]
    fn timeline_scene_shapes() {
        let DiagramModel::Timeline { events } = timeline("2026-01: A\n2026-02: B\n2026-03: C") else {
            panic!("expected Timeline");
        };
        let scene = scene_timeline(&events);
        let shapes: Vec<&NativeShape> = scene
            .items
            .iter()
            .filter_map(|i| match i {
                SvgItem::Shape { shape, .. } => Some(shape),
                _ => None,
            })
            .collect();
        // 1 spine + 3 stems = 4 lines; 3 dots; 3 markers + 3 labels = 6 texts.
        let lines = shapes.iter().filter(|s| matches!(s, NativeShape::Line { .. })).count();
        let dots = shapes.iter().filter(|s| matches!(s, NativeShape::Ellipse { .. })).count();
        let texts = shapes.iter().filter(|s| matches!(s, NativeShape::Label { .. })).count();
        assert_eq!(lines, 4);
        assert_eq!(dots, 3);
        assert_eq!(texts, 6);
    }

    // ── journey ─────────────────────────────────────────────────────

    fn journey(content: &str) -> DiagramModel {
        parse_diagram_source("journey", content).expect("journey should parse")
    }

    #[test]
    fn journey_parses_sections_and_scores() {
        let DiagramModel::Journey { tasks } = journey(
            "section Onboarding\nSign up: 3\nVerify email: 2\nsection Daily use\nOpen app: 5",
        ) else {
            panic!("expected Journey");
        };
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].section.as_deref(), Some("Onboarding"));
        assert_eq!(tasks[0].score, 3);
        assert_eq!(tasks[2].section.as_deref(), Some("Daily use"));
        assert_eq!(tasks[2].score, 5);
    }

    #[test]
    fn journey_rejects_bad_scores() {
        let e = parse_diagram_source("journey", "Task: 0").expect_err("score too low");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("journey", "Task: 6").expect_err("score too high");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("journey", "Task: great").expect_err("non-numeric score");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("journey", "no colon here").expect_err("missing colon");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("journey", ": 3").expect_err("empty label");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn journey_svg_structure_and_determinism() {
        let model = journey("section On\nSign up: 3\nVerify: 2\nsection Use\nOpen: 5");
        let svg = render_svg(&model, Some("Journey"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Journey</title>"));
        assert!(svg.contains("surfdoc-diagram-lane"));
        assert!(svg.contains("surfdoc-diagram-section"));
        assert!(svg.contains("surfdoc-diagram-grid"));
        assert!(svg.contains("surfdoc-diagram-journey-line"));
        assert!(svg.contains("surfdoc-diagram-task"));
        assert!(svg.contains(">Sign up</text>"));
        assert!(svg.contains(">5</text>")); // score axis tick
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, None), render_svg(&model, None));
    }

    #[test]
    fn journey_scene_shapes() {
        let DiagramModel::Journey { tasks } = journey("section S\nA: 1\nB: 5") else {
            panic!("expected Journey");
        };
        let scene = scene_journey(&tasks);
        let shapes: Vec<&NativeShape> = scene
            .items
            .iter()
            .filter_map(|i| match i {
                SvgItem::Shape { shape, .. } => Some(shape),
                _ => None,
            })
            .collect();
        // 1 lane rect; 5 gridlines + 1 connector = 6 lines; 2 dots;
        // 1 section + 5 ticks + 2 task labels = 8 texts.
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Rect { .. })).count(), 1);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Line { .. })).count(), 6);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Ellipse { .. })).count(), 2);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Label { .. })).count(), 8);
    }

    // ── quadrant ────────────────────────────────────────────────────

    fn quadrant(content: &str) -> DiagramModel {
        parse_diagram_source("quadrant", content).expect("quadrant should parse")
    }

    #[test]
    fn quadrant_parses_axes_labels_and_points() {
        let DiagramModel::Quadrant { x_axis, y_axis, labels, points } = quadrant(
            "x-axis Low effort --> High effort\ny-axis Low impact --> High impact\nquadrant-1: Quick wins\nquadrant-3: Fill-ins\nTask A: 0.3, 0.7\nTask B: 1, 0",
        ) else {
            panic!("expected Quadrant");
        };
        assert_eq!(x_axis, Some(("Low effort".into(), "High effort".into())));
        assert_eq!(y_axis, Some(("Low impact".into(), "High impact".into())));
        assert_eq!(labels[0].as_deref(), Some("Quick wins"));
        assert_eq!(labels[1], None);
        assert_eq!(labels[2].as_deref(), Some("Fill-ins"));
        assert_eq!(points.len(), 2);
        assert_eq!((points[0].x_mil, points[0].y_mil), (300, 700));
        assert_eq!((points[1].x_mil, points[1].y_mil), (1000, 0));
    }

    #[test]
    fn quadrant_rejects_malformed_lines() {
        let e = parse_diagram_source("quadrant", "x-axis no arrow").expect_err("axis arrow");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("quadrant", "quadrant-5: Nope").expect_err("bad number");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("quadrant", "A: 1.5, 0.5").expect_err("out of range");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("quadrant", "A: 0.5").expect_err("missing y");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("quadrant", "A: x, y").expect_err("non-numeric");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("quadrant", "no colon").expect_err("junk line");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn quadrant_svg_structure_and_determinism() {
        let model = quadrant(
            "x-axis Low --> High\ny-axis Cold --> Hot\nquadrant-1: Invest\nA: 0.75, 0.75",
        );
        let svg = render_svg(&model, Some("Q"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Q</title>"));
        assert!(svg.contains("surfdoc-diagram-frame"));
        assert!(svg.contains("surfdoc-diagram-grid"));
        assert!(svg.contains("surfdoc-diagram-quadrant-label"));
        assert!(svg.contains("surfdoc-diagram-axis-label"));
        assert!(svg.contains("surfdoc-diagram-point"));
        assert!(svg.contains(">Invest</text>"));
        assert!(svg.contains(">A</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Q")), render_svg(&model, Some("Q")));
    }

    #[test]
    fn quadrant_scene_shapes() {
        let DiagramModel::Quadrant { x_axis, y_axis, labels, points } =
            quadrant("x-axis L --> R\nA: 0, 0\nB: 0.5, 0.5")
        else {
            panic!("expected Quadrant");
        };
        let scene = scene_quadrant(x_axis.as_ref(), y_axis.as_ref(), &labels, &points);
        let shapes: Vec<&NativeShape> = scene
            .items
            .iter()
            .filter_map(|i| match i {
                SvgItem::Shape { shape, .. } => Some(shape),
                _ => None,
            })
            .collect();
        // 1 frame rect; 2 midlines; 2 dots; 2 x-axis labels + 2 point labels.
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Rect { .. })).count(), 1);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Line { .. })).count(), 2);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Ellipse { .. })).count(), 2);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Label { .. })).count(), 4);
    }

    // ── kanban ──────────────────────────────────────────────────────

    fn kanban(content: &str) -> DiagramModel {
        parse_diagram_source("kanban", content).expect("kanban should parse")
    }

    #[test]
    fn kanban_parses_both_header_forms_and_cards() {
        let DiagramModel::Kanban { columns } = kanban(
            "column To do\n  Write spec\n  Review API\nDoing:\n  Build parser\nDone:\n  Ship v1",
        ) else {
            panic!("expected Kanban");
        };
        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].name, "To do");
        assert_eq!(columns[0].cards, vec!["Write spec", "Review API"]);
        assert_eq!(columns[1].name, "Doing");
        assert_eq!(columns[1].cards, vec!["Build parser"]);
        assert_eq!(columns[2].name, "Done");
    }

    #[test]
    fn kanban_rejects_malformed_lines() {
        let e = parse_diagram_source("kanban", "  Orphan card").expect_err("card before column");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("kanban", "column To do\nnot a header").expect_err("junk line");
        assert_eq!(e.line, 2);
        let e = parse_diagram_source("kanban", "column ").expect_err("empty keyword name");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("kanban", ":").expect_err("empty colon name");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn kanban_svg_structure_and_determinism() {
        let model = kanban("column To do\n  Spec\nDoing:\n  Parser\nDone:\n  v1");
        let svg = render_svg(&model, Some("Board"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Board</title>"));
        assert!(svg.contains("surfdoc-diagram-column"));
        assert!(svg.contains("surfdoc-diagram-column-title"));
        assert!(svg.contains("surfdoc-diagram-card"));
        assert!(svg.contains(">To do</text>"));
        assert!(svg.contains(">Parser</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Board")), render_svg(&model, Some("Board")));
    }

    #[test]
    fn kanban_overflow_note_after_eight_cards() {
        let cards: Vec<String> = (1..=11).map(|i| format!("  Card {i}")).collect();
        let body = format!("column Busy\n{}", cards.join("\n"));
        let svg = render_svg(&kanban(&body), None);
        assert!(svg.contains(">Card 8</text>"));
        assert!(!svg.contains(">Card 9</text>"));
        assert!(svg.contains("surfdoc-diagram-more"));
        assert!(svg.contains(">+3 more</text>"));
    }

    #[test]
    fn kanban_scene_shapes() {
        let DiagramModel::Kanban { columns } = kanban("column A\n  one\n  two\nB:\n  three") else {
            panic!("expected Kanban");
        };
        let scene = scene_kanban(&columns);
        let shapes: Vec<&NativeShape> = scene
            .items
            .iter()
            .filter_map(|i| match i {
                SvgItem::Shape { shape, .. } => Some(shape),
                _ => None,
            })
            .collect();
        // Per column: outer + header rects; per card: one rect. 2*2 + 3 = 7.
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Rect { .. })).count(), 7);
        // 2 column names + 3 card labels.
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Label { .. })).count(), 5);
    }

    // ── usecase ─────────────────────────────────────────────────────

    fn usecase(content: &str) -> DiagramModel {
        parse_diagram_source("usecase", content).expect("usecase should parse")
    }

    #[test]
    fn usecase_parses_actors_cases_and_edges() {
        let DiagramModel::UseCase { actors, cases, edges } = usecase(
            "actor customer: Customer\nusecase browse: Browse catalog\nusecase pay: Enter payment\ncustomer -> browse\nbrowse ^-> pay: includes",
        ) else {
            panic!("expected UseCase");
        };
        assert_eq!(actors.len(), 1);
        assert_eq!(actors[0].label, "Customer");
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].label, "Browse catalog");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].kind, UcEdgeKind::Association);
        assert_eq!(edges[1].kind, UcEdgeKind::Include);
    }

    #[test]
    fn usecase_auto_declares_edge_endpoints() {
        // `->` source becomes an actor, target a use case; both `^->`
        // endpoints become use cases.
        let DiagramModel::UseCase { actors, cases, edges } =
            usecase("guest -> checkout\ncheckout ^-> pay: extends")
        else {
            panic!("expected UseCase");
        };
        assert_eq!(actors.len(), 1);
        assert_eq!(actors[0].id, "guest");
        let ids: Vec<&str> = cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["checkout", "pay"]);
        assert_eq!(edges[1].kind, UcEdgeKind::Extend);
    }

    #[test]
    fn usecase_rejects_malformed_lines() {
        let e = parse_diagram_source("usecase", "actor").expect_err("bare actor keyword");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("usecase", "usecase : X").expect_err("missing id");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("usecase", "a ^-> b").expect_err("dependency needs label");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("usecase", "a ^-> b: sometimes").expect_err("bad label");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("usecase", "a -> b: uses").expect_err("assoc label");
        assert_eq!(e.line, 1);
        let e = parse_diagram_source("usecase", "a ->").expect_err("dangling arrow");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn usecase_svg_structure_and_determinism() {
        let model = usecase(
            "actor customer: Customer\nusecase browse: Browse\nusecase pay: Pay\ncustomer -> browse\nbrowse ^-> pay: includes",
        );
        let svg = render_svg(&model, Some("Shop"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>Shop</title>"));
        assert!(svg.contains("surfdoc-diagram-boundary"));
        assert!(svg.contains("surfdoc-diagram-actor-figure"));
        assert!(svg.contains("surfdoc-diagram-usecase"));
        assert!(svg.contains("surfdoc-diagram-assoc"));
        assert!(svg.contains("surfdoc-diagram-uc-rel"));
        assert!(svg.contains("stroke-dasharray=\"6 4\"")); // dashed dependency
        assert!(svg.contains(">\u{ab}include\u{bb}</text>"));
        assert!(svg.contains("<ellipse")); // use-case bubbles
        assert!(svg.contains(">Customer</text>"));
        assert!(svg.ends_with("</svg>"));
        assert_eq!(render_svg(&model, Some("Shop")), render_svg(&model, Some("Shop")));
    }

    #[test]
    fn usecase_scene_shapes() {
        let DiagramModel::UseCase { actors, cases, edges } =
            usecase("actor u: User\nusecase a: One\nusecase b: Two\nu -> a\na ^-> b: includes")
        else {
            panic!("expected UseCase");
        };
        let scene = scene_usecase(&actors, &cases, &edges);
        let shapes: Vec<&NativeShape> = scene
            .items
            .iter()
            .filter_map(|i| match i {
                SvgItem::Shape { shape, .. } => Some(shape),
                _ => None,
            })
            .collect();
        // 1 boundary rect; head circle + 2 non-circular ellipses = 3
        // Ellipse shapes; 4 figure limbs + 1 assoc + 1 dependency = 6 lines;
        // 1 actor name + 2 case labels + 1 «include» = 4 texts.
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Rect { .. })).count(), 1);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Ellipse { .. })).count(), 3);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Line { .. })).count(), 6);
        assert_eq!(shapes.iter().filter(|s| matches!(s, NativeShape::Label { .. })).count(), 4);
    }

    // ── chart aliases ───────────────────────────────────────────────

    #[test]
    fn chart_alias_covers_exactly_the_alias_types() {
        use crate::types::ChartType;
        assert_eq!(chart_alias("pie"), Some(ChartType::Pie));
        assert_eq!(chart_alias("donut"), Some(ChartType::Donut));
        assert_eq!(chart_alias("radar"), Some(ChartType::Radar));
        // xychart renders as the general xy line chart.
        assert_eq!(chart_alias("xychart"), Some(ChartType::Line));
        // Native geometry types never alias.
        for kind in [
            "architecture", "erd", "flowchart", "sequence", "gantt", "state",
            "mindmap", "class", "timeline", "journey", "quadrant", "kanban", "usecase",
        ] {
            assert_eq!(chart_alias(kind), None, "{kind} must not alias to a chart");
        }
        // And the alias types are not parseable as geometry diagrams.
        for kind in ["pie", "donut", "radar", "xychart"] {
            assert!(parse_diagram_source(kind, "a | b\nc | 1").is_err());
        }
    }

    // ── geometry scenes ─────────────────────────────────────────────

    #[test]
    fn scenes_carry_shapes_without_svg_chrome() {
        // Every kind lays out through the scene stage; the scene holds only
        // typed shapes (defs/groups are SVG-side serialization details).
        for (kind, body) in [
            ("architecture", "a: A\nb: B\na -> b: x"),
            ("erd", "u: id pk\nu 1--* o: owns"),
            ("flowchart", "s [rounded]: S\nd [diamond]: D?\ns -> d: y"),
            ("sequence", "actor a: A\na -> b: hi\nactivate b\nb --> a: yo\ndeactivate b\na -> a: self"),
            ("gantt", "section P\nT: 0, 3"),
            ("state", "[*] -> Idle\nIdle -> [*]: done"),
            ("mindmap", "Root\n  Leaf"),
            ("class", "U: +id, +save()\nR: enum, X\nU *-> R: has"),
            ("timeline", "2026-01: Kickoff\n2026-02: Beta"),
            ("journey", "section S\nSign up: 3\nUse: 5"),
            ("quadrant", "x-axis L --> R\nA: 0.2, 0.8"),
            ("kanban", "column Todo\n  Card one"),
            ("usecase", "actor u: User\nusecase c: Case\nu -> c"),
            ("gitgraph", "commit: init\nbranch dev\ncommit\ncheckout main\nmerge dev: ship"),
            ("c4", "person u: User\nboundary Core {\ncontainer api: API: Rust\n}\nu -> api: Uses"),
            ("requirement", "requirement r1: Fast: under 5ms\nelement p: parser\np -> r1: satisfies"),
            ("sankey", "Wind -> Grid: 40\nGrid -> Homes: 25.5"),
        ] {
            let model = parse_diagram_source(kind, body).expect("body parses");
            let scene = build_scene(&model);
            assert!(scene.w > 0 && scene.h > 0, "{kind} scene has a canvas");
            let shapes: Vec<&NativeShape> = scene
                .items
                .iter()
                .filter_map(|i| match i {
                    SvgItem::Shape { shape, .. } => Some(shape),
                    _ => None,
                })
                .collect();
            assert!(!shapes.is_empty(), "{kind} scene has shapes");
            // Scene labels stay raw; escaping happens at SVG serialization.
            assert!(
                shapes.iter().all(|s| match s {
                    NativeShape::Label { text, .. } => !text.contains("&lt;"),
                    _ => true,
                }),
                "{kind} labels are unescaped in the scene"
            );
        }
    }

    // ── DSL parsing: gitgraph ───────────────────────────────────────

    #[test]
    fn gitgraph_branches_commits_and_merge() {
        let DiagramModel::GitGraph { branches, commits } = parse_diagram_source(
            "gitgraph",
            "commit: init\nbranch feature\ncommit: draft\ncommit\ncheckout main\nmerge feature: ship",
        )
        .expect("gitgraph should parse") else {
            panic!("expected GitGraph model");
        };
        assert_eq!(branches, vec!["main", "feature"]);
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].branch, 0);
        assert_eq!(commits[0].label.as_deref(), Some("init"));
        assert_eq!(commits[1].branch, 1);
        assert_eq!(commits[2].label, None);
        // The merge commit lands on main and points at feature's tip.
        assert_eq!(commits[3].branch, 0);
        assert_eq!(commits[3].merge_from, Some(2));
        assert_eq!(commits[3].label.as_deref(), Some("ship"));
    }

    #[test]
    fn gitgraph_merge_of_empty_branch_degrades_to_commit() {
        let DiagramModel::GitGraph { commits, .. } =
            parse_diagram_source("gitgraph", "commit\nmerge ghost").expect("should parse")
        else {
            panic!("expected GitGraph model");
        };
        assert_eq!(commits[1].merge_from, None);
    }

    #[test]
    fn gitgraph_junk_line_reports_line_number() {
        let e = parse_diagram_source("gitgraph", "commit\nrebase main").expect_err("junk");
        assert_eq!(e.line, 2);
    }

    #[test]
    fn gitgraph_svg_structure() {
        let model = parse_diagram_source(
            "gitgraph",
            "commit: init\nbranch dev\ncommit\ncheckout main\nmerge dev",
        )
        .expect("parses");
        let svg = render_svg(&model, Some("History"));
        assert!(svg.starts_with("<svg class=\"surfdoc-diagram-svg\""));
        assert!(svg.contains("<title>History</title>"));
        assert!(svg.contains("surfdoc-diagram-lane"));
        assert!(svg.contains("surfdoc-diagram-commit"));
        assert!(svg.contains("surfdoc-diagram-merge"));
        assert!(svg.contains(">main</text>"));
        assert!(svg.contains(">dev</text>"));
        assert_eq!(svg.matches("<circle").count(), 3, "one dot per commit");
    }

    // ── DSL parsing: c4 ─────────────────────────────────────────────

    #[test]
    fn c4_nodes_boundary_and_edges() {
        let DiagramModel::C4 { nodes, boundaries, edges } = parse_diagram_source(
            "c4",
            "person u: Customer\nboundary Platform {\ncontainer api: API App: Rust\ncontainer db: Database\n}\nsystem mail: Mailer [ext]\nu -> api: Uses\napi -> mail: SMTP",
        )
        .expect("c4 should parse") else {
            panic!("expected C4 model");
        };
        assert_eq!(boundaries, vec!["Platform"]);
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0].kind, C4Kind::Person);
        assert_eq!(nodes[0].boundary, None);
        assert_eq!(nodes[1].label, "API App");
        assert_eq!(nodes[1].tech.as_deref(), Some("Rust"));
        assert_eq!(nodes[1].boundary, Some(0));
        assert_eq!(nodes[2].tech, None);
        assert!(nodes[3].external);
        assert_eq!(nodes[3].boundary, None);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].label.as_deref(), Some("Uses"));
    }

    #[test]
    fn c4_nested_boundary_fails() {
        let e = parse_diagram_source("c4", "boundary A {\nboundary B {\n}\n}").expect_err("nesting");
        assert_eq!(e.line, 2);
    }

    #[test]
    fn c4_stray_close_fails() {
        let e = parse_diagram_source("c4", "}").expect_err("stray close");
        assert_eq!(e.line, 1);
    }

    #[test]
    fn c4_auto_declares_edge_endpoints_as_systems() {
        let DiagramModel::C4 { nodes, .. } =
            parse_diagram_source("c4", "a -> b: x").expect("should parse")
        else {
            panic!("expected C4 model");
        };
        assert_eq!(nodes.len(), 2);
        assert!(nodes.iter().all(|n| n.kind == C4Kind::System && !n.external));
    }

    #[test]
    fn c4_svg_structure() {
        let model = parse_diagram_source(
            "c4",
            "person u: Customer\nboundary Core {\ncontainer api: API: Rust\n}\nsystem ext1: Billing [ext]\nu -> api: Uses\napi -> ext1",
        )
        .expect("parses");
        let svg = render_svg(&model, None);
        assert!(svg.contains("surfdoc-diagram-boundary"));
        assert!(svg.contains(">Core</text>"));
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains(">[Rust]</text>"));
        assert!(svg.contains("marker-end=\"url(#surfdoc-arrow)\""));
        // The person head circle is drawn.
        assert!(svg.contains("<circle"));
        // External system paints muted.
        assert!(svg.contains("fill=\"#e2e8f0\""));
    }

    // ── DSL parsing: requirement ────────────────────────────────────

    #[test]
    fn requirement_nodes_and_edges() {
        let DiagramModel::Requirement { nodes, edges } = parse_diagram_source(
            "requirement",
            "requirement r1: Fast render: SVG in under 5ms\nrequirement r2: Stable output\nelement parser: surf-parse\nparser -> r1: satisfies\nr2 -> r1: derives",
        )
        .expect("requirement should parse") else {
            panic!("expected Requirement model");
        };
        assert_eq!(nodes.len(), 3);
        assert!(nodes[0].requirement);
        assert_eq!(nodes[0].label, "Fast render");
        assert_eq!(nodes[0].text.as_deref(), Some("SVG in under 5ms"));
        assert_eq!(nodes[1].text, None);
        assert!(!nodes[2].requirement);
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].kind, ReqEdgeKind::Satisfies);
        assert_eq!(edges[1].kind, ReqEdgeKind::Derives);
    }

    #[test]
    fn requirement_unknown_relation_fails() {
        let e = parse_diagram_source("requirement", "a -> b: blesses").expect_err("bad kind");
        assert_eq!(e.line, 1);
        assert!(e.message.contains("blesses"));
    }

    #[test]
    fn requirement_edge_without_kind_fails() {
        assert!(parse_diagram_source("requirement", "a -> b").is_err());
    }

    #[test]
    fn requirement_svg_structure() {
        let model = parse_diagram_source(
            "requirement",
            "requirement r1: Fast: under 5ms\nelement p: parser\np -> r1: satisfies",
        )
        .expect("parses");
        let svg = render_svg(&model, None);
        assert!(svg.contains("surfdoc-diagram-requirement"));
        assert!(svg.contains("\u{ab}requirement\u{bb}"));
        assert!(svg.contains("\u{ab}element\u{bb}"));
        assert!(svg.contains("\u{ab}satisfies\u{bb}"));
        assert!(svg.contains("stroke-dasharray=\"6 4\""));
        assert!(svg.contains(">under 5ms</text>"));
    }

    // ── DSL parsing: sankey ─────────────────────────────────────────

    #[test]
    fn sankey_nodes_and_scaled_values() {
        let DiagramModel::Sankey { nodes, flows } = parse_diagram_source(
            "sankey",
            "Wind farm -> Grid: 40\nSolar -> Grid: 30.25\nGrid -> Homes: 50\nGrid -> Industry: 20",
        )
        .expect("sankey should parse") else {
            panic!("expected Sankey model");
        };
        assert_eq!(nodes, vec!["Wind farm", "Grid", "Solar", "Homes", "Industry"]);
        assert_eq!(flows.len(), 4);
        assert_eq!(flows[0].value_cs, 4000);
        assert_eq!(flows[1].value_cs, 3025);
    }

    #[test]
    fn sankey_rejects_non_positive_values() {
        assert!(parse_diagram_source("sankey", "a -> b: 0").is_err());
        assert!(parse_diagram_source("sankey", "a -> b: -3").is_err());
        assert!(parse_diagram_source("sankey", "a -> b: lots").is_err());
    }

    #[test]
    fn sankey_missing_value_reports_line_number() {
        let e = parse_diagram_source("sankey", "a -> b: 5\nc -> d").expect_err("no value");
        assert_eq!(e.line, 2);
    }

    #[test]
    fn sankey_svg_structure() {
        let model = parse_diagram_source(
            "sankey",
            "Wind -> Grid: 40\nSolar -> Grid: 30\nGrid -> Homes: 50\nGrid -> Industry: 20",
        )
        .expect("parses");
        let svg = render_svg(&model, Some("Energy"));
        assert!(svg.contains("<title>Energy</title>"));
        // One trapezoid band per flow.
        assert_eq!(svg.matches("surfdoc-diagram-flow").count(), 4);
        // One bar per node.
        assert_eq!(svg.matches("surfdoc-diagram-sankey-node").count(), 5);
        assert!(svg.contains(">Wind</text>"));
        assert!(svg.contains(">Industry</text>"));
    }

    #[test]
    fn sankey_huge_values_render_without_overflow() {
        // Values are stored in centi-units; enormous magnitudes (the f64→i64
        // conversion saturates at i64::MAX) must not overflow the pixel
        // scaling arithmetic. Output stays a valid bounded-canvas SVG.
        let model = parse_diagram_source(
            "sankey",
            "A -> B: 99999999999999999999\nB -> C: 92233720368547758.07",
        )
        .expect("parses");
        let svg = render_svg(&model, None);
        assert!(svg.starts_with("<svg"));
        assert_eq!(svg.matches("surfdoc-diagram-flow").count(), 2);
    }

    #[test]
    fn sankey_flow_heights_match_bar_segments() {
        // Grid's bar height equals the sum of its two outbound band heights
        // (single global scale, integer arithmetic).
        let model = parse_diagram_source("sankey", "A -> C: 30\nB -> C: 10\nC -> D: 40")
            .expect("parses");
        let scene = build_scene(&model);
        let polys: Vec<&NativeShape> = scene
            .items
            .iter()
            .filter_map(|i| match i {
                SvgItem::Shape { shape: s @ NativeShape::Polygon { .. }, .. } => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(polys.len(), 3);
        for p in polys {
            let NativeShape::Polygon { points, .. } = p else { unreachable!() };
            assert_eq!(points.len(), 4, "bands are straight-edged trapezoids");
        }
    }
}

//! Mermaid-syntax acceptance for `::diagram` bodies.
//!
//! Every `::diagram` body is sniffed: when its first significant line is a
//! mermaid header (`flowchart LR`, `sequenceDiagram`, `erDiagram`, …) — or
//! the block says `type=mermaid` explicitly — the body is translated to the
//! native diagram DSL and rendered through the normal geometry pipeline.
//! Thirteen mermaid families are accepted: flowchart/graph, sequence, class,
//! state, er, gantt, mindmap, pie (via the chart pipeline), timeline,
//! journey, kanban, quadrant and gitGraph.
//!
//! Translation covers the common core of each family. Constructs outside
//! that core (subgraphs, notes, loop/alt frames, styling directives, …)
//! degrade per line: the line is skipped and recorded as a [`MermaidNote`],
//! which the `L040` lint surfaces. A body that yields no statements at all
//! translates to nothing (`None`) and falls back to prose like any other
//! unparseable diagram — a diagram must NEVER fail a render.
//!
//! DETERMINISM: translation is a pure line-by-line function of the body
//! text; the emitted native DSL is rendered by [`crate::diagram`] under its
//! usual byte-stability guarantees.

/// A skipped mermaid construct: the 1-based line number within the diagram
/// body and a short description of what was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MermaidNote {
    pub(crate) line: usize,
    pub(crate) construct: String,
}

/// The result of translating a mermaid body: the native diagram type (or
/// chart-alias type for `pie`), the native DSL body, and per-line notes for
/// every construct the translation skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MermaidTranslation {
    pub(crate) diagram_type: &'static str,
    pub(crate) content: String,
    pub(crate) notes: Vec<MermaidNote>,
}

/// The mermaid diagram families the translator understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Flowchart,
    Sequence,
    Class,
    State,
    Er,
    Gantt,
    Mindmap,
    Pie,
    Timeline,
    Journey,
    Kanban,
    Quadrant,
    GitGraph,
}

/// Translate a mermaid `::diagram` body to the native DSL.
///
/// Returns `None` when the body is not mermaid (first significant line is
/// no known header) or when translation produced no usable statements —
/// callers then continue down the normal native-DSL / prose-fallback path.
/// `diagram_type` is the block's (lowercased) `type` attribute; the value
/// `mermaid` forces the sniff, every other value merely allows it.
pub(crate) fn translate(diagram_type: &str, content: &str) -> Option<MermaidTranslation> {
    let _ = diagram_type; // `type=mermaid` and sniffed bodies take the same path
    let lines: Vec<&str> = content.lines().collect();
    let (family, header_idx) = sniff(&lines)?;

    let mut notes: Vec<MermaidNote> = Vec::new();
    let body = match family {
        Family::Flowchart => flowchart(&lines, header_idx, &mut notes),
        Family::Sequence => sequence(&lines, header_idx, &mut notes),
        Family::Class => class(&lines, header_idx, &mut notes),
        Family::State => state(&lines, header_idx, &mut notes),
        Family::Er => er(&lines, header_idx, &mut notes),
        Family::Gantt => gantt(&lines, header_idx, &mut notes),
        Family::Mindmap => mindmap(&lines, header_idx, &mut notes),
        Family::Pie => pie(&lines, header_idx, &mut notes),
        Family::Timeline => timeline(&lines, header_idx, &mut notes),
        Family::Journey => journey(&lines, header_idx, &mut notes),
        Family::Kanban => kanban(&lines, header_idx, &mut notes),
        Family::Quadrant => quadrant(&lines, header_idx, &mut notes),
        Family::GitGraph => gitgraph(&lines, header_idx, &mut notes),
    };
    if body.trim().is_empty() {
        return None; // nothing translated — prose fallback shows the source
    }
    Some(MermaidTranslation {
        diagram_type: match family {
            Family::Flowchart => "flowchart",
            Family::Sequence => "sequence",
            Family::Class => "class",
            Family::State => "state",
            Family::Er => "erd",
            Family::Gantt => "gantt",
            Family::Mindmap => "mindmap",
            Family::Pie => "pie",
            Family::Timeline => "timeline",
            Family::Journey => "journey",
            Family::Kanban => "kanban",
            Family::Quadrant => "quadrant",
            Family::GitGraph => "gitgraph",
        },
        content: body,
        notes,
    })
}

/// Find the mermaid header: the first significant line (skipping blanks,
/// `%%` comments and a leading `---` front-matter fence) must match a known
/// family header. Returns the family and the header's line index.
fn sniff(lines: &[&str]) -> Option<(Family, usize)> {
    let mut i = 0;
    // Skip a leading mermaid YAML front-matter block (`---` … `---`).
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    if i < lines.len() && lines[i].trim() == "---" {
        i += 1;
        while i < lines.len() && lines[i].trim() != "---" {
            i += 1;
        }
        i += 1; // past the closing fence
    }
    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || line.starts_with("%%") {
            i += 1;
            continue;
        }
        return header_family(line).map(|f| (f, i));
    }
    None
}

/// Classify a candidate header line.
fn header_family(line: &str) -> Option<Family> {
    let word = line.split_whitespace().next().unwrap_or("");
    match word {
        "flowchart" | "graph" => {
            // Only a bare header or a direction suffix counts.
            let rest = line[word.len()..].trim();
            matches!(rest, "" | "LR" | "TD" | "TB" | "RL" | "BT").then_some(Family::Flowchart)
        }
        "sequenceDiagram" => Some(Family::Sequence),
        "classDiagram" | "classDiagram-v2" => Some(Family::Class),
        "stateDiagram" | "stateDiagram-v2" => Some(Family::State),
        "erDiagram" => Some(Family::Er),
        "gantt" if line == "gantt" => Some(Family::Gantt),
        "mindmap" if line == "mindmap" => Some(Family::Mindmap),
        "pie" => Some(Family::Pie),
        "timeline" if line == "timeline" => Some(Family::Timeline),
        "journey" if line == "journey" => Some(Family::Journey),
        "kanban" if line == "kanban" => Some(Family::Kanban),
        "quadrantChart" => Some(Family::Quadrant),
        "gitGraph" => Some(Family::GitGraph),
        _ => None,
    }
}

// ------------------------------------------------------------------
// Shared helpers
// ------------------------------------------------------------------

/// Record a skipped construct.
fn note(notes: &mut Vec<MermaidNote>, idx: usize, construct: impl Into<String>) {
    notes.push(MermaidNote {
        line: idx + 1,
        construct: construct.into(),
    });
}

/// True for lines the translators always ignore silently.
fn is_noise(line: &str) -> bool {
    line.is_empty() || line.starts_with("%%")
}

/// Strip one pair of surrounding double quotes.
fn unquote(s: &str) -> &str {
    let s = s.trim();
    s.strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(s)
}

/// Clean a mermaid label for the native DSL: quotes stripped, `<br>` line
/// breaks flattened to spaces, whitespace collapsed.
fn clean_label(s: &str) -> String {
    let mut t = unquote(s).to_string();
    for br in ["<br/>", "<br />", "<br>"] {
        t = t.replace(br, " ");
    }
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Sanitize a mermaid id into the native id charset `[A-Za-z0-9_-]`.
fn safe_id(s: &str) -> String {
    let cleaned: String = s
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if cleaned.is_empty() { "_".to_string() } else { cleaned }
}

/// Take a leading mermaid id off `s`, returning `(id, rest)`.
fn take_id(s: &str) -> (&str, &str) {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    (&s[..end], &s[end..])
}

/// Does the line start with `keyword` followed by whitespace or end?
fn keyword<'a>(line: &'a str, kw: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(kw)?;
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some(rest.trim_start())
    } else {
        None
    }
}

// ------------------------------------------------------------------
// flowchart / graph
// ------------------------------------------------------------------

/// Bracketed node shapes: opener → (closer, native shape token).
const FLOW_BRACKETS: &[(&str, &str, &str)] = &[
    ("([", "])", "rounded"),
    ("[[", "]]", "box"),
    ("[(", ")]", "rounded"),
    ("((", "))", "rounded"),
    ("{{", "}}", "diamond"),
    ("[", "]", "box"),
    ("(", ")", "rounded"),
    ("{", "}", "diamond"),
    (">", "]", "box"),
];

/// One parsed flowchart node term: id, optional (label, shape).
struct FlowTerm {
    id: String,
    decl: Option<(String, &'static str)>,
}

/// Parse a node term (`A`, `A[Label]`, `B{Choice?}`, …) off the front of
/// `s`. Returns the term and the remainder.
fn take_flow_term(s: &str) -> Option<(FlowTerm, &str)> {
    let s = s.trim_start();
    let (id, rest) = take_id(s);
    if id.is_empty() {
        return None;
    }
    for (open, close, shape) in FLOW_BRACKETS {
        if let Some(inner) = rest.strip_prefix(open) {
            if let Some(pos) = inner.find(close) {
                let label = clean_label(&inner[..pos]);
                return Some((
                    FlowTerm {
                        id: safe_id(id),
                        decl: Some((label, shape)),
                    },
                    &inner[pos + close.len()..],
                ));
            }
        }
    }
    Some((
        FlowTerm {
            id: safe_id(id),
            decl: None,
        },
        rest,
    ))
}

/// Parse an arrow/link token off the front of `s`. Returns the optional
/// inline label and the remainder. All link styles map to the one native
/// arrow.
fn take_flow_arrow(s: &str) -> Option<(Option<String>, &str)> {
    let s = s.trim_start();

    // Dotted links: `-.->`, `-..->`, `-.-`, `-. text .->`.
    if let Some(r) = s.strip_prefix("-.") {
        let r2 = r.trim_start_matches(['.', '-']);
        if let Some(rest) = r2.strip_prefix('>') {
            return Some((None, rest)); // -.-> and friends
        }
        if let Some(pos) = r.find(".->") {
            return Some((Some(clean_label(&r[..pos])), &r[pos + 3..]));
        }
        if let Some(pos) = r.find("-.-") {
            return Some((Some(clean_label(&r[..pos])), &r[pos + 3..]));
        }
        if r2.len() < r.len() && !r2.starts_with('>') {
            return Some((None, r2)); // -.- open link
        }
        return None;
    }

    // Thick links: `==>`, `===`, `== text ==>`.
    if s.starts_with("==") {
        let r = s.trim_start_matches('=');
        if let Some(rest) = r.strip_prefix('>') {
            return Some((None, rest));
        }
        if let Some(after) = s.strip_prefix("==") {
            if let Some(pos) = after.find("==>") {
                return Some((Some(clean_label(&after[..pos])), &after[pos + 3..]));
            }
        }
        return Some((None, r)); // === open link
    }

    // Dash links: `-->`, `--->`, `---`, `-- text -->`.
    if s.starts_with("--") {
        let dashes = s.len() - s.trim_start_matches('-').len();
        let after_dashes = &s[dashes..];
        if let Some(rest) = after_dashes.strip_prefix('>') {
            return Some((None, rest));
        }
        if dashes >= 3 {
            return Some((None, after_dashes)); // --- open link
        }
        let after = &s[2..];
        if let Some(pos) = after.find("-->") {
            return Some((Some(clean_label(&after[..pos])), &after[pos + 3..]));
        }
        if let Some(pos) = after.find("---") {
            return Some((Some(clean_label(&after[..pos])), &after[pos + 3..]));
        }
        return None;
    }

    None
}

/// Optional `|label|` after an arrow.
fn take_pipe_label(s: &str) -> (Option<String>, &str) {
    let t = s.trim_start();
    if let Some(r) = t.strip_prefix('|') {
        if let Some(pos) = r.find('|') {
            return (Some(clean_label(&r[..pos])), &r[pos + 1..]);
        }
    }
    (None, s)
}

fn flowchart(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    // Nodes in first-reference order; declarations update label/shape.
    let mut nodes: Vec<(String, Option<(String, &'static str)>)> = Vec::new();
    let mut edges: Vec<(String, String, Option<String>)> = Vec::new();

    let register = |nodes: &mut Vec<(String, Option<(String, &'static str)>)>, term: FlowTerm| {
        match nodes.iter_mut().find(|(id, _)| *id == term.id) {
            Some((_, decl)) => {
                if term.decl.is_some() {
                    *decl = term.decl;
                }
            }
            None => nodes.push((term.id, term.decl)),
        }
    };

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if keyword(line, "subgraph").is_some() || line == "end" {
            note(notes, idx, "subgraph");
            continue;
        }
        if ["direction", "classDef", "class", "style", "linkStyle", "click"]
            .iter()
            .any(|kw| keyword(line, kw).is_some())
        {
            note(notes, idx, line.split_whitespace().next().unwrap_or("directive"));
            continue;
        }
        if line.contains(" & ") {
            note(notes, idx, "`&` node list");
            continue;
        }

        // A chain of node terms joined by links: `A[x] --> B --> C`.
        let Some((first, mut rest)) = take_flow_term(line) else {
            note(notes, idx, line.split_whitespace().next().unwrap_or("statement"));
            continue;
        };
        let mut prev = first.id.clone();
        register(&mut nodes, first);
        let mut parsed_chain = true;
        loop {
            let t = rest.trim_start();
            if t.is_empty() {
                break;
            }
            let Some((mut label, after_arrow)) = take_flow_arrow(t) else {
                parsed_chain = false;
                break;
            };
            let (pipe, after_label) = take_pipe_label(after_arrow);
            if pipe.is_some() {
                label = pipe;
            }
            let Some((term, after_term)) = take_flow_term(after_label) else {
                parsed_chain = false;
                break;
            };
            edges.push((prev.clone(), term.id.clone(), label.filter(|l| !l.is_empty())));
            prev = term.id.clone();
            register(&mut nodes, term);
            rest = after_term;
        }
        if !parsed_chain {
            note(notes, idx, "unrecognized link syntax");
        }
    }

    let mut out = String::new();
    for (id, decl) in &nodes {
        if let Some((label, shape)) = decl {
            let label = if label.is_empty() { id.as_str() } else { label.as_str() };
            if *shape == "box" {
                out.push_str(&format!("{id}: {label}\n"));
            } else {
                out.push_str(&format!("{id} [{shape}]: {label}\n"));
            }
        }
    }
    for (from, to, label) in &edges {
        match label {
            Some(l) => out.push_str(&format!("{from} -> {to}: {l}\n")),
            None => out.push_str(&format!("{from} -> {to}\n")),
        }
    }
    out
}

// ------------------------------------------------------------------
// sequenceDiagram
// ------------------------------------------------------------------

/// Mermaid message arrows, longest first. All map to solid (`->`) or
/// dashed (`-->`) native messages.
const SEQ_ARROWS: &[(&str, bool)] = &[
    ("-->>", true),
    ("->>", false),
    ("--)", true),
    ("-)", false),
    ("--x", true),
    ("-x", false),
    ("-->", true),
    ("->", false),
];

fn sequence(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }

        if let Some(rest) = keyword(line, "participant").or_else(|| keyword(line, "actor")) {
            // `participant X` / `participant X as Display Name`.
            let (id, after) = match rest.split_once(" as ") {
                Some((id, label)) => (id, Some(label)),
                None => (rest, None),
            };
            let id = safe_id(unquote(id));
            match after {
                Some(label) => out.push_str(&format!("actor {id}: {}\n", clean_label(label))),
                None => out.push_str(&format!("actor {id}\n")),
            }
            continue;
        }
        if let Some(rest) = keyword(line, "activate") {
            out.push_str(&format!("activate {}\n", safe_id(rest)));
            continue;
        }
        if let Some(rest) = keyword(line, "deactivate") {
            out.push_str(&format!("deactivate {}\n", safe_id(rest)));
            continue;
        }
        if line == "autonumber" || keyword(line, "title").is_some() {
            note(notes, idx, line.split_whitespace().next().unwrap_or("directive"));
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if ["note", "loop", "alt", "opt", "par", "and", "else", "end", "rect", "box", "critical", "break"]
            .iter()
            .any(|kw| keyword(&lower, kw).is_some() || lower == *kw)
        {
            note(notes, idx, lower.split_whitespace().next().unwrap_or("frame").to_string());
            continue;
        }

        // Message: `A->>B: text`, with optional `+`/`-` activation shorthand.
        let (from_raw, rest) = take_id(line);
        let mut matched = false;
        if !from_raw.is_empty() {
            let t = rest.trim_start();
            for (arrow, dashed) in SEQ_ARROWS {
                if let Some(after) = t.strip_prefix(arrow) {
                    let after = after.trim_start();
                    let (sign, after) = match after.strip_prefix('+') {
                        Some(r) => (Some('+'), r),
                        None => match after.strip_prefix('-') {
                            Some(r) => (Some('-'), r),
                            None => (None, after),
                        },
                    };
                    let (to_raw, tail) = take_id(after.trim_start());
                    if to_raw.is_empty() {
                        break;
                    }
                    let (from, to) = (safe_id(from_raw), safe_id(to_raw));
                    let msg = tail.trim_start().strip_prefix(':').map(|m| clean_label(m));
                    let native_arrow = if *dashed { "-->" } else { "->" };
                    match &msg {
                        Some(m) if !m.is_empty() => {
                            out.push_str(&format!("{from} {native_arrow} {to}: {m}\n"));
                        }
                        _ => out.push_str(&format!("{from} {native_arrow} {to}\n")),
                    }
                    // `+` activates the recipient, `-` deactivates the sender.
                    match sign {
                        Some('+') => out.push_str(&format!("activate {to}\n")),
                        Some('-') => out.push_str(&format!("deactivate {from}\n")),
                        _ => {}
                    }
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            note(notes, idx, "unrecognized statement");
        }
    }

    out
}

// ------------------------------------------------------------------
// classDiagram
// ------------------------------------------------------------------

/// Mermaid class relations, longest first: token → (native arrow, reversed).
/// `reversed` swaps endpoints so the native arrow decorates the same class
/// mermaid decorates (e.g. `A <|-- B` means B inherits A → `B ^-> A`).
const CLASS_RELATIONS: &[(&str, &str, bool)] = &[
    ("<|..", "^->", true),
    ("..|>", "^->", false),
    ("<|--", "^->", true),
    ("--|>", "^->", false),
    ("*--", "*->", false),
    ("--*", "*->", true),
    ("o--", "o->", false),
    ("--o", "o->", true),
    ("<--", "->", true),
    ("-->", "->", false),
    ("<..", "->", true),
    ("..>", "->", false),
    ("--", "->", false),
    ("..", "->", false),
];

/// Translate one mermaid member (`+String name`, `+save() void`) into a
/// native class member.
fn class_member(raw: &str) -> Option<String> {
    let m = raw.trim();
    if m.is_empty() {
        return None;
    }
    let (vis, rest) = match m.chars().next() {
        Some(v @ ('+' | '-' | '#')) => (Some(v), m[1..].trim_start()),
        Some('~') => (None, m[1..].trim_start()),
        _ => (None, m),
    };
    let text = match rest.find('(') {
        // Method: keep the bare name, normalize to `name()`.
        Some(pos) => format!("{}()", rest[..pos].trim_end()),
        None => rest.replace(',', ";"),
    };
    if text.is_empty() || text == "()" {
        return None;
    }
    match vis {
        Some(v) => Some(format!("{v}{text}")),
        None => Some(text),
    }
}

fn class(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    // Classes in first-reference order: (name, stereotype, members).
    let mut classes: Vec<(String, Option<&'static str>, Vec<String>)> = Vec::new();
    let mut relations: Vec<String> = Vec::new();
    let mut open: Option<usize> = None; // index into `classes` for `class X {`

    let ensure = |classes: &mut Vec<(String, Option<&'static str>, Vec<String>)>, name: &str| {
        match classes.iter().position(|(n, _, _)| n == name) {
            Some(i) => i,
            None => {
                classes.push((name.to_string(), None, Vec::new()));
                classes.len() - 1
            }
        }
    };

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }

        // Inside a `class X { … }` body.
        if let Some(ci) = open {
            if line == "}" {
                open = None;
                continue;
            }
            if let Some(st) = line.strip_prefix("<<").and_then(|r| r.strip_suffix(">>")) {
                match st.trim().to_ascii_lowercase().as_str() {
                    "enumeration" | "enum" => classes[ci].1 = Some("enum"),
                    "interface" | "trait" => classes[ci].1 = Some("trait"),
                    other => note(notes, idx, format!("stereotype <<{other}>>")),
                }
                continue;
            }
            match class_member(line) {
                Some(m) => classes[ci].2.push(m),
                None => note(notes, idx, "member"),
            }
            continue;
        }

        if let Some(rest) = keyword(line, "class") {
            let (name, after) = take_id(rest);
            if name.is_empty() {
                note(notes, idx, "class");
                continue;
            }
            let ci = ensure(&mut classes, &safe_id(name));
            if after.trim() == "{" {
                open = Some(ci);
            } else if !after.trim().is_empty() {
                note(notes, idx, "class annotation");
            }
            continue;
        }
        if ["direction", "namespace", "note", "style", "classDef"]
            .iter()
            .any(|kw| keyword(line, kw).is_some())
        {
            note(notes, idx, line.split_whitespace().next().unwrap_or("directive"));
            continue;
        }

        // Relation line: `A <|-- B : label` (quoted multiplicities dropped).
        let mut matched = false;
        for (token, native, reversed) in CLASS_RELATIONS {
            if let Some(pos) = line.find(token) {
                let left = line[..pos].trim().trim_matches('"').trim();
                let right_all = line[pos + token.len()..].trim();
                let (right_part, label) = match right_all.split_once(':') {
                    Some((r, l)) => (r.trim(), Some(clean_label(l))),
                    None => (right_all, None),
                };
                // Drop quoted multiplicity tokens hugging the relation.
                let left = left.split('"').next().unwrap_or(left).trim();
                let right = right_part.rsplit('"').next().unwrap_or(right_part).trim();
                let (l_id, _) = take_id(left);
                let (r_id, _) = take_id(right);
                if l_id.is_empty() || r_id.is_empty() {
                    break;
                }
                let (from, to) = if *reversed {
                    (safe_id(r_id), safe_id(l_id))
                } else {
                    (safe_id(l_id), safe_id(r_id))
                };
                ensure(&mut classes, &from);
                ensure(&mut classes, &to);
                match label.filter(|l| !l.is_empty()) {
                    Some(l) => relations.push(format!("{from} {native} {to}: {l}")),
                    None => relations.push(format!("{from} {native} {to}")),
                }
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }

        // One-line member: `X : +field`.
        if let Some((name, member)) = line.split_once(':') {
            let (id, rest) = take_id(name.trim());
            if !id.is_empty() && rest.trim().is_empty() {
                let ci = ensure(&mut classes, &safe_id(id));
                match class_member(member) {
                    Some(m) => classes[ci].2.push(m),
                    None => note(notes, idx, "member"),
                }
                continue;
            }
        }

        note(notes, idx, "unrecognized statement");
    }

    let mut out = String::new();
    for (name, stereotype, members) in &classes {
        let mut parts: Vec<String> = Vec::new();
        if let Some(st) = stereotype {
            parts.push((*st).to_string());
        }
        parts.extend(members.iter().cloned());
        if parts.is_empty() {
            // Only emit a declaration when the class never appears in a
            // relation (relations auto-declare endpoints).
            if !relations.iter().any(|r| {
                r.split_whitespace().next() == Some(name.as_str())
                    || r.split([' ', ':']).nth(2) == Some(name.as_str())
            }) {
                out.push_str(&format!("{name}:\n"));
            }
        } else {
            out.push_str(&format!("{name}: {}\n", parts.join(", ")));
        }
    }
    for r in &relations {
        out.push_str(r);
        out.push('\n');
    }
    out
}

// ------------------------------------------------------------------
// stateDiagram / stateDiagram-v2
// ------------------------------------------------------------------

fn state(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if line == "}" || line == "--" {
            continue; // composite close / concurrency separator
        }
        if let Some(rest) = keyword(line, "state") {
            // `state "Description" as s1` → a native state declaration.
            if let Some((desc, id)) = rest.split_once(" as ") {
                out.push_str(&format!("{}: {}\n", safe_id(id), clean_label(desc)));
                continue;
            }
            // `state X {` opens a composite — flatten its transitions.
            note(notes, idx, "composite state");
            continue;
        }
        if keyword(line, "note").is_some() || keyword(line, "direction").is_some() {
            note(notes, idx, line.split_whitespace().next().unwrap_or("directive"));
            continue;
        }
        if line.contains("<<") {
            note(notes, idx, "state annotation");
            continue;
        }

        // Transition: `A --> B : ev` (also `[*]` endpoints).
        if let Some(pos) = line.find("-->") {
            let from = line[..pos].trim();
            let rest = line[pos + 3..].trim();
            let (to, label) = match rest.split_once(':') {
                Some((t, l)) => (t.trim(), Some(clean_label(l))),
                None => (rest, None),
            };
            let from_t = if from == "[*]" { "[*]".to_string() } else { safe_id(from) };
            let to_t = if to == "[*]" { "[*]".to_string() } else { safe_id(to) };
            match label.filter(|l| !l.is_empty()) {
                Some(l) => out.push_str(&format!("{from_t} -> {to_t}: {l}\n")),
                None => out.push_str(&format!("{from_t} -> {to_t}\n")),
            }
            continue;
        }

        // State description: `A : label` (native-identical).
        if let Some((id, label)) = line.split_once(':') {
            let (name, rest) = take_id(id.trim());
            if !name.is_empty() && rest.trim().is_empty() {
                out.push_str(&format!("{}: {}\n", safe_id(name), clean_label(label)));
                continue;
            }
        }

        note(notes, idx, "unrecognized statement");
    }

    out
}

// ------------------------------------------------------------------
// erDiagram
// ------------------------------------------------------------------

/// Map one side of a mermaid ER cardinality to the native `1`/`*`.
fn er_card(side: &str) -> char {
    if side.contains('{') || side.contains('}') {
        '*'
    } else {
        '1'
    }
}

fn er(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    // Entities in first-reference order: (name, fields).
    let mut entities: Vec<(String, Vec<String>)> = Vec::new();
    let mut relations: Vec<String> = Vec::new();
    let mut open: Option<usize> = None;

    let ensure = |entities: &mut Vec<(String, Vec<String>)>, name: &str| {
        match entities.iter().position(|(n, _)| n == name) {
            Some(i) => i,
            None => {
                entities.push((name.to_string(), Vec::new()));
                entities.len() - 1
            }
        }
    };

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }

        if let Some(ei) = open {
            if line == "}" {
                open = None;
                continue;
            }
            // Attribute row: `type name [PK|FK|UK] ["comment"]`.
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }
            let name = if tokens.len() >= 2 { tokens[1] } else { tokens[0] };
            let mut field = safe_id(name);
            for t in &tokens[1..] {
                match t.trim_end_matches(',') {
                    "PK" => field.push_str(" pk"),
                    "FK" => field.push_str(" fk"),
                    "UK" => field.push_str(" unique"),
                    _ => {}
                }
            }
            entities[ei].1.push(field);
            continue;
        }

        // Relation: `A ||--o{ B : label` (also dotted `..`).
        let mut matched = false;
        for conn in ["--", ".."] {
            let Some(pos) = line.find(conn) else { continue };
            let left = line[..pos].trim();
            let right = line[pos + 2..].trim();
            // The two cardinality glyph pairs hug the connector.
            let (l_ent, l_card) = left.split_at(left.len().saturating_sub(2));
            let (r_card, r_ent) = right.split_at(2.min(right.len()));
            let l_ok = l_card.chars().all(|c| "|o}{".contains(c)) && l_card.len() == 2;
            let r_ok = r_card.chars().all(|c| "|o}{".contains(c)) && r_card.len() == 2;
            if !l_ok || !r_ok {
                continue;
            }
            let (l_id, _) = take_id(l_ent.trim());
            let (r_part, label) = match r_ent.split_once(':') {
                Some((r, l)) => (r.trim(), Some(clean_label(l))),
                None => (r_ent.trim(), None),
            };
            let (r_id, _) = take_id(r_part);
            if l_id.is_empty() || r_id.is_empty() {
                continue;
            }
            let (from, to) = (safe_id(l_id), safe_id(r_id));
            ensure(&mut entities, &from);
            ensure(&mut entities, &to);
            let card = format!("{}--{}", er_card(l_card), er_card(r_card));
            match label.filter(|l| !l.is_empty()) {
                Some(l) => relations.push(format!("{from} {card} {to}: {l}")),
                None => relations.push(format!("{from} {card} {to}")),
            }
            matched = true;
            break;
        }
        if matched {
            continue;
        }

        // Entity block open: `A {`.
        if let Some(name) = line.strip_suffix('{') {
            let (id, rest) = take_id(name.trim());
            if !id.is_empty() && rest.trim().is_empty() {
                open = Some(ensure(&mut entities, &safe_id(id)));
                continue;
            }
        }

        note(notes, idx, "unrecognized statement");
    }

    let mut out = String::new();
    for (name, fields) in &entities {
        if !fields.is_empty() {
            out.push_str(&format!("{name}: {}\n", fields.join(", ")));
        }
    }
    for r in &relations {
        out.push_str(r);
        out.push('\n');
    }
    out
}

// ------------------------------------------------------------------
// gantt
// ------------------------------------------------------------------

/// Parse a mermaid gantt duration token (`5d`, `2w`, `36h`, `10`).
fn gantt_duration(tok: &str) -> Option<i64> {
    if let Ok(n) = tok.parse::<i64>() {
        return (n >= 0).then_some(n);
    }
    let (num, unit) = tok.split_at(tok.len().saturating_sub(1));
    let n: i64 = num.parse().ok()?;
    match unit {
        "d" => Some(n),
        "w" => Some(n * 7),
        "h" => Some((n + 23) / 24),
        _ => None,
    }
}

/// Is this token an ISO `YYYY-MM-DD` date?
fn is_iso_date(tok: &str) -> bool {
    let parts: Vec<&str> = tok.split('-').collect();
    parts.len() == 3
        && parts[0].len() == 4
        && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn gantt(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if let Some(rest) = keyword(line, "section") {
            out.push_str(&format!("section {rest}\n"));
            continue;
        }
        if ["title", "dateFormat", "axisFormat", "excludes", "todayMarker", "tickInterval", "weekday"]
            .iter()
            .any(|kw| keyword(line, kw).is_some())
        {
            note(notes, idx, line.split_whitespace().next().unwrap_or("directive"));
            continue;
        }

        // Task: `Label : [tags,] [id,] start, duration`.
        let Some((label, spec)) = line.split_once(':') else {
            note(notes, idx, "unrecognized statement");
            continue;
        };
        let label = label.trim();
        let tokens: Vec<&str> = spec.split(',').map(str::trim).filter(|t| !t.is_empty()).collect();
        if tokens.iter().any(|t| t.starts_with("after ") || *t == "after" || t.starts_with("until ")) {
            note(notes, idx, "`after`/`until` dependency");
            continue;
        }
        let milestone = tokens.contains(&"milestone");
        let start = tokens.iter().find(|t| is_iso_date(t));
        let duration = tokens
            .iter()
            .filter(|t| !is_iso_date(t))
            .find_map(|t| gantt_duration(t));
        match (start, duration) {
            (Some(start), Some(d)) => out.push_str(&format!("{label}: {start}, {d}\n")),
            (Some(start), None) if milestone => out.push_str(&format!("{label}: {start}, 0\n")),
            _ => note(notes, idx, "task without a date + duration"),
        }
    }

    out
}

// ------------------------------------------------------------------
// mindmap
// ------------------------------------------------------------------

/// Extract a mindmap node label, unwrapping shape brackets
/// (`root((Label))`, `id(Label)`, `id[Label]`, `id{{Label}}`).
fn mindmap_label(s: &str) -> String {
    let t = s.trim();
    for (open, close) in [("((", "))"), ("{{", "}}"), ("[", "]"), ("(", ")")] {
        if let Some(start) = t.find(open) {
            let after = &t[start + open.len()..];
            if let Some(end) = after.rfind(close) {
                let inner = &after[..end];
                if !inner.trim().is_empty() {
                    return clean_label(inner);
                }
            }
        }
    }
    clean_label(t)
}

fn mindmap(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    // Normalize arbitrary indentation steps to the native 2-space levels.
    let mut out = String::new();
    let mut stack: Vec<usize> = Vec::new(); // indent widths per open depth

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        if raw.trim().is_empty() || raw.trim().starts_with("%%") {
            continue;
        }
        if raw.trim().starts_with("::icon") {
            note(notes, idx, "::icon");
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        while let Some(&top) = stack.last() {
            if indent < top {
                stack.pop();
            } else {
                break;
            }
        }
        match stack.last() {
            Some(&top) if indent == top => {} // sibling
            _ => stack.push(indent),          // deeper (or first) level
        }
        let depth = stack.len() - 1;
        out.push_str(&"  ".repeat(depth));
        out.push_str(&mindmap_label(raw));
        out.push('\n');
    }

    out
}

// ------------------------------------------------------------------
// pie (→ ::chart pipeline)
// ------------------------------------------------------------------

fn pie(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut rows: Vec<(String, String)> = Vec::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if keyword(line, "title").is_some() {
            note(notes, idx, "title (use the block's title= attribute)");
            continue;
        }
        // Slice: `"Label" : 42`.
        let Some((label, value)) = line.rsplit_once(':') else {
            note(notes, idx, "unrecognized statement");
            continue;
        };
        let value = value.trim();
        if value.parse::<f64>().is_err() {
            note(notes, idx, "non-numeric slice value");
            continue;
        }
        rows.push((clean_label(label), value.to_string()));
    }

    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::from("Slice | Value\n");
    for (label, value) in rows {
        out.push_str(&format!("{label} | {value}\n"));
    }
    out
}

// ------------------------------------------------------------------
// timeline
// ------------------------------------------------------------------

fn timeline(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    // Collect (marker, event) pairs; `: more` continuation lines reuse the
    // previous marker.
    let mut events: Vec<(String, String)> = Vec::new();
    let mut last_marker: Option<String> = None;

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if keyword(line, "title").is_some() || keyword(line, "section").is_some() {
            note(notes, idx, line.split_whitespace().next().unwrap_or("directive"));
            continue;
        }
        let segments: Vec<&str> = line.split(':').map(str::trim).collect();
        if line.starts_with(':') {
            // Continuation: further events under the previous marker.
            let Some(marker) = last_marker.clone() else {
                note(notes, idx, "event before any period");
                continue;
            };
            for seg in segments.iter().filter(|s| !s.is_empty()) {
                events.push((marker.clone(), clean_label(seg)));
            }
            continue;
        }
        if segments.len() < 2 {
            // A bare period with no events — nothing to draw for it.
            note(notes, idx, "period without events");
            continue;
        }
        let marker = clean_label(segments[0]);
        for seg in segments[1..].iter().filter(|s| !s.is_empty()) {
            events.push((marker.clone(), clean_label(seg)));
        }
        last_marker = Some(marker);
    }

    // Native timelines require every marker to be an integer or all-ISO
    // dates; otherwise render the events as plain ordered labels.
    let classify = |m: &str| -> Option<bool> {
        if m.parse::<i64>().is_ok() {
            return Some(false);
        }
        let parts: Vec<&str> = m.split('-').collect();
        ((2..=3).contains(&parts.len())
            && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())))
        .then_some(true)
    };
    let kinds: Vec<Option<bool>> = events.iter().map(|(m, _)| classify(m)).collect();
    let uniform = !kinds.is_empty()
        && kinds.iter().all(|k| k.is_some())
        && kinds.windows(2).all(|w| w[0] == w[1]);

    let mut out = String::new();
    for (marker, label) in &events {
        if uniform {
            out.push_str(&format!("{marker}: {label}\n"));
        } else {
            out.push_str(&format!("{marker} \u{2014} {label}\n"));
        }
    }
    out
}

// ------------------------------------------------------------------
// journey
// ------------------------------------------------------------------

fn journey(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if let Some(rest) = keyword(line, "section") {
            out.push_str(&format!("section {rest}\n"));
            continue;
        }
        if keyword(line, "title").is_some() {
            note(notes, idx, "title (use the block's title= attribute)");
            continue;
        }
        // Task: `Label : score : actor, actor` (actors dropped).
        let parts: Vec<&str> = line.split(':').map(str::trim).collect();
        if parts.len() < 2 {
            note(notes, idx, "unrecognized statement");
            continue;
        }
        let Ok(score) = parts[1].parse::<i64>() else {
            note(notes, idx, "non-numeric score");
            continue;
        };
        out.push_str(&format!("{}: {}\n", parts[0], score.clamp(1, 5)));
    }

    out
}

// ------------------------------------------------------------------
// kanban
// ------------------------------------------------------------------

/// Unwrap the `id[Label]` shorthand used by mermaid kanban columns/cards.
fn kanban_label(s: &str) -> String {
    let t = s.trim();
    if let Some(start) = t.find('[') {
        if let Some(end) = t.rfind(']') {
            if end > start {
                return clean_label(&t[start + 1..end]);
            }
        }
    }
    clean_label(t)
}

fn kanban(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();
    let mut have_column = false;

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        if raw.trim().is_empty() || raw.trim().starts_with("%%") {
            continue;
        }
        if raw.trim().starts_with("@{") {
            note(notes, idx, "card metadata");
            continue;
        }
        let indented = raw.starts_with(' ') || raw.starts_with('\t');
        if indented && have_column {
            out.push_str(&format!("  {}\n", kanban_label(raw)));
        } else {
            out.push_str(&format!("column {}\n", kanban_label(raw)));
            have_column = true;
        }
    }

    out
}

// ------------------------------------------------------------------
// quadrantChart
// ------------------------------------------------------------------

fn quadrant(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if keyword(line, "title").is_some() {
            note(notes, idx, "title (use the block's title= attribute)");
            continue;
        }
        if let Some(rest) = keyword(line, "x-axis").or_else(|| keyword(line, "y-axis")) {
            let axis = &line[..6];
            match rest.split_once("-->") {
                Some((low, high)) => {
                    out.push_str(&format!("{axis} {} --> {}\n", clean_label(low), clean_label(high)));
                }
                None => note(notes, idx, "single-ended axis label"),
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("quadrant-") {
            if rest.is_empty() {
                note(notes, idx, "quadrant label");
                continue;
            }
            // Mermaid writes `quadrant-1 Label` (no colon); accept both.
            let (n, label) = rest.split_at(1);
            let label = label.trim_start().trim_start_matches(':').trim_start();
            out.push_str(&format!("quadrant-{n}: {}\n", clean_label(label)));
            continue;
        }
        // Point: `Name: [0.3, 0.6]`.
        if let Some((name, coords)) = line.split_once(':') {
            let coords = coords.trim();
            if let Some(inner) = coords.strip_prefix('[').and_then(|c| c.split(']').next()) {
                out.push_str(&format!("{}: {}\n", clean_label(name), inner.trim()));
                continue;
            }
        }
        note(notes, idx, "unrecognized statement");
    }

    out
}

// ------------------------------------------------------------------
// gitGraph
// ------------------------------------------------------------------

/// Pull a `key: "value"` option out of a mermaid gitGraph statement.
fn git_option(rest: &str, key: &str) -> Option<String> {
    let pos = rest.find(&format!("{key}:"))?;
    let after = rest[pos + key.len() + 1..].trim_start();
    let quoted = after.strip_prefix('"')?;
    let end = quoted.find('"')?;
    Some(quoted[..end].to_string())
}

fn gitgraph(lines: &[&str], header: usize, notes: &mut Vec<MermaidNote>) -> String {
    let mut out = String::new();

    for (idx, raw) in lines.iter().enumerate().skip(header + 1) {
        let line = raw.trim();
        if is_noise(line) {
            continue;
        }
        if let Some(rest) = keyword(line, "commit").or_else(|| (line == "commit").then_some("")) {
            let label = git_option(rest, "id").or_else(|| git_option(rest, "tag"));
            match label {
                Some(l) => out.push_str(&format!("commit: {l}\n")),
                None => out.push_str("commit\n"),
            }
            continue;
        }
        if let Some(rest) = keyword(line, "branch") {
            // `order: n` suffixes are dropped.
            let name = rest.split_whitespace().next().unwrap_or("");
            if name.is_empty() {
                note(notes, idx, "branch");
            } else {
                out.push_str(&format!("branch {}\n", safe_id(name)));
            }
            continue;
        }
        if let Some(rest) = keyword(line, "checkout").or_else(|| keyword(line, "switch")) {
            out.push_str(&format!("checkout {}\n", safe_id(rest)));
            continue;
        }
        if let Some(rest) = keyword(line, "merge") {
            let name = rest.split_whitespace().next().unwrap_or("");
            if name.is_empty() {
                note(notes, idx, "merge");
                continue;
            }
            match git_option(rest, "id").or_else(|| git_option(rest, "tag")) {
                Some(l) => out.push_str(&format!("merge {}: {l}\n", safe_id(name))),
                None => out.push_str(&format!("merge {}\n", safe_id(name))),
            }
            continue;
        }
        if keyword(line, "cherry-pick").is_some() {
            note(notes, idx, "cherry-pick");
            continue;
        }
        note(notes, idx, "unrecognized statement");
    }

    out
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Render a `::diagram` block body to HTML through the full pipeline.
    fn html_for(body: &str) -> String {
        let source = format!("::diagram\n{body}\n::\n");
        let result = crate::parse::parse(&source);
        result.doc.to_html_fragment()
    }

    fn assert_renders_svg(body: &str, expect_fragment: &str) {
        let html = html_for(body);
        assert!(
            html.contains("<svg"),
            "mermaid body should render SVG, got: {html}"
        );
        assert!(
            !html.contains("surfdoc-diagram-fallback"),
            "mermaid body degraded to prose: {html}"
        );
        assert!(
            html.contains(expect_fragment),
            "expected {expect_fragment:?} in: {html}"
        );
    }

    // ── golden sample per family ────────────────────────────────────

    #[test]
    fn golden_flowchart_renders_svg() {
        assert_renders_svg(
            "flowchart LR\n  A[Start] --> B{Valid?}\n  B -->|yes| C[Save]\n  B -->|no| A",
            "surfdoc-diagram-flowchart",
        );
    }

    #[test]
    fn golden_sequence_renders_svg() {
        assert_renders_svg(
            "sequenceDiagram\n  participant U as User\n  U->>API: POST /login\n  API-->>U: 200 OK",
            "surfdoc-diagram-sequence",
        );
    }

    #[test]
    fn golden_class_renders_svg() {
        assert_renders_svg(
            "classDiagram\n  class Animal {\n    +String name\n    +speak() void\n  }\n  Animal <|-- Dog",
            "surfdoc-diagram-class",
        );
    }

    #[test]
    fn golden_state_renders_svg() {
        assert_renders_svg(
            "stateDiagram-v2\n  [*] --> Idle\n  Idle --> Running : start\n  Running --> [*]",
            "surfdoc-diagram-state",
        );
    }

    #[test]
    fn golden_er_renders_svg() {
        assert_renders_svg(
            "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  ORDER {\n    int id PK\n    int customer_id FK\n  }",
            "surfdoc-diagram-erd",
        );
    }

    #[test]
    fn golden_gantt_renders_svg() {
        assert_renders_svg(
            "gantt\n  dateFormat YYYY-MM-DD\n  section Build\n  Backend :a1, 2026-01-01, 5d\n  Frontend :2026-01-03, 2w",
            "surfdoc-diagram-gantt",
        );
    }

    #[test]
    fn golden_mindmap_renders_svg() {
        assert_renders_svg(
            "mindmap\n  root((Product))\n    Web\n      Landing\n    Mobile",
            "surfdoc-diagram-mindmap",
        );
    }

    #[test]
    fn golden_pie_renders_chart_svg() {
        assert_renders_svg(
            "pie\n  \"Organic\" : 52\n  \"Referral\" : 26\n  \"Paid\" : 22",
            "surfdoc-diagram-pie",
        );
        // Pie forwards to the ::chart pipeline.
        assert!(html_for("pie\n  \"A\" : 1\n  \"B\" : 2").contains("surfdoc-chart-svg"));
    }

    #[test]
    fn golden_timeline_renders_svg() {
        assert_renders_svg(
            "timeline\n  2024 : Prototype\n  2025 : Beta : First customers\n  2026 : Launch",
            "surfdoc-diagram-timeline",
        );
    }

    #[test]
    fn golden_journey_renders_svg() {
        assert_renders_svg(
            "journey\n  title My day\n  section Work\n    Write code: 5: Me\n    Fix bugs: 3: Me",
            "surfdoc-diagram-journey",
        );
    }

    #[test]
    fn golden_kanban_renders_svg() {
        assert_renders_svg(
            "kanban\n  todo[To do]\n    t1[Write spec]\n  done[Done]\n    t2[Ship it]",
            "surfdoc-diagram-kanban",
        );
    }

    #[test]
    fn golden_quadrant_renders_svg() {
        assert_renders_svg(
            "quadrantChart\n  x-axis Low --> High\n  y-axis Slow --> Fast\n  quadrant-1 Invest\n  Point A: [0.8, 0.9]",
            "surfdoc-diagram-quadrant",
        );
    }

    #[test]
    fn golden_gitgraph_renders_svg() {
        assert_renders_svg(
            "gitGraph\n  commit id: \"init\"\n  branch develop\n  commit\n  checkout main\n  merge develop",
            "surfdoc-diagram-gitgraph",
        );
    }

    // ── explicit type=mermaid ───────────────────────────────────────

    #[test]
    fn explicit_mermaid_type_translates() {
        let source = "::diagram[type=mermaid]\nflowchart TD\n  A --> B\n::\n";
        let html = crate::parse::parse(source).doc.to_html_fragment();
        assert!(html.contains("surfdoc-diagram-flowchart"));
        assert!(html.contains("<svg"));
    }

    // ── per-family edge cases ───────────────────────────────────────

    #[test]
    fn flowchart_chains_shapes_and_labels() {
        let t = translate("", "graph TD\nA(Round) --> B --> C{Choice?}\nB -.->|maybe| D([Stadium])").unwrap();
        assert_eq!(t.diagram_type, "flowchart");
        assert!(t.content.contains("A [rounded]: Round"));
        assert!(t.content.contains("C [diamond]: Choice?"));
        assert!(t.content.contains("D [rounded]: Stadium"));
        assert!(t.content.contains("A -> B"));
        assert!(t.content.contains("B -> C"));
        assert!(t.content.contains("B -> D: maybe"));
        assert!(t.notes.is_empty());
    }

    #[test]
    fn flowchart_subgraph_is_skipped_with_note() {
        let t = translate("", "flowchart LR\nsubgraph One\nA --> B\nend\nB --> C").unwrap();
        assert!(t.content.contains("A -> B"));
        assert!(t.content.contains("B -> C"));
        assert_eq!(t.notes.len(), 2); // subgraph + end
        assert!(t.notes.iter().all(|n| n.construct == "subgraph"));
    }

    #[test]
    fn flowchart_mid_edge_text_form() {
        let t = translate("", "graph LR\nA -- uses --> B\nC == big ==> D").unwrap();
        assert!(t.content.contains("A -> B: uses"));
        assert!(t.content.contains("C -> D: big"));
    }

    #[test]
    fn sequence_activation_shorthand() {
        let t = translate("", "sequenceDiagram\nA->>+B: go\nB-->>-A: done").unwrap();
        assert!(t.content.contains("A -> B: go"));
        assert!(t.content.contains("activate B"));
        assert!(t.content.contains("B --> A: done"));
        assert!(t.content.contains("deactivate B"));
    }

    #[test]
    fn sequence_frames_are_noted() {
        let t = translate("", "sequenceDiagram\nA->>B: hi\nloop retry\nB->>A: pong\nend\nNote over A: hmm").unwrap();
        assert!(t.content.contains("A -> B: hi"));
        assert!(t.content.contains("B -> A: pong"));
        assert_eq!(t.notes.len(), 3); // loop, end, Note
    }

    #[test]
    fn class_relations_and_stereotypes() {
        let t = translate(
            "",
            "classDiagram\nclass Shape {\n<<interface>>\n+area() float\n}\nShape <|-- Circle\nCircle *-- Point : has",
        )
        .unwrap();
        assert!(t.content.contains("Shape: trait, +area()"));
        assert!(t.content.contains("Circle ^-> Shape"));
        assert!(t.content.contains("Circle *-> Point: has"));
    }

    #[test]
    fn class_reversed_arrows_and_multiplicities() {
        let t = translate("", "classDiagram\nA \"1\" --> \"*\" B : owns\nC --|> D").unwrap();
        assert!(t.content.contains("A -> B: owns"));
        assert!(t.content.contains("C ^-> D"));
    }

    #[test]
    fn state_descriptions_and_composites() {
        let t = translate(
            "",
            "stateDiagram\nstate \"Waiting for input\" as w\n[*] --> w\nw --> Done : submit\nstate Big {\nDone --> [*]\n}",
        )
        .unwrap();
        assert!(t.content.contains("w: Waiting for input"));
        assert!(t.content.contains("[*] -> w"));
        assert!(t.content.contains("w -> Done: submit"));
        assert!(t.content.contains("Done -> [*]"));
        assert_eq!(t.notes.len(), 1); // composite state opener
    }

    #[test]
    fn er_cardinalities_map() {
        let t = translate(
            "",
            "erDiagram\nA ||--o{ B : has\nC }|--|| D : belongs\nE ||..|| F",
        )
        .unwrap();
        assert!(t.content.contains("A 1--* B: has"));
        assert!(t.content.contains("C *--1 D: belongs"));
        assert!(t.content.contains("E 1--1 F"));
    }

    #[test]
    fn gantt_after_dependency_is_noted() {
        let t = translate(
            "",
            "gantt\ntitle Plan\nsection S\nA :a1, 2026-01-01, 3d\nB :after a1, 5d",
        )
        .unwrap();
        assert!(t.content.contains("A: 2026-01-01, 3"));
        assert!(!t.content.contains("after"));
        assert_eq!(t.notes.len(), 2); // title + after-dependency
    }

    #[test]
    fn timeline_text_periods_stay_plain() {
        let t = translate("", "timeline\nBronze Age : Smelting\nIron Age : Better smelting").unwrap();
        // Mixed/non-numeric periods render as ordered labels, never a parse error.
        assert!(t.content.contains("Bronze Age \u{2014} Smelting"));
        let model = crate::diagram::parse_diagram_source(t.diagram_type, &t.content);
        assert!(model.is_ok());
    }

    #[test]
    fn journey_scores_clamp() {
        let t = translate("", "journey\nsection S\nGreat: 7: Me\nBad: 0: Me").unwrap();
        assert!(t.content.contains("Great: 5"));
        assert!(t.content.contains("Bad: 1"));
    }

    #[test]
    fn quadrant_points_unbracket() {
        let t = translate("", "quadrantChart\nPoint A: [0.25, 0.75]").unwrap();
        assert!(t.content.contains("Point A: 0.25, 0.75"));
    }

    #[test]
    fn gitgraph_options_map_to_labels() {
        let t = translate(
            "",
            "gitGraph\ncommit id: \"init\"\nbranch dev order: 2\ncommit tag: \"v1\"\ncheckout main\nmerge dev id: \"ship\"\ncherry-pick id: \"x\"",
        )
        .unwrap();
        assert!(t.content.contains("commit: init"));
        assert!(t.content.contains("branch dev\n"));
        assert!(t.content.contains("commit: v1"));
        assert!(t.content.contains("merge dev: ship"));
        assert_eq!(t.notes.len(), 1); // cherry-pick
    }

    // ── non-mermaid bodies pass through ─────────────────────────────

    #[test]
    fn native_dsl_is_not_translated() {
        assert!(translate("architecture", "web: Web\nweb -> api").is_none());
        assert!(translate("flowchart", "a: Start\na -> b").is_none());
        assert!(translate("", "not a diagram at all").is_none());
    }

    #[test]
    fn garbage_stays_prose() {
        // A mermaid header whose body translates to nothing falls back.
        let html = html_for("flowchart LR\nsubgraph Only\nend");
        assert!(html.contains("surfdoc-diagram-fallback"));
        assert!(!html.contains("<svg"));
        // Plain prose is untouched.
        let html = html_for("just some words\nand more words");
        assert!(html.contains("surfdoc-diagram-fallback"));
    }

    #[test]
    fn frontmatter_and_comments_are_skipped_by_the_sniff() {
        let t = translate("", "---\ntitle: X\n---\n%% a comment\nflowchart LR\nA --> B").unwrap();
        assert_eq!(t.diagram_type, "flowchart");
        assert!(t.content.contains("A -> B"));
    }
}

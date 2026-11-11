//! The diagram spec: strict parsing, the checks beyond the schema, and the group tree.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::envelope::AppError;
use crate::util::did_you_mean;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagram {
    Flow,
    Layers,
    Sequence,
    Free,
    State,
    Timeline,
    Compare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Right,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Box,
    Service,
    Client,
    Datastore,
    Queue,
    Actor,
    External,
    Decision,
    Document,
    /// State diagrams: a state, and the markers where the machine starts and ends.
    State,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeStyle {
    Sync,
    Async,
    Dependency,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    End,
    Both,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub emphasis: bool,
    pub width: Option<f64>,
    pub at: Option<(f64, f64)>,
    pub detail: Option<String>,
    pub link: Option<String>,
    /// Timelines: the date or moment printed at the node's place on the axis.
    pub when: Option<String>,
    /// Comparisons: nodes on the two sides with the same key share a row.
    pub matches: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
    pub arrow: Arrow,
    pub detail: Option<String>,
    pub link: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: String,
    pub label: Option<String>,
    pub members: Vec<String>,
    pub detail: Option<String>,
    pub link: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub attach: String,
    pub text: String,
    pub side: Side,
}

/// One step of a walk-through: a sentence and the things it is about.
#[derive(Clone, Debug)]
pub struct Step {
    /// Its number in the spec, from 1; kept when a single step is drawn on its own.
    pub number: usize,
    pub text: String,
    pub show: Vec<Ref>,
}

/// What a legend entry explains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    Style(EdgeStyle),
    Emphasis,
    Kind(Kind),
}

/// What an id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ref {
    Node(usize),
    Group(usize),
    Edge(usize),
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub diagram: Diagram,
    pub title: String,
    pub description: Option<String>,
    pub direction: Direction,
    pub theme: Option<String>,
    pub max_width: f64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub groups: Vec<Group>,
    pub notes: Vec<Note>,
    pub steps: Vec<Step>,
    /// Things to keep at full strength while the rest is dimmed; empty dims nothing.
    pub focus: Vec<Ref>,
    pub legend: Vec<(Sample, String)>,
    pub caption: Option<String>,
    /// Set when one frame of the walk-through is drawn: the step it shows.
    pub shown: Option<usize>,
    /// The innermost group holding each node and each group.
    pub node_parent: Vec<Option<usize>>,
    pub group_parent: Vec<Option<usize>>,
    ids: BTreeMap<String, Ref>,
}

impl Spec {
    pub fn lookup(&self, id: &str) -> Option<Ref> {
        self.ids.get(id).copied()
    }

    /// The steps that name this node, edge or group, by number.
    pub fn steps_of(&self, r: Ref) -> Vec<usize> {
        self.steps.iter().filter(|s| s.show.contains(&r)).map(|s| s.number).collect()
    }

    /// Whether something is drawn at full strength. With no focus set, everything is.
    /// A group in focus carries its members; an edge is in focus when it is named or
    /// when both of its ends are.
    pub fn in_focus(&self, r: Ref) -> bool {
        if self.focus.is_empty() || self.focus.contains(&r) {
            return true;
        }
        match r {
            Ref::Edge(i) => {
                let end = |id: &str| self.lookup(id).is_some_and(|x| self.focus.contains(&x) || self.ancestors(x).iter().any(|g| self.focus.contains(&Ref::Group(*g))));
                end(&self.edges[i].from) && end(&self.edges[i].to)
            }
            _ => self.ancestors(r).iter().any(|g| self.focus.contains(&Ref::Group(*g))),
        }
    }

    /// Whether a step's badges are drawn: all of them, or only the frame's own.
    pub fn marks(&self, number: usize) -> bool {
        self.shown.is_none_or(|s| s == number)
    }

    /// The spec as one frame of its walk-through: step `number` marked, with what it shows in focus.
    pub fn frame(&self, number: usize) -> Spec {
        // Every step stays in the spec, so each frame is laid out exactly alike.
        let mut s = self.clone();
        s.focus = s.steps.iter().find(|st| st.number == number).map(|st| st.show.clone()).unwrap_or_default();
        s.shown = Some(number);
        s
    }

    /// The two sides of a comparison, in spec order: each group and its nodes.
    pub fn sides(&self) -> Vec<(usize, Vec<usize>)> {
        (0..self.groups.len()).filter(|g| self.group_parent[*g].is_none()).map(|g| (g, (0..self.nodes.len()).filter(|i| self.node_parent[*i] == Some(g)).collect())).collect()
    }

    /// What a comparison lines nodes up by: the `match` key, or failing that the label.
    pub fn match_key(&self, node: usize) -> &str {
        self.nodes[node].matches.as_deref().unwrap_or(&self.nodes[node].label)
    }

    /// In a comparison, whether a node has no counterpart on the other side.
    pub fn one_sided(&self, node: usize) -> bool {
        self.diagram == Diagram::Compare && !self.sides().iter().filter(|(g, _)| self.node_parent[node] != Some(*g)).any(|(_, ns)| ns.iter().any(|o| self.match_key(*o) == self.match_key(node)))
    }

    /// Groups from the innermost outwards that hold a node or group.
    pub fn ancestors(&self, r: Ref) -> Vec<usize> {
        let mut out = Vec::new();
        let mut cur = match r {
            Ref::Node(i) => self.node_parent[i],
            Ref::Group(i) => self.group_parent[i],
            Ref::Edge(_) => None,
        };
        while let Some(g) = cur {
            out.push(g);
            cur = self.group_parent[g];
        }
        out
    }
}

pub const ROOT_FIELDS: &[&str] = &["version", "diagram", "title", "description", "direction", "theme", "max_width", "nodes", "edges", "groups", "notes", "steps", "focus", "legend", "caption"];
pub const NODE_FIELDS: &[&str] = &["id", "label", "kind", "emphasis", "width", "at", "detail", "link", "when", "match"];
pub const DIAGRAMS: &[&str] = &["flow", "layers", "sequence", "free", "state", "timeline", "compare"];
pub const EDGE_FIELDS: &[&str] = &["id", "from", "to", "label", "style", "arrow", "detail", "link"];
pub const GROUP_FIELDS: &[&str] = &["id", "label", "members", "detail", "link"];
pub const STEP_FIELDS: &[&str] = &["text", "show"];
pub const LEGEND_FIELDS: &[&str] = &["style", "emphasis", "kind", "text"];
pub const MAX_STEPS: usize = 20;
pub const NOTE_FIELDS: &[&str] = &["attach", "text", "side"];
const KINDS: &[&str] = &["box", "service", "client", "datastore", "queue", "actor", "external", "decision", "document", "state", "start", "end"];

fn err(code: &'static str, path: &str, message: String) -> AppError {
    AppError::input(code, message).path(path)
}

fn object<'a>(v: &'a Value, path: &str, allowed: &[&str]) -> Result<&'a Map<String, Value>, AppError> {
    let what = if path.is_empty() { "The spec".to_string() } else { path.to_string() };
    let map = v.as_object().ok_or_else(|| err("invalid-spec", path, format!("{what} must be a mapping.")))?;
    for k in map.keys() {
        if !allowed.contains(&k.as_str()) {
            let at = if path.is_empty() { "the spec root".to_string() } else { path.to_string() };
            let full = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
            return Err(err("unknown-field", &full, format!("Unknown field '{k}' in {at}.")).suggest(did_you_mean(k, allowed.iter().copied())));
        }
    }
    Ok(map)
}

fn join(path: &str, key: &str) -> String {
    if path.is_empty() { key.to_string() } else { format!("{path}.{key}") }
}

fn string(map: &Map<String, Value>, path: &str, key: &str, required: bool, min: usize, max: usize) -> Result<Option<String>, AppError> {
    let p = join(path, key);
    match map.get(key) {
        None | Some(Value::Null) if required => Err(err("missing-field", &p, format!("Missing required field '{key}'{}.", if path.is_empty() { String::new() } else { format!(" in {path}") }))),
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let n = s.chars().count();
            if n < min {
                return Err(err("invalid-spec", &p, format!("{p} must not be empty.")));
            }
            if n > max {
                return Err(err("invalid-spec", &p, format!("{p} is {n} characters long; the limit is {max}.")));
            }
            Ok(Some(s.clone()))
        }
        // YAML reads bare words such as 42 or true as numbers and booleans; accept them where text is meant.
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(Value::Bool(b)) => Ok(Some(b.to_string())),
        Some(_) => Err(err("invalid-spec", &p, format!("{p} must be a string."))),
    }
}

fn choice(map: &Map<String, Value>, path: &str, key: &str, options: &[&'static str]) -> Result<Option<&'static str>, AppError> {
    let p = join(path, key);
    let Some(s) = string(map, path, key, false, 0, 200)? else { return Ok(None) };
    options.iter().copied().find(|o| *o == s).map(Some).ok_or_else(|| {
        err("invalid-value", &p, format!("'{s}' is not a valid {key}; use one of: {}.", options.join(", "))).suggest(did_you_mean(&s, options.iter().copied()))
    })
}

fn kind_of(name: &str) -> Kind {
    match name {
        "service" => Kind::Service,
        "client" => Kind::Client,
        "datastore" => Kind::Datastore,
        "queue" => Kind::Queue,
        "actor" => Kind::Actor,
        "external" => Kind::External,
        "decision" => Kind::Decision,
        "document" => Kind::Document,
        "state" => Kind::State,
        "start" => Kind::Start,
        "end" => Kind::End,
        _ => Kind::Box,
    }
}

fn style_of(name: Option<&str>) -> EdgeStyle {
    match name {
        Some("async") => EdgeStyle::Async,
        Some("dependency") => EdgeStyle::Dependency,
        _ => EdgeStyle::Sync,
    }
}

/// A link a reader may follow: http(s), mailto, or a path or fragment relative to the page.
fn link(map: &Map<String, Value>, path: &str) -> Result<Option<String>, AppError> {
    let Some(s) = string(map, path, "link", false, 1, 500)? else { return Ok(None) };
    let t = s.trim();
    let scheme = t.split_once(':').map(|(a, _)| a).filter(|a| !a.is_empty() && !a.contains(['/', '?', '#']) && a.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)));
    match scheme.map(|a| a.to_ascii_lowercase()).as_deref() {
        // A bare fragment would point into the drawing itself, where nothing answers to it.
        None | Some("http" | "https" | "mailto") if !t.contains(char::is_whitespace) && !t.starts_with('#') => Ok(Some(t.to_string())),
        _ => Err(err("invalid-value", &join(path, "link"), format!("{} is '{t}'; a link is an http, https or mailto address, or a relative path to a page.", join(path, "link")))),
    }
}

fn slug_ok(s: &str) -> bool {
    let mut chars = s.chars();
    s.len() <= 63 && chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The long form of a shorthand edge such as `api -> db: reads`.
pub fn shorthand(text: &str) -> Result<Map<String, Value>, String> {
    let (head, label) = match text.split_once(':') {
        Some((h, l)) => (h, Some(l.trim())),
        None => (text, None),
    };
    let parts: Vec<&str> = head.split_whitespace().collect();
    let [from, arrow, to] = parts[..] else {
        return Err("a short edge is two ids with an arrow between them".into());
    };
    let (style, ends) = match arrow {
        "->" => ("sync", "end"),
        "-->" => ("async", "end"),
        "..>" => ("dependency", "end"),
        "<->" => ("sync", "both"),
        "<-->" => ("async", "both"),
        "--" => ("sync", "none"),
        other => return Err(format!("'{other}' is not an arrow")),
    };
    let mut m = Map::new();
    m.insert("from".into(), Value::String(from.into()));
    m.insert("to".into(), Value::String(to.into()));
    if let Some(l) = label.filter(|l| !l.is_empty()) {
        m.insert("label".into(), Value::String(l.into()));
    }
    if style != "sync" {
        m.insert("style".into(), Value::String(style.into()));
    }
    if ends != "end" {
        m.insert("arrow".into(), Value::String(ends.into()));
    }
    Ok(m)
}

fn slug(map: &Map<String, Value>, path: &str, key: &str, required: bool) -> Result<Option<String>, AppError> {
    let Some(s) = string(map, path, key, required, 1, 63)? else { return Ok(None) };
    if !slug_ok(&s) {
        let fixed: String = s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        return Err(err("invalid-value", &join(path, key), format!("'{s}' is not a valid id; ids are lowercase letters, digits and hyphens, starting with a letter."))
            .suggest(Some(format!("Did you mean '{}'?", fixed.trim_matches('-')))));
    }
    Ok(Some(s))
}

fn list<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a [Value], AppError> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err(err("invalid-spec", key, format!("{key} must be a list."))),
    }
}

/// Read a YAML or JSON spec into a generic document.
pub fn read_document(text: &str, name: &str) -> Result<Value, AppError> {
    let json = text.trim_start().starts_with('{');
    if json {
        serde_json::from_str(text).map_err(|e| AppError::input("malformed-spec", format!("{name} is not valid JSON: {e}.")).path(name))
    } else {
        serde_yaml_ng::from_str::<Value>(text).map_err(|e| AppError::input("malformed-spec", format!("{name} is not valid YAML: {e}.")).path(name))
    }
}

pub fn is_json(text: &str) -> bool {
    text.trim_start().starts_with('{')
}

pub fn parse(doc: &Value) -> Result<Spec, AppError> {
    let root = object(doc, "", ROOT_FIELDS)?;
    match root.get("version") {
        Some(v) if v.as_i64() == Some(1) => {}
        None => return Err(err("missing-field", "version", "Missing required field 'version'.".into()).suggest(Some("Add 'version: 1'.".into()))),
        Some(v) => return Err(err("invalid-value", "version", format!("Spec version {v} is not supported; this build reads version 1."))),
    }
    let diagram = match choice(root, "", "diagram", DIAGRAMS)? {
        Some("flow") => Diagram::Flow,
        Some("layers") => Diagram::Layers,
        Some("free") => Diagram::Free,
        Some("state") => Diagram::State,
        Some("timeline") => Diagram::Timeline,
        Some("compare") => Diagram::Compare,
        Some(_) => Diagram::Sequence,
        None => return Err(err("missing-field", "diagram", "Missing required field 'diagram'.".into()).suggest(Some(format!("Use one of: {}.", DIAGRAMS.join(", "))))),
    };
    let title = string(root, "", "title", true, 1, 120)?.unwrap();
    let description = string(root, "", "description", false, 0, 500)?;
    let direction = match choice(root, "", "direction", &["right", "down"])? {
        Some("down") => Direction::Down,
        _ => Direction::Right,
    };
    let theme = string(root, "", "theme", false, 1, 60)?;
    let max_width = match root.get("max_width") {
        None | Some(Value::Null) => 960.0,
        Some(v) => match v.as_i64() {
            Some(n) if (200..=2000).contains(&n) => n as f64,
            _ => return Err(err("invalid-value", "max_width", "max_width must be a whole number from 200 to 2000.".into())),
        },
    };

    let mut nodes = Vec::new();
    let raw_nodes = list(root, "nodes")?;
    for (i, v) in raw_nodes.iter().enumerate() {
        let p = format!("nodes[{i}]");
        let m = object(v, &p, NODE_FIELDS)?;
        // In a state diagram a node is a state unless it says otherwise.
        let kind = kind_of(choice(m, &p, "kind", KINDS)?.unwrap_or(if diagram == Diagram::State { "state" } else { "box" }));
        if matches!(kind, Kind::Start | Kind::End) && diagram != Diagram::State {
            return Err(err("invalid-value", &join(&p, "kind"), format!("{p}.kind is a start or end marker, which belongs to a state diagram.")).suggest(Some("Set 'diagram: state', or use another kind.".into())));
        }
        for (key, home, name) in [("when", Diagram::Timeline, "timeline"), ("match", Diagram::Compare, "compare")] {
            if m.get(key).is_some_and(|v| !v.is_null()) && diagram != home {
                return Err(err("invalid-spec", &join(&p, key), format!("{p}.{key} is only used in a {name} diagram.")).suggest(Some(format!("Remove '{key}', or set 'diagram: {name}'."))));
            }
        }
        // The start and end markers are dots; they may go without a label.
        let marker = matches!(kind, Kind::Start | Kind::End);
        let emphasis = match m.get("emphasis") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(err("invalid-value", &join(&p, "emphasis"), format!("{p}.emphasis must be true or false."))),
        };
        let width = match m.get("width") {
            None | Some(Value::Null) => None,
            Some(v) => match v.as_f64() {
                Some(w) if w >= 40.0 => Some(w),
                _ => return Err(err("invalid-value", &join(&p, "width"), format!("{p}.width must be a number of at least 40."))),
            },
        };
        let at = match m.get("at") {
            None | Some(Value::Null) => None,
            Some(Value::Array(a)) if a.len() == 2 && a.iter().all(|n| n.is_number()) => Some((a[0].as_f64().unwrap(), a[1].as_f64().unwrap())),
            Some(_) => return Err(err("invalid-value", &join(&p, "at"), format!("{p}.at must be two numbers: [x, y]."))),
        };
        nodes.push(Node {
            id: slug(m, &p, "id", true)?.unwrap(),
            label: if marker { String::new() } else { string(m, &p, "label", true, 1, 80)?.unwrap() },
            kind,
            emphasis,
            width,
            at,
            detail: string(m, &p, "detail", false, 0, 300)?.filter(|s| !s.trim().is_empty()),
            link: link(m, &p)?,
            when: string(m, &p, "when", false, 0, 30)?.filter(|s| !s.trim().is_empty()),
            matches: string(m, &p, "match", false, 1, 80)?,
        });
    }

    let mut edges = Vec::new();
    for (i, v) in list(root, "edges")?.iter().enumerate() {
        let p = format!("edges[{i}]");
        let short;
        let m = match v {
            Value::String(text) => {
                short = shorthand(text).map_err(|why| err("invalid-value", &p, format!("{p} is '{text}': {why}.")).suggest(Some("Write 'from -> to: label', with spaces around the arrow. Arrows: ->, -->, ..>, <->, <-->, --.".into())))?;
                &short
            }
            _ => object(v, &p, EDGE_FIELDS)?,
        };
        let (from, to) = (slug(m, &p, "from", true)?.unwrap(), slug(m, &p, "to", true)?.unwrap());
        let style = style_of(choice(m, &p, "style", &["sync", "async", "dependency"])?);
        let arrow = match choice(m, &p, "arrow", &["end", "both", "none"])? {
            Some("both") => Arrow::Both,
            Some("none") => Arrow::None,
            _ => Arrow::End,
        };
        let id = slug(m, &p, "id", false)?.unwrap_or_else(|| format!("{from}--{to}"));
        edges.push(Edge { id, from, to, label: string(m, &p, "label", false, 0, 40)?.filter(|s| !s.trim().is_empty()), style, arrow, detail: string(m, &p, "detail", false, 0, 300)?.filter(|s| !s.trim().is_empty()), link: link(m, &p)? });
    }

    let mut groups = Vec::new();
    for (i, v) in list(root, "groups")?.iter().enumerate() {
        let p = format!("groups[{i}]");
        let m = object(v, &p, GROUP_FIELDS)?;
        let raw = match m.get("members") {
            Some(Value::Array(a)) if !a.is_empty() => a,
            _ => return Err(err("missing-field", &join(&p, "members"), format!("{p} needs a non-empty 'members' list."))),
        };
        let mut members: Vec<String> = Vec::new();
        for (k, mv) in raw.iter().enumerate() {
            let mp = format!("{p}.members[{k}]");
            let s = mv.as_str().ok_or_else(|| err("invalid-spec", &mp, format!("{mp} must be an id.")))?;
            if members.iter().any(|x| x == s) {
                return Err(err("invalid-spec", &mp, format!("{mp} repeats '{s}'.")));
            }
            members.push(s.to_string());
        }
        groups.push(Group { id: slug(m, &p, "id", true)?.unwrap(), label: string(m, &p, "label", false, 0, 60)?.filter(|s| !s.trim().is_empty()), members, detail: string(m, &p, "detail", false, 0, 300)?.filter(|s| !s.trim().is_empty()), link: link(m, &p)? });
    }

    let mut notes = Vec::new();
    for (i, v) in list(root, "notes")?.iter().enumerate() {
        let p = format!("notes[{i}]");
        let m = object(v, &p, NOTE_FIELDS)?;
        let side = match choice(m, &p, "side", &["top", "right", "bottom", "left"])? {
            Some("top") => Side::Top,
            Some("bottom") => Side::Bottom,
            Some("left") => Side::Left,
            _ => Side::Right,
        };
        notes.push(Note { attach: slug(m, &p, "attach", true)?.unwrap(), text: string(m, &p, "text", true, 1, 120)?.unwrap(), side });
    }

    // ---- nodes that are only mentioned: an id used by an edge or a group but never declared
    {
        let declared: BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).chain(groups.iter().map(|g| g.id.clone())).collect();
        let edge_ids: BTreeSet<&str> = edges.iter().map(|e| e.id.as_str()).collect();
        let mentions = edges.iter().enumerate().flat_map(|(i, e)| [(e.from.clone(), format!("edges[{i}].from")), (e.to.clone(), format!("edges[{i}].to"))]).chain(groups.iter().enumerate().flat_map(|(gi, g)| g.members.iter().enumerate().map(move |(k, m)| (m.clone(), format!("groups[{gi}].members[{k}]")))));
        let mut added: BTreeSet<String> = BTreeSet::new();
        for (id, path) in mentions.collect::<Vec<_>>() {
            if declared.contains(&id) || edge_ids.contains(id.as_str()) || added.contains(&id) {
                continue;
            }
            // One letter away from a declared id is far more often a typo than a new node.
            if id.chars().count() >= 5
                && let Some(near) = declared.iter().find(|d| crate::util::levenshtein(d, &id) == 1)
            {
                return Err(err("unknown-reference", &path, format!("{path} names '{id}', which is not defined but is one letter away from '{near}'.")).suggest(Some(format!("Did you mean '{near}'? If '{id}' is a separate node, declare it under nodes."))));
            }
            if slug_ok(&id) {
                nodes.push(Node { label: id.replace('-', " "), id: id.clone(), kind: if diagram == Diagram::State { Kind::State } else { Kind::Box }, emphasis: false, width: None, at: None, detail: None, link: None, when: None, matches: None });
                added.insert(id);
            }
        }
    }
    if nodes.is_empty() {
        return Err(err("missing-field", "nodes", "A spec needs at least one node: list nodes, or name them in edges.".into()));
    }

    // ---- checks beyond the schema
    let mut ids: BTreeMap<String, Ref> = BTreeMap::new();
    let mut claim = |id: &str, r: Ref, path: String, hint: Option<&str>| -> Result<(), AppError> {
        if ids.insert(id.to_string(), r).is_some() {
            return Err(err("duplicate-id", &path, format!("Id '{id}' is used twice; ids are unique across nodes, groups and edges.")).suggest(hint.map(|s| s.to_string())));
        }
        Ok(())
    };
    for (i, n) in nodes.iter().enumerate() {
        claim(&n.id, Ref::Node(i), format!("nodes[{i}].id"), None)?;
    }
    for (i, g) in groups.iter().enumerate() {
        claim(&g.id, Ref::Group(i), format!("groups[{i}].id"), None)?;
    }
    for (i, e) in edges.iter().enumerate() {
        claim(&e.id, Ref::Edge(i), format!("edges[{i}].id"), Some("Give parallel edges their own 'id'."))?;
    }
    let known = |id: &str, path: String, edges_ok: bool| -> Result<Ref, AppError> {
        match ids.get(id) {
            Some(Ref::Edge(_)) if !edges_ok => Err(err("unknown-reference", &path, format!("{path} names the edge '{id}'; a node or group id is needed."))),
            Some(r) => Ok(*r),
            None => {
                let pool = ids.iter().filter(|(_, r)| edges_ok || !matches!(r, Ref::Edge(_))).map(|(k, _)| k.as_str());
                Err(err("unknown-reference", &path, format!("{path} names '{id}', which is not defined.")).suggest(did_you_mean(id, pool)))
            }
        }
    };
    for (i, e) in edges.iter().enumerate() {
        for (id, end) in [(&e.from, "from"), (&e.to, "to")] {
            let r = known(id, format!("edges[{i}].{end}"), false)?;
            if diagram == Diagram::Sequence && !matches!(r, Ref::Node(_)) {
                return Err(err("invalid-value", &format!("edges[{i}].{end}"), format!("edges[{i}].{end} names the group '{id}'; in a sequence diagram a message joins two nodes.")));
            }
        }
    }
    for (i, n) in notes.iter().enumerate() {
        known(&n.attach, format!("notes[{i}].attach"), true)?;
    }
    // Direct listings: which groups name each node or group as a member.
    let mut listed: BTreeMap<Ref, Vec<usize>> = BTreeMap::new();
    for (gi, g) in groups.iter().enumerate() {
        for (k, m) in g.members.iter().enumerate() {
            let r = known(m, format!("groups[{gi}].members[{k}]"), false)?;
            listed.entry(r).or_default().push(gi);
        }
    }
    // Membership must not loop.
    let child_groups = |g: usize| -> Vec<usize> { groups[g].members.iter().filter_map(|m| match ids.get(m) { Some(Ref::Group(c)) => Some(*c), _ => None }).collect() };
    fn visit(g: usize, state: &mut Vec<u8>, kids: &dyn Fn(usize) -> Vec<usize>) -> Option<usize> {
        state[g] = 1;
        for c in kids(g) {
            if state[c] == 1 {
                return Some(c);
            }
            if state[c] == 0
                && let Some(x) = visit(c, state, kids)
            {
                return Some(x);
            }
        }
        state[g] = 2;
        None
    }
    let mut state = vec![0u8; groups.len()];
    for g in 0..groups.len() {
        if state[g] == 0
            && let Some(c) = visit(g, &mut state, &child_groups)
        {
            return Err(err("group-cycle", &format!("groups[{c}].members"), format!("Group membership forms a cycle through '{}'.", groups[c].id)));
        }
    }
    // Every group above a group, through any chain of memberships.
    let mut above: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); groups.len()];
    #[allow(clippy::needless_range_loop)] // `above[g]` is filled while other entries are read
    for g in 0..groups.len() {
        let mut stack = listed.get(&Ref::Group(g)).cloned().unwrap_or_default();
        while let Some(p) = stack.pop() {
            if above[g].insert(p) {
                stack.extend(listed.get(&Ref::Group(p)).cloned().unwrap_or_default());
            }
        }
    }
    let parent = |r: Ref, name: &str| -> Result<Option<usize>, AppError> {
        let Some(ls) = listed.get(&r) else { return Ok(None) };
        for (a, b) in ls.iter().flat_map(|a| ls.iter().map(move |b| (*a, *b))).filter(|(a, b)| a < b) {
            if !above[a].contains(&b) && !above[b].contains(&a) {
                return Err(err("group-overlap", &format!("groups[{b}].members"), format!("'{name}' belongs to both '{}' and '{}', which are not nested.", groups[a].id, groups[b].id))
                    .suggest(Some("List it in one group, or make one group a member of the other.".into())));
            }
        }
        // The innermost lister is the one every other lister sits above.
        Ok(ls.iter().copied().max_by_key(|g| above[*g].len()))
    };
    let mut node_parent = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        node_parent.push(parent(Ref::Node(i), &n.id)?);
    }
    let mut group_parent = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        group_parent.push(parent(Ref::Group(i), &g.id)?);
    }
    for (i, n) in nodes.iter().enumerate() {
        match (diagram, n.at) {
            (Diagram::Free, None) => {
                return Err(err("missing-field", &format!("nodes[{i}].at"), format!("Node '{}' needs 'at: [x, y]' in a free diagram.", n.id)));
            }
            (d, Some(_)) if d != Diagram::Free => {
                return Err(err("at-outside-free", &format!("nodes[{i}].at"), format!("Node '{}' sets 'at', which is allowed only in a free diagram.", n.id))
                    .suggest(Some("Remove 'at', or set 'diagram: free'.".into())));
            }
            _ => {}
        }
    }

    // ---- what each diagram type asks of the spec
    match diagram {
        Diagram::Timeline => {
            if let Some(g) = group_parent.iter().position(|p| p.is_some()) {
                return Err(err("invalid-spec", &format!("groups[{g}]"), format!("Group '{}' is inside another group; a timeline's phases are not nested.", groups[g].id)));
            }
            if !edges.is_empty() {
                return Err(err("invalid-spec", "edges", "A timeline has no edges: its nodes follow one another in the order they are listed.".into()).suggest(Some("Remove 'edges', or use 'diagram: flow'.".into())));
            }
            if !notes.is_empty() {
                return Err(err("invalid-spec", "notes", "A timeline has no notes; say it in the node's label, or in 'steps'.".into()));
            }
        }
        Diagram::Compare => {
            let tops: Vec<usize> = (0..groups.len()).filter(|g| group_parent[*g].is_none()).collect();
            if tops.len() != 2 || groups.len() != 2 {
                return Err(err("invalid-spec", "groups", format!("A comparison needs exactly two groups, one per side, with no groups inside them; this spec has {}.", groups.len())).suggest(Some("Make one group for 'before' and one for 'after', and list every node in one of them.".into())));
            }
            if let Some(i) = node_parent.iter().position(|p| p.is_none()) {
                return Err(err("invalid-spec", &format!("nodes[{i}]"), format!("Node '{}' is on neither side; in a comparison every node belongs to one of the two groups.", nodes[i].id)));
            }
            if !notes.is_empty() {
                return Err(err("invalid-spec", "notes", "A comparison has no notes; say it in 'steps' or the caption.".into()));
            }
        }
        _ => {}
    }

    // ---- the explanation layer: steps, focus, legend, caption
    let id_list = |v: Option<&Value>, path: &str| -> Result<Vec<Ref>, AppError> {
        let items = match v {
            None | Some(Value::Null) => return Ok(Vec::new()),
            Some(Value::Array(a)) => a,
            Some(_) => return Err(err("invalid-spec", path, format!("{path} must be a list of ids."))),
        };
        let mut out = Vec::new();
        for (k, item) in items.iter().enumerate() {
            let ip = format!("{path}[{k}]");
            let id = item.as_str().ok_or_else(|| err("invalid-spec", &ip, format!("{ip} must be an id.")))?;
            let r = known(id, ip, true)?;
            if !out.contains(&r) {
                out.push(r);
            }
        }
        Ok(out)
    };
    let mut steps = Vec::new();
    for (i, v) in list(root, "steps")?.iter().enumerate() {
        let p = format!("steps[{i}]");
        if i >= MAX_STEPS {
            return Err(err("invalid-spec", "steps", format!("A spec has at most {MAX_STEPS} steps; this one has {}.", list(root, "steps")?.len())).suggest(Some("Split the walk-through over two diagrams.".into())));
        }
        let (text, show) = match v {
            Value::String(t) if !t.trim().is_empty() => (t.clone(), Vec::new()),
            Value::Object(_) => {
                let m = object(v, &p, STEP_FIELDS)?;
                (string(m, &p, "text", true, 1, 200)?.unwrap(), id_list(m.get("show"), &format!("{p}.show"))?)
            }
            _ => return Err(err("invalid-spec", &p, format!("{p} must be a sentence, or a mapping with 'text' and 'show'."))),
        };
        steps.push(Step { number: i + 1, text, show });
    }
    let focus = id_list(root.get("focus"), "focus")?;
    let caption = string(root, "", "caption", false, 0, 300)?.filter(|s| !s.trim().is_empty());
    let legend = match root.get("legend") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => Vec::new(),
        // Explain what the drawing actually uses, and nothing it does not.
        Some(Value::Bool(true)) => {
            let mut auto = Vec::new();
            let styles: BTreeSet<u8> = edges.iter().map(|e| e.style as u8).collect();
            if styles.len() > 1 || styles.iter().any(|s| *s != EdgeStyle::Sync as u8) {
                for (style, text) in [(EdgeStyle::Sync, "call"), (EdgeStyle::Async, "asynchronous message"), (EdgeStyle::Dependency, "dependency")] {
                    if styles.contains(&(style as u8)) {
                        auto.push((Sample::Style(style), text.to_string()));
                    }
                }
            }
            if nodes.iter().any(|n| n.emphasis) {
                auto.push((Sample::Emphasis, "highlighted".to_string()));
            }
            auto
        }
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for (i, v) in items.iter().enumerate() {
                let p = format!("legend[{i}]");
                let m = object(v, &p, LEGEND_FIELDS)?;
                let text = string(m, &p, "text", true, 1, 60)?.unwrap();
                let style = choice(m, &p, "style", &["sync", "async", "dependency"])?;
                let kind = choice(m, &p, "kind", KINDS)?;
                let emphasis = m.get("emphasis").is_some_and(|v| v.as_bool() == Some(true));
                let sample = match (style, kind, emphasis) {
                    (Some(s), None, false) => Sample::Style(style_of(Some(s))),
                    (None, Some(k), false) => Sample::Kind(kind_of(k)),
                    (None, None, true) => Sample::Emphasis,
                    _ => return Err(err("invalid-spec", &p, format!("{p} needs exactly one of 'style', 'kind' or 'emphasis: true', plus 'text'."))),
                };
                out.push((sample, text));
            }
            out
        }
        Some(_) => return Err(err("invalid-spec", "legend", "legend must be true, false, or a list of entries.".into())),
    };

    Ok(Spec { diagram, title, description, direction, theme, max_width, nodes, edges, groups, notes, steps, focus, legend, caption, shown: None, node_parent, group_parent, ids })
}

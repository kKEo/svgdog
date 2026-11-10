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
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub style: EdgeStyle,
    pub arrow: Arrow,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: String,
    pub label: Option<String>,
    pub members: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub attach: String,
    pub text: String,
    pub side: Side,
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
    /// The innermost group holding each node and each group.
    pub node_parent: Vec<Option<usize>>,
    pub group_parent: Vec<Option<usize>>,
    ids: BTreeMap<String, Ref>,
}

impl Spec {
    pub fn lookup(&self, id: &str) -> Option<Ref> {
        self.ids.get(id).copied()
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

pub const ROOT_FIELDS: &[&str] = &["version", "diagram", "title", "description", "direction", "theme", "max_width", "nodes", "edges", "groups", "notes"];
pub const NODE_FIELDS: &[&str] = &["id", "label", "kind", "emphasis", "width", "at"];
pub const EDGE_FIELDS: &[&str] = &["id", "from", "to", "label", "style", "arrow"];
pub const GROUP_FIELDS: &[&str] = &["id", "label", "members"];
pub const NOTE_FIELDS: &[&str] = &["attach", "text", "side"];
const KINDS: &[&str] = &["box", "service", "client", "datastore", "queue", "actor", "external", "decision", "document"];

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

fn slug(map: &Map<String, Value>, path: &str, key: &str, required: bool) -> Result<Option<String>, AppError> {
    let Some(s) = string(map, path, key, required, 1, 63)? else { return Ok(None) };
    let mut chars = s.chars();
    let ok = chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !ok {
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
    let diagram = match choice(root, "", "diagram", &["flow", "layers", "sequence", "free"])? {
        Some("flow") => Diagram::Flow,
        Some("layers") => Diagram::Layers,
        Some("free") => Diagram::Free,
        Some(_) => Diagram::Sequence,
        None => return Err(err("missing-field", "diagram", "Missing required field 'diagram'.".into()).suggest(Some("Use one of: flow, layers, sequence, free.".into()))),
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
    if raw_nodes.is_empty() {
        return Err(err("missing-field", "nodes", "A spec needs at least one node.".into()));
    }
    for (i, v) in raw_nodes.iter().enumerate() {
        let p = format!("nodes[{i}]");
        let m = object(v, &p, NODE_FIELDS)?;
        let kind = match choice(m, &p, "kind", KINDS)?.unwrap_or("box") {
            "service" => Kind::Service,
            "client" => Kind::Client,
            "datastore" => Kind::Datastore,
            "queue" => Kind::Queue,
            "actor" => Kind::Actor,
            "external" => Kind::External,
            "decision" => Kind::Decision,
            "document" => Kind::Document,
            _ => Kind::Box,
        };
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
        nodes.push(Node { id: slug(m, &p, "id", true)?.unwrap(), label: string(m, &p, "label", true, 1, 80)?.unwrap(), kind, emphasis, width, at });
    }

    let mut edges = Vec::new();
    for (i, v) in list(root, "edges")?.iter().enumerate() {
        let p = format!("edges[{i}]");
        let m = object(v, &p, EDGE_FIELDS)?;
        let (from, to) = (slug(m, &p, "from", true)?.unwrap(), slug(m, &p, "to", true)?.unwrap());
        let style = match choice(m, &p, "style", &["sync", "async", "dependency"])? {
            Some("async") => EdgeStyle::Async,
            Some("dependency") => EdgeStyle::Dependency,
            _ => EdgeStyle::Sync,
        };
        let arrow = match choice(m, &p, "arrow", &["end", "both", "none"])? {
            Some("both") => Arrow::Both,
            Some("none") => Arrow::None,
            _ => Arrow::End,
        };
        let id = slug(m, &p, "id", false)?.unwrap_or_else(|| format!("{from}--{to}"));
        edges.push(Edge { id, from, to, label: string(m, &p, "label", false, 0, 40)?.filter(|s| !s.trim().is_empty()), style, arrow });
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
        groups.push(Group { id: slug(m, &p, "id", true)?.unwrap(), label: string(m, &p, "label", false, 0, 60)?.filter(|s| !s.trim().is_empty()), members });
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

    Ok(Spec { diagram, title, description, direction, theme, max_width, nodes, edges, groups, notes, node_parent, group_parent, ids })
}

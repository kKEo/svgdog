//! Edits to a YAML spec that keep its comments and layout.
//!
//! Each operation is first applied to the data. This module then tries to make the
//! same change in the text. The caller re-reads the patched text and keeps it only
//! when it means exactly what the data says; otherwise the spec is rewritten whole.
//! So every routine here may simply give up by returning `None`.

use serde_json::Value;
use yaml_rust2::Event;
use yaml_rust2::parser::{MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

use crate::ops::Op;

enum Node {
    Scalar { start: usize, end: usize, text: String },
    Map { start: usize, close: Option<usize>, entries: Vec<(String, usize, Node)> },
    Seq { start: usize, close: Option<usize>, items: Vec<Node> },
}

impl Node {
    fn start(&self) -> usize {
        match self {
            Node::Scalar { start, .. } | Node::Map { start, .. } | Node::Seq { start, .. } => *start,
        }
    }
    /// Offset just past the node's last character.
    fn end(&self) -> Option<usize> {
        match self {
            Node::Scalar { end, .. } => Some(*end),
            Node::Map { close: Some(c), .. } | Node::Seq { close: Some(c), .. } => Some(c + 1),
            Node::Map { entries, .. } => entries.last()?.2.end(),
            Node::Seq { items, .. } => items.last()?.end(),
        }
    }
    /// Column where the node's own text begins: for a block mapping, its first key.
    fn col(&self, src: &str) -> usize {
        let at = match self {
            Node::Map { close: None, entries, .. } if !entries.is_empty() => entries[0].1,
            other => other.start(),
        };
        at - line_start(src, at)
    }
    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Map { entries, .. } => entries.iter().find(|(k, _, _)| k == key).map(|e| &e.2),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        match self {
            Node::Scalar { text, .. } => Some(text),
            _ => None,
        }
    }
    fn items(&self) -> &[Node] {
        match self {
            Node::Seq { items, .. } => items,
            _ => &[],
        }
    }
    fn flow(&self) -> bool {
        matches!(self, Node::Map { close: Some(_), .. } | Node::Seq { close: Some(_), .. })
    }
}

struct Sink(Vec<(Event, usize)>);

impl MarkedEventReceiver for Sink {
    fn on_event(&mut self, ev: Event, mark: Marker) {
        self.0.push((ev, mark.index()));
    }
}

fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn line_end(src: &str, at: usize) -> usize {
    src[at..].find('\n').map(|i| at + i).unwrap_or(src.len())
}

fn scalar_end(src: &str, start: usize, text: &str, style: TScalarStyle) -> Option<usize> {
    let rest = &src[start..];
    match style {
        TScalarStyle::Plain => rest.starts_with(text).then_some(start + text.len()),
        TScalarStyle::DoubleQuoted => {
            let mut chars = rest.char_indices().skip(1);
            if !rest.starts_with('"') {
                return None;
            }
            while let Some((i, c)) = chars.next() {
                match c {
                    '\\' => {
                        chars.next();
                    }
                    '"' => return Some(start + i + 1),
                    _ => {}
                }
            }
            None
        }
        TScalarStyle::SingleQuoted => {
            if !rest.starts_with('\'') {
                return None;
            }
            let b = rest.as_bytes();
            let mut i = 1;
            while i < b.len() {
                if b[i] == b'\'' {
                    if b.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    return Some(start + i + 1);
                }
                i += 1;
            }
            None
        }
        _ => None,
    }
}

fn build(src: &str, offs: &[usize], evs: &[(Event, usize)], i: &mut usize) -> Option<Node> {
    let (ev, mark) = evs.get(*i)?;
    let start = *offs.get(*mark)?;
    *i += 1;
    match ev {
        Event::Scalar(text, style, _, _) => Some(Node::Scalar { start, end: scalar_end(src, start, text, *style)?, text: text.clone() }),
        Event::MappingStart(..) | Event::SequenceStart(..) => {
            let map = matches!(ev, Event::MappingStart(..));
            let (open, shut) = if map { ('{', '}') } else { ('[', ']') };
            let flow = src[start..].starts_with(open);
            let mut entries = Vec::new();
            let mut items = Vec::new();
            loop {
                let (next, end_mark) = evs.get(*i)?;
                if matches!(next, Event::MappingEnd | Event::SequenceEnd) {
                    *i += 1;
                    // In flow style the end event sits at, or just past, the closing bracket.
                    let close = if flow {
                        let at = *offs.get(*end_mark)?;
                        Some(if src[at..].starts_with(shut) { at } else if at > 0 && src[..at].ends_with(shut) { at - 1 } else { return None })
                    } else {
                        None
                    };
                    return Some(if map { Node::Map { start, close, entries } } else { Node::Seq { start, close, items } });
                }
                if map {
                    let key = build(src, offs, evs, i)?;
                    let value = build(src, offs, evs, i)?;
                    entries.push((key.text()?.to_string(), key.start(), value));
                } else {
                    items.push(build(src, offs, evs, i)?);
                }
            }
        }
        _ => None,
    }
}

fn parse(src: &str) -> Option<Node> {
    let mut sink = Sink(Vec::new());
    Parser::new_from_str(src).load(&mut sink, false).ok()?;
    let offs: Vec<usize> = src.char_indices().map(|(b, _)| b).chain([src.len()]).collect();
    let evs: Vec<(Event, usize)> = sink.0.into_iter().filter(|(e, _)| !matches!(e, Event::StreamStart | Event::StreamEnd | Event::DocumentStart | Event::DocumentEnd | Event::Nothing)).collect();
    let mut i = 0;
    let root = build(src, &offs, &evs, &mut i)?;
    (i == evs.len()).then_some(root)
}

fn plain_ok(s: &str) -> bool {
    let mut chars = s.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '/');
    first_ok
        && !s.ends_with(' ')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || " _./()+-".contains(c))
        && serde_yaml_ng::from_str::<Value>(s).ok() == Some(Value::String(s.to_string()))
}

/// A value written the way a person would in a flow context.
fn flow(v: &Value) -> String {
    match v {
        Value::String(s) if plain_ok(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.iter().map(flow).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!("{{{}}}", m.iter().map(|(k, v)| format!("{k}: {}", flow(v))).collect::<Vec<_>>().join(", ")),
        other => other.to_string(),
    }
}

fn splice(src: &str, at: usize, end: usize, text: &str) -> String {
    format!("{}{}{}", &src[..at], text, &src[end..])
}

fn edge_id(n: &Node) -> Option<String> {
    match n.get("id").and_then(|v| v.text()) {
        Some(id) => Some(id.to_string()),
        None => Some(format!("{}--{}", n.get("from")?.text()?, n.get("to")?.text()?)),
    }
}

/// The mapping a target names: the root, or the node, group or edge with that id.
fn find<'a>(root: &'a Node, target: &str) -> Option<&'a Node> {
    let t = target.trim();
    if t == ":root" || t == "spec" {
        return Some(root);
    }
    let id = t.strip_prefix('#')?;
    for list in ["nodes", "groups"] {
        if let Some(n) = root.get(list).map(|l| l.items()).unwrap_or(&[]).iter().find(|n| n.get("id").and_then(|v| v.text()) == Some(id)) {
            return Some(n);
        }
    }
    root.get("edges")?.items().iter().find(|n| edge_id(n).as_deref() == Some(id))
}

/// Set `key` in a mapping to the value the data now holds.
fn set_key(src: &str, map: &Node, key: &str, value: &Value) -> Option<String> {
    let Node::Map { close, entries, .. } = map else { return None };
    if let Some(old) = map.get(key) {
        return Some(splice(src, old.start(), old.end()?, &flow(value)));
    }
    match close {
        Some(c) => {
            let lead = if entries.is_empty() { "" } else { ", " };
            Some(splice(src, *c, *c, &format!("{lead}{key}: {}", flow(value))))
        }
        None => {
            // A new line after the mapping's last one, at the mapping's own indentation.
            let at = line_end(src, map.end()?);
            Some(splice(src, at, at, &format!("\n{}{key}: {}", " ".repeat(map.col(src)), flow(value))))
        }
    }
}

fn remove_key(src: &str, map: &Node, key: &str) -> Option<String> {
    let Node::Map { close: None, entries, .. } = map else { return None };
    let (_, key_at, value) = entries.iter().find(|(k, _, _)| k == key)?;
    let (ls, le) = (line_start(src, *key_at), line_end(src, value.end()?));
    // Only a key that has its line to itself can be cut cleanly.
    if !src[ls..*key_at].trim().is_empty() || line_end(src, *key_at) != le || le >= src.len() {
        return None;
    }
    Some(splice(src, ls, le + 1, ""))
}

/// Add an item to a sequence, in the style of the items already there.
fn append(src: &str, seq: &Node, value: &Value) -> Option<String> {
    let Node::Seq { close, items, .. } = seq else { return None };
    if let Some(c) = close {
        let lead = if items.is_empty() { "" } else { ", " };
        return Some(splice(src, *c, *c, &format!("{lead}{}", flow(value))));
    }
    let last = items.last()?;
    let dash = items[0].col(src).checked_sub(2)?;
    let pad = " ".repeat(dash);
    let text = match value {
        Value::Object(m) if !last.flow() => {
            let lines: Vec<String> = m.iter().enumerate().map(|(k, (key, v))| format!("{pad}{}{key}: {}", if k == 0 { "- " } else { "  " }, flow(v))).collect();
            lines.join("\n")
        }
        other => format!("{pad}- {}", flow(other)),
    };
    let at = line_end(src, last.end()?);
    Some(splice(src, at, at, &format!("\n{text}")))
}

fn append_to_list(src: &str, root: &Node, list: &str, value: &Value) -> Option<String> {
    match root.get(list) {
        Some(seq) => append(src, seq, value),
        None => {
            // A new top-level list goes at the end of the file.
            let Value::Object(m) = value else { return None };
            let lines: Vec<String> = m.iter().enumerate().map(|(k, (key, v))| format!("  {}{key}: {}", if k == 0 { "- " } else { "  " }, flow(v))).collect();
            let gap = if src.ends_with('\n') { "" } else { "\n" };
            Some(format!("{src}{gap}{list}:\n{}\n", lines.join("\n")))
        }
    }
}

/// Make in the text the change that turned `before` into `after`. `None` means
/// this module cannot do it and the spec should be rewritten instead.
pub fn patch(src: &str, op: &Op, before: &Value, after: &Value) -> Option<String> {
    let root = parse(src)?;
    let item = |doc: &'_ Value, target: &str| -> Option<Value> {
        let t = target.trim();
        if t == ":root" || t == "spec" {
            return Some(doc.clone());
        }
        let id = t.strip_prefix('#')?;
        for list in ["nodes", "groups"] {
            if let Some(v) = doc[list].as_array().and_then(|a| a.iter().find(|v| v["id"].as_str() == Some(id))) {
                return Some(v.clone());
            }
        }
        doc["edges"].as_array()?.iter().filter(|e| e.is_object()).find(|e| e["id"].as_str().map(|s| s.to_string()).unwrap_or_else(|| format!("{}--{}", e["from"].as_str().unwrap_or(""), e["to"].as_str().unwrap_or(""))) == id).cloned()
    };
    match op {
        Op::SetAttr { target, attr, .. } => set_key(src, find(&root, target)?, attr, item(after, target)?.get(attr)?),
        Op::Move { target, .. } => set_key(src, find(&root, target)?, "at", item(after, target)?.get("at")?),
        Op::RemoveAttr { target, attr } => remove_key(src, find(&root, target)?, attr),
        Op::RenameId { target, to } => {
            let node = find(&root, target)?;
            let old = node.get("id")?.text()?.to_string();
            // The id itself, and every place that refers to it.
            let mut spots: Vec<&Node> = vec![node.get("id")?];
            for e in root.get("edges").map(|l| l.items()).unwrap_or(&[]) {
                spots.extend(["from", "to"].iter().filter_map(|k| e.get(k)));
            }
            for g in root.get("groups").map(|l| l.items()).unwrap_or(&[]) {
                spots.extend(g.get("members").map(|m| m.items()).unwrap_or(&[]));
            }
            for n in root.get("notes").map(|l| l.items()).unwrap_or(&[]) {
                spots.extend(n.get("attach"));
            }
            let mut hits: Vec<(usize, usize)> = spots.iter().filter(|s| s.text() == Some(old.as_str())).filter_map(|s| Some((s.start(), s.end()?))).collect();
            hits.sort();
            hits.dedup();
            let mut out = src.to_string();
            for (start, end) in hits.into_iter().rev() {
                out = splice(&out, start, end, &flow(&Value::String(to.clone())));
            }
            Some(out)
        }
        Op::AddNode { target, .. } => {
            let node = after["nodes"].as_array()?.last()?;
            let text = append_to_list(src, &root, "nodes", node)?;
            let t = target.trim();
            if t == ":root" || t == "spec" {
                return Some(text);
            }
            // The node also joins the group it was added to.
            let root = parse(&text)?;
            append(&text, find(&root, target)?.get("members")?, node.get("id")?)
        }
        Op::AddEdge { .. } => append_to_list(src, &root, "edges", after["edges"].as_array()?.last()?),
        // Removing an item drags edges, notes and memberships with it: rewrite.
        Op::RemoveElem { .. } | Op::SetStyle { .. } | Op::RemoveStyle { .. } => {
            let _ = before;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "# Orders\nversion: 1\ndiagram: flow   # layered\ntitle: \"Order: placement\"\n\nnodes:\n  # entry point\n  - id: web\n    label: Storefront   # shown to shoppers\n  - id: api\n    label: 'Order API'\n    kind: service\n\ngroups:\n  - {id: backend, label: Backend, members: [api]}\n\nedges:\n  - from: web\n    to: api\n";

    fn node<'a>(root: &'a Node, t: &str) -> &'a Node {
        find(root, t).unwrap()
    }

    #[test]
    fn values_change_and_comments_stay() {
        let root = parse(SRC).unwrap();
        let out = set_key(SRC, node(&root, "#web"), "label", &Value::String("Shop front".into())).unwrap();
        assert_eq!(out, SRC.replace("label: Storefront   # shown", "label: Shop front   # shown"));
        let out = set_key(SRC, node(&root, "#api"), "label", &Value::String("Orders: API".into())).unwrap();
        assert!(out.contains("label: \"Orders: API\"\n    kind: service"));
        let out = set_key(SRC, &root, "title", &Value::String("Orders".into())).unwrap();
        assert!(out.contains("title: Orders\n\nnodes:"));
    }

    #[test]
    fn new_keys_and_items_follow_the_local_style() {
        let root = parse(SRC).unwrap();
        let out = set_key(SRC, node(&root, "#web"), "kind", &Value::String("client".into())).unwrap();
        assert!(out.contains("label: Storefront   # shown to shoppers\n    kind: client\n  - id: api"), "{out}");
        let out = set_key(SRC, node(&root, "#backend"), "label", &Value::String("Core".into())).unwrap();
        assert!(out.contains("{id: backend, label: Core, members: [api]}"));
        let out = append(SRC, root.get("nodes").unwrap(), &serde_json::json!({ "id": "db", "label": "Orders DB" })).unwrap();
        assert!(out.contains("    kind: service\n  - id: db\n    label: Orders DB\n\ngroups:"), "{out}");
        let out = append(SRC, node(&root, "#backend").get("members").unwrap(), &Value::String("db".into())).unwrap();
        assert!(out.contains("members: [api, db]}"));
        assert!(out.starts_with("# Orders\n") && out.contains("# entry point"));
    }

    #[test]
    fn gives_up_on_what_it_cannot_place() {
        assert!(parse("a: |\n  block text\n").is_none());
        assert!(parse("a: &x 1\nb: *x\n").is_none());
        let root = parse(SRC).unwrap();
        assert!(remove_key(SRC, node(&root, "#web"), "id").is_none(), "the first key shares its line with the dash");
        assert_eq!(remove_key(SRC, node(&root, "#api"), "kind").unwrap(), SRC.replace("    kind: service\n", ""));
    }
}

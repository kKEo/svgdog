//! Edit operations on a diagram spec. The spec is changed as data and written
//! back in canonical form, so comments and hand formatting do not survive.

use serde_json::{Map, Value};

use crate::envelope::AppError;
use crate::ops::Op;
use crate::spec::{EDGE_FIELDS, GROUP_FIELDS, NODE_FIELDS, ROOT_FIELDS};
use crate::util::did_you_mean;

fn bad(i: usize, field: &str, message: String) -> AppError {
    AppError::input("invalid-op", format!("ops[{i}]: {message}")).path(format!("ops[{i}].{field}"))
}

fn edge_id(e: &Value) -> String {
    match e.get("id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => format!("{}--{}", e["from"].as_str().unwrap_or(""), e["to"].as_str().unwrap_or("")),
    }
}

/// Where an id lives: the list it is in, its position, and the fields that list allows.
fn locate(doc: &Value, id: &str) -> Option<(&'static str, usize, &'static [&'static str])> {
    for (list, fields) in [("nodes", NODE_FIELDS), ("groups", GROUP_FIELDS)] {
        if let Some(i) = doc[list].as_array().and_then(|a| a.iter().position(|v| v["id"].as_str() == Some(id))) {
            return Some((list, i, fields));
        }
    }
    let i = doc["edges"].as_array()?.iter().position(|e| edge_id(e) == id)?;
    Some(("edges", i, EDGE_FIELDS))
}

fn target_id(target: &str, i: usize) -> Result<Option<&str>, AppError> {
    match target.trim() {
        ":root" | "spec" => Ok(None),
        t => t.strip_prefix('#').map(Some).ok_or_else(|| bad(i, "target", format!("on a spec, a target is '#id' or ':root', not '{t}'."))),
    }
}

fn find(doc: &Value, id: &str, i: usize) -> Result<(&'static str, usize, &'static [&'static str]), AppError> {
    locate(doc, id).ok_or_else(|| {
        let known: Vec<String> = ["nodes", "groups"].iter().flat_map(|l| doc[*l].as_array().into_iter().flatten()).filter_map(|v| v["id"].as_str().map(|s| s.to_string())).chain(doc["edges"].as_array().into_iter().flatten().map(edge_id)).collect();
        AppError::input("no-match", format!("ops[{i}]: the spec has no node, group or edge '{id}'.")).path(format!("ops[{i}].target")).suggest(did_you_mean(id, known.iter().map(|s| s.as_str())))
    })
}

/// Text from the command line or an ops file, read as the value it most plausibly is.
fn typed(raw: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(v) if !v.is_string() && !v.is_null() => v,
        _ => Value::String(raw.to_string()),
    }
}

fn check_field(field: &str, allowed: &[&str], i: usize) -> Result<(), AppError> {
    if allowed.contains(&field) {
        return Ok(());
    }
    Err(AppError::input("unknown-field", format!("ops[{i}]: '{field}' is not a field here.")).path(format!("ops[{i}].attr")).suggest(did_you_mean(field, allowed.iter().copied())))
}

fn list_mut<'a>(doc: &'a mut Value, name: &str) -> &'a mut Vec<Value> {
    let root = doc.as_object_mut().expect("a parsed spec is a mapping");
    let slot = root.entry(name.to_string()).or_insert_with(|| Value::Array(Vec::new()));
    if !slot.is_array() {
        *slot = Value::Array(Vec::new());
    }
    slot.as_array_mut().unwrap()
}

fn rename_refs(doc: &mut Value, old: &str, new: &str) {
    let swap = |v: &mut Value| {
        if v.as_str() == Some(old) {
            *v = Value::String(new.to_string());
        }
    };
    for e in list_mut(doc, "edges") {
        for k in ["from", "to"] {
            if let Some(v) = e.get_mut(k) {
                swap(v);
            }
        }
    }
    for g in list_mut(doc, "groups") {
        if let Some(ms) = g.get_mut("members").and_then(|m| m.as_array_mut()) {
            ms.iter_mut().for_each(swap);
        }
    }
    for n in list_mut(doc, "notes") {
        if let Some(v) = n.get_mut("attach") {
            swap(v);
        }
    }
}

/// Remove an id and everything that would dangle without it.
fn remove(doc: &mut Value, id: &str, list: &str, at: usize) {
    list_mut(doc, list).remove(at);
    if list != "edges" {
        let gone: Vec<String> = list_mut(doc, "edges").iter().filter(|e| e["from"].as_str() == Some(id) || e["to"].as_str() == Some(id)).map(edge_id).collect();
        list_mut(doc, "edges").retain(|e| e["from"].as_str() != Some(id) && e["to"].as_str() != Some(id));
        list_mut(doc, "notes").retain(|n| !gone.iter().any(|g| n["attach"].as_str() == Some(g.as_str())));
        for g in list_mut(doc, "groups") {
            if let Some(ms) = g.get_mut("members").and_then(|m| m.as_array_mut()) {
                ms.retain(|m| m.as_str() != Some(id));
            }
        }
        // A group left without members has nothing to frame.
        list_mut(doc, "groups").retain(|g| g["members"].as_array().is_some_and(|m| !m.is_empty()));
    }
    list_mut(doc, "notes").retain(|n| n["attach"].as_str() != Some(id));
    for name in ["edges", "groups", "notes"] {
        if list_mut(doc, name).is_empty() {
            doc.as_object_mut().unwrap().remove(name);
        }
    }
}

pub struct Edited {
    /// Operations that changed the spec.
    pub applied: usize,
    /// The new text; `preserved` says whether comments and layout were kept.
    pub text: String,
    pub preserved: bool,
}

/// Apply operations to a spec. Each change is made in the data and, where the
/// text can be patched to mean the same, in the text too, so comments survive.
/// When a change cannot be placed, the whole spec is written out afresh.
pub fn edit(src: &str, doc: &mut Value, ops: &[Op]) -> Result<Edited, AppError> {
    if !doc.is_object() {
        return Err(AppError::input("invalid-spec", "The spec must be a mapping."));
    }
    let json = crate::spec::is_json(src);
    let mut text = (!json).then(|| src.to_string());
    let mut applied = 0;
    for (i, op) in ops.iter().enumerate() {
        let before = doc.clone();
        apply_one(doc, op, i)?;
        if *doc == before {
            continue;
        }
        applied += 1;
        // Keep the patched text only if it reads back as exactly the new data.
        text = text.and_then(|t| crate::yaml_patch::patch(&t, op, &before, doc)).filter(|t| serde_yaml_ng::from_str::<Value>(t).ok().as_ref() == Some(&*doc));
    }
    Ok(match text {
        _ if applied == 0 => Edited { applied, text: src.to_string(), preserved: true },
        Some(text) => Edited { applied, text, preserved: true },
        None => Edited { applied, text: write(doc, json)?, preserved: false },
    })
}

fn apply_one(doc: &mut Value, op: &Op, i: usize) -> Result<(), AppError> {
    {
        match op {
            Op::SetAttr { target, attr, value } => match target_id(target, i)? {
                None => {
                    check_field(attr, ROOT_FIELDS, i)?;
                    doc.as_object_mut().unwrap().insert(attr.clone(), typed(value));
                }
                Some(id) => {
                    let (list, at, fields) = find(doc, id, i)?;
                    check_field(attr, fields, i)?;
                    if attr == "id" {
                        return Err(bad(i, "attr", "use rename-id to change an id, so references follow.".into()));
                    }
                    list_mut(doc, list)[at].as_object_mut().unwrap().insert(attr.clone(), typed(value));
                }
            },
            Op::RemoveAttr { target, attr } => match target_id(target, i)? {
                None => {
                    check_field(attr, ROOT_FIELDS, i)?;
                    doc.as_object_mut().unwrap().remove(attr);
                }
                Some(id) => {
                    let (list, at, fields) = find(doc, id, i)?;
                    check_field(attr, fields, i)?;
                    list_mut(doc, list)[at].as_object_mut().unwrap().remove(attr);
                }
            },
            Op::RemoveElem { target } => {
                let id = target_id(target, i)?.ok_or_else(|| bad(i, "target", "the spec root cannot be removed.".into()))?;
                let (list, at, _) = find(doc, id, i)?;
                remove(doc, id, list, at);
            }
            Op::Move { target, dx, dy } => {
                let id = target_id(target, i)?.ok_or_else(|| bad(i, "target", "move needs a node.".into()))?;
                let (list, at, _) = find(doc, id, i)?;
                let node = &mut list_mut(doc, list)[at];
                let pos = node.get("at").and_then(|a| a.as_array()).filter(|a| a.len() == 2).map(|a| (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)));
                let Some((x, y)) = pos.filter(|_| list == "nodes") else {
                    return Err(bad(i, "target", format!("'{id}' has no 'at'; only pinned nodes of a free diagram can be moved.")));
                };
                node["at"] = serde_json::json!([crate::util::num(x + dx), crate::util::num(y + dy)]);
            }
            Op::RenameId { target, to } => {
                let id = target_id(target, i)?.ok_or_else(|| bad(i, "target", "rename-id needs '#id'.".into()))?.to_string();
                let (list, at, _) = find(doc, &id, i)?;
                if id != *to && locate(doc, to).is_some() {
                    return Err(bad(i, "to", format!("id '{to}' is already in use.")));
                }
                list_mut(doc, list)[at].as_object_mut().unwrap().insert("id".into(), Value::String(to.clone()));
                rename_refs(doc, &id, to);
            }
            Op::AddNode { target, attrs, .. } => {
                let mut node = Map::new();
                for (k, v) in attrs {
                    check_field(k, NODE_FIELDS, i)?;
                    node.insert(k.clone(), typed(v));
                }
                let id = node.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| bad(i, "attrs", "a new node needs attrs.id and attrs.label.".into()))?;
                let group = target_id(target, i)?.map(|s| s.to_string());
                if let Some(g) = &group {
                    let (list, at, _) = find(doc, g, i)?;
                    if list != "groups" {
                        return Err(bad(i, "target", format!("'{g}' is not a group; target a group or ':root'.")));
                    }
                    if let Some(m) = list_mut(doc, "groups")[at]["members"].as_array_mut() { m.push(Value::String(id.clone())) }
                }
                list_mut(doc, "nodes").push(Value::Object(node));
            }
            Op::AddEdge { from, to, attrs } => {
                let mut edge = Map::new();
                edge.insert("from".into(), Value::String(from.clone()));
                edge.insert("to".into(), Value::String(to.clone()));
                for (k, v) in attrs {
                    check_field(k, EDGE_FIELDS, i)?;
                    edge.insert(k.clone(), typed(v));
                }
                list_mut(doc, "edges").push(Value::Object(edge));
            }
            Op::SetStyle { .. } | Op::RemoveStyle { .. } => return Err(bad(i, "style", "a spec has no styles; set a field with 'attr'.".into())),
        }
    }
    Ok(())
}

/// Canonical text for a spec document, in the format the input used.
pub fn write(doc: &Value, json: bool) -> Result<String, AppError> {
    if json {
        Ok(serde_json::to_string_pretty(doc).expect("a spec serialises") + "\n")
    } else {
        serde_yaml_ng::to_string(doc).map_err(|e| AppError::new(crate::envelope::EXIT_INTERNAL, "internal-error", format!("Cannot write the spec as YAML: {e}.")))
    }
}

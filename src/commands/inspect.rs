use serde_json::{Map, Value, json};

use super::read_input;
use crate::envelope::{AppError, EXIT_OK, Output, head};
use crate::rules::bbox_json;
use crate::svg::selector::Selector;
use crate::svg::{self, Kind, Scene};
use crate::util::{did_you_mean, fmt};

const FIELDS: &[&str] = &["id", "tag", "bbox", "text", "fill", "stroke", "refs"];

pub fn run(file: &str, select: Option<&str>, depth: Option<usize>, fields: &[String]) -> Result<Output, AppError> {
    let mut want: Vec<&'static str> = Vec::new();
    for f in fields.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        match FIELDS.iter().find(|k| **k == f) {
            Some(k) => want.push(k),
            None => return Err(AppError::usage("invalid-argument", format!("Unknown field '{f}' in --fields.")).suggest(did_you_mean(f, FIELDS.iter().copied()))),
        }
    }
    let has = |k: &str| want.contains(&k);
    let selector = match select {
        Some(s) => Some(Selector::parse(s).map_err(|e| AppError::usage("invalid-argument", format!("Cannot read selector '{s}': {e}.")))?),
        None => None,
    };

    let src = read_input(file)?;
    let doc = roxmltree::Document::parse_with_options(&src, svg::parse_options())
        .map_err(|e| AppError::input("malformed-xml", format!("{file} is not well-formed XML: {e}.")).path(file))?;
    let scene = Scene::build(&doc);

    // With a selector, depth counts from each match; otherwise from the root.
    let mut base: Vec<Option<usize>> = vec![None; scene.elems.len()];
    for (i, e) in scene.elems.iter().enumerate() {
        base[i] = match &selector {
            None => Some(0),
            Some(sel) if sel.matches(e.node) => Some(e.depth),
            Some(_) => e.parent.and_then(|p| base[p]),
        };
    }

    let mut elements = Vec::new();
    let mut text = String::new();
    for (i, e) in scene.elems.iter().enumerate() {
        let Some(b) = base[i] else { continue };
        if depth.is_some_and(|d| e.depth - b > d) {
            continue;
        }
        let mut m = Map::new();
        let target = scene.target(e.node);
        m.insert("target".into(), json!(target));
        m.insert("line".into(), json!(scene.line(e.node)));
        m.insert("depth".into(), json!(e.depth));
        let mut line = format!("{}{}", "  ".repeat(e.depth - b), target);
        if has("id") {
            m.insert("id".into(), json!(e.node.attribute("id")));
        }
        if has("tag") {
            m.insert("tag".into(), json!(e.tag));
            line.push_str(&format!(" <{}>", e.tag));
        }
        if has("bbox") {
            m.insert("bbox".into(), e.bbox.map(|r| json!(bbox_json(r))).unwrap_or(Value::Null));
            if let Some(r) = e.bbox {
                line.push_str(&format!(" [{} {} {} {}]", fmt(r.x0), fmt(r.y0), fmt(r.width()), fmt(r.height())));
            }
        }
        if has("text")
            && let Kind::Text(t) = &e.kind
        {
            m.insert("text".into(), json!(t.content));
            line.push_str(&format!(" {:?}", t.content));
        }
        if e.is_leaf() {
            if has("fill") {
                m.insert("fill".into(), json!(e.style.fill.describe()));
                line.push_str(&format!(" fill={}", e.style.fill.describe()));
            }
            if has("stroke") {
                m.insert("stroke".into(), json!(e.style.stroke.describe()));
                line.push_str(&format!(" stroke={}", e.style.stroke.describe()));
            }
        }
        if has("refs") {
            let mut refs: Vec<String> = crate::rules::refs(e.node).into_iter().map(|r| format!("#{r}")).collect();
            for id in [&e.style.marker_start, &e.style.marker_end].into_iter().flatten() {
                refs.push(format!("#{id}"));
            }
            refs.sort();
            refs.dedup();
            if !refs.is_empty() {
                line.push_str(&format!(" refs={}", refs.join(",")));
            }
            m.insert("refs".into(), json!(refs));
        }
        elements.push(Value::Object(m));
        text.push_str(&line);
        text.push('\n');
    }

    let mut m = head("inspect", true);
    m.insert("file".into(), json!(file));
    m.insert("viewBox".into(), scene.viewbox.map(|r| json!(bbox_json(r))).unwrap_or(Value::Null));
    m.insert("summary".into(), json!({ "elements": scene.elems.len(), "returned": elements.len() }));
    text.push_str(&format!("\n{} of {} visible elements\n", elements.len(), scene.elems.len()));
    m.insert("elements".into(), Value::Array(elements));
    Ok(Output { json: Value::Object(m), text, exit: EXIT_OK, raw: None })
}

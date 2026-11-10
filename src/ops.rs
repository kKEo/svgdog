//! Edit operations and the text patcher that applies them.
//!
//! Operations change bytes only inside the elements they address: the source is
//! patched in place, never re-serialised. Every target in a batch refers to the
//! file as it was before the batch.

use roxmltree::{Document, Node};
use serde_json::Value;

use crate::envelope::AppError;
use crate::svg::selector::Selector;
use crate::svg::{self, style::url_id};
use crate::util::{did_you_mean, fmt};

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    SetAttr { target: String, attr: String, value: String },
    SetStyle { target: String, prop: String, value: String },
    RemoveElem { target: String },
    RemoveAttr { target: String, attr: String },
    RemoveStyle { target: String, prop: String },
    Move { target: String, dx: f64, dy: f64 },
    RenameId { target: String, to: String },
    AddNode { target: String, tag: String, attrs: Vec<(String, String)>, text: Option<String>, at_start: bool },
}

pub const OP_NAMES: &[&str] = &["set", "remove", "add-node", "add-edge", "move", "rename-id"];

fn is_xml_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == ':') && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '-' | '.'))
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(match n.as_f64() {
            Some(f) if n.is_f64() => fmt(f),
            _ => n.to_string(),
        }),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn bad(i: usize, field: &str, message: String) -> AppError {
    AppError::input("invalid-op", format!("ops[{i}]: {message}")).path(format!("ops[{i}]{}{field}", if field.is_empty() { "" } else { "." }))
}

/// Parse an ops document: an array, or an object with an `ops` array such as a finding's `fix`.
pub fn parse_ops(doc: &Value) -> Result<Vec<Op>, AppError> {
    let list = match doc {
        Value::Array(a) => a,
        Value::Object(o) if o.get("ops").is_some_and(|v| v.is_array()) => o["ops"].as_array().unwrap(),
        _ => return Err(AppError::input("invalid-op", "Operations must be a JSON array, or an object with an 'ops' array.")),
    };
    let mut out = Vec::new();
    for (i, item) in list.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| bad(i, "", "an operation must be an object.".into()))?;
        let name = obj.get("op").and_then(|v| v.as_str()).ok_or_else(|| bad(i, "op", "missing 'op'.".into()))?;
        let allowed: &[&str] = match name {
            "set" => &["op", "target", "attr", "style", "value"],
            "remove" => &["op", "target", "attr", "style"],
            "move" => &["op", "target", "dx", "dy"],
            "rename-id" => &["op", "target", "to"],
            "add-node" => &["op", "target", "tag", "attrs", "text", "at"],
            "add-edge" => return Err(bad(i, "op", "add-edge applies to a diagram spec, which arrives with the render command.".into())),
            other => {
                return Err(AppError::input("unknown-op", format!("ops[{i}]: unknown operation '{other}'."))
                    .path(format!("ops[{i}].op"))
                    .suggest(did_you_mean(other, OP_NAMES.iter().copied())));
            }
        };
        for k in obj.keys() {
            if !allowed.contains(&k.as_str()) {
                return Err(AppError::input("unknown-field", format!("ops[{i}]: unknown field '{k}' in a {name} operation."))
                    .path(format!("ops[{i}].{k}"))
                    .suggest(did_you_mean(k, allowed.iter().copied())));
            }
        }
        let text = |k: &str| obj.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
        let need = |k: &str| text(k).ok_or_else(|| bad(i, k, format!("a {name} operation needs a string '{k}'.")));
        let target = need("target")?;
        let (attr, style) = (text("attr"), text("style"));
        if attr.is_some() && style.is_some() {
            return Err(bad(i, "style", "give 'attr' or 'style', not both.".into()));
        }
        if let Some(a) = &attr
            && !is_xml_name(a.trim_end_matches('*'))
            && a != "*"
        {
            return Err(bad(i, "attr", format!("'{a}' is not an attribute name.")));
        }
        out.push(match name {
            "set" => {
                let value = obj.get("value").and_then(scalar).ok_or_else(|| bad(i, "value", "a set operation needs a string or number 'value'.".into()))?;
                match (attr, style) {
                    (Some(a), _) if a.contains('*') => return Err(bad(i, "attr", "a set operation needs one attribute name, not a pattern.".into())),
                    (Some(attr), _) => Op::SetAttr { target, attr, value },
                    (_, Some(prop)) => Op::SetStyle { target, prop, value },
                    _ => return Err(bad(i, "attr", "a set operation needs 'attr' or 'style'.".into())),
                }
            }
            "remove" => match (attr, style) {
                (Some(attr), _) => Op::RemoveAttr { target, attr },
                (_, Some(prop)) => Op::RemoveStyle { target, prop },
                _ => Op::RemoveElem { target },
            },
            "move" => {
                let n = |k: &str| match obj.get(k) {
                    None => Ok(0.0),
                    Some(v) => v.as_f64().ok_or_else(|| bad(i, k, format!("'{k}' must be a number."))),
                };
                Op::Move { target, dx: n("dx")?, dy: n("dy")? }
            }
            "rename-id" => {
                let to = need("to")?;
                if !is_xml_name(&to) || to.contains(':') {
                    return Err(bad(i, "to", format!("'{to}' is not a valid id.")));
                }
                Op::RenameId { target, to }
            }
            "add-node" => {
                let tag = need("tag")?;
                if !is_xml_name(&tag) {
                    return Err(bad(i, "tag", format!("'{tag}' is not an element name.")));
                }
                let mut attrs = Vec::new();
                if let Some(a) = obj.get("attrs") {
                    let map = a.as_object().ok_or_else(|| bad(i, "attrs", "'attrs' must be an object.".into()))?;
                    for (k, v) in map {
                        if !is_xml_name(k) {
                            return Err(bad(i, "attrs", format!("'{k}' is not an attribute name.")));
                        }
                        attrs.push((k.clone(), scalar(v).ok_or_else(|| bad(i, "attrs", format!("attribute '{k}' must be a string or number.")))?));
                    }
                }
                let at_start = match text("at").as_deref() {
                    None | Some("end") => false,
                    Some("start") => true,
                    Some(other) => return Err(bad(i, "at", format!("'at' is start or end, not '{other}'."))),
                };
                Op::AddNode { target, tag, attrs, text: text("text"), at_start }
            }
            _ => unreachable!(),
        });
    }
    Ok(out)
}

/// Parse one `--set 'SELECTOR.attr=value'` argument.
pub fn parse_set(arg: &str) -> Result<Op, AppError> {
    let err = || AppError::usage("invalid-argument", format!("Cannot read --set '{arg}'; the form is SELECTOR.attr=value.")).suggest(Some("Example: --set '#node-api > rect.fill=#2563eb'".into()));
    let mut depth = 0;
    let mut eq = None;
    for (i, c) in arg.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            '=' if depth == 0 => {
                eq = Some(i);
                break;
            }
            _ => {}
        }
    }
    let eq = eq.ok_or_else(err)?;
    let (left, value) = (&arg[..eq], &arg[eq + 1..]);
    let mut depth = 0;
    let mut dot = None;
    for (i, c) in left.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            '.' if depth == 0 => dot = Some(i),
            _ => {}
        }
    }
    let dot = dot.ok_or_else(err)?;
    let (target, attr) = (left[..dot].trim(), left[dot + 1..].trim());
    if target.is_empty() || !is_xml_name(attr) {
        return Err(err());
    }
    Ok(Op::SetAttr { target: target.to_string(), attr: attr.to_string(), value: value.to_string() })
}

// ---------------------------------------------------------------- start tags

struct RawAttr<'s> {
    name: &'s str,
    /// From the whitespace before the name to after the closing quote.
    full: (usize, usize),
    /// Inside the quotes.
    value: (usize, usize),
}

struct StartTag<'s> {
    name: &'s str,
    name_end: usize,
    attrs: Vec<RawAttr<'s>>,
    /// Offset of `>` or `/>`.
    close: usize,
    self_closing: bool,
}

fn scan(src: &str, start: usize) -> StartTag<'_> {
    let b = src.as_bytes();
    let mut i = start + 1;
    while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    let name_end = i;
    let mut attrs = Vec::new();
    loop {
        let ws = i;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' || b[i] == b'/' {
            break;
        }
        let ns = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = &src[ns..i];
        while i < b.len() && b[i] != b'"' && b[i] != b'\'' {
            i += 1;
        }
        let quote = b[i];
        let vs = i + 1;
        i = vs;
        while i < b.len() && b[i] != quote {
            i += 1;
        }
        attrs.push(RawAttr { name, full: (ws, i + 1), value: (vs, i) });
        i += 1;
    }
    StartTag { name: &src[start + 1..name_end], name_end, attrs, close: i, self_closing: i < b.len() && b[i] == b'/' }
}

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;")
}

#[derive(Debug, Clone, PartialEq)]
struct Patch {
    start: usize,
    end: usize,
    text: String,
}

fn set_attr(src: &str, node: Node, name: &str, value: &str, out: &mut Vec<Patch>) {
    let tag = scan(src, node.range().start);
    let v = escape(value);
    match tag.attrs.iter().find(|a| a.name == name) {
        Some(a) => {
            // Keep the author's quote style valid: a value quoted with ' must not contain one.
            let quoted_single = src.as_bytes()[a.value.0 - 1] == b'\'';
            let v = if quoted_single { v.replace('\'', "&apos;") } else { v };
            if src[a.value.0..a.value.1] != v {
                out.push(Patch { start: a.value.0, end: a.value.1, text: v });
            }
        }
        None => {
            let at = tag.attrs.last().map(|a| a.full.1).unwrap_or(tag.name_end);
            out.push(Patch { start: at, end: at, text: format!(" {name}=\"{v}\"") });
        }
    }
}

fn remove_attr(src: &str, node: Node, pattern: &str, out: &mut Vec<Patch>) {
    let tag = scan(src, node.range().start);
    for a in &tag.attrs {
        let hit = match pattern.strip_suffix('*') {
            Some(prefix) => a.name.starts_with(prefix),
            None => a.name == pattern,
        };
        if hit {
            out.push(Patch { start: a.full.0, end: a.full.1, text: String::new() });
        }
    }
}

fn style_edit(src: &str, node: Node, prop: &str, value: Option<&str>, out: &mut Vec<Patch>) {
    let current = node.attribute("style").unwrap_or("");
    let mut decls: Vec<(String, String)> = current
        .split(';')
        .filter_map(|d| d.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect();
    let before = decls.clone();
    match value {
        Some(v) => match decls.iter_mut().find(|(k, _)| k == prop) {
            Some(d) => d.1 = v.to_string(),
            None => decls.push((prop.to_string(), v.to_string())),
        },
        None => decls.retain(|(k, _)| k != prop),
    }
    if decls == before {
        return;
    }
    if decls.is_empty() {
        remove_attr(src, node, "style", out);
    } else {
        let text: Vec<String> = decls.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        set_attr(src, node, "style", &text.join("; "), out);
    }
}

/// The byte range to delete for a node; takes the whole line when the node is alone on it.
fn removal(src: &str, start: usize, end: usize) -> Patch {
    let line_start = src[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = src[end..].find('\n').map(|i| end + i + 1);
    let alone = src[line_start..start].trim().is_empty() && line_end.is_some_and(|e| src[end..e].trim().is_empty());
    match (alone, line_end) {
        (true, Some(e)) => Patch { start: line_start, end: e, text: String::new() },
        _ => Patch { start, end, text: String::new() },
    }
}

fn shift_list(value: &str, d: f64) -> Option<String> {
    let mut out = Vec::new();
    for tok in value.split(|c: char| c == ',' || c.is_whitespace()).filter(|t| !t.is_empty()) {
        out.push(fmt(tok.parse::<f64>().ok()? + d));
    }
    (!out.is_empty()).then(|| out.join(" "))
}

fn move_node(src: &str, node: Node, dx: f64, dy: f64, out: &mut Vec<Patch>) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    let tag = node.tag_name().name();
    let pairs: &[(&str, f64)] = match tag {
        "rect" | "image" | "use" | "foreignObject" | "text" => &[("x", dx), ("y", dy)],
        "circle" | "ellipse" => &[("cx", dx), ("cy", dy)],
        "line" => &[("x1", dx), ("x2", dx), ("y1", dy), ("y2", dy)],
        _ => &[],
    };
    let mut local = Vec::new();
    let mut ok = !pairs.is_empty();
    for (name, d) in pairs {
        if *d == 0.0 {
            continue;
        }
        match node.attribute(*name) {
            Some(v) => match shift_list(v, *d) {
                Some(new) => set_attr(src, node, name, &new, &mut local),
                None => ok = false,
            },
            None => set_attr(src, node, name, &fmt(*d), &mut local),
        }
    }
    if ok && tag == "text" {
        // Lines positioned absolutely move with their text.
        for t in node.descendants().filter(|n| n.is_element() && n.tag_name().name() == "tspan") {
            for (name, d) in [("x", dx), ("y", dy)] {
                if d != 0.0
                    && let Some(v) = t.attribute(name)
                {
                    match shift_list(v, d) {
                        Some(new) => set_attr(src, t, name, &new, &mut local),
                        None => ok = false,
                    }
                }
            }
        }
    }
    if ok {
        out.extend(local);
        return;
    }
    let shift = format!("translate({} {})", fmt(dx), fmt(dy));
    match node.attribute("transform") {
        Some(t) => set_attr(src, node, "transform", &format!("{shift} {t}"), out),
        None => set_attr(src, node, "transform", &shift, out),
    }
}

fn rename_id(src: &str, doc: &Document, node: Node, to: &str, out: &mut Vec<Patch>) {
    let old = node.attribute("id");
    set_attr(src, node, "id", to, out);
    let Some(old) = old else { return };
    // References resolve to the first element with an id, so they follow only that one.
    let first = doc.descendants().find(|n| n.is_element() && n.attribute("id") == Some(old));
    if first != Some(node) {
        return;
    }
    let (needle, hash) = (format!("#{old})"), format!("#{old}"));
    for n in doc.descendants().filter(|n| n.is_element()) {
        let tag = scan(src, n.range().start);
        for a in &tag.attrs {
            let raw = &src[a.value.0..a.value.1];
            if (a.name == "href" || a.name.ends_with(":href")) && raw.trim() == hash {
                out.push(Patch { start: a.value.0, end: a.value.1, text: format!("#{to}") });
            } else if raw.contains("url(") && url_refs_contain(raw, old) {
                let new = raw.replace(&needle, &format!("#{to})")).replace(&format!("#{old}\")"), &format!("#{to}\")")).replace(&format!("#{old}')"), &format!("#{to}')"));
                out.push(Patch { start: a.value.0, end: a.value.1, text: new });
            }
        }
        if n.tag_name().name() == "style" {
            for t in n.children().filter(|c| c.is_text()) {
                let r = t.range();
                let raw = &src[r.clone()];
                if raw.contains(&needle) {
                    out.push(Patch { start: r.start, end: r.end, text: raw.replace(&needle, &format!("#{to})")) });
                }
            }
        }
    }
}

fn url_refs_contain(raw: &str, id: &str) -> bool {
    let mut rest = raw;
    while let Some(i) = rest.find("url(") {
        if url_id(&rest[i..]) == Some(id) {
            return true;
        }
        rest = &rest[i + 4..];
    }
    false
}

fn add_node(src: &str, parent: Node, tag: &str, attrs: &[(String, String)], text: &Option<String>, at_start: bool, out: &mut Vec<Patch>) {
    let mut el = format!("<{tag}");
    for (k, v) in attrs {
        el.push_str(&format!(" {k}=\"{}\"", escape(v)));
    }
    match text {
        Some(t) => el.push_str(&format!(">{}</{tag}>", t.replace('&', "&amp;").replace('<', "&lt;"))),
        None => el.push_str("/>"),
    }
    let ptag = scan(src, parent.range().start);
    let indent_of = |pos: usize| -> Option<&str> {
        let ls = src[..pos].rfind('\n').map(|i| i + 1)?;
        src[ls..pos].trim().is_empty().then(|| &src[ls..pos])
    };
    if ptag.self_closing {
        let end = ptag.close + 2;
        out.push(Patch { start: ptag.close, end, text: format!(">{el}</{}>", ptag.name) });
        return;
    }
    const HEAD: &[&str] = &["title", "desc", "defs", "style", "metadata"];
    let first_body = parent.children().find(|c| c.is_element() && !HEAD.contains(&c.tag_name().name()));
    if at_start && let Some(sib) = first_body {
        let at = sib.range().start;
        let text = match indent_of(at) {
            Some(ind) => format!("{el}\n{ind}"),
            None => el,
        };
        out.push(Patch { start: at, end: at, text });
        return;
    }
    let close = src[..parent.range().end].rfind("</").unwrap_or(parent.range().end);
    match indent_of(close) {
        Some(ind) => {
            let at = close - ind.len();
            out.push(Patch { start: at, end: at, text: format!("{ind}  {el}\n") });
        }
        None => out.push(Patch { start: close, end: close, text: el }),
    }
}

/// Lookup tables that keep target resolution cheap across a large batch.
struct IdIndex<'a, 'i> {
    ids: std::collections::HashMap<&'a str, Vec<Node<'a, 'i>>>,
    /// Node id -> 1-based position among same-named siblings.
    nth: std::collections::HashMap<usize, usize>,
}

fn id_index<'a, 'i>(doc: &'a Document<'i>) -> IdIndex<'a, 'i> {
    let mut ids: std::collections::HashMap<&str, Vec<Node>> = std::collections::HashMap::new();
    let mut nth = std::collections::HashMap::new();
    for n in doc.descendants().filter(|n| n.is_element()) {
        if let Some(id) = n.attribute("id") {
            ids.entry(id).or_default().push(n);
        }
        let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for c in n.children().filter(|c| c.is_element()) {
            let k = seen.entry(c.tag_name().name()).or_insert(0);
            *k += 1;
            nth.insert(c.id().get_usize(), *k);
        }
    }
    IdIndex { ids, nth }
}

fn resolve<'a, 'i>(doc: &'a Document<'i>, ids: &IdIndex<'a, 'i>, target: &str, i: usize, wildcard_ok: bool) -> Result<Vec<Node<'a, 'i>>, AppError> {
    let no_match = || AppError::input("no-match", format!("ops[{i}]: target '{target}' matches nothing in the file.")).path(format!("ops[{i}].target"));
    if let Some(n) = target.strip_prefix("comment(").and_then(|r| r.strip_suffix(')')) {
        let n: usize = n.trim().parse().map_err(|_| bad(i, "target", "comment(N) needs a number.".into()))?;
        let c = doc.descendants().filter(|x| x.is_comment()).nth(n.wrapping_sub(1)).ok_or_else(no_match)?;
        return Ok(vec![c]);
    }
    let sel = Selector::parse(target).map_err(|e| bad(i, "target", format!("cannot read selector '{target}': {e}.")))?;
    // Most targets start at an id; search only below it instead of the whole document.
    let nth = |n: Node| ids.nth.get(&n.id().get_usize()).copied().unwrap_or(1);
    let hits: Vec<Node> = match sel.anchor_id() {
        Some(id) => ids.ids.get(id).into_iter().flatten().flat_map(|a| a.descendants()).filter(|n| n.is_element() && sel.matches_with(*n, &nth)).collect(),
        None => doc.descendants().filter(|n| n.is_element() && sel.matches_with(*n, &nth)).collect(),
    };
    if hits.is_empty() && !wildcard_ok {
        return Err(no_match());
    }
    Ok(hits)
}

/// Accepted patches, with their ranges indexed by start for overlap checks.
#[derive(Default)]
struct Taken {
    list: Vec<Patch>,
    by_start: std::collections::BTreeMap<usize, Vec<usize>>,
    longest: usize,
}

impl Taken {
    fn near(&self, p: &Patch) -> impl Iterator<Item = &Patch> {
        self.by_start.range(p.start.saturating_sub(self.longest)..=p.end).flat_map(|(_, v)| v.iter().map(|i| &self.list[*i]))
    }
    fn has(&self, p: &Patch) -> bool {
        self.near(p).any(|a| a == p)
    }
    fn clashes(&self, p: &Patch) -> bool {
        self.near(p).any(|a| p.start < a.end && a.start < p.end)
    }
    fn add(&mut self, p: Patch) {
        self.longest = self.longest.max(p.end - p.start);
        self.by_start.entry(p.start).or_default().push(self.list.len());
        self.list.push(p);
    }
}

pub struct Applied {
    pub text: String,
    /// Per group: true when it was accepted and changed the text.
    pub accepted: Vec<bool>,
}

impl Applied {
    pub fn count(&self) -> usize {
        self.accepted.iter().filter(|a| **a).count()
    }
}

fn patches_for<'a, 'i>(src: &str, doc: &'a Document<'i>, ids: &IdIndex<'a, 'i>, op: &Op, i: usize) -> Result<Vec<Patch>, AppError> {
    let mut p = Vec::new();
    match op {
        Op::SetAttr { target, attr, value } => {
            for n in resolve(doc, ids, target, i, false)? {
                set_attr(src, n, attr, value, &mut p);
            }
        }
        Op::SetStyle { target, prop, value } => {
            for n in resolve(doc, ids, target, i, false)? {
                style_edit(src, n, prop, Some(value), &mut p);
            }
        }
        Op::RemoveAttr { target, attr } => {
            for n in resolve(doc, ids, target, i, attr.contains('*') || target == "*")? {
                if n.is_element() {
                    remove_attr(src, n, attr, &mut p);
                }
            }
        }
        Op::RemoveStyle { target, prop } => {
            for n in resolve(doc, ids, target, i, false)? {
                style_edit(src, n, prop, None, &mut p);
            }
        }
        Op::RemoveElem { target } => {
            for n in resolve(doc, ids, target, i, false)? {
                if n == doc.root_element() {
                    return Err(bad(i, "target", "the root element cannot be removed.".into()));
                }
                p.push(removal(src, n.range().start, n.range().end));
            }
        }
        Op::Move { target, dx, dy } => {
            for n in resolve(doc, ids, target, i, false)? {
                move_node(src, n, *dx, *dy, &mut p);
            }
        }
        Op::RenameId { target, to } => {
            let hits = resolve(doc, ids, target, i, false)?;
            if hits.len() != 1 {
                return Err(bad(i, "target", format!("rename-id needs exactly one element; '{target}' matches {}.", hits.len())));
            }
            if hits[0].attribute("id") != Some(to.as_str()) && doc.descendants().any(|n| n.attribute("id") == Some(to.as_str())) {
                return Err(bad(i, "to", format!("id '{to}' is already in use.")));
            }
            rename_id(src, doc, hits[0], to, &mut p);
        }
        Op::AddNode { target, tag, attrs, text, at_start } => {
            let hits = resolve(doc, ids, target, i, false)?;
            if hits.len() != 1 {
                return Err(bad(i, "target", format!("add-node needs exactly one parent; '{target}' matches {}.", hits.len())));
            }
            add_node(src, hits[0], tag, attrs, text, *at_start, &mut p);
        }
    }
    Ok(p)
}

/// Apply a batch where each operation stands alone. Two operations that change
/// the same bytes are an error.
pub fn apply(src: &str, ops: &[Op]) -> Result<Applied, AppError> {
    let groups: Vec<Vec<Op>> = ops.iter().map(|o| vec![o.clone()]).collect();
    apply_groups(src, &groups, true)
}

/// Apply groups of operations; a group lands whole or not at all. With `strict`
/// off, a group that fails to resolve or collides with an earlier one is dropped.
pub fn apply_groups(src: &str, groups: &[Vec<Op>], strict: bool) -> Result<Applied, AppError> {
    let doc = Document::parse_with_options(src, svg::parse_options()).map_err(|e| AppError::input("malformed-xml", format!("The file is not well-formed XML: {e}.")))?;
    let ids = id_index(&doc);
    let mut per_group: Vec<Option<Vec<Patch>>> = Vec::new();
    let mut index = 0;
    for g in groups {
        let mut ps = Some(Vec::new());
        for op in g {
            match patches_for(src, &doc, &ids, op, index) {
                Ok(p) => {
                    if let Some(v) = ps.as_mut() {
                        v.extend(p)
                    }
                }
                Err(e) if strict => return Err(e),
                Err(_) => ps = None,
            }
            index += 1;
        }
        per_group.push(ps);
    }

    // A deletion swallows any other group's patch that lies inside it.
    let deletions: Vec<(usize, usize, usize)> = per_group
        .iter()
        .enumerate()
        .flat_map(|(i, ps)| ps.iter().flatten().filter(|p| p.text.is_empty() && p.end > p.start).map(move |p| (i, p.start, p.end)))
        .collect();
    for (i, ps) in per_group.iter_mut().enumerate() {
        if let Some(ps) = ps {
            ps.retain(|p| !deletions.iter().any(|(j, s, e)| *j != i && *s <= p.start && p.end <= *e && !(p.start == *s && p.end == *e)));
        }
    }

    let mut taken = Taken::default();
    let mut accepted = Vec::new();
    for (i, ps) in per_group.into_iter().enumerate() {
        let Some(ps) = ps else {
            accepted.push(false);
            continue;
        };
        let mut fresh: Vec<Patch> = Vec::new();
        for p in ps {
            if !taken.has(&p) && !fresh.contains(&p) {
                fresh.push(p);
            }
        }
        let clash = fresh.iter().any(|p| taken.clashes(p)) || fresh.iter().enumerate().any(|(k, p)| fresh[..k].iter().any(|a| p.start < a.end && a.start < p.end));
        if clash {
            if strict {
                return Err(AppError::input("conflicting-ops", format!("ops[{i}] changes bytes that an earlier operation already changes; split the batch.")).path(format!("ops[{i}]")));
            }
            accepted.push(false);
            continue;
        }
        accepted.push(!fresh.is_empty());
        for p in fresh {
            taken.add(p);
        }
    }

    // Stable sort keeps insertions at one point in operation order.
    let mut taken = taken.list;
    taken.sort_by_key(|p| p.start);
    let mut text = String::with_capacity(src.len() + 64);
    let mut pos = 0;
    for p in &taken {
        text.push_str(&src[pos..p.start]);
        text.push_str(&p.text);
        pos = p.end;
    }
    text.push_str(&src[pos..]);

    if text != src
        && let Err(e) = Document::parse_with_options(&text, svg::parse_options())
    {
        return Err(AppError::input("edit-breaks-xml", format!("The change would leave the file malformed ({e}); nothing was written.")));
    }
    Ok(Applied { text, accepted })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SRC: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\">\n  <g id=\"a\">\n    <rect x=\"1\" y='2' width=\"3\"/>\n    <text x=\"5\" y=\"6\">hi</text>\n  </g>\n  <rect id=\"b\" fill=\"url(#a)\"/>\n</svg>\n";

    fn run(ops: Value) -> String {
        apply(SRC, &parse_ops(&ops).unwrap()).unwrap().text
    }

    #[test]
    fn set_replaces_and_inserts() {
        let out = run(json!([{"op": "set", "target": "#a > rect", "attr": "width", "value": 164}, {"op": "set", "target": "#a > rect", "attr": "rx", "value": "4"}]));
        assert!(out.contains("<rect x=\"1\" y='2' width=\"164\" rx=\"4\"/>"));
        assert_eq!(out.replace(" width=\"164\" rx=\"4\"", " width=\"3\""), SRC);
    }

    #[test]
    fn remove_takes_the_whole_line() {
        let out = run(json!([{"op": "remove", "target": "#a > text"}]));
        assert_eq!(out, SRC.replace("    <text x=\"5\" y=\"6\">hi</text>\n", ""));
    }

    #[test]
    fn rename_follows_references() {
        let out = run(json!([{"op": "rename-id", "target": "#a", "to": "grad"}]));
        assert!(out.contains("<g id=\"grad\">") && out.contains("fill=\"url(#grad)\""));
    }

    #[test]
    fn move_shifts_coordinates_or_adds_a_transform() {
        let out = run(json!([{"op": "move", "target": "#a > text", "dy": -4}, {"op": "move", "target": "#a", "dx": 10}]));
        assert!(out.contains("<text x=\"5\" y=\"2\">") && out.contains("<g id=\"a\" transform=\"translate(10 0)\">"));
    }

    #[test]
    fn style_and_add_node() {
        let out = run(json!([{"op": "set", "target": "#b", "style": "stroke-width", "value": 2}, {"op": "add-node", "target": "svg", "tag": "rect", "attrs": {"width": 5}, "at": "start"}]));
        assert!(out.contains("style=\"stroke-width: 2\""));
        assert!(out.contains("\n  <rect width=\"5\"/>\n  <g id=\"a\">"));
    }

    #[test]
    fn strict_errors() {
        let e = parse_ops(&json!([{"op": "sett", "target": "#a"}])).unwrap_err();
        assert_eq!((e.code, e.suggestion.as_deref()), ("unknown-op", Some("Did you mean 'set'?")));
        let e = parse_ops(&json!([{"op": "set", "target": "#a", "atr": "x", "value": 1}])).unwrap_err();
        assert_eq!(e.code, "unknown-field");
        let e = apply(SRC, &parse_ops(&json!([{"op": "remove", "target": "#zzz"}])).unwrap()).err().unwrap();
        assert_eq!(e.code, "no-match");
        let clash = json!([{"op": "set", "target": "#b", "attr": "fill", "value": "red"}, {"op": "set", "target": "#b", "attr": "fill", "value": "blue"}]);
        let clash = parse_ops(&clash).unwrap();
        assert_eq!(apply(SRC, &clash).err().unwrap().code, "conflicting-ops");
        let groups: Vec<Vec<Op>> = clash.iter().map(|o| vec![o.clone()]).collect();
        let lenient = apply_groups(SRC, &groups, false).unwrap();
        assert_eq!(lenient.accepted, vec![true, false]);
        assert!(lenient.text.contains("fill=\"red\""));
    }

    #[test]
    fn set_argument_form() {
        assert_eq!(parse_set("rect[fill=\"#fff\"].stroke=#000").unwrap(), Op::SetAttr { target: "rect[fill=\"#fff\"]".into(), attr: "stroke".into(), value: "#000".into() });
        assert!(parse_set("nonsense").is_err());
    }
}

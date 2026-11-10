//! Structure rules: the file is valid, self-consistent static SVG.

use std::collections::BTreeSet;
use std::str::FromStr;

use roxmltree::Node;
use serde_json::json;

use super::Ctx;
use crate::svg::style::{PRESENTATION, url_id};
use crate::svg::{SVG_NS, XLINK_NS, href};
use crate::util::nearest;

const ELEMENTS: &[&str] = &[
    "svg", "g", "defs", "symbol", "use", "image", "switch", "a", "path", "rect", "circle", "ellipse", "line",
    "polyline", "polygon", "text", "tspan", "textPath", "marker", "linearGradient", "radialGradient", "stop",
    "pattern", "clipPath", "mask", "filter", "title", "desc", "metadata", "style", "view",
];

/// Not static SVG, but each has its own rule; unknown-element stays quiet about them.
const ELSEWHERE: &[&str] = &["script", "foreignObject", "animate", "animateTransform", "animateMotion", "animateColor", "set", "mpath"];

const CORE: &[&str] = &[
    "id", "class", "style", "lang", "tabindex", "role", "focusable", "autofocus", "requiredExtensions",
    "requiredFeatures", "systemLanguage", "externalResourcesRequired",
];

fn specific(tag: &str) -> &'static [&'static str] {
    match tag {
        "svg" => &["x", "y", "width", "height", "viewBox", "preserveAspectRatio", "version", "baseProfile", "zoomAndPan", "contentScriptType", "contentStyleType"],
        "symbol" => &["x", "y", "width", "height", "viewBox", "preserveAspectRatio", "refX", "refY"],
        "use" => &["x", "y", "width", "height", "href"],
        "image" => &["x", "y", "width", "height", "href", "preserveAspectRatio", "crossorigin", "decoding"],
        "a" => &["href", "target", "download", "rel", "hreflang", "type", "ping", "referrerpolicy"],
        "path" => &["d", "pathLength"],
        "rect" => &["x", "y", "width", "height", "rx", "ry", "pathLength"],
        "circle" => &["cx", "cy", "r", "pathLength"],
        "ellipse" => &["cx", "cy", "rx", "ry", "pathLength"],
        "line" => &["x1", "y1", "x2", "y2", "pathLength"],
        "polyline" | "polygon" => &["points", "pathLength"],
        "text" | "tspan" => &["x", "y", "dx", "dy", "rotate", "textLength", "lengthAdjust"],
        "textPath" => &["href", "startOffset", "method", "spacing", "side", "path", "textLength", "lengthAdjust"],
        "marker" => &["viewBox", "preserveAspectRatio", "refX", "refY", "markerUnits", "markerWidth", "markerHeight", "orient"],
        "linearGradient" => &["x1", "y1", "x2", "y2", "gradientUnits", "gradientTransform", "spreadMethod", "href"],
        "radialGradient" => &["cx", "cy", "r", "fx", "fy", "fr", "gradientUnits", "gradientTransform", "spreadMethod", "href"],
        "stop" => &["offset"],
        "pattern" => &["x", "y", "width", "height", "viewBox", "preserveAspectRatio", "patternUnits", "patternContentUnits", "patternTransform", "href"],
        "clipPath" => &["clipPathUnits"],
        "mask" => &["x", "y", "width", "height", "maskUnits", "maskContentUnits"],
        "filter" => &["x", "y", "width", "height", "filterUnits", "primitiveUnits", "href", "filterRes"],
        "style" => &["type", "media", "title"],
        "view" => &["viewBox", "preserveAspectRatio", "viewTarget"],
        _ => &[],
    }
}

/// Every `url(#id)` in a string.
pub fn url_refs(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find("url(") {
        if let Some(id) = url_id(&rest[i..]) {
            out.push(id);
        }
        rest = &rest[i + 4..];
    }
    out
}

/// Ids an element points at through url(), href or a style.
pub fn refs_of<'a>(node: Node<'a, '_>) -> Vec<&'a str> {
    let mut out = Vec::new();
    for a in node.attributes() {
        out.extend(url_refs(a.value()));
    }
    if let Some(id) = href(node).and_then(|h| h.strip_prefix('#')) {
        out.push(id);
    }
    if node.tag_name().name() == "style" {
        for t in node.children().filter(|c| c.is_text()) {
            out.extend(url_refs(t.text().unwrap_or("")));
        }
    }
    out
}

fn has_function(v: &str) -> bool {
    v.contains("var(") || v.contains("calc(") || v.contains("env(")
}

fn check_value(name: &str, value: &str, tag: &str) -> Option<String> {
    let v = value.trim();
    if has_function(v) || v == "inherit" || v == "initial" || v == "unset" {
        return None;
    }
    let length = |v: &str| svgtypes::Length::from_str(v).ok();
    match name {
        "width" | "height" | "r" | "rx" | "ry" | "stroke-width" | "markerWidth" | "markerHeight" => {
            if v == "auto" && matches!(name, "width" | "height" | "rx" | "ry") {
                return None;
            }
            match length(v) {
                None => Some("is not a length".into()),
                Some(l) if l.number < 0.0 => Some("must not be negative".into()),
                _ => None,
            }
        }
        "cx" | "cy" | "x1" | "y1" | "x2" | "y2" | "refX" | "refY" => length(v).is_none().then(|| "is not a length".into()),
        "x" | "y" if !matches!(tag, "text" | "tspan") => length(v).is_none().then(|| "is not a length".into()),
        "font-size" => {
            const KEYWORDS: &[&str] = &["xx-small", "x-small", "small", "medium", "large", "x-large", "xx-large", "smaller", "larger"];
            if KEYWORDS.contains(&v) {
                return None;
            }
            match length(v) {
                None => Some("is not a font size".into()),
                Some(l) if l.number < 0.0 => Some("must not be negative".into()),
                _ => None,
            }
        }
        "fill" | "stroke" => svgtypes::Paint::from_str(v).is_err().then(|| "is not a paint".into()),
        "stop-color" | "flood-color" | "lighting-color" | "color" => {
            (v != "currentColor" && v != "currentcolor" && svgtypes::Color::from_str(v).is_err()).then(|| "is not a colour".into())
        }
        "transform" | "gradientTransform" | "patternTransform" => svgtypes::Transform::from_str(v).is_err().then(|| "is not a transform".into()),
        "viewBox" => svgtypes::ViewBox::from_str(v).is_err().then(|| "is not four numbers with a positive size".into()),
        "points" => {
            let nums: Vec<_> = svgtypes::NumberListParser::from(v).collect();
            if nums.iter().any(|n| n.is_err()) {
                Some("is not a list of numbers".into())
            } else if nums.len() % 2 != 0 {
                Some("has an odd count of numbers".into())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn run(ctx: &mut Ctx) {
    let scene = ctx.scene;
    let root = scene.root;

    if ctx.on("svg-root") {
        let name = root.tag_name();
        if name.name() != "svg" {
            ctx.push("svg-root", root, None, format!("The root element is <{}>, not <svg>.", name.name()), vec![]);
        } else if name.namespace() != Some(SVG_NS) {
            let msg = match name.namespace() {
                None => "The root <svg> has no namespace; add xmlns=\"http://www.w3.org/2000/svg\".".to_string(),
                Some(ns) => format!("The root <svg> is in namespace '{ns}', not the SVG namespace."),
            };
            ctx.push("svg-root", root, None, msg, vec![]);
        }
    }

    if ctx.on("viewbox-missing") && root.tag_name().name() == "svg" && root.attribute("viewBox").is_none() {
        ctx.push("viewbox-missing", root, None, "The root <svg> has no viewBox, so the drawing cannot scale.".into(), vec![]);
    }

    let mut referenced: BTreeSet<&str> = BTreeSet::new();
    let elements: Vec<Node> = scene.doc.descendants().filter(|n| n.is_element()).collect();
    for &n in &elements {
        referenced.extend(refs_of(n));
    }

    for &n in &elements {
        if !scene.is_svg(n) {
            continue;
        }
        if n.ancestors().skip(1).any(|a| matches!(a.tag_name().name(), "foreignObject" | "metadata")) {
            continue;
        }
        let tag = n.tag_name().name();
        let known = ELEMENTS.contains(&tag) || tag.starts_with("fe");
        let elsewhere = ELSEWHERE.contains(&tag);

        if ctx.on("unknown-element") && !known && !elsewhere {
            let hint = nearest(tag, ELEMENTS.iter().copied()).map(|s| format!(" Did you mean <{s}>?")).unwrap_or_default();
            ctx.push("unknown-element", n, None, format!("<{tag}> is not a static SVG element.{hint}"), vec![]);
        }

        if ctx.on("unknown-attribute") && known && !tag.starts_with("fe") {
            let allowed = specific(tag);
            for a in n.attributes() {
                let name = a.name();
                let ok = match a.namespace() {
                    Some(ns) if ns == XLINK_NS => true,
                    Some(_) => true, // xml: and editor namespaces are not this rule's business
                    None => {
                        allowed.contains(&name)
                            || CORE.contains(&name)
                            || PRESENTATION.contains(&name)
                            || name.starts_with("data-")
                            || name.starts_with("aria-")
                            || name.starts_with("on")
                    }
                };
                if !ok {
                    let candidates = allowed.iter().chain(CORE).chain(PRESENTATION).copied();
                    let hint = nearest(name, candidates).map(|s| format!(" Did you mean '{s}'?")).unwrap_or_default();
                    ctx.push("unknown-attribute", n, None, format!("Attribute '{name}' is not valid on <{tag}>.{hint}"), vec![("attribute", json!(name))]);
                }
            }
        }

        if ctx.on("invalid-path-data")
            && tag == "path"
            && let Some(d) = n.attribute("d")
            && !d.trim().is_empty()
            && d.trim() != "none"
            && let Err(e) = kurbo::BezPath::from_svg(d)
        {
            ctx.push("invalid-path-data", n, None, format!("Path data does not parse: {e}."), vec![]);
        }

        if ctx.on("invalid-value") {
            let mut problems: Vec<(String, String, String)> = Vec::new();
            for a in n.attributes().filter(|a| a.namespace().is_none()) {
                if let Some(why) = check_value(a.name(), a.value(), tag) {
                    problems.push((a.name().to_string(), a.value().to_string(), why));
                }
            }
            if let Some(style) = n.attribute("style") {
                for d in simplecss::DeclarationTokenizer::from(style) {
                    if matches!(d.name, "fill" | "stroke" | "stroke-width" | "font-size" | "stop-color" | "color")
                        && let Some(why) = check_value(d.name, d.value, tag)
                    {
                        problems.push((d.name.to_string(), d.value.to_string(), why));
                    }
                }
            }
            for (name, value, why) in problems {
                ctx.push("invalid-value", n, None, format!("Value '{value}' of '{name}' {why}."), vec![("attribute", json!(name)), ("value", json!(value))]);
            }
        }

        if ctx.on("dangling-reference") {
            let mut seen = BTreeSet::new();
            for id in refs_of(n) {
                if !scene.ids.contains_key(id) && seen.insert(id) {
                    ctx.push("dangling-reference", n, None, format!("Reference to '#{id}' names no element in the file."), vec![("id", json!(id))]);
                }
            }
        }

        if ctx.on("unused-def")
            && n.parent_element().is_some_and(|p| p.tag_name().name() == "defs" && scene.is_svg(p))
            && !matches!(tag, "style" | "script" | "title" | "desc" | "metadata")
        {
            match n.attribute("id") {
                Some(id) if referenced.contains(id) => {}
                Some(id) => ctx.push("unused-def", n, None, format!("<{tag} id=\"{id}\"> in <defs> is never referenced."), vec![]),
                None => ctx.push("unused-def", n, None, format!("<{tag}> in <defs> has no id, so nothing can reference it."), vec![]),
            }
        }
    }

    if ctx.on("duplicate-id") {
        for (id, nodes) in &scene.ids {
            let first = scene.line(nodes[0]);
            for &n in &nodes[1..] {
                ctx.push("duplicate-id", n, None, format!("Id '{id}' is already used on line {first}."), vec![("id", json!(id)), ("first_line", json!(first))]);
            }
        }
    }
}

//! Geometry and legibility rules. On files this tool did not render they rest
//! on heuristics, and the engine marks their findings approximate.

use std::collections::BTreeMap;
use std::str::FromStr;

use kurbo::{Point, Rect};
use serde_json::json;

use super::Ctx;
use crate::svg::{self, Elem, Kind, Scene, TextInfo, selector};
use crate::util::{fmt, num};

fn text_of<'e>(e: &'e Elem) -> Option<(&'e TextInfo, Rect)> {
    match (&e.kind, e.bbox) {
        (Kind::Text(t), Some(b)) => Some((t, b)),
        _ => None,
    }
}

/// The smallest painted closed shape drawn before `ti` that encloses `p`.
pub fn container(scene: &Scene, ti: usize, p: Point, opaque: bool) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (i, e) in scene.elems[..ti].iter().enumerate() {
        let (Some(path), Some(b)) = (e.closed_shape(), e.bbox) else { continue };
        if !e.painted() || !b.contains(p) || (opaque && e.style.fill_alpha() < 0.5) {
            continue;
        }
        if !svg::contains_point(path, p) {
            continue;
        }
        if best.is_none_or(|(a, _)| b.area() <= a) {
            best = Some((b.area(), i));
        }
    }
    best.map(|(_, i)| i)
}

/// Sibling units for overlap and spacing: closed shapes, and groups stood in for by their largest shape.
fn units(scene: &Scene) -> BTreeMap<usize, Vec<(usize, Rect)>> {
    let mut frames: BTreeMap<usize, Rect> = BTreeMap::new();
    let mut map: BTreeMap<usize, Vec<(usize, Rect)>> = BTreeMap::new();
    for (i, e) in scene.elems.iter().enumerate() {
        let (Some(_), Some(b), Some(p)) = (e.closed_shape(), e.bbox, e.parent) else { continue };
        if !e.painted() || scene.is_background(e) {
            continue;
        }
        map.entry(p).or_default().push((i, b));
        let f = frames.entry(p).or_insert(b);
        if b.area() > f.area() {
            *f = b;
        }
    }
    for (g, frame) in frames {
        if let Some(p) = scene.elems[g].parent {
            map.entry(p).or_default().push((g, frame));
        }
    }
    for v in map.values_mut() {
        v.sort_by_key(|(i, _)| *i);
    }
    map
}

/// Unstroked shapes that sit tightly behind a label, such as an edge label's background.
/// They are part of the label, not nodes.
fn label_backings(scene: &Scene, texts: &[usize]) -> Vec<usize> {
    let mut out = Vec::new();
    for &ti in texts {
        let Some((t, tb)) = text_of(&scene.elems[ti]) else { continue };
        if let Some(ci) = container(scene, ti, t.anchor, false) {
            let c = &scene.elems[ci];
            if c.style.stroke_alpha() <= 0.0 && c.bbox.is_some_and(|b| b.height() <= 2.0 * tb.height()) {
                out.push(ci);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn inside(outer: Rect, inner: Rect) -> bool {
    let t = 0.5;
    inner.x0 >= outer.x0 - t && inner.y0 >= outer.y0 - t && inner.x1 <= outer.x1 + t && inner.y1 <= outer.y1 + t
}

/// How far a marker reaches past the end of its path, in the path's units.
fn marker_reach(scene: &Scene, e: &Elem, id: Option<&String>) -> f64 {
    let Some(m) = id.and_then(|id| scene.ids.get(id.as_str())).and_then(|v| v.first()) else { return 0.0 };
    let n = |name: &str, default: f64| m.attribute(name).and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(default);
    let width = n("markerWidth", 3.0);
    let ref_x = n("refX", 0.0);
    let reach = match m.attribute("viewBox").and_then(|v| svgtypes::ViewBox::from_str(v).ok()) {
        Some(vb) if vb.w > 0.0 => (vb.x + vb.w - ref_x) * width / vb.w,
        _ => width - ref_x,
    };
    let unit = if m.attribute("markerUnits") == Some("userSpaceOnUse") { 1.0 } else { e.style.stroke_width };
    (reach * unit * e.scale).max(0.0)
}

pub fn run(ctx: &mut Ctx) {
    let scene = ctx.scene;
    let elems = &scene.elems;
    let view = scene.view_scale(ctx.set.render_width);
    let name = |i: usize| selector::target(elems[i].node);

    let texts: Vec<usize> = (0..elems.len()).filter(|&i| text_of(&elems[i]).is_some()).collect();
    let connectors: Vec<usize> = (0..elems.len()).filter(|&i| elems[i].connector).collect();
    // Shapes a connector may attach to or cross.
    let bodies: Vec<usize> = (0..elems.len())
        .filter(|&i| matches!(elems[i].kind, Kind::Shape { .. }) && !elems[i].connector && elems[i].painted() && !scene.is_background(&elems[i]))
        .collect();

    let backings = label_backings(scene, &texts);

    if ctx.on("text-overflow") {
        let padding = ctx.set.opt("text-overflow", "padding");
        let reserve = ctx.set.opt("text-overflow", "reserve");
        for &ti in &texts {
            let (t, tb) = text_of(&elems[ti]).unwrap();
            let Some(ci) = container(scene, ti, t.anchor, false) else { continue };
            let cb = elems[ci].bbox.unwrap();
            if cb.height() < tb.height() * 0.5 && cb.width() < tb.height() {
                continue; // a dot or tick under the anchor, not a container
            }
            let need = tb.width() * (1.0 + reserve);
            // A label's own background hugs the text by design; only real spill counts there.
            let padding = if backings.contains(&ci) { 0.0 } else { padding };
            let available = (cb.width() - 2.0 * padding).max(0.0);
            let measured = |extra: Vec<(&'static str, serde_json::Value)>| {
                let mut m = extra;
                m.push(("container", json!(name(ci))));
                m
            };
            if need > available + 0.01 {
                let msg = format!("Label is {} px wide; its container allows {} px.", fmt(need), fmt(available));
                let m = measured(vec![("text_width", num(need)), ("available", num(available))]);
                if t.approximate { ctx.push_approx("text-overflow", elems[ti].node, Some(tb), msg, m) } else { ctx.push("text-overflow", elems[ti].node, Some(tb), msg, m) }
            } else if tb.height() > cb.height() + 0.01 {
                let msg = format!("Label is {} px tall; its container is {} px tall.", fmt(tb.height()), fmt(cb.height()));
                let m = measured(vec![("text_height", num(tb.height())), ("available", num(cb.height()))]);
                if t.approximate { ctx.push_approx("text-overflow", elems[ti].node, Some(tb), msg, m) } else { ctx.push("text-overflow", elems[ti].node, Some(tb), msg, m) }
            }
        }
    }

    if ctx.on("text-overlap") {
        let mut order = texts.clone();
        order.sort_by(|&a, &b| elems[a].bbox.unwrap().x0.total_cmp(&elems[b].bbox.unwrap().x0));
        let mut hit: BTreeMap<usize, (usize, Rect)> = BTreeMap::new();
        for (k, &a) in order.iter().enumerate() {
            let ab = elems[a].bbox.unwrap();
            for &b in &order[k + 1..] {
                let bb = elems[b].bbox.unwrap();
                if bb.x0 >= ab.x1 {
                    break;
                }
                let i = ab.intersect(bb);
                if i.width() > 1.0 && i.height() > 0.15 * ab.height().min(bb.height()) {
                    let (later, earlier) = (a.max(b), a.min(b));
                    hit.entry(later).or_insert((earlier, i));
                }
            }
        }
        for (later, (earlier, i)) in hit {
            let approx = [later, earlier].iter().any(|&x| text_of(&elems[x]).unwrap().0.approximate);
            let msg = format!("Text overlaps the text at {} by {} x {} px.", name(earlier), fmt(i.width()), fmt(i.height()));
            let m = vec![("other", json!(name(earlier))), ("overlap_width", num(i.width())), ("overlap_height", num(i.height()))];
            if approx { ctx.push_approx("text-overlap", elems[later].node, elems[later].bbox, msg, m) } else { ctx.push("text-overlap", elems[later].node, elems[later].bbox, msg, m) }
        }
    }

    if ctx.on("shape-overlap") || ctx.on("tight-spacing") {
        let min_gap = ctx.set.opt("tight-spacing", "min_gap");
        for group in units(scene).values() {
            let mut tight: BTreeMap<usize, (usize, f64)> = BTreeMap::new();
            for (k, &(a, ab)) in group.iter().enumerate() {
                for &(b, bb) in &group[k + 1..] {
                    // A group and the shape that stands in for it are the same unit.
                    if elems[b].parent == Some(a) || elems[a].parent == Some(b) {
                        continue;
                    }
                    let i = ab.intersect(bb);
                    if i.width() > 1.0 && i.height() > 1.0 {
                        if ctx.on("shape-overlap") && !inside(ab, bb) && !inside(bb, ab) {
                            let msg = format!("Shape overlaps {} by {} x {} px without containing it.", name(a), fmt(i.width()), fmt(i.height()));
                            ctx.push("shape-overlap", elems[b].node, Some(bb), msg, vec![("other", json!(name(a))), ("overlap_width", num(i.width())), ("overlap_height", num(i.height()))]);
                        }
                        continue;
                    }
                    let overlap_x = ab.x0.max(bb.x0) < ab.x1.min(bb.x1);
                    let overlap_y = ab.y0.max(bb.y0) < ab.y1.min(bb.y1);
                    let gap = if overlap_y && !overlap_x {
                        (bb.x0 - ab.x1).max(ab.x0 - bb.x1)
                    } else if overlap_x && !overlap_y {
                        (bb.y0 - ab.y1).max(ab.y0 - bb.y1)
                    } else {
                        continue;
                    };
                    if gap > 0.01 && gap < min_gap - 0.01 && tight.get(&b).is_none_or(|(_, g)| gap < *g) {
                        tight.insert(b, (a, gap));
                    }
                }
            }
            if ctx.on("tight-spacing") {
                for (b, (a, gap)) in tight {
                    let msg = format!("Gap to {} is {} px; the minimum is {} px.", name(a), fmt(gap), fmt(min_gap));
                    ctx.push("tight-spacing", elems[b].node, elems[b].bbox, msg, vec![("other", json!(name(a))), ("gap", num(gap)), ("min_gap", num(min_gap))]);
                }
            }
        }
    }

    if let Some(canvas) = scene.canvas {
        if ctx.on("content-clipped") {
            for e in elems.iter().filter(|e| e.is_leaf()) {
                let Some(v) = e.vbox else { continue };
                let over = [canvas.x0 - v.x0, canvas.y0 - v.y0, v.x1 - canvas.x1, v.y1 - canvas.y1];
                if over.iter().any(|o| *o > 0.5) {
                    let sides: Vec<String> = ["left", "top", "right", "bottom"].iter().zip(over).filter(|(_, o)| *o > 0.5).map(|(s, o)| format!("{} px past the {s}", fmt(o))).collect();
                    let msg = format!("Content extends outside the viewBox: {}.", sides.join(", "));
                    let m = vec![("left", num(over[0].max(0.0))), ("top", num(over[1].max(0.0))), ("right", num(over[2].max(0.0))), ("bottom", num(over[3].max(0.0)))];
                    ctx.push("content-clipped", e.node, Some(v), msg, m);
                }
            }
        }

        if ctx.on("excess-margin") {
            let max_ratio = ctx.set.opt("excess-margin", "max_ratio");
            let mut content: Option<Rect> = None;
            for e in elems.iter().filter(|e| e.is_leaf() && !scene.is_background(e)) {
                if let Some(v) = e.vbox {
                    content = Some(content.map_or(v, |c| c.union(v)));
                }
            }
            if let Some(c) = content {
                let m = [
                    ("left", (c.x0 - canvas.x0) / canvas.width()),
                    ("top", (c.y0 - canvas.y0) / canvas.height()),
                    ("right", (canvas.x1 - c.x1) / canvas.width()),
                    ("bottom", (canvas.y1 - c.y1) / canvas.height()),
                ];
                if let Some((side, worst)) = m.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1))
                    && worst > max_ratio + 1e-9
                {
                    let msg = format!("Empty margin on the {side} is {}% of the drawing; the limit is {}%.", fmt(worst * 100.0), fmt(max_ratio * 100.0));
                    let mut measured: Vec<(&'static str, serde_json::Value)> = m.iter().map(|(s, r)| (*s, num(r * 100.0))).collect();
                    measured.push(("limit", num(max_ratio * 100.0)));
                    ctx.push("excess-margin", scene.root, Some(c), msg, measured);
                }
            }
        }
    }

    // Per connector: its two ends and how far past them a marker reaches.
    type End = (Point, f64, &'static str);
    let ends: Vec<(usize, [End; 2])> = connectors
        .iter()
        .filter_map(|&ci| {
            let e = &elems[ci];
            let (a, b) = e.ends()?;
            Some((ci, [(a, marker_reach(scene, e, e.style.marker_start.as_ref()), "start"), (b, marker_reach(scene, e, e.style.marker_end.as_ref()), "end")]))
        })
        .collect();
    let max_gap = ctx.set.opt("edge-detached", "max_gap");

    if ctx.on("edge-detached") && !bodies.is_empty() {
        for (ci, pair) in &ends {
            for (p, reach, which) in pair {
                let mut best = (f64::INFINITY, usize::MAX);
                for &bi in &bodies {
                    let b = elems[bi].vbox.unwrap().inflate(best.0.min(1e9), best.0.min(1e9));
                    if best.0.is_finite() && !b.contains(*p) {
                        continue;
                    }
                    let d = svg::distance_to(&elems[bi].outline, *p);
                    if d < best.0 {
                        best = (d, bi);
                    }
                }
                let gap = (best.0 - reach).max(0.0);
                // An end inside a filled shape visibly reaches that shape.
                let sunk = gap > max_gap && bodies.iter().any(|&bi| {
                    let b = &elems[bi];
                    b.style.fill_alpha() > 0.0 && b.bbox.is_some_and(|r| r.contains(*p)) && b.closed_shape().is_some_and(|path| svg::contains_point(path, *p))
                });
                if gap > max_gap + 0.01 && best.1 != usize::MAX && !sunk {
                    let msg = format!("Connector {which} is {} px from the nearest shape edge ({}); the limit is {} px.", fmt(gap), name(best.1), fmt(max_gap));
                    let m = vec![("end", json!(which)), ("gap", num(gap)), ("max_gap", num(max_gap)), ("nearest", json!(name(best.1))), ("point", json!([num(p.x), num(p.y)]))];
                    ctx.push("edge-detached", elems[*ci].node, elems[*ci].bbox, msg, m);
                }
            }
        }
    }

    if ctx.on("edge-through-node") {
        for (ci, pair) in &ends {
            let c = &elems[*ci];
            let Some(cb) = c.bbox else { continue };
            for &bi in &bodies {
                let s = &elems[bi];
                let (Some(path), Some(sb)) = (s.closed_shape(), s.bbox) else { continue };
                if backings.contains(&bi) {
                    continue;
                }
                if sb.x1 < cb.x0 || sb.x0 > cb.x1 || sb.y1 < cb.y0 || sb.y0 > cb.y1 {
                    continue;
                }
                let holds_end = pair.iter().any(|(p, reach, _)| svg::contains_point(path, *p) || svg::distance_to(&s.outline, *p) <= max_gap + reach + 0.01);
                if holds_end {
                    continue;
                }
                let mut inside_len = 0.0;
                for poly in &c.outline {
                    for w in poly.windows(2) {
                        let len = w[0].distance(w[1]);
                        let steps = (len / 2.0).ceil().max(1.0) as usize;
                        for k in 0..=steps {
                            let p = w[0].lerp(w[1], k as f64 / steps as f64);
                            if sb.contains(p) && svg::contains_point(path, p) && svg::distance_to(&s.outline, p) > 1.5 {
                                inside_len += len / steps as f64;
                            }
                        }
                    }
                }
                if inside_len > 2.0 {
                    let msg = format!("Connector crosses {}, which is not one of its endpoints, for about {} px.", name(bi), fmt(inside_len));
                    ctx.push("edge-through-node", c.node, c.bbox, msg, vec![("crossed", json!(name(bi))), ("length", num(inside_len))]);
                }
            }
        }
    }

    if ctx.on("label-on-line") {
        for &ti in &texts {
            let (t, tb) = text_of(&elems[ti]).unwrap();
            let probe = tb.inflate(-1.0f64.min(tb.width() / 4.0), -(tb.height() * 0.2));
            let cover = container(scene, ti, tb.center(), true);
            for &ci in &connectors {
                let c = &elems[ci];
                if c.vbox.is_none_or(|v| v.intersect(probe).area() <= 0.0 && !(v.x0 <= probe.x1 && v.x1 >= probe.x0 && v.y0 <= probe.y1 && v.y1 >= probe.y0)) {
                    continue;
                }
                if cover.is_some_and(|k| k > ci) {
                    continue; // an opaque shape drawn over the line hides it behind the label
                }
                let hit = c.outline.iter().any(|poly| poly.windows(2).any(|w| svg::segment_hits_rect(w[0], w[1], probe)));
                if hit {
                    let msg = format!("Label \"{}\" sits on the connector {}.", t.content.replace('\n', " "), name(ci));
                    ctx.push("label-on-line", elems[ti].node, Some(tb), msg, vec![("connector", json!(name(ci)))]);
                    break;
                }
            }
        }
    }

    if ctx.on("font-too-small") {
        let min_px = ctx.set.opt("font-too-small", "min_px");
        for &ti in &texts {
            let e = &elems[ti];
            let (t, tb) = text_of(e).unwrap();
            let px = t.font_px * e.scale * view;
            if px < min_px - 0.01 {
                let msg = format!("Text renders at {} px when the diagram is {} px wide; the minimum is {} px.", fmt(px), fmt(ctx.set.render_width), fmt(min_px));
                ctx.push("font-too-small", e.node, Some(tb), msg, vec![("rendered_px", num(px)), ("min_px", num(min_px)), ("font_size", num(t.font_px)), ("scale", num(e.scale * view))]);
            }
        }
    }

    if ctx.on("stroke-too-thin") {
        let min_px = ctx.set.opt("stroke-too-thin", "min_px");
        for e in elems.iter().filter(|e| matches!(e.kind, Kind::Shape { .. }) && e.style.stroke_alpha() > 0.0) {
            let px = e.style.stroke_width * e.scale * view;
            if px < min_px - 0.001 {
                let msg = format!("Stroke renders at {} px when the diagram is {} px wide; the minimum is {} px.", fmt(px), fmt(ctx.set.render_width), fmt(min_px));
                ctx.push("stroke-too-thin", e.node, e.bbox, msg, vec![("rendered_px", num(px)), ("min_px", num(min_px)), ("stroke_width", num(e.style.stroke_width))]);
            }
        }
    }

    if ctx.on("off-grid")
        && let Some(grid) = ctx.set.grid
    {
        let off = |v: f64| ((v / grid) - (v / grid).round()).abs() > 1e-6;
        for e in elems {
            let Some(b) = e.bbox else { continue };
            let values: Vec<(&str, f64)> = match e.tag {
                "rect" => vec![("x", b.x0), ("y", b.y0), ("width", b.width()), ("height", b.height())],
                "circle" | "ellipse" => vec![("cx", b.center().x), ("cy", b.center().y)],
                "line" => match e.ends() {
                    Some((p, q)) => vec![("x1", p.x), ("y1", p.y), ("x2", q.x), ("y2", q.y)],
                    None => vec![],
                },
                _ => vec![],
            };
            let bad: Vec<String> = values.iter().filter(|(_, v)| off(*v)).map(|(k, v)| format!("{k}={}", fmt(*v))).collect();
            if !bad.is_empty() {
                let msg = format!("Off the {} px grid: {}.", fmt(grid), bad.join(", "));
                ctx.push("off-grid", e.node, Some(b), msg, vec![("grid", num(grid)), ("off", json!(bad))]);
            }
        }
    }
}

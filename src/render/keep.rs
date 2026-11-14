//! Keeping a layout: read where the boxes of an earlier drawing are, and put the
//! boxes of the new one in the same places. Boxes that are new go into free room;
//! what cannot stay is listed, so the caller can stop or lay out afresh.

use std::collections::BTreeMap;

use kurbo::{Point, Rect};

use super::layout::{self, GAP, Geometry, Measured, PAD, PAD_LABEL, RANK_GAP, Tuning, snap, up};
use crate::envelope::AppError;
use crate::spec::{Diagram, Direction, Ref, Spec};
use crate::svg::{self, Scene};

/// Where things are in a drawing this tool rendered earlier.
#[derive(Debug, Default)]
pub struct Previous {
    pub nodes: BTreeMap<String, Rect>,
    pub groups: BTreeMap<String, Rect>,
    /// Note boxes in the order of the spec's notes.
    pub notes: Vec<Rect>,
}

/// One thing that cannot stay where it was, and why.
#[derive(Clone, Debug)]
pub struct Reason {
    pub id: String,
    pub why: String,
}

/// What became of the earlier layout.
#[derive(Clone, Debug, Default)]
pub struct Kept {
    /// Boxes in both drawings.
    pub kept: Vec<String>,
    /// Boxes only in the new one.
    pub placed: Vec<String>,
    /// Boxes only in the old one.
    pub gone: Vec<String>,
    /// Empty when every kept box is where it was.
    pub reasons: Vec<Reason>,
}

fn tidy(r: Rect) -> Rect {
    let t = |v: f64| (v * 1000.0).round() / 1000.0;
    Rect::new(t(r.x0), t(r.y0), t(r.x1), t(r.y1))
}

/// The box a node's shape was drawn in. Two shapes are drawn a little inside
/// their box: the ring of an end marker by half its stroke, and the wavy foot
/// of a document by 2 px.
fn box_of_shape(e: &svg::Elem, bbox: Rect) -> Rect {
    match e.tag {
        "circle" => {
            let r = e.node.attribute("r").and_then(|r| r.parse::<f64>().ok()).unwrap_or(0.0);
            let lost = r.ceil() - r;
            tidy(bbox.inflate(lost, lost))
        }
        "path" if e.node.attribute("d").is_some_and(|d| d.contains('Q')) => tidy(Rect::new(bbox.x0, bbox.y0, bbox.x1, bbox.y1 + 2.0)),
        _ => tidy(bbox),
    }
}

/// Read the boxes and frames of a rendered drawing.
pub fn read(text: &str, name: &str) -> Result<Previous, AppError> {
    let doc = roxmltree::Document::parse_with_options(text, svg::parse_options()).map_err(|e| AppError::input("malformed-xml", format!("{name} is not well-formed XML: {e}.")).path(name))?;
    let scene = Scene::build(&doc);
    if !scene.svgdog_output {
        return Err(AppError::input("not-svgdog-output", format!("{name} was not rendered by svgdog, so its layout cannot be read.")).path(name).suggest(Some("Pass the SVG that 'svgdog render' wrote for the earlier spec, or drop --previous.".into())));
    }
    let mut out = Previous::default();
    let mut notes: BTreeMap<usize, Rect> = BTreeMap::new();
    for e in &scene.elems {
        let (Some(parent), Some(bbox), svg::Kind::Shape { .. }) = (e.parent, e.bbox, &e.kind) else { continue };
        let Some(owner) = scene.elems[parent].node.attribute("id") else { continue };
        if let Some(id) = owner.strip_prefix("node-") {
            // The shape itself comes first; a lifeline, a place on a timeline's axis
            // and step badges are drawn with the node but are not its box.
            // Text set as outlines is a path with no stroke.
            let beside = e.tag == "line" || e.node.has_attribute("data-badge") || (e.tag == "circle" && e.node.attribute("r") == Some("5")) || (e.tag == "path" && !e.node.has_attribute("stroke"));
            if !beside && !out.nodes.contains_key(id) {
                out.nodes.insert(id.to_string(), box_of_shape(e, bbox));
            }
        } else if let Some(id) = owner.strip_prefix("group-") {
            if e.tag == "rect" && !out.groups.contains_key(id) {
                out.groups.insert(id.to_string(), tidy(bbox));
            }
        } else if let Some(n) = owner.strip_prefix("note-").and_then(|n| n.parse::<usize>().ok())
            && e.tag == "rect"
        {
            notes.entry(n).or_insert_with(|| tidy(bbox));
        }
    }
    out.notes = notes.into_values().collect();
    Ok(out)
}

fn same_size(a: Rect, w: f64, h: f64) -> bool {
    (a.width() - w).abs() < 0.5 && (a.height() - h).abs() < 0.5
}

/// Which boxes are in both drawings, only in the new one, only in the old one.
fn census(spec: &Spec, prev: &Previous) -> Kept {
    let mut k = Kept::default();
    for n in &spec.nodes {
        if prev.nodes.contains_key(&n.id) { k.kept.push(n.id.clone()) } else { k.placed.push(n.id.clone()) }
    }
    k.gone = prev.nodes.keys().filter(|id| !matches!(spec.lookup(id), Some(Ref::Node(_)))).cloned().collect();
    k
}

/// True when the earlier drawing can steer where boxes go: the diagram is one whose
/// layout is computed, and at least one box is in both.
pub fn pins(spec: &Spec, prev: &Previous) -> bool {
    matches!(spec.diagram, Diagram::Flow | Diagram::State | Diagram::Layers) && spec.nodes.iter().any(|n| prev.nodes.contains_key(&n.id))
}

/// The spacing level the earlier drawing was set at: the one at which most boxes
/// and notes have the size they had. Rendering a drawing again over itself then
/// measures everything as before.
pub fn level(spec: &Spec, prev: &Previous, theme: &super::theme::Theme) -> usize {
    let mut best = (0, 0);
    for (li, t) in layout::LEVELS.iter().enumerate() {
        let m = layout::measure(spec, t, theme);
        let mut score = spec.nodes.iter().enumerate().filter(|(i, n)| prev.nodes.get(&n.id).is_some_and(|r| same_size(*r, m.nodes[*i].w, m.nodes[*i].h))).count();
        if prev.notes.len() == spec.notes.len() {
            score += prev.notes.iter().zip(&m.notes).filter(|(r, n)| same_size(**r, n.w, n.h)).count();
        }
        if li == 0 || score > best.1 {
            best = (li, score);
        }
    }
    best.0
}

/// For layouts that follow the order of the spec: lay out as usual, then say which
/// of the boxes in both drawings moved against the others.
pub fn compare(spec: &Spec, geo: &Geometry, prev: &Previous) -> Kept {
    let mut k = census(spec, prev);
    if spec.diagram == Diagram::Free {
        // Pinned boxes are where the spec says; there is nothing to keep or to lose.
        return k;
    }
    // Twice the centre's shift, as whole numbers, so that boxes can be counted by it.
    let shift = |i: usize, old: Rect| -> (i64, i64) {
        let (a, b) = (old.center(), geo.nodes[i].center());
        (((b.x - a.x) * 2.0).round() as i64, ((b.y - a.y) * 2.0).round() as i64)
    };
    let shifts: Vec<(usize, (i64, i64))> = spec.nodes.iter().enumerate().filter_map(|(i, n)| prev.nodes.get(&n.id).map(|r| (i, shift(i, *r)))).collect();
    let mut common: Option<((i64, i64), usize)> = None;
    for (_, s) in &shifts {
        let count = shifts.iter().filter(|(_, o)| o == s).count();
        if common.is_none_or(|(_, c)| count > c) {
            common = Some((*s, count));
        }
    }
    if let Some((base, _)) = common {
        for (i, s) in &shifts {
            if *s != base {
                let (dx, dy) = ((s.0 - base.0) as f64 / 2.0, (s.1 - base.1) as f64 / 2.0);
                k.reasons.push(Reason { id: spec.nodes[*i].id.clone(), why: format!("'{}' would move by {}, {} against the boxes that stay", spec.nodes[*i].id, crate::util::fmt(dx), crate::util::fmt(dy)) });
            }
        }
    }
    k
}

/// How close two things may come in a drawing whose boxes are kept in place.
const NEAR: f64 = 16.0;
/// The same for two boxes with an edge between them: room for both ends of the line.
const NEAR_JOINED: f64 = 32.0;
/// How far from where a new box belongs a free spot is looked for, in 16 px steps.
const REACH: i64 = 40;
/// The margin a drawing has on each side, at most.
const MARGIN: f64 = 24.0;

fn overlap(a: Rect, b: Rect) -> bool {
    a.intersect(b).area() > 1e-6
}

struct Pin<'a> {
    spec: &'a Spec,
    m: &'a Measured,
    prev: &'a Previous,
    /// Each node's own box, once it has a place.
    own: Vec<Option<Rect>>,
}

impl Pin<'_> {
    /// A node's box with its notes and loop label, for a box at `own`.
    fn footprint(&self, i: usize, own: Rect) -> Rect {
        let f = layout::foot(self.spec, self.m, Ref::Node(i), own.width(), own.height());
        Rect::new(own.x0 - f.ox, own.y0 - f.oy, own.x0 - f.ox + f.w, own.y0 - f.oy + f.h)
    }

    fn placed(&self, i: usize) -> Option<Rect> {
        self.own[i].map(|r| self.footprint(i, r))
    }

    fn under(&self, r: Ref, g: usize) -> bool {
        self.spec.ancestors(r).contains(&g)
    }

    /// Room a frame keeps round what it holds.
    fn padded(&self, g: usize, inner: Rect) -> Rect {
        let top = if self.spec.groups[g].label.is_some() { PAD_LABEL } else { PAD };
        let need = up(self.m.group_label[g] + 32.0, 16.0);
        let grow = ((need - (inner.width() + 2.0 * PAD)).max(0.0) / 2.0 / 8.0).ceil() * 8.0;
        Rect::new(inner.x0 - PAD - grow, inner.y0 - top, inner.x1 + PAD + grow, inner.y1 + PAD)
    }

    /// The smallest frame round the members that have a place, or none.
    fn wrap(&self, g: usize) -> Option<Rect> {
        let mut inner: Option<Rect> = None;
        for k in layout::children(self.spec, Some(g)) {
            let r = match k {
                Ref::Node(i) => self.placed(i),
                Ref::Group(h) => self.frame(h),
                Ref::Edge(_) => None,
            };
            if let Some(r) = r {
                inner = Some(inner.map_or(r, |a| a.union(r)));
            }
        }
        inner.map(|r| self.padded(g, r))
    }

    fn old(&self, g: usize) -> Option<Rect> {
        self.prev.groups.get(&self.spec.groups[g].id).copied()
    }

    /// A frame while boxes are still being placed: the old one, grown round its members.
    fn frame(&self, g: usize) -> Option<Rect> {
        match (self.old(g), self.wrap(g)) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        }
    }

    /// Whether a new node can have its box at `own` without crowding anything.
    fn free(&self, i: usize, own: Rect, frames: &[Option<Rect>]) -> bool {
        let fp = self.footprint(i, own);
        let (along, across) = if self.spec.direction == Direction::Right { (RANK_GAP, GAP) } else { (GAP, RANK_GAP) };
        for j in 0..self.spec.nodes.len() {
            if let Some(other) = self.placed(j)
                && overlap(fp, other.inflate(along - 0.01, across - 0.01))
            {
                return false;
            }
        }
        let mine = self.spec.ancestors(Ref::Node(i));
        // A spot that makes the drawing wider than it may be is no spot, unless it is that wide already.
        let span = |with: Option<Rect>| {
            let all = (0..self.spec.nodes.len()).filter_map(|j| self.placed(j)).chain(frames.iter().flatten().copied()).chain(with).reduce(|a, b| a.union(b));
            all.map_or(0.0, |r| r.width())
        };
        let widest = self.spec.max_width - 2.0 * MARGIN;
        let outer = mine.iter().fold(fp, |inner, g| self.padded(*g, inner));
        if span(Some(outer)) > widest && span(Some(outer)) > span(None) {
            return false;
        }
        let room = fp.inflate(NEAR, NEAR);
        if frames.iter().enumerate().any(|(g, f)| !mine.contains(&g) && f.is_some_and(|f| overlap(room, f))) {
            return false;
        }
        // The frames round it grow to hold it; they must not reach anything outside them.
        let mut inner = fp;
        for g in &mine {
            let grown = match frames[*g] {
                Some(f) => f.union(self.padded(*g, inner)),
                None => self.padded(*g, inner),
            };
            let room = grown.inflate(NEAR, NEAR);
            for j in 0..self.spec.nodes.len() {
                if j != i
                    && !self.under(Ref::Node(j), *g)
                    && let Some(other) = self.placed(j)
                    && overlap(room, other)
                {
                    return false;
                }
            }
            for (h, f) in frames.iter().enumerate() {
                let apart = h != *g && !self.under(Ref::Group(h), *g) && !self.under(Ref::Group(*g), h);
                if apart && f.is_some_and(|f| overlap(room, f)) {
                    return false;
                }
            }
            inner = grown;
        }
        true
    }
}

/// Place the boxes of `spec` where the earlier drawing had them. `fresh` is the
/// layout the spec would get on its own; it says where a new box belongs relative
/// to the others.
pub fn pinned(spec: &Spec, m: &Measured, t: &Tuning, prev: &Previous) -> (Geometry, Kept) {
    let fresh = layout::layout(spec, m, t);
    let mut kept = census(spec, prev);
    let mut p = Pin { spec, m, prev, own: vec![None; spec.nodes.len()] };
    let mut resized = vec![false; spec.nodes.len()];

    // Boxes in both drawings keep their centre.
    for (i, n) in spec.nodes.iter().enumerate() {
        if let Some(old) = prev.nodes.get(&n.id) {
            let (w, h) = (m.nodes[i].w, m.nodes[i].h);
            if same_size(*old, w, h) {
                p.own[i] = Some(*old);
            } else {
                let c = old.center();
                let (x, y) = (snap(c.x - w / 2.0), snap(c.y - h / 2.0));
                p.own[i] = Some(Rect::new(x, y, x + w, y + h));
                resized[i] = true;
            }
        }
    }

    // Nearest first, and in a fixed order among equals.
    let mut spots: Vec<(i64, i64)> = (-REACH..=REACH).flat_map(|dy| (-REACH..=REACH).map(move |dx| (dx, dy))).collect();
    spots.sort_by_key(|(dx, dy)| (dx * dx + dy * dy, dy.abs(), dx.abs(), *dy, *dx));

    // New boxes: where the fresh layout puts them beside their neighbours, moved to the nearest free spot.
    for i in 0..spec.nodes.len() {
        if p.own[i].is_some() {
            continue;
        }
        let has = |j: &usize| p.own[*j].is_some();
        let node = |id: &str| match spec.lookup(id) {
            Some(Ref::Node(j)) => Some(j),
            _ => None,
        };
        let mut near: Vec<usize> = spec.edges.iter().filter_map(|e| match (node(&e.from), node(&e.to)) {
            (Some(a), Some(b)) if a == i && b != i => Some(b),
            (Some(a), Some(b)) if b == i && a != i => Some(a),
            _ => None,
        }).filter(has).collect();
        if near.is_empty() {
            near = (0..spec.nodes.len()).filter(|j| spec.node_parent[*j] == spec.node_parent[i]).filter(has).collect();
        }
        if near.is_empty() {
            near = (0..spec.nodes.len()).filter(has).collect();
        }
        near.sort();
        near.dedup();
        let (mut x, mut y) = (0.0, 0.0);
        for j in &near {
            let at = p.own[*j].unwrap().center() + (fresh.nodes[i].center() - fresh.nodes[*j].center());
            x += at.x;
            y += at.y;
        }
        let (w, h) = (m.nodes[i].w, m.nodes[i].h);
        let want = Point::new(snap(x / near.len() as f64 - w / 2.0), snap(y / near.len() as f64 - h / 2.0));
        let frames: Vec<Option<Rect>> = (0..spec.groups.len()).map(|g| p.frame(g)).collect();
        let at = |dx: i64, dy: i64| Rect::new(want.x + 16.0 * dx as f64, want.y + 16.0 * dy as f64, want.x + 16.0 * dx as f64 + w, want.y + 16.0 * dy as f64 + h);
        match spots.iter().find(|(dx, dy)| p.free(i, at(*dx, *dy), &frames)) {
            Some((dx, dy)) => p.own[i] = Some(at(*dx, *dy)),
            None => {
                p.own[i] = Some(at(0, 0));
                kept.reasons.push(Reason { id: spec.nodes[i].id.clone(), why: format!("there is no free room for the new box '{}' near the boxes it belongs with", spec.nodes[i].id) });
            }
        }
    }

    // Frames, innermost first: the old one where it still holds its members and nothing else.
    let fp: Vec<Rect> = (0..spec.nodes.len()).map(|i| p.placed(i).unwrap_or(Rect::ZERO)).collect();
    let mut frames: Vec<Option<Rect>> = vec![None; spec.groups.len()];
    let mut order: Vec<usize> = (0..spec.groups.len()).collect();
    order.sort_by_key(|g| (std::cmp::Reverse(spec.ancestors(Ref::Group(*g)).len()), *g));
    for g in order {
        let mut inner: Option<Rect> = None;
        for k in layout::children(spec, Some(g)) {
            let r = match k {
                Ref::Node(i) => Some(fp[i]),
                Ref::Group(h) => frames[h],
                Ref::Edge(_) => None,
            };
            if let Some(r) = r {
                inner = Some(inner.map_or(r, |a| a.union(r)));
            }
        }
        let tight = inner.map(|r| p.padded(g, r));
        let foreign = |f: Rect| (0..spec.nodes.len()).any(|j| !p.under(Ref::Node(j), g) && overlap(f, fp[j]));
        frames[g] = match (p.old(g), tight) {
            (Some(old), Some(tight)) => {
                let both = old.union(tight);
                Some(if foreign(both) { tight } else { both })
            }
            (old, tight) => tight.or(old),
        };
    }

    // Bands keep one common length, as in a fresh layout.
    if spec.diagram == Diagram::Layers {
        let bands: Vec<usize> = (0..spec.groups.len()).filter(|g| spec.group_parent[*g].is_none() && frames[*g].is_some()).collect();
        let right = spec.direction == Direction::Right;
        let ends = bands.iter().filter_map(|g| frames[*g]).map(|f| if right { (f.x0, f.x1) } else { (f.y0, f.y1) }).reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)));
        if let Some((lo, hi)) = ends {
            for g in bands {
                let f = frames[g].unwrap();
                frames[g] = Some(if right { Rect::new(lo, f.y0, hi, f.y1) } else { Rect::new(f.x0, lo, f.x1, hi) });
            }
        }
    }

    // What no longer fits.
    let joined = |a: usize, b: usize| spec.edges.iter().any(|e| matches!((spec.lookup(&e.from), spec.lookup(&e.to)), (Some(Ref::Node(x)), Some(Ref::Node(y))) if (x == a && y == b) || (x == b && y == a)));
    for a in 0..spec.nodes.len() {
        for b in a + 1..spec.nodes.len() {
            if !(resized[a] || resized[b]) {
                continue;
            }
            let near = if joined(a, b) { NEAR_JOINED } else { NEAR } / 2.0 - 0.01;
            if overlap(fp[a].inflate(near, near), fp[b].inflate(near, near)) {
                let (big, other) = if resized[a] { (a, b) } else { (b, a) };
                kept.reasons.push(Reason { id: spec.nodes[big].id.clone(), why: format!("'{}' changed size and no longer fits beside '{}'", spec.nodes[big].id, spec.nodes[other].id) });
            }
        }
    }
    for g in 0..spec.groups.len() {
        let Some(f) = frames[g] else { continue };
        for (j, n) in spec.nodes.iter().enumerate() {
            if !p.under(Ref::Node(j), g) && overlap(f, fp[j]) {
                kept.reasons.push(Reason { id: n.id.clone(), why: format!("'{}' would sit inside the frame of group '{}', which it is not in", n.id, spec.groups[g].id) });
            }
        }
        for (h, other) in frames.iter().enumerate().skip(g + 1) {
            let apart = !p.under(Ref::Group(h), g) && !p.under(Ref::Group(g), h);
            if apart && other.is_some_and(|o| overlap(f, o)) {
                kept.reasons.push(Reason { id: spec.groups[g].id.clone(), why: format!("the frames of groups '{}' and '{}' would overlap", spec.groups[g].id, spec.groups[h].id) });
            }
        }
    }

    let mut geo = Geometry { nodes: p.own.iter().map(|r| r.unwrap_or(Rect::ZERO)).collect(), groups: frames.iter().map(|f| f.unwrap_or(Rect::ZERO)).collect(), notes: vec![None; spec.notes.len()] };
    for (r, own) in (0..spec.nodes.len()).map(|i| (Ref::Node(i), geo.nodes[i])).chain((0..spec.groups.len()).map(|g| (Ref::Group(g), geo.groups[g]))).collect::<Vec<_>>() {
        let f = layout::foot(spec, m, r, own.width(), own.height());
        for (i, rel) in f.notes {
            geo.notes[i] = Some(rel + own.origin().to_vec2());
        }
    }
    (geo, kept)
}

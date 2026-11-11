//! Sequence diagrams: nodes become lifelines side by side, edges become
//! messages in the order they are listed.

use kurbo::{Point, Rect};

use super::layout::{self, Measured, PAD, PAD_LABEL, Tuning, up};
use super::route::Dir;
use super::{Drawn, Label, Shape, land};
use crate::spec::{Ref, Spec};

const STEP: f64 = 40.0;
const LOOP: f64 = 32.0;

/// Lifelines keep the spec's order, except that the members of a group stay together.
fn order(spec: &Spec) -> Vec<usize> {
    fn take(spec: &Spec, r: Ref, out: &mut Vec<usize>) {
        match r {
            Ref::Node(i) => {
                if !out.contains(&i) {
                    out.push(i)
                }
            }
            Ref::Group(g) => {
                for k in layout::children(spec, Some(g)) {
                    take(spec, k, out);
                }
            }
            Ref::Edge(_) => {}
        }
    }
    let mut out = Vec::new();
    for i in 0..spec.nodes.len() {
        if out.contains(&i) {
            continue;
        }
        // The outermost group holding the node comes along as a block.
        match spec.ancestors(Ref::Node(i)).last() {
            Some(top) => take(spec, Ref::Group(*top), &mut out),
            None => out.push(i),
        }
    }
    out
}

pub fn arrange(spec: &Spec, m: &Measured, t: &Tuning, shapes_of: &dyn Fn(usize, Rect) -> Shape) -> Drawn {
    let cols = order(spec);
    let n = cols.len();
    let col_of = |node: usize| cols.iter().position(|c| *c == node).unwrap();
    let ends: Vec<(usize, usize)> = spec
        .edges
        .iter()
        .map(|e| {
            let node = |id: &str| match spec.lookup(id) {
                Some(Ref::Node(i)) => col_of(i),
                _ => 0,
            };
            (node(&e.from), node(&e.to))
        })
        .collect();
    let label_w = |e: usize| m.edge_label[e].map(|l| l.0).unwrap_or(0.0);
    let half = |k: usize| m.nodes[cols[k]].w / 2.0;

    // Distance between neighbouring lifelines: room for the headers, for the frames
    // of the groups that separate them, and for the labels of the messages between them.
    let mut gap: Vec<f64> = (0..n.saturating_sub(1))
        .map(|k| {
            let (a, b) = (spec.ancestors(Ref::Node(cols[k])), spec.ancestors(Ref::Node(cols[k + 1])));
            let apart = a.iter().filter(|g| !b.contains(g)).count() + b.iter().filter(|g| !a.contains(g)).count();
            half(k) + half(k + 1) + (t.label_pad + 16.0).max(PAD * apart as f64 + 16.0)
        })
        .collect();
    let mut frames: Vec<Rect> = vec![Rect::ZERO; spec.groups.len()];
    let mut centre = vec![0.0; n];
    let top = m.nodes.iter().map(|b| b.h).fold(0.0, f64::max);
    for _ in 0..3 {
        for (e, (a, b)) in ends.iter().enumerate() {
            let lo = *a.min(b);
            let need = if a == b { LOOP + 8.0 + label_w(e) + 16.0 } else { label_w(e) + 24.0 };
            if a == b {
                if lo + 1 < n && gap[lo] - half(lo + 1) < need {
                    gap[lo] = need + half(lo + 1);
                }
            } else {
                // The label sits in the gap next to the sender, so no lifeline runs through it.
                let near = if a < b { *a } else { a - 1 };
                if gap[near] < need {
                    gap[near] = need;
                }
            }
        }
        let mut x = 0.0;
        for k in 0..n {
            centre[k] = x;
            if k + 1 < n {
                gap[k] = up(gap[k], 8.0);
                x += gap[k];
            }
        }
        // Frames wrap the headers of their members; a long group label asks for more room.
        let mut by_depth: Vec<usize> = (0..spec.groups.len()).collect();
        by_depth.sort_by_key(|g| std::cmp::Reverse(spec.ancestors(Ref::Group(*g)).len()));
        let mut widened = false;
        for g in by_depth {
            let mut f: Option<Rect> = None;
            for k in layout::children(spec, Some(g)) {
                let r = match k {
                    Ref::Node(i) => {
                        let c = centre[col_of(i)];
                        Rect::new(c - m.nodes[i].w / 2.0, top - m.nodes[i].h, c + m.nodes[i].w / 2.0, top)
                    }
                    Ref::Group(i) => frames[i],
                    Ref::Edge(_) => continue,
                };
                f = Some(f.map_or(r, |f| f.union(r)));
            }
            let Some(f) = f else { continue };
            let head = if spec.groups[g].label.is_some() { PAD_LABEL } else { PAD };
            frames[g] = Rect::new(f.x0 - PAD, f.y0 - head, f.x1 + PAD, f.y1 + PAD);
            let short = up(m.group_label[g] + 32.0 - frames[g].width(), 16.0);
            if short > 0.0 {
                // Spread the lifelines on either side of the group to make the room.
                let inside: Vec<usize> = (0..n).filter(|k| spec.ancestors(Ref::Node(cols[*k])).contains(&g)).collect();
                if let (Some(first), Some(last)) = (inside.first(), inside.last()) {
                    if *first > 0 {
                        gap[first - 1] += short / 2.0;
                    }
                    if last + 1 < n {
                        gap[*last] += short / 2.0;
                    }
                    frames[g] = Rect::new(frames[g].x0 - short / 2.0, frames[g].y0, frames[g].x1 + short / 2.0, frames[g].y1);
                    widened = true;
                }
            }
        }
        if !widened {
            break;
        }
    }

    let mut geo = layout::Geometry { nodes: vec![Rect::ZERO; spec.nodes.len()], groups: frames, notes: vec![None; spec.notes.len()] };
    for k in 0..n {
        let i = cols[k];
        geo.nodes[i] = Rect::new(centre[k] - m.nodes[i].w / 2.0, top - m.nodes[i].h, centre[k] + m.nodes[i].w / 2.0, top);
    }

    // Messages, top to bottom.
    let mut routes = Vec::new();
    let mut labels = Vec::new();
    let mut y = top + STEP;
    for (e, (a, b)) in ends.iter().enumerate() {
        let size = m.edge_label[e];
        let text = spec.edges[e].label.clone();
        if a == b {
            let x = centre[*a];
            routes.push(vec![Point::new(x, y), Point::new(x + LOOP, y), Point::new(x + LOOP, y + 16.0), Point::new(x, y + 16.0)]);
            labels.push(size.map(|(w, h)| Label { rect: Rect::new(x + LOOP + 8.0, y + 8.0 - h / 2.0, x + LOOP + 8.0 + w, y + 8.0 + h / 2.0), text: text.unwrap_or_default() }));
            y += STEP + 16.0;
        } else {
            routes.push(vec![Point::new(centre[*a], y), Point::new(centre[*b], y)]);
            let next = if a < b { a + 1 } else { a - 1 };
            let mid = (centre[*a] + centre[next]) / 2.0;
            labels.push(size.map(|(w, h)| Label { rect: Rect::new(mid - w / 2.0, y - 5.0 - h, mid + w / 2.0, y - 5.0), text: text.unwrap_or_default() }));
            y += STEP;
        }
    }
    let bottom = y - STEP + 32.0;
    let lifelines: Vec<Option<(Point, Point)>> = (0..spec.nodes.len())
        .map(|i| {
            let x = centre[col_of(i)];
            let start = land(&shapes_of(i, geo.nodes[i]).outline, Point::new(x, top), Dir::S);
            Some((start, Point::new(x, bottom)))
        })
        .collect();

    // Notes: on a message, in a column to the right at the message's height; on a
    // lifeline, in rows under it; on a group, above its frame.
    let mut right = geo.nodes.iter().chain(&geo.groups).map(|r| r.x1).fold(f64::NEG_INFINITY, f64::max);
    for l in labels.iter().flatten() {
        right = right.max(l.rect.x1);
    }
    for r in &routes {
        for p in r {
            right = right.max(p.x);
        }
    }
    let side_x = up(right + 32.0, 8.0);
    let mut leaders: Vec<Option<(Point, Point)>> = vec![None; spec.notes.len()];
    let (mut side_floor, mut under, mut above): (f64, Vec<Rect>, Vec<Rect>) = (f64::NEG_INFINITY, Vec::new(), Vec::new());
    for (i, note) in spec.notes.iter().enumerate() {
        let (w, h) = (m.notes[i].w, m.notes[i].h);
        match spec.lookup(&note.attach) {
            Some(Ref::Edge(e)) => {
                let at = routes[e].iter().map(|p| p.y).sum::<f64>() / routes[e].len() as f64;
                let y0 = layout::snap(at - h / 2.0).max(side_floor);
                let r = Rect::new(side_x, y0, side_x + w, y0 + h);
                side_floor = r.y1 + 8.0;
                // The leader stops short of the message's own label when that sits on its path.
                let beside = labels[e].as_ref().filter(|l| l.rect.y0 <= at && at <= l.rect.y1).map(|l| l.rect.x1 + 4.0);
                let reach = routes[e].iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max).max(beside.unwrap_or(f64::NEG_INFINITY));
                leaders[i] = Some((Point::new(r.x0, at.clamp(r.y0 + 4.0, r.y1 - 4.0)), Point::new(reach, at)));
                geo.notes[i] = Some(r);
            }
            Some(Ref::Node(node)) => {
                let x = centre[col_of(node)];
                let mut r = Rect::new(x - w / 2.0, bottom + 24.0, x + w / 2.0, bottom + 24.0 + h);
                while under.iter().any(|o| o.inflate(8.0, 4.0).intersect(r).area() > 0.0) {
                    r = r + kurbo::Vec2::new(0.0, 8.0);
                }
                leaders[i] = Some((Point::new(x, r.y0), Point::new(x, bottom)));
                under.push(r);
                geo.notes[i] = Some(r);
            }
            Some(Ref::Group(g)) => {
                let f = geo.groups[g];
                let x = layout::snap(f.center().x);
                let mut r = Rect::new(x - w / 2.0, f.y0 - 24.0 - h, x + w / 2.0, f.y0 - 24.0);
                while above.iter().any(|o| o.inflate(8.0, 4.0).intersect(r).area() > 0.0) {
                    r = r + kurbo::Vec2::new(0.0, -8.0);
                }
                leaders[i] = Some((Point::new(x, r.y1), Point::new(x, f.y0)));
                above.push(r);
                geo.notes[i] = Some(r);
            }
            None => {}
        }
    }
    super::fit(geo, routes, labels, leaders, lifelines)
}

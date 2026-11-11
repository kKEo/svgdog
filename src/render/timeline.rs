//! Timelines: nodes in spec order along an axis, each with its `when` at the
//! axis and its box beside it. Groups are phases, framed around their members.

use kurbo::{Point, Rect, Vec2};

use super::layout::{self, GAP, Geometry, Measured, PAD, PAD_LABEL, up};
use super::theme::SMALL;
use super::{Drawn, Mark};
use crate::spec::{Direction, Spec};

/// A phase with its members, or a node on its own.
struct Unit {
    group: Option<usize>,
    nodes: Vec<usize>,
}

fn units(spec: &Spec) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::new();
    for i in 0..spec.nodes.len() {
        match spec.node_parent[i] {
            Some(g) => match out.iter_mut().find(|u| u.group == Some(g)) {
                Some(u) => u.nodes.push(i),
                None => out.push(Unit { group: Some(g), nodes: vec![i] }),
            },
            None => out.push(Unit { group: None, nodes: vec![i] }),
        }
    }
    out
}

const DOT_TO_BOX: f64 = 24.0;
const LINE: f64 = 16.0;

pub fn arrange(spec: &Spec, m: &Measured) -> Drawn {
    let units = units(spec);
    let when_w: Vec<f64> = spec.nodes.iter().map(|n| n.when.as_deref().map(|w| layout::text_width(w, SMALL) * 1.08).unwrap_or(0.0)).collect();
    let pad = if spec.groups.is_empty() { 0.0 } else { PAD };
    let mut geo = Geometry { nodes: vec![Rect::ZERO; spec.nodes.len()], groups: vec![Rect::ZERO; spec.groups.len()], notes: Vec::new() };
    let mut marks: Vec<Vec<Mark>> = vec![Vec::new(); spec.nodes.len()];
    let mut axis: Vec<(Point, Point)> = Vec::new();

    if spec.direction == Direction::Right {
        // Units run left to right and wrap to a new row, whole phases at a time.
        let slot = |i: usize| up(m.nodes[i].w.max(when_w[i]), 16.0);
        let width = |u: &Unit| {
            let inner: f64 = u.nodes.iter().map(|i| slot(*i)).sum::<f64>() + GAP * (u.nodes.len() as f64 - 1.0);
            match u.group {
                Some(g) => (inner + 2.0 * PAD).max(up(m.group_label[g] + 32.0, 16.0)),
                None => inner,
            }
        };
        let limit = spec.max_width - 48.0;
        let mut rows: Vec<Vec<&Unit>> = vec![Vec::new()];
        let mut x = 0.0;
        for u in &units {
            let w = width(u);
            if x > 0.0 && x + w > limit {
                rows.push(Vec::new());
                x = 0.0;
            }
            rows.last_mut().unwrap().push(u);
            x += w + GAP;
        }
        let mut y = 0.0;
        for row in rows {
            // A row leaves headroom only for the frames it holds.
            let labelled = row.iter().any(|u| u.group.is_some_and(|g| spec.groups[g].label.is_some()));
            let (head, pad) = if labelled { (PAD_LABEL, PAD) } else if row.iter().any(|u| u.group.is_some()) { (PAD, PAD) } else { (0.0, 0.0) };
            let tall = row.iter().flat_map(|u| &u.nodes).map(|i| m.nodes[*i].h).fold(0.0, f64::max);
            let (when_y, axis_y) = (y + head + LINE / 2.0, y + head + LINE + 12.0);
            let box_y = axis_y + DOT_TO_BOX;
            let bottom = box_y + tall + pad;
            let mut x = 0.0;
            let (mut first, mut last) = (f64::INFINITY, f64::NEG_INFINITY);
            for u in row {
                let w = width(u);
                let inner: f64 = u.nodes.iter().map(|i| slot(*i)).sum::<f64>() + GAP * (u.nodes.len() as f64 - 1.0);
                let mut sx = x + (w - inner) / 2.0;
                for &i in &u.nodes {
                    let c = layout::snap(sx + slot(i) / 2.0);
                    geo.nodes[i] = Rect::new(c - m.nodes[i].w / 2.0, box_y, c + m.nodes[i].w / 2.0, box_y + m.nodes[i].h);
                    marks[i].push(Mark::Stem(Point::new(c, axis_y), Point::new(c, box_y)));
                    marks[i].push(Mark::Dot(Point::new(c, axis_y)));
                    if let Some(w) = &spec.nodes[i].when {
                        marks[i].push(Mark::When { at: Point::new(c, when_y), text: w.clone(), anchor: "middle", width: when_w[i] });
                    }
                    first = first.min(c);
                    last = last.max(c);
                    sx += slot(i) + GAP;
                }
                if let Some(g) = u.group {
                    geo.groups[g] = Rect::new(x, y, x + w, bottom);
                }
                x += w + GAP;
            }
            axis.push((Point::new(first - 24.0, axis_y), Point::new(last + 24.0, axis_y)));
            y = bottom + 40.0;
        }
    } else {
        // One column: the axis runs down, dates to its left, boxes to its right.
        let dates = up(when_w.iter().copied().fold(0.0, f64::max), 8.0);
        let axis_x = pad + dates + if dates > 0.0 { 16.0 } else { 8.0 };
        let box_x = axis_x + DOT_TO_BOX;
        let wide = spec.nodes.iter().enumerate().map(|(i, _)| m.nodes[i].w).fold(0.0, f64::max);
        let right = (box_x + wide + pad).max(spec.groups.iter().enumerate().map(|(g, _)| up(m.group_label[g] + 32.0, 16.0)).fold(0.0, f64::max));
        let mut y = 0.0;
        let (mut first, mut last) = (f64::INFINITY, f64::NEG_INFINITY);
        for u in &units {
            let top = y;
            if let Some(g) = u.group {
                y += if spec.groups[g].label.is_some() { PAD_LABEL } else { PAD };
            }
            for (k, &i) in u.nodes.iter().enumerate() {
                if k > 0 {
                    y += 24.0;
                }
                geo.nodes[i] = Rect::new(box_x, y, box_x + m.nodes[i].w, y + m.nodes[i].h);
                let c = layout::snap(y + m.nodes[i].h / 2.0);
                marks[i].push(Mark::Stem(Point::new(axis_x, c), Point::new(box_x, c)));
                marks[i].push(Mark::Dot(Point::new(axis_x, c)));
                if let Some(w) = &spec.nodes[i].when {
                    marks[i].push(Mark::When { at: Point::new(axis_x - 12.0, c), text: w.clone(), anchor: "end", width: when_w[i] });
                }
                first = first.min(c);
                last = last.max(c);
                y += m.nodes[i].h;
            }
            if let Some(g) = u.group {
                y += PAD;
                geo.groups[g] = Rect::new(0.0, top, right, y);
            }
            y += 24.0;
        }
        axis.push((Point::new(axis_x, first - 24.0), Point::new(axis_x, last + 24.0)));
    }

    // Fit the canvas around everything, marks included.
    let mut b = geo.nodes.iter().chain(&geo.groups).copied().reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
    for (a, z) in &axis {
        b = b.union(Rect::from_points(*a, *z));
    }
    for mark in marks.iter().flatten() {
        if let Mark::When { at, anchor, width, .. } = mark {
            let x0 = if *anchor == "end" { at.x - width } else { at.x - width / 2.0 };
            b = b.union(Rect::new(x0, at.y - LINE / 2.0, x0 + width, at.y + LINE / 2.0));
        }
    }
    let (left, top) = ((b.x0 / 8.0).floor() * 8.0, (b.y0 / 8.0).floor() * 8.0);
    let (cw, ch) = (up(b.x1 - left, 8.0), up(b.y1 - top, 8.0));
    // The same rule as every other layout: small drawings get a narrow margin.
    let margin = |c: f64, slack: f64| [24.0, 16.0, 8.0].into_iter().find(|m| (m + slack) / (c + 2.0 * m) <= 0.13).unwrap_or(8.0);
    let (mx, my) = (margin(cw, (b.x0 - left).max(cw - (b.x1 - left))), margin(ch, (b.y0 - top).max(ch - (b.y1 - top))));
    let mut d = Drawn { geo, routes: Vec::new(), labels: Vec::new(), leaders: Vec::new(), lifelines: vec![None; spec.nodes.len()], marks, axis, size: (cw + 2.0 * mx, ch + 2.0 * my) };
    d.shift(Vec2::new(mx - left, my - top));
    d
}

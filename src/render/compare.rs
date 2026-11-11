//! Comparisons: two groups side by side. Nodes with the same key share a row, so
//! what stayed lines up and what was added or removed stands alone.

use kurbo::Rect;

use super::layout::{GAP, Geometry, Measured, PAD, PAD_LABEL, snap, up};
use crate::spec::Spec;

/// Rows of the comparison: the node on each side, where there is one.
pub fn rows(spec: &Spec) -> Vec<(Option<usize>, Option<usize>)> {
    let sides = spec.sides();
    let (left, right) = (&sides[0].1, &sides[1].1);
    let mut out = Vec::new();
    let mut next = 0;
    for &a in left {
        // Look ahead only: both sides keep the order they were written in.
        match right[next..].iter().position(|b| spec.match_key(*b) == spec.match_key(a)) {
            Some(skip) => {
                out.extend(right[next..next + skip].iter().map(|b| (None, Some(*b))));
                out.push((Some(a), Some(right[next + skip])));
                next += skip + 1;
            }
            None => out.push((Some(a), None)),
        }
    }
    out.extend(right[next..].iter().map(|b| (None, Some(*b))));
    out
}

pub fn layout(spec: &Spec, m: &Measured) -> Geometry {
    let sides = spec.sides();
    let rows = rows(spec);
    let mut geo = Geometry { nodes: vec![Rect::ZERO; spec.nodes.len()], groups: vec![Rect::ZERO; spec.groups.len()], notes: vec![None; spec.notes.len()] };
    let head = if sides.iter().any(|(g, _)| spec.groups[*g].label.is_some()) { PAD_LABEL } else { PAD };
    // Arrows between rows need room for themselves and for a label.
    let gap = if spec.edges.iter().any(|e| e.label.is_some()) { 48.0 } else { GAP };
    let col_w: Vec<f64> = sides.iter().map(|(g, ns)| up(ns.iter().map(|i| m.nodes[*i].w).fold(0.0, f64::max).max(m.group_label[*g] + 32.0 - 2.0 * PAD), 16.0)).collect();
    let frame_x = [0.0, col_w[0] + 2.0 * PAD + 64.0];
    let mut y = head;
    for (a, b) in &rows {
        let h = [a, b].iter().filter_map(|n| n.map(|i| m.nodes[i].h)).fold(0.0, f64::max);
        for (side, node) in [a, b].into_iter().enumerate() {
            if let Some(i) = node {
                let cx = snap(frame_x[side] + PAD + col_w[side] / 2.0);
                let cy = y + h / 2.0;
                geo.nodes[*i] = Rect::new(cx - m.nodes[*i].w / 2.0, cy - m.nodes[*i].h / 2.0, cx + m.nodes[*i].w / 2.0, cy + m.nodes[*i].h / 2.0);
            }
        }
        y += h + gap;
    }
    let bottom = y - gap + PAD;
    for (side, (g, _)) in sides.iter().enumerate() {
        geo.groups[*g] = Rect::new(frame_x[side], 0.0, frame_x[side] + col_w[side] + 2.0 * PAD, bottom);
    }
    geo
}

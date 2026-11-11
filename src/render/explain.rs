//! The explanation layer: step badges, the dimming of what is out of focus, and
//! the footer under the drawing that holds the legend, the steps and the caption.

use kurbo::{Point, Rect, Vec2};

use super::layout::{self, up};
use super::theme::{SMALL, STROKE, Theme};
use super::{Drawn, Writer, shape};
use crate::spec::{EdgeStyle, Kind, Ref, Sample, Spec};
use crate::util::fmt;

/// Diameter of a step badge.
pub const BADGE: f64 = 18.0;
/// Badges side by side sit this far apart, centre to centre.
pub const PITCH: f64 = BADGE + 2.0;
const SIDE: f64 = 24.0;
const LINE: f64 = 16.0;

/// Room an edge label needs for its badges, and the gap before any text.
pub fn badge_room(count: usize, has_text: bool) -> f64 {
    match count {
        0 => 0.0,
        n => n as f64 * PITCH - 2.0 + if has_text { 6.0 } else { 0.0 },
    }
}

/// Where the badges of a node go: on its top-left corner, running right along the top.
/// Shapes without a corner there get a point on their outline instead.
pub fn node_badges(kind: Kind, r: Rect, count: usize) -> Vec<Point> {
    let first = match kind {
        Kind::Decision => Point::new(r.x0 + r.width() / 4.0, r.y0 + r.height() / 4.0),
        Kind::Start | Kind::End => Point::new(r.x0 - 4.0, r.y0 - 4.0),
        Kind::Actor | Kind::Queue => {
            let rad = r.height() / 2.0;
            Point::new(r.x0 + rad * 0.3, r.y0 + rad * 0.3)
        }
        _ => Point::new(r.x0, r.y0),
    };
    (0..count).map(|k| first + Vec2::new(k as f64 * PITCH, 0.0)).collect()
}

/// A frame carries its badges on the top-right corner, clear of its label.
pub fn group_badges(r: Rect, count: usize) -> Vec<Point> {
    (0..count).map(|k| Point::new(r.x1 - k as f64 * PITCH, r.y0)).collect()
}

/// Make the canvas large enough for badges that stick out past the boxes.
pub fn grow_for_badges(spec: &Spec, d: &mut Drawn) {
    let mut all: Option<Rect> = None;
    let mut add = |pts: Vec<Point>| {
        for p in pts {
            let r = Rect::from_center_size(p, (BADGE, BADGE));
            all = Some(all.map_or(r, |a| a.union(r)));
        }
    };
    for (i, n) in spec.nodes.iter().enumerate() {
        add(node_badges(n.kind, d.geo.nodes[i], spec.steps_of(Ref::Node(i)).len()));
    }
    for g in 0..spec.groups.len() {
        add(group_badges(d.geo.groups[g], spec.steps_of(Ref::Group(g)).len()));
    }
    let Some(b) = all else { return };
    const KEEP: f64 = 8.0;
    let left = up((KEEP - b.x0).max(0.0), 8.0);
    let top = up((KEEP - b.y0).max(0.0), 8.0);
    let right = up((b.x1 + KEEP - d.size.0).max(0.0), 8.0);
    let bottom = up((b.y1 + KEEP - d.size.1).max(0.0), 8.0);
    d.shift(Vec2::new(left, top));
    d.size = (d.size.0 + left + right, d.size.1 + top + bottom);
}

/// The block under the drawing, laid out for a given width.
pub struct Footer {
    /// Legend rows: each entry with its x offset from the left edge.
    legend: Vec<Vec<(f64, Sample, String)>>,
    steps: Vec<(usize, Vec<String>)>,
    caption: Vec<String>,
    /// In a single frame, the step that is current; the others are listed quietly.
    shown: Option<usize>,
    /// Width of the widest thing in it.
    pub width: f64,
    pub height: f64,
}

const SAMPLE: f64 = 28.0;

fn kind_row(legend: &[Vec<(f64, Sample, String)>]) -> f64 {
    if legend.iter().flatten().any(|e| matches!(e.1, Sample::Kind(_))) { 28.0 } else { 20.0 }
}

impl Footer {
    pub fn is_empty(&self) -> bool {
        self.legend.is_empty() && self.steps.is_empty() && self.caption.is_empty()
    }

    /// Lay the footer out for a drawing `drawing` wide; text wraps at that width,
    /// or at a comfortable reading width when the drawing is narrower than that.
    pub fn plan(spec: &Spec, drawing: f64) -> Footer {
        let cap = (drawing.max(480.0f64.min(spec.max_width)) - 2.0 * SIDE).max(120.0);
        let mut width: f64 = 0.0;
        let mut legend: Vec<Vec<(f64, Sample, String)>> = Vec::new();
        let mut x = 0.0;
        for (sample, text) in &spec.legend {
            let w = SAMPLE + 8.0 + layout::text_width(text, SMALL) * 1.05;
            if legend.is_empty() || (x > 0.0 && x + w > cap) {
                legend.push(Vec::new());
                x = 0.0;
            }
            legend.last_mut().unwrap().push((x, *sample, text.clone()));
            width = width.max(x + w);
            x += up(w + 24.0, 8.0);
        }
        let steps: Vec<(usize, Vec<String>)> = spec.steps.iter().map(|s| (s.number, layout::wrap(&s.text, SMALL, cap - 28.0))).collect();
        for (_, lines) in &steps {
            width = lines.iter().map(|l| 28.0 + layout::text_width(l, SMALL)).fold(width, f64::max);
        }
        let caption = spec.caption.as_deref().map(|c| layout::wrap(c, SMALL, cap)).unwrap_or_default();
        width = caption.iter().map(|l| layout::text_width(l, SMALL)).fold(width, f64::max);

        let mut f = Footer { legend, steps, caption, shown: spec.shown, width, height: 0.0 };
        f.height = if f.is_empty() { 0.0 } else { up(f.walk(0.0, 0.0, None, None), 8.0) };
        f
    }

    /// Walk the layout from `top`, drawing when given a writer; returns the height used.
    fn walk(&self, left: f64, top: f64, mut wr: Option<&mut Writer>, theme: Option<&Theme>) -> f64 {
        let mut y = top + 8.0;
        let section = |y: &mut f64, first: &mut bool| {
            if !*first {
                *y += 12.0;
            }
            *first = false;
        };
        let mut first = true;
        if !self.legend.is_empty() {
            section(&mut y, &mut first);
            let row = kind_row(&self.legend);
            if let (Some(wr), Some(t)) = (wr.as_deref_mut(), theme) {
                wr.out.push_str("  <g id=\"legend\">\n");
                for (r, entries) in self.legend.iter().enumerate() {
                    let cy = y + r as f64 * row + row / 2.0;
                    for (dx, sample, text) in entries {
                        let x = left + dx;
                        match sample {
                            Sample::Style(style) => {
                                let dash = match style {
                                    EdgeStyle::Sync => "",
                                    EdgeStyle::Async => " stroke-dasharray=\"6 4\"",
                                    EdgeStyle::Dependency => " stroke-dasharray=\"2 4\"",
                                };
                                wr.out.push_str(&format!("    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"{dash}/>\n", fmt(x), fmt(cy), fmt(x + SAMPLE), fmt(cy), t.edge, fmt(STROKE)));
                            }
                            Sample::Emphasis => {
                                wr.out.push_str(&format!("    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"12\" rx=\"2\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n", fmt(x + 2.0), fmt(cy - 6.0), fmt(SAMPLE - 4.0), t.accent_fill, t.node_stroke, fmt(STROKE)));
                            }
                            Sample::Kind(kind) => {
                                let r = Rect::new(x, cy - 10.0, x + SAMPLE, cy + 10.0);
                                // The small shapes keep their outline only; inner detail lines do not scale down.
                                if let Some(p) = shape(*kind, r).parts.first() {
                                    wr.part(p, t.node_fill, t.node_stroke);
                                }
                            }
                        }
                        wr.text("    ", std::slice::from_ref(text), x + SAMPLE + 8.0, cy, SMALL, LINE, t.text, "start");
                    }
                }
                wr.out.push_str("  </g>\n");
            }
            y += self.legend.len() as f64 * row;
        }
        if !self.steps.is_empty() {
            section(&mut y, &mut first);
            if let Some(wr) = wr.as_deref_mut() {
                wr.out.push_str("  <g id=\"steps\">\n");
            }
            for (k, (number, lines)) in self.steps.iter().enumerate() {
                let h = (lines.len() as f64 * LINE).max(BADGE);
                if let (Some(wr), Some(t)) = (wr.as_deref_mut(), theme) {
                    wr.out.push_str(&format!("    <g id=\"step-{number}\">\n"));
                    let quiet = self.shown.is_some_and(|s| s != *number);
                    wr.badge("      ", Point::new(left + BADGE / 2.0, y + BADGE / 2.0), *number, t, quiet);
                    wr.text("      ", lines, left + 28.0, y + h / 2.0, SMALL, LINE, if quiet { t.muted } else { t.text }, "start");
                    wr.out.push_str("    </g>\n");
                }
                y += h + if k + 1 < self.steps.len() { 8.0 } else { 0.0 };
            }
            if let Some(wr) = wr.as_deref_mut() {
                wr.out.push_str("  </g>\n");
            }
        }
        if !self.caption.is_empty() {
            section(&mut y, &mut first);
            let h = self.caption.len() as f64 * LINE;
            if let (Some(wr), Some(t)) = (wr, theme) {
                wr.out.push_str("  <g id=\"caption\">\n");
                wr.text("    ", &self.caption, left, y + h / 2.0, SMALL, LINE, t.muted, "start");
                wr.out.push_str("  </g>\n");
            }
            y += h;
        }
        y + 16.0 - top
    }

    pub fn write(&self, wr: &mut Writer, theme: &Theme, top: f64) {
        if !self.is_empty() {
            self.walk(SIDE, top, Some(wr), Some(theme));
        }
    }

    /// The canvas width the footer needs.
    pub fn canvas(&self) -> f64 {
        if self.is_empty() { 0.0 } else { up(self.width + 2.0 * SIDE, 8.0) }
    }
}

//! The create path: a spec in, deterministic SVG out.

mod compare;
mod explain;
pub mod interactive;
pub mod layout;
pub mod route;
mod sequence;
mod timeline;
pub mod theme;

use std::collections::BTreeMap;

use kurbo::{BezPath, Point, Rect, Shape as _, Vec2};

use crate::envelope::AppError;
use crate::spec::{Arrow, Diagram, Direction, EdgeStyle, Kind, Ref, Side, Spec};
use crate::svg::text::{self, Class};
use crate::svg::{self, SVG_NS};
use crate::util::{did_you_mean, fmt};
use layout::{Geometry, Measured, Tuning};
use route::{Dir, Grid};
use theme::{FONT, SMALL, STROKE, THIN};

pub struct Options<'a> {
    /// Theme from the command line; wins over the spec's.
    pub theme: Option<&'a str>,
    /// `[style] fonts` from the config; empty uses the bundled family.
    pub fonts: &'a [String],
    pub text_as_paths: bool,
    /// The spec document to store in the drawing, when asked for.
    pub embed: Option<&'a serde_json::Value>,
    /// Add the classes and the style block of the `css` interactive version.
    pub interactive: bool,
}

const STUB: f64 = 16.0;

pub(crate) enum Part {
    Rect { r: Rect, rx: f64, dashed: bool },
    Path { d: String, filled: bool },
    Polygon(Vec<Point>),
    Line(Point, Point),
    /// A circle; `ink` fills it with the stroke colour, as a solid dot.
    Disc { c: Point, r: f64, ink: bool },
}

/// Something drawn with a node that is not its shape: its place on a timeline's axis.
#[derive(Clone)]
pub(crate) enum Mark {
    Stem(Point, Point),
    Dot(Point),
    When { at: Point, text: String, anchor: &'static str, width: f64 },
}

pub(crate) struct Shape {
    /// Flattened silhouette, for landing connectors exactly on the edge.
    pub outline: Vec<Vec<Point>>,
    parts: Vec<Part>,
}

fn p2(x: f64, y: f64) -> String {
    format!("{} {}", fmt(x), fmt(y))
}

fn shape(kind: Kind, r: Rect) -> Shape {
    let (x0, y0, x1, y1) = (r.x0, r.y0, r.x1, r.y1);
    let (w, h) = (r.width(), r.height());
    let rounded = |rx: f64, dashed: bool, extra: Vec<Part>| {
        let outline = svg::flatten(&kurbo::RoundedRect::from_rect(r, rx).to_path(0.05));
        let mut parts = vec![Part::Rect { r, rx, dashed }];
        parts.extend(extra);
        Shape { outline, parts }
    };
    let from_d = |d: String, extra: Vec<Part>| {
        let outline = svg::flatten(&BezPath::from_svg(&d).expect("shape path data is valid"));
        let mut parts = vec![Part::Path { d, filled: true }];
        parts.extend(extra);
        Shape { outline, parts }
    };
    match kind {
        Kind::Box => rounded(0.0, false, vec![]),
        Kind::State => rounded(12.0, false, vec![]),
        Kind::Start | Kind::End => {
            let (c, r) = (r.center(), w.min(h) / 2.0);
            let outline = svg::flatten(&kurbo::Circle::new(c, r).to_path(0.05));
            let parts = if kind == Kind::Start { vec![Part::Disc { c, r, ink: true }] } else { vec![Part::Disc { c, r: r - STROKE / 2.0, ink: false }, Part::Disc { c, r: r - 5.0, ink: true }] };
            Shape { outline, parts }
        }
        Kind::Service => rounded(8.0, false, vec![]),
        Kind::External => rounded(4.0, true, vec![]),
        Kind::Actor => rounded(h / 2.0, false, vec![]),
        Kind::Client => rounded(8.0, false, vec![Part::Line(Point::new(x0, y0 + 16.0), Point::new(x1, y0 + 16.0))]),
        Kind::Decision => {
            let pts = vec![Point::new(x0 + w / 2.0, y0), Point::new(x1, y0 + h / 2.0), Point::new(x0 + w / 2.0, y1), Point::new(x0, y0 + h / 2.0)];
            let mut closed = pts.clone();
            closed.push(pts[0]);
            Shape { outline: vec![closed], parts: vec![Part::Polygon(pts)] }
        }
        Kind::Datastore => {
            let (rx, ry) = (w / 2.0, 8.0);
            let arc = format!("A{} 0 0 1", p2(rx, ry));
            let d = format!("M{} {arc} {} L{} {arc} {} Z", p2(x0, y0 + ry), p2(x1, y0 + ry), p2(x1, y1 - ry), p2(x0, y1 - ry));
            let lid = format!("M{} A{} 0 0 0 {}", p2(x0, y0 + ry), p2(rx, ry), p2(x1, y0 + ry));
            from_d(d, vec![Part::Path { d: lid, filled: false }])
        }
        Kind::Queue => {
            let (rx, ry) = (8.0, h / 2.0);
            let arc = format!("A{} 0 0 1", p2(rx, ry));
            let d = format!("M{} L{} {arc} {} L{} {arc} {} Z", p2(x0 + rx, y0), p2(x1 - rx, y0), p2(x1 - rx, y1), p2(x0 + rx, y1), p2(x0 + rx, y0));
            let lid = format!("M{} A{} 0 0 0 {}", p2(x1 - rx, y0), p2(rx, ry), p2(x1 - rx, y1));
            from_d(d, vec![Part::Path { d: lid, filled: false }])
        }
        Kind::Document => {
            let b = y1 - 8.0;
            let d = format!("M{} L{} L{} Q{} {} Q{} {} Z", p2(x0, y0), p2(x1, y0), p2(x1, b), p2(x0 + w * 0.75, b - 12.0), p2(x0 + w / 2.0, b), p2(x0 + w * 0.25, b + 12.0), p2(x0, b));
            from_d(d, vec![])
        }
    }
}

/// Slide a point on a box side inwards until it meets the outline.
pub(crate) fn land(outline: &[Vec<Point>], p: Point, outward: Dir) -> Point {
    let inward = outward.opposite().vec();
    let mut best: Option<f64> = None;
    for poly in outline {
        for w in poly.windows(2) {
            let (a, b) = (w[0], w[1]);
            let t = if inward.x != 0.0 {
                // Horizontal ray: the segment must straddle p.y.
                if (a.y - p.y) * (b.y - p.y) > 0.0 || (a.y - b.y).abs() < 1e-12 {
                    continue;
                }
                let x = a.x + (b.x - a.x) * (p.y - a.y) / (b.y - a.y);
                (x - p.x) * inward.x
            } else {
                if (a.x - p.x) * (b.x - p.x) > 0.0 || (a.x - b.x).abs() < 1e-12 {
                    continue;
                }
                let y = a.y + (b.y - a.y) * (p.x - a.x) / (b.x - a.x);
                (y - p.y) * inward.y
            };
            if t >= -1e-6 && best.is_none_or(|b| t < b) {
                best = Some(t.max(0.0));
            }
        }
    }
    match best {
        Some(t) => p + inward * t,
        None => p,
    }
}

/// Which sides two boxes use for an edge between them. Boxes apart along one axis
/// only use that axis. Boxes apart along both follow the diagram: a flow keeps to its
/// direction, bands connect across it, and pinned boxes take the wider gap.
fn sides(a: Rect, b: Rect, dir: Direction, diagram: Diagram) -> (Dir, Dir) {
    let (fwd, back, across, back_across) = if dir == Direction::Right { (Dir::E, Dir::W, Dir::S, Dir::N) } else { (Dir::S, Dir::N, Dir::E, Dir::W) };
    let main = |r: Rect| if dir == Direction::Right { (r.x0, r.x1) } else { (r.y0, r.y1) };
    let cross = |r: Rect| if dir == Direction::Right { (r.y0, r.y1) } else { (r.x0, r.x1) };
    let gap = |p: (f64, f64), q: (f64, f64)| (q.0 - p.1).max(p.0 - q.1);
    let (mg, cg) = (gap(main(a), main(b)), gap(cross(a), cross(b)));
    let along_main = if main(b).0 >= main(a).1 { (fwd, back) } else { (back, fwd) };
    let along_cross = if cross(b).0 >= cross(a).1 { (across, back_across) } else { (back_across, across) };
    match (mg >= 0.0, cg >= 0.0) {
        (true, false) => along_main,
        (false, true) => along_cross,
        (true, true) => match diagram {
            Diagram::Flow | Diagram::State => along_main,
            Diagram::Layers => along_cross,
            Diagram::Free | Diagram::Sequence | Diagram::Timeline | Diagram::Compare => if mg >= cg { along_main } else { along_cross },
        },
        (false, false) => (fwd, back),
    }
}

fn side_point(r: Rect, side: Dir, offset: f64) -> Point {
    match side {
        Dir::E => Point::new(r.x1, r.y0 + offset),
        Dir::W => Point::new(r.x0, r.y0 + offset),
        Dir::S => Point::new(r.x0 + offset, r.y1),
        Dir::N => Point::new(r.x0 + offset, r.y0),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn esc_attr(s: &str) -> String {
    esc(s).replace('"', "&quot;")
}

pub(crate) struct Label {
    pub rect: Rect,
    pub text: String,
}

/// Everything placed and ready to write.
pub(crate) struct Drawn {
    geo: Geometry,
    routes: Vec<Vec<Point>>,
    labels: Vec<Option<Label>>,
    leaders: Vec<Option<(Point, Point)>>,
    /// Sequence diagrams only: the line under each node's header.
    lifelines: Vec<Option<(Point, Point)>>,
    /// Timelines only: what each node carries on the axis, and the axis itself.
    marks: Vec<Vec<Mark>>,
    axis: Vec<(Point, Point)>,
    size: (f64, f64),
}

impl Drawn {
    /// Move everything drawn by `v`.
    fn shift(&mut self, v: Vec2) {
        for r in self.geo.nodes.iter_mut().chain(self.geo.groups.iter_mut()).chain(self.geo.notes.iter_mut().flatten()) {
            *r = *r + v;
        }
        for p in self.routes.iter_mut().flatten() {
            *p += v;
        }
        for l in self.labels.iter_mut().flatten() {
            l.rect = l.rect + v;
        }
        for l in self.leaders.iter_mut().chain(self.lifelines.iter_mut()).flatten() {
            *l = (l.0 + v, l.1 + v);
        }
        for l in self.axis.iter_mut() {
            *l = (l.0 + v, l.1 + v);
        }
        for mark in self.marks.iter_mut().flatten() {
            match mark {
                Mark::Stem(a, b) => (*a, *b) = (*a + v, *b + v),
                Mark::Dot(c) => *c += v,
                Mark::When { at, .. } => *at += v,
            }
        }
    }
}

fn box_of(geo: &Geometry, r: Ref) -> Rect {
    match r {
        Ref::Node(i) => geo.nodes[i],
        Ref::Group(g) => geo.groups[g],
        Ref::Edge(_) => Rect::ZERO,
    }
}

fn group_label_rect(frame: Rect, width: f64) -> Rect {
    Rect::new(frame.x0 + 8.0, frame.y0 + 8.0, frame.x0 + 16.0 + width, frame.y0 + 32.0)
}

fn segments(route: &[Point]) -> impl Iterator<Item = (Point, Point)> + '_ {
    route.windows(2).map(|w| (w[0], w[1]))
}

fn midpoint(route: &[Point]) -> Point {
    let total: f64 = segments(route).map(|(a, b)| a.distance(b)).sum();
    let mut left = total / 2.0;
    for (a, b) in segments(route) {
        let len = a.distance(b);
        if left <= len && len > 0.0 {
            return a.lerp(b, left / len);
        }
        left -= len;
    }
    route.first().copied().unwrap_or(Point::ZERO)
}

fn arrange(spec: &Spec, m: &Measured, t: &Tuning, shapes_of: &dyn Fn(usize, Rect) -> Shape) -> Drawn {
    connect(spec, m, layout::layout(spec, m, t), shapes_of)
}

/// Route the edges between boxes that are already placed, then set labels and notes.
fn connect(spec: &Spec, m: &Measured, mut geo: Geometry, shapes_of: &dyn Fn(usize, Rect) -> Shape) -> Drawn {

    // Work in positive coordinates with room around the content for routes.
    let all = |geo: &Geometry| geo.nodes.iter().chain(&geo.groups).chain(geo.notes.iter().flatten()).copied().reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
    let b = all(&geo);
    let shift = Vec2::new(64.0 - b.x0, 64.0 - b.y0);
    for r in geo.nodes.iter_mut().chain(geo.groups.iter_mut()).chain(geo.notes.iter_mut().flatten()) {
        *r = *r + shift;
    }
    let b = all(&geo);
    let mut grid = Grid::new(b.inflate(64.0, 64.0), Point::ZERO);
    for r in geo.nodes.iter().chain(geo.notes.iter().flatten()) {
        grid.block(r.inflate(8.0, 8.0));
    }
    for (g, frame) in geo.groups.iter().enumerate() {
        grid.outline(*frame);
        if spec.groups[g].label.is_some() {
            grid.penalise(group_label_rect(*frame, m.group_label[g]).inflate(8.0, 8.0), 80);
        }
    }

    // Ports: each side shares its length among the edges that use it, ordered to avoid crossings.
    let ends: Vec<(Ref, Ref)> = spec.edges.iter().map(|e| (spec.lookup(&e.from).unwrap(), spec.lookup(&e.to).unwrap())).collect();
    let mut using: BTreeMap<(Ref, usize), Vec<(usize, bool)>> = BTreeMap::new();
    let mut chosen: Vec<Option<(Dir, Dir)>> = Vec::new();
    for (i, (a, b)) in ends.iter().enumerate() {
        if a == b {
            chosen.push(None);
            continue;
        }
        let (sa, sb) = sides(box_of(&geo, *a), box_of(&geo, *b), spec.direction, spec.diagram);
        using.entry((*a, sa as usize)).or_default().push((i, true));
        using.entry((*b, sb as usize)).or_default().push((i, false));
        chosen.push(Some((sa, sb)));
    }
    // Each port aims straight at the other end, so aligned boxes get a straight edge;
    // ports that want the same spot are spread along the side. The second pass aims at
    // the ports the first one chose.
    // A box with a loop keeps its top right corner for it.
    let looped: Vec<Ref> = ends.iter().filter(|(a, b)| a == b).map(|(a, _)| *a).collect();
    for r in &looped {
        let b = box_of(&geo, *r);
        grid.mark(&[Point::new(b.x1 - 24.0, b.y0 - 16.0), Point::new(b.x1 + 16.0, b.y0 - 16.0), Point::new(b.x1 + 16.0, b.y0 + 16.0)]);
    }
    let mut port: BTreeMap<(usize, bool), Point> = BTreeMap::new();
    for pass in 0..2 {
        let before = port.clone();
        for ((r, side), list) in &using {
            let rect = box_of(&geo, *r);
            let side = [Dir::E, Dir::S, Dir::W, Dir::N][*side];
            let upright = matches!(side, Dir::E | Dir::W);
            let (lo, len) = if upright { (rect.y0, rect.height()) } else { (rect.x0, rect.width()) };
            let k = list.len() as f64;
            let spacing = if (k - 1.0) * 16.0 <= len - 32.0 { 16.0 } else if (k - 1.0) * 8.0 <= len - 16.0 { 8.0 } else { 0.0 };
            let inset: f64 = if spacing == 16.0 { 16.0 } else { 8.0 };
            let has_loop = looped.contains(r);
            let (first, last) = (if has_loop && side == Dir::E { inset.max(40.0) } else { inset }, if has_loop && side == Dir::N { (len - 48.0).max(inset) } else { len - inset });
            let mut wants: Vec<(f64, usize, bool)> = list
                .iter()
                .map(|(edge, is_from)| {
                    let other = if *is_from { ends[*edge].1 } else { ends[*edge].0 };
                    let aim = match before.get(&(*edge, !*is_from)) {
                        Some(p) if pass > 0 => *p,
                        _ => box_of(&geo, other).center(),
                    };
                    (layout::snap((if upright { aim.y } else { aim.x }) - lo), *edge, *is_from)
                })
                .collect();
            wants.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let mut at: Vec<f64> = Vec::new();
            for (n, w) in wants.iter().enumerate() {
                let floor = if n == 0 { first } else { at[n - 1] + spacing };
                at.push(w.0.max(floor));
            }
            // Whatever ran past the end is pushed back, keeping the spacing.
            let mut ceiling = last;
            for v in at.iter_mut().rev() {
                *v = v.min(ceiling).max(first.min(len / 2.0));
                ceiling = *v - spacing;
            }
            if spacing == 0.0 {
                at.iter_mut().for_each(|v| *v = layout::snap(len / 2.0));
            }
            for (w, off) in wants.iter().zip(at) {
                port.insert((w.1, w.2), side_point(rect, side, off));
            }
        }
    }
    let tip = |r: Ref, p: Point, side: Dir| match r {
        Ref::Node(i) => land(&shapes_of(i, geo.nodes[i]).outline, p, side),
        _ => p,
    };

    let mut routes: Vec<Vec<Point>> = Vec::new();
    for (i, (a, b)) in ends.iter().enumerate() {
        let Some((sa, sb)) = chosen[i] else {
            // A loop from a box back to itself, round its top right corner.
            let r = box_of(&geo, *a);
            let (start, end) = (Point::new(r.x1 - 24.0, r.y0), Point::new(r.x1, r.y0 + 16.0));
            routes.push(vec![tip(*a, start, Dir::N), Point::new(start.x, r.y0 - 16.0), Point::new(r.x1 + 16.0, r.y0 - 16.0), Point::new(r.x1 + 16.0, end.y), tip(*a, end, Dir::E)]);
            continue;
        };
        let (pa, pb) = (port[&(i, true)], port[&(i, false)]);
        let (s, t) = (pa + sa.vec() * STUB, pb + sb.vec() * STUB);
        // Frames that hold neither end are walls for this edge.
        let mut inside: Vec<usize> = spec.ancestors(*a);
        inside.extend(spec.ancestors(*b));
        for r in [a, b] {
            if let Ref::Group(g) = r {
                inside.push(*g);
            }
        }
        let walls: Vec<Rect> = (0..spec.groups.len()).filter(|g| !inside.contains(g)).map(|g| geo.groups[g]).collect();
        let mid = grid.route(s, sa, t, sb.opposite(), &walls).or_else(|| grid.route(s, sa, t, sb.opposite(), &[])).unwrap_or_else(|| vec![s, t]);
        let mut pts = vec![tip(*a, pa, sa)];
        pts.extend(mid);
        pts.push(tip(*b, pb, sb));
        routes.push(route::simplify(pts));
    }

    // Edge labels: beside the longest segment that has room.
    // How badly a rectangle collides with what is already drawn; 0 means free.
    let trouble = |rect: Rect, routes: &[Vec<Point>], taken: &[Rect], geo: &Geometry| -> usize {
        let probe = rect.inflate(2.0, 2.0);
        let boxes = geo.nodes.iter().chain(geo.notes.iter().flatten()).chain(taken).filter(|r| r.intersect(probe).area() > 0.0).count();
        let captions = (0..spec.groups.len()).filter(|g| spec.groups[*g].label.is_some() && group_label_rect(geo.groups[*g], m.group_label[*g]).intersect(probe).area() > 0.0).count();
        let frames = geo.groups.iter().filter(|f| f.intersect(probe).area() > 0.0 && !(probe.x0 > f.x0 && probe.y0 > f.y0 && probe.x1 < f.x1 && probe.y1 < f.y1)).count();
        let lines = routes.iter().filter(|r| segments(r).any(|(a, b)| svg::segment_hits_rect(a, b, probe))).count();
        (boxes + captions) * 100 + frames * 10 + lines
    };
    let mut taken: Vec<Rect> = Vec::new();
    let mut labels: Vec<Option<Label>> = Vec::new();
    for (i, e) in spec.edges.iter().enumerate() {
        let Some((w, h)) = m.edge_label[i] else {
            labels.push(None);
            continue;
        };
        let text = e.label.clone().unwrap_or_default();
        let mut segs: Vec<(usize, Point, Point)> = segments(&routes[i]).enumerate().map(|(k, (a, b))| (k, a, b)).collect();
        segs.sort_by(|x, y| y.1.distance(y.2).total_cmp(&x.1.distance(x.2)).then(x.0.cmp(&y.0)));
        let mut options: Vec<Rect> = Vec::new();
        for t in [0.5, 0.25, 0.75, 0.125, 0.875] {
            for (_, a, b) in &segs {
                let c = a.lerp(*b, t);
                if (a.y - b.y).abs() < 1e-9 {
                    options.push(Rect::new(c.x - w / 2.0, c.y - 5.0 - h, c.x + w / 2.0, c.y - 5.0));
                    options.push(Rect::new(c.x - w / 2.0, c.y + 5.0, c.x + w / 2.0, c.y + 5.0 + h));
                } else {
                    options.push(Rect::new(c.x + 6.0, c.y - h / 2.0, c.x + 6.0 + w, c.y + h / 2.0));
                    options.push(Rect::new(c.x - 6.0 - w, c.y - h / 2.0, c.x - 6.0, c.y + h / 2.0));
                }
            }
        }
        // A loop is too small to carry a label along its sides: try beside it and above it first.
        if ends[i].0 == ends[i].1 {
            let r = box_of(&geo, ends[i].0);
            options.insert(0, Rect::new(r.x1 + 22.0, r.y0 - h / 2.0, r.x1 + 22.0 + w, r.y0 + h / 2.0));
            options.insert(1, Rect::new(r.x1 - 4.0 - w / 2.0, r.y0 - 21.0 - h, r.x1 - 4.0 + w / 2.0, r.y0 - 21.0));
        }
        // The first free spot, or failing that the least crowded one.
        let rect = options.iter().enumerate().min_by_key(|(k, r)| (trouble(**r, &routes, &taken, &geo), *k)).map(|(_, r)| *r).unwrap_or(Rect::ZERO);
        taken.push(rect);
        labels.push(Some(Label { rect, text }));
    }

    // Notes on edges go in free space near the middle of the route.
    for (i, n) in spec.notes.iter().enumerate() {
        let Some(Ref::Edge(e)) = spec.lookup(&n.attach) else { continue };
        let mid = midpoint(&routes[e]);
        let (w, h) = (m.notes[i].w, m.notes[i].h);
        let order: Vec<Side> = [n.side, Side::Right, Side::Bottom, Side::Top, Side::Left].into_iter().fold(Vec::new(), |mut v, s| {
            if !v.contains(&s) {
                v.push(s)
            }
            v
        });
        // Widening rings round the middle of the edge, the preferred side first.
        let mut best: Option<(usize, Rect)> = None;
        'search: for ring in 0..14 {
            let gap = 24.0 + 16.0 * ring as f64;
            for side in &order {
                for slide in (0..=ring * 2).map(|k| if k % 2 == 0 { 16.0 * (k / 2) as f64 } else { -16.0 * (k / 2 + 1) as f64 }) {
                    let (x, y) = match side {
                        Side::Right => (mid.x + gap, mid.y - h / 2.0 + slide),
                        Side::Left => (mid.x - gap - w, mid.y - h / 2.0 + slide),
                        Side::Top => (mid.x - w / 2.0 + slide, mid.y - gap - h),
                        Side::Bottom => (mid.x - w / 2.0 + slide, mid.y + gap),
                    };
                    let (x, y) = (layout::snap(x), layout::snap(y));
                    let r = Rect::new(x, y, x + w, y + h);
                    // The leader should not run through a box on its way to the edge.
                    let from = Point::new(mid.x.clamp(r.x0, r.x1), mid.y.clamp(r.y0, r.y1));
                    let through = geo.nodes.iter().chain(geo.notes.iter().flatten()).filter(|b| svg::segment_hits_rect(from, mid, b.inflate(-1.0, -1.0))).count();
                    let score = trouble(r.inflate(8.0, 8.0), &routes, &taken, &geo) + through * 100;
                    if best.is_none_or(|(s, _)| score < s) {
                        best = Some((score, r));
                    }
                    if score == 0 {
                        break 'search;
                    }
                }
            }
        }
        geo.notes[i] = best.map(|(_, r)| r);
    }

    // Leaders: a short line from each note to what it points at.
    let mut leaders = Vec::new();
    for (i, n) in spec.notes.iter().enumerate() {
        let Some(note) = geo.notes[i] else {
            leaders.push(None);
            continue;
        };
        let target = spec.lookup(&n.attach).unwrap();
        let line = match target {
            Ref::Edge(e) => {
                let mid = midpoint(&routes[e]);
                let from = Point::new(mid.x.clamp(note.x0, note.x1), mid.y.clamp(note.y0, note.y1));
                (from, mid)
            }
            _ => {
                let own = box_of(&geo, target);
                let c = note.center();
                let (from, to, side) = if note.x0 >= own.x1 {
                    (Point::new(note.x0, c.y), Point::new(own.x1, c.y.clamp(own.y0 + 8.0, own.y1 - 8.0)), Dir::E)
                } else if note.x1 <= own.x0 {
                    (Point::new(note.x1, c.y), Point::new(own.x0, c.y.clamp(own.y0 + 8.0, own.y1 - 8.0)), Dir::W)
                } else if note.y1 <= own.y0 {
                    (Point::new(c.x, note.y1), Point::new(c.x.clamp(own.x0 + 8.0, own.x1 - 8.0), own.y0), Dir::N)
                } else {
                    (Point::new(c.x, note.y0), Point::new(c.x.clamp(own.x0 + 8.0, own.x1 - 8.0), own.y1), Dir::S)
                };
                (from, tip(target, to, side))
            }
        };
        leaders.push(Some(line));
    }

    fit(geo, routes, labels, leaders, vec![None; spec.nodes.len()])
}

/// Fit the canvas to everything drawn, with a margin that scales down for small diagrams.
pub(crate) fn fit(mut geo: Geometry, mut routes: Vec<Vec<Point>>, mut labels: Vec<Option<Label>>, mut leaders: Vec<Option<(Point, Point)>>, mut lifelines: Vec<Option<(Point, Point)>>) -> Drawn {
    let all = |geo: &Geometry| geo.nodes.iter().chain(&geo.groups).chain(geo.notes.iter().flatten()).copied().reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
    let mut b = all(&geo);
    for l in lifelines.iter().flatten() {
        b = b.union(Rect::from_points(l.0, l.1));
    }
    for r in &routes {
        for p in r {
            b = b.union(Rect::from_center_size(*p, (4.0, 4.0)));
        }
    }
    for l in labels.iter().flatten() {
        b = b.union(l.rect);
    }
    let (left, top) = ((b.x0 / 8.0).floor() * 8.0, (b.y0 / 8.0).floor() * 8.0);
    let (cw, ch) = (layout::up(b.x1 - left, 8.0), layout::up(b.y1 - top, 8.0));
    // The widest margin on the grid that keeps each side, rounding slack included, under
    // the excess-margin limit; small drawings get a narrow one.
    let margin = |c: f64, slack: f64| [24.0, 16.0, 8.0].into_iter().find(|m| (m + slack) / (c + 2.0 * m) <= 0.13).unwrap_or(8.0);
    let mx = margin(cw, (b.x0 - left).max(cw - (b.x1 - left)));
    let my = margin(ch, (b.y0 - top).max(ch - (b.y1 - top)));
    let shift = Vec2::new(mx - left, my - top);
    for r in geo.nodes.iter_mut().chain(geo.groups.iter_mut()).chain(geo.notes.iter_mut().flatten()) {
        *r = *r + shift;
    }
    for r in routes.iter_mut() {
        for p in r.iter_mut() {
            *p += shift;
        }
    }
    for l in labels.iter_mut().flatten() {
        l.rect = l.rect + shift;
    }
    for l in leaders.iter_mut().chain(lifelines.iter_mut()).flatten() {
        *l = (l.0 + shift, l.1 + shift);
    }
    Drawn { geo, routes, labels, leaders, lifelines, marks: Vec::new(), axis: Vec::new(), size: (cw + 2.0 * mx, ch + 2.0 * my) }
}

struct Writer {
    out: String,
    as_paths: bool,
}

impl Writer {
    /// Lines of text centred on (cx, cy); `anchor` is start or middle.
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, indent: &str, lines: &[String], x: f64, cy: f64, size: f64, leading: f64, fill: &str, anchor: &str) {
        let (asc, desc) = text::vmetrics(Class::Sans, false);
        let n = lines.len() as f64;
        let baseline = |i: usize| cy + (i as f64 - (n - 1.0) / 2.0) * leading + (asc - desc) / 2.0 * size;
        if self.as_paths {
            for (i, line) in lines.iter().enumerate() {
                let w = layout::text_width(line, size);
                let start = match anchor {
                    "middle" => x - w / 2.0,
                    "end" => x - w,
                    _ => x,
                };
                let d = text::outline(line, size, start, baseline(i));
                if !d.is_empty() {
                    self.out.push_str(&format!("{indent}<path d=\"{d}\" fill=\"{fill}\"/>\n"));
                }
            }
            return;
        }
        let anchor_attr = match anchor {
            "middle" => " text-anchor=\"middle\"",
            "end" => " text-anchor=\"end\"",
            _ => "",
        };
        if lines.len() == 1 {
            self.out.push_str(&format!("{indent}<text x=\"{}\" y=\"{}\"{anchor_attr} font-size=\"{}\" fill=\"{fill}\">{}</text>\n", fmt(x), fmt(baseline(0)), fmt(size), esc(&lines[0])));
        } else {
            self.out.push_str(&format!("{indent}<text{anchor_attr} font-size=\"{}\" fill=\"{fill}\">", fmt(size)));
            for (i, line) in lines.iter().enumerate() {
                self.out.push_str(&format!("<tspan x=\"{}\" y=\"{}\">{}</tspan>", fmt(x), fmt(baseline(i)), esc(line)));
            }
            self.out.push_str("</text>\n");
        }
    }

    /// A numbered step badge centred on `c`; `quiet` greys it, for a step that is not the current one.
    fn badge(&mut self, indent: &str, c: Point, number: usize, theme: &theme::Theme, quiet: bool) {
        let (disc, ink) = if quiet { (theme.muted, theme.background) } else { (theme.accent_fill, theme.accent_text) };
        self.out.push_str(&format!("{indent}<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{disc}\" data-badge=\"{number}\"/>\n", fmt(c.x), fmt(c.y), fmt(explain::BADGE / 2.0)));
        self.text(indent, &[number.to_string()], c.x, c.y, SMALL, 16.0, ink, "middle");
    }

    /// What every drawn thing opens with: its group, with the steps that name it, a
    /// link around it and its detail as a tooltip, where the spec gives them.
    fn open(&mut self, id: &str, steps: &[usize], link: Option<&str>, detail: Option<&str>, extra: &str) {
        if let Some(href) = link {
            self.out.push_str(&format!("  <a href=\"{}\">\n", esc_attr(href)));
        }
        let data = if steps.is_empty() { String::new() } else { format!(" data-step=\"{}\"", steps.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" ")) };
        self.out.push_str(&format!("  <g id=\"{id}\"{data}{extra}>\n"));
        if let Some(d) = detail {
            self.out.push_str(&format!("    <title>{}</title>\n", esc(d)));
        }
    }

    fn close(&mut self, link: Option<&str>) {
        self.out.push_str("  </g>\n");
        if link.is_some() {
            self.out.push_str("  </a>\n");
        }
    }

    fn part(&mut self, p: &Part, fill: &str, stroke: &str) {
        let pen = format!("stroke=\"{stroke}\" stroke-width=\"{}\"", fmt(STROKE));
        match p {
            Part::Rect { r, rx, dashed } => {
                let rx = if *rx > 0.0 { format!(" rx=\"{}\"", fmt(*rx)) } else { String::new() };
                let dash = if *dashed { " stroke-dasharray=\"6 4\"" } else { "" };
                self.out.push_str(&format!("    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} fill=\"{fill}\" {pen}{dash}/>\n", fmt(r.x0), fmt(r.y0), fmt(r.width()), fmt(r.height())));
            }
            Part::Path { d, filled } => {
                let f = if *filled { fill } else { "none" };
                self.out.push_str(&format!("    <path d=\"{d}\" fill=\"{f}\" {pen}/>\n"));
            }
            Part::Polygon(pts) => {
                let list: Vec<String> = pts.iter().map(|p| format!("{},{}", fmt(p.x), fmt(p.y))).collect();
                self.out.push_str(&format!("    <polygon points=\"{}\" fill=\"{fill}\" {pen}/>\n", list.join(" ")));
            }
            Part::Line(a, b) => {
                self.out.push_str(&format!("    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" {pen}/>\n", fmt(a.x), fmt(a.y), fmt(b.x), fmt(b.y)));
            }
            Part::Disc { c, r, ink: true } => {
                self.out.push_str(&format!("    <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{stroke}\"/>\n", fmt(c.x), fmt(c.y), fmt(*r)));
            }
            Part::Disc { c, r, ink: false } => {
                self.out.push_str(&format!("    <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{fill}\" {pen}/>\n", fmt(c.x), fmt(c.y), fmt(*r)));
            }
        }
    }
}

pub fn render(spec: &Spec, opt: &Options) -> Result<String, AppError> {
    let name = opt.theme.or(spec.theme.as_deref()).unwrap_or("docs-light");
    let theme = theme::find(name).ok_or_else(|| {
        let e = if opt.theme.is_some() { AppError::usage("unknown-theme", format!("Unknown theme '{name}'.")) } else { AppError::input("unknown-theme", format!("Unknown theme '{name}'.")).path("theme") };
        e.suggest(did_you_mean(name, theme::names()).or(Some(format!("Themes: {}.", theme::names().collect::<Vec<_>>().join(", ")))))
    })?;
    let kinds: Vec<Kind> = spec.nodes.iter().map(|n| n.kind).collect();
    // Set the diagram as roomily as fits: tighter wrapping and gaps are tried only
    // when the drawing would be wider than max_width.
    let mut chosen = None;
    for t in &layout::LEVELS {
        let m = layout::measure(spec, t);
        let d = match spec.diagram {
            Diagram::Sequence => sequence::arrange(spec, &m, t, &|i, r| shape(kinds[i], r)),
            Diagram::Timeline => timeline::arrange(spec, &m),
            Diagram::Compare => connect(spec, &m, compare::layout(spec, &m), &|i, r| shape(kinds[i], r)),
            _ => arrange(spec, &m, t, &|i, r| shape(kinds[i], r)),
        };
        let fits = d.size.0 <= spec.max_width + 1e-6;
        chosen = Some((m, d));
        if fits {
            break;
        }
    }
    let (m, mut d) = chosen.expect("there is at least one layout level");
    explain::grow_for_badges(spec, &mut d);
    let (w, h) = d.size;
    if w > spec.max_width + 1e-6 {
        let hint = if matches!(spec.diagram, Diagram::Flow | Diagram::Layers | Diagram::State | Diagram::Timeline) && spec.direction == Direction::Right { "Try 'direction: down', shorter labels, or a larger max_width." } else { "Try shorter labels, fewer boxes side by side, or a larger max_width." };
        return Err(AppError::input("too-wide", format!("The drawing is {} px wide even at the tightest spacing; max_width allows {}.", fmt(w), fmt(spec.max_width))).path("max_width").suggest(Some(hint.into())));
    }

    // The legend, the steps and the caption go under the drawing; a drawing narrower
    // than they are is centred above them.
    let footer = explain::Footer::plan(spec, w);
    let (drawing_w, drawing_h) = (w, h);
    let w = w.max(footer.canvas());
    let h = h + footer.height;
    d.shift(Vec2::new(((w - drawing_w) / 16.0).floor() * 8.0, 0.0));
    let dim: Vec<bool> = (0..spec.edges.len()).map(|i| !spec.in_focus(Ref::Edge(i))).collect();

    let family = if opt.fonts.is_empty() { "Inter, sans-serif".to_string() } else { opt.fonts.join(", ") };
    let mut wr = Writer { out: String::new(), as_paths: opt.text_as_paths };
    let font_attr = if opt.text_as_paths { String::new() } else { format!(" font-family=\"{}\"", esc_attr(&family)) };
    wr.out.push_str(&format!("<svg xmlns=\"{SVG_NS}\" viewBox=\"0 0 {} {}\" data-svgdog=\"1\"{font_attr}>\n", fmt(w), fmt(h)));
    wr.out.push_str(&format!("  <title>{}</title>\n", esc(&spec.title)));
    // Without a description of its own, the drawing says in a sentence what it shows.
    let desc = spec.description.clone().unwrap_or_else(|| crate::describe::short(spec));
    wr.out.push_str(&format!("  <desc>{}</desc>\n", esc(&desc)));
    if let Some(doc) = opt.embed {
        wr.out.push_str(&format!("  <metadata id=\"{}\">{}</metadata>\n", crate::describe::EMBED_ID, esc(&doc.to_string())));
    }
    let need = |start: bool, dimmed: bool| spec.edges.iter().zip(&dim).any(|(e, d)| *d == dimmed && if start { e.arrow == Arrow::Both } else { e.arrow != Arrow::None });
    let (need_end, need_start) = (need(false, false), need(true, false));
    if need_end || need_start || need(false, true) || need(true, true) {
        wr.out.push_str("  <defs>\n");
        if need_end {
            wr.out.push_str(&format!("    <marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"8\" refX=\"10\" refY=\"4\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0 0L10 4L0 8Z\" fill=\"{}\"/></marker>\n", theme.edge));
        }
        if need_start {
            wr.out.push_str(&format!("    <marker id=\"arrow-start\" markerWidth=\"10\" markerHeight=\"8\" refX=\"0\" refY=\"4\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M10 0L0 4L10 8Z\" fill=\"{}\"/></marker>\n", theme.edge));
        }
        if need(false, true) {
            wr.out.push_str(&format!("    <marker id=\"arrow-dim\" markerWidth=\"10\" markerHeight=\"8\" refX=\"10\" refY=\"4\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M0 0L10 4L0 8Z\" fill=\"{}\"/></marker>\n", theme.muted));
        }
        if need(true, true) {
            wr.out.push_str(&format!("    <marker id=\"arrow-start-dim\" markerWidth=\"10\" markerHeight=\"8\" refX=\"0\" refY=\"4\" orient=\"auto\" markerUnits=\"userSpaceOnUse\"><path d=\"M10 0L0 4L10 8Z\" fill=\"{}\"/></marker>\n", theme.muted));
        }
        wr.out.push_str("  </defs>\n");
    }
    wr.out.push_str(&format!("  <rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n", fmt(w), fmt(h), theme.background));

    // What the interactive version adds to a thing: its classes, and a place in the tab order for nodes.
    let live = |r: Ref, focusable: bool| -> String {
        if !opt.interactive {
            return String::new();
        }
        format!(" class=\"{}\"{}", interactive::classes(spec, r), if focusable { " tabindex=\"0\"" } else { "" })
    };
    // Frames first, outermost first, so everything else paints over them.
    let mut order: Vec<usize> = (0..spec.groups.len()).collect();
    order.sort_by_key(|g| (spec.ancestors(Ref::Group(*g)).len(), *g));
    for g in order {
        let f = d.geo.groups[g];
        let gr = &spec.groups[g];
        let steps = spec.steps_of(Ref::Group(g));
        wr.open(&format!("group-{}", gr.id), &steps, gr.link.as_deref(), gr.detail.as_deref(), &live(Ref::Group(g), false));
        wr.out.push_str(&format!("    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>\n", fmt(f.x0), fmt(f.y0), fmt(f.width()), fmt(f.height()), theme.muted, fmt(THIN)));
        if let Some(label) = &spec.groups[g].label {
            wr.text("    ", std::slice::from_ref(label), f.x0 + 12.0, f.y0 + 20.0, SMALL, 16.0, theme.muted, "start");
        }
        for (c, number) in explain::group_badges(f, steps.len()).into_iter().zip(&steps) {
            if spec.marks(*number) {
                wr.badge("    ", c, *number, theme, false);
            }
        }
        wr.close(gr.link.as_deref());
    }
    if !d.axis.is_empty() {
        wr.out.push_str("  <g id=\"axis\">\n");
        for (a, b) in &d.axis {
            wr.out.push_str(&format!("    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>\n", fmt(a.x), fmt(a.y), fmt(b.x), fmt(b.y), theme.edge, fmt(STROKE)));
        }
        wr.out.push_str("  </g>\n");
    }
    for (i, n) in spec.nodes.iter().enumerate() {
        let r = d.geo.nodes[i];
        // Out of focus, a node keeps its shape and loses its colour.
        let (fill, ink, pen) = match (spec.in_focus(Ref::Node(i)), n.emphasis || spec.one_sided(i)) {
            (false, _) => (theme.background, theme.muted, theme.muted),
            (true, true) => (theme.accent_fill, theme.accent_text, theme.node_stroke),
            (true, false) => (theme.node_fill, theme.text, theme.node_stroke),
        };
        let steps = spec.steps_of(Ref::Node(i));
        wr.open(&format!("node-{}", n.id), &steps, n.link.as_deref(), n.detail.as_deref(), &live(Ref::Node(i), true));
        if let Some((a, b)) = d.lifelines[i] {
            wr.out.push_str(&format!("    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"4 4\"/>\n", fmt(a.x), fmt(a.y), fmt(b.x), fmt(b.y), theme.muted, fmt(THIN)));
        }
        for mark in d.marks.get(i).into_iter().flatten() {
            match mark {
                Mark::Stem(a, b) => wr.out.push_str(&format!("    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n", fmt(a.x), fmt(a.y), fmt(b.x), fmt(b.y), theme.muted, fmt(THIN))),
                Mark::Dot(c) => wr.out.push_str(&format!("    <circle cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n", fmt(c.x), fmt(c.y), theme.background, pen, fmt(STROKE))),
                Mark::When { at, text, anchor, .. } => wr.text("    ", std::slice::from_ref(text), at.x, at.y, SMALL, 16.0, if spec.in_focus(Ref::Node(i)) { theme.text } else { theme.muted }, anchor),
            }
        }
        for p in &shape(n.kind, r).parts {
            wr.part(p, fill, pen);
        }
        if !n.label.is_empty() {
            wr.text("    ", &m.nodes[i].text.lines, r.center().x, r.center().y + m.nodes[i].text_dy, FONT, 20.0, ink, "middle");
        }
        for (c, number) in explain::node_badges(n.kind, r, steps.len()).into_iter().zip(&steps) {
            if spec.marks(*number) {
                wr.badge("    ", c, *number, theme, false);
            }
        }
        wr.close(n.link.as_deref());
    }
    for (i, e) in spec.edges.iter().enumerate() {
        let pts = &d.routes[i];
        let mut path = String::new();
        for (k, p) in pts.iter().enumerate() {
            path.push_str(&format!("{}{}", if k == 0 { "M" } else { " L" }, p2(p.x, p.y)));
        }
        let dash = match e.style {
            EdgeStyle::Sync => "",
            EdgeStyle::Async => " stroke-dasharray=\"6 4\"",
            EdgeStyle::Dependency => " stroke-dasharray=\"2 4\"",
        };
        let markers = match (e.arrow, dim[i]) {
            (Arrow::End, false) => " marker-end=\"url(#arrow)\"",
            (Arrow::Both, false) => " marker-start=\"url(#arrow-start)\" marker-end=\"url(#arrow)\"",
            (Arrow::End, true) => " marker-end=\"url(#arrow-dim)\"",
            (Arrow::Both, true) => " marker-start=\"url(#arrow-start-dim)\" marker-end=\"url(#arrow-dim)\"",
            (Arrow::None, _) => "",
        };
        let (line, ink) = if dim[i] { (theme.muted, theme.muted) } else { (theme.edge, theme.text) };
        let steps = spec.steps_of(Ref::Edge(i));
        wr.open(&format!("edge-{}", e.id), &steps, e.link.as_deref(), e.detail.as_deref(), &live(Ref::Edge(i), false));
        wr.out.push_str(&format!("    <path d=\"{path}\" fill=\"none\" stroke=\"{line}\" stroke-width=\"{}\"{dash}{markers}/>\n", fmt(STROKE)));
        if let Some(l) = &d.labels[i] {
            // Badges take the left of the label's box; the words are centred in the rest.
            let room = explain::badge_room(steps.len(), !l.text.is_empty());
            for (k, number) in steps.iter().enumerate() {
                if spec.marks(*number) {
                    wr.badge("    ", Point::new(l.rect.x0 + explain::BADGE / 2.0 + k as f64 * explain::PITCH, l.rect.center().y), *number, theme, false);
                }
            }
            if !l.text.is_empty() {
                wr.text("    ", std::slice::from_ref(&l.text), (l.rect.x0 + room + l.rect.x1) / 2.0, l.rect.center().y, SMALL, 16.0, ink, "middle");
            }
        }
        wr.close(e.link.as_deref());
    }
    for (i, _) in spec.notes.iter().enumerate() {
        let Some(r) = d.geo.notes[i] else { continue };
        let class = if opt.interactive { format!(" class=\"{}\"", interactive::note_classes(spec, &spec.notes[i].attach)) } else { String::new() };
        wr.out.push_str(&format!("  <g id=\"note-{}\"{class}>\n", i + 1));
        if let Some((a, b)) = d.leaders[i] {
            wr.out.push_str(&format!("    <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"2 3\"/>\n", fmt(a.x), fmt(a.y), fmt(b.x), fmt(b.y), theme.muted, fmt(THIN)));
        }
        wr.out.push_str(&format!("    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n", fmt(r.x0), fmt(r.y0), fmt(r.width()), fmt(r.height()), theme.note_fill, theme.muted, fmt(THIN)));
        let ink = if spec.lookup(&spec.notes[i].attach).is_none_or(|r| spec.in_focus(r)) { theme.text } else { theme.muted };
        wr.text("    ", &m.notes[i].text.lines, r.center().x, r.center().y, SMALL, 16.0, ink, "middle");
        wr.out.push_str("  </g>\n");
    }
    footer.write(&mut wr, theme, drawing_h);
    wr.out.push_str("</svg>\n");
    let mut out = if theme.name == theme::AUTO { theme::adapt(&wr.out) } else { wr.out };
    if opt.interactive {
        let accent = if theme.name == theme::AUTO { theme::THEMES[0].accent_fill } else { theme.accent_fill };
        out = interactive::style(&out, spec, theme, accent);
    }
    Ok(out)
}

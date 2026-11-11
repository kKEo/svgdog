//! Sizes and positions: every box lands on the 8 px grid, groups are laid out
//! inside out, and notes reserve room beside the thing they point at.

use kurbo::{Point, Rect};

use super::theme::{FONT, SMALL};
use crate::spec::{Diagram, Direction, Kind, Ref, Side, Spec};
use crate::svg::text::{self, Class};

pub const GAP: f64 = 32.0;
pub const RANK_GAP: f64 = 48.0;
pub const PAD: f64 = 16.0;
pub const PAD_LABEL: f64 = 40.0;
pub const NOTE_GAP: f64 = 24.0;
/// Share of the measured width kept free for font substitution at the reader's end.
pub const RESERVE: f64 = 0.08;

/// How tightly a diagram is set. Render starts with the roomiest level and moves
/// to a tighter one only when the drawing would be wider than max_width.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Widest a node label runs before it wraps.
    pub wrap: f64,
    pub note_wrap: f64,
    /// Room kept beside an edge label in the gap it sits in.
    pub label_pad: f64,
}

pub const LEVELS: [Tuning; 3] = [Tuning { wrap: 160.0, note_wrap: 168.0, label_pad: 32.0 }, Tuning { wrap: 120.0, note_wrap: 136.0, label_pad: 24.0 }, Tuning { wrap: 88.0, note_wrap: 104.0, label_pad: 16.0 }];

pub fn up(v: f64, step: f64) -> f64 {
    ((v - 1e-9) / step).ceil() * step
}

pub fn snap(v: f64) -> f64 {
    (v / 8.0).round() * 8.0
}

pub fn text_width(s: &str, size: f64) -> f64 {
    text::measure(s, Class::Sans, false).width * size
}

/// Greedy word wrap; explicit newlines always break.
pub fn wrap(label: &str, size: f64, max: f64) -> Vec<String> {
    let mut lines = Vec::new();
    for para in label.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if !cur.is_empty() && text_width(&trial, size) * (1.0 + RESERVE) > max {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            } else {
                cur = trial;
            }
        }
        lines.push(cur);
    }
    if lines.iter().all(|l| l.is_empty()) {
        lines.truncate(1);
    }
    lines
}

pub struct TextBlock {
    pub lines: Vec<String>,
    /// Widest line with the reserve included.
    pub width: f64,
}

fn block(label: &str, size: f64, max: f64) -> TextBlock {
    let lines = wrap(label, size, max);
    let width = lines.iter().map(|l| text_width(l, size)).fold(0.0, f64::max) * (1.0 + RESERVE);
    TextBlock { lines, width }
}

pub struct NodeBox {
    pub w: f64,
    pub h: f64,
    pub text: TextBlock,
    /// Shift of the label from the box centre, for shapes with a cap or a header.
    pub text_dy: f64,
}

pub struct NoteBox {
    pub w: f64,
    pub h: f64,
    pub text: TextBlock,
}

pub struct Measured {
    pub nodes: Vec<NodeBox>,
    pub notes: Vec<NoteBox>,
    pub group_label: Vec<f64>,
    /// Width and height of each edge label.
    pub edge_label: Vec<Option<(f64, f64)>>,
}

pub fn measure(spec: &Spec, t: &Tuning) -> Measured {
    let nodes = spec
        .nodes
        .iter()
        .map(|n| {
            let fixed = n.width.map(|w| up(w, 16.0));
            let text = block(&n.label, FONT, fixed.map(|w| (w - 32.0).max(24.0)).unwrap_or(t.wrap));
            let lines = text.lines.len() as f64;
            let (w, h, text_dy) = match n.kind {
                Kind::Start | Kind::End => (24.0, 24.0, 0.0),
                Kind::Decision => (up(text.width * 1.5 + 48.0, 16.0).max(128.0), up(lines * 20.0 + 44.0, 16.0).max(64.0), 0.0),
                Kind::Datastore => (up(text.width + 32.0, 16.0).max(96.0), up(lines * 20.0 + 44.0, 16.0).max(64.0), 6.0),
                Kind::Client => (up(text.width + 32.0, 16.0).max(96.0), up(lines * 20.0 + 44.0, 16.0).max(64.0), 8.0),
                Kind::Queue => (up(text.width + 56.0, 16.0).max(112.0), up(lines * 20.0 + 28.0, 16.0).max(48.0), 0.0),
                Kind::Actor => (up(text.width + 48.0, 16.0).max(96.0), up(lines * 20.0 + 28.0, 16.0).max(48.0), 0.0),
                Kind::Document => (up(text.width + 32.0, 16.0).max(96.0), up(lines * 20.0 + 40.0, 16.0).max(64.0), -4.0),
                _ => (up(text.width + 32.0, 16.0).max(96.0), up(lines * 20.0 + 28.0, 16.0).max(48.0), 0.0),
            };
            NodeBox { w: fixed.map(|f| f.max(w.min(f))).unwrap_or(w), h, text, text_dy }
        })
        .collect();
    let notes = spec
        .notes
        .iter()
        .map(|n| {
            let text = block(&n.text, SMALL, t.note_wrap);
            NoteBox { w: up(text.width + 20.0, 16.0).max(64.0), h: up(text.lines.len() as f64 * 16.0 + 14.0, 16.0).max(32.0), text }
        })
        .collect();
    let group_label = spec.groups.iter().map(|g| g.label.as_deref().map(|l| text_width(l, SMALL) * (1.0 + RESERVE)).unwrap_or(0.0)).collect();
    let (asc, desc) = text::vmetrics(Class::Sans, false);
    // An edge named by a step carries its number beside the label, or alone.
    let edge_label = spec
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let badges = spec.steps_of(crate::spec::Ref::Edge(i)).len();
            let text = e.label.as_deref().map(|l| text_width(l, SMALL) * (1.0 + RESERVE));
            (text.is_some() || badges > 0).then(|| (text.unwrap_or(0.0) + super::explain::badge_room(badges, text.is_some()), if badges > 0 { super::explain::BADGE } else { (asc + desc) * SMALL }))
        })
        .collect();
    Measured { nodes, notes, group_label, edge_label }
}

#[derive(Default)]
pub struct Geometry {
    pub nodes: Vec<Rect>,
    pub groups: Vec<Rect>,
    /// Notes on nodes and groups; edge notes are placed after routing.
    pub notes: Vec<Option<Rect>>,
}

/// An item's own box plus the notes around it.
pub(crate) struct Foot {
    pub w: f64,
    pub h: f64,
    /// Where the own box sits inside the footprint.
    pub ox: f64,
    pub oy: f64,
    /// Note boxes relative to the own box's origin.
    pub notes: Vec<(usize, Rect)>,
}

pub(crate) fn foot(spec: &Spec, m: &Measured, me: Ref, w: f64, h: f64) -> Foot {
    let mut notes = Vec::new();
    for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
        let mine: Vec<usize> = (0..spec.notes.len()).filter(|i| spec.notes[*i].side == side && spec.lookup(&spec.notes[*i].attach) == Some(me)).collect();
        if mine.is_empty() {
            continue;
        }
        let vertical = matches!(side, Side::Left | Side::Right);
        let total: f64 = mine.iter().map(|i| if vertical { m.notes[*i].h } else { m.notes[*i].w }).sum::<f64>() + 8.0 * (mine.len() as f64 - 1.0);
        let mut along = snap(((if vertical { h } else { w }) - total) / 2.0);
        for i in mine {
            let (nw, nh) = (m.notes[i].w, m.notes[i].h);
            let r = match side {
                Side::Right => Rect::new(w + NOTE_GAP, along, w + NOTE_GAP + nw, along + nh),
                Side::Left => Rect::new(-NOTE_GAP - nw, along, -NOTE_GAP, along + nh),
                Side::Top => Rect::new(along, -NOTE_GAP - nh, along + nw, -NOTE_GAP),
                Side::Bottom => Rect::new(along, h + NOTE_GAP, along + nw, h + NOTE_GAP + nh),
            };
            along += (if vertical { nh } else { nw }) + 8.0;
            notes.push((i, r));
        }
    }
    let mut all = notes.iter().fold(Rect::new(0.0, 0.0, w, h), |a, (_, r)| a.union(*r));
    // A labelled loop back to the same box needs room beside its top right corner for the words.
    for (i, e) in spec.edges.iter().enumerate() {
        if let (Some(label), true) = (m.edge_label[i], spec.lookup(&e.from) == Some(me) && e.from == e.to) {
            all = all.union(Rect::new(w, -16.0, w + 22.0 + label.0, 16.0));
        }
    }
    let (ox, oy) = (up(-all.x0, 8.0), up(-all.y0, 8.0));
    Foot { w: ox + up(all.x1, 8.0), h: oy + up(all.y1, 8.0), ox, oy, notes }
}

/// Direct children of a group, or of the root for `None`.
pub fn children(spec: &Spec, parent: Option<usize>) -> Vec<Ref> {
    match parent {
        None => (0..spec.nodes.len())
            .filter(|i| spec.node_parent[*i].is_none())
            .map(Ref::Node)
            .chain((0..spec.groups.len()).filter(|i| spec.group_parent[*i].is_none()).map(Ref::Group))
            .collect(),
        Some(g) => spec.groups[g]
            .members
            .iter()
            .filter_map(|m| spec.lookup(m))
            .filter(|r| match r {
                Ref::Node(i) => spec.node_parent[*i] == Some(g),
                Ref::Group(i) => spec.group_parent[*i] == Some(g),
                Ref::Edge(_) => false,
            })
            .collect(),
    }
}

/// Edges between the children of one container, with the label width each needs.
fn level_edges(spec: &Spec, m: &Measured, kids: &[Ref]) -> Vec<(usize, usize, f64)> {
    let slot = |id: &str| -> Option<usize> {
        let r = spec.lookup(id)?;
        let up = spec.ancestors(r);
        kids.iter().position(|k| *k == r || matches!(k, Ref::Group(g) if up.contains(g)))
    };
    let mut out = Vec::new();
    for (i, e) in spec.edges.iter().enumerate() {
        if let (Some(a), Some(b)) = (slot(&e.from), slot(&e.to))
            && a != b
        {
            out.push((a, b, m.edge_label[i].map(|l| l.0).unwrap_or(0.0)));
        }
    }
    out
}

/// Layered placement of boxes: ranks along the main axis, stacks across it.
/// Returns each box's top-left corner and the total size.
///
/// `anchor` is where edges meet each box across the flow, measured from the box's
/// start; lining anchors up, not box centres, is what keeps edges straight.
fn layered(sizes: &[(f64, f64)], anchor: &[f64], edges: &[(usize, usize, f64)], dir: Direction, pad: f64) -> (Vec<Point>, (f64, f64)) {
    let n = sizes.len();
    let main = |i: usize| if dir == Direction::Right { sizes[i].0 } else { sizes[i].1 };
    let cross = |i: usize| if dir == Direction::Right { sizes[i].1 } else { sizes[i].0 };

    // Break cycles: an edge back to a box still on the search stack is turned round.
    let mut out_of: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (a, b, _) in edges {
        if !out_of[*a].contains(b) {
            out_of[*a].push(*b);
        }
    }
    let mut state = vec![0u8; n];
    let mut flow: Vec<(usize, usize)> = Vec::new();
    fn dfs(u: usize, out_of: &[Vec<usize>], state: &mut [u8], flow: &mut Vec<(usize, usize)>) {
        state[u] = 1;
        for &v in &out_of[u] {
            if state[v] == 1 {
                flow.push((v, u));
            } else {
                flow.push((u, v));
                if state[v] == 0 {
                    dfs(v, out_of, state, flow);
                }
            }
        }
        state[u] = 2;
    }
    for u in 0..n {
        if state[u] == 0 {
            dfs(u, &out_of, &mut state, &mut flow);
        }
    }
    flow.sort();
    flow.dedup();
    let preds = |v: usize| flow.iter().filter(move |(_, b)| *b == v).map(|(a, _)| *a);
    let succs = |u: usize| flow.iter().filter(move |(a, _)| *a == u).map(|(_, b)| *b);

    // Longest-path ranks.
    let mut rank = vec![0usize; n];
    let mut indeg: Vec<usize> = (0..n).map(|v| preds(v).count()).collect();
    let mut queue: Vec<usize> = (0..n).filter(|v| indeg[*v] == 0).collect();
    let mut head = 0;
    while head < queue.len() {
        let u = queue[head];
        head += 1;
        for v in succs(u) {
            rank[v] = rank[v].max(rank[u] + 1);
            indeg[v] -= 1;
            if indeg[v] == 0 {
                queue.push(v);
            }
        }
    }
    let count = rank.iter().max().map(|r| r + 1).unwrap_or(0);
    let mut ranks: Vec<Vec<usize>> = (0..count).map(|r| (0..n).filter(|v| rank[*v] == r).collect()).collect();

    // Order each rank by the average position of its neighbours, a few sweeps each way.
    let place = |ranks: &[Vec<usize>]| -> Vec<f64> {
        let mut p = vec![0.0; n];
        for r in ranks {
            for (k, v) in r.iter().enumerate() {
                p[*v] = (k as f64 + 0.5) / r.len() as f64;
            }
        }
        p
    };
    for _ in 0..3 {
        for r in 1..count {
            let p = place(&ranks);
            let key = |v: usize| {
                let ns: Vec<f64> = preds(v).map(|u| p[u]).collect();
                if ns.is_empty() { p[v] } else { ns.iter().sum::<f64>() / ns.len() as f64 }
            };
            ranks[r].sort_by(|a, b| key(*a).total_cmp(&key(*b)).then(a.cmp(b)));
        }
        for r in (0..count.saturating_sub(1)).rev() {
            let p = place(&ranks);
            let key = |v: usize| {
                let ns: Vec<f64> = succs(v).map(|u| p[u]).collect();
                if ns.is_empty() { p[v] } else { ns.iter().sum::<f64>() / ns.len() as f64 }
            };
            ranks[r].sort_by(|a, b| key(*a).total_cmp(&key(*b)).then(a.cmp(b)));
        }
    }

    // Main axis: one band per rank, with room for the labels of the edges that cross the gap.
    let mut m_pos = vec![0.0; n];
    let mut at = 0.0;
    #[allow(clippy::needless_range_loop)] // the rank number is also compared against edge ranks
    for r in 0..count {
        let band = ranks[r].iter().map(|v| main(*v)).fold(0.0, f64::max);
        for &v in &ranks[r] {
            m_pos[v] = at + snap((band - main(v)) / 2.0);
        }
        let label = edges.iter().filter(|(a, b, _)| rank[*a].min(rank[*b]) <= r && rank[*a].max(rank[*b]) > r).map(|e| e.2).fold(0.0, f64::max);
        let gap = if dir == Direction::Right && label > 0.0 { up(label + pad, 8.0).max(RANK_GAP) } else { RANK_GAP };
        at += band + gap;
    }

    // Cross axis: aim each box at the middle of its predecessors, then push apart.
    let mut c_pos = vec![0.0; n];
    for r in 0..count {
        let mut cursor = f64::NEG_INFINITY;
        let mut wanted: Vec<Option<f64>> = Vec::new();
        for &v in &ranks[r] {
            let ps: Vec<f64> = preds(v).map(|u| c_pos[u] + anchor[u]).collect();
            let want = (!ps.is_empty()).then(|| ps.iter().sum::<f64>() / ps.len() as f64 - anchor[v]);
            let start = if cursor == f64::NEG_INFINITY { want.unwrap_or(0.0) } else { want.unwrap_or(cursor).max(cursor) };
            c_pos[v] = start;
            cursor = start + cross(v) + GAP;
            wanted.push(want);
        }
        // Packing pushes later boxes down; share the displacement so the rank stays centred.
        let drift: Vec<f64> = ranks[r].iter().zip(&wanted).filter_map(|(v, w)| w.map(|w| w - c_pos[*v])).collect();
        let shift = if drift.is_empty() {
            let total = cursor - GAP - c_pos[ranks[r][0]];
            -total / 2.0 - c_pos[ranks[r][0]]
        } else {
            drift.iter().sum::<f64>() / drift.len() as f64
        };
        for &v in &ranks[r] {
            c_pos[v] = snap(c_pos[v] + shift);
        }
    }

    let (m0, c0) = (m_pos.iter().copied().fold(f64::INFINITY, f64::min), c_pos.iter().copied().fold(f64::INFINITY, f64::min));
    let m1 = (0..n).map(|v| m_pos[v] + main(v)).fold(0.0, f64::max) - m0;
    let c1 = (0..n).map(|v| c_pos[v] + cross(v)).fold(f64::NEG_INFINITY, f64::max) - c0;
    let pts = (0..n).map(|v| if dir == Direction::Right { Point::new(m_pos[v] - m0, c_pos[v] - c0) } else { Point::new(c_pos[v] - c0, m_pos[v] - m0) }).collect();
    (pts, if dir == Direction::Right { (m1, c1) } else { (c1, m1) })
}

/// A single row (or column) in the given order.
fn row(sizes: &[(f64, f64)], gap: f64, dir: Direction) -> (Vec<Point>, (f64, f64)) {
    let cross = sizes.iter().map(|s| if dir == Direction::Right { s.1 } else { s.0 }).fold(0.0, f64::max);
    let mut at = 0.0;
    let mut pts = Vec::new();
    for s in sizes {
        let (m, c) = if dir == Direction::Right { (s.0, s.1) } else { (s.1, s.0) };
        let off = snap((cross - c) / 2.0);
        pts.push(if dir == Direction::Right { Point::new(at, off) } else { Point::new(off, at) });
        at += m + gap;
    }
    let main = (at - gap).max(0.0);
    (pts, if dir == Direction::Right { (main, cross) } else { (cross, main) })
}

struct Build<'a> {
    spec: &'a Spec,
    m: &'a Measured,
    t: &'a Tuning,
    /// Footprint of every node and group, and children placed relative to a group's frame.
    node_foot: Vec<Option<Foot>>,
    group_foot: Vec<Option<Foot>>,
    group_size: Vec<(f64, f64)>,
    group_kids: Vec<Vec<(Ref, Point)>>,
    /// Across the flow, where edges meet each footprint.
    node_anchor: Vec<f64>,
    group_anchor: Vec<f64>,
}

impl Build<'_> {
    fn size(&mut self, r: Ref) -> (f64, f64) {
        match r {
            Ref::Node(i) => {
                let f = foot(self.spec, self.m, r, self.m.nodes[i].w, self.m.nodes[i].h);
                let s = (f.w, f.h);
                self.node_anchor[i] = if self.spec.direction == Direction::Right { f.oy + self.m.nodes[i].h / 2.0 } else { f.ox + self.m.nodes[i].w / 2.0 };
                self.node_foot[i] = Some(f);
                s
            }
            Ref::Group(g) => {
                let kids = children(self.spec, Some(g));
                let banded = self.spec.diagram == Diagram::Layers && self.spec.group_parent[g].is_none();
                let (pts, (cw, ch)) = self.arrange(&kids, banded);
                let labelled = self.spec.groups[g].label.is_some();
                let top = if labelled { PAD_LABEL } else { PAD };
                let w = up((cw + 2.0 * PAD).max(self.m.group_label[g] + 32.0), 16.0);
                let h = up(ch + top + PAD, 16.0);
                let dx = snap((w - cw) / 2.0);
                self.group_kids[g] = kids.iter().zip(pts).map(|(k, p)| (*k, Point::new(p.x + dx, p.y + top))).collect();
                self.group_size[g] = (w, h);
                let f = foot(self.spec, self.m, r, w, h);
                let s = (f.w, f.h);
                // Edges that leave the group meet it at the children they belong to.
                let right = self.spec.direction == Direction::Right;
                let outward: Vec<f64> = self.group_kids[g]
                    .iter()
                    .filter(|(k, _)| self.crosses(*k, g))
                    .map(|(k, p)| (if right { p.y + f.oy } else { p.x + f.ox }) + self.anchor(*k))
                    .collect();
                self.group_anchor[g] = if outward.is_empty() { if right { f.oy + h / 2.0 } else { f.ox + w / 2.0 } } else { outward.iter().sum::<f64>() / outward.len() as f64 };
                self.group_foot[g] = Some(f);
                s
            }
            Ref::Edge(_) => (0.0, 0.0),
        }
    }

    fn anchor(&self, r: Ref) -> f64 {
        match r {
            Ref::Node(i) => self.node_anchor[i],
            Ref::Group(g) => self.group_anchor[g],
            Ref::Edge(_) => 0.0,
        }
    }

    /// True when an edge joins something at or under `kid` to something outside group `g`.
    fn crosses(&self, kid: Ref, g: usize) -> bool {
        let under = |id: &str, top: Ref| self.spec.lookup(id).is_some_and(|r| r == top || matches!(top, Ref::Group(t) if self.spec.ancestors(r).contains(&t)));
        self.spec.edges.iter().any(|e| (under(&e.from, kid) && !under(&e.to, Ref::Group(g))) || (under(&e.to, kid) && !under(&e.from, Ref::Group(g))))
    }

    fn arrange(&mut self, kids: &[Ref], as_row: bool) -> (Vec<Point>, (f64, f64)) {
        let sizes: Vec<(f64, f64)> = kids.iter().map(|k| self.size(*k)).collect();
        let anchors: Vec<f64> = kids.iter().map(|k| self.anchor(*k)).collect();
        let edges = level_edges(self.spec, self.m, kids);
        if as_row {
            let label = edges.iter().map(|e| e.2).fold(0.0, f64::max);
            let gap = if self.spec.direction == Direction::Right { up(label + self.t.label_pad + 8.0, 8.0).max(48.0) } else { 48.0 };
            row(&sizes, gap, self.spec.direction)
        } else {
            layered(&sizes, &anchors, &edges, self.spec.direction, self.t.label_pad)
        }
    }

    fn place(&self, r: Ref, origin: Point, out: &mut Geometry) {
        let (f, w, h) = match r {
            Ref::Node(i) => (self.node_foot[i].as_ref().unwrap(), self.m.nodes[i].w, self.m.nodes[i].h),
            Ref::Group(g) => (self.group_foot[g].as_ref().unwrap(), self.group_size[g].0, self.group_size[g].1),
            Ref::Edge(_) => return,
        };
        let own = Point::new(origin.x + f.ox, origin.y + f.oy);
        for (i, rel) in &f.notes {
            out.notes[*i] = Some(*rel + own.to_vec2());
        }
        match r {
            Ref::Node(i) => out.nodes[i] = Rect::new(own.x, own.y, own.x + w, own.y + h),
            Ref::Group(g) => {
                out.groups[g] = Rect::new(own.x, own.y, own.x + w, own.y + h);
                for (k, p) in &self.group_kids[g] {
                    self.place(*k, Point::new(own.x + p.x, own.y + p.y), out);
                }
            }
            Ref::Edge(_) => {}
        }
    }
}

pub fn layout(spec: &Spec, m: &Measured, t: &Tuning) -> Geometry {
    let mut out = Geometry { nodes: vec![Rect::ZERO; spec.nodes.len()], groups: vec![Rect::ZERO; spec.groups.len()], notes: vec![None; spec.notes.len()] };
    let mut b = Build {
        spec,
        m,
        t,
        node_foot: (0..spec.nodes.len()).map(|_| None).collect(),
        group_foot: (0..spec.groups.len()).map(|_| None).collect(),
        group_size: vec![(0.0, 0.0); spec.groups.len()],
        group_kids: vec![Vec::new(); spec.groups.len()],
        node_anchor: vec![0.0; spec.nodes.len()],
        group_anchor: vec![0.0; spec.groups.len()],
    };

    if spec.diagram == Diagram::Free {
        // Pinned boxes, snapped to the grid; frames wrap their members; notes sit beside their target.
        for (i, n) in spec.nodes.iter().enumerate() {
            let (x, y) = n.at.map(|(x, y)| (snap(x), snap(y))).unwrap_or((0.0, 0.0));
            out.nodes[i] = Rect::new(x, y, x + m.nodes[i].w, y + m.nodes[i].h);
        }
        let mut order: Vec<usize> = (0..spec.groups.len()).collect();
        order.sort_by_key(|g| std::cmp::Reverse(spec.ancestors(Ref::Group(*g)).len()));
        for g in order {
            let mut frame: Option<Rect> = None;
            for k in children(spec, Some(g)) {
                let r = match k {
                    Ref::Node(i) => out.nodes[i],
                    Ref::Group(i) => out.groups[i],
                    Ref::Edge(_) => continue,
                };
                frame = Some(frame.map_or(r, |f| f.union(r)));
            }
            if let Some(f) = frame {
                let top = if spec.groups[g].label.is_some() { PAD_LABEL } else { PAD };
                let need = up(m.group_label[g] + 32.0, 16.0);
                let grow = ((need - (f.width() + 2.0 * PAD)).max(0.0) / 2.0 / 8.0).ceil() * 8.0;
                out.groups[g] = Rect::new(f.x0 - PAD - grow, f.y0 - top, f.x1 + PAD + grow, f.y1 + PAD);
            }
        }
        for (r, own) in (0..spec.nodes.len()).map(|i| (Ref::Node(i), out.nodes[i])).chain((0..spec.groups.len()).map(|g| (Ref::Group(g), out.groups[g]))).collect::<Vec<_>>() {
            let f = foot(spec, m, r, own.width(), own.height());
            for (i, rel) in f.notes {
                out.notes[i] = Some(rel + own.origin().to_vec2());
            }
        }
        return out;
    }

    let kids = children(spec, None);
    if spec.diagram == Diagram::Layers {
        // Top-level groups become bands of one common length, stacked across the flow.
        // Nodes outside any group share a first band without a frame.
        let loose: Vec<Ref> = kids.iter().copied().filter(|k| matches!(k, Ref::Node(_))).collect();
        let bands: Vec<Ref> = kids.iter().copied().filter(|k| matches!(k, Ref::Group(_))).collect();
        let right = spec.direction == Direction::Right;
        let mut cursor = 0.0;
        type Band = (Vec<Ref>, Vec<Point>, (f64, f64));
        let mut rows: Vec<Band> = Vec::new();
        if !loose.is_empty() {
            let (pts, size) = b.arrange(&loose, true);
            rows.push((loose, pts, size));
        }
        for g in &bands {
            let size = b.size(*g);
            rows.push((vec![*g], vec![Point::ZERO], size));
        }
        let length = rows.iter().map(|r| if right { r.2.0 } else { r.2.1 }).fold(0.0, f64::max);
        // Stretch every band frame to the common length and re-centre what it holds.
        for g in &bands {
            let Ref::Group(gi) = *g else { continue };
            let f = b.group_foot[gi].as_ref().unwrap();
            let extra = length - if right { f.w } else { f.h };
            if extra > 0.0 {
                let (w, h) = b.group_size[gi];
                b.group_size[gi] = if right { (w + extra, h) } else { (w, h + extra) };
                let half = snap(extra / 2.0);
                for k in b.group_kids[gi].iter_mut() {
                    k.1 = if right { Point::new(k.1.x + half, k.1.y) } else { Point::new(k.1.x, k.1.y + half) };
                }
                let f = b.group_foot[gi].as_mut().unwrap();
                if right { f.w += extra } else { f.h += extra }
            }
        }
        for (items, pts, size) in &rows {
            let (len, thick) = if right { (size.0, size.1) } else { (size.1, size.0) };
            let lead = if matches!(items[0], Ref::Group(_)) { 0.0 } else { snap((length - len) / 2.0) };
            for (k, p) in items.iter().zip(pts) {
                let o = if right { Point::new(p.x + lead, p.y + cursor) } else { Point::new(p.x + cursor, p.y + lead) };
                b.place(*k, o, &mut out);
            }
            cursor += thick + RANK_GAP;
        }
        return out;
    }

    let (pts, _) = b.arrange(&kids, false);
    for (k, p) in kids.iter().zip(pts) {
        b.place(*k, p, &mut out);
    }
    out
}

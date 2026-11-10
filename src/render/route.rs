//! Orthogonal edge routing: A* over a uniform grid, with boxes as obstacles and
//! penalties that keep edges from sharing a track.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use kurbo::{Point, Rect};

pub const STEP: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    E = 0,
    S = 1,
    W = 2,
    N = 3,
}

impl Dir {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
            Dir::N => (0, -1),
        }
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::E => Dir::W,
            Dir::S => Dir::N,
            Dir::W => Dir::E,
            Dir::N => Dir::S,
        }
    }
    fn from(i: usize) -> Dir {
        [Dir::E, Dir::S, Dir::W, Dir::N][i % 4]
    }
    pub fn vec(self) -> kurbo::Vec2 {
        let (dx, dy) = self.delta();
        kurbo::Vec2::new(dx as f64, dy as f64)
    }
}

const COST_STEP: u32 = 10;
const COST_BEND: u32 = 45;
const COST_CROSS: u32 = 30;
const COST_SHARE: u32 = 400;
const COST_BORDER: u32 = 60;

pub struct Grid {
    origin: Point,
    cols: i32,
    rows: i32,
    hard: Vec<bool>,
    soft: Vec<u32>,
    /// Points on a frame's outline: fine to cross, poor to run along.
    border: Vec<bool>,
    /// Unit segments already taken: from a point to its east and south neighbours.
    east: Vec<bool>,
    south: Vec<bool>,
}

impl Grid {
    /// A grid whose points cover `bounds`; `origin` fixes where the grid lines fall.
    pub fn new(bounds: Rect, anchor: Point) -> Grid {
        let ox = anchor.x + ((bounds.x0 - anchor.x) / STEP).floor() * STEP;
        let oy = anchor.y + ((bounds.y0 - anchor.y) / STEP).floor() * STEP;
        let cols = ((bounds.x1 - ox) / STEP).ceil() as i32 + 1;
        let rows = ((bounds.y1 - oy) / STEP).ceil() as i32 + 1;
        let n = (cols * rows) as usize;
        Grid { origin: Point::new(ox, oy), cols, rows, hard: vec![false; n], soft: vec![0; n], border: vec![false; n], east: vec![false; n], south: vec![false; n] }
    }

    fn idx(&self, i: i32, j: i32) -> usize {
        (j * self.cols + i) as usize
    }
    fn inside(&self, i: i32, j: i32) -> bool {
        i >= 0 && j >= 0 && i < self.cols && j < self.rows
    }
    pub fn point(&self, i: i32, j: i32) -> Point {
        Point::new(self.origin.x + i as f64 * STEP, self.origin.y + j as f64 * STEP)
    }
    pub fn cell(&self, p: Point) -> (i32, i32) {
        (((p.x - self.origin.x) / STEP).round() as i32, ((p.y - self.origin.y) / STEP).round() as i32)
    }

    fn each_in(&self, r: Rect, mut f: impl FnMut(usize)) {
        let eps = 1e-6;
        let i0 = (((r.x0 - self.origin.x) / STEP) + eps).floor() as i32 + 1;
        let i1 = (((r.x1 - self.origin.x) / STEP) - eps).ceil() as i32 - 1;
        let j0 = (((r.y0 - self.origin.y) / STEP) + eps).floor() as i32 + 1;
        let j1 = (((r.y1 - self.origin.y) / STEP) - eps).ceil() as i32 - 1;
        for j in j0.max(0)..=j1.min(self.rows - 1) {
            for i in i0.max(0)..=i1.min(self.cols - 1) {
                f(self.idx(i, j));
            }
        }
    }

    /// No route may pass strictly inside `r`.
    pub fn block(&mut self, r: Rect) {
        let mut hits = Vec::new();
        self.each_in(r, |k| hits.push(k));
        for k in hits {
            self.hard[k] = true;
        }
    }

    /// Running along the outline of `r` costs extra at every step.
    pub fn outline(&mut self, r: Rect) {
        let ((i0, j0), (i1, j1)) = (self.cell(Point::new(r.x0, r.y0)), self.cell(Point::new(r.x1, r.y1)));
        for i in i0..=i1 {
            for j in [j0, j1] {
                if self.inside(i, j) {
                    let k = self.idx(i, j);
                    self.border[k] = true;
                }
            }
        }
        for j in j0..=j1 {
            for i in [i0, i1] {
                if self.inside(i, j) {
                    let k = self.idx(i, j);
                    self.border[k] = true;
                }
            }
        }
    }

    /// Passing inside `r` costs extra.
    pub fn penalise(&mut self, r: Rect, cost: u32) {
        let mut hits = Vec::new();
        self.each_in(r, |k| hits.push(k));
        for k in hits {
            self.soft[k] += cost;
        }
    }

    fn segment_used(&self, i: i32, j: i32, d: Dir) -> bool {
        match d {
            Dir::E => self.east[self.idx(i, j)],
            Dir::W => i > 0 && self.east[self.idx(i - 1, j)],
            Dir::S => self.south[self.idx(i, j)],
            Dir::N => j > 0 && self.south[self.idx(i, j - 1)],
        }
    }

    /// True when a route already runs through the point along the given axis.
    fn runs(&self, i: i32, j: i32, horizontal: bool) -> bool {
        if horizontal { self.segment_used(i, j, Dir::E) || self.segment_used(i, j, Dir::W) } else { self.segment_used(i, j, Dir::S) || self.segment_used(i, j, Dir::N) }
    }

    /// Route from `from`, leaving in `out`, to `to`, arriving while travelling in `arrive`.
    /// `avoid` lists extra areas this one route must stay out of. The result is marked
    /// as taken so later routes keep clear of it.
    pub fn route(&mut self, from: Point, out: Dir, to: Point, arrive: Dir, avoid: &[Rect]) -> Option<Vec<Point>> {
        let (si, sj) = self.cell(from);
        let (ti, tj) = self.cell(to);
        if !self.inside(si, sj) || !self.inside(ti, tj) {
            return None;
        }
        let avoided = |p: Point| avoid.iter().any(|r| p.x >= r.x0 - 1e-6 && p.x <= r.x1 + 1e-6 && p.y >= r.y0 - 1e-6 && p.y <= r.y1 + 1e-6);
        let n = (self.cols * self.rows) as usize * 4;
        let mut dist = vec![u32::MAX; n];
        let mut prev = vec![u32::MAX; n];
        let state = |i: i32, j: i32, d: Dir| self.idx(i, j) * 4 + d as usize;
        let h = |i: i32, j: i32| ((i - ti).unsigned_abs() + (j - tj).unsigned_abs()) * COST_STEP;
        let mut heap = BinaryHeap::new();
        let start = state(si, sj, out);
        dist[start] = 0;
        let mut seq = 0u32;
        heap.push(Reverse((h(si, sj), seq, start as u32)));
        let goal = state(ti, tj, arrive);
        while let Some(Reverse((_, _, s))) = heap.pop() {
            let s = s as usize;
            if s == goal {
                break;
            }
            let (cell, d) = (s / 4, Dir::from(s % 4));
            let (i, j) = ((cell as i32) % self.cols, (cell as i32) / self.cols);
            let base = dist[s];
            for nd in [d, Dir::from(d as usize + 1), Dir::from(d as usize + 3)] {
                let (dx, dy) = nd.delta();
                let (ni, nj) = (i + dx, j + dy);
                if !self.inside(ni, nj) {
                    continue;
                }
                let k = self.idx(ni, nj);
                let end = ni == ti && nj == tj;
                if (self.hard[k] || avoided(self.point(ni, nj))) && !end {
                    continue;
                }
                let horizontal = matches!(nd, Dir::E | Dir::W);
                let mut cost = base + COST_STEP + self.soft[k] + if self.border[k] { COST_BORDER } else { 0 };
                if nd != d {
                    cost += COST_BEND;
                }
                if self.segment_used(i, j, nd) {
                    cost += COST_SHARE;
                }
                if self.runs(ni, nj, !horizontal) && !end {
                    cost += COST_CROSS;
                }
                // Turning on top of another route hides which line goes where.
                if nd != d && (self.runs(i, j, true) || self.runs(i, j, false)) {
                    cost += COST_SHARE / 2;
                }
                let ns = state(ni, nj, nd);
                if cost < dist[ns] {
                    dist[ns] = cost;
                    prev[ns] = s as u32;
                    seq += 1;
                    heap.push(Reverse((cost + h(ni, nj), seq, ns as u32)));
                }
            }
        }
        if dist[goal] == u32::MAX {
            return None;
        }
        let mut cells = Vec::new();
        let mut s = goal;
        loop {
            let cell = s / 4;
            cells.push(((cell as i32) % self.cols, (cell as i32) / self.cols));
            if s == start {
                break;
            }
            s = prev[s] as usize;
        }
        cells.reverse();
        let mut pts = simplify(cells.iter().map(|(i, j)| self.point(*i, *j)).collect());
        // A single jog reads best half way along; the search has no reason to put it there.
        if pts.len() == 4 {
            let (a, b, c, d) = (pts[0], pts[1], pts[2], pts[3]);
            let moved = if a.y == b.y && c.y == d.y && b.x == c.x {
                let x = self.point(self.cell(Point::new((a.x + d.x) / 2.0, a.y)).0, 0).x;
                Some([a, Point::new(x, a.y), Point::new(x, d.y), d])
            } else if a.x == b.x && c.x == d.x && b.y == c.y {
                let y = self.point(0, self.cell(Point::new(a.x, (a.y + d.y) / 2.0)).1).y;
                Some([a, Point::new(a.x, y), Point::new(d.x, y), d])
            } else {
                None
            };
            if let Some(m) = moved
                && m.windows(2).all(|w| self.line_free(w[0], w[1], &avoided))
            {
                pts = m.to_vec();
            }
        }
        self.mark(&pts);
        Some(pts)
    }

    /// True when nothing blocks, penalises or already occupies the straight run a-b.
    fn line_free(&self, a: Point, b: Point, avoided: &dyn Fn(Point) -> bool) -> bool {
        let ((ai, aj), (bi, bj)) = (self.cell(a), self.cell(b));
        let steps = (bi - ai).abs().max((bj - aj).abs());
        let (dx, dy) = ((bi - ai).signum(), (bj - aj).signum());
        let d = match (dx, dy) {
            (1, 0) => Dir::E,
            (-1, 0) => Dir::W,
            (0, 1) => Dir::S,
            _ => Dir::N,
        };
        let mut on_border = 0;
        for k in 0..=steps {
            let (i, j) = (ai + dx * k, aj + dy * k);
            if !self.inside(i, j) {
                return false;
            }
            let idx = self.idx(i, j);
            // Crossing a frame touches its outline once or twice; more means running along it.
            on_border += self.border[idx] as u32;
            if on_border > 2 {
                return false;
            }
            if self.hard[idx] || self.soft[idx] > 0 || avoided(self.point(i, j)) {
                return false;
            }
            if k < steps && self.segment_used(i, j, d) {
                return false;
            }
            if k > 0 && k < steps && self.runs(i, j, !matches!(d, Dir::E | Dir::W)) {
                return false;
            }
        }
        true
    }

    /// Record a route so later ones keep off its track.
    pub fn mark(&mut self, pts: &[Point]) {
        for w in pts.windows(2) {
            let ((ai, aj), (bi, bj)) = (self.cell(w[0]), self.cell(w[1]));
            if aj == bj {
                for i in ai.min(bi)..ai.max(bi) {
                    let k = self.idx(i, aj);
                    self.east[k] = true;
                }
            } else {
                for j in aj.min(bj)..aj.max(bj) {
                    let k = self.idx(ai, j);
                    self.south[k] = true;
                }
            }
        }
    }
}

/// Drop repeated points and the middle of every straight run.
pub fn simplify(points: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    for p in points {
        if out.last().is_some_and(|q| q.distance(p) < 1e-9) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let cross = (b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x);
            let dot = (b.x - a.x) * (p.x - b.x) + (b.y - a.y) * (p.y - b.y);
            if cross.abs() < 1e-9 && dot >= 0.0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_around_a_box_and_keeps_clear_of_earlier_routes() {
        let mut g = Grid::new(Rect::new(0.0, 0.0, 240.0, 160.0), Point::ZERO);
        g.block(Rect::new(88.0, 40.0, 152.0, 120.0));
        let a = g.route(Point::new(16.0, 80.0), Dir::E, Point::new(224.0, 80.0), Dir::E, &[]).unwrap();
        assert!(a.len() >= 4, "{a:?}");
        assert!(a.iter().all(|p| !(p.x > 88.0 && p.x < 152.0 && p.y > 40.0 && p.y < 120.0)));
        assert!(a.windows(2).all(|w| w[0].x == w[1].x || w[0].y == w[1].y));
        let b = g.route(Point::new(16.0, 80.0), Dir::E, Point::new(224.0, 80.0), Dir::E, &[]).unwrap();
        assert_ne!(a, b, "a second edge should take another track");
    }

    #[test]
    fn straight_when_nothing_is_in_the_way() {
        let mut g = Grid::new(Rect::new(0.0, 0.0, 200.0, 100.0), Point::ZERO);
        let r = g.route(Point::new(16.0, 48.0), Dir::E, Point::new(160.0, 48.0), Dir::E, &[]).unwrap();
        assert_eq!(r, vec![Point::new(16.0, 48.0), Point::new(160.0, 48.0)]);
    }
}

//! The scene: every rendered element with its resolved style and geometry in viewBox units.

pub mod selector;
pub mod style;
pub mod text;

use std::collections::BTreeMap;
use std::str::FromStr;

use kurbo::{Affine, BezPath, PathEl, Point, Rect, Shape};
use roxmltree::{Document, Node};

use style::{Anchor, Baseline, Css, Paint, Style};

pub const SVG_NS: &str = "http://www.w3.org/2000/svg";
pub const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// Elements whose subtree is never painted directly.
const NOT_RENDERED: &[&str] = &[
    "defs", "clipPath", "mask", "marker", "pattern", "symbol", "linearGradient", "radialGradient", "filter",
    "style", "script", "title", "desc", "metadata", "view",
];

pub fn href<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.attribute("href").or_else(|| node.attribute((XLINK_NS, "href")))
}

pub fn line_of(doc: &Document, node: Node) -> u32 {
    doc.text_pos_at(node.range().start).row
}

pub fn parse_options<'i>() -> roxmltree::ParsingOptions<'i> {
    roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }
}

#[derive(Debug, Clone)]
pub struct TextInfo {
    pub content: String,
    /// Font size in the element's own units.
    pub font_px: f64,
    pub bold: bool,
    /// Anchor of the first line, at mid-height, in viewBox units.
    pub anchor: Point,
    /// True when the named font is not bundled or a glyph was missing.
    pub approximate: bool,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Group,
    Shape { path: BezPath, closed: bool },
    Text(TextInfo),
    Image,
    Use,
    Foreign,
}

pub struct Elem<'a, 'i> {
    pub node: Node<'a, 'i>,
    pub tag: &'a str,
    pub depth: usize,
    pub parent: Option<usize>,
    pub style: Style,
    /// Uniform scale of the element's transform.
    pub scale: f64,
    /// Transform from the element's own units to viewBox units.
    pub tf: Affine,
    pub kind: Kind,
    pub bbox: Option<Rect>,
    /// Bounding box grown by half the stroke width.
    pub vbox: Option<Rect>,
    pub connector: bool,
    /// Flattened outline in viewBox units, one polyline per subpath.
    pub outline: Vec<Vec<Point>>,
}

impl Elem<'_, '_> {
    pub fn is_leaf(&self) -> bool {
        !matches!(self.kind, Kind::Group)
    }
    pub fn closed_shape(&self) -> Option<&BezPath> {
        match &self.kind {
            Kind::Shape { path, closed: true } if !self.connector => Some(path),
            _ => None,
        }
    }
    pub fn painted(&self) -> bool {
        self.style.fill_alpha() > 0.0 || self.style.stroke_alpha() > 0.0
    }
    pub fn ends(&self) -> Option<(Point, Point)> {
        let first = *self.outline.first()?.first()?;
        let last = *self.outline.last()?.last()?;
        Some((first, last))
    }
}

pub struct Scene<'a, 'i> {
    pub doc: &'a Document<'i>,
    pub root: Node<'a, 'i>,
    /// True when the root carries no namespace, so bare names are read as SVG.
    pub lenient_ns: bool,
    pub viewbox: Option<Rect>,
    /// The viewBox, or the root's width and height when there is none.
    pub canvas: Option<Rect>,
    /// True for files this tool rendered; their geometry is exact, not heuristic.
    pub svgdog_output: bool,
    pub css: Css<'a>,
    pub ids: BTreeMap<&'a str, Vec<Node<'a, 'i>>>,
    pub elems: Vec<Elem<'a, 'i>>,
}

fn affine(value: &str) -> Option<Affine> {
    let t = svgtypes::Transform::from_str(value).ok()?;
    Some(Affine::new([t.a, t.b, t.c, t.d, t.e, t.f]))
}

fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    }
}

fn first_number(value: &str, font: f64, reference: f64) -> Option<f64> {
    let tok = value.split(|c: char| c == ',' || c.is_whitespace()).find(|t| !t.is_empty())?;
    style::length(tok, font, reference)
}

pub fn flatten(path: &BezPath) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    kurbo::flatten(path.iter(), 0.2, |el| match el {
        PathEl::MoveTo(p) => out.push(vec![p]),
        PathEl::LineTo(p) => {
            if let Some(cur) = out.last_mut() {
                cur.push(p)
            }
        }
        PathEl::ClosePath => {
            if let Some(cur) = out.last_mut()
                && let Some(&first) = cur.first()
            {
                cur.push(first)
            }
        }
        _ => {}
    });
    out
}

struct Builder<'s, 'a, 'i> {
    css: &'s Css<'a>,
    ids: &'s BTreeMap<&'a str, Vec<Node<'a, 'i>>>,
    lenient_ns: bool,
    canvas: (f64, f64),
    root: Node<'a, 'i>,
}

struct Chunk {
    x: f64,
    y: f64,
    anchor: Anchor,
    baseline: Baseline,
    parts: Vec<(String, f64, text::Class, bool, bool)>,
}

struct Layout {
    rect: Rect,
    info: TextInfo,
}

impl<'s, 'a, 'i> Builder<'s, 'a, 'i> {
    fn is_svg(&self, node: Node) -> bool {
        match node.tag_name().namespace() {
            Some(ns) => ns == SVG_NS,
            None => self.lenient_ns,
        }
    }

    fn len(&self, node: Node, name: &str, st: &Style, reference: f64) -> f64 {
        node.attribute(name).and_then(|v| style::length(v, st.font_size, reference)).unwrap_or(0.0)
    }

    fn local_path(&self, node: Node, tag: &str, st: &Style) -> Option<(BezPath, bool)> {
        let (cw, ch) = self.canvas;
        let diag = ((cw * cw + ch * ch) / 2.0).sqrt();
        match tag {
            "rect" => {
                let (x, y) = (self.len(node, "x", st, cw), self.len(node, "y", st, ch));
                let (w, h) = (self.len(node, "width", st, cw), self.len(node, "height", st, ch));
                (w > 0.0 && h > 0.0).then(|| (Rect::new(x, y, x + w, y + h).to_path(0.1), true))
            }
            "circle" => {
                let r = self.len(node, "r", st, diag);
                let c = Point::new(self.len(node, "cx", st, cw), self.len(node, "cy", st, ch));
                (r > 0.0).then(|| (kurbo::Circle::new(c, r).to_path(0.05), true))
            }
            "ellipse" => {
                let (rx, ry) = (self.len(node, "rx", st, cw), self.len(node, "ry", st, ch));
                let c = Point::new(self.len(node, "cx", st, cw), self.len(node, "cy", st, ch));
                (rx > 0.0 && ry > 0.0).then(|| (kurbo::Ellipse::new(c, (rx, ry), 0.0).to_path(0.05), true))
            }
            "line" => {
                let mut p = BezPath::new();
                p.move_to((self.len(node, "x1", st, cw), self.len(node, "y1", st, ch)));
                p.line_to((self.len(node, "x2", st, cw), self.len(node, "y2", st, ch)));
                Some((p, false))
            }
            "polyline" | "polygon" => {
                let pts: Vec<(f64, f64)> = svgtypes::PointsParser::from(node.attribute("points")?).collect();
                if pts.len() < 2 {
                    return None;
                }
                let mut p = BezPath::new();
                p.move_to(pts[0]);
                for q in &pts[1..] {
                    p.line_to(*q);
                }
                if tag == "polygon" {
                    p.close_path();
                }
                Some((p, tag == "polygon"))
            }
            "path" => {
                let p = BezPath::from_svg(node.attribute("d")?).ok()?;
                if p.elements().len() < 2 {
                    return None;
                }
                let closed = matches!(p.elements().last(), Some(PathEl::ClosePath));
                Some((p, closed))
            }
            _ => None,
        }
    }

    fn text_walk(&self, node: Node<'a, 'i>, st: &Style, chunks: &mut Vec<Chunk>, ok: &mut bool) {
        for child in node.children() {
            if child.is_text() {
                let raw = child.text().unwrap_or("");
                let mut s = String::new();
                for c in raw.chars() {
                    if c.is_whitespace() {
                        if !s.ends_with(' ') {
                            s.push(' ');
                        }
                    } else {
                        s.push(c);
                    }
                }
                if s.is_empty() {
                    continue;
                }
                let (class, exact) = text::classify(st.font_family.as_deref());
                if let Some(c) = chunks.last_mut() {
                    c.parts.push((s, st.font_size, class, st.bold, exact));
                }
            } else if child.is_element() && self.is_svg(child) {
                let tag = child.tag_name().name();
                match tag {
                    "tspan" | "a" => {
                        let cst = st.derive(&self.css.declared(child));
                        if !cst.display {
                            continue;
                        }
                        let num = |name: &str, reference: f64| child.attribute(name).and_then(|v| first_number(v, cst.font_size, reference));
                        let (x, y) = (num("x", self.canvas.0), num("y", self.canvas.1));
                        let (dx, dy) = (num("dx", self.canvas.0).unwrap_or(0.0), num("dy", self.canvas.1).unwrap_or(0.0));
                        if x.is_some() || y.is_some() || dy != 0.0 {
                            let (px, py) = chunks.last().map(|c| (c.x, c.y)).unwrap_or((0.0, 0.0));
                            chunks.push(Chunk {
                                x: x.unwrap_or(px) + dx,
                                y: y.unwrap_or(py) + dy,
                                anchor: cst.anchor,
                                baseline: cst.baseline,
                                parts: vec![],
                            });
                        } else if dx != 0.0
                            && let Some(c) = chunks.last_mut()
                        {
                            // A horizontal shift inside a line is modelled as extra advance.
                            c.parts.push((String::new(), dx, text::Class::Sans, false, true));
                        }
                        self.text_walk(child, &cst, chunks, ok);
                    }
                    "textPath" => *ok = false,
                    _ => {}
                }
            }
        }
    }

    fn layout_text(&self, node: Node<'a, 'i>, st: &Style) -> Option<Layout> {
        let num = |name: &str, reference: f64| node.attribute(name).and_then(|v| first_number(v, st.font_size, reference)).unwrap_or(0.0);
        let mut chunks = vec![Chunk {
            x: num("x", self.canvas.0) + num("dx", self.canvas.0),
            y: num("y", self.canvas.1) + num("dy", self.canvas.1),
            anchor: st.anchor,
            baseline: st.baseline,
            parts: vec![],
        }];
        let mut ok = true;
        self.text_walk(node, st, &mut chunks, &mut ok);
        if !ok {
            return None;
        }
        let mut rect: Option<Rect> = None;
        let mut anchor = None;
        let mut lines = Vec::new();
        let mut approximate = false;
        let (mut font_px, mut bold) = (0.0f64, false);
        for c in &mut chunks {
            if let Some(first) = c.parts.iter_mut().find(|p| !p.0.is_empty()) {
                first.0 = first.0.trim_start().to_string();
            }
            if let Some(last) = c.parts.iter_mut().rev().find(|p| !p.0.is_empty()) {
                last.0 = last.0.trim_end().to_string();
            }
            let line: String = c.parts.iter().map(|p| p.0.as_str()).collect();
            if line.trim().is_empty() {
                continue;
            }
            let (mut width, mut size, mut asc, mut desc) = (0.0, 0.0f64, 0.0f64, 0.0f64);
            for (s, px, class, b, exact) in &c.parts {
                if s.is_empty() {
                    width += px; // dx shift
                    continue;
                }
                let m = text::measure(s, *class, *b);
                width += m.width * px;
                approximate |= m.missing || !exact;
                let (a, d) = text::vmetrics(*class, *b);
                asc = asc.max(a * px);
                desc = desc.max(d * px);
                if *px > size {
                    size = *px;
                }
                if *px >= font_px {
                    font_px = *px;
                    bold = *b;
                }
            }
            let x0 = match c.anchor {
                Anchor::Start => c.x,
                Anchor::Middle => c.x - width / 2.0,
                Anchor::End => c.x - width,
            };
            let (top, bottom) = match c.baseline {
                Baseline::Alphabetic => (c.y - asc, c.y + desc),
                Baseline::Middle => (c.y - (asc + desc) / 2.0, c.y + (asc + desc) / 2.0),
                Baseline::Hanging => (c.y, c.y + asc + desc),
            };
            let r = Rect::new(x0, top, x0 + width, bottom);
            if anchor.is_none() {
                anchor = Some(Point::new(c.x, (top + bottom) / 2.0));
            }
            rect = union(rect, Some(r));
            lines.push(line);
        }
        Some(Layout { rect: rect?, info: TextInfo { content: lines.join("\n"), font_px, bold, anchor: anchor?, approximate } })
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(&self, node: Node<'a, 'i>, parent_style: &Style, parent_tf: Affine, depth: usize, parent: Option<usize>, out: &mut Vec<Elem<'a, 'i>>, uses: usize) {
        if !node.is_element() || !self.is_svg(node) {
            return;
        }
        let tag = node.tag_name().name();
        if NOT_RENDERED.contains(&tag) && !(tag == "symbol" && uses > 0) {
            return;
        }
        let st = parent_style.derive(&self.css.declared(node));
        if !st.display {
            return;
        }
        let mut tf = parent_tf;
        if let Some(t) = node.attribute("transform").and_then(affine) {
            tf *= t;
        }
        let is_root = node == self.root;
        let (cw, ch) = self.canvas;
        let scale = tf.determinant().abs().sqrt();
        let idx = out.len();
        let mut elem = Elem { node, tag, depth, parent, style: st.clone(), scale, tf, kind: Kind::Group, bbox: None, vbox: None, connector: false, outline: vec![] };
        let hidden = !st.visible || st.opacity <= 0.0;
        match tag {
            "svg" | "g" | "a" | "switch" | "symbol" => {
                if tag == "svg" && !is_root {
                    let (x, y) = (self.len(node, "x", &st, cw), self.len(node, "y", &st, ch));
                    tf *= Affine::translate((x, y));
                    if let Some(vb) = node.attribute("viewBox").and_then(|v| svgtypes::ViewBox::from_str(v).ok()) {
                        let (w, h) = (self.len(node, "width", &st, cw), self.len(node, "height", &st, ch));
                        if w > 0.0 && h > 0.0 && vb.w > 0.0 && vb.h > 0.0 {
                            let s = (w / vb.w).min(h / vb.h);
                            tf = tf * Affine::scale(s) * Affine::translate((-vb.x, -vb.y));
                        }
                    }
                }
                out.push(elem);
                for child in node.children() {
                    self.visit(child, &st, tf, depth + 1, Some(idx), out, uses);
                }
                let (mut b, mut v) = (None, None);
                for e in &out[idx + 1..] {
                    if e.parent == Some(idx) {
                        b = union(b, e.bbox);
                        v = union(v, e.vbox);
                    }
                }
                out[idx].bbox = b;
                out[idx].vbox = v;
            }
            "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" | "path" => {
                if hidden {
                    return;
                }
                let Some((local, closed)) = self.local_path(node, tag, &st) else { return };
                let path = tf * local;
                let b = path.bounding_box();
                elem.bbox = Some(b);
                let grow = if st.stroke_alpha() > 0.0 { st.stroke_width * scale / 2.0 } else { 0.0 };
                elem.vbox = Some(b.inflate(grow, grow));
                elem.outline = flatten(&path);
                elem.connector = !closed && st.stroke_alpha() > 0.0 && (st.marker_start.is_some() || st.marker_end.is_some());
                elem.kind = Kind::Shape { path, closed };
                out.push(elem);
            }
            "text" => {
                if hidden {
                    return;
                }
                elem.kind = Kind::Text(TextInfo { content: String::new(), font_px: st.font_size, bold: st.bold, anchor: Point::ZERO, approximate: true });
                if let Some(l) = self.layout_text(node, &st) {
                    let b = tf.transform_rect_bbox(l.rect);
                    elem.bbox = Some(b);
                    elem.vbox = Some(b);
                    let mut info = l.info;
                    info.anchor = tf * info.anchor;
                    elem.kind = Kind::Text(info);
                }
                out.push(elem);
            }
            "image" | "foreignObject" => {
                if hidden {
                    return;
                }
                let (x, y) = (self.len(node, "x", &st, cw), self.len(node, "y", &st, ch));
                let (w, h) = (self.len(node, "width", &st, cw), self.len(node, "height", &st, ch));
                if w > 0.0 && h > 0.0 {
                    let b = tf.transform_rect_bbox(Rect::new(x, y, x + w, y + h));
                    elem.bbox = Some(b);
                    elem.vbox = Some(b);
                }
                elem.kind = if tag == "image" { Kind::Image } else { Kind::Foreign };
                out.push(elem);
            }
            "use" => {
                if hidden {
                    return;
                }
                elem.kind = Kind::Use;
                if uses < 6
                    && let Some(id) = href(node).and_then(|h| h.strip_prefix('#'))
                    && let Some(target) = self.ids.get(id).and_then(|v| v.first())
                    && !target.ancestors().any(|a| a == node)
                {
                    let (x, y) = (self.len(node, "x", &st, cw), self.len(node, "y", &st, ch));
                    let mut tmp = Vec::new();
                    self.visit(*target, &st, tf * Affine::translate((x, y)), 0, None, &mut tmp, uses + 1);
                    for e in tmp.iter().filter(|e| e.is_leaf()) {
                        elem.bbox = union(elem.bbox, e.bbox);
                        elem.vbox = union(elem.vbox, e.vbox);
                    }
                }
                out.push(elem);
            }
            _ => {}
        }
    }
}

impl<'a, 'i> Scene<'a, 'i> {
    pub fn build(doc: &'a Document<'i>) -> Scene<'a, 'i> {
        let root = doc.root_element();
        let lenient_ns = root.tag_name().namespace().is_none();
        let mut ids: BTreeMap<&'a str, Vec<Node<'a, 'i>>> = BTreeMap::new();
        let mut styles: Vec<&'a str> = Vec::new();
        for n in doc.descendants().filter(|n| n.is_element()) {
            if let Some(id) = n.attribute("id") {
                ids.entry(id).or_default().push(n);
            }
            if n.tag_name().name() == "style" {
                for t in n.children().filter(|c| c.is_text()) {
                    if let Some(text) = t.text() {
                        styles.push(text);
                    }
                }
            }
        }
        let css = Css::new(&styles);
        let viewbox = root
            .attribute("viewBox")
            .and_then(|v| svgtypes::ViewBox::from_str(v).ok())
            .filter(|vb| vb.w > 0.0 && vb.h > 0.0)
            .map(|vb| Rect::new(vb.x, vb.y, vb.x + vb.w, vb.y + vb.h));
        let canvas = viewbox.or_else(|| {
            let w = root.attribute("width").filter(|v| !v.trim().ends_with('%')).and_then(|v| style::length(v, 16.0, 0.0))?;
            let h = root.attribute("height").filter(|v| !v.trim().ends_with('%')).and_then(|v| style::length(v, 16.0, 0.0))?;
            (w > 0.0 && h > 0.0).then(|| Rect::new(0.0, 0.0, w, h))
        });
        let mut elems = Vec::new();
        {
            let b = Builder { css: &css, ids: &ids, lenient_ns, canvas: canvas.map(|c| (c.width(), c.height())).unwrap_or((0.0, 0.0)), root };
            b.visit(root, &Style::default(), Affine::IDENTITY, 0, None, &mut elems, 0);
        }
        let svgdog_output = root.attribute("data-svgdog").is_some();
        if svgdog_output {
            for e in elems.iter_mut() {
                if let Kind::Shape { closed: false, .. } = e.kind
                    && e.style.stroke_alpha() > 0.0
                    && e.node.ancestors().any(|a| a.attribute("id").is_some_and(|id| id.starts_with("edge-")))
                {
                    e.connector = true;
                }
            }
        }
        Scene { doc, root, lenient_ns, viewbox, canvas, svgdog_output, css, ids, elems }
    }

    pub fn is_svg(&self, node: Node) -> bool {
        match node.tag_name().namespace() {
            Some(ns) => ns == SVG_NS,
            None => self.lenient_ns,
        }
    }

    pub fn line(&self, node: Node) -> u32 {
        line_of(self.doc, node)
    }

    /// The selector for an element. Only the first element with a given id is addressed by it.
    pub fn target(&self, node: Node) -> String {
        selector::target_with(node, &|n| n.attribute("id").and_then(|id| self.ids.get(id)).is_some_and(|v| v.first() == Some(&n)))
    }

    /// Factor from viewBox units to px at the page width the diagram is shown at.
    pub fn view_scale(&self, render_width: f64) -> f64 {
        match self.canvas {
            Some(c) if c.width() > 0.0 => render_width / c.width(),
            _ => 1.0,
        }
    }

    /// A filled shape that covers nearly the whole canvas.
    pub fn is_background(&self, e: &Elem) -> bool {
        let (Some(c), Some(b), Some(_)) = (self.canvas, e.bbox, e.closed_shape()) else { return false };
        let i = c.intersect(b);
        i.width() > 0.0 && i.height() > 0.0 && i.area() >= 0.95 * c.area()
    }

    /// Fill colour of a paint, when it is a plain colour.
    pub fn fill_color(e: &Elem) -> Option<crate::util::Rgba> {
        match &e.style.fill {
            Paint::Color(c) => Some(*c),
            _ => None,
        }
    }
}

/// Smallest distance from a point to any outline segment.
pub fn distance_to(outline: &[Vec<Point>], p: Point) -> f64 {
    let mut best = f64::INFINITY;
    for poly in outline {
        if poly.len() == 1 {
            best = best.min(poly[0].distance(p));
        }
        for w in poly.windows(2) {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let len2 = ab.hypot2();
            let t = if len2 == 0.0 { 0.0 } else { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) };
            best = best.min((a + ab * t).distance(p));
        }
    }
    best
}

/// True when the segment a-b touches the rectangle.
pub fn segment_hits_rect(a: Point, b: Point, r: Rect) -> bool {
    // Liang-Barsky clipping.
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.x - r.x0), (dx, r.x1 - a.x), (-dy, a.y - r.y0), (dy, r.y1 - a.y)] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

pub fn contains_point(path: &BezPath, p: Point) -> bool {
    path.contains(p)
}

/// The point on an outline closest to `p`.
pub fn nearest_on(outline: &[Vec<Point>], p: Point) -> Point {
    let mut best = (f64::INFINITY, p);
    for poly in outline {
        if poly.len() == 1 && poly[0].distance(p) < best.0 {
            best = (poly[0].distance(p), poly[0]);
        }
        for w in poly.windows(2) {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let len2 = ab.hypot2();
            let t = if len2 == 0.0 { 0.0 } else { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) };
            let q = a + ab * t;
            if q.distance(p) < best.0 {
                best = (q.distance(p), q);
            }
        }
    }
    best.1
}

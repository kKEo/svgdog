//! Rasterizing with the embedded renderer. Only the bundled fonts are loaded, so
//! the same SVG gives the same PNG on every machine.


use kurbo::Rect;
use resvg::{tiny_skia, usvg};

use crate::envelope::{AppError, Finding, Severity};
use crate::svg::text::{self, Class};
use crate::util::{Rgba, fmt};

pub const MAX_WIDTH: u32 = 2000;
pub const MAX_HEIGHT: u32 = 4000;
pub const DEFAULT_WIDTH: u32 = 1200;

pub fn options() -> usvg::Options<'static> {
    let mut opt = usvg::Options::default();
    let db = opt.fontdb_mut();
    for data in text::bundled() {
        db.load_font_data(data.to_vec());
    }
    // Every generic family, and any font the file names but we do not have, falls back to a bundled face.
    db.set_sans_serif_family("Inter");
    db.set_serif_family("Inter");
    db.set_cursive_family("Inter");
    db.set_fantasy_family("Inter");
    db.set_monospace_family("JetBrains Mono");
    opt.font_family = "Inter".into();
    opt
}

pub struct Shot {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// The part of the drawing shown, in viewBox units.
    pub region: Rect,
    /// Output pixels per viewBox unit.
    pub scale: f64,
    /// Finding boxes drawn on the image.
    pub annotated: usize,
}

fn not_renderable(e: impl std::fmt::Display) -> AppError {
    AppError::input("not-renderable", format!("The file cannot be rendered: {e}."))
}

fn colour(s: Severity) -> &'static str {
    match s {
        Severity::Error => "#dc2626",
        Severity::Warning => "#b45309",
        Severity::Info => "#2563eb",
    }
}

/// An SVG, in output pixels, that outlines each finding's box and names its rules.
fn overlay(findings: &[Finding], region: Rect, scale: f64, width: u32, height: u32) -> (String, usize) {
    // Findings on one box share an outline and a label.
    let mut boxes: Vec<(Rect, Severity, Vec<&str>)> = Vec::new();
    let mut loose: Vec<&str> = Vec::new();
    for f in findings {
        let Some(b) = &f.bbox else {
            if !loose.contains(&f.rule) {
                loose.push(f.rule);
            }
            continue;
        };
        let n: Vec<f64> = b.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
        let r = Rect::new((n[0] - region.x0) * scale, (n[1] - region.y0) * scale, (n[0] + n[2] - region.x0) * scale, (n[1] + n[3] - region.y0) * scale);
        if r.x1 < 0.0 || r.y1 < 0.0 || r.x0 > width as f64 || r.y0 > height as f64 {
            continue;
        }
        match boxes.iter_mut().find(|(o, _, _)| (o.x0 - r.x0).abs() < 0.5 && (o.y0 - r.y0).abs() < 0.5 && (o.x1 - r.x1).abs() < 0.5 && (o.y1 - r.y1).abs() < 0.5) {
            Some((_, sev, rules)) => {
                *sev = (*sev).max(f.severity);
                if !rules.contains(&f.rule) {
                    rules.push(f.rule);
                }
            }
            None => boxes.push((r, f.severity, vec![f.rule])),
        }
    }
    let (w, h) = (width as f64, height as f64);
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" font-family=\"Inter\" font-size=\"12\">\n");
    let mut placed: Vec<Rect> = Vec::new();
    let mut tag = |out: &mut String, text: &str, x: f64, y: f64, fill: &str| {
        let tw = text::measure(text, Class::Sans, false).width * 12.0 + 8.0;
        let x = x.min(w - tw).max(0.0);
        let mut y = y.clamp(0.0, (h - 16.0).max(0.0));
        // Labels that would cover each other step down until they are clear.
        for _ in 0..12 {
            let r = Rect::new(x, y, x + tw, y + 16.0);
            if !placed.iter().any(|p| p.intersect(r).area() > 0.0) || y + 32.0 > h {
                break;
            }
            y += 16.0;
        }
        placed.push(Rect::new(x, y, x + tw, y + 16.0));
        out.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"16\" fill=\"{fill}\"/>\n", fmt(x), fmt(y), fmt(tw)));
        out.push_str(&format!("<text x=\"{}\" y=\"{}\" fill=\"#ffffff\">{}</text>\n", fmt(x + 4.0), fmt(y + 12.0), text.replace('&', "&amp;").replace('<', "&lt;")));
    };
    for (r, sev, _) in &boxes {
        // A line or a dot still needs an outline you can see.
        let r = Rect::new(r.x0.min(r.x1 - 2.0), r.y0.min(r.y1 - 2.0), r.x1.max(r.x0 + 2.0), r.y1.max(r.y0 + 2.0)).inflate(2.0, 2.0);
        out.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n", fmt(r.x0), fmt(r.y0), fmt(r.width()), fmt(r.height()), colour(*sev)));
    }
    for (r, sev, rules) in &boxes {
        tag(&mut out, &rules.join(", "), r.x0 - 3.0, r.y0 - 19.0, colour(*sev));
    }
    if !loose.is_empty() {
        tag(&mut out, &format!("whole file: {}", loose.join(", ")), 4.0, h - 20.0, "#374151");
    }
    out.push_str("</svg>\n");
    (out, boxes.len())
}

/// Render `region` of an SVG (the whole canvas when `None`) to a PNG `width` pixels wide.
/// `canvas` is the drawing's viewBox as the scene resolved it.
pub fn rasterize(svg: &str, canvas: Option<Rect>, region: Option<Rect>, width: u32, background: Rgba, annotate: Option<&[Finding]>) -> Result<Shot, AppError> {
    let opt = options();
    let tree = usvg::Tree::from_str(svg, &opt).map_err(not_renderable)?;
    let (tw, th) = (tree.size().width() as f64, tree.size().height() as f64);
    let canvas = canvas.unwrap_or(Rect::new(0.0, 0.0, tw, th));
    let region = region.unwrap_or(canvas);
    if region.width() <= 0.0 || region.height() <= 0.0 {
        return Err(not_renderable("the drawing has no size"));
    }
    let mut scale = width.min(MAX_WIDTH) as f64 / region.width();
    if region.height() * scale > MAX_HEIGHT as f64 {
        scale = MAX_HEIGHT as f64 / region.height();
    }
    let (w, h) = (((region.width() * scale).round() as u32).max(1), ((region.height() * scale).ceil() as u32).max(1));
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or_else(|| not_renderable("the image would have no pixels"))?;
    pixmap.fill(tiny_skia::Color::from_rgba8(background.r, background.g, background.b, 255));
    // viewBox units -> the renderer's own pixels -> output pixels.
    let (ux, uy) = (tw / canvas.width(), th / canvas.height());
    let tf = tiny_skia::Transform::from_row((scale / ux) as f32, 0.0, 0.0, (scale / uy) as f32, ((canvas.x0 - region.x0) * scale) as f32, ((canvas.y0 - region.y0) * scale) as f32);
    resvg::render(&tree, tf, &mut pixmap.as_mut());

    let mut annotated = 0;
    if let Some(findings) = annotate {
        let (over, n) = overlay(findings, region, scale, w, h);
        annotated = n;
        let layer = usvg::Tree::from_str(&over, &opt).map_err(not_renderable)?;
        resvg::render(&layer, tiny_skia::Transform::identity(), &mut pixmap.as_mut());
    }
    let png = pixmap.encode_png().map_err(|e| AppError::new(crate::envelope::EXIT_INTERNAL, "internal-error", format!("PNG encoding failed: {e}.")))?;
    Ok(Shot { png, width: w, height: h, region, scale, annotated })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::Scene;

    /// What is measured is what is drawn: the scene's boxes agree with the renderer's.
    #[test]
    fn scene_boxes_match_the_renderer() {
        let src = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300">
  <g id="g" transform="translate(40 20) scale(1.5)">
    <rect id="r" x="10" y="10" width="80" height="40" fill="#ddd"/>
    <circle id="c" cx="120" cy="60" r="25" fill="#ccc"/>
    <path id="p" d="M10 100 C 40 60, 80 140, 120 100 L 120 130 Z" fill="#bbb"/>
    <polygon id="d" points="150,20 190,45 150,70 110,45" fill="#aaa" transform="rotate(20 150 45)"/>
  </g>
  <text id="t" x="200" y="250" text-anchor="middle" font-family="Inter, sans-serif" font-size="20">Order service</text>
</svg>"##;
        let doc = roxmltree::Document::parse(src).unwrap();
        let scene = Scene::build(&doc);
        let tree = usvg::Tree::from_str(src, &options()).unwrap();
        for id in ["r", "c", "p", "d", "t"] {
            let mine = scene.elems.iter().find(|e| e.node.attribute("id") == Some(id)).and_then(|e| e.bbox).unwrap();
            let theirs = tree.node_by_id(id).unwrap_or_else(|| panic!("renderer lost #{id}")).abs_bounding_box();
            let (tx0, tx1) = (theirs.left() as f64, theirs.right() as f64);
            let (ty0, ty1) = (theirs.top() as f64, theirs.bottom() as f64);
            // Shapes agree to a fraction of a unit. Text agrees in width and position; the
            // renderer's box is the ink, ours the line box, so heights are only nested.
            let tol = if id == "t" { 2.5 } else { 0.3 };
            assert!((mine.x0 - tx0).abs() < tol && (mine.x1 - tx1).abs() < tol, "#{id} x: {mine:?} vs {theirs:?}");
            if id == "t" {
                assert!(mine.y0 <= ty0 + 0.5 && mine.y1 >= ty1 - 0.5, "#{id} y: {mine:?} vs {theirs:?}");
            } else {
                assert!((mine.y0 - ty0).abs() < tol && (mine.y1 - ty1).abs() < tol, "#{id} y: {mine:?} vs {theirs:?}");
            }
        }
    }

    /// The same comparison on every drawn element of every fixture and golden file:
    /// each element gets an id, so the renderer's node can be found, and the two
    /// boxes must agree. This is what stands behind using the scene's boxes in rules.
    #[test]
    fn every_fixture_element_matches_the_renderer() {
        use crate::ops::{self, Op};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let files = ["fixtures/orders.svg", "fixtures/broken.svg", "golden/orders.svg", "golden/checkout.svg", "golden/tiers.svg", "golden/rack.svg", "golden/login.svg"];
        let (mut compared, mut texts) = (0, 0);
        for f in files {
            let src = std::fs::read_to_string(root.join(f)).unwrap();
            // Give every drawn element without an id one of its own.
            let tagged = {
                let doc = roxmltree::Document::parse_with_options(&src, crate::svg::parse_options()).unwrap();
                let scene = Scene::build(&doc);
                let list: Vec<Op> = scene.elems.iter().enumerate().filter(|(_, e)| e.node.attribute("id").is_none()).map(|(i, e)| Op::SetAttr { target: scene.target(e.node), attr: "id".into(), value: format!("probe-{i}") }).collect();
                ops::apply(&src, &list).unwrap().text
            };
            let doc = roxmltree::Document::parse_with_options(&tagged, crate::svg::parse_options()).unwrap();
            let scene = Scene::build(&doc);
            let tree = usvg::Tree::from_str(&tagged, &options()).unwrap();
            for e in scene.elems.iter().filter(|e| e.is_leaf()) {
                let (Some(mine), Some(id)) = (e.bbox, e.node.attribute("id")) else { continue };
                // The duplicate id in broken.svg resolves to the first element only.
                if scene.ids.get(id).is_some_and(|v| v.len() > 1) {
                    continue;
                }
                let Some(node) = tree.node_by_id(id) else { continue };
                let t = node.abs_bounding_box();
                let (tx0, ty0, tx1, ty1) = (t.left() as f64, t.top() as f64, t.right() as f64, t.bottom() as f64);
                if matches!(e.kind, crate::svg::Kind::Text(_)) {
                    // Ours is the line box, the renderer's the ink: same extent sideways within
                    // side bearings, and the ink lies inside our box top to bottom.
                    let slack = 1.0 + 0.04 * mine.width();
                    assert!((mine.x0 - tx0).abs() < slack && (mine.x1 - tx1).abs() < slack, "{f} #{id} text x: {mine:?} vs {t:?}");
                    assert!(mine.y0 <= ty0 + 1.0 && mine.y1 >= ty1 - 1.0, "{f} #{id} text y: {mine:?} vs {t:?}");
                    texts += 1;
                } else if matches!(e.kind, crate::svg::Kind::Shape { .. }) {
                    let ok = (mine.x0 - tx0).abs() < 0.3 && (mine.y0 - ty0).abs() < 0.3 && (mine.x1 - tx1).abs() < 0.3 && (mine.y1 - ty1).abs() < 0.3;
                    assert!(ok, "{f} #{id} <{}>: {mine:?} vs {t:?}", e.tag);
                    compared += 1;
                }
            }
        }
        assert!(compared > 120 && texts > 60, "compared only {compared} shapes and {texts} texts");
    }

    #[test]
    fn width_and_height_are_bounded() {
        let tall = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 1000\"><rect width=\"100\" height=\"1000\"/></svg>";
        let s = rasterize(tall, Some(Rect::new(0.0, 0.0, 100.0, 1000.0)), None, 9000, Rgba::WHITE, None).unwrap();
        assert!(s.width <= MAX_WIDTH && s.height <= MAX_HEIGHT, "{} x {}", s.width, s.height);
        assert_eq!(&s.png[1..4], b"PNG");
    }
}

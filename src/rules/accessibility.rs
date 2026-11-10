//! Accessibility and style rules.

use serde_json::json;

use super::Ctx;
use super::geometry::container;
use crate::svg::style::Paint;
use crate::svg::{Kind, Scene};
use crate::util::{Rgba, contrast, fmt, num};

fn has_child(scene: &Scene, name: &str) -> bool {
    scene.root.children().any(|c| c.is_element() && c.tag_name().name() == name && c.text().is_some_and(|t| !t.trim().is_empty()))
}

fn nearest(palette: &[Rgba], c: &Rgba) -> Rgba {
    *palette.iter().min_by(|a, b| a.distance(c).total_cmp(&b.distance(c))).unwrap()
}

pub fn run(ctx: &mut Ctx) {
    let scene = ctx.scene;
    let elems = &scene.elems;
    let root = scene.root;
    let view = scene.view_scale(ctx.set.render_width);

    if ctx.on("title-missing") && !has_child(scene, "title") {
        ctx.push("title-missing", root, None, "The root <svg> has no <title>; it is the image's accessible name.".into(), vec![]);
    }
    if ctx.on("desc-missing") && !has_child(scene, "desc") {
        ctx.push("desc-missing", root, None, "The root <svg> has no <desc> saying what the diagram shows.".into(), vec![]);
    }

    let has_background = elems.iter().any(|e| scene.is_background(e) && e.style.fill_alpha() >= 0.5);
    let page = ctx.set.backgrounds.first().cloned().unwrap_or(("light".into(), Rgba::WHITE));
    let dark_page = ctx.set.backgrounds.iter().find(|(_, c)| c.luminance() < 0.2).cloned();
    let dark_min = ctx.set.opt("background-missing", "min_ratio");
    let mut on_canvas_dark = 0usize;
    let mut first_dark = None;

    for (i, e) in elems.iter().enumerate() {
        let (Kind::Text(t), Some(tb)) = (&e.kind, e.bbox) else { continue };
        let Paint::Color(fg) = &e.style.fill else { continue };
        let alpha = e.style.fill_alpha();
        if alpha <= 0.0 {
            continue;
        }
        let under = container(scene, i, tb.center(), true);
        let (bg, bg_name) = match under {
            Some(k) => match Scene::fill_color(&elems[k]) {
                Some(c) => (c.over(elems[k].style.fill_alpha(), &page.1), scene.target(elems[k].node)),
                None => continue, // gradient or pattern: no single colour to compare
            },
            None => (page.1, format!("{} page", page.0)),
        };
        if under.is_none()
            && let Some((_, dark)) = &dark_page
            && contrast(&fg.over(alpha, dark), dark) < dark_min
        {
            on_canvas_dark += 1;
            first_dark.get_or_insert(e.node);
        }
        if !ctx.on("low-contrast") {
            continue;
        }
        let px = t.font_px * e.scale * view;
        let large = px >= 24.0 || (t.bold && px >= 18.66);
        let min = if large { ctx.set.opt("low-contrast", "min_ratio_large") } else { ctx.set.opt("low-contrast", "min_ratio") };
        let ratio = contrast(&fg.over(alpha, &bg), &bg);
        if ratio < min - 0.005 {
            let msg = format!("Text {} on {} ({}) has contrast {}:1; the minimum is {}:1.", fg.hex(), bg.hex(), bg_name, fmt(ratio), fmt(min));
            let m = vec![("ratio", num(ratio)), ("min_ratio", num(min)), ("foreground", json!(fg.hex())), ("background", json!(bg.hex())), ("large_text", json!(large))];
            ctx.push("low-contrast", e.node, Some(tb), msg, m);
            let fallback = [Rgba::BLACK, Rgba::WHITE];
            let candidates: &[Rgba] = if ctx.set.palette.is_empty() { &fallback } else { &ctx.set.palette };
            let pick = candidates.iter().filter(|c| contrast(&c.over(alpha, &bg), &bg) >= min).min_by(|a, b| a.distance(fg).total_cmp(&b.distance(fg)));
            if let Some(c) = pick {
                let op = ctx.set_prop(e.node, "fill", json!(c.hex()));
                ctx.fix(false, vec![op]);
            }
        }
    }

    if ctx.on("background-missing")
        && !has_background
        && let Some((name, dark)) = &dark_page
    {
        let mut count = on_canvas_dark;
        for (i, e) in elems.iter().enumerate() {
            let (Kind::Shape { closed, .. }, Some(b)) = (&e.kind, e.bbox) else { continue };
            // Strokes drawn straight on the canvas: connectors, lines and unfilled frames.
            let Paint::Color(c) = &e.style.stroke else { continue };
            let alpha = e.style.stroke_alpha();
            if alpha <= 0.0 || (*closed && e.style.fill_alpha() >= 0.5) {
                continue;
            }
            if container(scene, i, b.center(), true).is_some() {
                continue;
            }
            if contrast(&c.over(alpha, dark), dark) < dark_min {
                count += 1;
                first_dark.get_or_insert(e.node);
            }
        }
        if count > 0 {
            let msg = format!("{count} dark element{} sit{} on a transparent background and will be unreadable on a {name} page ({}).", if count == 1 { "" } else { "s" }, if count == 1 { "s" } else { "" }, dark.hex());
            let first = first_dark.map(|n| scene.target(n)).unwrap_or_default();
            ctx.push("background-missing", root, scene.canvas, msg, vec![("elements", json!(count)), ("first", json!(first)), ("page", json!(dark.hex())), ("min_ratio", num(dark_min))]);
            if let Some(c) = scene.canvas {
                let light = ctx.set.backgrounds.iter().map(|(_, c)| *c).find(|c| c.luminance() >= 0.5).unwrap_or(Rgba::WHITE);
                let fill = if ctx.set.palette.is_empty() { light } else { nearest(&ctx.set.palette, &light) };
                let t = ctx.target(root);
                ctx.fix(true, vec![json!({ "op": "add-node", "target": t, "tag": "rect", "at": "start", "attrs": { "x": num(c.x0), "y": num(c.y0), "width": num(c.width()), "height": num(c.height()), "fill": fill.hex() } })]);
            }
        }
    }

    if ctx.on("palette") && !ctx.set.palette.is_empty() {
        let palette = ctx.set.palette.clone();
        for e in elems.iter().filter(|e| e.is_leaf()) {
            let mut off: Vec<(&str, Rgba)> = Vec::new();
            if let Paint::Color(c) = &e.style.fill
                && e.style.fill_alpha() > 0.0
                && !matches!(e.kind, Kind::Image | Kind::Foreign | Kind::Use)
                && !palette.iter().any(|p| p.same_rgb(c))
            {
                off.push(("fill", *c));
            }
            if let Paint::Color(c) = &e.style.stroke
                && e.style.stroke_alpha() > 0.0
                && matches!(e.kind, Kind::Shape { .. } | Kind::Text(_))
                && !palette.iter().any(|p| p.same_rgb(c))
            {
                off.push(("stroke", *c));
            }
            if off.is_empty() {
                continue;
            }
            let parts: Vec<String> = off.iter().map(|(k, c)| format!("{k} {} (nearest {})", c.hex(), nearest(&palette, c).hex())).collect();
            let mut m: Vec<(&'static str, serde_json::Value)> = Vec::new();
            for (k, c) in &off {
                if *k == "fill" {
                    m.push(("fill", json!(c.hex())));
                    m.push(("fill_nearest", json!(nearest(&palette, c).hex())));
                } else {
                    m.push(("stroke", json!(c.hex())));
                    m.push(("stroke_nearest", json!(nearest(&palette, c).hex())));
                }
            }
            ctx.push("palette", e.node, e.bbox, format!("Not in the palette: {}.", parts.join(", ")), m);
            let ops = off.iter().map(|(k, c)| ctx.set_prop(e.node, k, json!(nearest(&palette, c).hex()))).collect();
            ctx.fix(false, ops);
        }
    }

    if ctx.on("stroke-width-style") && !ctx.set.stroke_widths.is_empty() {
        let allowed = ctx.set.stroke_widths.clone();
        for e in elems.iter().filter(|e| matches!(e.kind, Kind::Shape { .. }) && e.style.stroke_alpha() > 0.0) {
            let w = e.style.stroke_width;
            if allowed.iter().any(|a| (a - w).abs() < 1e-6) {
                continue;
            }
            let near = allowed.iter().copied().min_by(|a, b| (a - w).abs().total_cmp(&(b - w).abs())).unwrap();
            let list: Vec<String> = allowed.iter().map(|a| fmt(*a)).collect();
            let msg = format!("Stroke width {} is not one of {}; the nearest is {}.", fmt(w), list.join(", "), fmt(near));
            ctx.push("stroke-width-style", e.node, e.bbox, msg, vec![("stroke_width", num(w)), ("nearest", num(near))]);
            let op = ctx.set_prop(e.node, "stroke-width", num(near));
            ctx.fix(true, vec![op]);
        }
    }
}

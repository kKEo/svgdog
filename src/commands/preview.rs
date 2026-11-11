use std::path::Path;

use serde_json::{Value, json};

use super::read_input;
use super::validate::{Paint, report};
use crate::cli::{FailOn, Page};
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_OK, EXIT_OUTPUT, Output};
use crate::rules::{self, bbox_json};
use crate::svg::selector::Selector;
use crate::svg::{self, Scene};
use crate::util::{Rgba, fmt};
use crate::preview;

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, output: &str, width: Option<u32>, page: Page, annotate: bool, crop: Option<&str>, step: Option<usize>, force: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let to_stdout = output == "-";
    if !to_stdout && !force && Path::new(output).exists() {
        return Err(AppError::new(EXIT_OUTPUT, "output-exists", format!("{output} already exists; pass --force to replace it.")).path(output).suggest(Some("Add --force, or choose a new path.".into())));
    }
    let asked = width.unwrap_or(preview::DEFAULT_WIDTH);
    if asked < 16 {
        return Err(AppError::usage("invalid-argument", "--width must be at least 16."));
    }
    let selector = match crop {
        Some(s) => Some(Selector::parse(s).map_err(|e| AppError::usage("invalid-argument", format!("Cannot read selector '{s}': {e}.")))?),
        None => None,
    };
    let src = read_input(file)?;
    let src = match step {
        Some(n) => at_step(&src, n, file)?,
        None => src,
    };
    let doc = roxmltree::Document::parse_with_options(&src, svg::parse_options()).map_err(|e| AppError::input("malformed-xml", format!("{file} is not well-formed XML: {e}.")).path(file))?;
    let scene = Scene::build(&doc);

    // --crop shows the matched elements with some of their surroundings.
    let region = match &selector {
        None => None,
        Some(sel) => {
            let hit = scene.elems.iter().filter(|e| sel.matches(e.node)).filter_map(|e| e.vbox.or(e.bbox)).reduce(|a, b| a.union(b));
            let b = hit.ok_or_else(|| AppError::input("no-match", format!("--crop '{}' matches no visible element with a box.", crop.unwrap_or(""))))?;
            let pad = (b.width().max(b.height()) * 0.25).max(24.0);
            let mut r = b.inflate(pad, pad);
            if let Some(c) = scene.canvas {
                let bound = c.union(b);
                r = Rect::new(r.x0.max(bound.x0), r.y0.max(bound.y0), r.x1.min(bound.x1), r.y1.min(bound.y1));
            }
            Some(r)
        }
    };

    let path = (file != "-").then(|| Path::new(file));
    let findings = if annotate {
        let settings = loader.for_input(path)?.settings(path, profile, &RuleSel::default());
        Some(rules::analyze(file, &src, &settings))
    } else {
        None
    };
    let background = match page {
        Page::Light => Rgba::WHITE,
        Page::Dark => Rgba { r: 0x0d, g: 0x11, b: 0x17, a: 1.0 },
    };
    let shot = preview::rasterize(&src, scene.canvas, region, asked, background, findings.as_deref())?;

    let mut r = report("preview", findings.clone().unwrap_or_default(), 1, 50, FailOn::Error, paint);
    // A preview succeeds when the image is made; findings are information here, not failure.
    r.json.insert("ok".into(), json!(true));
    let mut text = String::new();
    if !to_stdout {
        super::change::write_bytes(Path::new(output), &shot.png)?;
        r.json.insert("outputs".into(), json!([{ "path": output, "bytes": shot.png.len() }]));
        text.push_str(&format!("Wrote {output} ({} x {} px, {} bytes).\n", shot.width, shot.height, shot.png.len()));
    }
    if findings.is_none() {
        r.json.insert("validated".into(), json!(false));
    }
    r.json.insert(
        "image".into(),
        json!({
            "width": shot.width,
            "height": shot.height,
            "scale": crate::util::num(shot.scale),
            "region": bbox_json(shot.region),
            "background": if page == Page::Light { "light" } else { "dark" },
            "annotated": shot.annotated,
        }),
    );
    if asked > preview::MAX_WIDTH {
        text.push_str(&format!("--width {asked} is over the limit; the image is {} px wide.\n", shot.width));
    }
    if selector.is_some() {
        text.push_str(&format!("Shows the region {} {} {} {} of the viewBox.\n", fmt(shot.region.x0), fmt(shot.region.y0), fmt(shot.region.width()), fmt(shot.region.height())));
    }
    if findings.is_some() {
        text.push_str(&format!("{} finding box{} outlined.\n\n", shot.annotated, if shot.annotated == 1 { "" } else { "es" }));
        text.push_str(&r.text);
    }
    Ok(Output { json: Value::Object(r.json), text, exit: EXIT_OK, raw: to_stdout.then_some(shot.png) })
}

use kurbo::Rect;

/// The drawing as it looks with one step of its walk-through chosen: what the step
/// does not show is faded, as the interactive versions fade it.
fn at_step(src: &str, step: usize, file: &str) -> Result<String, AppError> {
    let doc = roxmltree::Document::parse_with_options(src, svg::parse_options()).map_err(|e| AppError::input("malformed-xml", format!("{file} is not well-formed XML: {e}.")).path(file))?;
    let mut fades: Vec<(usize, &str)> = Vec::new();
    let mut found = false;
    for n in doc.descendants().filter(|n| n.is_element()) {
        let Some(id) = n.attribute("id") else { continue };
        let after_name = n.range().start + 1 + n.tag_name().name().len();
        if let Some(k) = id.strip_prefix("step-").and_then(|k| k.parse::<usize>().ok()) {
            found |= k == step;
            if k != step {
                fades.push((after_name, " opacity=\"0.45\""));
            }
        } else if ["node-", "edge-", "group-", "note-"].iter().any(|p| id.starts_with(p)) {
            let shown = n.attribute("data-step").is_some_and(|s| s.split_whitespace().any(|t| t.parse::<usize>().ok() == Some(step)));
            if !shown {
                fades.push((after_name, " opacity=\"0.25\""));
            }
        }
    }
    if !found {
        return Err(AppError::input("no-match", format!("{file} has no step {step}.")).suggest(Some("Steps come from the spec's 'steps' list and are numbered from 1.".into())));
    }
    let mut out = src.to_string();
    fades.sort();
    for (at, attr) in fades.into_iter().rev() {
        out.insert_str(at, attr);
    }
    Ok(out)
}

/// A plain picture of an SVG at the default width, for callers that already hold the text.
pub fn picture(src: &str) -> Result<preview::Shot, AppError> {
    let doc = roxmltree::Document::parse_with_options(src, svg::parse_options()).map_err(|e| AppError::input("malformed-xml", format!("The drawing is not well-formed XML: {e}.")))?;
    let canvas = Scene::build(&doc).canvas;
    preview::rasterize(src, canvas, None, preview::DEFAULT_WIDTH, Rgba::WHITE, None)
}

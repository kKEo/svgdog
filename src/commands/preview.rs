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
pub fn run(file: &str, output: &str, width: Option<u32>, page: Page, annotate: bool, crop: Option<&str>, force: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
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

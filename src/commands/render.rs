use std::path::Path;

use serde_json::{Value, json};

use super::read_input;
use super::validate::{Paint, report};
use crate::cli::{FailOn, Live};
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, EXIT_OUTPUT, Output};
use crate::{render, rules, spec};

/// Where frame `n` of `output` goes: `fig/orders.svg` gives `fig/orders.2.svg`.
fn frame_path(output: &str, n: usize) -> String {
    let p = Path::new(output);
    match (p.file_stem().and_then(|s| s.to_str()), p.extension().and_then(|s| s.to_str())) {
        (Some(stem), Some(ext)) => p.with_file_name(format!("{stem}.{n}.{ext}")).to_string_lossy().into_owned(),
        _ => format!("{output}.{n}"),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, inline: Option<&str>, output: &str, theme: Option<&str>, live: Live, frames: bool, embed: bool, check: bool, no_validate: bool, force: bool, text_as_paths: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let to_stdout = output == "-";
    if check && to_stdout {
        return Err(AppError::usage("invalid-argument", "--check compares with a file, so it needs an output file, not '-'."));
    }
    if frames && live == Live::Html {
        return Err(AppError::usage("argument-conflict", "--frames writes pictures, one per step; the html version already steps through them. Use one or the other."));
    }
    if frames && to_stdout {
        return Err(AppError::usage("invalid-argument", "--frames writes one file per step, so it needs an output file, not '-'."));
    }
    // A tool call can hand the spec over as text; it then has no file of its own.
    let (file, text) = match inline {
        Some(t) => ("spec", t.to_string()),
        None => (file, read_input(file)?),
    };
    let doc = spec::read_document(&text, file)?;
    let parsed = spec::parse(&doc)?;
    if frames && parsed.steps.is_empty() {
        return Err(AppError::input("no-steps", format!("--frames draws one picture per step, and {file} has no 'steps'.")).path("steps").suggest(Some("Add steps to the spec, or drop --frames.".into())));
    }
    // What to draw and where it goes: the diagram itself, or one picture per step.
    let jobs: Vec<(String, spec::Spec)> = if frames { parsed.steps.iter().map(|s| (frame_path(output, s.number), parsed.frame(s.number))).collect() } else { vec![(output.to_string(), parsed)] };
    if !to_stdout && !force && !check && let Some((taken, _)) = jobs.iter().find(|(p, _)| Path::new(p).exists()) {
        return Err(AppError::new(EXIT_OUTPUT, "output-exists", format!("{taken} already exists; pass --force to replace it.")).path(taken.clone()).suggest(Some("Add --force, or choose a new path.".into())));
    }
    let path = (inline.is_none() && file != "-").then(|| Path::new(file));
    let cfg = loader.for_input(path)?;
    let settings = cfg.settings(path, profile, &RuleSel::default());

    let mut drawn = Vec::new();
    for (out, job) in &jobs {
        drawn.push((out, render::render(job, &render::Options { theme, fonts: cfg.fonts(), text_as_paths, embed: embed.then_some(&doc), interactive: live != Live::None })?));
    }
    // Findings are about the drawing; the page around it, if any, is added afterwards.
    let mut findings = Vec::new();
    if !no_validate && !check {
        for (out, svg) in &drawn {
            findings.extend(rules::analyze(if to_stdout { file } else { out.as_str() }, svg, &settings));
        }
    }
    if live == Live::Html {
        let name = theme.or(jobs[0].1.theme.as_deref()).unwrap_or("docs-light");
        let (light, dark) = if name == render::theme::AUTO { (&render::theme::THEMES[0], Some(&render::theme::THEMES[1])) } else { (render::theme::find(name).unwrap_or(&render::theme::THEMES[0]), None) };
        for ((_, svg), (_, job)) in drawn.iter_mut().zip(&jobs) {
            *svg = render::interactive::page(svg, job, light, dark);
        }
    }
    if check {
        // Compare with what is on disk; a file that is missing or different is stale.
        let stale: Vec<Value> = drawn
            .iter()
            .filter_map(|(out, svg)| match std::fs::read(out.as_str()) {
                Ok(bytes) if bytes == svg.as_bytes() => None,
                Ok(_) => Some(json!({ "path": out, "reason": "differs" })),
                Err(_) => Some(json!({ "path": out, "reason": "missing" })),
            })
            .collect();
        let mut m = crate::envelope::head("render", stale.is_empty());
        let mut text = String::new();
        for s in &stale {
            text.push_str(&format!("{} is {}.\n", s["path"].as_str().unwrap_or(""), if s["reason"] == "missing" { "missing" } else { "out of date" }));
        }
        text.push_str(&format!("{} of {} file{} up to date.\n", drawn.len() - stale.len(), drawn.len(), if drawn.len() == 1 { "" } else { "s" }));
        if !stale.is_empty() {
            text.push_str("Render again without --check (add --force) to update.\n");
        }
        m.insert("checked".into(), json!(drawn.len()));
        let exit = if stale.is_empty() { EXIT_OK } else { EXIT_FINDINGS };
        m.insert("stale".into(), Value::Array(stale));
        return Ok(Output { json: Value::Object(m), text, exit, raw: None });
    }
    let mut r = report("render", findings, drawn.len(), 50, FailOn::Error, paint);
    let mut text_out = String::new();
    if !to_stdout {
        let mut outputs = Vec::new();
        for (out, svg) in &drawn {
            super::change::write(Path::new(out.as_str()), svg)?;
            outputs.push(json!({ "path": out, "bytes": svg.len() }));
            text_out.push_str(&format!("Wrote {out} ({} bytes).\n", svg.len()));
        }
        r.json.insert("outputs".into(), Value::Array(outputs));
        text_out.push('\n');
    }
    if no_validate {
        r.json.insert("validated".into(), json!(false));
        text_out.push_str("Validation skipped.\n");
    } else {
        text_out.push_str(&r.text);
    }
    let exit = if r.failed { EXIT_FINDINGS } else { EXIT_OK };
    let raw = to_stdout.then(|| drawn.pop().map(|(_, svg)| svg.into_bytes()).unwrap_or_default());
    Ok(Output { json: Value::Object(r.json), text: text_out, exit, raw })
}

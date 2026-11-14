use std::path::Path;

use serde_json::{Value, json};

use super::read_input;
use super::validate::{Paint, report};
use crate::cli::{FailOn, Live};
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, EXIT_OUTPUT, Output};
use crate::render::keep::{Kept, Previous};
use crate::{render, rules, spec};

/// Where frame `n` of `output` goes: `fig/orders.svg` gives `fig/orders.2.svg`.
fn frame_path(output: &str, n: usize) -> String {
    let p = Path::new(output);
    match (p.file_stem().and_then(|s| s.to_str()), p.extension().and_then(|s| s.to_str())) {
        (Some(stem), Some(ext)) => p.with_file_name(format!("{stem}.{n}.{ext}")).to_string_lossy().into_owned(),
        _ => format!("{output}.{n}"),
    }
}

/// A drawing, and what became of the earlier layout when there was one to keep.
pub struct Laid {
    pub svg: String,
    pub kept: Option<Kept>,
    /// The earlier layout could not be kept and --force let it go.
    pub relaid: bool,
}

/// The failure when boxes cannot stay where an earlier drawing had them.
fn changed(previous: &str, kept: &Kept) -> AppError {
    let more = match kept.reasons.len() - 1 {
        0 => String::new(),
        n => format!(" (and {n} more)"),
    };
    let reasons: Vec<Value> = kept.reasons.iter().map(|r| json!({ "id": r.id, "why": r.why })).collect();
    AppError::new(EXIT_OUTPUT, "layout-changed", format!("The layout of {previous} cannot be kept: {}{more}.", kept.reasons[0].why))
        .path(previous)
        .suggest(Some("Pass --force to lay the diagram out afresh, or change the spec so that the boxes fit.".into()))
        .details(json!({ "previous": previous, "reasons": reasons }))
}

/// Render, keeping the boxes where `previous` has them. When that cannot be done,
/// `force` lays the diagram out afresh; without it the render fails.
pub fn lay(spec: &spec::Spec, opt: &render::Options, previous: Option<(&Previous, &str)>, force: bool) -> Result<Laid, AppError> {
    let Some((prev, name)) = previous else {
        return Ok(Laid { svg: render::render(spec, opt)?, kept: None, relaid: false });
    };
    let (svg, kept) = render::render_keeping(spec, opt, prev)?;
    if kept.reasons.is_empty() {
        return Ok(Laid { svg, kept: Some(kept), relaid: false });
    }
    if !force {
        return Err(changed(name, &kept));
    }
    Ok(Laid { svg: render::render(spec, opt)?, kept: Some(kept), relaid: true })
}

/// What the report says about the earlier layout.
pub fn layout_json(previous: &str, l: &Laid) -> Option<Value> {
    let k = l.kept.as_ref()?;
    let reasons: Vec<Value> = k.reasons.iter().map(|r| json!({ "id": r.id, "why": r.why })).collect();
    Some(json!({ "previous": previous, "kept": k.kept.len(), "placed": k.placed, "gone": k.gone, "relaid": l.relaid, "reasons": reasons }))
}

pub fn layout_text(l: &Laid) -> String {
    let Some(k) = &l.kept else { return String::new() };
    let boxes = |n: usize| format!("{n} box{}", if n == 1 { "" } else { "es" });
    if l.relaid {
        return format!("Layout not kept: {}. Laid out afresh.\n", k.reasons[0].why);
    }
    let mut s = format!("Layout kept: {} in place", boxes(k.kept.len()));
    if !k.placed.is_empty() {
        s.push_str(&format!(", {} added ({})", k.placed.len(), k.placed.join(", ")));
    }
    if !k.gone.is_empty() {
        s.push_str(&format!(", {} gone", k.gone.len()));
    }
    s.push_str(".\n");
    s
}

/// The earlier drawing in a file, when the file is one this tool rendered.
pub fn previous_in(path: &Path) -> Option<Previous> {
    let text = std::fs::read_to_string(path).ok()?;
    render::keep::read(&text, &path.to_string_lossy()).ok()
}

/// Whether `bytes` is what the spec renders now: over itself as the earlier
/// drawing, or from nothing.
pub fn current(bytes: &[u8], path: &Path, spec: &spec::Spec, opt: &render::Options, wrap: &dyn Fn(String) -> String) -> Result<bool, AppError> {
    if let Some(prev) = previous_in(path)
        && let Ok((svg, kept)) = render::render_keeping(spec, opt, &prev)
        && kept.reasons.is_empty()
        && wrap(svg).as_bytes() == bytes
    {
        return Ok(true);
    }
    Ok(wrap(render::render(spec, opt)?).as_bytes() == bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, inline: Option<&str>, output: &str, theme: Option<&str>, live: Live, frames: bool, embed: bool, check: bool, previous: Option<&str>, no_validate: bool, force: bool, text_as_paths: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
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
    // Naming the file as the earlier drawing says it is meant to be replaced.
    if !to_stdout && !force && !check && let Some((taken, _)) = jobs.iter().find(|(p, _)| Path::new(p).exists() && previous != Some(p.as_str())) {
        return Err(AppError::new(EXIT_OUTPUT, "output-exists", format!("{taken} already exists; pass --force to replace it.")).path(taken.clone()).suggest(Some(format!("Add --previous {taken} to update it with its boxes kept in place, --force to replace it, or choose a new path."))));
    }
    let earlier = match previous {
        Some(p) => Some(render::keep::read(&read_input(p)?, p)?),
        None => None,
    };
    let path = (inline.is_none() && file != "-").then(|| Path::new(file));
    let cfg = loader.for_input(path)?;
    let settings = cfg.settings(path, profile, &RuleSel::default());

    // A theme folder named in the spec is found from where the spec is.
    let spec_dir = path.and_then(|p| p.parent()).unwrap_or(Path::new(""));
    let look = render::theme::choose(&render::theme::Choice { flag: theme, spec: jobs[0].1.theme.as_deref(), spec_dir, config: cfg.theme(), ceiling: loader.ceiling() })?.with_fonts(cfg.fonts());

    let page = |svg: String, job: &spec::Spec| -> String {
        if live != Live::Html {
            return svg;
        }
        render::interactive::page(&svg, job, &look.theme, look.dark.as_ref())
    };
    if check {
        // Compare with what is on disk; a file that is missing or different is stale.
        let mut stale: Vec<Value> = Vec::new();
        for (out, job) in &jobs {
            let opt = render::Options { theme: &look, text_as_paths, embed: embed.then_some(&doc), interactive: live != Live::None };
            match std::fs::read(out.as_str()) {
                Ok(bytes) if current(&bytes, Path::new(previous.unwrap_or(out)), job, &opt, &|svg| page(svg, job))? => {}
                Ok(_) => stale.push(json!({ "path": out, "reason": "differs" })),
                Err(_) => stale.push(json!({ "path": out, "reason": "missing" })),
            }
        }
        let mut m = crate::envelope::head("render", stale.is_empty());
        let mut text = String::new();
        for s in &stale {
            text.push_str(&format!("{} is {}.\n", s["path"].as_str().unwrap_or(""), if s["reason"] == "missing" { "missing" } else { "out of date" }));
        }
        text.push_str(&format!("{} of {} file{} up to date.\n", jobs.len() - stale.len(), jobs.len(), if jobs.len() == 1 { "" } else { "s" }));
        if !stale.is_empty() {
            text.push_str("Render again without --check (add --force) to update.\n");
        }
        m.insert("checked".into(), json!(jobs.len()));
        let exit = if stale.is_empty() { EXIT_OK } else { EXIT_FINDINGS };
        m.insert("stale".into(), Value::Array(stale));
        return Ok(Output { json: Value::Object(m), text, exit, raw: None });
    }

    let mut drawn = Vec::new();
    let mut layout: Option<(Value, String)> = None;
    for (out, job) in &jobs {
        let opt = render::Options { theme: &look, text_as_paths, embed: embed.then_some(&doc), interactive: live != Live::None };
        let laid = lay(job, &opt, earlier.as_ref().zip(previous), force)?;
        if layout.is_none() || laid.relaid {
            layout = layout_json(previous.unwrap_or(""), &laid).map(|j| (j, layout_text(&laid)));
        }
        drawn.push((out, laid.svg));
    }
    // Findings are about the drawing; the page around it, if any, is added afterwards.
    let mut findings = Vec::new();
    if !no_validate {
        for (out, svg) in &drawn {
            findings.extend(rules::analyze(if to_stdout { file } else { out.as_str() }, svg, &settings));
        }
    }
    for ((_, svg), (_, job)) in drawn.iter_mut().zip(&jobs) {
        *svg = page(std::mem::take(svg), job);
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
    if let Some((j, t)) = layout {
        r.json.insert("layout".into(), j);
        text_out.push_str(&t);
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

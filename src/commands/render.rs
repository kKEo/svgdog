use std::path::Path;

use serde_json::{Value, json};

use super::read_input;
use super::validate::{Paint, report};
use crate::cli::FailOn;
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, EXIT_OUTPUT, Output};
use crate::{render, rules, spec};

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, output: &str, theme: Option<&str>, no_validate: bool, force: bool, text_as_paths: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let to_stdout = output == "-";
    if !to_stdout && !force && Path::new(output).exists() {
        return Err(AppError::new(EXIT_OUTPUT, "output-exists", format!("{output} already exists; pass --force to replace it.")).path(output).suggest(Some("Add --force, or choose a new path.".into())));
    }
    let text = read_input(file)?;
    let doc = spec::read_document(&text, file)?;
    let parsed = spec::parse(&doc)?;
    let path = (file != "-").then(|| Path::new(file));
    let cfg = loader.for_input(path)?;
    let settings = cfg.settings(path, profile, &RuleSel::default());
    let svg = render::render(&parsed, &render::Options { theme, fonts: cfg.fonts(), text_as_paths })?;

    let label = if to_stdout { file.to_string() } else { output.to_string() };
    let findings = if no_validate { Vec::new() } else { rules::analyze(&label, &svg, &settings) };
    let mut r = report("render", findings, 1, 50, FailOn::Error, paint);
    let mut text_out = String::new();
    if !to_stdout {
        super::change::write(Path::new(output), &svg)?;
        r.json.insert("outputs".into(), json!([{ "path": output, "bytes": svg.len() }]));
        text_out.push_str(&format!("Wrote {output} ({} bytes).\n\n", svg.len()));
    }
    if no_validate {
        r.json.insert("validated".into(), json!(false));
        text_out.push_str("Validation skipped.\n");
    } else {
        text_out.push_str(&r.text);
    }
    let exit = if r.failed { EXIT_FINDINGS } else { EXIT_OK };
    Ok(Output { json: Value::Object(r.json), text: text_out, exit, raw: to_stdout.then_some(svg) })
}

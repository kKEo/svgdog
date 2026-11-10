//! What `edit` and `fix` share: where the result goes, and the envelope that reports it.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::validate::{Paint, report};
use crate::cli::FailOn;
use crate::config::Settings;
use crate::diff;
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, EXIT_OUTPUT, Output};
use crate::rules;

pub enum Dest {
    DryRun,
    InPlace,
    File(PathBuf),
    Stdout,
}

/// Decide where a change is written, before any work is done.
pub fn dest(input: &str, dry_run: bool, output: Option<&str>, in_place: bool) -> Result<Dest, AppError> {
    if output.is_some() && in_place {
        return Err(AppError::usage("argument-conflict", "Give -o or --in-place, not both."));
    }
    if dry_run {
        return Ok(Dest::DryRun);
    }
    match (output, in_place) {
        (Some("-"), _) => Ok(Dest::Stdout),
        (Some(path), _) => {
            let p = Path::new(path);
            if p.exists() {
                let same = input != "-" && p.canonicalize().ok() == Path::new(input).canonicalize().ok();
                let hint = if same { "Use --in-place to change the input file." } else { "Choose a new path, or use --in-place to change the input file." };
                return Err(AppError::new(EXIT_OUTPUT, "output-exists", format!("{path} already exists and is not overwritten.")).path(path).suggest(Some(hint.into())));
            }
            Ok(Dest::File(p.to_path_buf()))
        }
        (None, true) if input == "-" => Err(AppError::usage("invalid-argument", "--in-place cannot be used with stdin; give -o.")),
        (None, true) => Ok(Dest::InPlace),
        (None, false) => Err(AppError::usage("missing-argument", "Say where the result goes: -o <out>, --in-place or --dry-run.").suggest(Some("--dry-run shows the diff and the resulting findings without writing.".into()))),
    }
}

fn write(path: &Path, text: &str) -> Result<(), AppError> {
    let fail = |e: std::io::Error| AppError::new(EXIT_OUTPUT, "write-failed", format!("Cannot write {}: {e}.", path.display())).path(path.display().to_string());
    // Write beside the target and rename, so a failure never leaves half a file.
    let tmp = path.with_extension(format!("{}.svgdoc-tmp", path.extension().and_then(|e| e.to_str()).unwrap_or("out")));
    std::fs::write(&tmp, text).map_err(fail)?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        fail(e)
    })
}

/// Write the result as `dest` says and build the envelope: the findings of the
/// changed text, the diff, and the files written.
#[allow(clippy::too_many_arguments)]
pub fn finish(command: &'static str, input: &str, before: &str, after: String, dest: Dest, settings: &Settings, extra: Vec<(&'static str, Value)>, note: String, paint: &Paint) -> Result<Output, AppError> {
    let label = match &dest {
        Dest::File(p) => p.to_string_lossy().into_owned(),
        _ => input.to_string(),
    };
    let findings = rules::analyze(&label, &after, settings);
    let mut r = report(command, findings, 1, 50, FailOn::Error, paint);
    let d = diff::unified(&label, before, &after);
    let changed = before != after;

    let mut outputs = Vec::new();
    match &dest {
        Dest::InPlace if changed => {
            write(Path::new(input), &after)?;
            outputs.push(json!({ "path": input, "bytes": after.len() }));
        }
        Dest::File(p) => {
            write(p, &after)?;
            outputs.push(json!({ "path": label, "bytes": after.len() }));
        }
        _ => {}
    }

    r.json.insert("outputs".into(), Value::Array(outputs.clone()));
    r.json.insert("changed".into(), json!(changed));
    r.json.insert("dry_run".into(), json!(matches!(dest, Dest::DryRun)));
    for (k, v) in extra {
        r.json.insert(k.into(), v);
    }
    r.json.insert("diff".into(), json!(d.text));
    r.json.insert("diff_truncated".into(), json!(d.truncated));

    let mut text = String::new();
    text.push_str(&d.text);
    if !d.text.is_empty() {
        text.push('\n');
    }
    text.push_str(&note);
    text.push_str(&match (&dest, changed) {
        (Dest::DryRun, true) => "Dry run: nothing written.\n".to_string(),
        (_, false) => "No change.\n".to_string(),
        (Dest::File(_), true) | (Dest::InPlace, true) => format!("Wrote {label}.\n"),
        (Dest::Stdout, true) => String::new(),
    });
    text.push_str("\nAfter the change:\n");
    text.push_str(&r.text);

    let exit = if r.failed { EXIT_FINDINGS } else { EXIT_OK };
    let raw = matches!(dest, Dest::Stdout).then_some(after);
    Ok(Output { json: Value::Object(r.json), text, exit, raw })
}

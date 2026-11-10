use std::path::Path;

use serde_json::json;

use super::change::{dest, finish};
use super::read_input;
use super::validate::Paint;
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, Output};
use crate::{ops, render, spec, spec_edit};

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, set: &[String], ops_arg: Option<&str>, inline: Option<&serde_json::Value>, dry_run: bool, output: Option<&str>, in_place: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    if inline.is_some() && (!set.is_empty() || ops_arg.is_some()) {
        return Err(AppError::usage("argument-conflict", "Give set or ops, not both."));
    }
    let list = match (set.is_empty(), ops_arg) {
        (true, None) if inline.is_some() => ops::parse_ops(inline.unwrap())?,
        (false, Some(_)) => return Err(AppError::usage("argument-conflict", "Give --set or --ops, not both.")),
        (true, None) => return Err(AppError::usage("missing-argument", "Nothing to change: give --set 'SELECTOR.attr=value' or --ops <file>.").suggest(Some("svgdoc schema ops prints the operation format.".into()))),
        (false, None) => set.iter().map(|s| ops::parse_set(s)).collect::<Result<Vec<_>, _>>()?,
        (true, Some(path)) => {
            if path == "-" && file == "-" {
                return Err(AppError::usage("invalid-argument", "The file and --ops cannot both come from stdin."));
            }
            let text = read_input(path)?;
            let doc: serde_json::Value = serde_json::from_str(&text).map_err(|e| AppError::input("invalid-op", format!("{path} is not JSON: {e}.")).path(path))?;
            ops::parse_ops(&doc)?
        }
    };
    let dest = dest(file, dry_run, output, in_place)?;
    let src = read_input(file)?;
    let path = (file != "-").then(|| Path::new(file));
    let cfg = loader.for_input(path)?;
    let settings = cfg.settings(path, profile, &RuleSel::default());

    if !src.trim_start().starts_with('<') {
        // A diagram spec: change it as data, check it still renders, and report what the drawing would say.
        let mut doc = spec::read_document(&src, file)?;
        let applied = spec_edit::apply(&mut doc, &list)?;
        let parsed = spec::parse(&doc)?;
        let after = if applied == 0 { src.clone() } else { spec_edit::write(&doc, spec::is_json(&src))? };
        let svg = render::render(&parsed, &render::Options { theme: None, fonts: cfg.fonts(), text_as_paths: false })?;
        let note = format!("{applied} of {} operation{} changed the spec. Findings are for the diagram it renders.\n", list.len(), if list.len() == 1 { "" } else { "s" });
        let extra = vec![("operations", json!({ "given": list.len(), "applied": applied }))];
        return finish("edit", file, &src, after, dest, &settings, extra, note, paint, Some(&svg));
    }

    let result = ops::apply(&src, &list)?;
    let note = format!("{} of {} operation{} changed the file.\n", result.count(), list.len(), if list.len() == 1 { "" } else { "s" });
    let extra = vec![("operations", json!({ "given": list.len(), "applied": result.count() }))];
    finish("edit", file, &src, result.text, dest, &settings, extra, note, paint, None)
}

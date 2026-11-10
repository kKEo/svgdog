use std::path::Path;

use serde_json::json;

use super::change::{dest, finish};
use super::read_input;
use super::validate::Paint;
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, Output};
use crate::ops;

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, set: &[String], ops_arg: Option<&str>, dry_run: bool, output: Option<&str>, in_place: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let list = match (set.is_empty(), ops_arg) {
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
    let lower = file.to_ascii_lowercase();
    if [".yaml", ".yml", ".json"].iter().any(|e| lower.ends_with(e)) {
        return Err(AppError::usage("not-implemented", "Editing a diagram spec arrives with the render command; this build edits SVG files."));
    }
    let dest = dest(file, dry_run, output, in_place)?;
    let src = read_input(file)?;
    let path = (file != "-").then(|| Path::new(file));
    let settings = loader.for_input(path)?.settings(path, profile, &RuleSel::default());

    let result = ops::apply(&src, &list)?;
    let note = format!("{} of {} operation{} changed the file.\n", result.count(), list.len(), if list.len() == 1 { "" } else { "s" });
    let extra = vec![("operations", json!({ "given": list.len(), "applied": result.count() }))];
    finish("edit", file, &src, result.text, dest, &settings, extra, note, paint)
}

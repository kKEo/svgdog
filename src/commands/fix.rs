use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use super::change::{dest, finish};
use super::read_input;
use super::validate::{Paint, rule_ids};
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, Output};
use crate::{ops, rules};

/// Fixes can expose further fixable findings (a new viewBox makes margins measurable),
/// so the loop repeats until a pass changes nothing.
const MAX_ROUNDS: usize = 30;

/// Fixes whose value depends on the drawing's scale. They wait until the fixes that
/// change the viewBox have settled, or each of those would make them run again.
const SCALE_BOUND: &[&str] = &["stroke-too-thin", "font-too-small"];
const SCALE_CHANGING: &[&str] = &["viewbox-missing", "content-clipped", "excess-margin", "text-overflow"];

#[allow(clippy::too_many_arguments)]
pub fn run(file: &str, only: &[String], allow_unsafe: bool, dry_run: bool, output: Option<&str>, in_place: bool, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let only = rule_ids(only, "--rules")?;
    let dest = dest(file, dry_run, output, in_place)?;
    let src = read_input(file)?;
    let path = (file != "-").then(|| Path::new(file));
    let settings = loader.for_input(path)?.settings(path, profile, &RuleSel::default());

    let mut text = src.clone();
    let mut seen = vec![src.clone()];
    let mut fixed: BTreeMap<&'static str, usize> = BTreeMap::new();
    for _ in 0..MAX_ROUNDS {
        let findings = rules::analyze(file, &text, &settings);
        let mut batch: Vec<(&'static str, Vec<ops::Op>)> = Vec::new();
        for f in &findings {
            let Some(fix) = &f.fix else { continue };
            if (!fix.safe && !allow_unsafe) || (!only.is_empty() && !only.contains(&f.rule)) {
                continue;
            }
            batch.push((f.rule, ops::parse_ops(&Value::Array(fix.ops.clone()))?));
        }
        if batch.iter().any(|(rule, _)| SCALE_CHANGING.contains(rule)) {
            batch.retain(|(rule, _)| !SCALE_BOUND.contains(rule));
        }
        if batch.is_empty() {
            break;
        }
        // A fix that collides with an earlier one in this pass waits for the next round.
        let groups: Vec<Vec<ops::Op>> = batch.iter().map(|(_, g)| g.clone()).collect();
        let result = ops::apply_groups(&text, &groups, false)?;
        if result.text == text {
            break;
        }
        for ((rule, _), ok) in batch.iter().zip(&result.accepted) {
            if *ok {
                *fixed.entry(rule).or_default() += 1;
            }
        }
        // A text seen before means two fixes undo each other; stop rather than loop.
        if seen.contains(&result.text) {
            break;
        }
        text = result.text;
        seen.push(text.clone());
    }

    let total: usize = fixed.values().sum();
    let list: Vec<String> = fixed.iter().map(|(r, n)| format!("{r} x{n}")).collect();
    let note = if total == 0 { String::new() } else { format!("Applied {total} fix{}: {}.\n", if total == 1 { "" } else { "es" }, list.join(", ")) };
    let extra = vec![("fixed", json!(fixed.iter().map(|(r, n)| json!({ "rule": r, "count": n })).collect::<Vec<_>>()))];
    finish("fix", file, &src, text, dest, &settings, extra, note, paint, None)
}

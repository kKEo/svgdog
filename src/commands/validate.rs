use std::path::Path;

use serde_json::{Value, json};

use super::read_input;
use crate::catalogue;
use crate::cli::FailOn;
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, Finding, Output, Severity, Summary, head};
use crate::rules;
use crate::util::did_you_mean;

pub struct Paint {
    pub color: bool,
}

impl Paint {
    fn wrap(&self, code: &str, s: &str) -> String {
        if self.color { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
    }
}

fn rule_ids(list: &[String], flag: &str) -> Result<Vec<&'static str>, AppError> {
    let mut out = Vec::new();
    for id in list.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        match catalogue::rule(id) {
            Some(r) => out.push(r.id),
            None => {
                return Err(AppError::usage("unknown-rule", format!("Unknown rule '{id}' in {flag}.")).suggest(did_you_mean(id, catalogue::ids())));
            }
        }
    }
    Ok(out)
}

fn expand(args: &[String]) -> Result<Vec<String>, AppError> {
    let mut files = Vec::new();
    for a in args {
        let globby = a.contains(['*', '?', '[']);
        if a == "-" || !globby || Path::new(a).exists() {
            files.push(a.clone());
            continue;
        }
        let paths = glob::glob(a).map_err(|e| AppError::usage("invalid-argument", format!("'{a}' is not a valid glob: {e}.")))?;
        let mut hits: Vec<String> = paths.filter_map(|p| p.ok()).filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()).collect();
        if hits.is_empty() {
            return Err(AppError::input("file-not-found", format!("No file matches '{a}'.")).path(a.clone()));
        }
        hits.sort();
        files.extend(hits);
    }
    if files.iter().filter(|f| *f == "-").count() > 1 {
        return Err(AppError::usage("invalid-argument", "stdin (-) can be given only once."));
    }
    let mut seen = std::collections::BTreeSet::new();
    files.retain(|f| seen.insert(f.clone()));
    Ok(files)
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    files: &[String],
    only: &[String],
    skip: &[String],
    fail_on: FailOn,
    max_findings: usize,
    loader: &mut Loader,
    profile: Option<Profile>,
    paint: &Paint,
) -> Result<Output, AppError> {
    let sel = RuleSel { only: if only.is_empty() { None } else { Some(rule_ids(only, "--rules")?) }, skip: rule_ids(skip, "--skip")? };
    let files = expand(files)?;

    let mut all: Vec<Finding> = Vec::new();
    for f in &files {
        let src = read_input(f)?;
        let path = (f != "-").then(|| Path::new(f.as_str()));
        let cfg = loader.for_input(path)?;
        let settings = cfg.settings(path, profile, &sel);
        all.extend(rules::analyze(f, &src, &settings));
    }
    all.sort_by(|a, b| (&a.file, a.line, a.rule).cmp(&(&b.file, b.line, b.rule)));

    let count = |s: Severity| all.iter().filter(|f| f.severity == s).count();
    let summary = Summary {
        files: files.len(),
        errors: count(Severity::Error),
        warnings: count(Severity::Warning),
        info: count(Severity::Info),
        fixable: 0,
        truncated: all.len() > max_findings,
    };
    let threshold = match fail_on {
        FailOn::Error => Severity::Error,
        FailOn::Warning => Severity::Warning,
    };
    let failed = all.iter().any(|f| f.severity >= threshold);
    let total = all.len();
    all.truncate(max_findings);

    let mut m = head("validate", !failed);
    m.insert("summary".into(), serde_json::to_value(&summary).unwrap());
    m.insert("findings".into(), serde_json::to_value(&all).unwrap());
    m.insert("outputs".into(), json!([]));

    let mut text = String::new();
    let mut current: Option<&str> = None;
    let width = all.iter().map(|f| f.rule.len()).max().unwrap_or(0);
    for f in &all {
        if current != Some(f.file.as_str()) {
            if current.is_some() {
                text.push('\n');
            }
            text.push_str(&paint.wrap("1", &f.file));
            text.push('\n');
            current = Some(f.file.as_str());
        }
        let sev = match f.severity {
            Severity::Error => paint.wrap("31", "error  "),
            Severity::Warning => paint.wrap("33", "warning"),
            Severity::Info => paint.wrap("36", "info   "),
        };
        text.push_str(&format!("  {:>5}  {sev}  {:<width$}  {}  {}\n", f.line, f.rule, paint.wrap("2", &f.target), f.message));
    }
    if !all.is_empty() {
        text.push('\n');
    }
    if summary.truncated {
        text.push_str(&format!("Showing {} of {total} findings; raise --max-findings to see more.\n", all.len()));
    }
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    text.push_str(&format!(
        "{} checked: {}, {}, {} info\n",
        plural(summary.files, "file", "files"),
        plural(summary.errors, "error", "errors"),
        plural(summary.warnings, "warning", "warnings"),
        summary.info
    ));

    Ok(Output { json: Value::Object(m), text, exit: if failed { EXIT_FINDINGS } else { EXIT_OK } })
}

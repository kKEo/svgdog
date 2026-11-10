use serde_json::{Value, json};

use crate::catalogue::{self, RULES, RuleMeta};
use crate::envelope::{AppError, EXIT_OK, Output, head};
use crate::util::{did_you_mean, fmt, num};

fn entry(r: &RuleMeta) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("id".into(), json!(r.id));
    m.insert("group".into(), json!(r.group.as_str()));
    m.insert("severity".into(), json!(r.severity.as_str()));
    m.insert("autofix".into(), json!(r.autofix.as_str()));
    m.insert("fails_when".into(), json!(r.fails_when));
    m
}

pub fn run(explain: Option<&str>) -> Result<Output, AppError> {
    let mut m = head("rules", true);
    let mut text = String::new();
    match explain {
        None => {
            let w = RULES.iter().map(|r| r.id.len()).max().unwrap_or(0);
            let mut group = "";
            for r in RULES {
                if r.group.as_str() != group {
                    group = r.group.as_str();
                    text.push_str(&format!("{}{group}\n", if text.is_empty() { "" } else { "\n" }));
                }
                text.push_str(&format!("  {:<w$}  {:<7}  {:<6}  {}\n", r.id, r.severity.as_str(), r.autofix.as_str(), r.fails_when));
            }
            text.push_str(&format!("\n{} rules. Columns: id, default severity, autofix, fails when.\n", RULES.len()));
            m.insert("rules".into(), Value::Array(RULES.iter().map(|r| Value::Object(entry(r))).collect()));
        }
        Some(id) => {
            let r = catalogue::rule(id).ok_or_else(|| AppError::usage("unknown-rule", format!("Unknown rule '{id}'.")).suggest(did_you_mean(id, catalogue::ids())))?;
            let mut e = entry(r);
            e.insert("autofix_note".into(), json!(r.autofix_note));
            e.insert("rationale".into(), json!(r.rationale));
            e.insert("options".into(), Value::Array(r.options.iter().map(|o| json!({ "key": o.key, "default": num(o.default), "about": o.about })).collect()));
            e.insert("example_config".into(), json!(r.example_config));
            e.insert("fail".into(), json!(r.fail));
            e.insert("pass".into(), json!(r.pass));
            m.insert("rule".into(), Value::Object(e));

            text.push_str(&format!("{}  ({}, {} by default)\n\n", r.id, r.group.as_str(), r.severity.as_str()));
            text.push_str(&format!("Fails when: {}.\nAutofix:    {}.\n\n{}\n", r.fails_when, r.autofix_note, r.rationale));
            if !r.options.is_empty() {
                text.push_str(&format!("\nThresholds, set in svgdoc.toml as [rules] {} = {{ key = value }}:\n", r.id));
                for o in r.options {
                    text.push_str(&format!("  {} = {}  {}\n", o.key, fmt(o.default), o.about));
                }
            }
            if !r.example_config.is_empty() {
                text.push_str(&format!("\nThe examples assume this svgdoc.toml:\n{}", indent(r.example_config)));
            }
            text.push_str(&format!("\nFails:\n{}\nPasses:\n{}", indent(r.fail), indent(r.pass)));
        }
    }
    Ok(Output { json: Value::Object(m), text, exit: EXIT_OK, raw: None })
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("  {l}\n")).collect()
}

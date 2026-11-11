//! `svgdog examples`: small, tested specs to start from.

use serde_json::{Value, json};

use crate::envelope::{AppError, EXIT_OK, Output, head};
use crate::util::did_you_mean;

/// Name and text of every bundled example. The first comment line of each says what it shows.
pub const EXAMPLES: &[(&str, &str)] = &[
    ("minimal", include_str!("../../examples/minimal.yaml")),
    ("flow", include_str!("../../examples/flow.yaml")),
    ("layers", include_str!("../../examples/layers.yaml")),
    ("sequence", include_str!("../../examples/sequence.yaml")),
    ("free", include_str!("../../examples/free.yaml")),
    ("state", include_str!("../../examples/state.yaml")),
    ("timeline", include_str!("../../examples/timeline.yaml")),
    ("compare", include_str!("../../examples/compare.yaml")),
    ("steps", include_str!("../../examples/steps.yaml")),
];

fn about(text: &str) -> &str {
    text.lines().next().and_then(|l| l.strip_prefix('#')).map(|l| l.trim()).unwrap_or("")
}

fn diagram(text: &str) -> &str {
    text.lines().find_map(|l| l.strip_prefix("diagram:")).map(|l| l.trim()).unwrap_or("")
}

pub fn run(name: Option<&str>) -> Result<Output, AppError> {
    let mut m = head("examples", true);
    let entry = |(n, t): &(&str, &str), full: bool| {
        let mut e = json!({ "name": n, "diagram": diagram(t), "about": about(t) });
        if full {
            e["spec"] = json!(t);
        }
        e
    };
    let text = match name {
        None => {
            m.insert("examples".into(), Value::Array(EXAMPLES.iter().map(|e| entry(e, false)).collect()));
            let w = EXAMPLES.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
            let mut out: String = EXAMPLES.iter().map(|(n, t)| format!("{n:<w$}  {}\n", about(t))).collect();
            out.push_str("\nPrint one with: svgdog examples NAME --format text\n");
            out
        }
        Some(n) => {
            let found = EXAMPLES.iter().find(|(k, _)| *k == n).ok_or_else(|| AppError::usage("unknown-example", format!("Unknown example '{n}'.")).suggest(did_you_mean(n, EXAMPLES.iter().map(|(k, _)| *k))))?;
            m.insert("examples".into(), json!([entry(found, true)]));
            found.1.to_string()
        }
    };
    Ok(Output { json: Value::Object(m), text, exit: EXIT_OK, raw: None })
}

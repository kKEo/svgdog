//! `svgdog describe`: a diagram as plain text, from its spec or from a drawing that carries one.

use serde_json::{Value, json};

use super::read_input;
use crate::envelope::{AppError, EXIT_OK, Output, head};
use crate::{describe, spec};

/// The spec text behind a file: the file itself, or what a rendered SVG has embedded.
pub fn source(file: &str, text: &str) -> Result<String, AppError> {
    if !text.trim_start().starts_with('<') {
        return Ok(text.to_string());
    }
    describe::embedded(text).ok_or_else(|| {
        AppError::input("no-spec", format!("{file} is an SVG without an embedded spec, so there is nothing to read it back from."))
            .path(file)
            .suggest(Some("Describe the spec file, or render with --embed-spec. For any SVG, 'svgdog inspect' lists what is drawn.".into()))
    })
}

pub fn run(file: &str, short: bool) -> Result<Output, AppError> {
    let text = read_input(file)?;
    let src = source(file, &text)?;
    let doc = spec::read_document(&src, file)?;
    let parsed = spec::parse(&doc)?;
    let (brief, full) = (describe::short(&parsed), describe::full(&parsed));
    let mut m = head("describe", true);
    m.insert("title".into(), json!(parsed.title));
    m.insert("short".into(), json!(brief));
    m.insert("text".into(), json!(full));
    m.insert("spec".into(), doc);
    Ok(Output { json: Value::Object(m), text: if short { format!("{brief}\n") } else { full }, exit: EXIT_OK, raw: None })
}

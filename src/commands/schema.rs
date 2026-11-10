use crate::cli::SchemaName;
use crate::envelope::{AppError, EXIT_OK, Output};

pub fn run(name: SchemaName) -> Result<Output, AppError> {
    let text = match name {
        SchemaName::Spec => include_str!("../../schemas/spec.json"),
        SchemaName::Config => include_str!("../../schemas/config.json"),
        SchemaName::Findings => include_str!("../../schemas/findings.json"),
        SchemaName::Scene => include_str!("../../schemas/scene.json"),
        SchemaName::Ops => {
            return Err(AppError::usage("not-implemented", "The ops schema arrives with the edit and fix commands; this build has validate, inspect, schema and rules."));
        }
    };
    let json = serde_json::from_str(text).expect("bundled schema is valid JSON");
    Ok(Output { json, text: text.to_string(), exit: EXIT_OK })
}

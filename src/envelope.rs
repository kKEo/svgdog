//! The output contract: envelope, findings, errors and exit codes.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const TOOL: &str = "svgdog";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const EXIT_OK: i32 = 0;
pub const EXIT_FINDINGS: i32 = 1;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_INPUT: i32 = 3;
pub const EXIT_OUTPUT: i32 = 4;
pub const EXIT_INTERNAL: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
    pub fn parse(s: &str) -> Option<Severity> {
        match s {
            "info" => Some(Severity::Info),
            "warning" => Some(Severity::Warning),
            "error" => Some(Severity::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Severity,
    pub file: String,
    pub target: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[Value; 4]>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<Map<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

/// An autofix: operations in the `edit --ops` format.
#[derive(Clone, Debug, Serialize)]
pub struct Fix {
    /// True when `fix` applies it without --unsafe.
    pub safe: bool,
    pub ops: Vec<Value>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub files: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub fixable: usize,
    pub truncated: bool,
}

/// A failure that ends the command with exit code 2 to 5.
#[derive(Clone, Debug)]
pub struct AppError {
    pub exit: i32,
    pub code: &'static str,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

impl AppError {
    pub fn new(exit: i32, code: &'static str, message: impl Into<String>) -> Self {
        AppError { exit, code, message: message.into(), path: None, suggestion: None }
    }
    pub fn usage(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(EXIT_USAGE, code, message)
    }
    pub fn input(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(EXIT_INPUT, code, message)
    }
    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.path = Some(p.into());
        self
    }
    pub fn suggest(mut self, s: Option<String>) -> Self {
        self.suggestion = s;
        self
    }

    pub fn to_json(&self, command: &str) -> Value {
        let mut err = Map::new();
        err.insert("code".into(), json!(self.code));
        err.insert("message".into(), json!(self.message));
        if let Some(p) = &self.path {
            err.insert("path".into(), json!(p));
        }
        if let Some(s) = &self.suggestion {
            err.insert("suggestion".into(), json!(s));
        }
        json!({
            "tool": TOOL,
            "version": VERSION,
            "command": command,
            "ok": false,
            "error": Value::Object(err),
        })
    }

    pub fn to_text(&self) -> String {
        let mut s = format!("error[{}]: {}", self.code, self.message);
        if let Some(sug) = &self.suggestion {
            s.push_str(&format!("\n  {sug}"));
        }
        s
    }
}

/// The opening fields every envelope shares.
pub fn head(command: &str, ok: bool) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("tool".into(), json!(TOOL));
    m.insert("version".into(), json!(VERSION));
    m.insert("command".into(), json!(command));
    m.insert("ok".into(), json!(ok));
    m
}

/// What a command hands back to `main`: one JSON object, its text rendering and an exit code.
pub struct Output {
    pub json: Value,
    pub text: String,
    pub exit: i32,
    /// When set, this goes to stdout in place of the envelope (`-o -`).
    pub raw: Option<Vec<u8>>,
}

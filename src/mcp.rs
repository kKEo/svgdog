//! `svgdog mcp`: the same commands as Model Context Protocol tools over stdio.
//!
//! The server adds no behaviour. A tool call is turned into the command line it
//! mirrors, run in process, and answered with the JSON envelope. Every path is
//! held inside the root directory given at start-up.
//!
//! Two protocol styles are served: versions with the `initialize` handshake, and
//! 2026-07-28, where each request carries its version and the client may call
//! `server/discover` first.

use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};

use clap::Parser;
use serde_json::{Map, Value, json};

use crate::cli::Cli;
use crate::envelope::{AppError, EXIT_INPUT, EXIT_INTERNAL, VERSION};
use crate::util::did_you_mean;
use crate::{Env, commands, execute};

const MODERN: &[&str] = &["2026-07-28"];
const LEGACY: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CAPS: &str = "io.modelcontextprotocol/clientCapabilities";
const META_SERVER: &str = "io.modelcontextprotocol/serverInfo";
const INSTRUCTIONS: &str = "SVGDog compiles diagram specs to SVG and checks any SVG. Typical loop: svgdog_schema (name: spec) once, write a spec file, svgdog_render, read the findings, svgdog_fix for those marked fixable or change the spec, repeat until there are no errors; svgdog_preview with annotate for a look. All paths are relative to the server's root directory.";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Path,
    PathList,
    Str,
    StrList,
    Bool,
    Int,
    Json,
}

struct Arg {
    name: &'static str,
    kind: Kind,
    /// The flag it becomes; `None` is a positional argument.
    flag: Option<&'static str>,
    required: bool,
    about: &'static str,
}

const fn arg(name: &'static str, kind: Kind, flag: Option<&'static str>, required: bool, about: &'static str) -> Arg {
    Arg { name, kind, flag, required, about }
}

struct Tool {
    name: &'static str,
    command: &'static str,
    about: &'static str,
    read_only: bool,
    /// Whether --profile, --config and --no-config apply.
    config: bool,
    args: &'static [Arg],
}

const CONFIG_ARGS: &[Arg] = &[
    arg("profile", Kind::Str, Some("--profile"), false, "Rule profile: minimal, docs or strict"),
    arg("config", Kind::Path, Some("--config"), false, "Config file to use instead of the nearest svgdog.toml"),
    arg("no_config", Kind::Bool, Some("--no-config"), false, "Ignore any config file"),
];

const TOOLS: &[Tool] = &[
    Tool {
        name: "svgdog_render",
        command: "render",
        about: "Compile a diagram spec (YAML or JSON) to SVG and validate the result. Returns the envelope with findings and the output path.",
        read_only: false,
        config: true,
        args: &[
            arg("spec", Kind::Path, None, true, "Diagram spec file"),
            arg("output", Kind::Path, Some("-o"), true, "Where to write the SVG"),
            arg("theme", Kind::Str, Some("--theme"), false, "docs-light or docs-dark"),
            arg("no_validate", Kind::Bool, Some("--no-validate"), false, "Skip the checks on the result"),
            arg("force", Kind::Bool, Some("--force"), false, "Replace an existing output file"),
            arg("text_as_paths", Kind::Bool, Some("--text-as-paths"), false, "Convert text to outlines"),
        ],
    },
    Tool {
        name: "svgdog_validate",
        command: "validate",
        about: "Run the rule catalogue on one or more SVG files. Returns the envelope with findings; fixable ones carry a fix.",
        read_only: true,
        config: true,
        args: &[
            arg("files", Kind::PathList, None, true, "SVG files or globs"),
            arg("rules", Kind::StrList, Some("--rules"), false, "Run only these rule ids"),
            arg("skip", Kind::StrList, Some("--skip"), false, "Skip these rule ids"),
            arg("fail_on", Kind::Str, Some("--fail-on"), false, "error or warning: lowest severity that makes ok false"),
            arg("max_findings", Kind::Int, Some("--max-findings"), false, "Cap on listed findings; default 50"),
        ],
    },
    Tool {
        name: "svgdog_inspect",
        command: "inspect",
        about: "Describe an SVG's visible elements as data, each with its bounding box in viewBox units. Returns scene JSON.",
        read_only: true,
        config: false,
        args: &[
            arg("file", Kind::Path, None, true, "SVG file"),
            arg("select", Kind::Str, Some("--select"), false, "Keep only matching elements and their descendants: #id, tag or tag[attr=\"value\"]"),
            arg("depth", Kind::Int, Some("--depth"), false, "Deepest nesting level to return"),
            arg("fields", Kind::StrList, Some("--fields"), false, "Any of id, tag, bbox, text, fill, stroke, refs"),
        ],
    },
    Tool {
        name: "svgdog_preview",
        command: "preview",
        about: "Rasterize an SVG to PNG. Without output the PNG is returned as image content; with output it is written there.",
        read_only: false,
        config: true,
        args: &[
            arg("file", Kind::Path, None, true, "SVG file"),
            arg("output", Kind::Path, Some("-o"), false, "Where to write the PNG; omit to get the image back"),
            arg("width", Kind::Int, Some("--width"), false, "Image width in px; default 1200, at most 2000"),
            arg("background", Kind::Str, Some("--background"), false, "light or dark page behind a transparent drawing"),
            arg("annotate", Kind::Bool, Some("--annotate"), false, "Outline each finding's box and print its rule id"),
            arg("crop", Kind::Str, Some("--crop"), false, "Show only the matching elements and their surroundings"),
            arg("force", Kind::Bool, Some("--force"), false, "Replace an existing output file"),
        ],
    },
    Tool {
        name: "svgdog_edit",
        command: "edit",
        about: "Apply targeted changes to an SVG or a diagram spec. Give set or ops, and one of dry_run, output or in_place. Returns the envelope with a diff.",
        read_only: false,
        config: true,
        args: &[
            arg("file", Kind::Path, None, true, "SVG or spec file"),
            arg("set", Kind::StrList, Some("--set"), false, "Changes of the form SELECTOR.attr=value"),
            arg("ops", Kind::Json, None, false, "Operations as JSON: an array, or a finding's fix object (see svgdog_schema, name: ops)"),
            arg("dry_run", Kind::Bool, Some("--dry-run"), false, "Report the diff and resulting findings; write nothing"),
            arg("output", Kind::Path, Some("-o"), false, "Write the result to a new file"),
            arg("in_place", Kind::Bool, Some("--in-place"), false, "Change the input file"),
        ],
    },
    Tool {
        name: "svgdog_fix",
        command: "fix",
        about: "Apply the autofixes that findings carry. Give one of dry_run, output or in_place. Returns the envelope with a diff.",
        read_only: false,
        config: true,
        args: &[
            arg("file", Kind::Path, None, true, "SVG file"),
            arg("rules", Kind::StrList, Some("--rules"), false, "Fix only these rule ids"),
            arg("unsafe", Kind::Bool, Some("--unsafe"), false, "Also apply fixes that move or recolour content"),
            arg("dry_run", Kind::Bool, Some("--dry-run"), false, "Report the diff and resulting findings; write nothing"),
            arg("output", Kind::Path, Some("-o"), false, "Write the result to a new file"),
            arg("in_place", Kind::Bool, Some("--in-place"), false, "Change the input file"),
        ],
    },
    Tool {
        name: "svgdog_schema",
        command: "schema",
        about: "Print the JSON Schema of a format: spec, config, findings, scene or ops.",
        read_only: true,
        config: false,
        args: &[arg("name", Kind::Str, None, true, "spec, config, findings, scene or ops")],
    },
    Tool {
        name: "svgdog_rules",
        command: "rules",
        about: "List every rule with its severity and autofix status, or explain one rule with its thresholds and a failing and a passing example.",
        read_only: true,
        config: false,
        args: &[arg("explain", Kind::Str, Some("--explain"), false, "Rule id to explain")],
    },
];

fn all_args(t: &Tool) -> impl Iterator<Item = &'static Arg> {
    t.args.iter().chain(if t.config { CONFIG_ARGS.iter() } else { [].iter() })
}

fn tool_json(t: &Tool) -> Value {
    let mut props = Map::new();
    let mut required = Vec::new();
    for a in all_args(t) {
        let schema = match a.kind {
            Kind::Path | Kind::Str => json!({ "type": "string", "description": a.about }),
            Kind::PathList | Kind::StrList => json!({ "type": "array", "items": { "type": "string" }, "description": a.about }),
            Kind::Bool => json!({ "type": "boolean", "description": a.about }),
            Kind::Int => json!({ "type": "integer", "minimum": 0, "description": a.about }),
            Kind::Json => json!({ "description": a.about }),
        };
        props.insert(a.name.into(), schema);
        if a.required {
            required.push(a.name);
        }
    }
    json!({
        "name": t.name,
        "description": t.about,
        "inputSchema": { "type": "object", "additionalProperties": false, "properties": props, "required": required },
        "annotations": { "readOnlyHint": t.read_only, "openWorldHint": false },
    })
}

/// Refuse a path that is, or resolves to, somewhere outside the root.
fn confine(root: &Path, raw: &str, name: &str) -> Result<(), AppError> {
    let refuse = |why: &str| AppError::new(EXIT_INPUT, "outside-root", format!("'{raw}' ({name}) {why}; the server only reads and writes inside its root directory.")).path(raw);
    if raw == "-" {
        return Err(AppError::usage("invalid-argument", format!("'{name}' cannot be '-': the server's stdin and stdout carry the protocol.")));
    }
    let joined = root.join(raw);
    // Resolve what exists, so a symlink cannot lead out; the rest is checked as written.
    let mut existing = joined.as_path();
    let mut tail: Vec<Component> = Vec::new();
    while !existing.exists() {
        match (existing.file_name(), existing.parent()) {
            (Some(_), Some(parent)) => {
                tail.push(existing.components().next_back().unwrap());
                existing = parent;
            }
            _ => break,
        }
    }
    if tail.iter().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(refuse("climbs out with '..'"));
    }
    let real = existing.canonicalize().map_err(|_| refuse("cannot be resolved"))?;
    if !real.starts_with(root) {
        return Err(refuse("is outside the root"));
    }
    Ok(())
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Turn a tool call into the command line it mirrors and run it.
fn call(root: &Path, tool: &Tool, args: &Map<String, Value>) -> Result<crate::envelope::Output, AppError> {
    let bad = |name: &str, want: &str| AppError::usage("invalid-argument", format!("Argument '{name}' of {} must be {want}.", tool.name));
    for k in args.keys() {
        if !all_args(tool).any(|a| a.name == k) {
            return Err(AppError::usage("unknown-field", format!("Unknown argument '{k}' for {}.", tool.name)).path(k.clone()).suggest(did_you_mean(k, all_args(tool).map(|a| a.name))));
        }
    }
    let mut argv: Vec<String> = vec!["svgdog".into(), "--format".into(), "json".into(), "--quiet".into(), tool.command.into()];
    let mut inline_ops = None;
    for a in all_args(tool) {
        let value = match args.get(a.name) {
            None | Some(Value::Null) if a.required => return Err(AppError::usage("missing-argument", format!("{} needs '{}'.", tool.name, a.name))),
            None | Some(Value::Null) => {
                // No output for a preview means: hand the image back.
                if tool.command == "preview" && a.name == "output" {
                    argv.extend(["-o".into(), "-".into()]);
                }
                continue;
            }
            Some(v) => v,
        };
        let strings = |v: &Value| -> Result<Vec<String>, AppError> {
            let list = v.as_array().ok_or_else(|| bad(a.name, "a list of strings"))?;
            list.iter().map(|s| s.as_str().map(|s| s.to_string()).ok_or_else(|| bad(a.name, "a list of strings"))).collect()
        };
        let mut push = |values: Vec<String>| match a.flag {
            Some(f) if matches!(a.kind, Kind::StrList) && f != "--set" => argv.extend([f.to_string(), values.join(",")]),
            Some(f) => values.into_iter().for_each(|v| argv.extend([f.to_string(), v])),
            None => argv.extend(values),
        };
        match a.kind {
            Kind::Path => {
                let s = value.as_str().ok_or_else(|| bad(a.name, "a string"))?;
                confine(root, s, a.name)?;
                push(vec![s.to_string()]);
            }
            Kind::PathList => {
                let list = strings(value)?;
                // Globs are expanded here so every file they reach is checked.
                let files = if tool.command == "validate" { commands::validate::expand(&list)? } else { list };
                for f in &files {
                    confine(root, f, a.name)?;
                }
                push(files);
            }
            Kind::Str => push(vec![value.as_str().ok_or_else(|| bad(a.name, "a string"))?.to_string()]),
            Kind::StrList => {
                let list = strings(value)?;
                if !list.is_empty() {
                    push(list);
                }
            }
            Kind::Int => push(vec![value.as_u64().ok_or_else(|| bad(a.name, "a whole number"))?.to_string()]),
            Kind::Bool => {
                if value.as_bool().ok_or_else(|| bad(a.name, "true or false"))? {
                    argv.push(a.flag.unwrap().to_string());
                }
            }
            Kind::Json => inline_ops = Some(value.clone()),
        }
    }
    let cli = Cli::try_parse_from(&argv).map_err(|e| crate::usage_error(e, &argv))?;
    execute(&cli, &Env { color: false, ceiling: Some(root.to_path_buf()), inline_ops })
}

fn tool_result(root: &Path, name: &str, args: &Map<String, Value>) -> Option<Value> {
    let tool = TOOLS.iter().find(|t| t.name == name)?;
    let outcome = std::panic::catch_unwind(|| call(root, tool, args)).unwrap_or_else(|_| Err(AppError::new(EXIT_INTERNAL, "internal-error", "svgdog hit a bug while running this tool. Please report it with the arguments and the input file.")));
    Some(match outcome {
        Ok(out) => {
            let mut content = Vec::new();
            if let Some(raw) = &out.raw {
                content.push(json!({ "type": "image", "data": base64(raw), "mimeType": "image/png" }));
            }
            content.push(json!({ "type": "text", "text": out.json.to_string() }));
            json!({ "content": content, "structuredContent": out.json, "isError": false })
        }
        Err(e) => {
            let env = e.to_json(tool.command);
            json!({ "content": [{ "type": "text", "text": env.to_string() }], "structuredContent": env, "isError": true })
        }
    })
}

fn rpc_error(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut err = json!({ "code": code, "message": message });
    if let Some(d) = data {
        err["data"] = d;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": err })
}

fn server_info() -> Value {
    json!({ "name": "svgdog", "version": VERSION })
}

/// Answer one message; `None` for notifications, which get no reply.
fn handle(root: &Path, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let Some(method) = msg.get("method").and_then(|m| m.as_str()) else {
        // A response to something we never asked, or not a request at all.
        return msg.get("result").or(msg.get("error")).is_none().then(|| rpc_error(id.unwrap_or(Value::Null), -32600, "Invalid request: no method", None));
    };
    let id = id?;
    let params = msg.get("params").cloned().unwrap_or(json!({}));
    let meta = params.get("_meta").cloned().unwrap_or(json!({}));

    // A request that states its version is in the per-request style and must name one we serve.
    let modern = match meta.get(META_VERSION).and_then(|v| v.as_str()) {
        Some(v) if MODERN.contains(&v) => true,
        Some(v) => return Some(rpc_error(id, -32022, "Unsupported protocol version", Some(json!({ "supported": MODERN, "requested": v })))),
        None => false,
    };
    if modern && method != "server/discover" && meta.get(META_CAPS).is_none() {
        return Some(rpc_error(id, -32602, "Invalid params: _meta is missing io.modelcontextprotocol/clientCapabilities", None));
    }
    let finish = |mut result: Value| {
        if modern || method == "server/discover" {
            result["resultType"] = json!("complete");
            result["_meta"] = json!({ META_SERVER: server_info() });
        }
        Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    };
    match method {
        "server/discover" => finish(json!({ "supportedVersions": MODERN, "capabilities": { "tools": {} }, "instructions": INSTRUCTIONS })),
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(|v| v.as_str()).unwrap_or("");
            let version = if LEGACY.contains(&asked) { asked } else { LEGACY[0] };
            finish(json!({ "protocolVersion": version, "capabilities": { "tools": {} }, "serverInfo": server_info(), "instructions": INSTRUCTIONS }))
        }
        "ping" => finish(json!({})),
        "tools/list" => finish(json!({ "tools": TOOLS.iter().map(tool_json).collect::<Vec<_>>() })),
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let empty = Map::new();
            let args = match params.get("arguments") {
                None | Some(Value::Null) => &empty,
                Some(Value::Object(m)) => m,
                Some(_) => return Some(rpc_error(id, -32602, "Invalid params: arguments must be an object", None)),
            };
            match tool_result(root, name, args) {
                Some(result) => finish(result),
                None => {
                    let hint = did_you_mean(name, TOOLS.iter().map(|t| t.name)).map(|s| format!(" {s}")).unwrap_or_default();
                    Some(rpc_error(id, -32602, &format!("Unknown tool: {name}.{hint}"), None))
                }
            }
        }
        other => Some(rpc_error(id, -32601, &format!("Method not found: {other}"), None)),
    }
}

/// Serve until stdin closes. Messages are JSON objects, one per line.
pub fn serve(root: &Path) -> Result<(), AppError> {
    let root: PathBuf = root.canonicalize().map_err(|e| AppError::input("root-not-found", format!("Cannot use {} as the root directory: {e}.", root.display())).path(root.display().to_string()))?;
    if !root.is_dir() {
        return Err(AppError::input("root-not-found", format!("{} is not a directory.", root.display())).path(root.display().to_string()));
    }
    // Relative paths in tool calls are relative to the root.
    std::env::set_current_dir(&root).map_err(|e| AppError::input("root-not-found", format!("Cannot enter {}: {e}.", root.display())))?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Object(m)) => handle(&root, &Value::Object(m)),
            Ok(_) => Some(rpc_error(Value::Null, -32600, "Invalid request: expected one JSON object per line", None)),
            Err(e) => Some(rpc_error(Value::Null, -32700, &format!("Parse error: {e}"), None)),
        };
        if let Some(r) = reply
            && (writeln!(stdout, "{r}").is_err() || stdout.flush().is_err())
        {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        for (raw, enc) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(raw.as_bytes()), enc);
        }
    }

    #[test]
    fn every_command_has_a_tool_and_every_tool_argument_is_a_real_flag() {
        let names: Vec<&str> = TOOLS.iter().map(|t| t.command).collect();
        assert_eq!(names, vec!["render", "validate", "inspect", "preview", "edit", "fix", "schema", "rules"]);
        use clap::CommandFactory;
        let cmd = Cli::command();
        for t in TOOLS {
            let sub = cmd.find_subcommand(t.command).unwrap();
            for a in all_args(t) {
                if let Some(flag) = a.flag {
                    let known = sub.get_arguments().chain(cmd.get_arguments()).any(|x| x.get_long().map(|l| format!("--{l}")) == Some(flag.to_string()) || x.get_short().map(|s| format!("-{s}")) == Some(flag.to_string()));
                    assert!(known, "{}: {flag} is not a flag of {}", t.name, t.command);
                }
            }
        }
    }
}

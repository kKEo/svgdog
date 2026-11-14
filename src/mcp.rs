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
const INSTRUCTIONS: &str = "SVGDog compiles diagram specs to SVG and checks any SVG. Typical loop: svgdog_examples for a spec to start from (svgdog_schema, name: spec, has every field), then svgdog_render with the spec as text and preview: true, read the findings and look at the image, svgdog_fix for those marked fixable or change the spec, repeat until there are no errors; svgdog_preview with annotate for a look. All paths are relative to the server's root directory.";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Path,
    PathList,
    Str,
    StrList,
    Bool,
    Int,
    Json,
    /// Content handed over inline, not a path.
    Text,
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
        about: "Compile a diagram spec (YAML or JSON) to SVG and validate the result. Give the spec as text in 'spec' or as a path in 'file'. Without 'output' the SVG comes back in the envelope's 'svg' field; with it, the file is written. Returns the envelope with findings.",
        read_only: false,
        config: true,
        args: &[
            arg("spec", Kind::Text, None, false, "The diagram spec itself, as YAML or JSON text (see svgdog_examples)"),
            arg("file", Kind::Path, None, false, "Diagram spec file, in place of 'spec'"),
            arg("output", Kind::Path, Some("-o"), false, "Where to write the SVG; omit to get it back in the result"),
            arg("preview", Kind::Bool, None, false, "Also return a PNG image of the result, to look at"),
            arg("theme", Kind::Str, Some("--theme"), false, "Built-in theme: docs, blueprint, paper, bold or cards; add -dark, or -auto to follow the reader's light or dark mode. Or a theme folder inside the root, such as ./themes/acme (add #dark or #auto). svgdog_theme_list describes them. Default: the spec's, else svgdog.toml's, else docs-light"),
            arg("interactive", Kind::Str, Some("--interactive"), false, "css: one SVG that highlights related things on hover and focus. html: a self-contained page with step controls and details. Default none"),
            arg("embed_spec", Kind::Bool, Some("--embed-spec"), false, "Store the spec inside the SVG, so svgdog_describe can read the drawing back"),
            arg("check", Kind::Bool, Some("--check"), false, "Write nothing; report whether 'output' is what the spec renders now"),
            arg("frames", Kind::Bool, Some("--frames"), false, "Write one picture per step of the spec's 'steps', each with its step in focus; needs 'output'"),
            arg("previous", Kind::Path, Some("--previous"), false, "The SVG rendered from the earlier spec: boxes in both stay where they were, new ones go into free room. May be the same file as 'output'. Fails with layout-changed when boxes cannot stay"),
            arg("no_validate", Kind::Bool, Some("--no-validate"), false, "Skip the checks on the result"),
            arg("force", Kind::Bool, Some("--force"), false, "Replace an existing output file; with 'previous', lay out afresh when the layout cannot be kept"),
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
            arg("step", Kind::Int, Some("--step"), false, "Show the drawing with this step of its walk-through chosen, the rest faded"),
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
    Tool {
        name: "svgdog_docs",
        command: "docs",
        about: "Render the diagrams written inside Markdown pages (fenced blocks with the info word svgdog) to SVG files next to each page, and keep the image line after each block pointing at its picture. With check, write nothing and report what is out of date.",
        read_only: false,
        config: true,
        args: &[
            arg("paths", Kind::PathList, None, true, "Markdown files, or folders to search for them"),
            arg("check", Kind::Bool, Some("--check"), false, "Write nothing; report what is out of date"),
            arg("assets", Kind::Str, Some("--assets"), false, "Folder for the pictures, next to each page; default diagrams"),
            arg("theme", Kind::Str, Some("--theme"), false, "A built-in theme (docs, blueprint, paper, bold, cards, cards; add -dark or -auto) or a theme folder, for every diagram"),
            arg("force", Kind::Bool, Some("--force"), false, "A picture that exists keeps its boxes in place when its diagram changes; with force, a diagram whose boxes cannot stay is laid out afresh in place of failing"),
            arg("interactive", Kind::Str, Some("--interactive"), false, "css or html; default none"),
        ],
    },
    Tool {
        name: "svgdog_describe",
        command: "describe",
        about: "Say in words what a diagram shows: its nodes, connections, notes and steps. Takes a spec file, or an SVG rendered with embed_spec; the answer also carries the spec itself.",
        read_only: true,
        config: false,
        args: &[arg("file", Kind::Path, None, true, "Diagram spec, or an SVG rendered with embed_spec"), arg("short", Kind::Bool, Some("--short"), false, "One or two sentences, as for alt text")],
    },
    Tool {
        name: "svgdog_theme_list",
        command: "theme list",
        about: "List the built-in themes with their colours, fonts, sizes and line styles. Use a name as 'theme' in svgdog_render or in a spec.",
        read_only: true,
        config: false,
        args: &[],
    },
    Tool {
        name: "svgdog_theme_init",
        command: "theme init",
        about: "Start a theme of your own: write a theme.toml, copied from a built-in theme, into a folder. Change the file, check it with svgdog_theme_check, and use the folder as 'theme' (for example ./themes/acme).",
        read_only: false,
        config: false,
        args: &[arg("dir", Kind::Path, None, true, "Folder for the theme; created when missing"), arg("from", Kind::Str, Some("--from"), false, "Built-in theme to copy: docs, blueprint, paper, bold or cards; default docs"), arg("force", Kind::Bool, Some("--force"), false, "Replace a theme.toml that is already there")],
    },
    Tool {
        name: "svgdog_theme_check",
        command: "theme check",
        about: "Check a theme folder: that its theme.toml is valid, that colours which sit on each other have enough contrast, and that the bundled examples drawn with it have no findings.",
        read_only: true,
        config: true,
        args: &[arg("dir", Kind::Path, None, true, "Folder that holds the theme.toml")],
    },
    Tool {
        name: "svgdog_examples",
        command: "examples",
        about: "List the bundled example specs, or return one in full to start from. Each is small and renders without findings.",
        read_only: true,
        config: false,
        args: &[arg("name", Kind::Str, None, false, "Example to return; omit to list them")],
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
            Kind::Path | Kind::Str | Kind::Text => json!({ "type": "string", "description": a.about }),
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
    let mut argv: Vec<String> = vec!["svgdog".into(), "--format".into(), "json".into(), "--quiet".into()];
    // A tool for a subcommand names both words.
    argv.extend(tool.command.split(' ').map(String::from));
    let mut inline_ops = None;
    let mut inline_spec = None;
    let render = tool.command == "render";
    let given = |name: &str| args.get(name).is_some_and(|v| !v.is_null());
    if render {
        match (given("spec"), given("file")) {
            (true, true) => return Err(AppError::usage("argument-conflict", "Give 'spec' (the text) or 'file' (a path), not both.")),
            (false, false) => return Err(AppError::usage("missing-argument", "svgdog_render needs 'spec' (the text) or 'file' (a path).").suggest(Some("svgdog_examples returns specs to start from.".into()))),
            _ => {}
        }
    }
    for a in all_args(tool) {
        let value = match args.get(a.name) {
            None | Some(Value::Null) if a.required => return Err(AppError::usage("missing-argument", format!("{} needs '{}'.", tool.name, a.name))),
            None | Some(Value::Null) => {
                // No output for a preview means: hand the image back.
                if (tool.command == "preview" || render) && a.name == "output" {
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
                // A switch without a flag is handled here, after the command has run.
                if value.as_bool().ok_or_else(|| bad(a.name, "true or false"))?
                    && let Some(f) = a.flag
                {
                    argv.push(f.to_string());
                }
            }
            Kind::Json => inline_ops = Some(value.clone()),
            Kind::Text => {
let text = value.as_str().ok_or_else(|| bad(a.name, "text"))?;
                let t = text.trim();
                if !t.contains('\n') && [".yaml", ".yml", ".json"].iter().any(|ext| t.ends_with(ext)) {
                    return Err(AppError::usage("invalid-argument", format!("'spec' is the spec text itself, but '{t}' looks like a path.")).suggest(Some(format!("Pass it as 'file': \"{t}\"."))));
                }
                inline_spec = Some(text.to_string());
                argv.push("-".into());
            }
        }
    }
    let cli = Cli::try_parse_from(&argv).map_err(|e| crate::usage_error(e, &argv))?;
    let mut out = execute(&cli, &Env { color: false, ceiling: Some(root.to_path_buf()), inline_ops, inline_spec })?;
    if render {
        // Hand the drawing back as data: the SVG text when no file was asked for, and a picture on request.
        let svg = match out.raw.take() {
            Some(bytes) => {
                let text = String::from_utf8(bytes).unwrap_or_default();
                out.json["svg"] = json!(text);
                text
            }
            None => args.get("output").and_then(|v| v.as_str()).and_then(|p| std::fs::read_to_string(root.join(p)).ok()).unwrap_or_default(),
        };
        if args.get("preview").and_then(|v| v.as_bool()) == Some(true) {
            let shot = commands::preview::picture(&svg)?;
            out.json["image"] = json!({ "width": shot.width, "height": shot.height, "scale": crate::util::num(shot.scale), "region": crate::rules::bbox_json(shot.region), "background": "light", "annotated": 0 });
            out.raw = Some(shot.png);
        }
    }
    Ok(out)
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
        assert_eq!(names, vec!["render", "validate", "inspect", "preview", "edit", "fix", "schema", "rules", "docs", "describe", "theme list", "theme init", "theme check", "examples"]);
        use clap::CommandFactory;
        let cmd = Cli::command();
        for t in TOOLS {
            // A tool for a subcommand names both words.
            let sub = t.command.split(' ').fold(&cmd, |c, word| c.find_subcommand(word).unwrap());
            for a in all_args(t) {
                if let Some(flag) = a.flag {
                    let known = sub.get_arguments().chain(cmd.get_arguments()).any(|x| x.get_long().map(|l| format!("--{l}")) == Some(flag.to_string()) || x.get_short().map(|s| format!("-{s}")) == Some(flag.to_string()));
                    assert!(known, "{}: {flag} is not a flag of {}", t.name, t.command);
                }
            }
        }
    }
}

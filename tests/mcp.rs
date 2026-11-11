//! The MCP server, driven over stdio the way a client would.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_svgdog");

struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Server {
    fn start(root: &Path) -> Server {
        let mut child = Command::new(BIN).args(["mcp", "--root", root.to_str().unwrap()]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Server { child, stdin, stdout, next: 0 }
    }

    fn send_raw(&mut self, line: &str) {
        let s = self.stdin.as_mut().unwrap();
        s.write_all(line.as_bytes()).unwrap();
        s.write_all(b"\n").unwrap();
        s.flush().unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("not a JSON line ({e}): {line:?}"))
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send_raw(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string());
        let reply = self.read();
        assert_eq!(reply["id"], id, "{reply}");
        assert_eq!(reply["jsonrpc"], "2.0");
        reply
    }

    /// Call a tool and return (envelope, isError, whole result).
    fn call(&mut self, name: &str, args: Value) -> (Value, bool, Value) {
        let reply = self.request("tools/call", json!({ "name": name, "arguments": args }));
        let result = reply["result"].clone();
        assert!(result.is_object(), "{reply}");
        let text = result["content"].as_array().unwrap().iter().find(|c| c["type"] == "text").unwrap()["text"].as_str().unwrap().to_string();
        let envelope: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(envelope, result["structuredContent"], "text and structured content agree");
        (envelope, result["isError"].as_bool().unwrap(), result)
    }

    /// Close stdin and wait: the server must end on its own.
    fn finish(mut self) -> i32 {
        drop(self.stdin.take());
        self.child.wait().unwrap().code().unwrap()
    }
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for f in ["broken.svg", "orders.svg", "orders.yaml"] {
        fs::copy(fixtures.join(f), dir.path().join(f)).unwrap();
    }
    dir
}

fn schema(name: &str) -> jsonschema::Validator {
    let out = Command::new(BIN).args(["schema", name]).output().unwrap();
    jsonschema::validator_for(&serde_json::from_slice(&out.stdout).unwrap()).unwrap()
}

#[test]
fn handshake_lists_one_tool_per_command() {
    let dir = project();
    let mut s = Server::start(dir.path());
    let init = s.request("initialize", json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(init["result"]["serverInfo"]["name"], "svgdog");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    // a version we do not know is answered with the newest handshake version we do
    let other = s.request("initialize", json!({ "protocolVersion": "1999-01-01", "capabilities": {} }));
    assert_eq!(other["result"]["protocolVersion"], "2025-11-25");
    // notifications get no reply: the next line read belongs to the next request
    s.send_raw(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string());
    assert_eq!(s.request("ping", json!({}))["result"], json!({}));

    let tools = s.request("tools/list", json!({}))["result"]["tools"].as_array().unwrap().clone();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["svgdog_render", "svgdog_validate", "svgdog_inspect", "svgdog_preview", "svgdog_edit", "svgdog_fix", "svgdog_schema", "svgdog_rules", "svgdog_docs", "svgdog_describe", "svgdog_examples"]);
    for t in &tools {
        assert!(!t["description"].as_str().unwrap().is_empty());
        // every input schema is itself a valid JSON Schema
        jsonschema::validator_for(&t["inputSchema"]).unwrap_or_else(|e| panic!("{}: {e}", t["name"]));
        assert_eq!(t["inputSchema"]["type"], "object");
    }
    assert_eq!(s.finish(), 0);
}

#[test]
fn tools_return_the_same_envelope_as_the_cli() {
    let dir = project();
    let mut s = Server::start(dir.path());
    let findings = schema("findings");
    let check = |v: &Value| assert!(findings.is_valid(v), "{:#?}\n{v:#}", findings.iter_errors(v).map(|e| e.to_string()).collect::<Vec<_>>());

    let (v, is_error, _) = s.call("svgdog_validate", json!({ "files": ["broken.svg"], "no_config": true, "max_findings": 500 }));
    assert!(!is_error, "findings are a result, not a tool error");
    check(&v);
    let cli = Command::new(BIN).args(["validate", "broken.svg", "--no-config", "--max-findings", "500"]).current_dir(dir.path()).output().unwrap();
    assert_eq!(v, serde_json::from_slice::<Value>(&cli.stdout).unwrap(), "the tool adds no behaviour");

    let (r, is_error, _) = s.call("svgdog_render", json!({ "file": "orders.yaml", "output": "out/o.svg" }));
    assert!(is_error, "the output directory does not exist: {r}");
    let (r, is_error, _) = s.call("svgdog_render", json!({ "file": "orders.yaml", "output": "o.svg" }));
    assert!(!is_error);
    check(&r);
    assert_eq!((r["ok"].as_bool(), r["outputs"][0]["path"].as_str()), (Some(true), Some("o.svg")));
    assert!(dir.path().join("o.svg").exists());
    // the same spec over its own drawing: the boxes are kept, and the file may be replaced without force
    let (k, is_error, _) = s.call("svgdog_render", json!({ "file": "orders.yaml", "output": "o.svg", "previous": "o.svg" }));
    assert!(!is_error, "{k}");
    check(&k);
    assert_eq!((k["layout"]["kept"].as_u64(), k["layout"]["relaid"].as_bool()), (Some(4), Some(false)), "{k}");
    let (k, is_error, _) = s.call("svgdog_render", json!({ "file": "orders.yaml", "output": "o2.svg", "previous": "../o.svg" }));
    assert!(is_error && k["error"]["code"] == "outside-root", "the earlier drawing is confined to the root like any path: {k}");

    let (scene, _, _) = s.call("svgdog_inspect", json!({ "file": "o.svg", "select": "#node-api", "fields": ["tag", "bbox", "text"] }));
    assert_eq!(scene["summary"]["returned"], 3);
    assert!(schema("scene").is_valid(&scene));

    let (fixed, _, _) = s.call("svgdog_fix", json!({ "file": "broken.svg", "dry_run": true, "no_config": true }));
    check(&fixed);
    assert!(fixed["changed"] == true && fixed["dry_run"] == true && !fixed["fixed"].as_array().unwrap().is_empty());

    // edit takes its operations as data; a finding's fix object can be passed straight in
    let fix = v["findings"].as_array().unwrap().iter().find(|f| f["rule"] == "unused-def").unwrap()["fix"].clone();
    let (edited, is_error, _) = s.call("svgdog_edit", json!({ "file": "broken.svg", "ops": fix, "output": "edited.svg", "no_config": true }));
    assert!(!is_error, "{edited}");
    assert_eq!(edited["operations"], json!({ "given": 1, "applied": 1 }));
    assert!(!fs::read_to_string(dir.path().join("edited.svg")).unwrap().contains("id=\"unused\""));
    let (set, _, _) = s.call("svgdog_edit", json!({ "file": "orders.yaml", "set": ["#api.label=Orders API", "#db.emphasis=true"], "dry_run": true }));
    assert_eq!(set["operations"]["applied"], 2);

    let (spec, _, _) = s.call("svgdog_schema", json!({ "name": "spec" }));
    assert_eq!(spec["title"], "svgdog diagram spec");
    let (rules, _, _) = s.call("svgdog_rules", json!({}));
    assert_eq!(rules["rules"].as_array().unwrap().len(), 38);
    let (one, _, _) = s.call("svgdog_rules", json!({ "explain": "text-overflow" }));
    assert_eq!(one["rule"]["id"], "text-overflow");
    assert_eq!(s.finish(), 0);
}

#[test]
fn preview_returns_the_png_as_image_content() {
    let dir = project();
    let mut s = Server::start(dir.path());
    let (v, is_error, result) = s.call("svgdog_preview", json!({ "file": "orders.svg", "width": 400, "annotate": true }));
    assert!(!is_error);
    assert_eq!((v["image"]["width"].as_u64(), v["outputs"].as_array().unwrap().len()), (Some(400), 0));
    let image = &result["content"][0];
    assert_eq!((image["type"].as_str(), image["mimeType"].as_str()), (Some("image"), Some("image/png")));
    let data = image["data"].as_str().unwrap();
    // "iVBORw0KGgo" is the base64 of the PNG signature
    assert!(data.starts_with("iVBORw0KGgo") && data.len() > 1000 && data.len() % 4 == 0);
    // with an output path the file is written and no image is sent back
    let (w, _, result) = s.call("svgdog_preview", json!({ "file": "orders.svg", "output": "p.png" }));
    assert_eq!(w["outputs"][0]["path"], "p.png");
    assert_eq!(result["content"].as_array().unwrap().len(), 1);
    assert_eq!(&fs::read(dir.path().join("p.png")).unwrap()[1..4], b"PNG");
    assert_eq!(s.finish(), 0);
}

#[test]
fn paths_outside_the_root_are_refused() {
    let outer = tempfile::tempdir().unwrap();
    let root = outer.path().join("docs");
    fs::create_dir_all(root.join("fig")).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::copy(fixtures.join("broken.svg"), root.join("fig/b.svg")).unwrap();
    fs::copy(fixtures.join("broken.svg"), outer.path().join("secret.svg")).unwrap();
    // a config above the root must not be picked up
    fs::write(outer.path().join("svgdog.toml"), "[rules]\ntitle-missing = \"off\"\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(outer.path().join("secret.svg"), root.join("link.svg")).unwrap();

    let mut s = Server::start(&root);
    let secret = outer.path().join("secret.svg");
    let escapes: Vec<(&str, Value)> = vec![
        ("svgdog_validate", json!({ "files": ["../secret.svg"] })),
        ("svgdog_validate", json!({ "files": [secret.to_str().unwrap()] })),
        ("svgdog_validate", json!({ "files": ["fig/../../secret.svg"] })),
        ("svgdog_validate", json!({ "files": ["../*.svg"] })),
        ("svgdog_inspect", json!({ "file": "../secret.svg" })),
        ("svgdog_fix", json!({ "file": "fig/b.svg", "output": "../stolen.svg" })),
        ("svgdog_edit", json!({ "file": "fig/b.svg", "set": ["svg.id=x"], "output": "../new-dir/x.svg" })),
        ("svgdog_preview", json!({ "file": "fig/b.svg", "output": "../p.png" })),
        ("svgdog_validate", json!({ "files": ["fig/b.svg"], "config": "../svgdog.toml" })),
        #[cfg(unix)]
        ("svgdog_validate", json!({ "files": ["link.svg"] })),
    ];
    for (tool, args) in escapes {
        let (v, is_error, _) = s.call(tool, args.clone());
        assert!(is_error, "{tool} {args} was allowed: {v}");
        assert_eq!(v["error"]["code"], "outside-root", "{tool} {args}: {v}");
    }
    assert!(!outer.path().join("stolen.svg").exists() && !outer.path().join("p.png").exists());
    // stdin and stdout belong to the protocol
    let (v, is_error, _) = s.call("svgdog_validate", json!({ "files": ["-"] }));
    assert!(is_error && v["error"]["code"] == "invalid-argument", "{v}");

    // inside the root everything works, and the parent's config was not read
    let (v, is_error, _) = s.call("svgdog_validate", json!({ "files": ["fig/*.svg"], "rules": ["title-missing"] }));
    assert!(!is_error);
    assert_eq!(v["summary"]["errors"], 1, "the svgdog.toml above the root must be ignored: {v}");
    assert_eq!(s.finish(), 0);
}

#[test]
fn bad_calls_fail_clearly_and_the_server_keeps_going() {
    let dir = project();
    let mut s = Server::start(dir.path());
    // tool-level mistakes come back as results the model can read and correct
    let (v, is_error, _) = s.call("svgdog_validate", json!({ "files": ["broken.svg"], "fail_no": "warning" }));
    assert!(is_error);
    assert_eq!((v["error"]["code"].as_str(), v["error"]["suggestion"].as_str()), (Some("unknown-field"), Some("Did you mean 'fail_on'?")));
    let (v, is_error, _) = s.call("svgdog_validate", json!({}));
    assert!(is_error && v["error"]["code"] == "missing-argument");
    let (v, is_error, _) = s.call("svgdog_validate", json!({ "files": "broken.svg" }));
    assert!(is_error && v["error"]["code"] == "invalid-argument");
    let (v, is_error, _) = s.call("svgdog_validate", json!({ "files": ["broken.svg"], "rules": ["text-overflw"] }));
    assert!(is_error && v["error"]["suggestion"] == "Did you mean 'text-overflow'?");
    let (v, is_error, _) = s.call("svgdog_inspect", json!({ "file": "missing.svg" }));
    assert!(is_error && v["error"]["code"] == "file-not-found");

    // protocol-level mistakes are JSON-RPC errors
    let unknown = s.request("tools/call", json!({ "name": "svgdog_validat", "arguments": {} }));
    assert_eq!(unknown["error"]["code"], -32602);
    assert!(unknown["error"]["message"].as_str().unwrap().contains("svgdog_validate"));
    assert_eq!(s.request("resources/list", json!({}))["error"]["code"], -32601);
    s.send_raw("this is not json");
    assert_eq!(s.read()["error"]["code"], -32700);
    s.send_raw("[1, 2]");
    assert_eq!(s.read()["error"]["code"], -32600);
    // and the server still answers
    assert_eq!(s.request("ping", json!({}))["result"], json!({}));
    assert_eq!(s.finish(), 0);
}

#[test]
fn per_request_protocol_version_is_served_without_a_handshake() {
    let dir = project();
    let mut s = Server::start(dir.path());
    let meta = |v: &str| json!({ "io.modelcontextprotocol/protocolVersion": v, "io.modelcontextprotocol/clientInfo": { "name": "t", "version": "0" }, "io.modelcontextprotocol/clientCapabilities": {} });

    let d = s.request("server/discover", json!({ "_meta": meta("2026-07-28") }));
    let r = &d["result"];
    assert_eq!((r["resultType"].as_str(), r["supportedVersions"][0].as_str()), (Some("complete"), Some("2026-07-28")));
    assert!(r["capabilities"]["tools"].is_object());
    assert_eq!(r["_meta"]["io.modelcontextprotocol/serverInfo"]["name"], "svgdog");

    // no initialize: a request states its own version
    let list = s.request("tools/list", json!({ "_meta": meta("2026-07-28") }));
    assert_eq!(list["result"]["resultType"], "complete");
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 11);
    let call = s.request("tools/call", json!({ "name": "svgdog_rules", "arguments": {}, "_meta": meta("2026-07-28") }));
    assert_eq!(call["result"]["resultType"], "complete");
    assert_eq!(call["result"]["structuredContent"]["rules"].as_array().unwrap().len(), 38);

    let old = s.request("tools/list", json!({ "_meta": meta("1900-01-01") }));
    assert_eq!(old["error"]["code"], -32022);
    assert_eq!(old["error"]["data"], json!({ "supported": ["2026-07-28"], "requested": "1900-01-01" }));
    let bare = s.request("tools/list", json!({ "_meta": { "io.modelcontextprotocol/protocolVersion": "2026-07-28" } }));
    assert_eq!(bare["error"]["code"], -32602, "client capabilities are required on every request");
    assert_eq!(s.finish(), 0);
}

#[test]
fn a_missing_root_is_an_input_error() {
    let out = Command::new(BIN).args(["mcp", "--root", "/no/such/dir/anywhere"]).stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"]["code"], "root-not-found");
    assert_eq!(Command::new(BIN).arg("mcp").stdin(Stdio::null()).output().unwrap().status.code(), Some(2));
}

#[test]
fn render_takes_the_spec_as_text_and_returns_the_drawing() {
    let dir = project();
    let mut s = Server::start(dir.path());
    let spec = "version: 1\ndiagram: flow\ntitle: Request path\ndescription: A browser calls the API.\nedges:\n  - \"browser -> api: request\"\n  - \"api --> queue\"\n";
    let (v, is_error, result) = s.call("svgdog_render", json!({ "spec": spec, "preview": true }));
    assert!(!is_error, "{v:#}");
    assert_eq!((v["ok"].as_bool(), v["summary"]["errors"].as_u64()), (Some(true), Some(0)), "{v:#}");
    let svg = v["svg"].as_str().unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("id=\"node-browser\"") && svg.contains("id=\"node-queue\""), "{svg}");
    assert!(v.get("outputs").is_none_or(|o| o.as_array().is_some_and(|a| a.is_empty())), "nothing is written: {v:#}");
    schema("findings").validate(&v).unwrap_or_else(|e| panic!("{e}"));
    let image = &result["content"][0];
    assert_eq!((image["type"].as_str(), image["mimeType"].as_str()), (Some("image"), Some("image/png")));
    assert!(image["data"].as_str().unwrap().starts_with("iVBORw0KGgo"));
    assert!(v["image"]["width"].as_u64().unwrap() > 100);

    // the same spec from a file, written to a file: no svg field, and a picture only on request
    fs::write(dir.path().join("p.yaml"), spec).unwrap();
    let (w, _, result) = s.call("svgdog_render", json!({ "file": "p.yaml", "output": "p.svg" }));
    assert_eq!(w["outputs"][0]["path"], "p.svg");
    assert!(w.get("svg").is_none() && result["content"].as_array().unwrap().len() == 1);
    assert_eq!(fs::read_to_string(dir.path().join("p.svg")).unwrap(), svg, "inline and file specs give the same bytes");

    // one of the two is needed, and only one
    let (e, is_error, _) = s.call("svgdog_render", json!({}));
    assert!(is_error && e["error"]["code"] == "missing-argument", "{e:#}");
    let (e, is_error, _) = s.call("svgdog_render", json!({ "spec": spec, "file": "p.yaml" }));
    assert!(is_error && e["error"]["code"] == "argument-conflict", "{e:#}");
    // a path in the wrong argument is caught, not parsed as a spec
    let (e, is_error, _) = s.call("svgdog_render", json!({ "spec": "p.yaml" }));
    assert!(is_error && e["error"]["suggestion"].as_str().unwrap().contains("'file'"), "{e:#}");
    // a spec error comes back as a tool error with its path
    let (e, is_error, _) = s.call("svgdog_render", json!({ "spec": "version: 1\ndiagram: flow\ntitle: T\nedges: [\"a => b\"]\n" }));
    assert!(is_error && e["error"]["code"] == "invalid-value" && e["error"]["path"] == "edges[0]", "{e:#}");
    assert_eq!(s.finish(), 0);
}

#[test]
fn examples_are_listed_and_returned() {
    let dir = project();
    let mut s = Server::start(dir.path());
    let (v, is_error, _) = s.call("svgdog_examples", json!({}));
    assert!(!is_error);
    let names: Vec<&str> = v["examples"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"minimal") && names.contains(&"sequence"), "{names:?}");
    let (one, _, _) = s.call("svgdog_examples", json!({ "name": "minimal" }));
    let text = one["examples"][0]["spec"].as_str().unwrap().to_string();
    // what comes back renders as it is
    let (r, is_error, _) = s.call("svgdog_render", json!({ "spec": text }));
    assert!(!is_error && r["summary"]["errors"] == 0 && r["summary"]["warnings"] == 0, "{r:#}");
    assert_eq!(s.finish(), 0);
}

#[test]
fn docs_and_describe_work_inside_the_root() {
    let dir = project();
    fs::write(dir.path().join("page.md"), "# P\n\n```svgdog\nversion: 1\ndiagram: flow\ntitle: T\nedges: [\"a -> b\"]\n```\n").unwrap();
    let mut s = Server::start(dir.path());
    let (c, is_error, _) = s.call("svgdog_docs", json!({ "paths": ["page.md"], "check": true }));
    assert!(!is_error && c["ok"] == false && c["stale"].as_array().unwrap().len() == 2, "{c:#}");
    let (w, _, _) = s.call("svgdog_docs", json!({ "paths": ["."], "theme": "docs-auto" }));
    assert_eq!(w["outputs"].as_array().unwrap().len(), 2, "{w:#}");
    assert!(dir.path().join("diagrams/page-1.svg").exists());
    // the pictures cannot be sent outside the root
    let (e, is_error, _) = s.call("svgdog_docs", json!({ "paths": ["page.md"], "assets": "../elsewhere" }));
    assert!(is_error && e["error"]["code"] == "invalid-argument", "{e:#}");
    let (e, is_error, _) = s.call("svgdog_docs", json!({ "paths": ["../page.md"] }));
    assert!(is_error && e["error"]["code"] == "outside-root", "{e:#}");

    // render with the spec inside, then read the drawing back
    let (r, _, _) = s.call("svgdog_render", json!({ "file": "orders.yaml", "output": "o.svg", "embed_spec": true }));
    assert_eq!(r["ok"], true, "{r:#}");
    let (d, is_error, _) = s.call("svgdog_describe", json!({ "file": "o.svg", "short": true }));
    assert!(!is_error && d["spec"]["title"] == "Order placement" && d["short"].as_str().unwrap().starts_with("Flow diagram: Storefront to Order API"), "{d:#}");
    let (k, _, _) = s.call("svgdog_render", json!({ "file": "orders.yaml", "output": "o.svg", "embed_spec": true, "check": true }));
    assert_eq!((k["ok"].as_bool(), k["stale"].as_array().unwrap().len()), (Some(true), 0), "{k:#}");
    assert_eq!(s.finish(), 0);
}

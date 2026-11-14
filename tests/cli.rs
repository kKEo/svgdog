//! End-to-end tests of the binary, organised by the spec's agent requirements (R1 to R16)
//! where phase 1 covers them.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_svgdog");

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON object ({e}):\n{}", self.stdout))
    }
}

fn run_in(dir: &Path, args: &[&str], stdin: Option<&str>) -> Run {
    let mut cmd = Command::new(BIN);
    cmd.args(args).current_dir(dir).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().unwrap();
    if let Some(s) = stdin {
        child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    Run { code: out.status.code().unwrap(), stdout: String::from_utf8(out.stdout).unwrap(), stderr: String::from_utf8(out.stderr).unwrap() }
}

fn run(args: &[&str]) -> Run {
    run_in(Path::new(env!("CARGO_MANIFEST_DIR")), args, None)
}

fn rules_of(v: &Value) -> Vec<String> {
    v["findings"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

fn schema(name: &str) -> jsonschema::Validator {
    let s = run(&["schema", name]).json();
    jsonschema::validator_for(&s).unwrap()
}

fn assert_valid(v: &jsonschema::Validator, doc: &Value) {
    let errors: Vec<String> = v.iter_errors(doc).map(|e| format!("{e} at {}", e.instance_path())).collect();
    assert!(errors.is_empty(), "schema errors: {errors:#?}\n{doc:#}");
}

const CLEAN: &str = "tests/fixtures/orders.svg";
const BROKEN: &str = "tests/fixtures/broken.svg";

// ------------------------------------------------------------------ catalogue

#[test]
fn catalogue_has_38_rules_in_four_groups() {
    let v = run(&["rules"]).json();
    let rules = v["rules"].as_array().unwrap();
    let count = |g: &str| rules.iter().filter(|r| r["group"] == g).count();
    assert_eq!(rules.len(), 38);
    assert_eq!((count("structure"), count("geometry"), count("portability"), count("accessibility")), (10, 12, 10, 6));
}

/// Every rule's failing example raises the rule and its passing example does not.
#[test]
fn every_rule_example_fails_and_passes_as_documented() {
    let list = run(&["rules"]).json();
    for r in list["rules"].as_array().unwrap() {
        let id = r["id"].as_str().unwrap();
        let e = run(&["rules", "--explain", id]).json();
        let rule = &e["rule"];
        let dir = tempfile::tempdir().unwrap();
        let cfg = rule["example_config"].as_str().unwrap();
        if !cfg.is_empty() {
            fs::write(dir.path().join("svgdog.toml"), cfg).unwrap();
        }
        fs::write(dir.path().join("fail.svg"), rule["fail"].as_str().unwrap()).unwrap();
        fs::write(dir.path().join("pass.svg"), rule["pass"].as_str().unwrap()).unwrap();

        let fail = run_in(dir.path(), &["validate", "fail.svg", "--rules", id, "--quiet"], None);
        assert!(rules_of(&fail.json()).iter().any(|x| x == id), "{id}: failing example raised nothing\n{}", fail.stdout);
        let pass = run_in(dir.path(), &["validate", "pass.svg", "--rules", id, "--quiet"], None);
        assert_eq!(pass.code, 0, "{id}: passing example failed\n{}", pass.stdout);
        assert!(rules_of(&pass.json()).is_empty(), "{id}: passing example raised findings\n{}", pass.stdout);

        // A passing example is clean for every other default rule too, apart from polish.
        let all = run_in(dir.path(), &["validate", "pass.svg", "--quiet", "--skip", "desc-missing,excess-margin,background-missing,title-missing"], None);
        let noise: Vec<String> = rules_of(&all.json()).into_iter().filter(|x| x != id).collect();
        assert!(noise.is_empty() || id == "file-too-large", "{id}: passing example trips other rules: {noise:?}");
    }
}

#[test]
fn clean_diagram_has_no_errors_or_warnings() {
    let r = run(&["validate", CLEAN]);
    let v = r.json();
    assert_eq!(r.code, 0);
    assert_eq!(v["ok"], true);
    assert_eq!((v["summary"]["errors"].as_u64(), v["summary"]["warnings"].as_u64()), (Some(0), Some(0)), "{v:#}");
}

#[test]
fn broken_diagram_reports_expected_rules() {
    let r = run(&["validate", BROKEN, "--no-config"]);
    assert_eq!(r.code, 1);
    let got = rules_of(&r.json());
    for want in [
        "title-missing", "desc-missing", "fixed-size", "editor-metadata", "font-not-portable", "unused-def", "text-overflow",
        "content-clipped", "low-contrast", "stroke-too-thin", "duplicate-id", "edge-detached", "edge-through-node",
        "font-too-small", "dangling-reference", "script-content", "foreign-object",
    ] {
        assert!(got.iter().any(|g| g == want), "missing {want} in {got:?}");
    }
}

#[test]
fn geometry_findings_on_third_party_files_are_approximate_warnings() {
    let v = run(&["validate", BROKEN, "--no-config", "--rules", "text-overflow,font-too-small,edge-detached"]).json();
    let findings = v["findings"].as_array().unwrap();
    assert!(!findings.is_empty());
    for f in findings {
        assert_eq!(f["severity"], "warning", "{f:#}");
        assert_eq!(f["measured"]["approximate"], true, "{f:#}");
    }
}

// ------------------------------------------------------------ R1, R2, R3: I/O

#[test]
fn r1_every_command_completes_without_stdin_or_terminal() {
    for args in [vec!["validate", CLEAN], vec!["inspect", CLEAN], vec!["schema", "spec"], vec!["rules"], vec!["rules", "--explain", "palette"]] {
        let r = run(&args);
        assert_eq!(r.code, 0, "{args:?}: {}", r.stderr);
    }
}

#[test]
fn r2_output_validates_against_the_published_schemas() {
    let findings = schema("findings");
    let scene = schema("scene");
    assert_valid(&findings, &run(&["validate", CLEAN]).json());
    assert_valid(&findings, &run(&["validate", BROKEN, "--no-config"]).json());
    assert_valid(&findings, &run(&["validate", "nope.svg"]).json());
    assert_valid(&findings, &run(&["validate", "--bogus"]).json());
    assert_valid(&scene, &run(&["inspect", CLEAN]).json());
    assert_valid(&scene, &run(&["inspect", BROKEN, "--fields", "id,tag,bbox,text,fill,stroke,refs"]).json());
    assert_valid(&scene, &run(&["inspect", "nope.svg"]).json());
    for name in ["spec", "config", "findings", "scene", "ops"] {
        schema(name);
    }
    assert_valid(&findings, &run(&["edit", BROKEN, "--no-config", "--set", "#b > rect.rx=4", "--dry-run"]).json());
    assert_valid(&findings, &run(&["fix", BROKEN, "--no-config", "--unsafe", "--dry-run"]).json());
    assert_valid(&findings, &run(&["edit", BROKEN, "--set", "#nope.rx=4", "--dry-run"]).json());
}

#[test]
fn r3_stdout_is_json_with_and_without_quiet_and_has_no_ansi() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("svgdog.toml"), "version = 1\n").unwrap();
    fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join(BROKEN), dir.path().join("b.svg")).unwrap();
    let loud = run_in(dir.path(), &["validate", "b.svg"], None);
    let quiet = run_in(dir.path(), &["validate", "b.svg", "--quiet"], None);
    assert_eq!(loud.json(), quiet.json());
    assert!(loud.stderr.contains("config"));
    assert!(quiet.stderr.is_empty());
    let text = run_in(dir.path(), &["validate", "b.svg", "--format", "text"], None);
    assert!(!text.stdout.contains('\x1b') && !loud.stdout.contains('\x1b'));
    assert!(text.stdout.contains("text-overflow"));
}

#[test]
fn stdin_input_is_validated() {
    let svg = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(BROKEN)).unwrap();
    let r = run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &["validate", "-", "--no-config"], Some(&svg));
    assert_eq!(r.code, 1);
    assert_eq!(r.json()["findings"][0]["file"], "-");
}

// ------------------------------------------------------------ R4: exit codes

#[test]
fn r4_exit_codes() {
    assert_eq!(run(&["validate", CLEAN]).code, 0);
    assert_eq!(run(&["validate", BROKEN, "--no-config"]).code, 1);
    // warnings fail only under --fail-on warning
    assert_eq!(run(&["validate", BROKEN, "--no-config", "--rules", "desc-missing"]).code, 0);
    assert_eq!(run(&["validate", BROKEN, "--no-config", "--rules", "desc-missing", "--fail-on", "warning"]).code, 1);

    let usage = run(&["validate", CLEAN, "--bogus"]);
    assert_eq!(usage.code, 2);
    assert_eq!(usage.json()["error"]["code"], "unknown-flag");
    assert_eq!(run(&["frobnicate"]).code, 2);
    assert_eq!(run(&[]).code, 2);

    let missing = run(&["validate", "nope.svg"]);
    assert_eq!(missing.code, 3);
    assert_eq!(missing.json()["error"]["code"], "file-not-found");
    assert_eq!(missing.json()["ok"], false);
}

#[test]
fn malformed_xml_is_a_finding_for_validate_and_an_input_error_for_inspect() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\">\n<rect>\n</svg>").unwrap();
    let v = run_in(dir.path(), &["validate", "bad.svg"], None);
    assert_eq!(v.code, 1);
    assert_eq!(rules_of(&v.json()), vec!["xml-well-formed"]);
    let i = run_in(dir.path(), &["inspect", "bad.svg"], None);
    assert_eq!(i.code, 3);
    assert_eq!(i.json()["error"]["code"], "malformed-xml");
}

// --------------------------------------------------------- R6: self-description

#[test]
fn r6_help_is_short_and_has_an_example() {
    for args in [vec!["--help"], vec!["render", "--help"], vec!["validate", "--help"], vec!["inspect", "--help"], vec!["preview", "--help"], vec!["edit", "--help"], vec!["fix", "--help"], vec!["mcp", "--help"], vec!["schema", "--help"], vec!["rules", "--help"], vec!["examples", "--help"], vec!["describe", "--help"], vec!["docs", "--help"]] {
        let r = run(&args);
        assert_eq!(r.code, 0);
        let lines = r.stdout.lines().count();
        assert!(lines <= 40, "{args:?} help is {lines} lines");
        assert!(r.stdout.contains("Example:"), "{args:?} help has no example");
    }
    assert!(run(&["--version"]).stdout.starts_with("svgdog "));
}

// ------------------------------------------------------------ R7: strict input

#[test]
fn r7_misspellings_fail_loudly_with_a_suggestion() {
    let flag = run(&["validate", CLEAN, "--fail-no", "warning"]);
    assert_eq!(flag.code, 2);
    assert!(flag.json()["error"]["suggestion"].as_str().unwrap().contains("--fail-on"));

    let rule = run(&["validate", CLEAN, "--rules", "text-overflw"]);
    assert_eq!(rule.code, 2);
    assert_eq!(rule.json()["error"]["suggestion"], "Did you mean 'text-overflow'?");

    let value = run(&["validate", CLEAN, "--fail-on", "eror"]);
    assert_eq!(value.code, 2);

    let profile = run(&["validate", CLEAN, "--profile", "strct"]);
    assert_eq!(profile.code, 2);
    assert_eq!(profile.json()["error"]["suggestion"], "Did you mean 'strict'?");

    let cases = [
        ("versoin = 1\n", "unknown-field", "version"),
        ("[target]\nrender_widht = 700\n", "unknown-field", "render_width"),
        ("[rules]\ntext-overflw = \"off\"\n", "unknown-rule", "text-overflow"),
        ("[rules]\nfont-too-small = { min_pxx = 10 }\n", "unknown-field", "min_px"),
        ("[rules]\npalette = \"eror\"\n", "invalid-config", "error"),
        ("profile = \"dcs\"\n", "unknown-profile", "docs"),
    ];
    for (toml, code, hint) in cases {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("svgdog.toml"), toml).unwrap();
        fs::write(dir.path().join("a.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        let r = run_in(dir.path(), &["validate", "a.svg"], None);
        assert_eq!(r.code, 3, "{toml}");
        let e = &r.json()["error"];
        assert_eq!(e["code"], code, "{toml}: {e}");
        assert!(e["suggestion"].as_str().unwrap_or("").contains(hint), "{toml}: {e}");
    }
}

// ----------------------------------------------------------- R8: determinism

#[test]
fn r8_same_input_gives_identical_bytes() {
    for args in [vec!["validate", BROKEN, "--no-config"], vec!["inspect", BROKEN], vec!["rules"]] {
        assert_eq!(run(&args).stdout, run(&args).stdout);
    }
}

// --------------------------------------------------------- R10: bounded output

#[test]
fn r10_findings_are_capped_and_counts_stay_complete() {
    let mut svg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><title>t</title><desc>d</desc>\n");
    for _ in 0..500 {
        svg.push_str("<rect x=\"1\" y=\"1\" width=\"-5\" height=\"5\"/>\n");
    }
    svg.push_str("</svg>");
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("many.svg"), &svg).unwrap();
    let r = run_in(dir.path(), &["validate", "many.svg", "--rules", "invalid-value"], None);
    let v = r.json();
    assert_eq!(v["findings"].as_array().unwrap().len(), 50);
    assert_eq!(v["summary"]["truncated"], true);
    assert_eq!(v["summary"]["errors"], 500);
    assert!(!r.stdout.contains("<rect"), "SVG source must not be echoed");
    let ten = run_in(dir.path(), &["validate", "many.svg", "--rules", "invalid-value", "--max-findings", "10"], None).json();
    assert_eq!(ten["findings"].as_array().unwrap().len(), 10);
}

#[test]
fn findings_are_sorted_by_file_line_and_rule() {
    let v = run(&["validate", BROKEN, CLEAN, "--no-config", "--max-findings", "500"]).json();
    let keys: Vec<(String, u64, String)> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["file"].as_str().unwrap().to_string(), f["line"].as_u64().unwrap(), f["rule"].as_str().unwrap().to_string()))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
    assert_eq!(v["summary"]["files"], 2);
}

// ------------------------------------------------------- R13: inspect geometry

#[test]
fn r13_inspect_returns_a_bbox_for_every_visible_element() {
    let v = run(&["inspect", CLEAN]).json();
    let els = v["elements"].as_array().unwrap();
    assert!(els.len() > 10);
    for e in els {
        assert!(e["bbox"].is_array(), "no bbox: {e}");
    }
    assert_eq!(v["viewBox"], serde_json::json!([0, 0, 720, 240]));
    let api = els.iter().find(|e| e["target"] == "#node-api").unwrap();
    assert_eq!(api["bbox"], serde_json::json!([288, 88, 144, 64]));
    let label = els.iter().find(|e| e["text"] == "Order API").unwrap();
    let b: Vec<f64> = label["bbox"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
    // centred on x=360, and plausibly sized for 14 px Inter
    assert!((b[0] + b[2] / 2.0 - 360.0).abs() < 0.01 && b[2] > 50.0 && b[2] < 75.0, "{b:?}");
}

#[test]
fn inspect_select_depth_and_fields() {
    let v = run(&["inspect", CLEAN, "--select", "#node-api", "--fields", "tag,fill,refs"]).json();
    let els = v["elements"].as_array().unwrap();
    assert_eq!(els.len(), 3);
    assert_eq!(els[1]["fill"], "#2563eb");
    assert!(els[0].get("bbox").is_none());
    let top = run(&["inspect", CLEAN, "--depth", "1"]).json();
    assert!(top["elements"].as_array().unwrap().iter().all(|e| e["depth"].as_u64().unwrap() <= 1));
    let edge = run(&["inspect", CLEAN, "--select", "line", "--fields", "refs"]).json();
    assert_eq!(edge["elements"][0]["refs"], serde_json::json!(["#arrow"]));
    assert_eq!(run(&["inspect", CLEAN, "--fields", "colour"]).code, 2);
    // a finding's target can be fed straight back to --select
    let f = run(&["validate", BROKEN, "--no-config", "--rules", "stroke-too-thin"]).json();
    let target = f["findings"][0]["target"].as_str().unwrap().to_string();
    let hit = run(&["inspect", BROKEN, "--select", &target]).json();
    assert_eq!(hit["summary"]["returned"], 1);
}

// ------------------------------------------------- config, profiles, precedence

fn project(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("svgdog.toml"), toml).unwrap();
    fs::create_dir_all(dir.path().join("docs/fig")).unwrap();
    fs::create_dir_all(dir.path().join("legacy")).unwrap();
    let broken = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(BROKEN)).unwrap();
    fs::write(dir.path().join("docs/fig/b.svg"), &broken).unwrap();
    fs::write(dir.path().join("legacy/b.svg"), &broken).unwrap();
    dir
}

fn severity_of(v: &Value, rule: &str) -> Option<String> {
    v["findings"].as_array().unwrap().iter().find(|f| f["rule"] == rule).map(|f| f["severity"].as_str().unwrap().to_string())
}

#[test]
fn config_is_found_from_the_input_file_not_the_working_directory() {
    let dir = project("[rules]\ntitle-missing = \"off\"\ndesc-missing = \"error\"\n");
    let outside = tempfile::tempdir().unwrap();
    let file = dir.path().join("docs/fig/b.svg");
    let v = run_in(outside.path(), &["validate", file.to_str().unwrap(), "--max-findings", "500"], None).json();
    assert_eq!(severity_of(&v, "title-missing"), None);
    assert_eq!(severity_of(&v, "desc-missing").as_deref(), Some("error"));
    let off = run_in(outside.path(), &["validate", file.to_str().unwrap(), "--no-config", "--max-findings", "500"], None).json();
    assert_eq!(severity_of(&off, "title-missing").as_deref(), Some("error"));
}

#[test]
fn precedence_flags_then_suppressions_then_overrides_then_rules_then_profile() {
    let dir = project(
        "profile = \"docs\"\n[rules]\ndesc-missing = \"error\"\nfixed-size = \"off\"\n\n[[overrides]]\nfiles = [\"legacy/**/*.svg\"]\nrules = { desc-missing = \"info\", fixed-size = \"warning\" }\n",
    );
    let v = run_in(dir.path(), &["validate", "docs/fig/b.svg", "--max-findings", "500"], None).json();
    assert_eq!(severity_of(&v, "desc-missing").as_deref(), Some("error"));
    assert_eq!(severity_of(&v, "fixed-size"), None);
    let legacy = run_in(dir.path(), &["validate", "legacy/b.svg", "--max-findings", "500"], None).json();
    assert_eq!(severity_of(&legacy, "desc-missing").as_deref(), Some("info"));
    assert_eq!(severity_of(&legacy, "fixed-size").as_deref(), Some("warning"));
    // flags beat the config
    let skipped = run_in(dir.path(), &["validate", "legacy/b.svg", "--skip", "desc-missing", "--max-findings", "500"], None).json();
    assert_eq!(severity_of(&skipped, "desc-missing"), None);
    let forced = run_in(dir.path(), &["validate", "docs/fig/b.svg", "--rules", "fixed-size"], None).json();
    assert_eq!(rules_of(&forced), vec!["fixed-size"]);
}

#[test]
fn profiles_change_the_active_rules_and_severities() {
    let minimal = run(&["validate", BROKEN, "--no-config", "--profile", "minimal", "--max-findings", "500"]).json();
    let got = rules_of(&minimal);
    assert!(got.contains(&"duplicate-id".to_string()) && got.contains(&"script-content".to_string()));
    assert!(!got.contains(&"title-missing".to_string()) && !got.contains(&"text-overflow".to_string()));
    let strict = run(&["validate", BROKEN, "--no-config", "--profile", "strict", "--max-findings", "500"]).json();
    assert_eq!(severity_of(&strict, "desc-missing").as_deref(), Some("error"));
    assert_eq!(severity_of(&strict, "unused-def").as_deref(), Some("warning"));
}

#[test]
fn thresholds_are_configurable() {
    let dir = project("[target]\nrender_width = 400\n[rules]\nstroke-too-thin = { min_px = 0.1 }\nfont-too-small = { severity = \"warning\", min_px = 3 }\n");
    let v = run_in(dir.path(), &["validate", "docs/fig/b.svg", "--max-findings", "500"], None).json();
    assert_eq!(severity_of(&v, "stroke-too-thin"), None);
    assert_eq!(severity_of(&v, "font-too-small"), None);
}

#[test]
fn suppression_needs_a_reason_and_silences_one_element() {
    let dir = tempfile::tempdir().unwrap();
    let svg = |comment: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 100\">\n<title>t</title>\n<rect x=\"20\" y=\"20\" width=\"90\" height=\"40\" fill=\"#dbeafe\"/>\n{comment}\n<rect x=\"90\" y=\"40\" width=\"90\" height=\"40\" fill=\"#dbeafe\"/>\n</svg>"
        )
    };
    fs::write(dir.path().join("plain.svg"), svg("")).unwrap();
    fs::write(dir.path().join("ok.svg"), svg("<!-- svgdog-disable shape-overlap: badge sits on the corner by design -->")).unwrap();
    fs::write(dir.path().join("bare.svg"), svg("<!-- svgdog-disable shape-overlap -->")).unwrap();
    let args = |f: &'static str| vec!["validate", f, "--rules", "shape-overlap"];
    assert_eq!(rules_of(&run_in(dir.path(), &args("plain.svg"), None).json()), vec!["shape-overlap"]);
    assert!(rules_of(&run_in(dir.path(), &args("ok.svg"), None).json()).is_empty());
    let bare = run_in(dir.path(), &args("bare.svg"), None);
    assert_eq!(bare.code, 1);
    let got = rules_of(&bare.json());
    assert!(got.contains(&"invalid-suppression".to_string()) && got.contains(&"shape-overlap".to_string()), "{got:?}");
}

#[test]
fn globs_expand_and_report_each_file() {
    let dir = project("");
    let v = run_in(dir.path(), &["validate", "**/*.svg", "--rules", "title-missing"], None).json();
    assert_eq!(v["summary"]["files"], 2);
    assert_eq!(v["summary"]["errors"], 2);
    assert_eq!(run_in(dir.path(), &["validate", "none/*.svg"], None).code, 3);
}

// ------------------------------------------------------ R14, R15: side effects

#[test]
fn r14_validate_and_inspect_write_nothing() {
    let dir = project("");
    let before: Vec<_> = walk(dir.path());
    run_in(dir.path(), &["validate", "docs/fig/b.svg"], None);
    run_in(dir.path(), &["inspect", "docs/fig/b.svg"], None);
    assert_eq!(before, walk(dir.path()));
}

fn walk(dir: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        if e.path().is_dir() {
            out.extend(walk(&e.path()));
        } else {
            out.push((e.path().display().to_string(), e.metadata().unwrap().len()));
        }
    }
    out.sort();
    out
}

#[test]
fn r15_a_200_kb_file_validates_quickly() {
    let mut svg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 4000 4000\"><title>t</title><desc>d</desc>\n<defs><marker id=\"m\" markerWidth=\"6\" markerHeight=\"6\" refX=\"6\" refY=\"3\"><path d=\"M0 0L6 3L0 6z\"/></marker></defs>\n");
    let mut i = 0;
    while svg.len() < 200_000 {
        let (x, y) = ((i % 20) * 200, (i / 20) * 100);
        svg.push_str(&format!(
            "<g id=\"n{i}\"><rect x=\"{x}\" y=\"{y}\" width=\"150\" height=\"60\" fill=\"#dbeafe\" stroke=\"#1f2937\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-family=\"Inter, sans-serif\" font-size=\"40\">Node {i}</text></g>\n<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#1f2937\" marker-end=\"url(#m)\"/>\n",
            x + 75, y + 45, x + 150, y + 30, x + 200, y + 30
        ));
        i += 1;
    }
    svg.push_str("</svg>");
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("big.svg"), &svg).unwrap();
    // Best of three: the first launch of a freshly built binary pays a one-off cost
    // to the operating system that says nothing about the tool.
    let mut took = std::time::Duration::MAX;
    for _ in 0..3 {
        let start = std::time::Instant::now();
        let r = run_in(dir.path(), &["validate", "big.svg"], None);
        took = took.min(start.elapsed());
        assert!(r.code <= 1, "{}", r.stderr);
        r.json();
    }
    // The 1 s budget is for a release build; debug builds get slack.
    let budget = if cfg!(debug_assertions) { 20.0 } else { 1.0 };
    assert!(took.as_secs_f64() < budget, "took {took:?}");
}

// ------------------------------------------------- phase 2: edit, fix, fix objects

fn copy_fixture(name: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap();
    fs::write(dir.path().join("d.svg"), &text).unwrap();
    (dir, text)
}

/// Rules whose catalogue entry lists an autofix that this build generates.
const FIXABLE: &[(&str, bool)] = &[
    ("svg-root", true), ("viewbox-missing", true), ("unused-def", true), ("duplicate-id", false), ("text-overflow", true),
    ("content-clipped", true), ("excess-margin", true), ("edge-detached", true), ("edge-through-node", false), ("label-on-line", false),
    ("font-too-small", false), ("stroke-too-thin", true), ("off-grid", true), ("script-content", true), ("animation", true),
    ("font-not-portable", true), ("fixed-size", true), ("editor-metadata", true), ("low-contrast", false),
    ("background-missing", true), ("palette", false), ("stroke-width-style", true),
];

/// R5: for each fixable rule, applying the finding's ops clears that finding.
#[test]
fn r5_applying_a_findings_ops_clears_it() {
    let ops_schema = schema("ops");
    for (id, safe) in FIXABLE {
        let rule = run(&["rules", "--explain", id]).json()["rule"].clone();
        assert_eq!(rule["autofix"], if *safe { "safe" } else { "unsafe" }, "{id}");
        let dir = tempfile::tempdir().unwrap();
        let cfg = rule["example_config"].as_str().unwrap();
        if !cfg.is_empty() {
            fs::write(dir.path().join("svgdog.toml"), cfg).unwrap();
        }
        fs::write(dir.path().join("fail.svg"), rule["fail"].as_str().unwrap()).unwrap();
        let before = run_in(dir.path(), &["validate", "fail.svg", "--rules", id, "--quiet"], None).json();
        let findings = before["findings"].as_array().unwrap();
        assert!(!findings.is_empty(), "{id}");
        assert_eq!(before["summary"]["fixable"].as_u64().unwrap() as usize, findings.len(), "{id}: every finding should carry a fix\n{before:#}");

        // Feed each finding's fix object straight back to edit --ops, one at a time.
        for round in 0..findings.len() + 1 {
            let now = run_in(dir.path(), &["validate", "fail.svg", "--rules", id, "--quiet"], None).json();
            let Some(f) = now["findings"].as_array().unwrap().first().cloned() else { break };
            assert!(round < findings.len(), "{id}: findings do not go away\n{now:#}");
            assert_eq!(f["fix"]["safe"], *safe, "{id}");
            assert_valid(&ops_schema, &f["fix"]);
            let fix = serde_json::to_string(&f["fix"]).unwrap();
            let r = run_in(dir.path(), &["edit", "fail.svg", "--ops", "-", "--in-place", "--quiet"], Some(&fix));
            assert!(r.code <= 1, "{id}: edit failed\n{}{}", r.stdout, r.stderr);
            assert_eq!(r.json()["changed"], true, "{id}: ops changed nothing\n{fix}");
        }
        let after = run_in(dir.path(), &["validate", "fail.svg", "--rules", id, "--quiet"], None).json();
        assert!(rules_of(&after).is_empty(), "{id}: still failing after its fix\n{after:#}");
    }
}

/// A box widened to fit its label takes the connectors on its edge with it; otherwise the
/// line would start inside the box.
#[test]
fn fix_keeps_connectors_on_the_edge_of_a_widened_box() {
    let (dir, _) = copy_fixture(BROKEN);
    run_in(dir.path(), &["fix", "d.svg", "--no-config", "--in-place"], None);
    let text = fs::read_to_string(dir.path().join("d.svg")).unwrap();
    // The box now spans x = -21..151 and the connector starts on its right edge, not at 110.
    assert!(text.contains(r##"<rect width="172" height="50" fill="#ffffff" stroke="#333" x="-41"/>"##), "{text}");
    assert!(text.contains(r#"d="M151,85 L250,85""#), "{text}");
}

/// A rerouted connector ends on its end point going forwards, so the arrowhead on the last
/// segment points at the target and not back out of it.
#[test]
fn fix_reroute_does_not_overshoot_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 460 200"><title>t</title><desc>d</desc>
  <defs><marker id="arrow" markerWidth="6" markerHeight="6" refX="6" refY="3" orient="auto"><path d="M0 0L6 3L0 6z"/></marker></defs>
  <rect x="20" y="60" width="90" height="50" fill="#ffffff" stroke="#333"/>
  <rect x="250" y="60" width="190" height="50" fill="#ffffff" stroke="#333"/>
  <rect x="150" y="70" width="40" height="30" fill="#eeeeee" stroke="#333"/>
  <path d="M110 85 L250 85" fill="none" stroke="#333" marker-end="url(#arrow)"/>
</svg>"##;
    fs::write(dir.path().join("d.svg"), svg).unwrap();
    run_in(dir.path(), &["fix", "d.svg", "--no-config", "--unsafe", "--in-place", "--rules", "edge-through-node"], None);
    let text = fs::read_to_string(dir.path().join("d.svg")).unwrap();
    let d = text.split("<path d=\"").nth(2).and_then(|t| t.split('"').next()).unwrap();
    let xs: Vec<f64> = d.split(['M', 'L']).filter(|t| !t.trim().is_empty()).map(|t| t.split_whitespace().next().unwrap().parse().unwrap()).collect();
    assert!(d != "M110 85 L250 85", "the connector was not rerouted: {d}");
    assert_eq!(xs.last(), Some(&250.0), "{d}");
    assert!(xs.iter().all(|x| *x <= 250.0), "the route runs past its end and turns back: {d}");
}

#[test]
fn fix_applies_safe_fixes_and_unsafe_only_on_request() {
    let (dir, _) = copy_fixture(BROKEN);
    let safe = run_in(dir.path(), &["fix", "d.svg", "--no-config", "-o", "safe.svg"], None).json();
    let fixed: Vec<String> = safe["fixed"].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect();
    for want in ["unused-def", "fixed-size", "editor-metadata", "script-content", "text-overflow", "edge-detached", "stroke-too-thin", "content-clipped"] {
        assert!(fixed.contains(&want.to_string()), "{want} not fixed: {fixed:?}");
    }
    for unsafe_rule in ["low-contrast", "duplicate-id", "font-too-small"] {
        assert!(!fixed.contains(&unsafe_rule.to_string()));
        assert!(rules_of(&safe).contains(&unsafe_rule.to_string()));
    }
    assert_eq!(safe["outputs"][0]["path"], "safe.svg");
    assert_eq!(safe["outputs"][0]["bytes"].as_u64().unwrap(), fs::metadata(dir.path().join("safe.svg")).unwrap().len());

    let all = run_in(dir.path(), &["fix", "d.svg", "--no-config", "--unsafe", "-o", "all.svg"], None).json();
    let left = rules_of(&all);
    for gone in ["low-contrast", "duplicate-id", "font-too-small", "label-on-line"] {
        assert!(!left.contains(&gone.to_string()), "{gone} left after --unsafe: {left:?}");
    }
    // what is left has no fix to offer
    assert_eq!(all["summary"]["fixable"], 0, "{all:#}");

    let only = run_in(dir.path(), &["fix", "d.svg", "--no-config", "--rules", "unused-def", "--dry-run"], None).json();
    assert_eq!(only["fixed"], serde_json::json!([{ "rule": "unused-def", "count": 1 }]));
}

/// R9: repeating a command changes nothing further.
#[test]
fn r9_fix_on_fixed_output_is_an_empty_diff() {
    for flags in [vec![], vec!["--unsafe"]] {
        let (dir, _) = copy_fixture(BROKEN);
        let mut first = vec!["fix", "d.svg", "--no-config", "--in-place"];
        first.extend(&flags);
        assert_eq!(run_in(dir.path(), &first, None).json()["changed"], true);
        let once = fs::read(dir.path().join("d.svg")).unwrap();
        let again = run_in(dir.path(), &first, None).json();
        assert_eq!(again["changed"], false);
        assert_eq!(again["diff"], "");
        assert_eq!(again["outputs"], serde_json::json!([]));
        assert_eq!(once, fs::read(dir.path().join("d.svg")).unwrap());
    }
    let (dir, text) = copy_fixture(CLEAN);
    let r = run_in(dir.path(), &["fix", "d.svg", "--in-place", "--rules", "text-overflow"], None);
    assert_eq!((r.code, r.json()["changed"].as_bool()), (0, Some(false)));
    assert_eq!(text, fs::read_to_string(dir.path().join("d.svg")).unwrap());
}

/// R11: edit changes only the addressed elements.
#[test]
fn r11_bytes_outside_the_edited_element_are_unchanged() {
    let (dir, before) = copy_fixture(CLEAN);
    let r = run_in(dir.path(), &["edit", "d.svg", "--set", "#node-api > rect.fill=#1d4ed8", "--set", "#node-api > rect.rx=8", "--in-place"], None);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let after = fs::read_to_string(dir.path().join("d.svg")).unwrap();
    let old = "<rect x=\"288\" y=\"88\" width=\"144\" height=\"64\" rx=\"6\" fill=\"#2563eb\" stroke=\"#1f2937\"/>";
    let new = "<rect x=\"288\" y=\"88\" width=\"144\" height=\"64\" rx=\"8\" fill=\"#1d4ed8\" stroke=\"#1f2937\"/>";
    assert_eq!(after, before.replace(old, new));
    assert_ne!(after, before);
    let v = r.json();
    assert_eq!(v["operations"], serde_json::json!({ "given": 2, "applied": 2 }));
    assert!(v["diff"].as_str().unwrap().contains("+    <rect x=\"288\""));
}

/// R12: edit and fix can report their effect without writing.
#[test]
fn r12_dry_run_reports_a_diff_and_findings_and_writes_nothing() {
    let (dir, before) = copy_fixture(CLEAN);
    let listing = walk(dir.path());
    // shrinking the box makes its label overflow: the dry run says so before anything is written
    let e = run_in(dir.path(), &["edit", "d.svg", "--set", "#node-api > rect.width=80", "--dry-run"], None);
    let v = e.json();
    assert_eq!((v["changed"].as_bool(), v["dry_run"].as_bool()), (Some(true), Some(true)));
    assert!(v["diff"].as_str().unwrap().starts_with("--- a/d.svg\n+++ b/d.svg\n@@"));
    assert!(rules_of(&v).contains(&"text-overflow".to_string()), "{v:#}");
    assert_eq!(v["outputs"], serde_json::json!([]));
    let (bdir, _) = copy_fixture(BROKEN);
    let f = run_in(bdir.path(), &["fix", "d.svg", "--no-config", "--dry-run"], None).json();
    assert!(f["changed"] == true && !f["diff"].as_str().unwrap().is_empty());
    assert_eq!(before, fs::read_to_string(dir.path().join("d.svg")).unwrap());
    assert_eq!(listing, walk(dir.path()));
}

#[test]
fn edit_ops_file_covers_every_operation() {
    let (dir, _) = copy_fixture(CLEAN);
    let ops = serde_json::json!([
        { "op": "set", "target": "#node-web > rect", "style": "stroke-dasharray", "value": "4 2" },
        { "op": "remove", "target": "#edge-api--db > text" },
        { "op": "remove", "target": "#node-db > rect", "attr": "rx" },
        { "op": "move", "target": "#node-db", "dy": 8 },
        { "op": "rename-id", "target": "#arrow", "to": "arrowhead" },
        { "op": "add-node", "target": "#node-web", "tag": "title", "text": "Storefront & shop" }
    ]);
    fs::write(dir.path().join("ops.json"), ops.to_string()).unwrap();
    let r = run_in(dir.path(), &["edit", "d.svg", "--ops", "ops.json", "-o", "out.svg"], None);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert_eq!(r.json()["operations"]["applied"], 6);
    let out = fs::read_to_string(dir.path().join("out.svg")).unwrap();
    assert!(out.contains("style=\"stroke-dasharray: 4 2\""));
    assert!(!out.contains(">insert<"));
    assert!(out.contains("<g id=\"node-db\" transform=\"translate(0 8)\">"));
    assert!(out.contains("<marker id=\"arrowhead\"") && out.matches("url(#arrowhead)").count() == 2 && !out.contains("url(#arrow)"));
    assert!(out.contains("<title>Storefront &amp; shop</title>"));
    assert_valid(&schema("ops"), &ops);
    // the input is untouched
    assert!(fs::read_to_string(dir.path().join("d.svg")).unwrap().contains(">insert<"));
}

#[test]
fn edit_and_fix_usage_input_and_output_errors() {
    let (dir, _) = copy_fixture(CLEAN);
    let code = |args: &[&str], stdin: Option<&str>| {
        let r = run_in(dir.path(), args, stdin);
        (r.code, r.json()["error"]["code"].as_str().unwrap_or("").to_string())
    };
    assert_eq!(code(&["edit", "d.svg", "--set", "#node-api.fill=red"], None), (2, "missing-argument".into()));
    assert_eq!(code(&["edit", "d.svg", "--dry-run"], None), (2, "missing-argument".into()));
    assert_eq!(code(&["edit", "d.svg", "--set", "nonsense", "--dry-run"], None), (2, "invalid-argument".into()));
    assert_eq!(code(&["edit", "d.svg", "--set", "#a.x=1", "--in-place", "-o", "x.svg"], None), (2, "argument-conflict".into()));
    assert_eq!(code(&["fix", "d.svg"], None), (2, "missing-argument".into()));
    assert_eq!(code(&["fix", "d.svg", "--rules", "nope", "--dry-run"], None), (2, "unknown-rule".into()));
    // input errors
    assert_eq!(code(&["edit", "d.svg", "--set", "#missing.fill=red", "--dry-run"], None), (3, "no-match".into()));
    assert_eq!(code(&["edit", "d.svg", "--ops", "-", "--dry-run"], Some("[{\"op\":\"sett\",\"target\":\"svg\"}]")), (3, "unknown-op".into()));
    assert_eq!(code(&["edit", "d.svg", "--ops", "-", "--dry-run"], Some("[{\"op\":\"remove\",\"target\":\"svg\",\"atr\":\"x\"}]")), (3, "unknown-field".into()));
    assert_eq!(code(&["edit", "d.svg", "--ops", "-", "--dry-run"], Some("not json")), (3, "invalid-op".into()));
    assert_eq!(code(&["edit", "d.svg", "--set", "#node-api.xlink:href=#x", "--dry-run"], None), (3, "edit-breaks-xml".into()));
    assert_eq!(code(&["fix", "nope.svg", "--dry-run"], None), (3, "file-not-found".into()));
    // output errors: exit code 4, and no silent overwrite
    fs::write(dir.path().join("taken.svg"), "keep").unwrap();
    assert_eq!(code(&["fix", "d.svg", "-o", "taken.svg"], None), (4, "output-exists".into()));
    assert_eq!(code(&["edit", "d.svg", "--set", "#node-api > rect.rx=1", "-o", "d.svg"], None), (4, "output-exists".into()));
    assert_eq!(fs::read_to_string(dir.path().join("taken.svg")).unwrap(), "keep");
    assert_eq!(code(&["edit", "d.svg", "--set", "#node-api > rect.rx=1", "-o", "no/such/dir/out.svg"], None), (4, "write-failed".into()));
}

#[test]
fn output_dash_writes_the_svg_to_stdout_without_an_envelope() {
    let (dir, before) = copy_fixture(CLEAN);
    let r = run_in(dir.path(), &["edit", "d.svg", "--set", "#node-api > rect.rx=8", "-o", "-"], None);
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with("<svg") && r.stdout.contains("rx=\"8\""));
    assert_eq!(r.stdout.len(), before.len());
    // stdin to stdout works as a filter
    let piped = run_in(dir.path(), &["fix", "-", "--no-config", "-o", "-"], Some(&fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(BROKEN)).unwrap()));
    assert_eq!(piped.code, 1); // errors remain, and the exit code still says so
    assert!(piped.stdout.contains("<svg") && !piped.stdout.contains("onclick"));
}

#[test]
fn duplicate_ids_get_distinct_targets() {
    let v = run(&["validate", BROKEN, "--no-config", "--rules", "duplicate-id"]).json();
    let f = &v["findings"][0];
    assert_ne!(f["target"], "#a");
    assert_eq!(f["fix"]["ops"][0]["op"], "rename-id");
    assert_eq!(f["fix"]["ops"][0]["to"], "a-2");
    let hit = run(&["inspect", BROKEN, "--select", f["target"].as_str().unwrap()]).json();
    assert_eq!(hit["summary"]["returned"], 1);
}

// ------------------------------------------------------------- phase 3: render

const ORDERS: &str = "tests/fixtures/orders.yaml";

fn render_fixture(name: &str, extra: &[&str]) -> (Value, String) {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    let mut args = vec!["render", spec.to_str().unwrap(), "-o", "out.svg", "--no-config"];
    args.extend(extra);
    let r = run_in(dir.path(), &args, None);
    assert!(r.code <= 1, "{name}: {}{}", r.stdout, r.stderr);
    (r.json(), fs::read_to_string(dir.path().join("out.svg")).unwrap())
}

fn ids_in(svg: &str) -> Vec<String> {
    svg.split(" id=\"").skip(1).map(|s| s.split('"').next().unwrap().to_string()).collect()
}

#[test]
fn the_spec_example_renders_clean() {
    let (v, svg) = render_fixture(ORDERS, &[]);
    assert_eq!(v["ok"], true);
    assert_eq!(v["findings"], serde_json::json!([]), "{v:#}");
    assert_eq!(v["outputs"][0]["bytes"].as_u64().unwrap() as usize, svg.len());
    assert!(svg.contains("<title>Order placement</title>") && svg.contains("<desc>How an order moves from the storefront to fulfilment.</desc>"));
    for id in ["node-web", "node-api", "node-db", "node-queue", "group-backend", "edge-web--api", "edge-api--db", "edge-api--queue", "note-1"] {
        assert!(ids_in(&svg).contains(&id.to_string()), "missing {id}");
    }
    for word in ["Storefront", "Order API", "Orders DB", "Fulfilment queue", "Backend", "POST /orders", "insert", "publish", "At-least-once delivery"] {
        assert!(svg.contains(&format!(">{word}<")), "missing text {word}");
    }
    assert!(svg.contains("stroke-dasharray=\"6 4\""), "the async edge is dashed");
    assert!(svg.contains("fill=\"#2563eb\""), "the emphasised node uses the accent");
}

#[test]
fn every_fixture_and_theme_renders_without_errors_or_warnings() {
    for name in [ORDERS, "tests/fixtures/checkout.yaml", "tests/fixtures/tiers.yaml", "tests/fixtures/rack.yaml", "tests/fixtures/login.yaml"] {
        for theme in ["docs-light", "docs-dark"] {
            let (v, _) = render_fixture(name, &["--theme", theme]);
            let bad: Vec<&Value> = v["findings"].as_array().unwrap().iter().filter(|f| f["rule"] != "desc-missing" && f["rule"] != "label-on-line").collect();
            assert!(bad.is_empty(), "{name} {theme}: {bad:#?}");
            assert_eq!(v["summary"]["errors"], 0, "{name} {theme}");
        }
    }
}

#[test]
fn every_node_kind_renders_clean_in_both_directions() {
    let kinds = ["box", "service", "client", "datastore", "queue", "actor", "external", "decision", "document"];
    for direction in ["right", "down"] {
        let mut spec = format!("version: 1\ndiagram: flow\ntitle: Kinds\ndescription: Every node kind.\ndirection: {direction}\nmax_width: 2000\nnodes:\n");
        for k in kinds {
            spec.push_str(&format!("  - {{id: n-{k}, label: \"A {k} node\", kind: {k}}}\n"));
        }
        spec.push_str("edges:\n");
        for w in kinds.windows(2) {
            spec.push_str(&format!("  - {{from: n-{}, to: n-{}}}\n", w[0], w[1]));
        }
        spec.push_str("  - {from: n-box, to: n-queue, label: skip}\n  - {from: n-decision, to: n-service, label: back}\n");
        let dir = tempfile::tempdir().unwrap();
        // A wide drawing shrinks on the page; this test is about geometry, not rendered size.
        fs::write(dir.path().join("svgdog.toml"), "[target]\nrender_width = 2000\n").unwrap();
        fs::write(dir.path().join("k.yaml"), &spec).unwrap();
        let r = run_in(dir.path(), &["render", "k.yaml", "-o", "k.svg"], None);
        let v = r.json();
        assert_eq!(v["findings"], serde_json::json!([]), "{direction}: {v:#}");
    }
}

#[test]
fn rendered_svg_uses_only_the_portable_subset() {
    let allowed = ["svg", "g", "rect", "circle", "ellipse", "line", "polyline", "polygon", "path", "text", "tspan", "defs", "marker", "title", "desc"];
    for name in [ORDERS, "tests/fixtures/checkout.yaml", "tests/fixtures/tiers.yaml", "tests/fixtures/rack.yaml", "tests/fixtures/login.yaml"] {
        let (_, svg) = render_fixture(name, &[]);
        for tag in svg.split('<').skip(1).filter(|t| !t.starts_with('/')) {
            let tag = tag.split(|c: char| c.is_whitespace() || c == '>' || c == '/').next().unwrap();
            assert!(allowed.contains(&tag), "{name}: element <{tag}>");
        }
        assert!(!svg.contains("style=") && !svg.contains("class=") && !svg.contains("<!--"), "{name}");
        let root = svg.lines().next().unwrap();
        assert!(root.contains("viewBox=\"0 0 ") && !root.contains(" width=") && !root.contains(" height="), "{root}");
        assert!(!svg.contains('\r') && !svg.starts_with('\u{feff}') && svg.ends_with("</svg>\n"));
        // numbers carry at most two decimals
        for num in svg.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter(|s| s.contains('.')) {
            assert!(num.split('.').nth(1).unwrap().len() <= 2, "{name}: {num}");
        }
    }
}

/// R8 in part: the same spec gives the same bytes. (The cross-platform half needs CI.)
#[test]
fn r8_render_is_deterministic() {
    for name in [ORDERS, "tests/fixtures/checkout.yaml"] {
        assert_eq!(render_fixture(name, &[]).1, render_fixture(name, &[]).1);
    }
    // YAML and JSON forms of one spec give the same drawing.
    let yaml = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS)).unwrap();
    let doc: Value = serde_yaml_ng::from_str(&yaml).unwrap();
    let from_json = run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &["render", "-", "-o", "-", "--no-config"], Some(&doc.to_string()));
    assert_eq!(from_json.code, 0);
    assert_eq!(from_json.stdout, render_fixture(ORDERS, &[]).1);
}

/// R16: adding a node to a spec changes no existing element id.
#[test]
fn r16_ids_survive_adding_a_node() {
    let (_, before) = render_fixture(ORDERS, &[]);
    let dir = tempfile::tempdir().unwrap();
    let yaml = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS)).unwrap();
    let grown = yaml.replace("direction: right", "direction: right\nmax_width: 1200").replace("\ngroups:", "\n  - id: cache\n    label: Cache\n    kind: datastore\n\ngroups:").replace("    style: async\n", "    style: async\n  - from: api\n    to: cache\n");
    assert_ne!(grown, yaml);
    fs::write(dir.path().join("s.yaml"), grown).unwrap();
    let r = run_in(dir.path(), &["render", "s.yaml", "-o", "s.svg"], None);
    assert!(r.code <= 1, "{}", r.stdout);
    let after = ids_in(&fs::read_to_string(dir.path().join("s.svg")).unwrap());
    for id in ids_in(&before) {
        assert!(after.contains(&id), "{id} vanished");
    }
    assert!(after.contains(&"node-cache".to_string()) && after.contains(&"edge-api--cache".to_string()));
}

#[test]
fn spec_fixtures_satisfy_the_published_schema() {
    let v = schema("spec");
    for name in [ORDERS, "tests/fixtures/checkout.yaml", "tests/fixtures/tiers.yaml", "tests/fixtures/rack.yaml", "tests/fixtures/login.yaml", "tests/fixtures/walkthrough.yaml", "tests/fixtures/states.yaml", "tests/fixtures/migration.yaml", "tests/fixtures/split.yaml"] {
        let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap()).unwrap();
        assert_valid(&v, &doc);
    }
}

fn render_err(spec: &str) -> Value {
    let r = run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &["render", "-", "-o", "-", "--no-config"], Some(spec));
    assert_eq!(r.code, 3, "{spec}\n{}", r.stdout);
    r.json()["error"].clone()
}

#[test]
fn spec_errors_name_the_path_and_suggest_a_fix() {
    let base = "version: 1\ndiagram: flow\ntitle: T\nnodes:\n  - {id: a, label: A}\n  - {id: b, label: B}\n";
    // the example from the specification
    let e = render_err("version: 1\ndiagram: flow\ntitle: T\nnodes:\n  - {id: a, label: A}\n  - {id: b, label: B}\n  - {id: c, lable: C}\n");
    assert_eq!(e, serde_json::json!({ "code": "unknown-field", "message": "Unknown field 'lable' in nodes[2].", "path": "nodes[2].lable", "suggestion": "Did you mean 'label'?" }));

    let cases: Vec<(String, &str, &str)> = vec![
        (base.replace("diagram: flow", "diagram: flwo"), "invalid-value", "diagram"),
        (base.replace("title: T\n", ""), "missing-field", "title"),
        (base.replace("version: 1", "version: 2"), "invalid-value", "version"),
        (base.replace("{id: b, label: B}", "{id: a, label: B}"), "duplicate-id", "nodes[1].id"),
        (base.replace("{id: b, label: B}", "{id: B_2, label: B}"), "invalid-value", "nodes[1].id"),
        (base.replace("{id: b, label: B}", "{id: b, label: B, kind: databse}"), "invalid-value", "nodes[1].kind"),
        (format!("{base}notes:\n  - {{attach: c, text: x}}\n"), "unknown-reference", "notes[0].attach"),
        (format!("{base}edges:\n  - {{from: a, to: b}}\n  - {{from: a, to: b}}\n"), "duplicate-id", "edges[1].id"),
        (format!("{base}groups:\n  - {{id: g, members: [a, Zz]}}\n"), "unknown-reference", "groups[0].members[1]"),
        (format!("{base}groups:\n  - {{id: g, members: [a]}}\n  - {{id: h, members: [a]}}\n"), "group-overlap", "groups[1].members"),
        (format!("{base}groups:\n  - {{id: g, members: [a, h]}}\n  - {{id: h, members: [g]}}\n"), "group-cycle", "groups[0].members"),
        (base.replace("{id: b, label: B}", "{id: b, label: B, at: [1, 2]}"), "at-outside-free", "nodes[1].at"),
        (base.replace("diagram: flow", "diagram: free"), "missing-field", "nodes[0].at"),
        (format!("{}edges:\n  - {{from: a, to: g}}\ngroups:\n  - {{id: g, members: [b]}}\n", base.replace("diagram: flow", "diagram: sequence")), "invalid-value", "edges[0].to"),
        (format!("{base}notes:\n  - {{attach: nope, text: hi}}\n"), "unknown-reference", "notes[0].attach"),
        (format!("{base}theme: neon\n"), "unknown-theme", "theme"),
        ("nodes: [".to_string(), "malformed-spec", "-"),
    ];
    for (spec, code, path) in cases {
        let e = render_err(&spec);
        assert_eq!((e["code"].as_str().unwrap(), e["path"].as_str().unwrap()), (code, path), "{spec}\n{e:#}");
    }
    // nested groups are fine: a node may sit in a group and in that group's parent
    let nested = format!("{base}groups:\n  - {{id: inner, members: [a]}}\n  - {{id: outer, members: [inner, a, b]}}\n");
    assert!(run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &["render", "-", "-o", "-", "--no-config"], Some(&nested)).code <= 1);
}

#[test]
fn render_output_rules() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS), dir.path().join("o.yaml")).unwrap();
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None).code, 0);
    // an existing file is replaced only with --force
    let again = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None);
    assert_eq!((again.code, again.json()["error"]["code"].as_str()), (4, Some("output-exists")));
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--force"], None).code, 0);
    // -o - writes the SVG itself
    let raw = run_in(dir.path(), &["render", "o.yaml", "-o", "-"], None);
    assert_eq!(raw.stdout, fs::read_to_string(dir.path().join("o.svg")).unwrap());
    let skipped = run_in(dir.path(), &["render", "o.yaml", "-o", "n.svg", "--no-validate"], None).json();
    assert_eq!((skipped["validated"].as_bool(), skipped["findings"].as_array().unwrap().len()), (Some(false), 0));
    assert_eq!(run_in(dir.path(), &["render", "o.yaml"], None).code, 2);
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "x.svg", "--theme", "neon"], None).code, 2);
    assert_eq!(run_in(dir.path(), &["render", "missing.yaml", "-o", "x.svg"], None).code, 3);
    // outlines in place of text
    let paths = run_in(dir.path(), &["render", "o.yaml", "-o", "-", "--text-as-paths"], None).stdout;
    assert!(!paths.contains("<text") && paths.matches("<path").count() > 10 && paths.contains("<title>Order placement</title>"));
    // too wide for max_width is an input error with advice
    let wide = fs::read_to_string(dir.path().join("o.yaml")).unwrap().replace("direction: right", "direction: right\nmax_width: 400");
    fs::write(dir.path().join("w.yaml"), wide).unwrap();
    let e = run_in(dir.path(), &["render", "w.yaml", "-o", "w.svg"], None);
    assert_eq!((e.code, e.json()["error"]["code"].as_str()), (3, Some("too-wide")));
    assert!(!dir.path().join("w.svg").exists());
    // A limit that the roomy layout misses but a tighter one meets is met without being asked:
    // the example is 872 px wide by default and is set more tightly to fit 760.
    let width = |svg: &str| svg.split("viewBox=\"0 0 ").nth(1).unwrap().split(' ').next().unwrap().parse::<f64>().unwrap();
    assert!(width(&fs::read_to_string(dir.path().join("o.svg")).unwrap()) > 760.0);
    let snug = fs::read_to_string(dir.path().join("o.yaml")).unwrap().replace("direction: right", "direction: right\nmax_width: 760");
    fs::write(dir.path().join("s.yaml"), snug).unwrap();
    let r = run_in(dir.path(), &["render", "s.yaml", "-o", "s.svg"], None);
    assert_eq!(r.json()["findings"], serde_json::json!([]), "{}", r.stdout);
    assert!(width(&fs::read_to_string(dir.path().join("s.svg")).unwrap()) <= 760.0);
}

#[test]
fn config_fonts_and_long_labels() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("svgdog.toml"), "[style]\nfonts = [\"Inter\", \"Helvetica\", \"sans-serif\"]\n").unwrap();
    fs::write(dir.path().join("s.yaml"), "version: 1\ndiagram: flow\ntitle: Wrap\ndescription: d\nnodes:\n  - {id: a, label: \"A service with quite a long descriptive name\"}\n  - {id: b, label: \"Two\\nlines & <more>\"}\nedges:\n  - {from: a, to: b}\n").unwrap();
    let r = run_in(dir.path(), &["render", "s.yaml", "-o", "s.svg"], None);
    assert_eq!(r.json()["findings"], serde_json::json!([]), "{}", r.stdout);
    let svg = fs::read_to_string(dir.path().join("s.svg")).unwrap();
    assert!(svg.contains("font-family=\"Inter, Helvetica, sans-serif\""));
    assert!(svg.matches("<tspan").count() >= 4, "long labels wrap onto lines");
    assert!(svg.contains("lines &amp; &lt;more&gt;"));
}

#[test]
fn edit_changes_a_spec_and_reports_on_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS), dir.path().join("o.yaml")).unwrap();
    let ops = serde_json::json!([
        { "op": "set", "target": "#api", "attr": "label", "value": "Orders API" },
        { "op": "set", "target": ":root", "attr": "direction", "value": "down" },
        { "op": "add-node", "target": "#backend", "attrs": { "id": "cache", "label": "Cache", "kind": "datastore" } },
        { "op": "add-edge", "from": "api", "to": "cache", "attrs": { "label": "read" } },
        { "op": "rename-id", "target": "#queue", "to": "events" },
        { "op": "remove", "target": "#web" }
    ]);
    assert_valid(&schema("ops"), &ops);
    fs::write(dir.path().join("ops.json"), ops.to_string()).unwrap();
    let r = run_in(dir.path(), &["edit", "o.yaml", "--ops", "ops.json", "--in-place"], None);
    assert!(r.code <= 1, "{}{}", r.stdout, r.stderr);
    let v = r.json();
    assert_eq!(v["operations"], serde_json::json!({ "given": 6, "applied": 6 }));
    assert_eq!(v["summary"]["errors"], 0, "{v:#}");
    let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(dir.path().join("o.yaml")).unwrap()).unwrap();
    let ids: Vec<&str> = doc["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["api", "db", "events", "cache"]);
    assert_eq!(doc["direction"], "down");
    assert_eq!(doc["groups"][0]["members"], serde_json::json!(["api", "db", "events", "cache"]));
    assert_eq!(doc["notes"][0]["attach"], "events");
    let edges: Vec<(&str, &str)> = doc["edges"].as_array().unwrap().iter().map(|e| (e["from"].as_str().unwrap(), e["to"].as_str().unwrap())).collect();
    assert_eq!(edges, vec![("api", "db"), ("api", "events"), ("api", "cache")]);
    // the edited spec renders
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None).code, 0);

    // --set works on a spec too, and a dry run writes nothing
    let before = fs::read(dir.path().join("o.yaml")).unwrap();
    let d = run_in(dir.path(), &["edit", "o.yaml", "--set", "#cache.emphasis=true", "--dry-run"], None).json();
    assert!(d["diff"].as_str().unwrap().contains("+  emphasis: true"), "{d:#}");
    assert_eq!(before, fs::read(dir.path().join("o.yaml")).unwrap());
    // strict: unknown ids and fields fail with a suggestion; a change that breaks the spec is refused
    let code = |args: &[&str]| run_in(dir.path(), args, None).json()["error"]["code"].as_str().unwrap_or("").to_string();
    assert_eq!(code(&["edit", "o.yaml", "--set", "#cahce.label=x", "--dry-run"]), "no-match");
    assert_eq!(code(&["edit", "o.yaml", "--set", "#cache.lable=x", "--dry-run"]), "unknown-field");
    assert_eq!(code(&["edit", "o.yaml", "--set", "#cache.kind=databse", "--dry-run"]), "invalid-value");
    assert_eq!(before, fs::read(dir.path().join("o.yaml")).unwrap());
}

// ------------------------------------------------------------ phase 4: preview

use resvg::tiny_skia::Pixmap;

fn preview(dir: &Path, file: &str, out: &str, extra: &[&str]) -> (Run, Option<Pixmap>) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    let mut args = vec!["preview", src.to_str().unwrap(), "-o", out, "--no-config"];
    args.extend(extra);
    let r = run_in(dir, &args, None);
    let png = fs::read(dir.join(out)).ok().and_then(|b| Pixmap::decode_png(&b).ok());
    (r, png)
}

fn rgb(p: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let c = p.pixel(x, y).unwrap();
    (c.red(), c.green(), c.blue())
}

/// R13: a bounded PNG for vision models.
#[test]
fn r13_preview_is_a_png_of_bounded_width() {
    let dir = tempfile::tempdir().unwrap();
    let (r, png) = preview(dir.path(), CLEAN, "a.png", &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let v = r.json();
    assert_valid(&schema("findings"), &v);
    let png = png.expect("a decodable PNG");
    // orders.svg is 720 x 240, so 1200 px wide gives 400 px tall
    assert_eq!((png.width(), png.height()), (1200, 400));
    assert_eq!((v["image"]["width"].as_u64(), v["image"]["height"].as_u64()), (Some(1200), Some(400)));
    assert_eq!(v["outputs"][0]["bytes"].as_u64().unwrap(), fs::metadata(dir.path().join("a.png")).unwrap().len());
    assert_eq!(v["validated"], false);
    // the accent node is drawn where the SVG puts it: (360, 120) in the viewBox
    assert_eq!(rgb(&png, 600, 150), (0x25, 0x63, 0xeb));

    let (wide, png) = preview(dir.path(), CLEAN, "wide.png", &["--width", "5000"]);
    assert_eq!(wide.code, 0);
    assert_eq!(png.unwrap().width(), 2000, "never wider than 2000 px");
    assert_eq!(wide.json()["image"]["width"], 2000);
    let (small, png) = preview(dir.path(), CLEAN, "small.png", &["--width", "300"]);
    assert_eq!((small.code, png.unwrap().width()), (0, 300));
}

#[test]
fn preview_is_deterministic_and_draws_text_with_the_bundled_font() {
    let dir = tempfile::tempdir().unwrap();
    preview(dir.path(), CLEAN, "a.png", &[]);
    preview(dir.path(), CLEAN, "b.png", &[]);
    assert_eq!(fs::read(dir.path().join("a.png")).unwrap(), fs::read(dir.path().join("b.png")).unwrap());
    // Text is really drawn: the label area of a node holds dark ink on the light fill.
    let png = Pixmap::decode_png(&fs::read(dir.path().join("a.png")).unwrap()).unwrap();
    let ink = (130..240).flat_map(|x| (185..215).map(move |y| (x, y))).filter(|(x, y)| rgb(&png, *x, *y).0 < 0x60).count();
    assert!(ink > 100, "expected glyphs in the Storefront label, found {ink} dark pixels");
}

#[test]
fn preview_background_annotate_and_crop() {
    let dir = tempfile::tempdir().unwrap();
    // broken.svg has no background of its own
    let (_, light) = preview(dir.path(), BROKEN, "light.png", &[]);
    let (_, dark) = preview(dir.path(), BROKEN, "dark.png", &["--background", "dark"]);
    assert_eq!(rgb(&light.unwrap(), 5, 5), (255, 255, 255));
    assert_eq!(rgb(&dark.unwrap(), 5, 5), (0x0d, 0x11, 0x17));

    let (r, marked) = preview(dir.path(), BROKEN, "marked.png", &["--annotate"]);
    let v = r.json();
    assert_eq!(r.code, 0, "a preview succeeds even when the file has findings");
    assert!(v["image"]["annotated"].as_u64().unwrap() >= 5, "{v:#}");
    assert!(rules_of(&v).contains(&"text-overflow".to_string()));
    assert_ne!(fs::read(dir.path().join("marked.png")).unwrap(), fs::read(dir.path().join("light.png")).unwrap());
    // The foreignObject finding is outlined in the error colour along its box: x=300..380, y=150..180 at 3 px per unit.
    let marked = marked.unwrap();
    let red = (890..1150).filter(|x| rgb(&marked, *x, 447) == (0xdc, 0x26, 0x26)).count();
    assert!(red > 200, "expected the outline of the foreignObject box, found {red} px");

    let (c, png) = preview(dir.path(), CLEAN, "crop.png", &["--crop", "#node-api", "--width", "600"]);
    let cv = c.json();
    let region: Vec<f64> = cv["image"]["region"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
    // the node is 288..432 x 88..152; the crop holds it with a margin and stays inside the drawing
    assert!(region[0] < 288.0 && region[0] + region[2] > 432.0 && region[1] < 88.0 && region[1] + region[3] > 152.0 && region[2] < 400.0, "{region:?}");
    let png = png.unwrap();
    assert_eq!(png.width(), 600);
    assert_eq!(rgb(&png, 300, png.height() / 2), (0x25, 0x63, 0xeb));
}

#[test]
fn preview_errors_and_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let (first, _) = preview(dir.path(), CLEAN, "a.png", &[]);
    assert_eq!(first.code, 0);
    let (again, _) = preview(dir.path(), CLEAN, "a.png", &[]);
    assert_eq!((again.code, again.json()["error"]["code"].as_str()), (4, Some("output-exists")));
    assert_eq!(preview(dir.path(), CLEAN, "a.png", &["--force"]).0.code, 0);
    let (miss, _) = preview(dir.path(), CLEAN, "b.png", &["--crop", "#nope"]);
    assert_eq!((miss.code, miss.json()["error"]["code"].as_str()), (3, Some("no-match")));
    assert_eq!(preview(dir.path(), CLEAN, "c.png", &["--background", "blue"]).0.code, 2);
    assert_eq!(preview(dir.path(), CLEAN, "c.png", &["--width", "4"]).0.code, 2);
    assert_eq!(preview(dir.path(), "tests/fixtures/nope.svg", "c.png", &[]).0.code, 3);
    assert_eq!(run_in(dir.path(), &["preview", "x.svg"], None).code, 2);
    // -o - writes the PNG itself
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(CLEAN);
    let out = Command::new(BIN).args(["preview", src.to_str().unwrap(), "-o", "-"]).current_dir(dir.path()).stdin(Stdio::null()).output().unwrap();
    assert_eq!(&out.stdout[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(out.stdout, fs::read(dir.path().join("a.png")).unwrap());
    // rendering then previewing a spec works end to end
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS);
    assert_eq!(run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "o.svg"], None).code, 0);
    let end = run_in(dir.path(), &["preview", "o.svg", "-o", "o.png", "--annotate"], None).json();
    assert_eq!(end["image"]["annotated"], 0);
}

#[test]
fn findings_on_drawn_elements_carry_a_box() {
    let v = run(&["validate", BROKEN, "--no-config", "--rules", "duplicate-id,dangling-reference,script-content,title-missing"]).json();
    for f in v["findings"].as_array().unwrap() {
        assert_eq!(f["bbox"].is_array(), f["rule"] != "title-missing", "{f:#}");
    }
}

// ------------------------------------------- phase 5: cross-platform and speed

/// R8: same input, config and version give byte-identical output. The files in
/// tests/golden were produced on one machine; CI runs this test on Linux, macOS
/// and Windows, so a pass on all three is the cross-platform comparison.
/// Regenerate with SVGDOG_UPDATE_GOLDEN=1 after an intended change.
#[test]
fn r8_output_matches_the_golden_files() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let update = std::env::var_os("SVGDOG_UPDATE_GOLDEN").is_some();
    let dir = tempfile::tempdir().unwrap();
    let mut produced: Vec<(String, Vec<u8>)> = Vec::new();
    for name in ["orders", "checkout", "tiers", "rack", "login", "walkthrough", "states", "migration", "split"] {
        let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}.yaml"));
        let out = format!("{name}.svg");
        let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", &out, "--no-config"], None);
        assert!(r.code <= 1, "{name}: {}", r.stdout);
        produced.push((out.clone(), fs::read(dir.path().join(&out)).unwrap()));
    }
    // one frame of the walk-through: a step in focus, the rest dimmed
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/walkthrough.yaml");
    run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "frame.svg", "--frames", "--theme", "docs-dark", "--no-config"], None);
    produced.push(("walkthrough-dark.2.svg".into(), fs::read(dir.path().join("frame.2.svg")).unwrap()));
    // the dark theme and outlined text exercise the remaining emitter paths
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS);
    run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "orders-dark-paths.svg", "--theme", "docs-dark", "--text-as-paths", "--no-config"], None);
    produced.push(("orders-dark-paths.svg".into(), fs::read(dir.path().join("orders-dark-paths.svg")).unwrap()));
    // one drawing per built-in theme beyond the default: type, lines and corners all differ
    for (fixture, theme, out) in [("orders", "blueprint-dark", "orders-blueprint-dark.svg"), ("walkthrough", "paper", "walkthrough-paper.svg"), ("login", "bold-auto", "login-bold-auto.svg")] {
        let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{fixture}.yaml"));
        let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", out, "--theme", theme, "--no-config"], None);
        assert_eq!(r.code, 0, "{out}: {}", r.stdout);
        produced.push((out.into(), fs::read(dir.path().join(out)).unwrap()));
    }
    // findings and scene data are output too
    let broken = Path::new(env!("CARGO_MANIFEST_DIR")).join(BROKEN);
    let v = run_in(dir.path(), &["validate", broken.to_str().unwrap(), "--no-config", "--max-findings", "500"], None);
    produced.push(("broken.findings.json".into(), v.stdout.replace(&broken.to_str().unwrap().replace('\\', "\\\\"), "broken.svg").into_bytes()));
    let i = run_in(dir.path(), &["inspect", dir.path().join("orders.svg").to_str().unwrap(), "--fields", "id,tag,bbox,text,fill,stroke,refs"], None);
    produced.push(("orders.scene.json".into(), i.stdout.replace(&dir.path().join("orders.svg").to_str().unwrap().replace('\\', "\\\\"), "orders.svg").into_bytes()));
    // and the PNG: text measured and drawn with the bundled fonts
    run_in(dir.path(), &["preview", "orders.svg", "-o", "orders.png", "--width", "800", "--annotate", "--no-config"], None);
    produced.push(("orders.png".into(), fs::read(dir.path().join("orders.png")).unwrap()));

    for (name, bytes) in produced {
        let path = golden.join(&name);
        if update {
            fs::write(&path, &bytes).unwrap();
            continue;
        }
        let want = fs::read(&path).unwrap_or_else(|_| panic!("{} is missing; run with SVGDOG_UPDATE_GOLDEN=1", path.display()));
        assert!(want == bytes, "{name} differs from tests/golden/{name} ({} vs {} bytes). If the change is intended, regenerate with SVGDOG_UPDATE_GOLDEN=1.", bytes.len(), want.len());
    }
}

/// R15: cold start under 100 ms. Debug builds and loaded CI machines get slack.
#[test]
fn r15_cold_start_is_fast() {
    let budget = if cfg!(debug_assertions) { 1.0 } else { 0.1 };
    let best = (0..5)
        .map(|_| {
            let start = std::time::Instant::now();
            let r = run(&["--version"]);
            assert_eq!(r.code, 0);
            start.elapsed().as_secs_f64()
        })
        .fold(f64::INFINITY, f64::min);
    assert!(best < budget, "cold start took {best:.3} s");
}

#[test]
fn sequence_diagrams_put_lifelines_in_order_and_messages_top_to_bottom() {
    let (v, svg) = render_fixture("tests/fixtures/login.yaml", &["--theme", "docs-light"]);
    assert_eq!(v["findings"], serde_json::json!([]), "{v:#}");
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("l.svg"), &svg).unwrap();
    let scene = run_in(dir.path(), &["inspect", "l.svg", "--depth", "1", "--fields", "id,bbox"], None).json();
    let top = |id: &str| -> Vec<f64> {
        let e = scene["elements"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("no {id}"));
        e["bbox"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect()
    };
    // lifelines left to right in spec order, each a header with a line under it
    let xs: Vec<f64> = ["node-user", "node-app", "node-auth", "node-db", "node-mail"].iter().map(|id| top(id)[0] + top(id)[2] / 2.0).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    assert!(top("node-user")[3] > 300.0, "a lifeline runs down the diagram");
    // messages top to bottom in listed order
    let ys: Vec<f64> = ["edge-user--app", "edge-app--auth", "edge-auth--db", "edge-found", "edge-make-code", "edge-auth--mail", "edge-accepted", "edge-prompt", "edge-code-in", "edge-verify", "edge-session"].iter().map(|id| top(id)[1] + top(id)[3]).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
    // the group frames its members' headers, and a message runs between its two lifelines
    let (frame, auth, db) = (top("group-backend"), top("node-auth"), top("node-db"));
    assert!(frame[0] < auth[0] && frame[0] + frame[2] > db[0] + db[2]);
    let line = svg.split("id=\"edge-app--auth\"").nth(1).unwrap().split("d=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    let nums: Vec<f64> = line.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter(|t| !t.is_empty()).map(|t| t.parse().unwrap()).collect();
    assert_eq!((nums[0], nums[2]), (xs[1], xs[2]), "{line}");
    assert_eq!(nums[1], nums[3], "a message is a level line: {line}");
}

/// Editing a YAML spec keeps its comments and layout; only what cannot be placed
/// in the text makes svgdog rewrite the file, and it says so.
#[test]
fn spec_edits_keep_yaml_comments() {
    let dir = tempfile::tempdir().unwrap();
    let src = "# Order flow, owned by the checkout team\nversion: 1\ndiagram: flow\ntitle: Orders   # shown as the image title\ndescription: How an order is placed.\n\nnodes:\n  # the public entry point\n  - id: web\n    label: Storefront\n    kind: client\n  - id: api          # owns the order lifecycle\n    label: Order API\n  - {id: db, label: Orders DB, kind: datastore}\n\ngroups:\n  - id: backend\n    label: Backend\n    members: [api, db]\n\nedges:\n  - from: web\n    to: api\n    label: POST /orders   # idempotent\n  - {from: api, to: db}\n\nnotes:\n  - attach: db\n    text: Nightly backup\n";
    fs::write(dir.path().join("o.yaml"), src).unwrap();
    let ops = serde_json::json!([
        { "op": "set", "target": "#api", "attr": "label", "value": "Orders: API" },
        { "op": "set", "target": "#api", "attr": "emphasis", "value": true },
        { "op": "set", "target": "#db", "attr": "label", "value": "Orders store" },
        { "op": "set", "target": ":root", "attr": "direction", "value": "down" },
        { "op": "set", "target": "#web--api", "attr": "style", "value": "async" },
        { "op": "remove", "target": "#web", "attr": "kind" },
        { "op": "rename-id", "target": "#db", "to": "store" },
        { "op": "add-node", "target": "#backend", "attrs": { "id": "cache", "label": "Cache", "kind": "datastore" } },
        { "op": "add-edge", "from": "api", "to": "cache", "attrs": { "label": "read" } }
    ]);
    fs::write(dir.path().join("ops.json"), ops.to_string()).unwrap();
    let r = run_in(dir.path(), &["edit", "o.yaml", "--ops", "ops.json", "--in-place"], None);
    let v = r.json();
    assert!(r.code <= 1, "{}", r.stdout);
    assert_eq!((v["formatting"].as_str(), v["operations"]["applied"].as_u64()), (Some("preserved"), Some(9)), "{v:#}");
    let out = fs::read_to_string(dir.path().join("o.yaml")).unwrap();
    let want = "# Order flow, owned by the checkout team\nversion: 1\ndiagram: flow\ntitle: Orders   # shown as the image title\ndescription: How an order is placed.\n\nnodes:\n  # the public entry point\n  - id: web\n    label: Storefront\n  - id: api          # owns the order lifecycle\n    label: \"Orders: API\"\n    emphasis: true\n  - {id: store, label: Orders store, kind: datastore}\n  - {id: cache, label: Cache, kind: datastore}\n\ngroups:\n  - id: backend\n    label: Backend\n    members: [api, store, cache]\n\nedges:\n  - from: web\n    to: api\n    label: POST /orders   # idempotent\n    style: async\n  - {from: api, to: store}\n  - {from: api, to: cache, label: read}\n\nnotes:\n  - attach: store\n    text: Nightly backup\ndirection: down\n";
    assert_eq!(out, want);
    // the edited spec still renders
    assert!(run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None).code <= 1);

    // Removing a node takes edges, notes and memberships with it: that is a rewrite, and the envelope says so.
    let rm = run_in(dir.path(), &["edit", "o.yaml", "--ops", "-", "--in-place"], Some("[{\"op\":\"remove\",\"target\":\"#cache\"}]")).json();
    assert_eq!(rm["formatting"], "rewritten", "{rm:#}");
    let rewritten = fs::read_to_string(dir.path().join("o.yaml")).unwrap();
    assert!(!rewritten.contains('#') && !rewritten.contains("cache"));
    // JSON specs have no comments to keep; they are always rewritten.
    let doc: Value = serde_yaml_ng::from_str(&rewritten).unwrap();
    fs::write(dir.path().join("o.json"), doc.to_string()).unwrap();
    let j = run_in(dir.path(), &["edit", "o.json", "--set", "#api.label=API", "--in-place"], None).json();
    assert_eq!(j["formatting"], "rewritten");
}

// ------------------------------------------------------------------ short specs

#[test]
fn short_edges_and_long_edges_draw_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let head = "version: 1\ndiagram: flow\ntitle: T\ndescription: D\n";
    let long = format!("{head}nodes:\n  - {{id: web, label: web}}\n  - {{id: api, label: api}}\n  - {{id: order-db, label: order db}}\n  - {{id: bus, label: bus}}\nedges:\n  - {{from: web, to: api, label: \"GET /orders: all\"}}\n  - {{from: api, to: order-db, style: async}}\n  - {{from: api, to: bus, style: dependency}}\n  - {{from: web, to: bus, arrow: both}}\n  - {{from: bus, to: order-db, arrow: none}}\n");
    let short = format!("{head}edges:\n  - \"web -> api: GET /orders: all\"\n  - api --> order-db\n  - api ..> bus\n  - web <-> bus\n  - bus -- order-db\n");
    fs::write(dir.path().join("long.yaml"), long).unwrap();
    fs::write(dir.path().join("short.yaml"), short).unwrap();
    for n in ["long", "short"] {
        let r = run_in(dir.path(), &["render", &format!("{n}.yaml"), "-o", &format!("{n}.svg")], None);
        assert_eq!(r.code, 0, "{}", r.stdout);
    }
    assert_eq!(fs::read(dir.path().join("long.svg")).unwrap(), fs::read(dir.path().join("short.svg")).unwrap());
    // the schema accepts what the parser accepts
    let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(dir.path().join("short.yaml")).unwrap()).unwrap();
    assert_valid(&schema("spec"), &doc);
}

#[test]
fn short_edge_mistakes_are_explained() {
    let head = "version: 1\ndiagram: flow\ntitle: T\n";
    let fails = |body: &str| run_in(Path::new("."), &["render", "-", "-o", "-"], Some(&format!("{head}{body}"))).json()["error"].clone();
    let e = fails("edges: [\"a => b\"]\n");
    assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some("invalid-value"), Some("edges[0]")), "{e:#}");
    assert!(e["message"].as_str().unwrap().contains("'=>' is not an arrow") && e["suggestion"].as_str().unwrap().contains("from -> to: label"));
    let e = fails("edges: [\"a->b\"]\n");
    assert!(e["message"].as_str().unwrap().contains("two ids with an arrow between them"), "{e:#}");
    // a mention one letter away from a declared node is a typo, not a new box
    let e = fails("nodes: [{id: orders, label: Orders}, {id: api, label: API}]\nedges: [\"api -> order\"]\n");
    assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some("unknown-reference"), Some("edges[0].to")), "{e:#}");
    assert!(e["suggestion"].as_str().unwrap().contains("Did you mean 'orders'?"));
    // declaring it settles the matter
    let ok = run_in(Path::new("."), &["render", "-", "-o", "-", "--no-validate"], Some(&format!("{head}nodes: [{{id: orders, label: Orders}}, {{id: order, label: Order}}]\nedges: [\"order -> orders\"]\n")));
    assert_eq!(ok.code, 0, "{}", ok.stdout);
    // nothing to draw
    let e = fails("");
    assert_eq!(e["code"], "missing-field");
    // a group can bring nodes in too
    let ok = run_in(Path::new("."), &["render", "-", "-o", "-", "--no-validate"], Some(&format!("{head}groups: [{{id: g, members: [a, b]}}]\n")));
    assert!(ok.code == 0 && ok.stdout.contains("id=\"node-a\""), "{}", ok.stdout);
}

#[test]
fn edits_reach_short_edges_and_mentioned_nodes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("s.yaml"), "version: 1\ndiagram: flow\ntitle: T\ndescription: D\nedges:\n  - \"web -> api: call\"\n  - api --> db\n").unwrap();
    let ops = serde_json::json!([
        { "op": "set", "target": "#api", "attr": "label", "value": "Order API" },
        { "op": "set", "target": "#api--db", "attr": "label", "value": "write" },
        { "op": "rename-id", "target": "#web", "to": "site" }
    ]);
    let r = run_in(dir.path(), &["edit", "s.yaml", "--ops", "-", "--in-place"], Some(&ops.to_string()));
    let v = r.json();
    assert!(r.code <= 1 && v["operations"]["applied"] == 3, "{v:#}");
    let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(dir.path().join("s.yaml")).unwrap()).unwrap();
    assert_eq!(doc["nodes"], serde_json::json!([{ "id": "api", "label": "Order API" }, { "id": "site", "label": "web" }]));
    assert_eq!(doc["edges"], serde_json::json!([{ "from": "site", "to": "api", "label": "call" }, { "from": "api", "to": "db", "label": "write", "style": "async" }]));
    // removing a mentioned node takes its short edges with it
    let v = run_in(dir.path(), &["edit", "s.yaml", "--set", "#db.label=Store", "--in-place"], None).json();
    assert_eq!(v["operations"]["applied"], 1, "{v:#}");
    let rm = run_in(dir.path(), &["edit", "s.yaml", "--ops", "-", "--in-place"], Some("[{\"op\":\"remove\",\"target\":\"#db\"}]"));
    assert!(rm.code <= 1, "{}", rm.stdout);
    let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(dir.path().join("s.yaml")).unwrap()).unwrap();
    assert_eq!(doc["edges"].as_array().unwrap().len(), 1);
}

#[test]
fn every_example_renders_without_findings() {
    let dir = tempfile::tempdir().unwrap();
    let list = run(&["examples"]).json();
    let names: Vec<String> = list["examples"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect();
    assert!(names.len() >= 9);
    let spec_schema = schema("spec");
    for n in &names {
        let one = run(&["examples", n]).json();
        let text = one["examples"][0]["spec"].as_str().unwrap();
        assert_eq!(run(&["examples", n, "--format", "text"]).stdout, text, "text output is the spec itself");
        assert_valid(&spec_schema, &serde_yaml_ng::from_str::<Value>(text).unwrap());
        for theme in ["docs-light", "docs-dark"] {
            let r = run_in(dir.path(), &["render", "-", "-o", "x.svg", "--force", "--theme", theme, "--profile", "strict"], Some(text));
            let v = r.json();
            assert_eq!((r.code, v["findings"].as_array().unwrap().len()), (0, 0), "{n} {theme}: {v:#}");
        }
    }
    let e = run(&["examples", "flwo"]).json();
    assert_eq!((e["error"]["code"].as_str(), e["error"]["suggestion"].as_str()), (Some("unknown-example"), Some("Did you mean 'flow'?")));
}

// ------------------------------------------------------------------ explaining

const WALK: &str = "tests/fixtures/walkthrough.yaml";

#[test]
fn steps_mark_what_they_show_and_are_listed_under_the_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(WALK);
    for theme in ["docs-light", "docs-dark"] {
        let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.svg", "--force", "--theme", theme, "--profile", "strict", "--no-config"], None);
        assert_eq!((r.code, r.json()["findings"].as_array().unwrap().len()), (0, 0), "{theme}: {}", r.stdout);
    }
    let svg = fs::read_to_string(dir.path().join("w.svg")).unwrap();
    // what a step names carries its number, as data and as a badge
    assert!(svg.contains("<g id=\"node-web\" data-step=\"1\">") && svg.contains("<g id=\"edge-api--queue\" data-step=\"3\">") && svg.contains("<g id=\"group-backend\" data-step=\"3\">"), "{svg}");
    assert_eq!(svg.matches("data-badge=\"2\"").count(), 5, "four things and the list entry");
    // the footer: legend, one entry per step, caption
    for id in ["legend", "steps", "step-1", "step-4", "caption"] {
        assert!(svg.contains(&format!("<g id=\"{id}\">")), "{id} is missing");
    }
    assert!(svg.contains("asynchronous message") && svg.contains("highlighted") && svg.contains("Figure 1."));
    // a link wraps its element, a detail is its tooltip
    assert!(svg.contains("<a href=\"https://example.com/storefront\">\n  <g id=\"node-web\""), "{svg}");
    assert!(svg.contains("<g id=\"node-api\" data-step=\"2\">\n    <title>Owns the order lifecycle.</title>"));
    // the scene still sees every element at its place
    let scene = run_in(dir.path(), &["inspect", "w.svg", "--select", "#node-web", "--fields", "id,bbox"], None).json();
    assert!(scene.to_string().contains("node-web"));
}

#[test]
fn frames_draw_one_step_each_on_the_same_canvas() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(WALK);
    let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "out/../w.svg", "--frames", "--profile", "strict", "--no-config"], None);
    let _ = r;
    let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.svg", "--frames", "--profile", "strict", "--no-config"], None);
    let v = r.json();
    assert_eq!((r.code, v["summary"]["files"].as_u64(), v["findings"].as_array().unwrap().len()), (0, Some(4), 0), "{v:#}");
    let paths: Vec<&str> = v["outputs"].as_array().unwrap().iter().map(|o| o["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["w.1.svg", "w.2.svg", "w.3.svg", "w.4.svg"]);
    assert!(!dir.path().join("w.svg").exists());
    let frames: Vec<String> = paths.iter().map(|p| fs::read_to_string(dir.path().join(p)).unwrap()).collect();
    let head = |s: &str| s.lines().next().unwrap().to_string();
    assert!(frames.iter().all(|f| head(f) == head(&frames[0])), "every frame has the same canvas");
    let rect_of = |s: &str, id: &str| s.split(&format!("<g id=\"{id}\"")).nth(1).unwrap().lines().find(|l| l.contains("<rect") || l.contains("<path")).unwrap().split(" fill=").next().unwrap().to_string();
    assert_eq!(rect_of(&frames[0], "node-api"), rect_of(&frames[2], "node-api"), "nothing moves between frames");
    // frame 1 marks only step 1, keeps the storefront at full strength and dims the database
    let f1 = &frames[0];
    assert_eq!((f1.matches("data-badge=\"1\"").count(), f1.matches("data-badge=\"2\"").count()), (3, 1), "badges of other steps appear only in the list");
    assert!(f1.contains("url(#arrow-dim)") && f1.contains("url(#arrow)"));
    let fill_of = |s: &str, id: &str| s.split(&format!("<g id=\"{id}\"")).nth(1).unwrap().split("fill=\"").nth(1).unwrap()[..7].to_string();
    assert_eq!((fill_of(f1, "node-web"), fill_of(f1, "node-db")), ("#dbeafe".to_string(), "#ffffff".to_string()));
    // a second run needs --force, and frames need steps and a file
    let again = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.svg", "--frames", "--no-config"], None);
    assert_eq!((again.code, again.json()["error"]["code"].as_str().map(|s| s.to_string())), (4, Some("output-exists".into())));
    let orders = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS);
    let none = run_in(dir.path(), &["render", orders.to_str().unwrap(), "-o", "o.svg", "--frames", "--no-config"], None);
    assert_eq!((none.code, none.json()["error"]["code"].as_str().map(|s| s.to_string())), (3, Some("no-steps".into())));
    assert_eq!(run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "-", "--frames"], None).code, 2);
}

#[test]
fn focus_dims_everything_else() {
    let head = "version: 1\ndiagram: flow\ntitle: T\ndescription: D\n";
    let body = "nodes:\n  - {id: a, label: A}\n  - {id: b, label: B}\n  - {id: c, label: C}\n  - {id: d, label: D, emphasis: true}\ngroups:\n  - {id: g, members: [c, d]}\nedges: [\"a -> b\", \"b -> c\", \"c -> d\"]\n";
    let draw = |focus: &str| {
        let r = run_in(Path::new("."), &["render", "-", "-o", "-", "--no-config", "--no-validate"], Some(&format!("{head}{focus}{body}")));
        assert_eq!(r.code, 0, "{}", r.stdout);
        r.stdout
    };
    let stroke_of = |s: &str, id: &str| s.split(&format!("<g id=\"{id}\">")).nth(1).unwrap().split("stroke=\"").nth(1).unwrap()[..7].to_string();
    let all = draw("");
    assert!(!all.contains("arrow-dim"));
    // two nodes: the edge between them stays, the others fade
    let two = draw("focus: [a, b]\n");
    assert_eq!((stroke_of(&two, "edge-a--b"), stroke_of(&two, "edge-b--c"), stroke_of(&two, "node-c")), ("#1f2937".into(), "#6b7280".into(), "#6b7280".into()));
    // a group brings its members and the edges inside it; emphasis survives only in focus
    let grp = draw("focus: [g]\n");
    assert_eq!((stroke_of(&grp, "node-c"), stroke_of(&grp, "edge-c--d"), stroke_of(&grp, "node-a")), ("#1f2937".into(), "#1f2937".into(), "#6b7280".into()));
    assert!(grp.contains("fill=\"#2563eb\"") && !two.contains("fill=\"#2563eb\""));
    // dimming changes colours only: the geometry is the same
    let strip = |s: &str| s.lines().filter(|l| l.contains("<rect") || l.contains("<path d=\"M")).map(|l| l.split(" fill=").next().unwrap().to_string()).filter(|l| !l.contains("marker")).collect::<Vec<_>>();
    assert_eq!(strip(&all), strip(&two));
    // dimmed drawings still pass the strict profile, contrast included
    for theme in ["docs-light", "docs-dark"] {
        let r = run_in(Path::new("."), &["render", "-", "-o", "-", "--no-config", "--profile", "strict", "--theme", theme, "--format", "json"], Some(&format!("{head}focus: [a, b]\n{body}")));
        assert_eq!(r.code, 0, "{theme}");
    }
}

#[test]
fn explanation_fields_are_checked() {
    let base = "version: 1\ndiagram: flow\ntitle: T\nnodes:\n  - {id: a, label: A}\n  - {id: b, label: B}\nedges:\n  - {from: a, to: b}\n";
    let cases = [
        (format!("{base}steps:\n  - {{text: x, show: [zz]}}\n"), "unknown-reference", "steps[0].show[0]"),
        (format!("{base}steps:\n  - {{show: [a]}}\n"), "missing-field", "steps[0].text"),
        (format!("{base}steps:\n  - {{text: x, shows: [a]}}\n"), "unknown-field", "steps[0].shows"),
        (format!("{base}steps: [3]\n"), "invalid-spec", "steps[0]"),
        (format!("{base}focus: [a, nope]\n"), "unknown-reference", "focus[1]"),
        (format!("{base}legend: yes please\n"), "invalid-spec", "legend"),
        (format!("{base}legend:\n  - {{text: x}}\n"), "invalid-spec", "legend[0]"),
        (format!("{base}legend:\n  - {{style: async, kind: box, text: x}}\n"), "invalid-spec", "legend[0]"),
        (format!("{base}legend:\n  - {{style: wavy, text: x}}\n"), "invalid-value", "legend[0].style"),
        (base.replace("{id: b, label: B}", "{id: b, label: B, link: \"javascript:alert(1)\"}"), "invalid-value", "nodes[1].link"),
        (base.replace("{id: b, label: B}", "{id: b, label: B, link: \"data:text/html,x\"}"), "invalid-value", "nodes[1].link"),
        (base.replace("{from: a, to: b}", "{from: a, to: b, link: \"two words\"}"), "invalid-value", "edges[0].link"),
        (base.replace("{from: a, to: b}", "{from: a, to: b, link: \"#anchor\"}"), "invalid-value", "edges[0].link"),
    ];
    for (spec, code, path) in cases {
        let e = render_err(&spec);
        assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some(code), Some(path)), "{spec}\n{e:#}");
    }
    let many: String = (0..21).map(|i| format!("  - step {i}\n")).collect();
    assert_eq!(render_err(&format!("{base}steps:\n{many}"))["path"], "steps");
    // links a reader may follow
    for link in ["https://example.com/a?b=1#c", "http://example.com", "mailto:team@example.com", "../guide/page.md#part", "docs/a:b.md"] {
        let spec = base.replace("{id: b, label: B}", &format!("{{id: b, label: B, link: \"{link}\"}}"));
        let r = run_in(Path::new("."), &["render", "-", "-o", "-", "--no-config", "--profile", "strict"], Some(&format!("{spec}description: D\n")));
        assert_eq!(r.code, 0, "{link}: {}", r.stdout);
    }
}

#[test]
fn spec_edits_keep_steps_and_focus_pointing_at_the_right_things() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("s.yaml"), "version: 1\ndiagram: flow\ntitle: T\ndescription: D\nfocus: [api, db]\nedges:\n  - \"web -> api\"\n  - \"api -> db\"\nsteps:\n  - {text: One, show: [web, web--api]}\n  - {text: Two, show: [api--db, db]}\n").unwrap();
    let v = run_in(dir.path(), &["edit", "s.yaml", "--ops", "-", "--in-place"], Some("[{\"op\":\"rename-id\",\"target\":\"#db\",\"to\":\"store\"},{\"op\":\"set\",\"target\":\"#api\",\"attr\":\"detail\",\"value\":\"The API.\"}]")).json();
    assert_eq!(v["operations"]["applied"], 2, "{v:#}");
    let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(dir.path().join("s.yaml")).unwrap()).unwrap();
    assert_eq!((doc["focus"].clone(), doc["steps"][1]["show"].clone()), (serde_json::json!(["api", "store"]), serde_json::json!(["api--store", "store"])), "{doc:#}");
    // removing a node removes its edges, and the steps stop naming both
    let rm = run_in(dir.path(), &["edit", "s.yaml", "--ops", "-", "--in-place"], Some("[{\"op\":\"remove\",\"target\":\"#store\"}]"));
    assert!(rm.code <= 1, "{}", rm.stdout);
    let doc: Value = serde_yaml_ng::from_str(&fs::read_to_string(dir.path().join("s.yaml")).unwrap()).unwrap();
    assert_eq!((doc["focus"].clone(), doc["steps"][1].clone()), (serde_json::json!(["api"]), serde_json::json!({ "text": "Two", "show": [] })), "{doc:#}");
}

// ------------------------------------------------------------------ more diagram types

fn draw(spec: &str, extra: &[&str]) -> (i32, String) {
    let mut args = vec!["render", "-", "-o", "-", "--no-config", "--profile", "strict"];
    args.extend(extra);
    let r = run_in(Path::new(env!("CARGO_MANIFEST_DIR")), &args, Some(spec));
    (r.code, r.stdout)
}

/// The x, y, width and height of the first rect in an element's group.
fn rect_in(svg: &str, id: &str) -> [f64; 4] {
    let g = svg.split(&format!("<g id=\"{id}\"")).nth(1).unwrap_or_else(|| panic!("{id} is missing"));
    let line = g.lines().find(|l| l.contains("<rect")).unwrap();
    let num = |k: &str| line.split(&format!(" {k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
    [num("x"), num("y"), num("width"), num("height")]
}

#[test]
fn state_diagrams_have_states_markers_and_loops() {
    let spec = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/states.yaml")).unwrap();
    for theme in ["docs-light", "docs-dark"] {
        for dir in ["right", "down"] {
            let (code, out) = draw(&format!("{spec}direction: {dir}\n"), &["--theme", theme]);
            assert_eq!(code, 0, "{theme} {dir}: {out}");
        }
    }
    let (_, svg) = draw(&spec, &[]);
    // a node is a rounded state unless it says otherwise; the markers are circles without text
    assert!(svg.split("<g id=\"node-draft\">").nth(1).unwrap().lines().nth(1).unwrap().contains("rx=\"12\""));
    let begin = svg.split("<g id=\"node-begin\">").nth(1).unwrap().split("</g>").next().unwrap();
    assert!(begin.contains("<circle") && !begin.contains("<text"), "{begin}");
    let done = svg.split("<g id=\"node-done\">").nth(1).unwrap().split("</g>").next().unwrap();
    assert_eq!(done.matches("<circle").count(), 2, "an end marker is a ring round a dot");
    // a transition back to the same state, and one back to an earlier state, are both drawn
    assert!(svg.contains("id=\"edge-paid--paid\"") && svg.contains("id=\"edge-placed--draft\""));
    // the markers belong to state diagrams
    let e = render_err("version: 1\ndiagram: flow\ntitle: T\nnodes:\n  - {id: a, kind: start}\n");
    assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some("invalid-value"), Some("nodes[0].kind")));
    // and only they may go without a label
    let e = render_err("version: 1\ndiagram: state\ntitle: T\nnodes:\n  - {id: a}\n");
    assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some("missing-field"), Some("nodes[0].label")));
}

#[test]
fn timelines_keep_order_wrap_by_phase_and_run_either_way() {
    let spec = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/migration.yaml")).unwrap();
    let (code, svg) = draw(&spec, &[]);
    assert_eq!(code, 0, "{svg}");
    let x = |id: &str| rect_in(&svg, &format!("node-{id}"))[0];
    let y = |id: &str| rect_in(&svg, &format!("node-{id}"))[1];
    assert!(x("audit") < x("dual") && x("dual") < x("backfill") && x("backfill") < x("cutover"), "in spec order, left to right");
    assert_eq!(y("audit"), y("cutover"));
    assert!(y("retire") > y("audit"), "what does not fit starts a new row");
    assert!(svg.contains("<g id=\"axis\">") && svg.contains(">Jan 2026<") && svg.contains("<g id=\"group-prep\">"));
    // a phase is never split over two rows
    let narrow = spec.replace("title: Migration plan", "title: Migration plan\nmax_width: 700");
    let (code, n) = draw(&narrow, &[]);
    assert_eq!(code, 0, "{n}");
    let ny = |id: &str| rect_in(&n, &format!("node-{id}"))[1];
    assert!(ny("audit") == ny("dual") && ny("backfill") == ny("cutover") && ny("backfill") > ny("audit"));
    let width: f64 = n.split("viewBox=\"0 0 ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
    assert!(width <= 700.0);
    // downwards: one column, dates to the left of the axis
    for theme in ["docs-light", "docs-dark"] {
        let (code, d) = draw(&spec.replace("title: Migration plan", "title: Migration plan\ndirection: down"), &["--theme", theme]);
        assert_eq!(code, 0, "{d}");
        let dy = |id: &str| rect_in(&d, &format!("node-{id}"))[1];
        assert!(dy("audit") < dy("dual") && dy("dual") < dy("backfill") && dy("cutover") < dy("retire"));
        assert!(d.contains("text-anchor=\"end\""));
    }
    // what a timeline does not take
    let base = "version: 1\ndiagram: timeline\ntitle: T\nnodes:\n  - {id: a, label: A}\n  - {id: b, label: B}\n";
    for (extra, path) in [("edges: [\"a -> b\"]\n", "edges"), ("notes:\n  - {attach: a, text: x}\n", "notes"), ("groups:\n  - {id: g, members: [a, h]}\n  - {id: h, members: [b]}\n", "groups[1]")] {
        let e = render_err(&format!("{base}{extra}"));
        assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some("invalid-spec"), Some(path)), "{e:#}");
    }
    let e = render_err("version: 1\ndiagram: flow\ntitle: T\nnodes:\n  - {id: a, label: A, when: now}\n");
    assert_eq!(e["path"], "nodes[0].when");
}

#[test]
fn comparisons_line_up_what_matches_and_mark_what_does_not() {
    let spec = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/split.yaml")).unwrap();
    for theme in ["docs-light", "docs-dark"] {
        assert_eq!(draw(&spec, &["--theme", theme]).0, 0, "{theme}");
    }
    let (_, svg) = draw(&spec, &[]);
    let at = |id: &str| rect_in(&svg, &format!("node-{id}"));
    let mid = |id: &str| at(id)[1] + at(id)[3] / 2.0;
    // same label, or the same match key, means the same row; the two sides are side by side
    assert_eq!(mid("b-web"), mid("a-web"));
    assert_eq!(mid("b-mono"), mid("a-orders"));
    assert!(at("b-web")[0] + at("b-web")[2] < at("a-web")[0]);
    assert!(mid("a-pay") > mid("a-orders") + at("a-orders")[3], "the new node has a row of its own");
    let frames = [rect_in(&svg, "group-before"), rect_in(&svg, "group-after")];
    assert_eq!((frames[0][1], frames[0][3]), (frames[1][1], frames[1][3]), "both frames are the same height");
    // only what has no counterpart is highlighted
    let fill = |id: &str| svg.split(&format!("<g id=\"node-{id}\">")).nth(1).unwrap().split("fill=\"").nth(1).unwrap()[..7].to_string();
    assert_eq!((fill("a-pay"), fill("a-orders"), fill("b-mono")), ("#2563eb".to_string(), "#dbeafe".to_string(), "#dbeafe".to_string()));
    // what a comparison needs
    let base = "version: 1\ndiagram: compare\ntitle: T\nnodes:\n  - {id: a, label: A}\n  - {id: b, label: B}\n  - {id: c, label: C}\n";
    for (extra, path) in [("groups:\n  - {id: g, members: [a, b, c]}\n", "groups"), ("groups:\n  - {id: g, members: [a]}\n  - {id: h, members: [b]}\n", "nodes[2]"), ("groups:\n  - {id: g, members: [a]}\n  - {id: h, members: [b]}\n  - {id: k, members: [c]}\n", "groups")] {
        let e = render_err(&format!("{base}{extra}"));
        assert_eq!((e["code"].as_str(), e["path"].as_str()), (Some("invalid-spec"), Some(path)), "{e:#}");
    }
    let e = render_err("version: 1\ndiagram: flow\ntitle: T\nnodes:\n  - {id: a, label: A, match: x}\n");
    assert_eq!(e["path"], "nodes[0].match");
}

// ------------------------------------------------------------------ living in documentation

#[test]
fn describe_says_what_a_diagram_shows() {
    let v = run(&["describe", WALK]).json();
    assert_eq!((v["ok"].as_bool(), v["title"].as_str()), (Some(true), Some("Order placement")));
    let text = v["text"].as_str().unwrap();
    for part in ["Order placement (flow diagram, left to right)", "- Order API (service, highlighted, in Backend) - Owns the order lifecycle.", "- Order API to Fulfilment queue (publish) [asynchronous]", "Steps:\n1. The storefront sends the order to the API.", "Figure 1."] {
        assert!(text.contains(part), "missing {part:?} in:\n{text}");
    }
    assert_eq!(run(&["describe", WALK, "--format", "text"]).stdout, text);
    assert_eq!(v["spec"]["title"], "Order placement", "the spec comes back as data");
    // one or two sentences for alt text, per diagram type
    let short = |f: &str| run(&["describe", f, "--short", "--format", "text"]).stdout;
    assert_eq!(short("examples/minimal.yaml"), "Flow diagram: browser to api (request); api to database (query); api to queue (publish).\n");
    assert!(short("tests/fixtures/migration.yaml").starts_with("Timeline: Jan 2026, Schema audit; "));
    assert_eq!(short("tests/fixtures/split.yaml"), "Comparison of Before (Web app, Monolith, Database) with After (Web app, Orders service, Payments service, Database).\n");
    let states = short("tests/fixtures/states.yaml");
    assert!(states.starts_with("State diagram: the start to Draft; Draft to Placed (submit); ") && states.contains("Paid to itself") && states.ends_with("and 3 more connections.\n"), "{states}");
}

#[test]
fn a_spec_without_a_description_gets_one_in_the_drawing() {
    let (code, svg) = draw("version: 1\ndiagram: flow\ntitle: T\nedges: [\"a -> b: go\"]\n", &[]);
    assert_eq!(code, 0, "no desc-missing warning any more: {svg}");
    assert!(svg.contains("<desc>Flow diagram: a to b (go).</desc>"));
    let (_, own) = draw("version: 1\ndiagram: flow\ntitle: T\ndescription: Mine & yours.\nedges: [\"a -> b\"]\n", &[]);
    assert!(own.contains("<desc>Mine &amp; yours.</desc>"));
}

#[test]
fn an_embedded_spec_can_be_read_back() {
    let dir = tempfile::tempdir().unwrap();
    let spec = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/steps.yaml")).unwrap();
    fs::write(dir.path().join("s.yaml"), &spec).unwrap();
    let r = run_in(dir.path(), &["render", "s.yaml", "-o", "s.svg", "--embed-spec", "--profile", "strict", "--no-config"], None);
    assert_eq!((r.code, r.json()["findings"].as_array().unwrap().len()), (0, 0), "the embedded spec is not editor metadata: {}", r.stdout);
    let svg = fs::read_to_string(dir.path().join("s.svg")).unwrap();
    assert!(svg.contains("<metadata id=\"svgdog-spec\">{"));
    // the drawing describes itself, and gives the spec back exactly
    let d = run_in(dir.path(), &["describe", "s.svg"], None).json();
    assert_eq!(d["spec"], serde_yaml_ng::from_str::<Value>(&spec).unwrap());
    assert_eq!(d["text"], run_in(dir.path(), &["describe", "s.yaml"], None).json()["text"]);
    // rendering what came back gives the same drawing
    fs::write(dir.path().join("back.json"), d["spec"].to_string()).unwrap();
    run_in(dir.path(), &["render", "back.json", "-o", "back.svg", "--embed-spec", "--no-config"], None);
    assert_eq!(fs::read(dir.path().join("back.svg")).unwrap(), fs::read(dir.path().join("s.svg")).unwrap());
    // fix leaves the spec in place; a drawing without one says so
    let fixed = run_in(dir.path(), &["fix", "s.svg", "--dry-run", "--no-config"], None).json();
    assert_eq!(fixed["changed"], false, "{fixed:#}");
    run_in(dir.path(), &["render", "s.yaml", "-o", "plain.svg", "--no-config"], None);
    assert!(!fs::read_to_string(dir.path().join("plain.svg")).unwrap().contains("<metadata"));
    let e = run_in(dir.path(), &["describe", "plain.svg"], None);
    assert_eq!((e.code, e.json()["error"]["code"].as_str().map(|s| s.to_string())), (3, Some("no-spec".into())));
}

#[test]
fn render_check_reports_stale_output_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS), dir.path().join("o.yaml")).unwrap();
    let missing = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--check"], None);
    let v = missing.json();
    assert_eq!((missing.code, v["ok"].as_bool(), v["stale"][0]["reason"].as_str()), (1, Some(false), Some("missing")), "{v:#}");
    assert!(!dir.path().join("o.svg").exists());
    run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None);
    let fresh = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--check"], None);
    assert_eq!((fresh.code, fresh.json()["stale"].as_array().unwrap().len(), fresh.json()["checked"].as_u64()), (0, 0, Some(1)));
    // the spec moves on, the picture does not
    let edited = fs::read_to_string(dir.path().join("o.yaml")).unwrap().replace("Storefront", "Shop");
    fs::write(dir.path().join("o.yaml"), edited).unwrap();
    let before = fs::read(dir.path().join("o.svg")).unwrap();
    let old = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--check"], None);
    assert_eq!((old.code, old.json()["stale"][0]["reason"].as_str().map(|s| s.to_string())), (1, Some("differs".into())));
    assert_eq!(fs::read(dir.path().join("o.svg")).unwrap(), before);
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "-", "--check"], None).code, 2);
}

#[test]
fn the_adaptive_theme_is_the_light_drawing_with_a_dark_override() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(WALK);
    for (theme, out) in [("docs-auto", "auto.svg"), ("docs-light", "light.svg"), ("docs-dark", "dark.svg")] {
        let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", out, "--theme", theme, "--profile", "strict", "--no-config"], None);
        assert_eq!((r.code, r.json()["findings"].as_array().unwrap().len()), (0, 0), "{theme}: {}", r.stdout);
    }
    let read = |n: &str| fs::read_to_string(dir.path().join(n)).unwrap();
    let (auto, light, dark) = (read("auto.svg"), read("light.svg"), read("dark.svg"));
    assert!(!auto.contains("\"@"), "no placeholder is left behind");
    // strip the classes and the one style rule, and it is the light drawing byte for byte
    let style = auto.lines().find(|l| l.contains("<style>")).unwrap().to_string();
    let mut plain = auto.replace(&format!("{style}\n"), "");
    while let Some(i) = plain.find(" class=\"") {
        let end = i + 8 + plain[i + 8..].find('"').unwrap() + 1;
        plain.replace_range(i..end, "");
    }
    assert_eq!(plain, light);
    // the override holds exactly the dark theme's colours
    assert!(style.trim().starts_with("<style>@media (prefers-color-scheme: dark){") && style.contains(".f-bg{fill:#0d1117}") && style.contains(".s-ns{stroke:#94a3b8}"), "{style}");
    for colour in style.split('#').skip(1).map(|c| format!("#{}", &c[..6])) {
        assert!(dark.contains(&colour), "{colour} is not a dark theme colour");
    }
    // a media query of any other kind is still reported
    let other = auto.replace("(prefers-color-scheme: dark)", "(min-width: 600px)");
    fs::write(dir.path().join("other.svg"), other).unwrap();
    assert!(rules_of(&run_in(dir.path(), &["validate", "other.svg", "--no-config"], None).json()).contains(&"css-unsupported".to_string()));
    // the picture made from it is the light one
    for n in ["auto", "light"] {
        run_in(dir.path(), &["preview", &format!("{n}.svg"), "-o", &format!("{n}.png"), "--width", "600"], None);
    }
    assert_eq!(fs::read(dir.path().join("auto.png")).unwrap(), fs::read(dir.path().join("light.png")).unwrap());
}

const PAGE: &str = "# Page\n\nWords.\n\n```svgdog request-path\nversion: 1\ndiagram: flow\ntitle: Request path\nedges:\n  - \"browser -> api: request\"\n```\nMore words.\n\n```yaml\nnot: ours\n```\n\n<!-- svgdog\nversion: 1\ndiagram: timeline\ntitle: Plan\ndescription: Two [milestones].\nnodes:\n  - { id: a, label: Start, when: Jan }\n  - { id: b, label: Finish, when: Mar }\n-->\n\nThe end.\n";

#[test]
fn docs_renders_the_diagrams_inside_markdown_pages() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("guide/deep")).unwrap();
    fs::write(dir.path().join("guide/page.md"), PAGE).unwrap();
    fs::write(dir.path().join("guide/deep/Other.md"), "~~~svgdog\nversion: 1\ndiagram: flow\ntitle: Other\nedges: [\"a -> b\"]\n~~~").unwrap();
    fs::write(dir.path().join("guide/plain.md"), "# No diagrams here\n").unwrap();

    // check first: everything is missing, nothing is written
    let c = run_in(dir.path(), &["docs", "guide", "--check"], None);
    let v = c.json();
    assert_eq!((c.code, v["ok"].as_bool(), v["stale"].as_array().unwrap().len()), (1, Some(false), 5), "three pictures and two pages: {v:#}");
    assert_valid(&schema("findings"), &v);
    assert!(!dir.path().join("guide/diagrams").exists() && fs::read_to_string(dir.path().join("guide/page.md")).unwrap() == PAGE);

    let w = run_in(dir.path(), &["docs", "guide", "--profile", "strict"], None);
    let v = w.json();
    assert_eq!((w.code, v["summary"]["files"].as_u64(), v["summary"]["errors"].as_u64(), v["documents"].as_array().unwrap().len()), (0, Some(3), Some(0), 2), "{v:#}");
    assert_valid(&schema("findings"), &v);
    let paths: Vec<&str> = v["outputs"].as_array().unwrap().iter().map(|o| o["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["guide/deep/diagrams/other-1.svg", "guide/deep/Other.md", "guide/diagrams/page-request-path.svg", "guide/diagrams/page-2.svg", "guide/page.md"]);
    let page = fs::read_to_string(dir.path().join("guide/page.md")).unwrap();
    // an image line follows each block; the rest of the page is untouched
    assert!(page.contains("```\n\n![Request path. Flow diagram: browser to api (request).](diagrams/page-request-path.svg)\n\nMore words.\n"), "{page}");
    assert!(page.contains("-->\n\n![Plan. Two milestones.](diagrams/page-2.svg)\n\nThe end.\n"), "brackets are dropped from alt text: {page}");
    assert!(page.contains("```yaml\nnot: ours\n```\n") && page.starts_with("# Page\n\nWords.\n"));
    assert_eq!(fs::read_to_string(dir.path().join("guide/deep/Other.md")).unwrap().lines().last().unwrap(), "![Other. Flow diagram: a to b.](diagrams/other-1.svg)");
    assert!(fs::read_to_string(dir.path().join("guide/diagrams/page-2.svg")).unwrap().contains("<g id=\"axis\">"));

    // a second run changes nothing, and check agrees
    let again = run_in(dir.path(), &["docs", "guide"], None).json();
    assert_eq!(again["outputs"].as_array().unwrap().len(), 0, "{again:#}");
    assert_eq!(run_in(dir.path(), &["docs", "guide", "--check"], None).code, 0);

    // editing a spec makes its picture stale; the image line is updated in place, not duplicated
    fs::write(dir.path().join("guide/page.md"), page.replace("title: Request path", "title: Request route")).unwrap();
    let stale = run_in(dir.path(), &["docs", "guide/page.md", "--check"], None).json();
    assert_eq!(stale["stale"].as_array().unwrap().len(), 2, "the picture and the alt text: {stale:#}");
    run_in(dir.path(), &["docs", "guide/page.md"], None);
    let page = fs::read_to_string(dir.path().join("guide/page.md")).unwrap();
    assert_eq!((page.matches("![").count(), page.matches("![Request route.").count()), (2, 1), "{page}");

    // themes, another folder for the pictures, and the mistakes
    let auto = run_in(dir.path(), &["docs", "guide/deep", "--theme", "docs-auto", "--assets", "img"], None);
    assert_eq!(auto.code, 0, "{}", auto.stdout);
    assert!(fs::read_to_string(dir.path().join("guide/deep/img/other-1.svg")).unwrap().contains("prefers-color-scheme"));
    assert_eq!(run_in(dir.path(), &["docs", "guide", "--assets", "../out"], None).code, 2);
    assert_eq!(run_in(dir.path(), &["docs", "nowhere"], None).code, 3);
    fs::write(dir.path().join("bad.md"), "text\n\n```svgdog\nversion: 1\ndiagram: flwo\ntitle: T\n```\n").unwrap();
    let e = run_in(dir.path(), &["docs", "bad.md"], None);
    let err = e.json()["error"].clone();
    assert_eq!((e.code, err["code"].as_str()), (3, Some("invalid-value")));
    assert!(err["message"].as_str().unwrap().starts_with("bad.md, diagram starting on line 4: ") && err["suggestion"] == "Did you mean 'flow'?", "{err:#}");
    fs::write(dir.path().join("twice.md"), "```svgdog a\nversion: 1\ndiagram: flow\ntitle: T\nedges: [\"a -> b\"]\n```\n\n```svgdog a\nversion: 1\ndiagram: flow\ntitle: T\nedges: [\"a -> b\"]\n```\n").unwrap();
    assert_eq!(run_in(dir.path(), &["docs", "twice.md"], None).json()["error"]["code"], "duplicate-id");
}

// ------------------------------------------------------------------ interactive versions

/// Remove every ` name="..."` attribute from a document.
fn without_attr(text: &str, name: &str) -> String {
    let mut out = text.to_string();
    let key = format!(" {name}=\"");
    while let Some(i) = out.find(&key) {
        let end = i + key.len() + out[i + key.len()..].find('"').unwrap() + 1;
        out.replace_range(i..end, "");
    }
    out
}

#[test]
fn the_css_version_is_the_static_drawing_plus_classes_and_one_style_block() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(WALK);
    for theme in ["docs-light", "docs-dark", "docs-auto"] {
        let plain = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "plain.svg", "--force", "--theme", theme, "--no-config"], None);
        assert_eq!(plain.code, 0);
        let live = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "live.svg", "--force", "--theme", theme, "--interactive", "css", "--profile", "strict", "--no-config"], None);
        assert_eq!((live.code, live.json()["findings"].as_array().unwrap().len()), (0, 0), "{theme}: no script, nothing for a rule to object to: {}", live.stdout);
        let (plain, live) = (fs::read_to_string(dir.path().join("plain.svg")).unwrap(), fs::read_to_string(dir.path().join("live.svg")).unwrap());
        assert!(!live.contains("<script") && live.contains("data-interactive=\"css\""));
        // take away what was added and the static drawing is left, byte for byte
        let style = live.lines().rfind(|l| l.contains("<style>")).unwrap().to_string();
        assert!(style.contains(":has(#node-api:hover,#node-api:focus-visible) .it:not(.r-api){opacity:.25}") && style.contains("[data-current=\"2\"] .it:not(.st-2){opacity:.25}"), "{style}");
        let mut bare = live.replace(&format!("{style}\n"), "");
        for attr in ["data-interactive", "tabindex"] {
            bare = without_attr(&bare, attr);
        }
        // classes: the scope on the root and the relations on each thing; the adaptive theme's own classes stay
        let scoped: Vec<String> = bare.split(" class=\"").skip(1).map(|c| c.split('"').next().unwrap().to_string()).filter(|c| c.starts_with("svgdog-") || c.starts_with("it")).collect();
        for c in scoped {
            bare = bare.replacen(&format!(" class=\"{c}\""), "", 1);
        }
        assert_eq!(bare, plain, "{theme}");
        // what a hover keeps lit: the node, its neighbours, its frame, and the edges between them
        let class_of = |id: &str| live.split(&format!("id=\"{id}\"")).nth(1).unwrap().split("class=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
        assert_eq!(class_of("node-web"), "it r-api r-web st-1");
        assert_eq!(class_of("edge-web--api"), "it r-api r-web st-1");
        assert!(class_of("node-api").contains("r-backend") && class_of("group-backend").contains("r-queue"));
        // and the picture a static renderer makes of it is the same
        for n in ["plain", "live"] {
            run_in(dir.path(), &["preview", &format!("{n}.svg"), "-o", &format!("{n}.png"), "--width", "500", "--force"], None);
        }
        assert_eq!(fs::read(dir.path().join("plain.png")).unwrap(), fs::read(dir.path().join("live.png")).unwrap(), "{theme}");
    }
}

#[test]
fn the_html_version_is_one_page_that_works_without_its_script() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(WALK);
    let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.html", "--interactive", "html", "--theme", "docs-auto", "--profile", "strict", "--no-config"], None);
    assert_eq!((r.code, r.json()["findings"].as_array().unwrap().len(), r.json()["outputs"][0]["path"].as_str().map(|s| s.to_string())), (0, 0, Some("w.html".into())), "{}", r.stdout);
    let page = fs::read_to_string(dir.path().join("w.html")).unwrap();
    assert!(page.starts_with("<!doctype html>") && page.contains("<title>Order placement</title>") && page.trim_end().ends_with("</html>"));
    // self-contained: nothing is fetched
    for needle in ["src=", "<link", "@import", "url(http", "fetch("] {
        assert!(!page.contains(needle), "the page reaches outside itself with {needle}");
    }
    // the drawing inside is the css version, complete with its step list, so the page reads without the script
    run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.svg", "--interactive", "css", "--theme", "docs-auto", "--no-config"], None);
    assert!(page.contains(&fs::read_to_string(dir.path().join("w.svg")).unwrap()));
    assert!(page.contains("<div class=\"bar\" hidden>") && page.contains("var total = 4;"), "controls stay hidden until the script shows them");
    assert!(page.contains("@media (prefers-color-scheme: dark){:root{--bg:#0d1117"));
    // no steps, no controls
    let orders = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS);
    run_in(dir.path(), &["render", orders.to_str().unwrap(), "-o", "o.html", "--interactive", "html", "--no-config"], None);
    let plain = fs::read_to_string(dir.path().join("o.html")).unwrap();
    assert!(!plain.contains("class=\"bar\"") && plain.contains("var total = 0;"));
    // frames and the page are two answers to the same question
    let both = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "x.html", "--interactive", "html", "--frames"], None);
    assert_eq!((both.code, both.json()["error"]["code"].as_str().map(|s| s.to_string())), (2, Some("argument-conflict".into())));
    // check compares the page like any other output
    assert_eq!(run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.html", "--interactive", "html", "--theme", "docs-auto", "--check", "--no-config"], None).code, 0);
}

#[test]
fn preview_can_show_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(WALK);
    run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "w.svg", "--no-config"], None);
    for (args, out) in [(vec![], "all.png"), (vec!["--step", "2"], "two.png"), (vec!["--step", "3"], "three.png")] {
        let mut a = vec!["preview", "w.svg", "-o", out, "--width", "500"];
        a.extend(args);
        assert_eq!(run_in(dir.path(), &a, None).code, 0);
    }
    let read = |n: &str| fs::read(dir.path().join(n)).unwrap();
    assert!(read("all.png") != read("two.png") && read("two.png") != read("three.png"));
    let e = run_in(dir.path(), &["preview", "w.svg", "-o", "x.png", "--step", "9"], None);
    assert_eq!((e.code, e.json()["error"]["code"].as_str().map(|s| s.to_string())), (3, Some("no-match".into())));
    let none = run_in(dir.path(), &["preview", Path::new(env!("CARGO_MANIFEST_DIR")).join(CLEAN).to_str().unwrap(), "-o", "y.png", "--step", "1"], None);
    assert_eq!(none.code, 3, "a drawing without steps has no step 1");
}

#[test]
fn docs_can_write_the_interactive_versions() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("page.md"), PAGE).unwrap();
    let r = run_in(dir.path(), &["docs", "page.md", "--interactive", "html"], None);
    let v = r.json();
    assert_eq!(r.code, 0, "{v:#}");
    let paths: Vec<&str> = v["outputs"].as_array().unwrap().iter().map(|o| o["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["diagrams/page-request-path.svg", "diagrams/page-request-path.html", "diagrams/page-2.svg", "diagrams/page-2.html", "page.md"]);
    let page = fs::read_to_string(dir.path().join("page.md")).unwrap();
    assert!(page.contains("(diagrams/page-request-path.svg) [Interactive version](diagrams/page-request-path.html)\n"), "{page}");
    // the picture in the page stays static; the page beside it is the interactive one
    assert!(!fs::read_to_string(dir.path().join("diagrams/page-2.svg")).unwrap().contains("data-interactive"));
    assert!(fs::read_to_string(dir.path().join("diagrams/page-2.html")).unwrap().contains("data-interactive=\"css\""));
    assert_eq!(run_in(dir.path(), &["docs", "page.md", "--interactive", "html", "--check"], None).code, 0);
    // without the switch the link goes away again, and check with other flags sees the difference
    assert_eq!(run_in(dir.path(), &["docs", "page.md", "--check"], None).code, 1);
    run_in(dir.path(), &["docs", "page.md", "--interactive", "css"], None);
    let page = fs::read_to_string(dir.path().join("page.md")).unwrap();
    assert!(!page.contains("Interactive version") && page.matches("![").count() == 2, "{page}");
    assert!(fs::read_to_string(dir.path().join("diagrams/page-2.svg")).unwrap().contains("data-interactive=\"css\""));
}

#[test]
fn the_page_script_steps_and_shows_details() {
    // The script is plain JavaScript; it is run against a stand-in document where Node is available.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Ok(out) = Command::new("node").arg(root.join("tests/player.test.js")).arg(root.join("src/render/player.js")).output() else {
        eprintln!("node is not installed; skipping the page script test");
        return;
    };
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("player ok"));
}

/// Where a node's box is in a rendered drawing: x, y, width, height.
fn box_in(dir: &Path, file: &str, id: &str) -> [f64; 4] {
    let v = run_in(dir, &["inspect", file, "--select", &format!("#node-{id}"), "--depth", "1", "--fields", "id,bbox"], None).json();
    // the shape is the first thing drawn in the node's group
    let shape = v["elements"].as_array().unwrap().iter().find(|e| e["depth"] == 2).unwrap_or_else(|| panic!("{id} has a shape in {file}: {v:#}"));
    let b: Vec<f64> = shape["bbox"].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
    [b[0], b[1], b[2], b[3]]
}

/// The centre of one box seen from another: equal before and after means neither moved against the other.
fn apart(dir: &Path, file: &str, a: &str, b: &str) -> (f64, f64) {
    let (p, q) = (box_in(dir, file, a), box_in(dir, file, b));
    (q[0] + q[2] / 2.0 - p[0] - p[2] / 2.0, q[1] + q[3] / 2.0 - p[1] - p[3] / 2.0)
}

fn orders_with(change: impl Fn(String) -> String) -> String {
    change(fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS)).unwrap())
}

#[test]
fn a_drawing_rendered_again_keeps_its_boxes_when_nodes_edges_and_words_change() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("o.yaml"), orders_with(|s| s)).unwrap();
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None).code, 0);
    fs::copy(dir.path().join("o.svg"), dir.path().join("first.svg")).unwrap();

    // two new boxes (one inside the group), two new edges, one label reworded
    let grown = orders_with(|s| {
        s.replace("groups:", "  - id: cache\n    label: Price cache\n    kind: datastore\n  - id: mailer\n    label: Mail provider\n    kind: external\n\ngroups:")
            .replace("members: [api, db, queue]", "members: [api, db, queue, cache]")
            .replace("notes:", "  - from: api\n    to: cache\n    label: read\n  - from: queue\n    to: mailer\n\nnotes:")
            .replace("label: Order API", "label: Orders API")
    });
    fs::write(dir.path().join("o.yaml"), &grown).unwrap();

    // the file exists: without a word about it, nothing is replaced, and the way out is named
    let refused = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg"], None);
    assert_eq!(refused.code, 4);
    assert!(refused.json()["error"]["suggestion"].as_str().unwrap().contains("--previous o.svg"), "{}", refused.stdout);

    // naming it as the earlier drawing updates it in place, without --force
    let r = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--previous", "o.svg", "--profile", "strict", "--no-config"], None);
    let v = r.json();
    assert_eq!((r.code, v["summary"]["errors"].as_u64(), v["summary"]["warnings"].as_u64()), (0, Some(0), Some(0)), "{v:#}");
    assert_valid(&schema("findings"), &v);
    assert_eq!((v["layout"]["kept"].as_u64(), v["layout"]["relaid"].as_bool(), v["layout"]["reasons"].as_array().unwrap().len()), (Some(4), Some(false), 0));
    assert_eq!(v["layout"]["placed"], serde_json::json!(["cache", "mailer"]));
    for (a, b) in [("web", "api"), ("api", "db"), ("api", "queue"), ("db", "queue")] {
        assert_eq!(apart(dir.path(), "first.svg", a, b), apart(dir.path(), "o.svg", a, b), "{a} and {b} stay where they were");
    }
    let text = run_in(dir.path(), &["render", "o.yaml", "-o", "t.svg", "--previous", "first.svg", "--format", "text"], None);
    assert!(text.stdout.contains("Layout kept: 4 boxes in place, 2 added (cache, mailer)."), "{}", text.stdout);

    // rendered over itself nothing changes, so --check agrees it is current
    let after = fs::read(dir.path().join("o.svg")).unwrap();
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--previous", "o.svg"], None).code, 0);
    assert_eq!(fs::read(dir.path().join("o.svg")).unwrap(), after);
    let c = run_in(dir.path(), &["render", "o.yaml", "-o", "o.svg", "--check"], None);
    assert_eq!((c.code, c.json()["stale"].as_array().unwrap().len()), (0, 0), "{}", c.stdout);
    // and it is not the drawing the spec would get from nothing
    assert_eq!(run_in(dir.path(), &["render", "o.yaml", "-o", "fresh.svg"], None).code, 0);
    assert_ne!(fs::read(dir.path().join("fresh.svg")).unwrap(), after);
    assert_ne!(apart(dir.path(), "first.svg", "api", "queue"), apart(dir.path(), "fresh.svg", "api", "queue"), "a fresh layout moves the boxes, which is what --previous avoids");

    // a box that is gone is counted, and the rest still stays
    fs::write(dir.path().join("less.yaml"), orders_with(|s| s.replace("  - id: queue\n    label: Fulfilment queue\n    kind: queue\n", "").replace("members: [api, db, queue]", "members: [api, db]").replace("  - from: api\n    to: queue\n    label: publish\n    style: async\n", "").replace("notes:\n  - attach: queue\n    text: At-least-once delivery\n", ""))).unwrap();
    let less = run_in(dir.path(), &["render", "less.yaml", "-o", "less.svg", "--previous", "first.svg"], None);
    assert_eq!((less.code, less.json()["layout"]["gone"].clone()), (0, serde_json::json!(["queue"])), "{}", less.stdout);
    assert_eq!(apart(dir.path(), "first.svg", "web", "db"), apart(dir.path(), "less.svg", "web", "db"));
}

const CHAIN: &str = "version: 1\ndiagram: flow\ntitle: Chain\ndirection: down\nnodes:\n  - { id: a, label: First }\n  - { id: b, label: Second }\n  - { id: c, label: Third }\nedges: [\"a -> b\", \"b -> c\"]\n";

#[test]
fn a_layout_that_cannot_be_kept_stops_the_render_unless_forced() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("c.yaml"), CHAIN).unwrap();
    assert_eq!(run_in(dir.path(), &["render", "c.yaml", "-o", "c.svg"], None).code, 0);
    let before = fs::read(dir.path().join("c.svg")).unwrap();

    // the middle box becomes six lines tall: it cannot stay between its neighbours
    fs::write(dir.path().join("c.yaml"), CHAIN.replace("label: Second", "label: \"Second\\nwith\\nmany\\nmore\\nlines\\nof text\"")).unwrap();
    let r = run_in(dir.path(), &["render", "c.yaml", "-o", "c.svg", "--previous", "c.svg"], None);
    let v = r.json();
    assert_eq!((r.code, v["ok"].as_bool(), v["error"]["code"].as_str()), (4, Some(false), Some("layout-changed")), "{v:#}");
    assert_valid(&schema("findings"), &v);
    let reasons = v["error"]["details"]["reasons"].as_array().unwrap();
    assert!(!reasons.is_empty() && reasons.iter().all(|x| x["id"] == "b") && v["error"]["suggestion"].as_str().unwrap().contains("--force"), "{v:#}");
    assert_eq!(fs::read(dir.path().join("c.svg")).unwrap(), before, "nothing is written");

    // --force: laid out afresh, and the report says so
    assert_eq!(run_in(dir.path(), &["render", "c.yaml", "-o", "fresh.svg"], None).code, 0);
    let f = run_in(dir.path(), &["render", "c.yaml", "-o", "c.svg", "--previous", "c.svg", "--force"], None);
    let v = f.json();
    assert_eq!((f.code, v["layout"]["relaid"].as_bool(), v["layout"]["reasons"].as_array().unwrap().is_empty()), (0, Some(true), false), "{v:#}");
    assert_valid(&schema("findings"), &v);
    assert_eq!(fs::read(dir.path().join("c.svg")).unwrap(), fs::read(dir.path().join("fresh.svg")).unwrap());

    // an earlier drawing that is not ours cannot be read for its layout
    fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/broken.svg"), dir.path().join("other.svg")).unwrap();
    let o = run_in(dir.path(), &["render", "c.yaml", "-o", "x.svg", "--previous", "other.svg"], None);
    assert_eq!((o.code, o.json()["error"]["code"].as_str().map(|s| s.to_string())), (3, Some("not-svgdog-output".into())));
    assert_eq!(run_in(dir.path(), &["render", "c.yaml", "-o", "x.svg", "--previous", "missing.svg"], None).code, 3);

    // nothing in common is nothing to keep: a plain layout, not a failure
    fs::write(dir.path().join("new.yaml"), CHAIN.replace("id: a", "id: x").replace("id: b", "id: y").replace("id: c", "id: z").replace("a -> b", "x -> y").replace("b -> c", "y -> z")).unwrap();
    let n = run_in(dir.path(), &["render", "new.yaml", "-o", "new.svg", "--previous", "fresh.svg"], None);
    assert_eq!((n.code, n.json()["layout"]["kept"].as_u64(), n.json()["layout"]["gone"].as_array().unwrap().len()), (0, Some(0), 3), "{}", n.stdout);
}

#[test]
fn layouts_that_follow_the_spec_order_are_compared_with_the_earlier_drawing() {
    let dir = tempfile::tempdir().unwrap();
    let login = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/login.yaml")).unwrap();
    fs::write(dir.path().join("l.yaml"), &login).unwrap();
    assert_eq!(run_in(dir.path(), &["render", "l.yaml", "-o", "l.svg"], None).code, 0);

    // one more message: the participants stay in their columns
    fs::write(dir.path().join("more.yaml"), login.replace("notes:", "  - {from: app, to: user, label: signed in, style: async, id: done}\nnotes:")).unwrap();
    let m = run_in(dir.path(), &["render", "more.yaml", "-o", "more.svg", "--previous", "l.svg"], None);
    assert_eq!((m.code, m.json()["layout"]["kept"].as_u64()), (0, Some(5)), "{}", m.stdout);

    // a participant put between two others moves every column after it
    fs::write(dir.path().join("wide.yaml"), login.replace("  - {id: auth,", "  - {id: cdn, label: Edge cache, kind: service}\n  - {id: auth,").replace("nodes:", "max_width: 1200\nnodes:")).unwrap();
    let w = run_in(dir.path(), &["render", "wide.yaml", "-o", "wide.svg", "--previous", "l.svg"], None);
    assert_eq!((w.code, w.json()["error"]["code"].as_str().map(|s| s.to_string())), (4, Some("layout-changed".into())), "{}", w.stdout);
    assert!(!dir.path().join("wide.svg").exists());
    let forced = run_in(dir.path(), &["render", "wide.yaml", "-o", "wide.svg", "--previous", "l.svg", "--force", "--no-validate"], None);
    assert_eq!((forced.code, forced.json()["layout"]["relaid"].as_bool()), (0, Some(true)), "{}", forced.stdout);
    assert!(dir.path().join("wide.svg").exists());

    // pinned boxes are where the spec puts them: the earlier drawing has no say
    let rack = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rack.yaml");
    assert_eq!(run_in(dir.path(), &["render", rack.to_str().unwrap(), "-o", "rack.svg"], None).code, 0);
    assert_eq!(run_in(dir.path(), &["render", rack.to_str().unwrap(), "-o", "rack2.svg", "--previous", "rack.svg"], None).code, 0);
}

const LIVING: &str = "# Page\n\n```svgdog path\nversion: 1\ndiagram: flow\ntitle: Request path\ndirection: down\nedges:\n  - \"browser -> api: request\"\n  - \"api -> db: query\"\n```\n";

#[test]
fn docs_keeps_the_boxes_of_a_picture_that_is_already_there() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("page.md"), LIVING).unwrap();
    assert_eq!(run_in(dir.path(), &["docs", "page.md"], None).code, 0);
    fs::copy(dir.path().join("diagrams/page-path.svg"), dir.path().join("first.svg")).unwrap();

    // a new box and edge: the three that were there do not move
    let page = fs::read_to_string(dir.path().join("page.md")).unwrap();
    fs::write(dir.path().join("page.md"), page.replace("  - \"api -> db: query\"\n", "  - \"api -> db: query\"\n  - \"api -> cache: read\"\n")).unwrap();
    let stale = run_in(dir.path(), &["docs", "page.md", "--check"], None);
    assert_eq!(stale.code, 1, "{}", stale.stdout);
    assert_eq!(run_in(dir.path(), &["docs", "page.md"], None).code, 0);
    for (a, b) in [("browser", "api"), ("api", "db")] {
        assert_eq!(apart(dir.path(), "first.svg", a, b), apart(dir.path(), "diagrams/page-path.svg", a, b));
    }
    box_in(dir.path(), "diagrams/page-path.svg", "cache");
    let ok = run_in(dir.path(), &["docs", "page.md", "--check"], None);
    assert_eq!((ok.code, ok.json()["stale"].as_array().unwrap().len()), (0, 0), "{}", ok.stdout);

    // a box that outgrows its place stops the run and names the page; --force lets the layout go
    let page = fs::read_to_string(dir.path().join("page.md")).unwrap();
    fs::write(dir.path().join("page.md"), page.replace("edges:\n", "nodes:\n  - { id: api, label: \"API\\nwith\\nmany\\nmore\\nlines\\nof text\" }\nedges:\n")).unwrap();
    let before = fs::read(dir.path().join("diagrams/page-path.svg")).unwrap();
    let no = run_in(dir.path(), &["docs", "page.md"], None);
    let v = no.json();
    assert_eq!((no.code, v["error"]["code"].as_str()), (4, Some("layout-changed")), "{v:#}");
    assert!(v["error"]["message"].as_str().unwrap().starts_with("page.md, diagram starting on line 4"), "{v:#}");
    assert_eq!(fs::read(dir.path().join("diagrams/page-path.svg")).unwrap(), before);
    let yes = run_in(dir.path(), &["docs", "page.md", "--force"], None);
    let v = yes.json();
    assert_eq!((yes.code, v["relaid"][0]["path"].as_str()), (0, Some("diagrams/page-path.svg")), "{v:#}");
    assert_valid(&schema("findings"), &v);
    assert_eq!(run_in(dir.path(), &["docs", "page.md", "--check"], None).code, 0);
}

#[test]
fn every_computed_layout_keeps_its_boxes_when_boxes_are_added() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = |n: &str| fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(n)).unwrap();
    let cases: [(&str, String, &[&str], usize); 3] = [
        (
            "tiers",
            fixture("tiers.yaml").replace("groups:", "  - {id: auth, label: Auth service, kind: service}\n  - {id: search, label: Search index, kind: datastore}\ngroups:").replace("members: [api, jobs]", "members: [api, jobs, auth]").replace("members: [pg, redis, s3]", "members: [pg, redis, s3, search]")
                + "  - {from: api, to: auth, label: verify}\n  - {from: jobs, to: search, label: index}\n",
            &["cdn", "spa", "api", "jobs", "pg", "redis", "s3"],
            2,
        ),
        ("states", fixture("states.yaml").replace("  - { id: done, kind: end }", "  - { id: refunded, label: Refunded }\n  - { id: done, kind: end }") + "  - \"shipped -> refunded: returned\"\n  - \"refunded -> done\"\n", &["draft", "placed", "paid", "shipped", "cancelled"], 1),
        (
            "checkout",
            fixture("checkout.yaml").replace("groups:", "  - {id: ledger, label: Ledger, kind: datastore}\ngroups:").replace("members: [odb, bus]", "members: [odb, bus, ledger]").replace("notes:", "  - {from: pay, to: ledger, label: record}\nnotes:"),
            &["user", "web", "gw", "auth", "orders", "pay", "stripe", "odb", "bus", "mail", "doc"],
            1,
        ),
    ];
    for (name, changed, stay, added) in cases {
        let (spec, old, new) = (format!("{name}.yaml"), format!("{name}.svg"), format!("{name}.new.svg"));
        fs::write(dir.path().join(&spec), fixture(&spec)).unwrap();
        assert_eq!(run_in(dir.path(), &["render", &spec, "-o", &old], None).code, 0);
        fs::write(dir.path().join(&spec), changed).unwrap();
        let r = run_in(dir.path(), &["render", &spec, "-o", &new, "--previous", &old], None);
        let v = r.json();
        assert_eq!((r.code, v["summary"]["errors"].as_u64(), v["layout"]["placed"].as_array().map(|a| a.len())), (0, Some(0), Some(added)), "{name}: {v:#}");
        for pair in stay.windows(2) {
            assert_eq!(apart(dir.path(), &old, pair[0], pair[1]), apart(dir.path(), &new, pair[0], pair[1]), "{name}: {} and {} stay", pair[0], pair[1]);
        }
        // once more over the result: the same bytes
        let again = format!("{name}.again.svg");
        assert_eq!(run_in(dir.path(), &["render", &spec, "-o", &again, "--previous", &new], None).code, 0);
        assert_eq!(fs::read(dir.path().join(&new)).unwrap(), fs::read(dir.path().join(&again)).unwrap(), "{name}");
    }
}

const THEMES: [&str; 4] = ["docs", "blueprint", "paper", "bold"];

#[test]
fn every_built_in_theme_draws_the_fixtures_in_each_variant_without_findings() {
    for theme in THEMES {
        for variant in ["", "-light", "-dark", "-auto"] {
            let name = format!("{theme}{variant}");
            for fixture in ["orders", "tiers", "rack", "login", "walkthrough", "migration", "split"] {
                let (v, svg) = render_fixture(&format!("tests/fixtures/{fixture}.yaml"), &["--theme", &name]);
                assert_eq!((v["summary"]["errors"].as_u64(), v["summary"]["warnings"].as_u64()), (Some(0), Some(0)), "{fixture} {name}: {v:#}");
                assert_eq!(svg.contains("prefers-color-scheme"), variant == "-auto", "{fixture} {name}");
            }
        }
    }
    // a bare name is the light variant
    let (_, plain) = render_fixture(ORDERS, &["--theme", "paper"]);
    let (_, light) = render_fixture(ORDERS, &["--theme", "paper-light"]);
    assert_eq!(plain, light);
}

#[test]
fn a_theme_sets_type_lines_and_corners_as_well_as_colours() {
    let (_, docs) = render_fixture(ORDERS, &[]);
    assert!(docs.contains("font-family=\"Inter, sans-serif\"") && docs.contains("font-size=\"14\"") && docs.contains("stroke-width=\"1.5\""));
    let (_, blue) = render_fixture(ORDERS, &["--theme", "blueprint"]);
    assert!(blue.contains("font-family=\"JetBrains Mono, monospace\"") && blue.contains("font-size=\"13\""), "{blue}");
    assert!(!blue.contains(" rx=\"") && !blue.contains("stroke-width=\"1.5\""), "square corners and fine lines: {blue}");
    let (_, bold) = render_fixture(ORDERS, &["--theme", "bold"]);
    assert!(bold.contains("font-size=\"16\"") && bold.contains("font-size=\"13\"") && bold.contains("stroke-width=\"2.5\""), "{bold}");
    // larger type needs larger boxes
    let width = |svg: &str| svg.split("viewBox=\"0 0 ").nth(1).unwrap().split(' ').next().unwrap().parse::<f64>().unwrap();
    assert!(width(&bold) > width(&docs), "{} vs {}", width(&bold), width(&docs));
    // outlined text follows the theme's face too
    let (v, paths) = render_fixture(ORDERS, &["--theme", "blueprint", "--text-as-paths"]);
    assert!(!paths.contains("<text") && v["summary"]["errors"] == 0, "{v:#}");
}

#[test]
fn a_theme_of_your_own_comes_from_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let at = |p: &str| dir.path().join(p);
    fs::create_dir_all(at("fig")).unwrap();
    // the fixture names a theme of its own; here the spec leaves the choice open
    let plain = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS)).unwrap().replace("theme: docs-light\n", "");
    fs::write(at("fig/orders.yaml"), &plain).unwrap();

    // start one from a built-in theme; a second init does not overwrite it
    let r = run_in(dir.path(), &["theme", "init", "themes/acme", "--from", "paper"], None);
    assert_eq!((r.code, r.json()["name"].as_str().map(String::from)), (0, Some("acme".into())), "{}", r.stdout);
    assert_eq!(run_in(dir.path(), &["theme", "init", "themes/acme"], None).code, 4);
    let file = at("themes/acme/theme.toml");
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("name = \"acme\"") && text.contains("[dark]"), "{text}");
    let c = run_in(dir.path(), &["theme", "check", "themes/acme", "--no-config"], None);
    let v = c.json();
    assert_eq!((c.code, v["ok"].as_bool(), v["contrast"].as_array().unwrap().len()), (0, Some(true), 0), "{v:#}");
    assert_eq!(v["summary"]["files"], 27, "nine examples in three variants: {v:#}");

    // change it, and the drawing follows
    fs::write(&file, text.replace("background = \"#fbf7ef\"", "background = \"#f4f1ea\"").replace("corner = 12", "corner = 3")).unwrap();
    let r = run_in(dir.path(), &["render", "fig/orders.yaml", "-o", "flag.svg", "--theme", "./themes/acme", "--no-config"], None);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let svg = fs::read_to_string(at("flag.svg")).unwrap();
    assert!(svg.contains("fill=\"#f4f1ea\"") && svg.contains(" rx=\"3\""), "{svg}");
    // the variant goes after '#'
    for (variant, mark) in [("#dark", "fill=\"#26221e\""), ("#auto", "prefers-color-scheme"), ("#light", "fill=\"#f4f1ea\"")] {
        let r = run_in(dir.path(), &["render", "fig/orders.yaml", "-o", "v.svg", "--force", "--theme", &format!("./themes/acme{variant}"), "--no-config"], None);
        assert!(r.code == 0 && fs::read_to_string(at("v.svg")).unwrap().contains(mark), "{variant}: {}", r.stdout);
    }

    // named in the spec, the folder is found from where the spec is
    fs::write(at("fig/themed.yaml"), plain.replacen("version: 1", "version: 1\ntheme: ../themes/acme", 1)).unwrap();
    let r = run_in(dir.path(), &["render", "fig/themed.yaml", "-o", "spec.svg", "--no-config"], None);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert_eq!(fs::read_to_string(at("spec.svg")).unwrap(), svg);
    // the flag still wins over the spec
    run_in(dir.path(), &["render", "fig/themed.yaml", "-o", "won.svg", "--theme", "docs-dark", "--no-config"], None);
    assert!(fs::read_to_string(at("won.svg")).unwrap().contains("fill=\"#0d1117\""));

    // named in svgdog.toml, it is the project default, found from where the config is
    fs::write(at("svgdog.toml"), "version = 1\n[style]\ntheme = \"themes/acme\"\n").unwrap();
    let r = run_in(&at("fig"), &["render", "orders.yaml", "-o", "cfg.svg"], None);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert_eq!(fs::read_to_string(at("fig/cfg.svg")).unwrap(), svg);
    // a picture is out of date once its theme changes
    assert_eq!(run_in(&at("fig"), &["render", "orders.yaml", "-o", "cfg.svg", "--check"], None).code, 0);
    fs::write(&file, fs::read_to_string(&file).unwrap().replace("width = 1.5", "width = 2")).unwrap();
    let k = run_in(&at("fig"), &["render", "orders.yaml", "-o", "cfg.svg", "--check"], None);
    assert_eq!((k.code, k.json()["stale"][0]["reason"].as_str().map(String::from)), (1, Some("differs".into())), "{}", k.stdout);

    // pages take a theme folder too, relative to the page
    fs::create_dir_all(at("docs")).unwrap();
    fs::write(at("docs/page.md"), "# P\n\n```svgdog\nversion: 1\ndiagram: flow\ntitle: T\ntheme: ../themes/acme#dark\nedges: [\"a -> b\"]\n```\n").unwrap();
    let r = run_in(dir.path(), &["docs", "docs", "--no-config"], None);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(fs::read_to_string(at("docs/diagrams/page-1.svg")).unwrap().contains("fill=\"#26221e\""));
}

#[test]
fn theme_mistakes_say_what_is_wrong_and_where() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS);
    let colours = "background = \"#ffffff\"\ntext = \"#111111\"\nmuted = \"#555555\"\nnode_fill = \"#eeeeee\"\nnode_stroke = \"#111111\"\naccent_fill = \"#111111\"\naccent_text = \"#ffffff\"\nnote_fill = \"#ffffff\"\nedge = \"#111111\"\n";
    let good = format!("name = \"lite\"\n[light]\n{colours}");
    let draw = |name: &str, text: Option<&str>, theme: &str| {
        if let Some(t) = text {
            fs::create_dir_all(dir.path().join(name)).unwrap();
            fs::write(dir.path().join(name).join("theme.toml"), t).unwrap();
        }
        let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "x.svg", "--force", "--theme", theme, "--no-config"], None);
        (r.code, r.json())
    };
    // a theme with light colours only draws, and says so when asked for the dark ones
    assert_eq!(draw("lite", Some(&good), "./lite").0, 0);
    for variant in ["#dark", "#auto"] {
        let (code, v) = draw("lite", None, &format!("./lite{variant}"));
        assert_eq!((code, v["error"]["code"].as_str()), (3, Some("theme-no-dark")), "{v:#}");
    }
    let (code, v) = draw("lite", None, "./lite#night");
    assert_eq!((code, v["error"]["code"].as_str()), (2, Some("unknown-theme")), "{v:#}");

    let cases: [(&str, String, &str, &str); 7] = [
        ("typo", good.replace("node_fill", "node_fil"), "node_fil", "Did you mean 'node_fill'?"),
        ("colour", good.replace("#eeeeee", "light grey"), "light.node_fill", ""),
        ("clear", good.replace("#eeeeee", "#eeeeee80"), "light.node_fill", ""),
        ("big", format!("name = \"lite\"\n[font]\nsize = 40\n[light]\n{colours}"), "font.size", ""),
        ("upside", format!("name = \"lite\"\n[font]\nsize = 12\nsmall = 14\n[light]\n{colours}"), "font.small", ""),
        ("named", good.replace("\"lite\"", "\"My Theme\""), "name", ""),
        ("version", format!("version = 2\n{good}"), "version", ""),
    ];
    for (name, text, path, hint) in cases {
        let (code, v) = draw(name, Some(&text), &format!("./{name}"));
        assert_eq!((code, v["error"]["code"].as_str(), v["error"]["path"].as_str()), (3, Some("invalid-theme"), Some(path)), "{name}: {v:#}");
        assert!(v["error"]["message"].as_str().unwrap().starts_with(&format!("./{name}/theme.toml: ")), "{v:#}");
        if !hint.is_empty() {
            assert_eq!(v["error"]["suggestion"], hint, "{v:#}");
        }
    }
    let (code, v) = draw("missing-colour", Some(&good.replace("edge = \"#111111\"\n", "")), "./missing-colour");
    assert_eq!((code, v["error"]["code"].as_str(), v["error"]["path"].as_str()), (3, Some("invalid-theme"), Some("edge")), "{v:#}");
    let (code, v) = draw("nowhere", None, "./nowhere");
    assert_eq!((code, v["error"]["code"].as_str()), (3, Some("theme-not-found")), "{v:#}");
    fs::create_dir_all(dir.path().join("empty")).unwrap();
    let (code, v) = draw("empty", None, "./empty");
    assert_eq!((code, v["error"]["code"].as_str()), (3, Some("theme-not-found")), "{v:#}");

    // unknown names: a usage error on the flag, an input error in the spec and in the config
    let (code, v) = draw("x", None, "papr");
    assert_eq!((code, v["error"]["suggestion"].as_str()), (2, Some("Did you mean 'paper'?")), "{v:#}");
    fs::write(dir.path().join("svgdog.toml"), "[style]\ntheme = \"neon\"\n").unwrap();
    fs::write(dir.path().join("plain.yaml"), "version: 1\ndiagram: flow\ntitle: T\nedges: [\"a -> b\"]\n").unwrap();
    let r = run_in(dir.path(), &["render", "plain.yaml", "-o", "x.svg", "--force"], None);
    let v = r.json();
    assert!(r.code == 3 && v["error"]["code"] == "unknown-theme" && v["error"]["message"].as_str().unwrap().contains("svgdog.toml"), "{v:#}");

    // check reports colours that are too close to read, and fails
    let faint = good.replace("text = \"#111111\"", "text = \"#dddddd\"");
    fs::write(dir.path().join("lite/theme.toml"), faint).unwrap();
    let c = run_in(dir.path(), &["theme", "check", "lite", "--no-config"], None);
    let v = c.json();
    let low = v["contrast"].as_array().unwrap();
    assert!(c.code == 1 && v["ok"] == false && low.iter().any(|l| l["foreground"] == "text" && l["background"] == "node_fill" && l["variant"] == "light"), "{v:#}");
    assert!(v["summary"]["errors"].as_u64().unwrap() > 0, "the drawings show it too: {v:#}");
    assert_eq!(run_in(dir.path(), &["theme", "init", "t", "--from", "neon"], None).code, 2);
}

#[test]
fn theme_list_and_schema_describe_the_built_in_themes() {
    let r = run(&["theme", "list"]);
    let v = r.json();
    let names: Vec<&str> = v["themes"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!((r.code, names), (0, THEMES.to_vec()), "{v:#}");
    for t in v["themes"].as_array().unwrap() {
        assert!(t["variants"] == serde_json::json!(["light", "dark", "auto"]) && !t["description"].as_str().unwrap().is_empty() && t["light"]["background"].is_string(), "{t:#}");
    }
    let text = run(&["--format", "text", "theme", "list"]).stdout;
    assert!(THEMES.iter().all(|n| text.contains(n)) && text.contains("svgdog theme init"), "{text}");
    // the bundled theme files are valid by the published schema
    let schema: Value = serde_json::from_str(&run(&["schema", "theme"]).stdout).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for name in THEMES {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("themes/{name}/theme.toml"));
        let theme: Value = toml::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let errors: Vec<String> = validator.iter_errors(&theme).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        assert_eq!(theme["name"], name);
    }
}

#[test]
fn a_change_of_theme_keeps_the_boxes_where_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join(ORDERS);
    assert_eq!(run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "o.svg", "--no-config"], None).code, 0);
    // colours and corners alone leave every box where it was
    let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "o.svg", "--previous", "o.svg", "--theme", "paper", "--no-config"], None);
    let v = r.json();
    assert!(r.code == 0 && v["layout"]["relaid"] == false && v["layout"]["reasons"].as_array().unwrap().is_empty(), "{v:#}");
    let centres = |svg: &str| -> Vec<(f64, f64)> {
        let num = |tag: &str, key: &str| tag.split(&format!(" {key}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
        ["node-web", "node-api"].iter().map(|id| {
            let tag = svg.split(&format!("<g id=\"{id}\"")).nth(1).unwrap().split("<rect").nth(1).unwrap().split("/>").next().unwrap();
            (num(tag, "x") + num(tag, "width") / 2.0, num(tag, "y") + num(tag, "height") / 2.0)
        }).collect()
    };
    let before = centres(&fs::read_to_string(dir.path().join("o.svg")).unwrap());
    // larger type makes larger boxes; each grows where it stands, as a longer label would make it
    let r = run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "o.svg", "--previous", "o.svg", "--theme", "bold", "--no-config"], None);
    let v = r.json();
    assert!(r.code == 0 && v["layout"]["kept"] == 4 && v["layout"]["relaid"] == false && v["summary"]["errors"] == 0, "{v:#}");
    let after = fs::read_to_string(dir.path().join("o.svg")).unwrap();
    assert!(after.contains("font-size=\"16\""), "{after}");
    // the canvas may grow round them; what stays is where the boxes are against each other
    let apart = |c: &[(f64, f64)]| (c[1].0 - c[0].0, c[1].1 - c[0].1);
    assert_eq!(apart(&centres(&after)), apart(&before));
    // and the picture is then current for that theme
    assert_eq!(run_in(dir.path(), &["render", spec.to_str().unwrap(), "-o", "o.svg", "--theme", "bold", "--check", "--no-config"], None).code, 0);
}

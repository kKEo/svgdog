//! End-to-end tests of the binary, organised by the spec's agent requirements (R1 to R16)
//! where phase 1 covers them.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_svgdoc");

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
            fs::write(dir.path().join("svgdoc.toml"), cfg).unwrap();
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
    fs::write(dir.path().join("svgdoc.toml"), "version = 1\n").unwrap();
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
    let planned = run(&["render", "spec.yaml", "-o", "x.svg"]);
    assert_eq!(planned.code, 2);
    assert!(planned.json()["error"]["message"].as_str().unwrap().contains("not built yet"));

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
    for args in [vec!["--help"], vec!["validate", "--help"], vec!["inspect", "--help"], vec!["edit", "--help"], vec!["fix", "--help"], vec!["schema", "--help"], vec!["rules", "--help"]] {
        let r = run(&args);
        assert_eq!(r.code, 0);
        let lines = r.stdout.lines().count();
        assert!(lines <= 40, "{args:?} help is {lines} lines");
        assert!(r.stdout.contains("Example:"), "{args:?} help has no example");
    }
    assert!(run(&["--version"]).stdout.starts_with("svgdoc "));
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
        fs::write(dir.path().join("svgdoc.toml"), toml).unwrap();
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
    fs::write(dir.path().join("svgdoc.toml"), toml).unwrap();
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
    fs::write(dir.path().join("ok.svg"), svg("<!-- svgdoc-disable shape-overlap: badge sits on the corner by design -->")).unwrap();
    fs::write(dir.path().join("bare.svg"), svg("<!-- svgdoc-disable shape-overlap -->")).unwrap();
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
    let start = std::time::Instant::now();
    let r = run_in(dir.path(), &["validate", "big.svg"], None);
    let took = start.elapsed();
    assert!(r.code <= 1, "{}", r.stderr);
    r.json();
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
    ("content-clipped", true), ("excess-margin", true), ("edge-detached", true), ("label-on-line", false),
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
            fs::write(dir.path().join("svgdoc.toml"), cfg).unwrap();
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
    assert_eq!(code(&["edit", "spec.yaml", "--set", "#a.x=1", "--dry-run"], None), (2, "not-implemented".into()));
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

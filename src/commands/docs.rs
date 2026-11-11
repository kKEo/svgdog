//! `svgdog docs`: diagrams written inside Markdown pages. Each `svgdog` block is
//! rendered to an SVG file next to the page, and an image line after the block is
//! kept pointing at it.

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use super::validate::{Paint, report};
use crate::cli::{FailOn, Live};
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, Output};
use crate::{describe, render, rules, spec};

/// A diagram found in a page.
struct Block {
    /// Line the spec starts on, from 1, for error messages.
    line: usize,
    name: Option<String>,
    spec: String,
    /// Index of the line that closes the block.
    end: usize,
}

fn name_of(rest: &str) -> Option<String> {
    let word = rest.split_whitespace().next()?;
    let ok = word.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| word.to_lowercase())
}

/// Find the diagram blocks of a page: fenced code with the info word `svgdog`, and
/// comments that open with `<!-- svgdog`, for a source the reader does not see.
fn blocks(lines: &[&str]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        let fence = ["```", "~~~"].into_iter().find(|f| t.starts_with(f));
        if let Some(mark) = fence {
            let ticks: String = t.chars().take_while(|c| *c == mark.chars().next().unwrap()).collect();
            let info = t[ticks.len()..].trim();
            let close = (i + 1..lines.len()).find(|k| lines[*k].trim_start().starts_with(&ticks) && lines[*k].trim() == ticks);
            let Some(end) = close else { break };
            if info.split_whitespace().next() == Some("svgdog") {
                out.push(Block { line: i + 2, name: name_of(info.trim_start_matches("svgdog")), spec: lines[i + 1..end].join("\n") + "\n", end });
            }
            i = end + 1;
            continue;
        }
        if let Some(rest) = t.strip_prefix("<!-- svgdog")
            && (rest.is_empty() || rest.starts_with(char::is_whitespace))
            && let Some(end) = (i + 1..lines.len()).find(|k| lines[*k].trim() == "-->")
        {
            out.push(Block { line: i + 2, name: name_of(rest), spec: lines[i + 1..end].join("\n") + "\n", end });
            i = end + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// Every Markdown page under the given files and directories, in a stable order.
fn pages(args: &[String]) -> Result<Vec<PathBuf>, AppError> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() {
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    walk(&p, out);
                }
            } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md")) {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for a in args {
        let p = Path::new(a);
        if p.is_dir() {
            walk(p, &mut out);
        } else if p.is_file() {
            out.push(p.to_path_buf());
        } else {
            return Err(AppError::input("file-not-found", format!("File not found: {a}.")).path(a.clone()));
        }
    }
    out.dedup();
    Ok(out)
}

fn shown(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[allow(clippy::too_many_arguments)]
pub fn run(paths: &[String], check: bool, assets: &str, theme: Option<&str>, live: Live, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let dir_ok = !assets.is_empty() && Path::new(assets).components().all(|c| matches!(c, Component::Normal(_)));
    if !dir_ok {
        return Err(AppError::usage("invalid-argument", format!("--assets '{assets}' must be a folder name or relative path below the page, without '..'.")));
    }
    let assets = assets.trim_end_matches('/').replace('\\', "/");
    let mut findings = Vec::new();
    let mut outputs: Vec<Value> = Vec::new();
    let mut stale: Vec<Value> = Vec::new();
    let mut documents: Vec<Value> = Vec::new();
    let mut diagrams = 0;

    for page in pages(paths)? {
        let label = shown(&page);
        let text = std::fs::read_to_string(&page).map_err(|e| AppError::input("file-not-readable", format!("Cannot read {label}: {e}.")).path(label.clone()))?;
        let lines: Vec<&str> = text.lines().collect();
        let found = blocks(&lines);
        if found.is_empty() {
            continue;
        }
        let cfg = loader.for_input(Some(&page))?;
        let settings = cfg.settings(Some(&page), profile, &RuleSel::default());
        let stem = page.file_stem().and_then(|s| s.to_str()).unwrap_or("page").to_lowercase();
        let folder = page.parent().unwrap_or(Path::new(""));

        // Build the page again, line by line, with each block's image line in place.
        let mut rebuilt: Vec<String> = Vec::new();
        let mut cursor = 0;
        let mut used: Vec<String> = Vec::new();
        for (n, b) in found.iter().enumerate() {
            let at = |e: AppError| {
                let mut e = e;
                e.message = format!("{label}, diagram starting on line {}: {}", b.line, e.message);
                e
            };
            let doc = spec::read_document(&b.spec, &label).map_err(at)?;
            let parsed = spec::parse(&doc).map_err(at)?;
            let file = format!("{stem}-{}.svg", b.name.clone().unwrap_or_else(|| (n + 1).to_string()));
            if used.contains(&file) {
                return Err(AppError::input("duplicate-id", format!("{label}, diagram starting on line {}: the name gives {file}, which another diagram on this page already uses.", b.line)).path(label.clone()));
            }
            used.push(file.clone());
            let rel = format!("{assets}/{file}");
            let target = folder.join(&rel);
            // The picture in the page is an SVG either way; the html version is a second file beside it.
            let svg = render::render(&parsed, &render::Options { theme, fonts: cfg.fonts(), text_as_paths: false, embed: None, interactive: live == Live::Css }).map_err(at)?;
            findings.extend(rules::analyze(&shown(&target), &svg, &settings));
            diagrams += 1;
            let mut files = vec![(target.clone(), svg)];
            if live == Live::Html {
                let inner = render::render(&parsed, &render::Options { theme, fonts: cfg.fonts(), text_as_paths: false, embed: None, interactive: true }).map_err(at)?;
                let name = theme.or(parsed.theme.as_deref()).unwrap_or("docs-light");
                let (light, dark) = if name == render::theme::AUTO { (&render::theme::THEMES[0], Some(&render::theme::THEMES[1])) } else { (render::theme::find(name).unwrap_or(&render::theme::THEMES[0]), None) };
                files.push((target.with_extension("html"), render::interactive::page(&inner, &parsed, light, dark)));
            }
            for (path, content) in &files {
                let current = std::fs::read(path).ok();
                if current.as_deref() == Some(content.as_bytes()) {
                    continue;
                }
                if check {
                    stale.push(json!({ "path": shown(path), "reason": if current.is_some() { "differs" } else { "missing" } }));
                } else {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| AppError::new(crate::envelope::EXIT_OUTPUT, "write-failed", format!("Cannot create {}: {e}.", shown(parent))).path(shown(parent)))?;
                    }
                    super::change::write(path, content)?;
                    outputs.push(json!({ "path": shown(path), "bytes": content.len() }));
                }
            }
            // The words a reader gets in place of the picture.
            let words = parsed.description.clone().unwrap_or_else(|| describe::short(&parsed));
            let alt = format!("{}. {words}", parsed.title.trim_end_matches('.')).replace(['[', ']'], "").replace('\n', " ");
            let more = if live == Live::Html { format!(" [Interactive version]({})", rel.replace(".svg", ".html")) } else { String::new() };
            let image = format!("![{alt}]({rel}){more}");

            rebuilt.extend(lines[cursor..=b.end].iter().map(|l| l.to_string()));
            cursor = b.end + 1;
            // An image line this command wrote earlier is replaced; anything else is left alone.
            let next = (cursor..lines.len()).find(|k| !lines[*k].trim().is_empty());
            let ours = next.filter(|k| lines[*k].trim_start().starts_with("![") && lines[*k].contains(&format!("]({assets}/")));
            rebuilt.push(String::new());
            rebuilt.push(image);
            match ours {
                Some(k) => cursor = k + 1,
                None => {
                    if cursor < lines.len() && !lines[cursor].trim().is_empty() {
                        rebuilt.push(String::new());
                    }
                }
            }
        }
        rebuilt.extend(lines[cursor..].iter().map(|l| l.to_string()));
        let mut after = rebuilt.join("\n");
        if text.ends_with('\n') || !after.ends_with('\n') {
            after.push('\n');
        }
        let changed = after != text;
        if changed {
            if check {
                stale.push(json!({ "path": label, "reason": "image links differ" }));
            } else {
                super::change::write(&page, &after)?;
                outputs.push(json!({ "path": label, "bytes": after.len() }));
            }
        }
        documents.push(json!({ "path": label, "diagrams": found.len() }));
    }

    let mut r = report("docs", findings, diagrams, 50, FailOn::Error, paint);
    let pages_n = documents.len();
    r.json.insert("outputs".into(), Value::Array(outputs.clone()));
    r.json.insert("documents".into(), Value::Array(documents));
    let mut text = format!("{diagrams} diagram{} in {pages_n} page{}.\n", if diagrams == 1 { "" } else { "s" }, if pages_n == 1 { "" } else { "s" });
    if check {
        for s in &stale {
            text.push_str(&format!("{}: {}.\n", s["path"].as_str().unwrap_or(""), s["reason"].as_str().unwrap_or("")));
        }
        text.push_str(if stale.is_empty() { "Everything is up to date.\n" } else { "Run 'svgdog docs' without --check to update.\n" });
        if !stale.is_empty() {
            r.json.insert("ok".into(), json!(false));
        }
        r.json.insert("stale".into(), Value::Array(stale.clone()));
    } else {
        for o in &outputs {
            text.push_str(&format!("Wrote {}.\n", o["path"].as_str().unwrap_or("")));
        }
        if outputs.is_empty() {
            text.push_str("Nothing to update.\n");
        }
    }
    text.push('\n');
    text.push_str(&r.text);
    let exit = if r.failed || !stale.is_empty() { EXIT_FINDINGS } else { EXIT_OK };
    Ok(Output { json: Value::Object(r.json), text, exit, raw: None })
}

//! `svgdog theme`: list the built-in themes, start one of your own, and check it.

use std::path::Path;

use serde_json::{Value, json};

use super::validate::{Paint, report};
use crate::cli::FailOn;
use crate::config::{Loader, Profile, RuleSel};
use crate::envelope::{AppError, EXIT_FINDINGS, EXIT_OK, EXIT_OUTPUT, Output, head};
use crate::render::theme::{self, Family, ROLES, Resolved, Theme, Variant};
use crate::util::{Rgba, contrast, did_you_mean, fmt};
use crate::{render, rules, spec};

fn colours(t: &Theme) -> Value {
    let mut m = serde_json::Map::new();
    for (role, token) in ROLES {
        m.insert(role.into(), json!(t.role(token).unwrap_or("")));
    }
    Value::Object(m)
}

fn family_json(f: &Family) -> Value {
    let t = &f.light;
    json!({
        "name": f.name,
        "description": f.description,
        "variants": if f.dark.is_some() { vec!["light", "dark", "auto"] } else { vec!["light"] },
        "font": { "family": t.family, "size": t.size, "small": t.small },
        "line": { "width": t.width, "thin": t.thin, "corner": t.corner, "box_corner": t.box_corner },
        "light": colours(t),
        "dark": f.dark.as_ref().map(colours),
    })
}

pub fn list() -> Result<Output, AppError> {
    let all: Vec<Family> = theme::BUILTIN.iter().filter_map(|(n, _)| theme::builtin(n)).collect();
    let mut m = head("theme", true);
    m.insert("themes".into(), Value::Array(all.iter().map(family_json).collect()));
    let w = all.iter().map(|f| f.name.len()).max().unwrap_or(0);
    let mut text: String = all.iter().map(|f| format!("{:<w$}  {}\n", f.name, f.description)).collect();
    text.push_str(&format!("\nUse one with --theme NAME, or 'theme: NAME' in a spec; add -dark or -auto for the dark or the adaptive variant.\nThe default is {}. Start your own with: svgdog theme init DIR --from NAME\n", theme::DEFAULT));
    Ok(Output { json: Value::Object(m), text, exit: EXIT_OK, raw: None })
}

/// A theme name from a folder name: what is left of it in lower-case letters, digits and hyphens.
fn name_from(dir: &Path) -> String {
    let base = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let mut name: String = base.to_lowercase().chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { '-' }).collect();
    name = name.trim_matches('-').to_string();
    if theme::valid_name(&name) { name } else { "custom".into() }
}

pub fn init(dir: &str, from: &str, force: bool) -> Result<Output, AppError> {
    let Some((_, source)) = theme::BUILTIN.iter().find(|(n, _)| *n == from) else {
        return Err(AppError::usage("unknown-theme", format!("Unknown theme '{from}' to start from.")).suggest(did_you_mean(from, theme::BUILTIN.iter().map(|(n, _)| *n)).or(Some(format!("Themes: {}.", theme::BUILTIN.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))))));
    };
    let folder = Path::new(dir);
    let file = folder.join(theme::FILE);
    let shown = file.display().to_string();
    if file.exists() && !force {
        return Err(AppError::new(EXIT_OUTPUT, "output-exists", format!("{shown} already exists; pass --force to replace it.")).path(shown));
    }
    let name = name_from(folder);
    // The copy keeps the comments of the theme it starts from; only what names it changes.
    let mut text = String::new();
    for (i, line) in source.lines().enumerate() {
        if i == 0 && line.starts_with('#') {
            text.push_str(&format!("# A theme for SVGDog, started from the built-in '{from}'. Check it with: svgdog theme check {dir}\n"));
        } else if line.starts_with("name = ") {
            text.push_str(&format!("name = \"{name}\"\n"));
        } else if line.starts_with("description = ") {
            text.push_str(&format!("description = \"Based on {from}\"\n"));
        } else {
            text.push_str(line);
            text.push('\n');
        }
    }
    theme::parse(&text, &shown)?;
    std::fs::create_dir_all(folder).map_err(|e| AppError::new(EXIT_OUTPUT, "write-failed", format!("Cannot create {dir}: {e}.")).path(dir))?;
    super::change::write(&file, &text)?;
    let mut m = head("theme", true);
    m.insert("outputs".into(), json!([{ "path": shown, "bytes": text.len() }]));
    m.insert("name".into(), json!(name));
    let how = if theme::is_path(dir) { dir.to_string() } else { format!("./{dir}") };
    let out = format!("Wrote {shown} ({} bytes), a copy of '{from}' named '{name}'.\nChange it, then check it with: svgdog theme check {dir}\nUse it with: --theme {how}\n", text.len());
    Ok(Output { json: Value::Object(m), text: out, exit: EXIT_OK, raw: None })
}

/// The pairs of colour roles that sit on each other, and the contrast each needs.
const PAIRS: [(&str, &str, f64, &str); 7] = [
    ("tx", "bg", 4.5, "text on the background"),
    ("tx", "nf", 4.5, "labels in boxes"),
    ("tx", "no", 4.5, "text in notes"),
    ("at", "af", 4.5, "labels in highlighted boxes"),
    ("mu", "bg", 4.5, "captions, frame labels and what is out of focus"),
    ("ed", "bg", 3.0, "arrows on the background"),
    ("ns", "bg", 3.0, "box outlines on the background"),
];

fn role_name(token: &str) -> &'static str {
    ROLES.iter().find(|(_, t)| *t == token).map(|(r, _)| *r).unwrap_or("")
}

pub fn check(dir: &str, loader: &mut Loader, profile: Option<Profile>, paint: &Paint) -> Result<Output, AppError> {
    let cwd = std::env::current_dir().unwrap_or_default();
    // Through `resolve`, so a folder outside the root is refused the same way as when drawing.
    let value = if theme::is_path(dir) { dir.to_string() } else { format!("./{dir}") };
    theme::resolve(&value, &cwd, loader.ceiling())?;
    let family = theme::load(&cwd.join(dir), dir)?;
    let cfg = loader.for_input(None)?;
    let settings = cfg.settings(None, profile, &RuleSel::default());

    let mut looks: Vec<(&str, Resolved)> = vec![("light", family.pick(Variant::Light, dir)?)];
    if family.dark.is_some() {
        looks.push(("dark", family.pick(Variant::Dark, dir)?));
        looks.push(("auto", family.pick(Variant::Auto, dir)?));
    }

    // Colours that sit on each other must be far enough apart to read.
    let mut low: Vec<Value> = Vec::new();
    let mut text = String::new();
    for (variant, look) in looks.iter().filter(|(v, _)| *v != "auto") {
        for (fg, bg, min, what) in PAIRS {
            let (a, b) = (look.theme.role(fg).unwrap_or(""), look.theme.role(bg).unwrap_or(""));
            let (Some(ca), Some(cb)) = (Rgba::parse(a), Rgba::parse(b)) else { continue };
            let ratio = contrast(&ca, &cb);
            if ratio < min {
                let (fg, bg) = (role_name(fg), role_name(bg));
                low.push(json!({ "variant": variant, "foreground": fg, "background": bg, "ratio": (ratio * 100.0).round() / 100.0, "minimum": min, "used_for": what }));
                text.push_str(&format!("[{variant}] {fg} {a} on {bg} {b} has contrast {}:1; the minimum for {what} is {}:1.\n", fmt((ratio * 100.0).round() / 100.0), fmt(min)));
            }
        }
    }

    // Every bundled example, drawn with each variant and checked like any drawing.
    let mut findings = Vec::new();
    let mut drawn = 0;
    let mut skipped: Vec<Value> = Vec::new();
    for (variant, look) in &looks {
        for (name, source) in super::examples::EXAMPLES {
            let parsed = spec::parse(&spec::read_document(source, name)?)?;
            let opt = render::Options { theme: look, text_as_paths: false, embed: None, interactive: false };
            match render::render(&parsed, &opt) {
                Ok(svg) => {
                    drawn += 1;
                    findings.extend(rules::analyze(&format!("example '{name}', {variant}"), &svg, &settings));
                }
                // A theme with large or wide type can make an example too wide; that is worth knowing, not a fault of the file.
                Err(e) => skipped.push(json!({ "example": name, "variant": variant, "code": e.code, "message": e.message })),
            }
        }
    }
    let mut r = report("theme", findings, drawn, 50, FailOn::Error, paint);
    let failed = r.failed || !low.is_empty();
    r.json.insert("ok".into(), json!(!failed));
    r.json.insert("theme".into(), family_json(&family));
    r.json.insert("contrast".into(), Value::Array(low));
    r.json.insert("skipped".into(), Value::Array(skipped.clone()));
    let mut out = format!("Theme '{}' in {dir}: {}.\n", family.name, if family.dark.is_some() { "light and dark colours" } else { "light colours only, so -dark and -auto are not available" });
    out.push_str(&text);
    out.push_str(&format!("Drew {drawn} example drawing{} with it.\n", if drawn == 1 { "" } else { "s" }));
    for s in &skipped {
        out.push_str(&format!("Not drawn: example '{}', {}: {}\n", s["example"].as_str().unwrap_or(""), s["variant"].as_str().unwrap_or(""), s["message"].as_str().unwrap_or("")));
    }
    out.push_str(&r.text);
    Ok(Output { json: Value::Object(r.json), text: out, exit: if failed { EXIT_FINDINGS } else { EXIT_OK }, raw: None })
}

//! Themes: the look of a drawing as data. The built-in ones are `theme.toml` files
//! compiled into the program; a folder with a `theme.toml` gives one of your own.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::Num;
use crate::envelope::{AppError, EXIT_INPUT};
use crate::svg::text::{self, Class};
use crate::util::{Rgba, did_you_mean, fmt};

/// The file a theme folder holds.
pub const FILE: &str = "theme.toml";
pub const DEFAULT: &str = "docs-light";

/// The built-in themes, by name, as the text of their files.
pub const BUILTIN: &[(&str, &str)] = &[
    ("docs", include_str!("../../themes/docs/theme.toml")),
    ("blueprint", include_str!("../../themes/blueprint/theme.toml")),
    ("paper", include_str!("../../themes/paper/theme.toml")),
    ("bold", include_str!("../../themes/bold/theme.toml")),
    ("cards", include_str!("../../themes/cards/theme.toml")),
];

/// The colour roles, in the order a theme file lists them, each with the two-letter
/// token that stands for it while an adaptive drawing is made.
pub const ROLES: [(&str, &str); 9] = [("background", "bg"), ("text", "tx"), ("muted", "mu"), ("node_fill", "nf"), ("node_stroke", "ns"), ("accent_fill", "af"), ("accent_text", "at"), ("note_fill", "no"), ("edge", "ed")];

/// One look: the colours of one variant, and the type and lines it shares with the other.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub background: String,
    pub text: String,
    pub muted: String,
    pub node_fill: String,
    pub node_stroke: String,
    pub accent_fill: String,
    pub accent_text: String,
    pub note_fill: String,
    pub edge: String,
    /// The font-family list written to the drawing.
    pub family: Vec<String>,
    /// The bundled face that text is measured with.
    pub class: Class,
    /// Text size of labels in boxes, and of everything smaller.
    pub size: f64,
    pub small: f64,
    /// Line width of boxes and arrows, and of frames, notes and guide lines.
    pub width: f64,
    pub thin: f64,
    /// Corner radius of the rounded kinds and of frames, and of the plain box.
    pub corner: f64,
    pub box_corner: f64,
}

impl Theme {
    /// Distance between the lines of a label in a box.
    pub fn lead(&self) -> f64 {
        self.size + 6.0
    }

    /// Distance between lines of small text.
    pub fn small_lead(&self) -> f64 {
        self.small + 4.0
    }

    pub fn role(&self, token: &str) -> Option<&str> {
        Some(match token {
            "bg" => &self.background,
            "tx" => &self.text,
            "mu" => &self.muted,
            "nf" => &self.node_fill,
            "ns" => &self.node_stroke,
            "af" => &self.accent_fill,
            "at" => &self.accent_text,
            "no" => &self.note_fill,
            "ed" => &self.edge,
            _ => return None,
        })
    }

    /// The same look with a token in place of each colour; `adapt` turns the tokens
    /// into the light colours plus classes that a dark-mode style rule overrides.
    pub fn placeholder(&self) -> Theme {
        let t = |k: &str| format!("@{k}");
        Theme { background: t("bg"), text: t("tx"), muted: t("mu"), node_fill: t("nf"), node_stroke: t("ns"), accent_fill: t("af"), accent_text: t("at"), note_fill: t("no"), edge: t("ed"), ..self.clone() }
    }

    /// Use the font-family list of the config in place of the theme's.
    pub fn with_fonts(mut self, fonts: &[String]) -> Theme {
        if !fonts.is_empty() {
            self.family = fonts.to_vec();
            self.class = text::classify(Some(&self.family.join(", "))).0;
        }
        self
    }
}

/// The theme a drawing is made with. `dark` is set for the adaptive variant only:
/// the drawing is then the light one, with a rule that swaps in the dark colours.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub theme: Theme,
    pub dark: Option<Theme>,
}

impl Resolved {
    pub fn with_fonts(self, fonts: &[String]) -> Resolved {
        Resolved { theme: self.theme.with_fonts(fonts), dark: self.dark.map(|d| d.with_fonts(fonts)) }
    }
}

/// A theme file, read and checked: both variants and what they share.
#[derive(Clone, Debug)]
pub struct Family {
    pub name: String,
    pub description: String,
    pub light: Theme,
    pub dark: Option<Theme>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Light,
    Dark,
    Auto,
}

impl Family {
    pub fn pick(&self, v: Variant, shown: &str) -> Result<Resolved, AppError> {
        let no_dark = || AppError::input("theme-no-dark", format!("Theme '{}' has no dark colours, so '{shown}' cannot be drawn.", self.name)).suggest(Some("Add a [dark] table to its theme.toml, or use the light variant.".into()));
        Ok(match v {
            Variant::Light => Resolved { theme: self.light.clone(), dark: None },
            Variant::Dark => Resolved { theme: self.dark.clone().ok_or_else(no_dark)?, dark: None },
            Variant::Auto => Resolved { theme: self.light.clone(), dark: Some(self.dark.clone().ok_or_else(no_dark)?) },
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    version: Option<i64>,
    name: String,
    description: Option<String>,
    #[serde(default)]
    font: RawFont,
    #[serde(default)]
    line: RawLine,
    light: RawColours,
    dark: Option<RawColours>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawFont {
    family: Option<Vec<String>>,
    size: Option<Num>,
    small: Option<Num>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLine {
    width: Option<Num>,
    thin: Option<Num>,
    corner: Option<Num>,
    box_corner: Option<Num>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColours {
    background: String,
    text: String,
    muted: String,
    node_fill: String,
    node_stroke: String,
    accent_fill: String,
    accent_text: String,
    note_fill: String,
    edge: String,
}

fn bad(shown: &str, key: &str, msg: String) -> AppError {
    AppError::input("invalid-theme", format!("{shown}: {msg}")).path(key.to_string())
}

/// A theme name: lower-case letters, digits and hyphens, not ending in a variant.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= 40
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.ends_with('-')
        && !["-light", "-dark", "-auto"].iter().any(|s| name.ends_with(s))
}

/// Read the text of a theme file. `shown` names the file in messages.
pub fn parse(text: &str, shown: &str) -> Result<Family, AppError> {
    let raw: Raw = toml::from_str(text).map_err(|e| {
        let msg = e.message().to_string();
        if let Some(rest) = msg.strip_prefix("unknown field `") {
            let name = rest.split('`').next().unwrap_or("");
            let known: Vec<&str> = rest.split('`').skip(2).step_by(2).collect();
            return AppError::input("invalid-theme", format!("{shown}: unknown key '{name}'.")).path(name.to_string()).suggest(did_you_mean(name, known.iter().copied()));
        }
        if let Some(rest) = msg.strip_prefix("missing field `") {
            let name = rest.split('`').next().unwrap_or("");
            return AppError::input("invalid-theme", format!("{shown}: '{name}' is missing.")).path(name.to_string()).suggest(Some("A theme needs a name and a [light] table with all nine colours; 'svgdog theme init' writes a complete file.".into()));
        }
        AppError::input("invalid-theme", format!("{shown}: {msg}"))
    })?;
    if let Some(v) = raw.version
        && v != 1
    {
        return Err(bad(shown, "version", format!("theme version {v} is not supported; this build reads version 1.")));
    }
    if !valid_name(&raw.name) {
        return Err(bad(shown, "name", format!("the name '{}' must be lower-case letters, digits and hyphens, start with a letter, and not end in -light, -dark or -auto.", raw.name)));
    }
    let within = |key: &str, v: Option<Num>, default: f64, lo: f64, hi: f64| -> Result<f64, AppError> {
        match v.map(Num::get) {
            None => Ok(default),
            Some(x) if x >= lo && x <= hi => Ok(x),
            Some(x) => Err(bad(shown, key, format!("{key} is {}; it must be between {} and {}.", fmt(x), fmt(lo), fmt(hi)))),
        }
    };
    let size = within("font.size", raw.font.size, 14.0, 10.0, 24.0)?;
    let small = within("font.small", raw.font.small, 12.0, 8.0, 20.0)?;
    if small > size {
        return Err(bad(shown, "font.small", format!("font.small ({}) must not be larger than font.size ({}).", fmt(small), fmt(size))));
    }
    let width = within("line.width", raw.line.width, 1.5, 0.5, 4.0)?;
    let thin = within("line.thin", raw.line.thin, 1.0, 0.5, 4.0)?;
    let corner = within("line.corner", raw.line.corner, 8.0, 0.0, 16.0)?;
    let box_corner = within("line.box_corner", raw.line.box_corner, 0.0, 0.0, 16.0)?;
    let family = raw.font.family.unwrap_or_else(|| vec!["Inter".into(), "sans-serif".into()]);
    if family.is_empty() || family.iter().any(|f| f.trim().is_empty() || f.contains(['"', '<', '>', '&', ','])) {
        return Err(bad(shown, "font.family", "font.family must be a list of font names, one per entry, without quotes or commas inside a name.".into()));
    }
    let class = text::classify(Some(&family.join(", "))).0;
    let variant = |table: &str, c: RawColours| -> Result<Theme, AppError> {
        let colour = |key: &str, v: String| -> Result<String, AppError> {
            let at = format!("{table}.{key}");
            match Rgba::parse(&v) {
                Some(c) if c.a >= 1.0 && v.starts_with('#') => Ok(v.to_lowercase()),
                Some(_) => Err(bad(shown, &at, format!("{at} is '{v}'; write an opaque colour as #rrggbb."))),
                None => Err(bad(shown, &at, format!("{at} is '{v}', which is not a colour; write it as #rrggbb."))),
            }
        };
        Ok(Theme {
            background: colour("background", c.background)?,
            text: colour("text", c.text)?,
            muted: colour("muted", c.muted)?,
            node_fill: colour("node_fill", c.node_fill)?,
            node_stroke: colour("node_stroke", c.node_stroke)?,
            accent_fill: colour("accent_fill", c.accent_fill)?,
            accent_text: colour("accent_text", c.accent_text)?,
            note_fill: colour("note_fill", c.note_fill)?,
            edge: colour("edge", c.edge)?,
            family: family.clone(),
            class,
            size,
            small,
            width,
            thin,
            corner,
            box_corner,
        })
    };
    let light = variant("light", raw.light)?;
    let dark = raw.dark.map(|d| variant("dark", d)).transpose()?;
    Ok(Family { name: raw.name, description: raw.description.unwrap_or_default(), light, dark })
}

/// A built-in theme by its family name.
pub fn builtin(name: &str) -> Option<Family> {
    BUILTIN.iter().find(|(n, _)| *n == name).map(|(n, text)| parse(text, &format!("built-in theme '{n}'")).expect("built-in themes are valid"))
}

/// Every name a built-in theme answers to.
pub fn names() -> Vec<String> {
    BUILTIN.iter().flat_map(|(n, _)| [n.to_string(), format!("{n}-light"), format!("{n}-dark"), format!("{n}-auto")]).collect()
}

/// A theme given as a folder rather than a name.
pub fn is_path(value: &str) -> bool {
    value.starts_with('.') || value.contains('/') || value.contains('\\')
}

/// Split a path value into the folder and the variant after `#`.
pub fn split_path(value: &str) -> (&str, Option<&str>) {
    match value.rsplit_once('#') {
        Some((p, v)) => (p, Some(v)),
        None => (value, None),
    }
}

/// Read the theme in a folder (or the theme file itself).
pub fn load(dir: &Path, shown: &str) -> Result<Family, AppError> {
    let file: PathBuf = if dir.is_dir() { dir.join(FILE) } else { dir.to_path_buf() };
    if !file.is_file() {
        let why = if dir.is_dir() { format!("{shown} has no {FILE}") } else { format!("{shown} is not a folder") };
        return Err(AppError::input("theme-not-found", format!("Cannot find the theme: {why}.")).path(shown.to_string()).suggest(Some(format!("A theme is a folder that holds a {FILE}; 'svgdog theme init DIR' writes one to start from."))));
    }
    let text = std::fs::read_to_string(&file).map_err(|e| AppError::input("theme-not-found", format!("Cannot read {}: {e}.", file.display())).path(shown.to_string()))?;
    parse(&text, &format!("{}/{FILE}", shown.trim_end_matches(['/', '\\'])))
}

/// Find the theme a value names: a built-in name with an optional `-light`, `-dark`
/// or `-auto`, or a folder with an optional `#dark` or `#auto`. Folders are relative
/// to `base` and, when `ceiling` is given, must be inside it.
pub fn resolve(value: &str, base: &Path, ceiling: Option<&Path>) -> Result<Resolved, AppError> {
    if is_path(value) {
        let (path, v) = split_path(value);
        let variant = match v {
            None | Some("light") => Variant::Light,
            Some("dark") => Variant::Dark,
            Some("auto") => Variant::Auto,
            Some(other) => return Err(AppError::input("unknown-theme", format!("'{value}': the part after '#' is '{other}'; it can be light, dark or auto."))),
        };
        let dir = base.join(path);
        if let Some(root) = ceiling {
            let real = dir.canonicalize().ok();
            let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
            if real.is_some_and(|r| !r.starts_with(&root)) {
                return Err(AppError::new(EXIT_INPUT, "outside-root", format!("The theme folder '{path}' is outside the root; only folders inside the root directory are read.")).path(path.to_string()));
            }
        }
        return load(&dir, path)?.pick(variant, value);
    }
    let (family, variant) = [("-light", Variant::Light), ("-dark", Variant::Dark), ("-auto", Variant::Auto)].into_iter().find_map(|(s, v)| value.strip_suffix(s).map(|f| (f, v))).unwrap_or((value, Variant::Light));
    let Some(found) = builtin(family) else {
        let all = names();
        let listed: Vec<&str> = BUILTIN.iter().map(|(n, _)| *n).collect();
        return Err(AppError::input("unknown-theme", format!("Unknown theme '{value}'.")).suggest(did_you_mean(value, all.iter().map(|s| s.as_str())).or(Some(format!("Themes: {} (add -dark or -auto), or the path of a theme folder such as ./my-theme.", listed.join(", "))))));
    };
    found.pick(variant, value)
}

/// Where a theme is named, in order of precedence: the command line, the spec, the
/// config. Each comes with the folder its relative paths start from.
pub struct Choice<'a> {
    pub flag: Option<&'a str>,
    pub spec: Option<&'a str>,
    pub spec_dir: &'a Path,
    pub config: Option<(&'a str, &'a Path)>,
    pub ceiling: Option<&'a Path>,
}

pub fn choose(c: &Choice) -> Result<Resolved, AppError> {
    if let Some(v) = c.flag {
        let cwd = std::env::current_dir().unwrap_or_default();
        return resolve(v, &cwd, c.ceiling).map_err(|mut e| {
            if e.code == "unknown-theme" {
                e.exit = crate::envelope::EXIT_USAGE;
            }
            e
        });
    }
    if let Some(v) = c.spec {
        return resolve(v, c.spec_dir, c.ceiling).map_err(|e| if e.code == "unknown-theme" || e.path.is_none() { e.path("theme") } else { e });
    }
    if let Some((v, dir)) = c.config {
        return resolve(v, dir, c.ceiling).map_err(|mut e| {
            e.message = format!("svgdog.toml, [style] theme: {}", e.message);
            e
        });
    }
    resolve(DEFAULT, Path::new(""), None)
}

/// The default theme, for drawings made without a choice of one.
#[cfg(test)]
pub fn default() -> Resolved {
    resolve(DEFAULT, Path::new(""), None).expect("the default theme is built in")
}

/// Finish a drawing made with the placeholder theme: every colour becomes the
/// light theme's, written as an ordinary attribute, and each element gets a class
/// per colour role. One style rule then swaps in the dark theme where the reader's
/// system asks for it. A renderer that ignores the rule shows the light drawing.
pub fn adapt(svg: &str, light: &Theme, dark: &Theme) -> String {
    let mut out = String::with_capacity(svg.len() + 512);
    let mut used: std::collections::BTreeSet<(char, &str)> = Default::default();
    let mut rest = svg;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('>').map_or(rest.len(), |c| open + c + 1);
        let tag = &rest[open..close];
        rest = &rest[close..];
        if !tag.contains("=\"@") {
            out.push_str(tag);
            continue;
        }
        let mut done = String::new();
        let mut classes: Vec<String> = Vec::new();
        let mut t = tag;
        while let Some(at) = t.find("=\"@") {
            // The attribute name ends right before the `=`: fill or stroke.
            let prop = if t[..at].ends_with("stroke") { 's' } else { 'f' };
            let token = &t[at + 3..at + 5];
            match light.role(token) {
                Some(colour) => {
                    done.push_str(&t[..at + 2]);
                    done.push_str(colour);
                    let key = ROLES.iter().map(|(_, k)| *k).find(|k| *k == token).unwrap_or("tx");
                    used.insert((prop, key));
                    classes.push(format!("{prop}-{key}"));
                }
                None => done.push_str(&t[..at + 5]),
            }
            t = &t[at + 5..];
        }
        // What is left ends the tag; the classes go in just before it closes.
        let tail = if t.ends_with("/>") { "/>" } else { ">" };
        done.push_str(&t[..t.len() - tail.len()]);
        done.push_str(&format!(" class=\"{}\"{tail}", classes.join(" ")));
        out.push_str(&done);
    }
    out.push_str(rest);
    let rules: String = used.iter().map(|(prop, key)| format!(".{prop}-{key}{{{}:{}}}", if *prop == 's' { "stroke" } else { "fill" }, dark.role(key).unwrap_or("inherit"))).collect();
    let style = format!("  <style>@media (prefers-color-scheme: dark){{{rules}}}</style>\n");
    if let Some(i) = out.find("</desc>\n") {
        out.insert_str(i + 8, &style);
    }
    out
}


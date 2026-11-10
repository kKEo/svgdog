//! svgdoc.toml: discovery, strict parsing, profiles and precedence.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde::Deserialize;

use crate::catalogue::{self, Group, RULES};
use crate::envelope::{AppError, Severity};
use crate::util::{Rgba, did_you_mean};

pub const CONFIG_NAME: &str = "svgdoc.toml";
pub const PROFILES: &[&str] = &["minimal", "docs", "strict"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Minimal,
    Docs,
    Strict,
}

impl Profile {
    pub fn parse(s: &str) -> Option<Profile> {
        match s {
            "minimal" => Some(Profile::Minimal),
            "docs" => Some(Profile::Docs),
            "strict" => Some(Profile::Strict),
            _ => None,
        }
    }

    fn default_level(&self, r: &catalogue::RuleMeta) -> Option<Severity> {
        match self {
            Profile::Docs => Some(r.severity),
            Profile::Minimal => {
                let on = r.group == Group::Structure || matches!(r.id, "script-content" | "external-resource");
                on.then_some(r.severity)
            }
            Profile::Strict => Some(match r.severity {
                Severity::Info => Severity::Warning,
                _ => Severity::Error,
            }),
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(untagged)]
enum Num {
    I(i64),
    F(f64),
}

impl Num {
    fn get(self) -> f64 {
        match self {
            Num::I(i) => i as f64,
            Num::F(f) => f,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Raw {
    version: Option<i64>,
    profile: Option<String>,
    #[serde(default)]
    target: RawTarget,
    #[serde(default)]
    style: RawStyle,
    #[serde(default)]
    rules: toml::Table,
    #[serde(default)]
    overrides: Vec<RawOverride>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawTarget {
    render_width: Option<Num>,
    backgrounds: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawStyle {
    fonts: Option<Vec<String>>,
    palette: Option<Vec<String>>,
    stroke_widths: Option<Vec<Num>>,
    grid: Option<Num>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOverride {
    files: Vec<String>,
    rules: toml::Table,
}

#[derive(Clone, Copy)]
enum Level {
    Off,
    On(Severity),
}

#[derive(Clone, Default)]
struct RuleCfg {
    level: Option<Level>,
    opts: Vec<(&'static str, f64)>,
}

struct Override {
    patterns: Vec<glob::Pattern>,
    rules: Vec<(&'static str, RuleCfg)>,
}

/// One parsed config file.
pub struct Config {
    dir: PathBuf,
    profile: Option<Profile>,
    render_width: f64,
    backgrounds: Vec<(String, Rgba)>,
    fonts: Vec<String>,
    palette: Vec<Rgba>,
    stroke_widths: Vec<f64>,
    grid: Option<f64>,
    rules: Vec<(&'static str, RuleCfg)>,
    overrides: Vec<Override>,
}

/// Rule selection given on the command line.
#[derive(Default, Clone)]
pub struct RuleSel {
    pub only: Option<Vec<&'static str>>,
    pub skip: Vec<&'static str>,
}

/// Everything the rules need to know for one file.
pub struct Settings {
    pub render_width: f64,
    pub backgrounds: Vec<(String, Rgba)>,
    pub palette: Vec<Rgba>,
    pub stroke_widths: Vec<f64>,
    pub grid: Option<f64>,
    levels: BTreeMap<&'static str, Option<Severity>>,
    opts: BTreeMap<(&'static str, &'static str), f64>,
}

impl Settings {
    pub fn severity(&self, rule: &str) -> Option<Severity> {
        self.levels.get(rule).copied().flatten()
    }
    pub fn on(&self, rule: &str) -> bool {
        self.severity(rule).is_some()
    }
    pub fn opt(&self, rule: &'static str, key: &'static str) -> f64 {
        *self.opts.get(&(rule, key)).unwrap_or_else(|| panic!("no option {rule}.{key} in the catalogue"))
    }
}

fn background(name: &str) -> Option<Rgba> {
    match name {
        "light" => Some(Rgba::WHITE),
        "dark" => Some(Rgba { r: 0x0d, g: 0x11, b: 0x17, a: 1.0 }),
        other => Rgba::parse(other),
    }
}

fn bad(path: &Path, key: &str, message: String) -> AppError {
    AppError::input("invalid-config", format!("{}: {message}", path.display())).path(key.to_string())
}

fn parse_rules(table: &toml::Table, at: &str, file: &Path) -> Result<Vec<(&'static str, RuleCfg)>, AppError> {
    let mut out = Vec::new();
    for (name, value) in table {
        let key = format!("{at}.{name}");
        let Some(meta) = catalogue::rule(name) else {
            return Err(AppError::input("unknown-rule", format!("{}: unknown rule '{name}' in {at}.", file.display()))
                .path(key)
                .suggest(did_you_mean(name, catalogue::ids())));
        };
        let level = |s: &str| -> Result<Level, AppError> {
            match s {
                "off" => Ok(Level::Off),
                other => Severity::parse(other).map(Level::On).ok_or_else(|| {
                    bad(file, &key, format!("'{other}' is not a severity for rule '{name}'; use off, info, warning or error."))
                        .suggest(did_you_mean(other, ["off", "info", "warning", "error"]))
                }),
            }
        };
        let mut cfg = RuleCfg::default();
        match value {
            toml::Value::String(s) => cfg.level = Some(level(s)?),
            toml::Value::Table(t) => {
                for (k, v) in t {
                    if k == "severity" {
                        let s = v.as_str().ok_or_else(|| bad(file, &key, format!("severity of rule '{name}' must be a string.")))?;
                        cfg.level = Some(level(s)?);
                        continue;
                    }
                    let Some(opt) = meta.options.iter().find(|o| o.key == k) else {
                        let known = meta.options.iter().map(|o| o.key).chain(["severity"]);
                        return Err(AppError::input("unknown-field", format!("{}: unknown option '{k}' for rule '{name}'.", file.display()))
                            .path(format!("{key}.{k}"))
                            .suggest(did_you_mean(k, known)));
                    };
                    let n = match v {
                        toml::Value::Integer(i) => *i as f64,
                        toml::Value::Float(f) => *f,
                        _ => return Err(bad(file, &key, format!("option '{k}' of rule '{name}' must be a number."))),
                    };
                    if n < 0.0 {
                        return Err(bad(file, &key, format!("option '{k}' of rule '{name}' must not be negative.")));
                    }
                    cfg.opts.push((opt.key, n));
                }
            }
            _ => return Err(bad(file, &key, format!("rule '{name}' must be a severity string or a table."))),
        }
        out.push((meta.id, cfg));
    }
    Ok(out)
}

impl Config {
    fn empty(dir: PathBuf) -> Config {
        Config {
            dir,
            profile: None,
            render_width: 720.0,
            backgrounds: vec![("light".into(), background("light").unwrap()), ("dark".into(), background("dark").unwrap())],
            fonts: vec![],
            palette: vec![],
            stroke_widths: vec![],
            grid: None,
            rules: vec![],
            overrides: vec![],
        }
    }

    pub fn load(path: &Path) -> Result<Config, AppError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| AppError::input("config-not-readable", format!("Cannot read config {}: {e}.", path.display())).path(path.display().to_string()))?;
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let dir = dir.canonicalize().unwrap_or(dir);
        Self::parse(&text, path, dir)
    }

    pub fn parse(text: &str, path: &Path, dir: PathBuf) -> Result<Config, AppError> {
        let raw: Raw = toml::from_str(text).map_err(|e| {
            let msg = e.message().to_string();
            if let Some(rest) = msg.strip_prefix("unknown field `") {
                let name = rest.split('`').next().unwrap_or("");
                let known: Vec<&str> = rest.split('`').skip(2).step_by(2).collect();
                return AppError::input("unknown-field", format!("{}: unknown key '{name}'.", path.display()))
                    .path(name.to_string())
                    .suggest(did_you_mean(name, known.iter().copied()));
            }
            AppError::input("invalid-config", format!("{}: {msg}", path.display()))
        })?;

        let mut c = Config::empty(dir);
        if let Some(v) = raw.version
            && v != 1
        {
            return Err(bad(path, "version", format!("config version {v} is not supported; this build reads version 1.")));
        }
        if let Some(p) = &raw.profile {
            c.profile = Some(Profile::parse(p).ok_or_else(|| {
                AppError::input("unknown-profile", format!("{}: unknown profile '{p}'.", path.display()))
                    .path("profile")
                    .suggest(did_you_mean(p, PROFILES.iter().copied()))
            })?);
        }
        if let Some(w) = raw.target.render_width {
            if w.get() <= 0.0 {
                return Err(bad(path, "target.render_width", "render_width must be positive.".into()));
            }
            c.render_width = w.get();
        }
        if let Some(list) = &raw.target.backgrounds {
            if list.is_empty() {
                return Err(bad(path, "target.backgrounds", "backgrounds must name at least one page background.".into()));
            }
            c.backgrounds.clear();
            for name in list {
                let col = background(name)
                    .ok_or_else(|| bad(path, "target.backgrounds", format!("'{name}' is not light, dark or a colour.")))?;
                c.backgrounds.push((name.clone(), col));
            }
        }
        c.fonts = raw.style.fonts.unwrap_or_default();
        for p in raw.style.palette.unwrap_or_default() {
            c.palette.push(Rgba::parse(&p).ok_or_else(|| bad(path, "style.palette", format!("'{p}' is not a colour.")))?);
        }
        c.stroke_widths = raw.style.stroke_widths.unwrap_or_default().into_iter().map(Num::get).collect();
        if let Some(g) = raw.style.grid {
            if g.get() <= 0.0 {
                return Err(bad(path, "style.grid", "grid must be positive.".into()));
            }
            c.grid = Some(g.get());
        }
        c.rules = parse_rules(&raw.rules, "rules", path)?;
        for (i, o) in raw.overrides.iter().enumerate() {
            let at = format!("overrides[{i}]");
            let mut patterns = Vec::new();
            for f in &o.files {
                patterns.push(glob::Pattern::new(f).map_err(|e| bad(path, &format!("{at}.files"), format!("'{f}' is not a valid glob: {e}.")))?);
            }
            c.overrides.push(Override { patterns, rules: parse_rules(&o.rules, &format!("{at}.rules"), path)? });
        }
        Ok(c)
    }

    /// `[style] fonts`, the font-family list render writes.
    pub fn fonts(&self) -> &[String] {
        &self.fonts
    }

    /// Resolve the precedence chain for one input file: flags, then overrides
    /// (later blocks first), then `[rules]`, then the profile.
    pub fn settings(&self, file: Option<&Path>, cli_profile: Option<Profile>, sel: &RuleSel) -> Settings {
        let profile = cli_profile.or(self.profile).unwrap_or(Profile::Docs);
        let mut levels: BTreeMap<&'static str, Option<Severity>> = BTreeMap::new();
        let mut opts = BTreeMap::new();
        for r in RULES {
            levels.insert(r.id, profile.default_level(r));
            for o in r.options {
                opts.insert((r.id, o.key), o.default);
            }
        }
        let mut apply = |rules: &[(&'static str, RuleCfg)]| {
            for (id, cfg) in rules {
                match cfg.level {
                    Some(Level::Off) => {
                        levels.insert(id, None);
                    }
                    Some(Level::On(s)) => {
                        levels.insert(id, Some(s));
                    }
                    None => {}
                }
                for (k, v) in &cfg.opts {
                    opts.insert((*id, *k), *v);
                }
            }
        };
        apply(&self.rules);
        if let Some(file) = file {
            let abs = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
            let rel = abs.strip_prefix(&self.dir).unwrap_or(file);
            let mo = glob::MatchOptions { case_sensitive: true, require_literal_separator: true, require_literal_leading_dot: false };
            for o in &self.overrides {
                if o.patterns.iter().any(|p| p.matches_path_with(rel, mo)) {
                    apply(&o.rules);
                }
            }
        }
        if let Some(only) = &sel.only {
            for r in RULES {
                if only.contains(&r.id) {
                    if levels[r.id].is_none() {
                        levels.insert(r.id, Some(r.severity));
                    }
                } else {
                    levels.insert(r.id, None);
                }
            }
        }
        for id in &sel.skip {
            levels.insert(id, None);
        }
        Settings {
            render_width: self.render_width,
            backgrounds: self.backgrounds.clone(),
            palette: self.palette.clone(),
            stroke_widths: self.stroke_widths.clone(),
            grid: self.grid,
            levels,
            opts,
        }
    }
}

/// Finds and caches the config that applies to each input file.
pub struct Loader {
    explicit: Option<Rc<Config>>,
    no_config: bool,
    cache: HashMap<PathBuf, Rc<Config>>,
    pub loaded: Vec<PathBuf>,
}

impl Loader {
    pub fn new(explicit: Option<&Path>, no_config: bool) -> Result<Loader, AppError> {
        let mut loaded = Vec::new();
        let explicit = match explicit {
            Some(p) => {
                loaded.push(p.to_path_buf());
                Some(Rc::new(Config::load(p)?))
            }
            None => None,
        };
        Ok(Loader { explicit, no_config, cache: HashMap::new(), loaded })
    }

    /// The config for an input file; `None` means stdin, which uses the working directory.
    pub fn for_input(&mut self, input: Option<&Path>) -> Result<Rc<Config>, AppError> {
        let cwd = std::env::current_dir().unwrap_or_default();
        if self.no_config {
            return Ok(Rc::new(Config::empty(cwd)));
        }
        if let Some(c) = &self.explicit {
            return Ok(c.clone());
        }
        let start = match input {
            Some(p) => {
                let abs = p.canonicalize().unwrap_or_else(|_| cwd.join(p));
                abs.parent().map(|d| d.to_path_buf()).unwrap_or(cwd.clone())
            }
            None => cwd.clone(),
        };
        if let Some(c) = self.cache.get(&start) {
            return Ok(c.clone());
        }
        let mut dir = Some(start.as_path());
        let mut found = None;
        while let Some(d) = dir {
            let candidate = d.join(CONFIG_NAME);
            if candidate.is_file() {
                found = Some(candidate);
                break;
            }
            dir = d.parent();
        }
        let cfg = match found {
            Some(p) => {
                let c = Config::load(&p)?;
                self.loaded.push(p);
                Rc::new(c)
            }
            None => Rc::new(Config::empty(start.clone())),
        };
        self.cache.insert(start, cfg.clone());
        Ok(cfg)
    }
}

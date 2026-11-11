//! Small shared helpers: number formatting, colours, name suggestions.

use serde_json::Value;

/// A number rounded to two decimals; integral values serialise without a fraction.
pub fn num(v: f64) -> Value {
    if !v.is_finite() {
        return Value::Null;
    }
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 && r.abs() < 1e15 {
        Value::from(r as i64)
    } else {
        Value::from(r)
    }
}

/// Human form of the same rounding, for messages.
pub fn fmt(v: f64) -> String {
    match num(v) {
        Value::Number(n) => n.to_string(),
        _ => "?".into(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Rgba {
    pub const WHITE: Rgba = Rgba { r: 255, g: 255, b: 255, a: 1.0 };
    pub const BLACK: Rgba = Rgba { r: 0, g: 0, b: 0, a: 1.0 };

    pub fn parse(s: &str) -> Option<Rgba> {
        use std::str::FromStr;
        let c = svgtypes::Color::from_str(s.trim()).ok()?;
        Some(Rgba { r: c.red, g: c.green, b: c.blue, a: c.alpha as f64 / 255.0 })
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn same_rgb(&self, o: &Rgba) -> bool {
        self.r == o.r && self.g == o.g && self.b == o.b
    }

    /// This colour at `alpha` painted over an opaque background.
    pub fn over(&self, alpha: f64, bg: &Rgba) -> Rgba {
        let a = alpha.clamp(0.0, 1.0);
        let mix = |f: u8, b: u8| (f as f64 * a + b as f64 * (1.0 - a)).round() as u8;
        Rgba { r: mix(self.r, bg.r), g: mix(self.g, bg.g), b: mix(self.b, bg.b), a: 1.0 }
    }

    /// WCAG relative luminance.
    pub fn luminance(&self) -> f64 {
        let ch = |v: u8| {
            let c = v as f64 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(self.r) + 0.7152 * ch(self.g) + 0.0722 * ch(self.b)
    }

    pub fn distance(&self, o: &Rgba) -> f64 {
        let d = |a: u8, b: u8| (a as f64 - b as f64).powi(2);
        (d(self.r, o.r) + d(self.g, o.g) + d(self.b, o.b)).sqrt()
    }
}

/// WCAG contrast ratio between two opaque colours.
pub fn contrast(a: &Rgba, b: &Rgba) -> f64 {
    let (la, lb) = (a.luminance(), b.luminance());
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The closest candidate to `name`, if one is near enough to be a likely typo.
pub fn nearest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).max(2);
    candidates
        .into_iter()
        .map(|c| (levenshtein(&name.to_lowercase(), &c.to_lowercase()), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, c)| (*d, *c))
        .map(|(_, c)| c)
}

pub fn did_you_mean<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    nearest(name, candidates).map(|c| format!("Did you mean '{c}'?"))
}

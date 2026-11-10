//! Resolved presentation properties: attributes, `<style>` rules and inline styles.

use std::str::FromStr;

use roxmltree::Node;

use crate::util::Rgba;

#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    None,
    Color(Rgba),
    /// A gradient or pattern, by id.
    Server(String),
}

impl Paint {
    pub fn describe(&self) -> String {
        match self {
            Paint::None => "none".into(),
            Paint::Color(c) => c.hex(),
            Paint::Server(id) => format!("url(#{id})"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    Alphabetic,
    Middle,
    Hanging,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub fill: Paint,
    pub fill_opacity: f64,
    pub stroke: Paint,
    pub stroke_opacity: f64,
    pub stroke_width: f64,
    /// Product of `opacity` down the tree.
    pub opacity: f64,
    pub color: Rgba,
    pub font_size: f64,
    pub font_family: Option<String>,
    pub bold: bool,
    pub anchor: Anchor,
    pub baseline: Baseline,
    pub display: bool,
    pub visible: bool,
    pub marker_start: Option<String>,
    pub marker_end: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            fill: Paint::Color(Rgba::BLACK),
            fill_opacity: 1.0,
            stroke: Paint::None,
            stroke_opacity: 1.0,
            stroke_width: 1.0,
            opacity: 1.0,
            color: Rgba::BLACK,
            font_size: 16.0,
            font_family: None,
            bold: false,
            anchor: Anchor::Start,
            baseline: Baseline::Alphabetic,
            display: true,
            visible: true,
            marker_start: None,
            marker_end: None,
        }
    }
}

pub type Decls<'a> = Vec<(&'a str, &'a str)>;

pub fn get<'a>(decls: &Decls<'a>, name: &str) -> Option<&'a str> {
    decls.iter().rev().find(|(n, _)| *n == name).map(|(_, v)| v.trim())
}

/// A length in user units. `font` resolves em; `reference` resolves percentages.
pub fn length(value: &str, font: f64, reference: f64) -> Option<f64> {
    use svgtypes::LengthUnit as U;
    let l = svgtypes::Length::from_str(value.trim()).ok()?;
    Some(match l.unit {
        U::None | U::Px => l.number,
        U::Em => l.number * font,
        U::Ex => l.number * font / 2.0,
        U::In => l.number * 96.0,
        U::Cm => l.number * 96.0 / 2.54,
        U::Mm => l.number * 96.0 / 25.4,
        U::Pt => l.number * 4.0 / 3.0,
        U::Pc => l.number * 16.0,
        U::Percent => l.number / 100.0 * reference,
    })
}

pub fn url_id(value: &str) -> Option<&str> {
    let v = value.trim();
    let rest = v.strip_prefix("url(")?;
    let inner = rest.split(')').next()?.trim().trim_matches(|c| c == '"' || c == '\'');
    inner.strip_prefix('#')
}

fn paint(value: &str, current: Rgba, inherited: &Paint) -> Paint {
    match svgtypes::Paint::from_str(value) {
        Ok(svgtypes::Paint::None) => Paint::None,
        Ok(svgtypes::Paint::Inherit) => inherited.clone(),
        Ok(svgtypes::Paint::CurrentColor) => Paint::Color(current),
        Ok(svgtypes::Paint::Color(c)) => Paint::Color(Rgba { r: c.red, g: c.green, b: c.blue, a: c.alpha as f64 / 255.0 }),
        Ok(svgtypes::Paint::FuncIRI(id, _)) => Paint::Server(id.to_string()),
        Ok(_) => inherited.clone(),
        // An unparsable paint is reported by invalid-value; keep what was inherited.
        Err(_) => inherited.clone(),
    }
}

fn unit_interval(value: &str) -> Option<f64> {
    let v = value.trim();
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse::<f64>().ok().map(|n| (n / 100.0).clamp(0.0, 1.0));
    }
    v.parse::<f64>().ok().map(|n| n.clamp(0.0, 1.0))
}

fn font_size(value: &str, parent: f64) -> Option<f64> {
    Some(match value {
        "xx-small" => 9.0,
        "x-small" => 10.0,
        "small" => 13.0,
        "medium" => 16.0,
        "large" => 18.0,
        "x-large" => 24.0,
        "xx-large" => 32.0,
        "smaller" => parent / 1.2,
        "larger" => parent * 1.2,
        "inherit" => parent,
        v => length(v, parent, parent)?,
    })
}

impl Style {
    /// The style of a child element that declares `decls`.
    pub fn derive(&self, decls: &Decls) -> Style {
        let mut s = self.clone();
        if let Some(v) = get(decls, "color")
            && let Some(c) = Rgba::parse(v)
        {
            s.color = c;
        }
        if let Some(v) = get(decls, "font-size")
            && let Some(px) = font_size(v, self.font_size)
        {
            s.font_size = px;
        }
        if let Some(v) = get(decls, "fill") {
            s.fill = paint(v, s.color, &self.fill);
        }
        if let Some(v) = get(decls, "stroke") {
            s.stroke = paint(v, s.color, &self.stroke);
        }
        if let Some(v) = get(decls, "fill-opacity").and_then(unit_interval) {
            s.fill_opacity = v;
        }
        if let Some(v) = get(decls, "stroke-opacity").and_then(unit_interval) {
            s.stroke_opacity = v;
        }
        if let Some(v) = get(decls, "stroke-width").and_then(|v| length(v, s.font_size, 100.0)) {
            s.stroke_width = v.max(0.0);
        }
        if let Some(v) = get(decls, "opacity").and_then(unit_interval) {
            s.opacity = self.opacity * v;
        }
        if let Some(v) = get(decls, "font-family")
            && v != "inherit"
        {
            s.font_family = Some(v.to_string());
        }
        if let Some(v) = get(decls, "font-weight") {
            s.bold = match v {
                "bold" | "bolder" => true,
                "normal" | "lighter" => false,
                "inherit" => self.bold,
                n => n.parse::<f64>().map(|w| w >= 600.0).unwrap_or(self.bold),
            };
        }
        if let Some(v) = get(decls, "text-anchor") {
            s.anchor = match v {
                "middle" => Anchor::Middle,
                "end" => Anchor::End,
                "start" => Anchor::Start,
                _ => self.anchor,
            };
        }
        if let Some(v) = get(decls, "dominant-baseline").or(get(decls, "alignment-baseline")) {
            s.baseline = match v {
                "middle" | "central" => Baseline::Middle,
                "hanging" | "text-before-edge" | "text-top" => Baseline::Hanging,
                "inherit" => self.baseline,
                _ => Baseline::Alphabetic,
            };
        }
        if let Some(v) = get(decls, "display") {
            s.display = v != "none";
        }
        if let Some(v) = get(decls, "visibility") {
            s.visible = match v {
                "hidden" | "collapse" => false,
                "inherit" => self.visible,
                _ => true,
            };
        }
        let marker = |name: &str, inherited: &Option<String>| -> Option<String> {
            match get(decls, name).or(get(decls, "marker")) {
                Some("none") => None,
                Some("inherit") | None => inherited.clone(),
                Some(v) => url_id(v).map(|s| s.to_string()),
            }
        };
        s.marker_start = marker("marker-start", &self.marker_start);
        s.marker_end = marker("marker-end", &self.marker_end);
        s
    }

    pub fn fill_alpha(&self) -> f64 {
        match &self.fill {
            Paint::None => 0.0,
            Paint::Color(c) => c.a * self.fill_opacity * self.opacity,
            Paint::Server(_) => self.fill_opacity * self.opacity,
        }
    }

    pub fn stroke_alpha(&self) -> f64 {
        if self.stroke_width <= 0.0 {
            return 0.0;
        }
        match &self.stroke {
            Paint::None => 0.0,
            Paint::Color(c) => c.a * self.stroke_opacity * self.opacity,
            Paint::Server(_) => self.stroke_opacity * self.opacity,
        }
    }
}

/// Properties that may be written as attributes.
pub const PRESENTATION: &[&str] = &[
    "alignment-baseline", "baseline-shift", "clip", "clip-path", "clip-rule", "color", "color-interpolation",
    "color-interpolation-filters", "color-rendering", "cursor", "direction", "display", "dominant-baseline",
    "enable-background", "fill", "fill-opacity", "fill-rule", "filter", "flood-color", "flood-opacity", "font",
    "font-family", "font-size", "font-size-adjust", "font-stretch", "font-style", "font-variant", "font-weight",
    "glyph-orientation-horizontal", "glyph-orientation-vertical", "image-rendering", "isolation", "kerning",
    "letter-spacing", "lighting-color", "line-height", "marker", "marker-end", "marker-mid", "marker-start",
    "mask", "mask-type", "mix-blend-mode", "opacity", "overflow", "paint-order", "pointer-events",
    "shape-rendering", "stop-color", "stop-opacity", "stroke", "stroke-dasharray", "stroke-dashoffset",
    "stroke-linecap", "stroke-linejoin", "stroke-miterlimit", "stroke-opacity", "stroke-width", "text-anchor",
    "text-decoration", "text-overflow", "text-rendering", "transform", "transform-box", "transform-origin",
    "unicode-bidi", "vector-effect", "visibility", "white-space", "word-spacing", "writing-mode",
];

struct CssNode<'a, 'i>(Node<'a, 'i>);

impl simplecss::Element for CssNode<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(CssNode)
    }
    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(CssNode)
    }
    fn has_local_name(&self, name: &str) -> bool {
        self.0.tag_name().name() == name
    }
    fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator<'_>) -> bool {
        self.0.attribute(local_name).is_some_and(|v| operator.matches(v))
    }
    fn pseudo_class_matches(&self, class: simplecss::PseudoClass<'_>) -> bool {
        matches!(class, simplecss::PseudoClass::FirstChild) && self.0.prev_sibling_element().is_none()
    }
}

/// All `<style>` rules of a document, in cascade order.
pub struct Css<'a> {
    rules: Vec<simplecss::Rule<'a>>,
}

impl<'a> Css<'a> {
    pub fn new(texts: &[&'a str]) -> Css<'a> {
        let mut sheet = simplecss::StyleSheet::new();
        for t in texts {
            sheet.parse_more(t);
        }
        let mut rules = sheet.rules;
        rules.sort_by_key(|r| r.selector.specificity());
        Css { rules }
    }

    pub fn rules(&self) -> &[simplecss::Rule<'a>] {
        &self.rules
    }

    /// Everything an element declares, lowest priority first.
    pub fn declared<'i>(&self, node: Node<'a, 'i>) -> Decls<'a> {
        let mut out: Decls<'a> = Vec::new();
        for a in node.attributes() {
            if a.namespace().is_none() {
                out.push((a.name(), a.value()));
            }
        }
        if !self.rules.is_empty() {
            let el = CssNode(node);
            for r in &self.rules {
                if r.selector.matches(&el) {
                    for d in &r.declarations {
                        out.push((d.name, d.value));
                    }
                }
            }
        }
        if let Some(style) = node.attribute("style") {
            for d in simplecss::DeclarationTokenizer::from(style) {
                out.push((d.name, d.value));
            }
        }
        out
    }
}

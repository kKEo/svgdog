//! Text measurement with the bundled fonts, so results match on every machine.

static INTER: &[u8] = include_bytes!("../../fonts/Inter-Regular.ttf");
static INTER_BOLD: &[u8] = include_bytes!("../../fonts/Inter-Bold.ttf");
static MONO: &[u8] = include_bytes!("../../fonts/JetBrainsMono-Regular.ttf");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Sans,
    Mono,
}

struct Fonts {
    sans: rustybuzz::Face<'static>,
    bold: rustybuzz::Face<'static>,
    mono: rustybuzz::Face<'static>,
}

thread_local! {
    static FONTS: Fonts = Fonts {
        sans: rustybuzz::Face::from_slice(INTER, 0).expect("bundled Inter Regular"),
        bold: rustybuzz::Face::from_slice(INTER_BOLD, 0).expect("bundled Inter Bold"),
        mono: rustybuzz::Face::from_slice(MONO, 0).expect("bundled JetBrains Mono"),
    };
}

impl Fonts {
    fn face(&self, class: Class, bold: bool) -> &rustybuzz::Face<'static> {
        match (class, bold) {
            (Class::Mono, _) => &self.mono,
            (Class::Sans, true) => &self.bold,
            (Class::Sans, false) => &self.sans,
        }
    }
}

pub struct Measured {
    /// Advance width in em.
    pub width: f64,
    /// True when a character had no glyph in the bundled face.
    pub missing: bool,
}

pub fn measure(text: &str, class: Class, bold: bool) -> Measured {
    if text.is_empty() {
        return Measured { width: 0.0, missing: false };
    }
    FONTS.with(|f| {
        let face = f.face(class, bold);
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(text);
        let out = rustybuzz::shape(face, &[], buf);
        let upem = face.units_per_em() as f64;
        let adv: i64 = out.glyph_positions().iter().map(|p| p.x_advance as i64).sum();
        let missing = out.glyph_infos().iter().any(|g| g.glyph_id == 0);
        Measured { width: adv as f64 / upem, missing }
    })
}

/// Ascent and descent in em, both positive.
pub fn vmetrics(class: Class, bold: bool) -> (f64, f64) {
    FONTS.with(|f| {
        let face = f.face(class, bold);
        let upem = face.units_per_em() as f64;
        (face.ascender() as f64 / upem, -(face.descender() as f64) / upem)
    })
}

const MONO_NAMES: &[&str] = &[
    "monospace", "ui-monospace", "jetbrains mono", "courier", "courier new", "consolas", "menlo", "monaco",
    "fira code", "fira mono", "source code pro", "sf mono", "sfmono-regular", "dejavu sans mono",
    "liberation mono", "roboto mono", "ibm plex mono", "cascadia code", "andale mono", "lucida console",
];

pub const GENERIC: &[&str] = &[
    "serif", "sans-serif", "monospace", "cursive", "fantasy", "system-ui", "ui-serif", "ui-sans-serif",
    "ui-monospace", "ui-rounded", "math", "emoji", "fangsong",
];

pub fn families(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|f| f.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_lowercase())
        .filter(|f| !f.is_empty())
        .collect()
}

/// Which bundled face stands in for a font-family list, and whether it is the named font itself.
pub fn classify(font_family: Option<&str>) -> (Class, bool) {
    let Some(value) = font_family else { return (Class::Sans, false) };
    let list = families(value);
    let Some(first) = list.first() else { return (Class::Sans, false) };
    let class = if MONO_NAMES.contains(&first.as_str()) { Class::Mono } else { Class::Sans };
    let exact = first == "inter" || first == "jetbrains mono";
    (class, exact)
}

struct Outline {
    d: String,
    x: f64,
    y: f64,
    scale: f64,
}

impl Outline {
    fn pt(&self, x: f32, y: f32) -> String {
        format!("{} {}", crate::util::fmt(self.x + x as f64 * self.scale), crate::util::fmt(self.y - y as f64 * self.scale))
    }
}

impl rustybuzz::ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.d.push_str(&format!("M{}", self.pt(x, y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.d.push_str(&format!("L{}", self.pt(x, y)));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.d.push_str(&format!("Q{} {}", self.pt(x1, y1), self.pt(x, y)));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.d.push_str(&format!("C{} {} {}", self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y)));
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// Path data for a line of text set in the bundled sans face, starting at `x` on `baseline`.
pub fn outline(text: &str, size: f64, x: f64, baseline: f64) -> String {
    FONTS.with(|f| {
        let face = f.face(Class::Sans, false);
        let mut buf = rustybuzz::UnicodeBuffer::new();
        buf.push_str(text);
        let shaped = rustybuzz::shape(face, &[], buf);
        let scale = size / face.units_per_em() as f64;
        let mut pen = 0.0;
        let mut d = String::new();
        for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            let mut o = Outline { d: String::new(), x: x + (pen + pos.x_offset as f64) * scale, y: baseline - pos.y_offset as f64 * scale, scale };
            face.outline_glyph(rustybuzz::ttf_parser::GlyphId(info.glyph_id as u16), &mut o);
            d.push_str(&o.d);
            pen += pos.x_advance as f64;
        }
        d
    })
}

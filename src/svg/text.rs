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

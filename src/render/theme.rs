//! Named themes. docs-light uses the palette, stroke widths and grid from the spec's sample config.

pub struct Theme {
    pub name: &'static str,
    pub background: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub node_fill: &'static str,
    pub node_stroke: &'static str,
    pub accent_fill: &'static str,
    pub accent_text: &'static str,
    pub note_fill: &'static str,
    pub edge: &'static str,
}

pub const THEMES: &[Theme] = &[
    Theme {
        name: "docs-light",
        background: "#ffffff",
        text: "#1f2937",
        muted: "#6b7280",
        node_fill: "#dbeafe",
        node_stroke: "#1f2937",
        accent_fill: "#2563eb",
        accent_text: "#ffffff",
        note_fill: "#ffffff",
        edge: "#1f2937",
    },
    Theme {
        name: "docs-dark",
        background: "#0d1117",
        text: "#e5e7eb",
        muted: "#94a3b8",
        node_fill: "#1e293b",
        node_stroke: "#94a3b8",
        accent_fill: "#60a5fa",
        accent_text: "#0d1117",
        note_fill: "#0d1117",
        edge: "#e5e7eb",
    },
];

pub fn names() -> impl Iterator<Item = &'static str> {
    THEMES.iter().map(|t| t.name)
}

pub fn find(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.name == name)
}

pub const FONT: f64 = 14.0;
pub const SMALL: f64 = 12.0;
pub const STROKE: f64 = 1.5;
pub const THIN: f64 = 1.0;

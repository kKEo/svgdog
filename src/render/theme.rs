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
    // Placeholders, one per role: `adapt` turns them into the light colours plus
    // classes that a dark-mode style rule overrides.
    Theme {
        name: AUTO,
        background: "@bg",
        text: "@tx",
        muted: "@mu",
        node_fill: "@nf",
        node_stroke: "@ns",
        accent_fill: "@af",
        accent_text: "@at",
        note_fill: "@no",
        edge: "@ed",
    },
];

pub const AUTO: &str = "docs-auto";

impl Theme {
    fn role(&self, token: &str) -> Option<&'static str> {
        Some(match token {
            "bg" => self.background,
            "tx" => self.text,
            "mu" => self.muted,
            "nf" => self.node_fill,
            "ns" => self.node_stroke,
            "af" => self.accent_fill,
            "at" => self.accent_text,
            "no" => self.note_fill,
            "ed" => self.edge,
            _ => return None,
        })
    }
}

/// Finish a drawing made with the placeholder theme: every colour becomes the
/// light theme's, written as an ordinary attribute, and each element gets a class
/// per colour role. One style rule then swaps in the dark theme where the reader's
/// system asks for it. A renderer that ignores the rule shows the light drawing.
pub fn adapt(svg: &str) -> String {
    let (light, dark) = (&THEMES[0], &THEMES[1]);
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
                    let key = ["bg", "tx", "mu", "nf", "ns", "af", "at", "no", "ed"].into_iter().find(|k| *k == token).unwrap_or("tx");
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

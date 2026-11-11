//! The interactive versions of a drawing. Both are the static drawing with
//! something added, never a different drawing: `css` adds classes and one style
//! block, `html` wraps that SVG in a page with a small script.

use std::collections::BTreeSet;

use super::theme::Theme;
use crate::spec::{Ref, Spec};

/// The ids whose hover keeps this thing lit: itself, what it is joined to, and the frames it sits in.
fn related(spec: &Spec, r: Ref) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let own = |r: Ref| match r {
        Ref::Node(i) => spec.nodes[i].id.clone(),
        Ref::Group(g) => spec.groups[g].id.clone(),
        Ref::Edge(e) => spec.edges[e].id.clone(),
    };
    match r {
        Ref::Edge(e) => {
            out.insert(spec.edges[e].from.clone());
            out.insert(spec.edges[e].to.clone());
        }
        _ => {
            let me = own(r);
            for e in &spec.edges {
                if e.from == me {
                    out.insert(e.to.clone());
                }
                if e.to == me {
                    out.insert(e.from.clone());
                }
            }
            for g in spec.ancestors(r) {
                out.insert(spec.groups[g].id.clone());
            }
            if let Ref::Group(g) = r {
                // A frame stays lit with anything inside it.
                for (i, n) in spec.nodes.iter().enumerate() {
                    if spec.ancestors(Ref::Node(i)).contains(&g) {
                        out.insert(n.id.clone());
                    }
                }
            }
            out.insert(me);
        }
    }
    out
}

/// The class list of one drawn thing.
pub fn classes(spec: &Spec, r: Ref) -> String {
    let mut c = vec!["it".to_string()];
    c.extend(related(spec, r).into_iter().map(|id| format!("r-{id}")));
    c.extend(spec.steps_of(r).into_iter().map(|n| format!("st-{n}")));
    c.join(" ")
}

/// A note belongs with what it is attached to.
pub fn note_classes(spec: &Spec, attach: &str) -> String {
    match spec.lookup(attach) {
        Some(r) => classes(spec, r).split(' ').filter(|c| !c.starts_with("st-")).collect::<Vec<_>>().join(" "),
        None => "it".into(),
    }
}

fn scope(svg: &str) -> String {
    // FNV-1a: a short, stable name so two drawings inlined in one page keep their rules apart.
    let mut h: u32 = 0x811c9dc5;
    for b in svg.bytes() {
        h = (h ^ b as u32).wrapping_mul(0x01000193);
    }
    format!("svgdog-{h:08x}")
}

/// Add the scope class to the root and the style block after the description.
pub fn style(svg: &str, spec: &Spec, theme: &Theme, accent: &str) -> String {
    let _ = theme;
    let name = scope(svg);
    let s = format!(".{name}");
    let mut css = String::new();
    css.push_str(&format!("{s} .it{{transition:opacity .15s}}{s} .it:focus-visible{{outline:2px solid {accent};outline-offset:3px}}{s} a .it{{cursor:pointer}}"));
    for n in &spec.nodes {
        css.push_str(&format!("{s}:has(#node-{id}:hover,#node-{id}:focus-visible) .it:not(.r-{id}){{opacity:.25}}", id = n.id));
    }
    for st in &spec.steps {
        let k = st.number;
        if !st.show.is_empty() {
            css.push_str(&format!("{s}:has(#step-{k}:hover) .it:not(.st-{k}),{s}[data-current=\"{k}\"] .it:not(.st-{k}){{opacity:.25}}"));
        }
        css.push_str(&format!("{s}[data-current=\"{k}\"] #steps>g:not(#step-{k}){{opacity:.45}}"));
    }
    let mut out = svg.replacen(" data-svgdog=\"1\"", &format!(" data-svgdog=\"1\" data-interactive=\"css\" class=\"{name}\""), 1);
    // After the description, and after the dark-mode rule if the theme added one.
    let at = out.find("</style>\n").map(|i| i + 9).or_else(|| out.find("</desc>\n").map(|i| i + 8)).unwrap_or(0);
    out.insert_str(at, &format!("  <style>{css}</style>\n"));
    out
}

const SCRIPT: &str = include_str!("player.js");

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A page that holds the drawing and, where the spec has steps or details, the controls for them.
pub fn page(svg: &str, spec: &Spec, light: &Theme, dark: Option<&Theme>) -> String {
    let palette = |t: &Theme| format!("--bg:{};--fg:{};--muted:{};--accent:{};--on:{}", t.background, t.text, t.muted, t.accent_fill, t.accent_text);
    let dark_css = dark.map(|t| format!("@media (prefers-color-scheme: dark){{:root{{{}}}}}", palette(t))).unwrap_or_default();
    let mut h = String::new();
    h.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    h.push_str(&format!("<title>{}</title>\n", esc(&spec.title)));
    h.push_str(&format!(
        "<style>:root{{{}}}{dark_css}body{{margin:0;padding:16px;background:var(--bg);color:var(--fg);font:14px/1.5 Inter,system-ui,sans-serif}}figure{{margin:0 auto;max-width:{}px}}figure svg{{display:block;width:100%;height:auto}}.bar{{display:flex;gap:8px;align-items:center;margin-top:12px;flex-wrap:wrap}}.bar button{{font:inherit;padding:4px 12px;border:1px solid var(--muted);border-radius:6px;background:var(--bg);color:var(--fg);cursor:pointer}}.bar button:disabled{{opacity:.4;cursor:default}}.bar button:focus-visible{{outline:2px solid var(--accent);outline-offset:2px}}.bar output{{color:var(--muted)}}.detail{{margin-top:12px;padding:8px 12px;border-left:3px solid var(--accent);min-height:1.5em}}.detail:empty{{display:none}}</style>\n",
        palette(light),
        svg.split("viewBox=\"0 0 ").nth(1).and_then(|v| v.split(' ').next()).unwrap_or("960")
    ));
    h.push_str("</head>\n<body>\n<figure>\n");
    h.push_str(svg);
    if !spec.steps.is_empty() {
        h.push_str("<div class=\"bar\" hidden><button type=\"button\" data-go=\"prev\">Previous</button><button type=\"button\" data-go=\"next\">Next</button><button type=\"button\" data-go=\"all\">Show all</button><output aria-live=\"polite\"></output></div>\n");
    }
    h.push_str("<p class=\"detail\" aria-live=\"polite\"></p>\n</figure>\n");
    h.push_str(&format!("<script>\n{}</script>\n</body>\n</html>\n", SCRIPT.replace("__STEPS__", &spec.steps.len().to_string())));
    h
}

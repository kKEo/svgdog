//! A diagram in words: what a reader who cannot see the picture needs to be told.

use crate::spec::{Arrow, Diagram, Direction, EdgeStyle, Kind, Ref, Spec};

fn type_name(d: Diagram) -> &'static str {
    match d {
        Diagram::Flow => "Flow diagram",
        Diagram::Layers => "Layered diagram",
        Diagram::Sequence => "Sequence diagram",
        Diagram::Free => "Diagram",
        Diagram::State => "State diagram",
        Diagram::Timeline => "Timeline",
        Diagram::Compare => "Comparison",
    }
}

/// What to call something in a sentence.
fn name(spec: &Spec, id: &str) -> String {
    match spec.lookup(id) {
        Some(Ref::Node(i)) => match spec.nodes[i].kind {
            Kind::Start => "the start".to_string(),
            Kind::End => "the end".to_string(),
            _ => spec.nodes[i].label.clone(),
        },
        Some(Ref::Group(g)) => spec.groups[g].label.clone().unwrap_or_else(|| spec.groups[g].id.clone()),
        Some(Ref::Edge(e)) => format!("{} to {}", name(spec, &spec.edges[e].from), name(spec, &spec.edges[e].to)),
        None => id.to_string(),
    }
}

fn edge_phrase(spec: &Spec, e: usize) -> String {
    let edge = &spec.edges[e];
    let (from, to) = (name(spec, &edge.from), if edge.from == edge.to { "itself".to_string() } else { name(spec, &edge.to) });
    let link = match edge.arrow {
        Arrow::Both => "to and from",
        Arrow::None => "with",
        Arrow::End => "to",
    };
    match &edge.label {
        Some(l) => format!("{from} {link} {to} ({l})"),
        None => format!("{from} {link} {to}"),
    }
}

/// One or two sentences: the picture's `<desc>` when the spec gives no description, and alt text.
pub fn short(spec: &Spec) -> String {
    const LIMIT: usize = 6;
    let lead = type_name(spec.diagram);
    let listed = |items: Vec<String>, noun: &str| -> String {
        let more = items.len().saturating_sub(LIMIT);
        let mut s = items.into_iter().take(LIMIT).collect::<Vec<_>>().join("; ");
        if more > 0 {
            s.push_str(&format!("; and {more} more {noun}{}", if more == 1 { "" } else { "s" }));
        }
        s
    };
    match spec.diagram {
        Diagram::Timeline => {
            let items = spec.nodes.iter().map(|n| match &n.when {
                Some(w) => format!("{w}, {}", n.label),
                None => n.label.clone(),
            });
            format!("{lead}: {}.", listed(items.collect(), "entry").replace("entrys", "entries"))
        }
        Diagram::Compare => {
            let sides = spec.sides();
            let side = |k: usize| format!("{} ({})", name(spec, &spec.groups[sides[k].0].id), sides[k].1.iter().map(|i| spec.nodes[*i].label.clone()).collect::<Vec<_>>().join(", "));
            format!("{lead} of {} with {}.", side(0), side(1))
        }
        _ if spec.edges.is_empty() => format!("{lead} of {}.", listed(spec.nodes.iter().filter(|n| !n.label.is_empty()).map(|n| n.label.clone()).collect(), "node")),
        _ => format!("{lead}: {}.", listed((0..spec.edges.len()).map(|e| edge_phrase(spec, e)).collect(), "connection")),
    }
}

/// The whole diagram as text, section by section.
pub fn full(spec: &Spec) -> String {
    let mut out = String::new();
    let way = match (spec.diagram, spec.direction) {
        (Diagram::Flow | Diagram::Layers | Diagram::State | Diagram::Timeline, Direction::Down) => ", top to bottom",
        (Diagram::Flow | Diagram::State | Diagram::Timeline, Direction::Right) => ", left to right",
        _ => "",
    };
    out.push_str(&format!("{} ({}{way})\n", spec.title, type_name(spec.diagram).to_lowercase()));
    if let Some(d) = &spec.description {
        out.push_str(&format!("{d}\n"));
    }
    let kind = |k: Kind| match k {
        Kind::Box | Kind::State => None,
        Kind::Service => Some("service"),
        Kind::Client => Some("client"),
        Kind::Datastore => Some("data store"),
        Kind::Queue => Some("queue"),
        Kind::Actor => Some("actor"),
        Kind::External => Some("external"),
        Kind::Decision => Some("decision"),
        Kind::Document => Some("document"),
        Kind::Start => Some("start marker"),
        Kind::End => Some("end marker"),
    };
    out.push_str(if spec.diagram == Diagram::State { "\nStates:\n" } else if spec.diagram == Diagram::Timeline { "\nIn order:\n" } else { "\nNodes:\n" });
    for (i, n) in spec.nodes.iter().enumerate() {
        let mut traits: Vec<String> = kind(n.kind).map(|k| k.to_string()).into_iter().collect();
        if n.emphasis || spec.one_sided(i) {
            traits.push("highlighted".into());
        }
        if let Some(g) = spec.node_parent[i] {
            traits.push(format!("in {}", name(spec, &spec.groups[g].id)));
        }
        let label = if n.label.is_empty() { n.id.clone() } else { n.label.clone() };
        let when = n.when.as_ref().map(|w| format!("{w}: ")).unwrap_or_default();
        let tail = if traits.is_empty() { String::new() } else { format!(" ({})", traits.join(", ")) };
        let detail = n.detail.as_ref().map(|d| format!(" - {d}")).unwrap_or_default();
        out.push_str(&format!("- {when}{label}{tail}{detail}\n"));
    }
    if !spec.edges.is_empty() {
        out.push_str(match spec.diagram {
            Diagram::Sequence => "\nMessages, in order:\n",
            Diagram::State => "\nTransitions:\n",
            _ => "\nConnections:\n",
        });
        for (e, edge) in spec.edges.iter().enumerate() {
            let style = match edge.style {
                EdgeStyle::Sync => "",
                EdgeStyle::Async => " [asynchronous]",
                EdgeStyle::Dependency => " [dependency]",
            };
            let detail = edge.detail.as_ref().map(|d| format!(" - {d}")).unwrap_or_default();
            out.push_str(&format!("- {}{style}{detail}\n", edge_phrase(spec, e)));
        }
    }
    if !spec.notes.is_empty() {
        out.push_str("\nNotes:\n");
        for n in &spec.notes {
            out.push_str(&format!("- {}: {}\n", name(spec, &n.attach), n.text));
        }
    }
    if !spec.steps.is_empty() {
        out.push_str("\nSteps:\n");
        for s in &spec.steps {
            out.push_str(&format!("{}. {}\n", s.number, s.text));
        }
    }
    if let Some(c) = &spec.caption {
        out.push_str(&format!("\n{c}\n"));
    }
    out
}

/// The spec a drawing was rendered from, when it was rendered with the spec embedded.
pub fn embedded(svg: &str) -> Option<String> {
    let doc = roxmltree::Document::parse_with_options(svg, crate::svg::parse_options()).ok()?;
    let node = doc.descendants().find(|n| n.is_element() && n.tag_name().name() == "metadata" && n.attribute("id") == Some(EMBED_ID))?;
    node.text().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

pub const EMBED_ID: &str = "svgdog-spec";

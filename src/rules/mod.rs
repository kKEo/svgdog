//! Runs the catalogue over one SVG source and turns raw hits into findings.

mod accessibility;
mod geometry;
mod portability;
mod structure;

pub use structure::refs_of as refs;

use std::collections::HashMap;

use kurbo::Rect;
use roxmltree::Node;
use serde_json::{Map, Value, json};

use crate::catalogue::{self, Group, INVALID_SUPPRESSION};
use crate::config::Settings;
use crate::envelope::{Finding, Fix, Severity};
use crate::svg::{self, Scene};
use crate::util::num;

pub struct Raw<'a, 'i> {
    pub rule: &'static str,
    pub node: Node<'a, 'i>,
    pub bbox: Option<Rect>,
    pub message: String,
    pub measured: Vec<(&'static str, Value)>,
    pub approximate: bool,
    pub fix: Option<Fix>,
}

pub struct Ctx<'s, 'a, 'i> {
    pub scene: &'s Scene<'a, 'i>,
    pub set: &'s Settings,
    pub bytes: usize,
    pub out: Vec<Raw<'a, 'i>>,
}

impl<'s, 'a, 'i> Ctx<'s, 'a, 'i> {
    pub fn on(&self, rule: &str) -> bool {
        self.set.on(rule)
    }

    pub fn push(&mut self, rule: &'static str, node: Node<'a, 'i>, bbox: Option<Rect>, message: String, measured: Vec<(&'static str, Value)>) {
        self.out.push(Raw { rule, node, bbox, message, measured, approximate: false, fix: None });
    }

    pub fn push_approx(&mut self, rule: &'static str, node: Node<'a, 'i>, bbox: Option<Rect>, message: String, measured: Vec<(&'static str, Value)>) {
        self.out.push(Raw { rule, node, bbox, message, measured, approximate: true, fix: None });
    }

    /// Attach an autofix to the finding pushed last.
    pub fn fix(&mut self, safe: bool, ops: Vec<Value>) {
        if let Some(last) = self.out.last_mut()
            && !ops.is_empty()
        {
            last.fix = Some(Fix { safe, ops });
        }
    }

    pub fn target(&self, node: Node) -> String {
        self.scene.target(node)
    }

    /// An operation that sets a presentation property where it takes effect:
    /// in the inline style when a style already governs it, else as an attribute.
    pub fn set_prop(&self, node: Node<'a, 'i>, prop: &str, value: Value) -> Value {
        let key = if self.scene.css.styled(node, prop) { "style" } else { "attr" };
        json!({ "op": "set", "target": self.target(node), key: prop, "value": value })
    }
}

pub fn bbox_json(r: Rect) -> [Value; 4] {
    [num(r.x0), num(r.y0), num(r.width()), num(r.height())]
}

/// `svgdog-disable` comments: element node id -> silenced rule ids.
fn suppressions<'a, 'i>(scene: &Scene<'a, 'i>, file: &str, findings: &mut Vec<Finding>) -> HashMap<usize, Vec<&'static str>> {
    let mut map: HashMap<usize, Vec<&'static str>> = HashMap::new();
    for c in scene.doc.descendants().filter(|n| n.is_comment()) {
        let text = c.text().unwrap_or("").trim();
        let Some(rest) = text.strip_prefix("svgdog-disable") else { continue };
        let mut invalid = |message: String| {
            findings.push(Finding {
                rule: INVALID_SUPPRESSION,
                severity: Severity::Error,
                file: file.to_string(),
                target: ":root".into(),
                line: scene.doc.text_pos_at(c.range().start).row,
                bbox: None,
                message,
                measured: None,
                fix: None,
            });
        };
        let Some((ids, reason)) = rest.split_once(':') else {
            invalid("Suppression has no reason; write 'svgdog-disable <rule>: <reason>'.".into());
            continue;
        };
        if reason.trim().is_empty() {
            invalid("Suppression has an empty reason; say why the rule does not apply.".into());
            continue;
        }
        let mut rules = Vec::new();
        let mut ok = true;
        for id in ids.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            match catalogue::rule(id) {
                Some(r) => rules.push(r.id),
                None => {
                    let hint = crate::util::did_you_mean(id, catalogue::ids()).map(|s| format!(" {s}")).unwrap_or_default();
                    invalid(format!("Suppression names unknown rule '{id}'.{hint}"));
                    ok = false;
                }
            }
        }
        if !ok {
            continue;
        }
        if rules.is_empty() {
            invalid("Suppression names no rule.".into());
            continue;
        }
        let mut target = None;
        for s in c.next_siblings().skip(1) {
            if s.is_element() {
                target = Some(s);
                break;
            }
            if s.is_text() && !s.text().unwrap_or("").trim().is_empty() {
                break;
            }
        }
        match target {
            Some(t) => map.entry(t.id().get_usize()).or_default().extend(rules),
            None => invalid("Suppression is not directly before an element.".into()),
        }
    }
    map
}

/// Validate one source text. Findings come back sorted by line, then rule id.
pub fn analyze(file: &str, src: &str, set: &Settings) -> Vec<Finding> {
    let mut findings = Vec::new();
    let doc = match roxmltree::Document::parse_with_options(src, svg::parse_options()) {
        Ok(d) => d,
        Err(e) => {
            if let Some(sev) = set.severity("xml-well-formed") {
                findings.push(Finding {
                    rule: "xml-well-formed",
                    severity: sev,
                    file: file.to_string(),
                    target: ":root".into(),
                    line: e.pos().row,
                    bbox: None,
                    message: format!("The file is not well-formed XML: {e}."),
                    measured: None,
                    fix: None,
                });
            }
            return findings;
        }
    };
    let scene = Scene::build(&doc);
    let suppressed = suppressions(&scene, file, &mut findings);
    let mut ctx = Ctx { scene: &scene, set, bytes: src.len(), out: Vec::new() };
    structure::run(&mut ctx);
    geometry::run(&mut ctx);
    portability::run(&mut ctx);
    accessibility::run(&mut ctx);

    // A finding on a drawn element carries that element's box even when its rule did not measure one.
    let boxes: HashMap<usize, Rect> = scene.elems.iter().filter(|e| e.node != scene.root).filter_map(|e| Some((e.node.id().get_usize(), e.bbox?))).collect();
    for raw in ctx.out {
        let Some(mut severity) = set.severity(raw.rule) else { continue };
        if raw.node.ancestors().any(|a| suppressed.get(&a.id().get_usize()).is_some_and(|r| r.contains(&raw.rule))) {
            continue;
        }
        let geometry = catalogue::rule(raw.rule).is_some_and(|r| r.group == Group::Geometry);
        let approximate = raw.approximate || (geometry && !scene.svgdog_output);
        let mut measured = Map::new();
        for (k, v) in raw.measured {
            measured.insert(k.to_string(), v);
        }
        if approximate {
            measured.insert("approximate".into(), json!(true));
            severity = severity.min(Severity::Warning);
        }
        findings.push(Finding {
            rule: raw.rule,
            severity,
            file: file.to_string(),
            target: scene.target(raw.node),
            line: scene.line(raw.node),
            bbox: raw.bbox.or_else(|| boxes.get(&raw.node.id().get_usize()).copied()).map(bbox_json),
            message: raw.message,
            measured: (!measured.is_empty()).then_some(measured),
            fix: raw.fix,
        });
    }
    findings.sort_by(|a, b| (a.line, a.rule, &a.target, &a.message).cmp(&(b.line, b.rule, &b.target, &b.message)));
    findings
}

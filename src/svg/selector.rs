//! Element selectors: the targets findings carry and the forms `--select` accepts.
//!
//! Grammar: compounds joined by ` > `; a compound is `tag`, `#id`, `[attr="value"]`
//! and `:nth-of-type(n)` in any sensible combination.

use roxmltree::Node;

#[derive(Debug, Default, Clone, PartialEq)]
struct Compound {
    tag: Option<String>,
    id: Option<String>,
    attrs: Vec<(String, Option<String>)>,
    nth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    chain: Vec<Compound>,
}

fn is_name(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')
}

fn compound(src: &str) -> Result<Compound, String> {
    let mut c = Compound::default();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let name = |i: &mut usize| -> String {
        let start = *i;
        while *i < chars.len() && is_name(chars[*i]) && !(chars[*i] == ':' && chars[*i..].iter().collect::<String>().starts_with(":nth-of-type")) {
            *i += 1;
        }
        chars[start..*i].iter().collect()
    };
    if i < chars.len() && (is_name(chars[i]) && chars[i] != ':' || chars[i] == '*') {
        if chars[i] == '*' {
            i += 1;
        } else {
            c.tag = Some(name(&mut i));
        }
    }
    while i < chars.len() {
        match chars[i] {
            '#' => {
                i += 1;
                let n = name(&mut i);
                if n.is_empty() {
                    return Err("'#' must be followed by an id".into());
                }
                c.id = Some(n);
            }
            '[' => {
                let end = chars[i..].iter().position(|&ch| ch == ']').ok_or("missing ']'")? + i;
                let body: String = chars[i + 1..end].iter().collect();
                match body.split_once('=') {
                    Some((k, v)) => {
                        let v = v.trim().trim_matches(|ch| ch == '"' || ch == '\'');
                        c.attrs.push((k.trim().to_string(), Some(v.to_string())));
                    }
                    None => c.attrs.push((body.trim().to_string(), None)),
                }
                i = end + 1;
            }
            ':' => {
                let rest: String = chars[i..].iter().collect();
                let inner = rest.strip_prefix(":nth-of-type(").and_then(|r| r.split_once(')')).ok_or("only :nth-of-type(n) is supported")?;
                c.nth = Some(inner.0.trim().parse::<usize>().map_err(|_| "nth-of-type needs a number")?);
                i += ":nth-of-type(".len() + inner.0.len() + 1;
            }
            other => return Err(format!("unexpected '{other}'")),
        }
    }
    if c == Compound::default() && src != "*" {
        return Err("empty selector".into());
    }
    Ok(c)
}

impl Selector {
    pub fn parse(src: &str) -> Result<Selector, String> {
        let chain = src.split('>').map(|p| compound(p.trim())).collect::<Result<Vec<_>, _>>()?;
        Ok(Selector { chain })
    }

    pub fn matches(&self, node: Node) -> bool {
        let mut cur = Some(node);
        for c in self.chain.iter().rev() {
            match cur {
                Some(n) if matches_compound(c, n) => cur = n.parent_element(),
                _ => return false,
            }
        }
        true
    }
}

fn nth_of_type(node: Node) -> usize {
    let name = node.tag_name().name();
    node.prev_siblings().filter(|s| s.is_element() && s.tag_name().name() == name).count()
}

fn matches_compound(c: &Compound, n: Node) -> bool {
    if !n.is_element() {
        return false;
    }
    if let Some(t) = &c.tag
        && n.tag_name().name() != t
    {
        return false;
    }
    if let Some(id) = &c.id
        && n.attribute("id") != Some(id.as_str())
    {
        return false;
    }
    for (k, v) in &c.attrs {
        match (n.attribute(k.as_str()), v) {
            (None, _) => return false,
            (Some(have), Some(want)) if have != want => return false,
            _ => {}
        }
    }
    if let Some(nth) = c.nth
        && nth_of_type(n) != nth
    {
        return false;
    }
    true
}

/// The selector a finding uses for an element: its id where it has one, otherwise
/// a child path from the nearest ancestor that does.
pub fn target(node: Node) -> String {
    if !node.is_element() {
        return ":root".into();
    }
    let mut parts: Vec<String> = Vec::new();
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(id) = n.attribute("id")
            && !id.is_empty()
            && id.chars().all(is_name)
        {
            parts.push(format!("#{id}"));
            break;
        }
        let name = n.tag_name().name();
        let same = |s: &Node| s.is_element() && s.tag_name().name() == name;
        let twins = n.prev_siblings().skip(1).any(|s| same(&s)) || n.next_siblings().skip(1).any(|s| same(&s));
        if twins && n.parent_element().is_some() {
            parts.push(format!("{name}:nth-of-type({})", nth_of_type(n)));
        } else {
            parts.push(name.to_string());
        }
        cur = n.parent_element();
    }
    parts.reverse();
    parts.join(" > ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_round_trip() {
        let doc = roxmltree::Document::parse("<svg><g id='a'><rect/><rect/><text>x</text></g><g><rect class='k'/></g></svg>").unwrap();
        for n in doc.descendants().filter(|n| n.is_element()) {
            let t = target(n);
            let sel = Selector::parse(&t).unwrap();
            let hits: Vec<_> = doc.descendants().filter(|m| sel.matches(*m)).collect();
            assert_eq!(hits, vec![n], "selector {t}");
        }
    }

    #[test]
    fn attribute_form() {
        let doc = roxmltree::Document::parse("<svg><rect fill='#fff'/><rect fill='#000'/></svg>").unwrap();
        let sel = Selector::parse("rect[fill=\"#fff\"]").unwrap();
        assert_eq!(doc.descendants().filter(|m| sel.matches(*m)).count(), 1);
        assert!(Selector::parse("rect[").is_err());
    }
}

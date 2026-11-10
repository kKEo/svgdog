//! A small line-based unified diff, bounded so it stays cheap to read.

const CONTEXT: usize = 3;
const MAX_LINES: usize = 400;
const MAX_WIDTH: usize = 400;

pub struct Diff {
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Tag {
    Same,
    Del,
    Add,
}

fn script<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<(Tag, &'a str)> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut out: Vec<(Tag, &str)> = a[..pre].iter().map(|l| (Tag::Same, *l)).collect();
    if ma.len().saturating_mul(mb.len()) > 4_000_000 {
        // Too large to align line by line: show the middle as one replaced block.
        out.extend(ma.iter().map(|l| (Tag::Del, *l)));
        out.extend(mb.iter().map(|l| (Tag::Add, *l)));
    } else {
        // Longest common subsequence over the changed middle.
        let (n, m) = (ma.len(), mb.len());
        let mut lcs = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i * (m + 1) + j] = if ma[i] == mb[j] { lcs[(i + 1) * (m + 1) + j + 1] + 1 } else { lcs[(i + 1) * (m + 1) + j].max(lcs[i * (m + 1) + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if ma[i] == mb[j] {
                out.push((Tag::Same, ma[i]));
                i += 1;
                j += 1;
            } else if lcs[(i + 1) * (m + 1) + j] >= lcs[i * (m + 1) + j + 1] {
                out.push((Tag::Del, ma[i]));
                i += 1;
            } else {
                out.push((Tag::Add, mb[j]));
                j += 1;
            }
        }
        out.extend(ma[i..].iter().map(|l| (Tag::Del, *l)));
        out.extend(mb[j..].iter().map(|l| (Tag::Add, *l)));
    }
    out.extend(a[a.len() - suf..].iter().map(|l| (Tag::Same, *l)));
    out
}

/// Unified diff of two texts; empty when they are equal.
pub fn unified(name: &str, before: &str, after: &str) -> Diff {
    if before == after {
        return Diff { text: String::new(), truncated: false };
    }
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let ops = script(&a, &b);
    let mut text = format!("--- a/{name}\n+++ b/{name}\n");
    let mut truncated = false;
    let mut lines = 0;

    let changed: Vec<usize> = ops.iter().enumerate().filter(|(_, o)| o.0 != Tag::Same).map(|(i, _)| i).collect();
    let mut k = 0;
    // Line numbers before each op index.
    let mut pos = Vec::with_capacity(ops.len() + 1);
    let (mut la, mut lb) = (1usize, 1usize);
    for o in &ops {
        pos.push((la, lb));
        match o.0 {
            Tag::Same => {
                la += 1;
                lb += 1;
            }
            Tag::Del => la += 1,
            Tag::Add => lb += 1,
        }
    }
    while k < changed.len() {
        let start = changed[k].saturating_sub(CONTEXT);
        let mut end = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= end + 2 * CONTEXT + 1 {
            k += 1;
            end = changed[k];
        }
        k += 1;
        let stop = (end + CONTEXT + 1).min(ops.len());
        let hunk = &ops[start..stop];
        let na = hunk.iter().filter(|o| o.0 != Tag::Add).count();
        let nb = hunk.iter().filter(|o| o.0 != Tag::Del).count();
        let (sa, sb) = pos[start];
        text.push_str(&format!("@@ -{},{na} +{},{nb} @@\n", if na == 0 { sa - 1 } else { sa }, if nb == 0 { sb - 1 } else { sb }));
        for (tag, line) in hunk {
            if lines >= MAX_LINES {
                truncated = true;
                break;
            }
            let sign = match tag {
                Tag::Same => ' ',
                Tag::Del => '-',
                Tag::Add => '+',
            };
            text.push(sign);
            if line.chars().count() > MAX_WIDTH {
                text.extend(line.chars().take(MAX_WIDTH));
                text.push('…');
                truncated = true;
            } else {
                text.push_str(line);
            }
            text.push('\n');
            lines += 1;
        }
        if truncated && lines >= MAX_LINES {
            break;
        }
    }
    Diff { text, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_texts_have_no_diff() {
        assert!(unified("a", "x\ny\n", "x\ny\n").text.is_empty());
    }

    #[test]
    fn one_changed_line() {
        let d = unified("f.svg", "a\nb\nc\nd\ne\n", "a\nb\nC\nd\ne\n");
        assert_eq!(d.text, "--- a/f.svg\n+++ b/f.svg\n@@ -1,5 +1,5 @@\n a\n b\n-c\n+C\n d\n e\n");
    }
}

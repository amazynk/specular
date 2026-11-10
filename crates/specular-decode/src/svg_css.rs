//! CSS that usvg 0.48 does not understand, rewritten before parsing.
//!
//! **Custom properties.** usvg has no `var()`: `fill: var(--accent)` falls back to black, and
//! simplecss stops reading a rule at its first `--name: value` declaration, losing the rest of
//! the rule. [`resolve_vars`] takes every custom property declared in the document (in
//! `<style>` rules or `style` attributes), removes the declarations, and replaces each
//! `var(--name, fallback)` with the value. One global scope: the first declaration of a name
//! wins (base definitions come before theme overrides), and declarations inside `@media`
//! blocks are not used (a dark-mode block would otherwise win). A name without a value and
//! without a fallback becomes `inherit`, close to CSS's "invalid at computed-value time"
//! (`unset`) for the inherited paint properties where it matters.
//!
//! **Tailwind's `fill-current` / `stroke-current`.** Icons exported from Tailwind projects
//! carry these classes without the stylesheet that defines them; [`UTILITY_SHEET`] supplies
//! the two rules (`currentColor`), injected below the document's own CSS.

use std::collections::HashMap;

/// Rules for Tailwind's paint utility classes, injected when the document mentions them.
pub(crate) const UTILITY_SHEET: &str =
    ".fill-current{fill:currentColor}.stroke-current{stroke:currentColor}";

/// Whether the document mentions a class [`UTILITY_SHEET`] defines.
pub(crate) fn uses_utilities(text: &[u8]) -> bool {
    memchr::memmem::find(text, b"-current").is_some()
}

/// Deepest chain of custom properties referring to each other (and the guard against cycles).
const MAX_DEPTH: usize = 16;

/// References resolved at least (more for a larger document): values referring to others
/// several times each would otherwise expand exponentially over [`MAX_DEPTH`] levels.
const MAX_EXPANSIONS: usize = 1 << 20;

/// What [`substitute`] may still do: references to resolve, and the output's length.
struct Budget {
    steps: usize,
    out_len: usize,
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

/// Byte ranges of the bodies of `@media` blocks.
fn media_blocks(text: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    // Where the last block ended: one inside it lies within it already (and rescanning from
    // each of many unclosed `@media{` would be quadratic).
    let mut covered = 0;
    for at in memchr::memmem::find_iter(text, b"@media") {
        if at < covered {
            continue;
        }
        let Some(open) = memchr::memchr(b'{', &text[at..]).map(|o| at + o) else {
            break;
        };
        let mut depth = 0usize;
        let mut end = text.len();
        for (i, &b) in text.iter().enumerate().skip(open) {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                // The end of the `<style>` element (CSS cannot contain `<`).
                b'<' => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        out.push((open, end));
        covered = end;
    }
    out
}

/// A custom property declaration: `--name: value;` spans `start..end` (the `;` included).
struct Decl<'a> {
    start: usize,
    end: usize,
    name: &'a str,
    value: &'a str,
}

/// The declaration starting at `at` (the first `-` of `--name`), if there is one there.
fn declaration(text: &str, at: usize) -> Option<Decl<'_>> {
    let b = text.as_bytes();
    // Only where a declaration can start: after `{`, `;`, or a `style` attribute's quote.
    let before = b[..at].iter().rev().find(|c| !c.is_ascii_whitespace());
    if !matches!(before, None | Some(b'{' | b';' | b'"' | b'\'')) {
        return None;
    }
    let name_start = at + 2;
    let name_end = name_start + b[name_start..].iter().take_while(|&&c| is_ident(c)).count();
    if name_end == name_start {
        return None;
    }
    let mut i = name_end;
    while b.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if b.get(i) != Some(&b':') {
        return None;
    }
    let value_start = i + 1;
    let mut depth = 0usize;
    let mut j = value_start;
    while let Some(&c) = b.get(j) {
        match c {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b';' | b'}' | b'"' | b'<' if depth == 0 => break,
            b'"' | b'<' => break,
            _ => {}
        }
        j += 1;
    }
    let mut value = text[value_start..j].trim();
    if let Some(v) = value.strip_suffix("!important") {
        value = v.trim_end();
    }
    let end = if b.get(j) == Some(&b';') { j + 1 } else { j };
    Some(Decl {
        start: at,
        end,
        name: &text[name_start..name_end],
        value,
    })
}

/// A `var(` reference at `at`: the name, the fallback and the end (after the `)`).
fn reference(text: &str, at: usize) -> Option<(&str, Option<&str>, usize)> {
    let b = text.as_bytes();
    let mut i = at + 4;
    while b.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if !text[i..].starts_with("--") {
        return None;
    }
    let name_start = i + 2;
    let name_end = name_start + b[name_start..].iter().take_while(|&&c| is_ident(c)).count();
    let mut j = name_end;
    while b.get(j).is_some_and(u8::is_ascii_whitespace) {
        j += 1;
    }
    match b.get(j) {
        Some(b')') => Some((&text[name_start..name_end], None, j + 1)),
        Some(b',') => {
            let fb_start = j + 1;
            let mut depth = 0usize;
            let mut k = fb_start;
            while let Some(&c) = b.get(k) {
                match c {
                    b'(' => depth += 1,
                    b')' if depth == 0 => {
                        return Some((
                            &text[name_start..name_end],
                            Some(text[fb_start..k].trim()),
                            k + 1,
                        ));
                    }
                    b')' => depth -= 1,
                    b'"' | b'<' | b';' | b'}' => return None,
                    _ => {}
                }
                k += 1;
            }
            None
        }
        _ => None,
    }
}

/// Replaces the `var()` references in `s`; `None` when that goes past the budget.
fn substitute(
    s: &str,
    vars: &HashMap<&str, &str>,
    depth: usize,
    out: &mut String,
    budget: &mut Budget,
) -> Option<()> {
    let mut last = 0;
    let mut pos = 0;
    while let Some(off) = s[pos..].find("var(") {
        let at = pos + off;
        let Some((name, fallback, end)) = reference(s, at) else {
            pos = at + 4;
            continue;
        };
        out.push_str(&s[last..at]);
        budget.steps = budget.steps.checked_sub(1)?;
        if out.len() > budget.out_len {
            return None;
        }
        let value = vars.get(name).copied().or(fallback);
        match value {
            Some(v) if depth < MAX_DEPTH => substitute(v, vars, depth + 1, out, budget)?,
            _ => out.push_str("inherit"),
        }
        last = end;
        pos = end;
    }
    out.push_str(&s[last..]);
    Some(())
}

/// The document with its custom properties resolved, or `None` when it uses none (or they
/// expand past a budget proportional to the document).
pub(crate) fn resolve_vars(text: &str) -> Option<String> {
    let b = text.as_bytes();
    memchr::memmem::find(b, b"var(")?;
    let media = media_blocks(b);
    let mut decls = Vec::new();
    let mut vars: HashMap<&str, &str> = HashMap::new();
    let mut skip_to = 0;
    for at in memchr::memmem::find_iter(b, b"--") {
        if at < skip_to {
            continue;
        }
        let Some(d) = declaration(text, at) else {
            continue;
        };
        skip_to = d.end;
        if !media.iter().any(|&(s, e)| (s..e).contains(&at)) {
            vars.entry(d.name).or_insert(d.value);
        }
        decls.push(d);
    }
    let mut out = String::with_capacity(text.len());
    let mut budget = Budget {
        steps: text.len().saturating_mul(MAX_DEPTH).max(MAX_EXPANSIONS),
        out_len: text.len().saturating_mul(4).max(64 << 20),
    };
    let mut last = 0;
    for d in &decls {
        substitute(&text[last..d.start], &vars, 0, &mut out, &mut budget)?;
        last = d.end;
    }
    substitute(&text[last..], &vars, 0, &mut out, &mut budget)?;
    (out.len() <= budget.out_len).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_resolve_and_declarations_go() {
        let doc = r#"<svg><style>:root{--bg:#123456;--fg: var(--bg) ; fill:red}
@media (prefers-color-scheme: dark){:root{--bg:#000}}
.a{fill:var(--fg)} .b{stroke:var(--missing, #00ff00)} .c{fill:var(--missing)}</style>
<rect style="--w: 4;stroke-width:var(--w)" fill="var(--bg)"/><!-- a comment --></svg>"#;
        let out = resolve_vars(doc).unwrap();
        assert!(out.contains(":root{ fill:red}"), "{out}");
        assert!(out.contains(".a{fill:#123456}"), "{out}");
        assert!(out.contains(".b{stroke:#00ff00}"), "{out}");
        assert!(out.contains(".c{fill:inherit}"), "{out}");
        assert!(out.contains(r#"style="stroke-width:4""#), "{out}");
        assert!(out.contains(r##"fill="#123456""##), "{out}");
        assert!(out.contains("<!-- a comment -->"), "{out}");
        // The @media block's declaration is removed but not used.
        assert!(out.contains(":root{}"), "{out}");
    }

    #[test]
    fn cycles_and_garbage_terminate() {
        let doc = "<style>.x{--a:var(--b);--b:var(--a);fill:var(--a)}</style> var( var(-- var(--";
        let out = resolve_vars(doc).unwrap();
        assert!(out.contains("fill:inherit"), "{out}");
        assert!(resolve_vars("<svg/>").is_none());
        for n in 0..doc.len() {
            if doc.is_char_boundary(n) {
                let _ = resolve_vars(&doc[..n]);
            }
        }
    }
}

/// Synthetic hostile inputs: none may panic, hang or blow up memory.
#[cfg(test)]
mod hostile {
    use super::*;

    /// Sixteen levels of properties each referring to the next 64 times ("billion laughs"),
    /// ending in an empty value or in text: the expansion stops at the budget.
    #[test]
    fn exponential_references_stop() {
        for leaf in ["", "#123456"] {
            let mut doc = String::from("<svg><style>.x{");
            for k in 0..MAX_DEPTH + 2 {
                doc.push_str(&format!("--v{k}:"));
                for _ in 0..64 {
                    doc.push_str(&format!("var(--v{})", k + 1));
                }
                doc.push(';');
            }
            doc.push_str(&format!(
                "--v{}:{leaf};fill:var(--v0)}}</style></svg>",
                MAX_DEPTH + 2
            ));
            let t = std::time::Instant::now();
            let out = resolve_vars(&doc);
            assert!(out.is_none_or(|o| o.len() <= 64 << 20));
            assert!(t.elapsed().as_secs() < 30);
        }
    }

    /// 200 000 unclosed `@media{` (and as many `@media` before one far `{`): each block is
    /// scanned once, not once per opening.
    #[test]
    fn unclosed_media_blocks_scan_once() {
        for unit in ["@media{", "@media "] {
            let mut doc = String::from("<svg><style>");
            doc.push_str(&unit.repeat(200_000));
            doc.push_str("{.a{fill:var(--x)}");
            let t = std::time::Instant::now();
            let blocks = media_blocks(doc.as_bytes());
            assert!(blocks.len() <= 1, "{}", blocks.len());
            let _ = resolve_vars(&doc);
            assert!(t.elapsed().as_secs() < 30);
        }
        // Nested and successive blocks are still found.
        let doc = "@media a{x{}} y{} @media b{z{}}";
        assert_eq!(media_blocks(doc.as_bytes()), [(8, 12), (26, 30)]);
    }
}

//! Limits against hostile input that parsers share. Light (memchr only), so any format crate
//! can use it.
//!
//! # XML nesting
//!
//! roxmltree (used directly, and by usvg for SVG) recurses once per element level while it
//! parses, with no depth limit of its own: a document nested deeply enough overflows the
//! stack before any of our code runs, and a stack overflow ends the whole app. Check
//! [`xml_within_depth`] first.
//!
//! # Conventions for our own recursive parsers
//!
//! * Every recursion a file can drive carries a depth and refuses past a cap (the crate's
//!   "too deeply nested" or "damaged" error), or uses an explicit stack.
//! * References that can loop (offsets, ids, includes) carry a visited or in-progress set;
//!   shared references (a DAG) also need a visit or output budget, or the work is
//!   exponential.
//! * Tests run the hostile input on a small stack (`specular_thread::on_stack`).

/// Element nesting refused by [`xml_within_depth`] users that have no tighter limit of their
/// own. Each level costs roxmltree well under a kilobyte of stack.
pub const MAX_XML_DEPTH: usize = 512;

/// How many levels deep roxmltree expands entity references in each other.
const ENTITY_LEVELS: usize = 10;

/// Whether the elements of `x` nest at most `max` deep, counting what entities declared in
/// an internal DTD subset could add when expanded.
///
/// Comments, CDATA sections and processing instructions are skipped, and a `>` inside a
/// quoted attribute value does not end a tag, so the count is never below the depth the
/// parser reaches (it stops at the first mismatched end tag).
pub fn xml_within_depth(x: &[u8], max: usize) -> bool {
    let (doc, entities) = nesting(x, true);
    doc.saturating_add(entities.saturating_mul(ENTITY_LEVELS)) <= max
}

/// The deepest element nesting of `x`, and the deepest nesting inside a quoted value of its
/// DOCTYPE (entity replacement text) when `dtd`.
fn nesting(x: &[u8], dtd: bool) -> (usize, usize) {
    // The end of the first `pat` at or after `from` (the input's end when there is none).
    let find = |from: usize, pat: &[u8]| {
        x.get(from..)
            .and_then(|t| memchr::memmem::find(t, pat))
            .map_or(x.len(), |p| from + p + pat.len())
    };
    let (mut depth, mut max, mut entities) = (0usize, 0usize, 0usize);
    let mut i = 0;
    while let Some(off) = x.get(i..).and_then(|t| memchr::memchr(b'<', t)) {
        i += off + 1;
        let rest = &x[i..];
        if rest.starts_with(b"!--") {
            i = find(i + 3, b"-->");
        } else if rest.starts_with(b"![CDATA[") {
            i = find(i + 8, b"]]>");
        } else if rest.starts_with(b"?") {
            i = find(i + 1, b"?>");
        } else if rest.starts_with(b"!") {
            // A declaration: the DOCTYPE's internal subset ends at the `>` after its `]`.
            // Comments and processing instructions inside it are skipped (a quote in one
            // must not start a value); quoted values (entity replacement text) are measured.
            let (mut quote, mut bracket, mut j) = (None, 0usize, i + 1);
            let mut value_start = 0;
            while j < x.len() {
                let b = x[j];
                match quote {
                    Some(q) if b == q => {
                        quote = None;
                        if dtd {
                            entities = entities.max(nesting(&x[value_start..j], false).0);
                        }
                    }
                    Some(_) => {}
                    None if x[j..].starts_with(b"<!--") => {
                        j = find(j + 4, b"-->");
                        continue;
                    }
                    None if x[j..].starts_with(b"<?") => {
                        j = find(j + 2, b"?>");
                        continue;
                    }
                    None if b == b'"' || b == b'\'' => {
                        quote = Some(b);
                        value_start = j + 1;
                    }
                    None if b == b'[' => bracket += 1,
                    None if b == b']' => bracket = bracket.saturating_sub(1),
                    None if b == b'>' && bracket == 0 => break,
                    None => {}
                }
                j += 1;
            }
            i = j.min(x.len());
        } else if rest.starts_with(b"/") {
            depth = depth.saturating_sub(1);
            i = find(i + 1, b">");
        } else {
            // A start tag: it ends at the first `>` outside a quoted attribute value.
            let (mut j, mut empty) = (i, false);
            while let Some(k) = memchr::memchr3(b'>', b'"', b'\'', &x[j..]) {
                j += k;
                let b = x[j];
                if b == b'>' {
                    empty = j > i && x[j - 1] == b'/';
                    break;
                }
                // Skip the quoted value.
                j = memchr::memchr(b, &x[j + 1..]).map_or(x.len(), |e| j + 1 + e + 1);
                if j >= x.len() {
                    break;
                }
            }
            if j < x.len() && x[j] != b'>' {
                j = x.len();
            }
            if !empty {
                depth += 1;
                max = max.max(depth);
            }
            i = j.min(x.len());
        }
    }
    (max, entities)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_plain_nesting() {
        assert!(xml_within_depth(b"<a><b/><c></c></a>", 2));
        assert!(!xml_within_depth(b"<a><b><c/></b></a>", 1));
        assert!(xml_within_depth(b"", 0));
        assert!(xml_within_depth(
            b"<?xml version=\"1.0\"?><a>x &lt; y</a>",
            1
        ));
    }

    #[test]
    fn counts_through_tricks() {
        // `/>` inside a quoted value, end tags inside a comment, CDATA or a PI: 3 deep.
        let x = b"<a x=\"/>\"><!-- </a> --><b><![CDATA[</b>]]><?p </b> ?><c y='>'></c></b></a>";
        assert!(!xml_within_depth(x, 2));
        assert!(xml_within_depth(x, 3));
        // A quote inside a comment in the DTD does not hide the elements after it.
        let d = b"<!DOCTYPE r [<!-- don't -->]><r><r><r></r></r></r>";
        assert!(!xml_within_depth(d, 2));
        assert!(xml_within_depth(d, 3));
    }

    #[test]
    fn counts_entities() {
        // Replacement text with markup counts ten times (entities expand in each other).
        let d = b"<!DOCTYPE r [<!ENTITY e \"<a><a/></a>\">]><r>&e;</r>";
        assert!(!xml_within_depth(d, 10));
        assert!(xml_within_depth(d, 11));
    }

    /// Scan speed on a 32 MB SVG-like document:
    /// `cargo test -p specular-limits --release scan_speed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn scan_speed() {
        let doc =
            r#"<g id="a"><path d="M0 0 L10 10 C1 2 3 4 5 6 Z" fill='#123' stroke="none"/></g>"#
                .repeat(400_000);
        let best = (0..5)
            .map(|_| {
                let t = std::time::Instant::now();
                assert!(xml_within_depth(doc.as_bytes(), MAX_XML_DEPTH));
                t.elapsed()
            })
            .min()
            .unwrap();
        println!(
            "{} MB in {best:?}: {:.0} MB/s",
            doc.len() >> 20,
            doc.len() as f64 / best.as_secs_f64() / 1e6
        );
    }

    #[test]
    fn deep_documents_are_refused() {
        let deep = "<a>".repeat(100_000) + &"</a>".repeat(100_000);
        assert!(!xml_within_depth(deep.as_bytes(), MAX_XML_DEPTH));
        let wide = "<a></a>".repeat(100_000);
        assert!(xml_within_depth(wide.as_bytes(), 1));
    }
}

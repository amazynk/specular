//! Greedy CTC decoding of a recogniser's per-step class probabilities, and the turn from
//! the visual (left-to-right) order a line recogniser reads in to the logical order text
//! is stored in for right-to-left scripts.

/// Text of one line and its confidence (mean probability of the kept steps, 0 when empty).
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    pub text: String,
    pub confidence: f32,
    /// Per kept character: the time step it was read at (for word boxes).
    pub steps: Vec<usize>,
}

/// Decodes `probs` (`steps × classes`, row-major): the best class per step, repeats
/// collapsed, blanks (class 0) dropped, classes mapped through `chars` (index 0 the blank).
pub fn greedy(probs: &[f32], classes: usize, chars: &[String]) -> Decoded {
    let mut text = String::new();
    let mut conf = 0f64;
    let mut kept = 0usize;
    let mut steps = Vec::new();
    let mut prev = usize::MAX;
    if classes == 0 {
        return Decoded {
            text,
            confidence: 0.0,
            steps,
        };
    }
    for (t, row) in probs.chunks_exact(classes).enumerate() {
        let (mut best, mut p) = (0usize, f32::MIN);
        for (i, &v) in row.iter().enumerate() {
            if v > p {
                p = v;
                best = i;
            }
        }
        if best != 0
            && best != prev
            && let Some(c) = chars.get(best)
        {
            text.push_str(c);
            conf += f64::from(p);
            kept += 1;
            steps.push(t);
        }
        prev = best;
    }
    Decoded {
        text,
        confidence: if kept == 0 {
            0.0
        } else {
            (conf / kept as f64) as f32
        },
        steps,
    }
}

/// Whether `c` is a strong right-to-left letter (Hebrew, Arabic, Syriac, Thaana, N'Ko and
/// their presentation forms).
/// Arabic-Indic digits are not: numbers keep their order.
pub fn is_rtl(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
        && !matches!(u, 0x0660..=0x0669 | 0x06F0..=0x06F9)
}

/// Whether `c` reads left to right inside right-to-left text: letters of other scripts and
/// digits (European and Arabic-Indic numbers keep their order).
fn keeps_order(c: char) -> bool {
    !is_rtl(c) && c.is_alphanumeric()
}

/// Turns a line read left to right (visual order) into logical order when it holds
/// right-to-left letters: the line reversed, with each run of left-to-right letters and
/// numbers (and the separators inside a number, such as `12.5` or `10:30`) kept in its own
/// order. Mirrored brackets are swapped back. A line without right-to-left letters is
/// returned as it is.
pub fn visual_to_logical(s: &str) -> String {
    if !s.chars().any(is_rtl) {
        return s.to_owned();
    }
    let cs: Vec<char> = s.chars().collect();
    // Split into runs: a left-to-right run is letters/digits plus inner separators that
    // sit between two of them.
    let mut runs: Vec<(bool, Vec<char>)> = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if keeps_order(cs[i]) {
            let mut j = i + 1;
            while j < cs.len() {
                if keeps_order(cs[j]) {
                    j += 1;
                } else if matches!(cs[j], '.' | ',' | ':' | '/' | '-' | '%' | '\'' | '+')
                    && j + 1 < cs.len()
                    && keeps_order(cs[j + 1])
                {
                    j += 2;
                } else {
                    break;
                }
            }
            runs.push((true, cs[i..j].to_vec()));
            i = j;
        } else {
            runs.push((false, vec![cs[i]]));
            i += 1;
        }
    }
    let mut out = String::with_capacity(s.len());
    for (ltr, run) in runs.iter().rev() {
        if *ltr {
            out.extend(run.iter());
        } else {
            out.extend(run.iter().map(|&c| mirror(c)));
        }
    }
    out
}

fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<String> {
        std::iter::once("blank".to_owned())
            .chain(s.chars().map(|c| c.to_string()))
            .collect()
    }

    #[test]
    fn collapses_repeats_and_blanks() {
        let cs = chars("ab");
        // Steps: a a blank a b b
        let rows = [1, 1, 0, 1, 2, 2];
        let mut p = vec![0f32; rows.len() * 3];
        for (t, &c) in rows.iter().enumerate() {
            p[t * 3 + c] = 0.9;
            p[t * 3 + (c + 1) % 3] = 0.1;
        }
        let d = greedy(&p, 3, &cs);
        assert_eq!(d.text, "aab");
        assert!((d.confidence - 0.9).abs() < 1e-6);
        assert_eq!(d.steps, vec![0, 3, 4]);
    }

    #[test]
    fn right_to_left_lines_are_reversed_but_numbers_kept() {
        // Hebrew "shalom" read visually (left to right is the word's end first).
        let visual: String = "םולש".into();
        assert_eq!(visual_to_logical(&visual), "שלום");
        // Visual "2024 (םולש" → logical "שלום) 2024" with the bracket mirrored back.
        assert_eq!(visual_to_logical("2024 (םולש"), "שלום) 2024");
        assert_eq!(visual_to_logical("12.5 ABC םולש"), "שלום ABC 12.5");
        assert_eq!(visual_to_logical("plain text"), "plain text");
    }
}

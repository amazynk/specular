//! Names as Lensfun's documentation says to compare them.
//!
//! Lens models ("Declaring lenses: <lens>", <https://lensfun.github.io/manual/latest/el_lens.html>):
//! case and the order of the words do not matter. A word is a run of characters of one type
//! (digits, punctuation, everything else); words of a single punctuation character are
//! ignored, and so is the word "f". So "smc PENTAX-DA 12-24mm F4 ED AL [IF]" is the words
//! smc, pentax, da, 12, 24, mm, 4, ed, al, if.
//!
//! Cameras ("Declaring cameras: <camera>", <https://lensfun.github.io/manual/latest/el_camera.html>):
//! maker and model exactly as the Exif data has them, case aside.
//!
//! The focal and aperture ranges a lens name states (the same <lens> page): "70-210mm
//! f/4-5.6", "18-55 3.5-5.6", "50 f2" (the "mm" and "f/" are optional), or
//! "2.8-3.5/25-45" (apertures first). A name with "converter" or "extender" in it states
//! the wrong focal length and is not read.

/// The character types of a lens name's words.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Digit,
    Punct,
    Other,
    Space,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_ascii_digit() {
        Class::Digit
    } else if c.is_alphanumeric() {
        Class::Other
    } else {
        Class::Punct
    }
}

/// The words of a lens model, lower case, in order (see the module documentation).
pub fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_class = Class::Space;
    let flush = |cur: &mut String, k: Class, out: &mut Vec<String>| {
        if cur.is_empty() {
            return;
        }
        let single_punct = k == Class::Punct && cur.chars().count() == 1;
        if !single_punct && cur != "f" {
            out.push(std::mem::take(cur));
        }
        cur.clear();
    };
    for c in s.chars() {
        let k = class(c);
        if k != cur_class {
            flush(&mut cur, cur_class, &mut out);
            cur_class = k;
        }
        if k != Class::Space {
            cur.extend(c.to_lowercase());
        }
    }
    flush(&mut cur, cur_class, &mut out);
    out
}

/// A camera maker or model for comparing: lower case, runs of white space as one space,
/// trimmed (Exif strings are often padded).
pub fn plain(s: &str) -> String {
    s.split(|c: char| c.is_whitespace() || c == '\0')
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A number or a range "a-b" at `s[i..]`: the values and the end. `i` must be at a digit.
fn range_at(s: &[u8], i: usize) -> (f64, f64, usize) {
    let number = |mut j: usize| -> (f64, usize) {
        let start = j;
        while j < s.len() && (s[j].is_ascii_digit() || s[j] == b'.') {
            j += 1;
        }
        // A trailing dot is not part of the number ("50mm F1.8.").
        let mut end = j;
        while end > start && s[end - 1] == b'.' {
            end -= 1;
        }
        let v = std::str::from_utf8(&s[start..end])
            .ok()
            .and_then(|t| t.parse::<f64>().ok())
            .unwrap_or(f64::NAN);
        (v, end)
    };
    let (a, mut j) = number(i);
    let mut b = a;
    if j + 1 < s.len() && s[j] == b'-' && s[j + 1].is_ascii_digit() {
        let (v, k) = number(j + 1);
        b = v;
        j = k;
    }
    (a, b, j)
}

/// The focal range a lens model states, in millimetres (see the module documentation).
pub fn focal_of_name(name: &str) -> Option<(f64, f64)> {
    let low = name.to_lowercase();
    if low.contains("converter") || low.contains("extender") {
        return None;
    }
    let s = low.as_bytes();
    let starts = |i: usize| {
        s[i].is_ascii_digit() && (i == 0 || !s[i - 1].is_ascii_digit() && s[i - 1] != b'.')
    };
    let ok = |a: f64, b: f64| a.is_finite() && b.is_finite() && a > 0.0 && b >= a;
    // "70-210mm": a range followed by "mm".
    let mut i = 0;
    while i < s.len() {
        if starts(i) {
            let (a, b, j) = range_at(s, i);
            let mut k = j;
            while k < s.len() && s[k] == b' ' {
                k += 1;
            }
            if s[k..].starts_with(b"mm") && ok(a, b) {
                return Some((a, b));
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    // "2.8-3.5/25-45": apertures, a slash, the focal range.
    let mut i = 0;
    while i < s.len() {
        if starts(i) {
            let (_, _, j) = range_at(s, i);
            if j + 1 < s.len() && s[j] == b'/' && s[j + 1].is_ascii_digit() {
                let (a, b, _) = range_at(s, j + 1);
                if ok(a, b) {
                    return Some((a, b));
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    // "18-55 3.5-5.6", "50 f2": the first number not an aperture ("f2", "f/2", "1:2").
    let mut i = 0;
    while i < s.len() {
        if starts(i) {
            let (a, b, j) = range_at(s, i);
            let before = &s[..i];
            let aperture =
                before.ends_with(b"f") || before.ends_with(b"f/") || before.ends_with(b"1:");
            if !aperture && ok(a, b) {
                return Some((a, b));
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_as_the_docs_split_them() {
        assert_eq!(
            words("smc PENTAX-DA 12-24mm F4 ED AL [IF]"),
            [
                "smc", "pentax", "da", "12", "24", "mm", "4", "ed", "al", "if"
            ]
        );
        // "f/2.8L": "f" and the single punctuation go; 2, 8 and l are words.
        assert_eq!(
            words("EF24-70mm f/2.8L II USM"),
            ["ef", "24", "70", "mm", "2", "8", "l", "ii", "usm"]
        );
        // Longer punctuation runs stay words.
        assert_eq!(words("A -- B"), ["a", "--", "b"]);
        assert!(words("").is_empty());
    }

    #[test]
    fn plain_names() {
        assert_eq!(plain("  NIKON  CORPORATION\0\0"), "nikon corporation");
        assert_eq!(plain("Canon EOS 5D Mark III"), "canon eos 5d mark iii");
    }

    #[test]
    fn focal_ranges_in_names() {
        assert_eq!(focal_of_name("70-210mm f/4-5.6"), Some((70.0, 210.0)));
        assert_eq!(focal_of_name("18-55 3.5-5.6"), Some((18.0, 55.0)));
        assert_eq!(focal_of_name("50 f2"), Some((50.0, 50.0)));
        assert_eq!(focal_of_name("2.8-3.5/25-45"), Some((25.0, 45.0)));
        assert_eq!(focal_of_name("4.5/300"), Some((300.0, 300.0)));
        assert_eq!(
            focal_of_name("Canon EF 24-70mm f/2.8L II USM"),
            Some((24.0, 70.0))
        );
        assert_eq!(
            focal_of_name("smc PENTAX-DA 12-24mm F4 ED AL [IF]"),
            Some((12.0, 24.0))
        );
        assert_eq!(focal_of_name("Sigma 1.4x Extender 70-200mm"), None);
        assert_eq!(focal_of_name("Fisheye f/2.8"), None);
    }
}

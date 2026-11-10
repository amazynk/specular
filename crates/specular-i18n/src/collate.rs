//! Natural ("human") ordering of file names: digit runs compare by value, text runs with the
//! locale's collator at primary strength (case- and accent-insensitive), so `img2` < `img10`
//! and `Äpfel` sorts next to `Apfel` in German.

use std::cmp::Ordering;
use std::collections::HashMap;

use icu_collator::CollatorBorrowed;

/// Value of `c` if it is a decimal digit in a common script (ASCII, fullwidth, Arabic-Indic,
/// Extended Arabic-Indic, Devanagari, Bengali, Thai).
fn digit(c: char) -> Option<u32> {
    const ZEROS: [u32; 7] = [0x30, 0xFF10, 0x0660, 0x06F0, 0x0966, 0x09E6, 0x0E50];
    let u = c as u32;
    ZEROS
        .iter()
        .find(|&&z| (z..z + 10).contains(&u))
        .map(|&z| u - z)
}

enum Run<'a> {
    Text(&'a str),
    /// A run of digits (compared by value: leading zeros ignored).
    Number(&'a str),
}

/// Splits a name into alternating text and digit runs, without allocating (sorting a large
/// folder compares names millions of times).
struct Runs<'a>(&'a str);

impl<'a> Iterator for Runs<'a> {
    type Item = Run<'a>;

    fn next(&mut self) -> Option<Run<'a>> {
        let mut it = self.0.char_indices();
        let (_, c) = it.next()?;
        let d = digit(c).is_some();
        let end = it
            .find(|&(_, c)| digit(c).is_some() != d)
            .map_or(self.0.len(), |(i, _)| i);
        let (run, rest) = self.0.split_at(end);
        self.0 = rest;
        Some(if d { Run::Number(run) } else { Run::Text(run) })
    }
}

/// The digit values of a run, leading zeros dropped.
fn value(s: &str) -> impl Iterator<Item = u32> + Clone + '_ {
    let v = s.chars().filter_map(digit);
    let lead = v.clone().take_while(|&d| d == 0).count();
    v.skip(lead)
}

fn cmp_numbers(a: &str, b: &str) -> Ordering {
    let (va, vb) = (value(a), value(b));
    va.clone()
        .count()
        .cmp(&vb.clone().count())
        .then_with(|| va.cmp(vb))
}

/// Natural comparison with explicit collators (`primary` for runs, `full` as tie-break).
pub(crate) fn natural_cmp_with(
    primary: &CollatorBorrowed<'_>,
    full: &CollatorBorrowed<'_>,
    a: &str,
    b: &str,
) -> Ordering {
    let (mut ra, mut rb) = (Runs(a), Runs(b));
    loop {
        let o = match (ra.next(), rb.next()) {
            (Some(Run::Number(x)), Some(Run::Number(y))) => cmp_numbers(x, y),
            // Equal runs (the "IMG_" in every camera name) need no collation.
            (Some(Run::Text(x)), Some(Run::Text(y))) if x == y => Ordering::Equal,
            (Some(Run::Text(x)), Some(Run::Text(y))) => primary.compare(x, y),
            // Numbers sort before text, as in file managers ("1.jpg" before "a.jpg").
            (Some(Run::Number(_)), Some(Run::Text(_))) => Ordering::Less,
            (Some(Run::Text(_)), Some(Run::Number(_))) => Ordering::Greater,
            // Fewer runs first.
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => break,
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    full.compare(a, b).then_with(|| a.cmp(b))
}

/// A precomputed natural sort key: comparing two keys' bytes gives the run comparison of
/// [`natural_cmp_with`], so sorting a large folder compares bytes instead of collating text
/// millions of times. Keys that compare equal (names that differ only in case, accents or
/// leading zeros) are ordered by [`NaturalKey::cmp_names`]'s full-strength tie-break, exactly
/// like [`natural_cmp_with`].
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NaturalKey(Box<[u8]>);

/// Run tags of a key. The end sorts before any run ("fewer runs first") and numbers before
/// text, as in [`natural_cmp_with`].
const KEY_END: u8 = 0;
const KEY_NUMBER: u8 = 1;
const KEY_TEXT: u8 = 2;

impl NaturalKey {
    /// The key's bytes (for tests and diagnostics).
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Natural order of two names given their keys: the keys, then (rarely, on equal keys)
    /// the full-strength collation and the raw text, as [`natural_cmp_with`] does.
    pub fn cmp_names(
        full: &CollatorBorrowed<'_>,
        (ka, a): (&NaturalKey, &str),
        (kb, b): (&NaturalKey, &str),
    ) -> Ordering {
        ka.0.cmp(&kb.0)
            .then_with(|| full.compare(a, b).then_with(|| a.cmp(b)))
    }
}

/// Makes [`NaturalKey`]s with one collator, remembering the keys of text runs (the "IMG_" and
/// ".jpg" of a camera folder are collated once, not once per file).
pub struct NaturalKeyer<'c> {
    primary: &'c CollatorBorrowed<'static>,
    runs: HashMap<Box<str>, Box<[u8]>>,
    scratch: Vec<u8>,
}

/// Text runs remembered at most (the cache is cleared when it is full).
const RUN_CACHE: usize = 4096;

impl<'c> NaturalKeyer<'c> {
    pub fn new(primary: &'c CollatorBorrowed<'static>) -> NaturalKeyer<'c> {
        NaturalKeyer {
            primary,
            runs: HashMap::new(),
            scratch: Vec::with_capacity(64),
        }
    }

    pub fn key(&mut self, name: &str) -> NaturalKey {
        let mut out = Vec::with_capacity(name.len() + 8);
        for run in Runs(name) {
            match run {
                Run::Number(s) => {
                    // Count of significant digits, then the digits: equal counts compare
                    // digit by digit, as `cmp_numbers` does.
                    let digits = value(s);
                    let n = digits.clone().count() as u32;
                    out.push(KEY_NUMBER);
                    out.extend_from_slice(&n.to_be_bytes());
                    out.extend(digits.map(|d| d as u8));
                }
                Run::Text(s) => {
                    out.push(KEY_TEXT);
                    match self.runs.get(s) {
                        Some(k) => out.extend_from_slice(k),
                        None => {
                            self.scratch.clear();
                            let Ok(()) = self.primary.write_sort_key_to(s, &mut self.scratch);
                            let k = escape(&self.scratch);
                            out.extend_from_slice(&k);
                            if self.runs.len() >= RUN_CACHE {
                                self.runs.clear();
                            }
                            self.runs.insert(s.into(), k);
                        }
                    }
                }
            }
        }
        out.push(KEY_END);
        NaturalKey(out.into_boxed_slice())
    }
}

/// A collation key made prefix-free and order-preserving: 0 and 1 become 1 1 and 1 2, and a 0
/// ends it, so a run that is a prefix of another sorts first whatever follows it.
fn escape(k: &[u8]) -> Box<[u8]> {
    let mut out = Vec::with_capacity(k.len() + 1);
    for &b in k {
        match b {
            0 => out.extend_from_slice(&[1, 1]),
            1 => out.extend_from_slice(&[1, 2]),
            b => out.push(b),
        }
    }
    out.push(0);
    out.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_split_text_and_digits() {
        let r: Vec<(bool, &str)> = Runs("img_007b12")
            .map(|r| match r {
                Run::Text(s) => (false, s),
                Run::Number(s) => (true, s),
            })
            .collect();
        assert_eq!(
            r,
            [(false, "img_"), (true, "007"), (false, "b"), (true, "12")]
        );
        assert_eq!(Runs("").count(), 0);
    }

    #[test]
    fn numbers_by_value() {
        assert_eq!(cmp_numbers("007", "7"), Ordering::Equal);
        assert_eq!(cmp_numbers("9", "10"), Ordering::Less);
        assert_eq!(cmp_numbers("0", "000"), Ordering::Equal);
        assert_eq!(cmp_numbers("٣", "2"), Ordering::Greater);
    }

    fn collators(locale: &str) -> (CollatorBorrowed<'static>, CollatorBorrowed<'static>) {
        use icu_collator::Collator;
        use icu_collator::options::{CollatorOptions, Strength};
        let make = |strength| {
            let mut o = CollatorOptions::default();
            o.strength = Some(strength);
            let l: icu_locale_core::Locale = locale.parse().unwrap();
            Collator::try_new((&l).into(), o).unwrap()
        };
        (make(Strength::Primary), make(Strength::Tertiary))
    }

    /// Sorting by keys gives exactly the comparator's order, pair by pair.
    #[test]
    fn keys_order_like_the_comparator() {
        #[rustfmt::skip]
        let names = [
            "", "a", "A", "á", "Äpfel", "apfel", "Apfel.jpg", "apfel.jpg", "Zebra", "zebra",
            "img2.jpg", "img10.jpg", "IMG2.jpg", "img002.jpg", "img02.jpg", "img0.jpg", "img.jpg",
            "img_1", "img-1", "img 1", "img1a", "img1", "1.jpg", "01.jpg", "10.jpg", "a1b2",
            "a1b10", "a01b2", "_x", ".hidden", "x", "x1", "x1y", "ß", "ss", "SS", "æ", "ae",
            "Ångström", "angstrom", "ö", "o", "z", "Ø", "صورة٢", "صورة١٠", "صورة2", "写真10",
            "写真9", "写真", "ｉｍｇ１２", "img१२", "日本", "emoji😀1", "emoji😀10", "e\u{301}",
            "é", "E", "IMG_0001.CR3", "IMG_0001.JPG", "IMG_0001.jpg", "IMG_1.jpg", "IMG_0010.jpg",
            "999999999999999999999", "1000000000000000000000", "0", "00", "000a", "a\u{0}b",
            "a\u{1}b", "ab", "a b", "a\tb", "tab\t", "Æble", "Øl", "Åbo", "zz", "Ǆ", "ǆ",
        ];
        for locale in ["en", "de", "sv", "da", "ar", "ja", "tr"] {
            let (p, f) = collators(locale);
            let mut keyer = NaturalKeyer::new(&p);
            let keys: Vec<NaturalKey> = names.iter().map(|n| keyer.key(n)).collect();
            for (i, a) in names.iter().enumerate() {
                for (j, b) in names.iter().enumerate() {
                    let want = natural_cmp_with(&p, &f, a, b);
                    let got = NaturalKey::cmp_names(&f, (&keys[i], a), (&keys[j], b));
                    assert_eq!(got, want, "{locale}: {a:?} vs {b:?}");
                }
            }
            // The run cache gives the same keys as a fresh keyer.
            let mut fresh = NaturalKeyer::new(&p);
            for (n, k) in names.iter().zip(&keys) {
                assert_eq!(&fresh.key(n), k);
            }
        }
    }

    #[test]
    fn escaped_keys_are_prefix_free_and_ordered() {
        let v: [&[u8]; 6] = [&[], &[0], &[0, 0], &[1], &[1, 5], &[2]];
        for a in v {
            for b in v {
                assert_eq!(escape(a).cmp(&escape(b)), a.cmp(b), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn digits() {
        assert_eq!(digit('7'), Some(7));
        assert_eq!(digit('٣'), Some(3));
        assert_eq!(digit('５'), Some(5));
        assert_eq!(digit('a'), None);
    }
}

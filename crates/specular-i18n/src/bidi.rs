//! Text direction and Unicode bidi isolation helpers.
//!
//! The UI never draws the control characters used here. They are passed to the text shaper
//! (`parley`), which applies the Unicode Bidirectional Algorithm (UAX #9) and gives them no
//! glyphs. See the crate docs for the integration rules.

use unic_langid::LanguageIdentifier;

/// First Strong Isolate: starts an isolate whose direction comes from its first strong character.
pub const FSI: char = '\u{2068}';
/// Left-to-Right Isolate.
pub const LRI: char = '\u{2066}';
/// Right-to-Left Isolate.
pub const RLI: char = '\u{2067}';
/// Pop Directional Isolate: ends an FSI/LRI/RLI.
pub const PDI: char = '\u{2069}';
/// Right-to-Left Override (used only by the `ar-XB` pseudo-locale).
pub const RLO: char = '\u{202E}';
/// Pop Directional Formatting: ends an override or embedding.
pub const PDF: char = '\u{202C}';
/// Right-to-Left Mark: a zero-width strong RTL character.
pub const RLM: char = '\u{200F}';
/// Left-to-Right Mark.
pub const LRM: char = '\u{200E}';
/// Arabic Letter Mark.
const ALM: char = '\u{061C}';

/// Layout direction of a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

impl Direction {
    pub fn is_rtl(self) -> bool {
        self == Direction::Rtl
    }
}

/// Languages written right to left (by language subtag) when no script is given.
const RTL_LANGUAGES: &[&str] = &[
    "ar", "arc", "ckb", "dv", "fa", "he", "iw", "ks", "lrc", "mzn", "nqo", "pnb", "ps", "sd",
    "syr", "ug", "ur", "yi",
];

/// Scripts written right to left.
const RTL_SCRIPTS: &[&str] = &[
    "Adlm", "Arab", "Hebr", "Mand", "Mend", "Nkoo", "Rohg", "Samr", "Syrc", "Thaa", "Yezi",
];

/// Direction for a language: from its script subtag when present (`az-Arab` is RTL,
/// `ku-Latn` LTR), otherwise from the language (`ar`, `he`, `fa`, `ur`…). The `ar-XB`
/// pseudo-locale is RTL.
pub fn direction(langid: &LanguageIdentifier) -> Direction {
    if let Some(script) = langid.script {
        return if RTL_SCRIPTS.contains(&script.as_str()) {
            Direction::Rtl
        } else {
            Direction::Ltr
        };
    }
    if crate::pseudo::Pseudo::of(langid) == Some(crate::pseudo::Pseudo::Bidi) {
        return Direction::Rtl;
    }
    if RTL_LANGUAGES.contains(&langid.language.as_str()) {
        Direction::Rtl
    } else {
        Direction::Ltr
    }
}

/// Wraps `s` in FSI…PDI so it keeps its own direction inside surrounding text
/// (a Latin file name in an Arabic sentence, an Arabic city in an English one).
pub fn isolate(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 6);
    out.push(FSI);
    out.push_str(s);
    out.push(PDI);
    out
}

/// Wraps `s` in LRI…PDI: for content that is always left to right (paths, URLs, numbers with
/// units, key combinations), even when it starts with an RTL character.
pub fn isolate_ltr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 6);
    out.push(LRI);
    out.push_str(s);
    out.push(PDI);
    out
}

/// Removes isolate and override controls (FSI, LRI, RLI, PDI, RLO, PDF) and directional
/// marks (LRM, RLM, ALM), for plain-text output such as logs, the clipboard or tests.
pub fn strip_isolates(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(*c, FSI | LRI | RLI | PDI | RLO | PDF | LRM | RLM | ALM))
        .collect()
}

/// A letter of a right-to-left script (Hebrew, Arabic, Syriac, Thaana, N'Ko, and their
/// presentation forms and supplementary-plane relatives).
pub fn is_rtl_letter(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
        && c.is_alphabetic()
}

/// Replaces every FSI with RLI when its content, nested isolates included, holds a
/// right-to-left letter, and with LRI otherwise.
///
/// The first-strong rule of FSI looks only at the isolate's own level and skips nested
/// isolates. Fluent nests them when a message's argument is itself a formatted message (the
/// date and the time inside "date, time"), so an outer FSI that holds only nested isolates and
/// punctuation resolves to left-to-right and an Arabic or Hebrew reader sees the parts in the
/// wrong order. Unpaired FSIs are left alone.
pub fn resolve_fsi(s: &str) -> String {
    if !s.contains(FSI) {
        return s.to_owned();
    }
    let mut chars: Vec<char> = s.chars().collect();
    for i in 0..chars.len() {
        if chars[i] != FSI {
            continue;
        }
        let mut depth = 0u32;
        let mut rtl = false;
        let mut closed = false;
        for &c in &chars[i..] {
            match c {
                FSI | LRI | RLI => depth += 1,
                PDI => {
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                        break;
                    }
                }
                c if is_rtl_letter(c) => rtl = true,
                _ => {}
            }
        }
        if closed {
            chars[i] = if rtl { RLI } else { LRI };
        }
    }
    chars.into_iter().collect()
}

/// Direction of the first strong character outside isolates (UAX #9 rules P2–P3, which is
/// how `parley` picks a paragraph's direction); `None` when there is none.
pub fn base_direction(s: &str) -> Option<Direction> {
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            FSI | LRI | RLI => depth += 1,
            PDI => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            RLM | ALM => return Some(Direction::Rtl),
            LRM => return Some(Direction::Ltr),
            c if is_rtl_letter(c) => return Some(Direction::Rtl),
            c if c.is_alphabetic() => return Some(Direction::Ltr),
            _ => {}
        }
    }
    None
}

/// A character of a unit or symbol written after a number: Latin and Greek letters
/// (`px`, `ms`, `σ`), `µ`, `Å`, `Ω`, degrees, primes, per cent and mille, superscripts, `/`.
fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic()
        || matches!(c as u32, 0x0370..=0x03FF)
        || matches!(
            c,
            'µ' | 'Å' | 'Ω' | '°' | '′' | '″' | '%' | '‰' | '²' | '³' | '/'
        )
}

/// Moves the end of an isolate without right-to-left letters past the unit written after it,
/// so `FSI 2 PDI σ` becomes `FSI 2σ PDI` and `FSI 5 PDI px` becomes `FSI 5 px PDI`.
///
/// Fluent isolates each argument, so in a right-to-left sentence a unit outside the isolate
/// is laid out on its own and lands on the wrong side of the number (`σ2`, `px 5`, `%50`).
/// A unit is a run of [`is_unit_char`] after at most one space that does not run on into
/// other letters or digits (`/דקה` is left alone).
pub fn attach_units(s: &str) -> String {
    if !s.contains(PDI) {
        return s.to_owned();
    }
    let mut chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != PDI {
            i += 1;
            continue;
        }
        // The isolate this PDI closes: back to its initiator, nested isolates skipped.
        let mut depth = 0u32;
        let mut start = None;
        for j in (0..i).rev() {
            match chars[j] {
                PDI => depth += 1,
                FSI | LRI | RLI if depth == 0 => {
                    start = Some(j);
                    break;
                }
                FSI | LRI | RLI => depth -= 1,
                _ => {}
            }
        }
        let ltr = start.is_some_and(|st| !chars[st..i].iter().any(|&c| is_rtl_letter(c)));
        let mut end = i + 1;
        if chars
            .get(end)
            .is_some_and(|&c| matches!(c, ' ' | '\u{a0}' | '\u{2009}' | '\u{202f}'))
        {
            end += 1;
        }
        let unit_start = end;
        // A unit that is an argument of its own ("{ $value } { $unit }"): the two isolates
        // become one when the first is a number and the second holds only unit characters.
        if ltr
            && start.is_some_and(|st| number_end(&chars, st) == Some(i + 1))
            && chars.get(unit_start) == Some(&FSI)
            && let Some(len) = chars[unit_start + 1..].iter().position(|&c| c == PDI)
            && len > 0
            && chars[unit_start + 1..unit_start + 1 + len]
                .iter()
                .all(|&c| is_unit_char(c) || c == ' ')
            && chars[unit_start + 1] != '/'
        {
            chars.remove(unit_start);
            chars.remove(i);
            i = unit_start + len - 1;
            continue;
        }
        // A unit never starts with `/` ("IMG.jpg / 3" is a separator).
        while chars
            .get(end)
            .is_some_and(|&c| is_unit_char(c) && (end > unit_start || c != '/'))
        {
            end += 1;
        }
        let runs_on = chars.get(end).is_some_and(|&c| c.is_alphanumeric());
        if ltr && end > unit_start && !runs_on {
            chars.remove(i);
            chars.insert(end - 1, PDI);
            i = end;
        } else {
            i += 1;
        }
    }
    chars.into_iter().collect()
}

/// A sign written before a number: plus, minus (hyphen and U+2212), plus-minus, about,
/// less/greater than.
fn is_sign_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '−' | '±' | '∓' | '~' | '≈' | '<' | '>' | '≤' | '≥'
    )
}

/// A mark written straight before a number that reads with it: number sign `#` and numero
/// `№` ("Person #21").
fn is_number_prefix(c: char) -> bool {
    matches!(c, '#' | '№')
}

/// Moves a sign written before a number isolate into it, so `±FSI 5 PDI` becomes
/// `FSI ±5 PDI` and `(+FSI 3 PDI)` becomes `(FSI +3 PDI)`. A number prefix touching the
/// isolate moves too (`#FSI 21 PDI` becomes `FSI #21 PDI`): outside, in a right-to-left
/// sentence, it lands on the right of the number (`21#`).
///
/// Signs are neutral (or number separators) in UAX #9: outside the isolate, in a right-to-left
/// sentence, they take its direction and land on the right of the number (`0.5±`, `(3+)`). The
/// sign must touch the isolate, and must not follow a letter or digit (Hebrew `ב-` before a
/// number is a prefix, `a-5` a range). A sign followed by one space moves only at the start
/// of the text or after an opening bracket (`± 0.5 · …`), so a binary `median + 2σ` is left
/// alone.
pub fn attach_signs(s: &str) -> String {
    if !s.chars().any(|c| is_sign_char(c) || is_number_prefix(c)) {
        return s.to_owned();
    }
    let mut chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let spaced = chars.get(i + 1).is_some_and(|&c| is_gap(c));
        if !is_sign_char(c) && !(is_number_prefix(c) && !spaced) {
            i += 1;
            continue;
        }
        let at = if spaced { i + 2 } else { i + 1 };
        let before = i.checked_sub(1).map(|j| chars[j]);
        let free = match before {
            None => true,
            Some(b) if spaced => matches!(b, '(' | '[' | '{' | RLM | LRM),
            Some(b) => !b.is_alphanumeric() && !is_sign_char(b) && b != PDI,
        };
        let number = free
            && chars.get(at).is_some_and(|&c| c == FSI || c == LRI)
            && number_end(&chars, at).is_some();
        if number {
            // `±`, ` ` → after the initiator.
            let moved: Vec<char> = chars.drain(i..at).collect();
            let ini = i + 1;
            for (k, m) in moved.into_iter().enumerate() {
                chars.insert(ini + k, m);
            }
        }
        i += 1;
    }
    chars.into_iter().collect()
}

/// A separator between numbers that reads left to right with them: dimensions (`×`), parts
/// (`·`), ranges (`–`, `—`), ratios and fractions (`/`, `:`, `∶`), a value and its spread
/// (`±`).
fn is_number_joiner(c: char) -> bool {
    matches!(c, '×' | '·' | '–' | '—' | '/' | ':' | '∶' | '±')
}

fn is_gap(c: char) -> bool {
    matches!(c, ' ' | '\u{a0}' | '\u{2009}' | '\u{202f}')
}

/// The end (exclusive) of a number at `i`: an isolate without right-to-left letters whose
/// text starts with a digit (or a sign and a digit), or bare digits with their group and
/// decimal marks.
fn number_end(chars: &[char], i: usize) -> Option<usize> {
    let starts_number = |rest: &[char]| {
        let mut it = rest
            .iter()
            .copied()
            .filter(|&c| !matches!(c, FSI | LRI | RLI | PDI | LRM));
        match it.next() {
            Some(c) if c.is_ascii_digit() => true,
            Some(c) if is_sign_char(c) => it.next().is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        }
    };
    match *chars.get(i)? {
        FSI | LRI => {
            let mut depth = 0u32;
            for (j, &c) in chars.iter().enumerate().skip(i) {
                match c {
                    FSI | LRI | RLI => depth += 1,
                    PDI => {
                        depth -= 1;
                        if depth == 0 {
                            let inner = &chars[i + 1..j];
                            let ok = !inner.iter().any(|&c| is_rtl_letter(c) || c == RLI)
                                && starts_number(inner);
                            return ok.then_some(j + 1);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        c if c.is_ascii_digit() => {
            let mut j = i;
            while chars.get(j).is_some_and(|&c| {
                c.is_ascii_digit()
                    || (matches!(c, ',' | '.')
                        && chars.get(j + 1).is_some_and(char::is_ascii_digit))
            }) {
                j += 1;
            }
            // Part of a word ("H264"): not a number of its own.
            let glued = i > 0 && chars[i - 1].is_alphanumeric();
            (!glued).then_some(j)
        }
        _ => None,
    }
}

/// The end of a joiner between two numbers at `i`: spaces, one [`is_number_joiner`],
/// spaces. A middle dot joins only in text without right-to-left letters ("1 · 24,576 ×
/// 17,920"): in a sentence it separates parts ("when · how long since the start").
fn joiner_end(chars: &[char], i: usize, dot: bool) -> Option<usize> {
    let mut j = i;
    while chars.get(j).is_some_and(|&c| is_gap(c)) {
        j += 1;
    }
    if !chars
        .get(j)
        .is_some_and(|&c| is_number_joiner(c) && (dot || c != '·'))
    {
        return None;
    }
    j += 1;
    while chars.get(j).is_some_and(|&c| is_gap(c)) {
        j += 1;
    }
    Some(j)
}

/// Wraps numbers joined by neutral separators in one left-to-right isolate, so `FSI 1 PDI ·
/// FSI 24,576 PDI × FSI 17,920 PDI` keeps its order in a right-to-left paragraph instead of
/// reading "17,920 × 24,576 · 1". Covers dimensions (`96 × 112`), ranges (`1–5`), ratios
/// and fractions (`1:50`, `3 / 12`) and times (`1:30:05`), with or without Fluent's isolates.
pub fn keep_number_runs_ltr(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if !chars.iter().any(|&c| is_number_joiner(c)) {
        return s.to_owned();
    }
    let dot = !chars.iter().any(|&c| is_rtl_letter(c));
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let Some(mut end) = number_end(&chars, i) else {
            out.push(chars[i]);
            i += 1;
            continue;
        };
        let mut numbers = 1;
        // Bare digits joined by an unspaced `:` or `/` (common separators) already stay in
        // order (UAX #9 W4): "7:05", "1/250".
        let mut needed = chars[i] == FSI || chars[i] == LRI;
        while let Some(j) = joiner_end(&chars, end, dot)
            && let Some(k) = number_end(&chars, j)
        {
            needed |= j - end > 1
                || !matches!(chars[end], ':' | '/')
                || chars[j] == FSI
                || chars[j] == LRI;
            end = k;
            numbers += 1;
        }
        if numbers > 1 && needed {
            out.push(LRI);
        }
        out.extend(&chars[i..end]);
        if numbers > 1 && needed {
            out.push(PDI);
        }
        i = end;
    }
    out
}

/// Prepares text of a right-to-left UI for the shaper: keeps units and signs with their
/// numbers (see [`attach_units`] and [`attach_signs`]), resolves nested FSIs (see [`resolve_fsi`]), keeps numbers joined by
/// neutral separators in order (see [`keep_number_runs_ltr`]), and when the text holds
/// right-to-left letters but would not start a right-to-left paragraph (it begins with an
/// isolate, a number or a Latin word), starts it with an RLM. Text without right-to-left
/// letters is only resolved.
pub fn for_rtl(s: &str) -> String {
    let mut out = keep_number_runs_ltr(&resolve_fsi(&attach_signs(&attach_units(s))));
    if out.chars().any(is_rtl_letter) && base_direction(&out) != Some(Direction::Rtl) {
        out.insert(0, RLM);
    }
    out
}

/// Joins the parts of one line ("name — details", "A · B") for a UI written in `dir`.
///
/// Left to right: the plain join, with only parts holding right-to-left letters isolated
/// (FSI…PDI). Right to left: each part is wrapped in
/// FSI…PDI (unless it already is one isolate) and the line starts with an RLM when it would
/// not start a right-to-left paragraph. A part then keeps its own order (a file name, a value
/// such as `100 ±0.2`, a frame such as `⌀0.25 | A | B`) and the parts read right to left,
/// the first on the right, even when the first is a Latin file name. Empty parts are left
/// out.
pub fn join_parts(parts: &[&str], sep: &str, dir: Direction) -> String {
    let parts = parts.iter().filter(|p| !p.is_empty());
    if !dir.is_rtl() {
        // A right-to-left part is isolated so a number after it ("תמונה 2 · 4000") does
        // not join its run and swap places with it.
        let mut out = String::new();
        for (i, p) in parts.enumerate() {
            if i > 0 {
                out.push_str(sep);
            }
            if p.chars().any(is_rtl_letter) && !is_one_isolate(p) {
                out.push(FSI);
                out.push_str(p);
                out.push(PDI);
            } else {
                out.push_str(p);
            }
        }
        return out;
    }
    let mut out = String::new();
    for (i, p) in parts.enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        if is_one_isolate(p) {
            out.push_str(p);
        } else {
            out.push(FSI);
            out.push_str(p);
            out.push(PDI);
        }
    }
    if !out.is_empty() && base_direction(&out) != Some(Direction::Rtl) {
        out.insert(0, RLM);
    }
    out
}

/// Joins a label and a text written by someone ("author: comment") for a UI written in
/// `dir`, the line taking the direction of the text's first strong character.
///
/// Left to right: the plain join, unchanged. Right to left: both parts are isolated and the
/// line starts with an RLM, or with an LRM when the text is left to right, so a Latin
/// comment reads "dana: The revision." while a Hebrew one reads from its author on the
/// right. Text without strong characters follows `dir`. Empty parts are left out.
pub fn join_labelled(label: &str, text: &str, sep: &str, dir: Direction) -> String {
    let line = join_parts(&[label, text], sep, dir);
    if !dir.is_rtl() || base_direction(text) != Some(Direction::Ltr) {
        return line;
    }
    let mut out = String::with_capacity(line.len() + 3);
    out.push(LRM);
    out.push_str(line.strip_prefix(RLM).unwrap_or(&line));
    out
}

/// True when `s` is one isolate from start to end (`FSI a PDI`, not `FSI a PDI b FSI c PDI`).
fn is_one_isolate(s: &str) -> bool {
    if !s.starts_with([FSI, LRI, RLI]) || !s.ends_with(PDI) {
        return false;
    }
    let mut depth = 0u32;
    let n = s.chars().count();
    for (i, c) in s.chars().enumerate() {
        match c {
            FSI | LRI | RLI => depth += 1,
            PDI => {
                depth = depth.saturating_sub(1);
                if depth == 0 && i + 1 < n {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// `s` cut to at most `n` visible characters, the last of them `…`, with isolates kept
/// balanced: bidi controls are not counted, and an isolate (or override) cut open is closed
/// after the ellipsis. Text that fits is returned unchanged.
pub fn truncate(s: &str, n: usize) -> String {
    let is_control = |c: char| matches!(c, FSI | LRI | RLI | PDI | RLO | PDF | LRM | RLM | ALM);
    if s.chars().filter(|&c| !is_control(c)).count() <= n {
        return s.to_owned();
    }
    let keep = n.saturating_sub(1);
    let mut out = String::with_capacity(s.len());
    // Isolates and overrides open at the cut, innermost last.
    let mut open = Vec::new();
    let mut seen = 0usize;
    for c in s.chars() {
        if seen == keep {
            break;
        }
        match c {
            FSI | LRI | RLI | RLO => open.push(c),
            PDI | PDF => {
                open.pop();
            }
            _ if is_control(c) => {}
            _ => seen += 1,
        }
        out.push(c);
    }
    out.push('…');
    for c in open.into_iter().rev() {
        out.push(if c == RLO { PDF } else { PDI });
    }
    out
}

/// `s` with its isolates and overrides made whole: one cut open (an FSI, LRI, RLI or RLO
/// without its end) is closed at the end, innermost first, and a PDI or PDF without its
/// opener is dropped. For text shortened anywhere (an ellipsis in the middle of a name).
pub fn balance_isolates(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 6);
    let mut open = Vec::new();
    for c in s.chars() {
        match c {
            FSI | LRI | RLI | RLO => open.push(c),
            PDI | PDF => {
                // The closer must match the innermost opener; one that does not is dropped.
                let want = if c == PDF {
                    open.last() == Some(&RLO)
                } else {
                    matches!(open.last(), Some(&(FSI | LRI | RLI)))
                };
                if !want {
                    continue;
                }
                open.pop();
            }
            _ => {}
        }
        out.push(c);
    }
    for c in open.into_iter().rev() {
        out.push(if c == RLO { PDF } else { PDI });
    }
    out
}

/// `chars` shortened to `keep` characters plus "…": the start kept, or (`middle`) both ends
/// (a file name keeps its extension). Isolates stay whole: one cut open is closed after the
/// ellipsis; in a middle cut, one open at both cuts holds the ellipsis, and one whose start
/// was cut away opens again before the kept end (see [`balance_isolates`]).
pub fn ellipsis_cut(chars: &[char], keep: usize, middle: bool) -> String {
    let keep = keep.min(chars.len());
    if middle {
        let head = keep.div_ceil(2);
        let start = chars.len() - keep / 2;
        let (at_head, at_tail) = (
            open_isolates(&chars[..head]),
            open_isolates(&chars[..start]),
        );
        // The isolates open at both cuts (the same ones, by position) stay open.
        let common = at_head
            .iter()
            .zip(&at_tail)
            .take_while(|(a, b)| a == b)
            .count();
        let mut o: String = chars[..head].iter().collect();
        o.push('…');
        o.extend(std::iter::repeat_n(PDI, at_head.len() - common));
        o.extend(at_tail[common..].iter().map(|&(_, c)| c));
        o.extend(&chars[start..]);
        balance_isolates(&o)
    } else {
        let mut o: String = chars[..keep]
            .iter()
            .collect::<String>()
            .trim_end()
            .to_owned();
        o.push('…');
        balance_isolates(&o)
    }
}

/// The longest form of `s` (whole, or cut by [`ellipsis_cut`]) that `fits` accepts; the
/// bare "…" when nothing does. `fits` is asked O(log n) times (a binary search on the
/// characters kept), so it may measure shaped text.
pub fn ellipsize_to_fit(s: &str, middle: bool, mut fits: impl FnMut(&str) -> bool) -> String {
    if fits(s) {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if fits(&ellipsis_cut(&chars, mid, middle)) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    ellipsis_cut(&chars, lo, middle)
}

/// The isolates open at the end of `chars`, outermost first: (position, initiator).
fn open_isolates(chars: &[char]) -> Vec<(usize, char)> {
    let mut open = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            FSI | LRI | RLI => open.push((i, c)),
            PDI => {
                open.pop();
            }
            _ => {}
        }
    }
    open
}

/// A name or label for a button, menu row or card title in a UI written in `dir`: wrapped
/// in FSI…PDI (unless it already is one isolate) so its own words keep their order and the
/// punctuation at its ends stays with it, and right to left started with an RLM so the
/// label is laid out as a right-to-left paragraph whatever its first letter. Empty text is
/// returned as it is.
pub fn isolate_label(s: &str, dir: Direction) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(s.len() + 9);
    if dir.is_rtl() {
        out.push(RLM);
    }
    let body = s.strip_prefix(RLM).unwrap_or(s);
    if is_one_isolate(body) {
        out.push_str(body);
    } else {
        // FSI looks only at its own level: a message made of isolated arguments and
        // punctuation ("{ $n }. { $mode }") would turn left to right and put a Hebrew row's
        // number on the left. Such a label takes the direction of its first letter, nested
        // isolates included.
        let open = match base_direction(body) {
            None if first_letter_direction(body) == Some(Direction::Rtl) => RLI,
            _ => FSI,
        };
        out.push(open);
        out.push_str(body);
        out.push(PDI);
    }
    out
}

/// A signed number as shown: `digits` (already in the locale's format, without a sign)
/// after "+" (when `plus`) or a true minus "−", never the locale's hyphen with its marks
/// (CLDR's Hebrew "‎-" leaves a gap and can be split from the digits).
pub fn signed_number(digits: &str, negative: bool, plus: bool) -> String {
    let digits = digits.trim_start_matches([LRM, RLM, ALM, '-', '\u{2212}', '+']);
    let mut out = String::with_capacity(digits.len() + 3);
    if negative {
        out.push('\u{2212}');
    } else if plus {
        out.push('+');
    }
    out.push_str(digits);
    out
}

/// A value with a sign or unit ("+0.060", "−1.50 EV", "+12°") for a UI written in `dir`:
/// right to left it is one LRI…PDI isolate, so the sign stays before the digits and the
/// unit after them wherever it is drawn (a slider, a readout, an Info row). Left to right,
/// or when it already is one isolate, it is returned as it is.
pub fn ltr_value(s: &str, dir: Direction) -> String {
    if !dir.is_rtl() || s.is_empty() || (s.starts_with(LRI) && is_one_isolate(s)) {
        return s.to_owned();
    }
    let body = if s.starts_with(FSI) && is_one_isolate(s) {
        &s[FSI.len_utf8()..s.len() - PDI.len_utf8()]
    } else {
        s
    };
    isolate_ltr(body)
}

/// Direction of the first letter of `s`, inside isolates too; `None` when there is none.
fn first_letter_direction(s: &str) -> Option<Direction> {
    s.chars().find_map(|c| {
        if is_rtl_letter(c) {
            Some(Direction::Rtl)
        } else if c.is_alphabetic() {
            Some(Direction::Ltr)
        } else {
            None
        }
    })
}

/// True when every isolate initiator (FSI, LRI, RLI) has its PDI and the other way round.
pub fn isolates_balanced(s: &str) -> bool {
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            FSI | LRI | RLI => depth += 1,
            PDI => match depth.checked_sub(1) {
                Some(d) => depth = d,
                None => return false,
            },
            _ => {}
        }
    }
    depth == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use unic_langid::langid;

    #[test]
    fn directions() {
        for rtl in [
            langid!("ar"),
            langid!("ar-EG"),
            langid!("he"),
            langid!("fa"),
            langid!("ur"),
            langid!("ar-XB"),
            langid!("az-Arab"),
        ] {
            assert_eq!(direction(&rtl), Direction::Rtl, "{rtl}");
        }
        for ltr in [
            langid!("en"),
            langid!("de"),
            langid!("ja"),
            langid!("zh-Hans"),
            langid!("en-XA"),
            langid!("ku-Latn"),
        ] {
            assert_eq!(direction(&ltr), Direction::Ltr, "{ltr}");
        }
    }

    #[test]
    fn isolates() {
        assert_eq!(isolate("IMG_1.jpg"), "\u{2068}IMG_1.jpg\u{2069}");
        assert_eq!(isolate_ltr("C:\\a"), "\u{2066}C:\\a\u{2069}");
        assert_eq!(strip_isolates(&isolate("x")), "x");
        assert_eq!(strip_isolates("\u{202E}abc\u{202C}"), "abc");
        assert_eq!(strip_isolates("\u{200F}abc"), "abc");
    }

    #[test]
    fn numbers_joined_by_neutral_separators_keep_their_order() {
        let iso = |s: &str| format!("{LRI}{s}{PDI}");
        let f = |s: &str| format!("{FSI}{s}{PDI}");
        // The annotation export's level line: placeables only.
        let s = format!("{} · {} × {}", f("1"), f("24,576"), f("17,920"));
        let out = for_rtl(&s);
        assert_eq!(
            out,
            iso(&format!(
                "{} · {} × {}",
                iso("1"),
                iso("24,576"),
                iso("17,920")
            ))
        );
        assert_eq!(strip_isolates(&out), "1 · 24,576 × 17,920");
        // Dimensions, en-dash ranges, ratios and times inside Hebrew text.
        for (src, run) in [
            (
                format!("גודל {} × {} פיקסלים", f("96"), f("112")),
                format!("{} × {}", iso("96"), iso("112")),
            ),
            (
                format!("{}×{}", f("1280"), f("720")),
                format!("{}×{}", iso("1280"), iso("720")),
            ),
            (
                format!("טווח {}–{}", f("1"), f("5")),
                format!("{}–{}", iso("1"), iso("5")),
            ),
            ("קנה מידה 1 : 50".to_owned(), "1 : 50".to_owned()),
            (
                format!("פרוסה {} / {}", f("3"), f("12")),
                format!("{} / {}", iso("3"), iso("12")),
            ),
            (
                format!("ממוצע {} ± {}", f("153.2"), f("12.71")),
                format!("{} ± {}", iso("153.2"), iso("12.71")),
            ),
            (
                format!("משך {}:{}", f("1"), f("30")),
                format!("{}:{}", iso("1"), iso("30")),
            ),
            (
                format!("{} – {} מ״מ", f("-2.5"), f("4")),
                format!("{} – {}", iso("-2.5"), iso("4")),
            ),
        ] {
            let out = keep_number_runs_ltr(&resolve_fsi(&src));
            assert!(out.contains(&iso(&run)), "{src:?} → {out:?}");
        }
        // Left alone: one number, numbers joined by a comma or a word, a separator between
        // words, a Hebrew argument, a digit inside a word.
        for s in [
            format!("{} תמונות", f("3")),
            format!("{}, {}", f("1"), f("2")),
            format!("{} מתוך {}", f("3"), f("12")),
            format!("{} · {}", f("IMG.jpg"), f("3")),
            format!("{} × {}", f("שלוש"), f("4")),
            "H264/5".to_owned(),
            "בשעה 7:05".to_owned(),
            // A middle dot in a sentence separates parts: "when · time since the start".
            format!("{} · {} מההתחלה", f("2024-06-03 08:25:00"), f("+0:25:00")),
            "שלב 2: עיבוד".to_owned(),
        ] {
            let r = resolve_fsi(&s);
            assert_eq!(keep_number_runs_ltr(&r), r, "{s:?}");
        }
    }

    #[test]
    fn units_stay_inside_the_number_isolate() {
        // Cull mode's "median + 2σ" in Hebrew read "σ2": the σ sat outside the isolate.
        assert_eq!(
            attach_units("(חציון + \u{2068}2\u{2069}σ):"),
            "(חציון + \u{2068}2σ\u{2069}):"
        );
        assert_eq!(
            for_rtl("חציון + \u{2068}2\u{2069}σ"),
            "חציון + \u{2066}2σ\u{2069}"
        );
        assert_eq!(attach_units("\u{2068}5\u{2069} px"), "\u{2068}5 px\u{2069}");
        assert_eq!(
            attach_units("ב\u{2068}50\u{2069}%."),
            "ב\u{2068}50%\u{2069}."
        );
        assert_eq!(
            attach_units("FWHM \u{2068}2.1\u{2069} px (\u{2068}1.4\u{2069}″)"),
            "FWHM \u{2068}2.1 px\u{2069} (\u{2068}1.4″\u{2069})"
        );
        assert_eq!(
            attach_units("\u{2068}9\u{2069} mm/s ·"),
            "\u{2068}9 mm/s\u{2069} ·"
        );
        // A unit given as its own argument (the science readout's "ערך { $value } { $unit }"
        // drew "m −5.419e-11").
        assert_eq!(
            for_rtl("ערך \u{2068}−5.419e-11\u{2069} \u{2068}m\u{2069}"),
            "ערך \u{2066}−5.419e-11 m\u{2069}"
        );
        assert_eq!(
            attach_units("\u{2068}2\u{2069} \u{2068}µm\u{2069}, \u{2068}3\u{2069}"),
            "\u{2068}2 µm\u{2069}, \u{2068}3\u{2069}"
        );
        // Not merged: a name before the unit, a Hebrew unit, a unit starting with `/`.
        for s in [
            "\u{2068}IMG.jpg\u{2069} \u{2068}px\u{2069}",
            "\u{2068}3\u{2069} \u{2068}מ׳\u{2069}",
            "\u{2068}3\u{2069} \u{2068}/s\u{2069}",
        ] {
            assert_eq!(attach_units(s), s);
        }
        // Not a unit: Hebrew after it, a run into other letters, a Hebrew argument, two spaces.
        for s in [
            "\u{2068}3\u{2069} /דקה",
            "\u{2068}3\u{2069}pxא",
            "\u{2068}שם\u{2069} px",
            "\u{2068}3\u{2069}  px",
        ] {
            assert_eq!(attach_units(s), s);
        }
        // Nested isolates: only the outer one takes the unit.
        let out = attach_units("\u{2068}\u{2068}1\u{2069}–\u{2068}2\u{2069}\u{2069} ms");
        assert!(isolates_balanced(&out));
        assert_eq!(
            out,
            "\u{2068}\u{2068}1\u{2069}–\u{2068}2\u{2069} ms\u{2069}"
        );
    }

    #[test]
    fn signs_stay_inside_the_number_isolate() {
        let (f, l, p) = (FSI, LRI, PDI);
        // "±{ $percent } %" and the ion prompt's "± { $tolerance } · …".
        assert_eq!(attach_signs(&format!("±{f}5{p} %")), format!("{f}±5{p} %"));
        assert_eq!(for_rtl(&format!("±{f}5{p} %")), format!("{l}±5 %{p}"));
        assert_eq!(
            attach_signs(&format!("± {f}0.5{p} · Tab סבולת")),
            format!("{f}± 0.5{p} · Tab סבולת")
        );
        // "«{ $file }» وغيره (+{ $more })" and a minus in a sentence.
        assert_eq!(
            attach_signs(&format!("وغيره (+{f}3{p})")),
            format!("وغيره ({f}+3{p})")
        );
        assert_eq!(
            attach_signs(&format!("שינוי −{f}2.5{p} מ״מ")),
            format!("שינוי {f}−2.5{p} מ״מ")
        );
        // Left alone: a Hebrew prefix ("from-"), a binary operator, a range, a sign before a
        // name, a sign apart from the number.
        for s in [
            format!("מ-{f}3{p} עד {f}5{p}"),
            format!("(חציון + {f}2{p}σ)"),
            format!("{f}1{p}-{f}5{p}"),
            format!("a-{f}5{p}"),
            format!("+{f}שלוש{p}"),
            format!("−{f}IMG.jpg{p}"),
            format!("שינוי + {f}2{p}"),
        ] {
            assert_eq!(attach_signs(&s), s, "{s:?}");
        }
        // "Person #{ $id }": the number sign reads with the number.
        assert_eq!(
            attach_signs(&format!("אדם #{f}21{p}")),
            format!("אדם {f}#21{p}")
        );
        assert_eq!(attach_signs(&format!("№{f}4{p}")), format!("{f}№4{p}"));
        // Left alone: a number sign after a letter or apart from the number.
        for s in [format!("C#{f}2{p}"), format!("# {f}2{p}")] {
            assert_eq!(attach_signs(&s), s, "{s:?}");
        }
        // The number run after a moved sign is still one number.
        let out = for_rtl(&format!("ממוצע {f}153.2{p} ± {f}12.71{p}"));
        assert!(
            out.contains(&format!("{l}{l}153.2{p} ± {l}12.71{p}{p}")),
            "{out:?}"
        );
    }

    #[test]
    fn labelled_lines_follow_their_text() {
        let (f, p) = (FSI, PDI);
        // Left to right: the plain join.
        assert_eq!(
            join_labelled("dana", "Revised.", ": ", Direction::Ltr),
            "dana: Revised."
        );
        // A Latin comment in a right-to-left UI reads left to right, author first.
        let s = join_labelled("dana", "Revised.", ": ", Direction::Rtl);
        assert_eq!(s, format!("{LRM}{f}dana{p}: {f}Revised.{p}"));
        assert_eq!(base_direction(&s), Some(Direction::Ltr));
        // A Hebrew comment, even by a Latin author, reads right to left.
        let s = join_labelled("dana", "תוקן.", ": ", Direction::Rtl);
        assert_eq!(s, format!("{RLM}{f}dana{p}: {f}תוקן.{p}"));
        // No strong letters: the UI's direction. No author: the text alone.
        let s = join_labelled("דנה", "+1", ": ", Direction::Rtl);
        assert_eq!(base_direction(&s), Some(Direction::Rtl));
        assert_eq!(
            join_labelled("", "Revised.", ": ", Direction::Rtl),
            format!("{LRM}{f}Revised.{p}")
        );
    }

    #[test]
    fn nested_fsi_takes_the_direction_of_its_letters() {
        // Fluent's "date-time" in ar: FSI[FSI(weekday)، FSI(date)]، FSI(time).
        let s = "\u{2068}\u{2068}الثلاثاء\u{2069}، \u{2068}2 أغسطس 2016\u{2069}\u{2069}، \u{2068}7:05 م\u{2069}";
        let r = for_rtl(s);
        assert_eq!(
            r,
            "\u{200F}\u{2067}\u{2067}الثلاثاء\u{2069}، \u{2067}2 أغسطس 2016\u{2069}\u{2069}، \u{2067}7:05 م\u{2069}"
        );
        assert!(isolates_balanced(&r));
        assert_eq!(base_direction(&r), Some(Direction::Rtl));
        // Numbers and Latin names become LRI; an Arabic sentence keeps its own direction.
        assert_eq!(
            for_rtl("\u{2068}12\u{2069} صورة"),
            "\u{2066}12\u{2069} صورة"
        );
        assert_eq!(
            for_rtl("\u{2068}IMG.jpg\u{2069} / \u{2068}3\u{2069}"),
            "\u{2066}IMG.jpg\u{2069} / \u{2066}3\u{2069}"
        );
        // An unpaired FSI is left alone.
        assert_eq!(resolve_fsi("\u{2068}abc"), "\u{2068}abc");
    }

    #[test]
    fn joined_parts_read_in_the_ui_direction() {
        // Left to right: the plain join.
        assert_eq!(
            join_parts(&["architecture.ifc", "", "4 parts"], " — ", Direction::Ltr),
            "architecture.ifc — 4 parts"
        );
        // Right to left: each part isolated, the line right to left even when it starts with
        // a Latin name or a number.
        let s = join_parts(&["100 ±0.2", "מרחק"], " — ", Direction::Rtl);
        assert_eq!(s, "\u{200F}\u{2068}100 ±0.2\u{2069} — \u{2068}מרחק\u{2069}");
        assert_eq!(base_direction(&s), Some(Direction::Rtl));
        let s = join_parts(&["MBD_A", "\u{2068}x\u{2069}"], " · ", Direction::Rtl);
        assert_eq!(s, "\u{200F}\u{2068}MBD_A\u{2069} · \u{2068}x\u{2069}");
        assert!(isolates_balanced(&s));
        // Two isolates side by side are not one.
        assert!(!is_one_isolate("\u{2068}a\u{2069}\u{2068}b\u{2069}"));
        assert_eq!(join_parts(&[], " · ", Direction::Rtl), "");
    }

    #[test]
    fn truncation_keeps_isolates_balanced() {
        assert_eq!(truncate("abcdef", 6), "abcdef");
        assert_eq!(truncate("abcdef", 4), "abc…");
        // Controls are not counted.
        let s = "\u{200F}\u{2068}abc\u{2069} — \u{2068}def\u{2069}";
        assert_eq!(truncate(s, 9), s);
        let t = truncate(s, 5);
        assert_eq!(t, "\u{200F}\u{2068}abc\u{2069} …");
        let t = truncate(s, 8);
        assert_eq!(t, "\u{200F}\u{2068}abc\u{2069} — \u{2068}d…\u{2069}");
        assert!(isolates_balanced(&t));
        assert_eq!(truncate("\u{202E}abcdef\u{202C}", 3), "\u{202E}ab…\u{202C}");
    }

    #[test]
    fn joined_rtl_parts_are_isolated_in_ltr_text() {
        // A Hebrew part keeps its number; a Latin one is left plain.
        let s = join_parts(
            &["IMG.jpg", "תמונה 2", "4000 × 3000"],
            " · ",
            Direction::Ltr,
        );
        assert_eq!(s, "IMG.jpg · \u{2068}תמונה 2\u{2069} · 4000 × 3000");
        assert!(isolates_balanced(&s));
    }

    #[test]
    fn balancing_closes_cut_isolates_and_drops_stray_ends() {
        let (f, l, r, p) = (FSI, LRI, RLI, PDI);
        assert_eq!(balance_isolates("abc"), "abc");
        // Cut open: closed at the end, innermost first.
        assert_eq!(
            balance_isolates(&format!("שם {f}Warm {l}lo…")),
            format!("שם {f}Warm {l}lo…{p}{p}")
        );
        // An end without its opener (the tail of a middle cut) is dropped.
        assert_eq!(balance_isolates(&format!("…ok{p} חם")), "…ok חם");
        assert_eq!(
            balance_isolates(&format!("{r}א{p}{p}b")),
            format!("{r}א{p}b")
        );
        // A PDF cannot close an isolate, nor a PDI an override.
        assert_eq!(
            balance_isolates(&format!("{f}a\u{202C}b")),
            format!("{f}ab{p}")
        );
        assert_eq!(balance_isolates("\u{202E}a\u{2069}b"), "\u{202E}ab\u{202C}");
        for s in [
            format!("{f}Warm look{p}: השוו…"),
            format!("…{p} · {f}חם"),
            format!("{l}{f}x"),
        ] {
            assert!(isolates_balanced(&balance_isolates(&s)), "{s:?}");
        }
    }

    #[test]
    fn labels_are_isolated_for_the_ui_direction() {
        let (f, p) = (FSI, PDI);
        assert_eq!(isolate_label("", Direction::Rtl), "");
        assert_eq!(
            isolate_label("Warm look!", Direction::Ltr),
            format!("{f}Warm look!{p}")
        );
        let s = isolate_label("Warm look!", Direction::Rtl);
        assert_eq!(s, format!("{RLM}{f}Warm look!{p}"));
        assert_eq!(base_direction(&s), Some(Direction::Rtl));
        // A message already prepared (an RLM, one isolate) is not wrapped twice.
        assert_eq!(isolate_label(&s, Direction::Rtl), s);
        // A numbered row, "{ $n }. { $mode }" in Hebrew: only isolates and punctuation at its
        // own level, so FSI would make it left to right; it is isolated right to left.
        let row = format!("{RLM}{LRI}3{PDI}. {RLI}ריפוי{PDI}");
        assert_eq!(
            isolate_label(&row, Direction::Rtl),
            format!("{RLM}{RLI}{LRI}3{PDI}. {RLI}ריפוי{PDI}{PDI}")
        );
        let latin = format!("{LRI}3{PDI}. {LRI}Heal{PDI}");
        assert_eq!(
            isolate_label(&latin, Direction::Ltr),
            format!("{f}{latin}{p}")
        );
    }

    #[test]
    fn signed_values_keep_their_sign_and_are_isolated_right_to_left() {
        assert_eq!(signed_number("0.060", false, true), "+0.060");
        assert_eq!(signed_number("\u{200E}-1.50", true, true), "\u{2212}1.50");
        assert_eq!(signed_number("0", false, false), "0");
        let (l, f, p) = (LRI, FSI, PDI);
        assert_eq!(ltr_value("+12°", Direction::Ltr), "+12°");
        assert_eq!(ltr_value("+12°", Direction::Rtl), format!("{l}+12°{p}"));
        assert_eq!(
            ltr_value(&format!("{f}−1.50 EV{p}"), Direction::Rtl),
            format!("{l}−1.50 EV{p}")
        );
        let once = format!("{l}+0.060{p}");
        assert_eq!(ltr_value(&once, Direction::Rtl), once);
        assert_eq!(ltr_value("", Direction::Rtl), "");
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn a_cut_isolate_is_closed_after_the_ellipsis() {
        // "Comparing: Warm look" in Hebrew, cut inside the name.
        let s = format!("{RLM}השוואה: {FSI}Warm look{PDI}");
        let c = chars(&s);
        let cut = ellipsis_cut(&c, 14, false);
        assert_eq!(cut, format!("{RLM}השוואה: {FSI}Warm…{PDI}"));
        assert!(isolates_balanced(&cut));
        // Cut before the isolate opens: nothing is added.
        assert_eq!(ellipsis_cut(&c, 7, false), format!("{RLM}השוואה…"));
        // Nested: both closed, innermost first.
        let s = format!("{FSI}a {LRI}12 34{PDI} b{PDI}");
        let cut = ellipsis_cut(&chars(&s), 5, false);
        assert_eq!(cut, format!("{FSI}a {LRI}1…{PDI}{PDI}"));
    }

    #[test]
    fn a_middle_cut_keeps_both_ends_whole() {
        // A Latin file name and a Hebrew title: the head opens an isolate, the tail closes one.
        let s = format!("{FSI}IMG_0001.jpg{PDI} · {FSI}תמונה ראשית{PDI}");
        let c = chars(&s);
        for keep in 0..=c.len() {
            let cut = ellipsis_cut(&c, keep, true);
            assert!(isolates_balanced(&cut), "{keep}: {cut:?}");
        }
        let cut = ellipsis_cut(&c, 10, true);
        assert_eq!(cut, format!("{FSI}IMG_…{PDI}{FSI}אשית{PDI}"));
        // A path cut in its middle keeps its tail left to right.
        let s = format!("{LRI}/home/ana/keymap.toml{PDI} · חדש");
        let cut = ellipsis_cut(&chars(&s), 16, true);
        assert_eq!(cut, format!("{LRI}/home/a…l{PDI} · חדש"));
    }

    #[test]
    fn hebrew_and_arabic_file_names_cut_inside_their_isolate() {
        // Hebrew "family photo at the sea.jpg" and Arabic "family photo by the sea.png", each
        // isolated in a right-to-left caption, cut in the middle of the name.
        for name in ["צילום משפחתי בים.jpg", "صورة العائلة على البحر.png"]
        {
            let s = format!("{RLM}{FSI}{name}{PDI}");
            let c = chars(&s);
            for keep in 0..=c.len() {
                let cut = ellipsis_cut(&c, keep, true);
                assert!(isolates_balanced(&cut), "{keep}: {cut:?}");
                assert_eq!(strip_isolates(&cut).matches('…').count(), 1, "{cut:?}");
            }
            let cut = ellipsis_cut(&c, 12, true);
            // The ellipsis stays inside the name's isolate and the extension is kept.
            let ext = &name[name.len() - 4..];
            assert!(cut.starts_with(&format!("{RLM}{FSI}")), "{cut:?}");
            assert!(cut.ends_with(&format!("{ext}{PDI}")), "{cut:?}");
            assert_eq!(cut.matches(FSI).count(), 1, "{cut:?}");
            assert_eq!(cut.matches(PDI).count(), 1, "{cut:?}");
            // A tail cut inside the name closes its isolate after the ellipsis.
            let cut = ellipsis_cut(&c, 8, false);
            assert!(cut.ends_with(&format!("…{PDI}")), "{cut:?}");
            assert!(isolates_balanced(&cut));
        }
        // Two names joined, the cut in the second one: the first stays whole and closed.
        let s = format!("{FSI}אלבום{PDI} · {FSI}صورة العائلة.png{PDI}");
        let c = chars(&s);
        for keep in 0..=c.len() {
            for middle in [false, true] {
                assert!(isolates_balanced(&ellipsis_cut(&c, keep, middle)));
            }
        }
    }

    #[test]
    fn ellipsize_to_fit_keeps_the_most_that_fits() {
        let s = format!("{RLM}{FSI}צילום משפחתי בים.jpg{PDI}");
        // Width as visible characters (bidi controls take no room).
        let width = |t: &str| strip_isolates(t).chars().count();
        assert_eq!(ellipsize_to_fit(&s, true, |t| width(t) <= 40), s);
        for max in 1..20 {
            let cut = ellipsize_to_fit(&s, true, |t| width(t) <= max);
            assert!(width(&cut) <= max, "{max}: {cut:?}");
            assert!(isolates_balanced(&cut), "{max}: {cut:?}");
        }
        assert_eq!(ellipsize_to_fit("abc", false, |_| false), "…");
    }
}

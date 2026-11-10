//! The byte stream of a plot file as events: HP-GL commands with their raw parameters,
//! labels, and what the PCL 5 wrapper around HP-GL/2 says (pages, the picture frame).
//!
//! HP-GL syntax: a two-letter mnemonic (either case), parameters separated by commas or
//! spaces, ended by `;`, by the next mnemonic or by an escape. Labels (`LB`, `BL`, `WD`)
//! run to the label terminator (ETX unless `DT` sets another); `DT` and `SM` take one
//! character; `CO`, `MG` and `BP` may carry quoted strings; `PE` runs to `;`.
//!
//! PCL: `ESC % -12345 X` (universal exit, then PJL lines), `ESC E` (reset), `ESC % n B`
//! (enter HP-GL/2), `ESC % n A` (back to PCL), parameterised escapes (`ESC & l 1 O`,
//! `ESC * c 7200 X`), whose `W` forms carry binary data (raster rows, fonts) that is
//! skipped by its stated length, and form feeds.

/// Escape.
pub(crate) const ESC: u8 = 0x1b;
/// End of text: the default label terminator.
pub(crate) const ETX: u8 = 0x03;

/// What the stream holds next.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event<'a> {
    /// A command: its mnemonic in upper case and its parameter bytes (without `;`).
    Cmd([u8; 2], &'a [u8]),
    /// `LB` (also `BL`, `WD`): the text up to the terminator (excluded).
    Label([u8; 2], &'a [u8]),
    /// `DT t,mode`: the new label terminator and whether it is printed (mode 0).
    Terminator(u8, bool),
    /// PCL reset (`ESC E`), universal exit or a form feed: the page ends.
    PageEnd,
    /// The PCL picture frame size (`ESC * c # X` / `Y`), decipoints.
    FrameX(f64),
    FrameY(f64),
    /// PCL logical page orientation (`ESC & l # O`).
    Orientation(i32),
    /// A PJL line or PCL entered or left HP-GL/2 (`ESC % # B`).
    EnterHpgl2,
    /// PCL mode was entered (a PCL job, or `ESC % # A`).
    Pcl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Hpgl,
    Pcl,
    /// After a universal exit: PJL lines, then PCL.
    Pjl,
}

pub(crate) struct Lexer<'a> {
    b: &'a [u8],
    pub(crate) i: usize,
    mode: Mode,
    term: u8,
    /// Whether any PCL or PJL was seen.
    pub(crate) wrapped: bool,
    /// Events found together (a chained PCL escape), oldest first.
    queue: std::collections::VecDeque<Event<'a>>,
}

pub(crate) fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
}

/// Whether the arguments of a `CO` are a comment written without quotes: a space, then
/// text that is not a quoted string (it runs to `;`).
pub(crate) fn unquoted_comment(args: &[u8]) -> bool {
    args.first().copied().is_some_and(is_space)
        && args
            .iter()
            .find(|&&c| !is_space(c))
            .is_some_and(|&c| c != b'"' && c != b';' && c != ESC)
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Lexer<'a> {
        let mut i = 0;
        // A byte-order mark or leading NULs (some drivers pad).
        if b.starts_with(b"\xEF\xBB\xBF") {
            i = 3;
        }
        let mode = match b.get(i..i + 2) {
            Some([ESC, b'%' | b'E' | b'&' | b'*' | b'(' | b')']) => Mode::Pcl,
            _ => Mode::Hpgl,
        };
        Lexer {
            b,
            i,
            mode,
            term: ETX,
            wrapped: mode == Mode::Pcl,
            queue: std::collections::VecDeque::new(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    /// The next event, `None` at the end.
    pub(crate) fn next_event(&mut self) -> Option<Event<'a>> {
        loop {
            if let Some(e) = self.queue.pop_front() {
                return Some(e);
            }
            let c = self.peek()?;
            match self.mode {
                Mode::Pjl => {
                    if let Some(e) = self.pjl_line() {
                        return Some(e);
                    }
                }
                Mode::Pcl => {
                    if c == ESC {
                        if let Some(e) = self.escape() {
                            return Some(e);
                        }
                    } else if c == 0x0c {
                        self.i += 1;
                        return Some(Event::PageEnd);
                    } else {
                        // Printable PCL text: not drawn.
                        self.i += 1;
                    }
                }
                Mode::Hpgl => {
                    if c == ESC {
                        if let Some(e) = self.escape() {
                            return Some(e);
                        }
                    } else if c.is_ascii_alphabetic() {
                        if let Some(e) = self.command() {
                            return Some(e);
                        }
                    } else {
                        // Separators, stray terminators, control bytes.
                        self.i += 1;
                    }
                }
            }
        }
    }

    /// One PJL line (`@PJL ...`); anything else switches to PCL.
    fn pjl_line(&mut self) -> Option<Event<'a>> {
        let rest = &self.b[self.i..];
        // Blank lines between the exit and the first PJL line.
        let skip = rest
            .iter()
            .take_while(|&&c| c == b'\r' || c == b'\n')
            .count();
        let rest = &rest[skip..];
        if rest.is_empty() {
            self.i = self.b.len();
            return None;
        }
        if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case(b"@PJL") {
            let end = rest
                .iter()
                .position(|&c| c == b'\n' || c == ESC)
                .unwrap_or(rest.len());
            let line = &rest[..end];
            self.i += skip + end + usize::from(rest.get(end) == Some(&b'\n'));
            let up: Vec<u8> = line
                .iter()
                .filter(|c| !is_space(**c))
                .map(u8::to_ascii_uppercase)
                .collect();
            if let Some(p) = find(&up, b"ENTERLANGUAGE=") {
                let lang = &up[p + 14..];
                if lang.starts_with(b"HPGL2") || lang.starts_with(b"HPGL") {
                    self.mode = Mode::Hpgl;
                    return Some(Event::EnterHpgl2);
                }
                self.mode = Mode::Pcl;
                return Some(Event::Pcl);
            }
            None
        } else {
            self.i += skip;
            self.mode = Mode::Pcl;
            Some(Event::Pcl)
        }
    }

    /// An escape sequence (`self.i` on the ESC).
    fn escape(&mut self) -> Option<Event<'a>> {
        let b = self.b;
        let i = self.i;
        let Some(&c1) = b.get(i + 1) else {
            self.i = b.len();
            return None;
        };
        match c1 {
            b'%' => {
                // ESC % [-]digits X|A|B
                let mut j = i + 2;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'-' || b[j] == b'+') {
                    j += 1;
                }
                let Some(&t) = b.get(j) else {
                    self.i = b.len();
                    return None;
                };
                self.i = j + 1;
                self.wrapped = true;
                match t {
                    b'X' => {
                        self.mode = Mode::Pjl;
                        Some(Event::PageEnd)
                    }
                    b'B' => {
                        self.mode = Mode::Hpgl;
                        Some(Event::EnterHpgl2)
                    }
                    b'A' => {
                        self.mode = Mode::Pcl;
                        Some(Event::Pcl)
                    }
                    _ => None,
                }
            }
            b'E' => {
                self.i = i + 2;
                self.wrapped = true;
                self.mode = Mode::Pcl;
                Some(Event::PageEnd)
            }
            b'.' => {
                // HP-GL device control (RS-232 plotters): ESC . c [params] [:]
                let mut j = i + 3;
                if j <= b.len() {
                    let mut k = j;
                    while k < b.len() && k - j < 64 && (b[k].is_ascii_digit() || b[k] == b';') {
                        k += 1;
                    }
                    if b.get(k) == Some(&b':') {
                        j = k + 1;
                    }
                }
                self.i = j.min(b.len());
                None
            }
            0x21..=0x2f => {
                // Parameterised: ESC c1 c2 (value param)*, lower-case params chain, an upper-
                // case one ends it.
                self.wrapped = true;
                let Some(&c2) = b.get(i + 2) else {
                    self.i = b.len();
                    return None;
                };
                let mut j = i + 3;
                loop {
                    let s = j;
                    while j < b.len()
                        && (b[j].is_ascii_digit() || matches!(b[j], b'+' | b'-' | b'.'))
                    {
                        j += 1;
                    }
                    let v = parse_f64(&b[s..j]).unwrap_or(0.0);
                    let Some(&p) = b.get(j) else {
                        self.i = b.len();
                        return None;
                    };
                    j += 1;
                    let up = p.to_ascii_uppercase();
                    // Binary data follows `W` (and `ESC & p # X`, transparent print data).
                    if up == b'W' || (c1 == b'&' && c2 == b'p' && up == b'X') {
                        let n = v.clamp(0.0, 1e12) as usize;
                        j = j.saturating_add(n).min(b.len());
                    }
                    let e = match (c1, c2.to_ascii_lowercase(), up) {
                        (b'*', b'c', b'X') => Some(Event::FrameX(v)),
                        (b'*', b'c', b'Y') => Some(Event::FrameY(v)),
                        (b'&', b'l', b'O') => Some(Event::Orientation(v as i32)),
                        // Page eject (`ESC & l 0 H`).
                        (b'&', b'l', b'H') if v == 0.0 => Some(Event::PageEnd),
                        _ => None,
                    };
                    self.queue.extend(e);
                    if !(0x60..=0x7e).contains(&p) {
                        break;
                    }
                }
                self.i = j;
                None
            }
            _ => {
                // Two-character escapes (ESC 9, ESC =, ESC Y, ESC Z).
                self.i = i + 2;
                None
            }
        }
    }

    /// A command (`self.i` on its first letter).
    fn command(&mut self) -> Option<Event<'a>> {
        let b = self.b;
        let i = self.i;
        let Some(&c2) = b.get(i + 1) else {
            self.i = b.len();
            return None;
        };
        if !c2.is_ascii_alphabetic() {
            // A lone letter: skip it.
            self.i = i + 1;
            return None;
        }
        let mn = [b[i].to_ascii_uppercase(), c2.to_ascii_uppercase()];
        let start = i + 2;
        match &mn {
            b"LB" | b"BL" | b"WD" => {
                let end = b[start..]
                    .iter()
                    .position(|&c| c == self.term)
                    .map_or(b.len(), |p| start + p);
                self.i = (end + 1).min(b.len());
                Some(Event::Label(mn, &b[start..end]))
            }
            b"DT" => {
                // DT t[,mode]; — DT; alone restores ETX.
                let (t, mut j) = match b.get(start) {
                    Some(&b';') | None => (ETX, start),
                    Some(&t) => (t, start + 1),
                };
                let mut printed = false;
                if b.get(j) == Some(&b',') {
                    j += 1;
                    let s = j;
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    printed = parse_int(&b[s..j]) == Some(0);
                }
                if b.get(j) == Some(&b';') {
                    j += 1;
                }
                self.i = j;
                // A terminator that is a separator would end every label at once.
                self.term = t;
                Some(Event::Terminator(t, printed))
            }
            b"SM" => {
                // SM c; or SM;
                let mut j = start;
                if let Some(&c) = b.get(j)
                    && c != b';'
                {
                    j += 1;
                }
                if b.get(j) == Some(&b';') {
                    j += 1;
                }
                self.i = j;
                Some(Event::Cmd(mn, &b[start..j.min(b.len())]))
            }
            // PE's data, and a comment some writers leave unquoted (`CO Circles;`).
            b"PE" | b"CO" if mn == *b"PE" || unquoted_comment(&b[start..]) => {
                let end = b[start..]
                    .iter()
                    .position(|&c| c == b';' || c == ESC)
                    .map_or(b.len(), |p| start + p);
                self.i = if b.get(end) == Some(&b';') {
                    end + 1
                } else {
                    end
                };
                Some(Event::Cmd(mn, &b[start..end]))
            }
            _ => {
                // Numbers, separators and quoted strings, to `;`, a letter or an escape.
                let mut j = start;
                while j < b.len() {
                    let c = b[j];
                    if c == b'"' {
                        j += 1;
                        while j < b.len() && b[j] != b'"' {
                            j += 1;
                        }
                        j += 1;
                        continue;
                    }
                    if c == b';' || c == ESC || c.is_ascii_alphabetic() {
                        break;
                    }
                    j += 1;
                }
                let end = j.min(b.len());
                self.i = if b.get(end) == Some(&b';') {
                    end + 1
                } else {
                    end
                };
                Some(Event::Cmd(mn, &b[start..end]))
            }
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// An integer, `None` when empty or not a number.
pub(crate) fn parse_int(s: &[u8]) -> Option<i64> {
    let s = trim(s);
    let (neg, d) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    if d.is_empty() || !d.iter().all(u8::is_ascii_digit) || d.len() > 18 {
        return None;
    }
    let v = d.iter().fold(0i64, |a, &c| a * 10 + i64::from(c - b'0'));
    Some(if neg { -v } else { v })
}

fn trim(s: &[u8]) -> &[u8] {
    let a = s.iter().position(|c| !is_space(*c)).unwrap_or(s.len());
    let b = s.iter().rposition(|c| !is_space(*c)).map_or(a, |p| p + 1);
    &s[a..b]
}

/// A decimal number (`-12`, `3.5`, `.25`, `+4.`), `None` when not one.
pub(crate) fn parse_f64(s: &[u8]) -> Option<f64> {
    let mut i = 0;
    let neg = match s.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let mut int: u64 = 0;
    let mut digits = 0usize;
    let mut extra_exp: i32 = 0;
    while i < s.len() && s[i].is_ascii_digit() {
        if int < 1_000_000_000_000_000 {
            int = int * 10 + u64::from(s[i] - b'0');
        } else {
            extra_exp += 1;
        }
        digits += 1;
        i += 1;
    }
    let mut frac_exp: i32 = 0;
    if i < s.len() && s[i] == b'.' {
        i += 1;
        while i < s.len() && s[i].is_ascii_digit() {
            if int < 1_000_000_000_000_000 {
                int = int * 10 + u64::from(s[i] - b'0');
                frac_exp -= 1;
            }
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 || i != s.len() {
        return None;
    }
    let e = extra_exp + frac_exp;
    let mut v = int as f64;
    if (-22..0).contains(&e) {
        // Exact for the short decimals plot files hold.
        v /= 10f64.powi(-e);
    } else if e != 0 {
        v *= 10f64.powi(e);
    }
    Some(if neg { -v } else { v })
}

/// The numeric parameters of a command into `out` (cleared first): numbers separated by
/// commas or spaces. Text that is not a number ends the list (quoted strings are skipped).
pub(crate) fn numbers(args: &[u8], out: &mut Vec<f64>) {
    out.clear();
    let mut i = 0;
    while i < args.len() {
        let c = args[i];
        if c == b',' || is_space(c) {
            i += 1;
            continue;
        }
        if c == b'"' {
            i += 1;
            while i < args.len() && args[i] != b'"' {
                i += 1;
            }
            i += 1;
            continue;
        }
        // A sign only leads a number: `10-20` is two numbers.
        let s = i;
        if matches!(c, b'+' | b'-') {
            i += 1;
        }
        while i < args.len() && (args[i].is_ascii_digit() || args[i] == b'.') {
            i += 1;
        }
        if s == i {
            // Not a number: stop.
            return;
        }
        match parse_f64(&args[s..i]) {
            Some(v) if v.is_finite() => out.push(v),
            _ => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(b: &[u8]) -> Vec<Event<'_>> {
        let mut l = Lexer::new(b);
        let mut v = Vec::new();
        while let Some(e) = l.next_event() {
            v.push(e);
        }
        v
    }

    #[test]
    fn commands_without_terminators() {
        let e = events(b"IN;SP1PU0,0PD 100 ,100;\r\npd200,0;lbHello\x03PU");
        assert_eq!(
            e,
            vec![
                Event::Cmd(*b"IN", b""),
                Event::Cmd(*b"SP", b"1"),
                Event::Cmd(*b"PU", b"0,0"),
                Event::Cmd(*b"PD", b" 100 ,100"),
                Event::Cmd(*b"PD", b"200,0"),
                Event::Label(*b"LB", b"Hello"),
                Event::Cmd(*b"PU", b""),
            ]
        );
    }

    #[test]
    fn label_terminator() {
        let e = events(b"DT*,1;LBab;c*LBx\x03*");
        assert_eq!(
            e,
            vec![
                Event::Terminator(b'*', false),
                Event::Label(*b"LB", b"ab;c"),
                Event::Label(*b"LB", b"x\x03"),
            ]
        );
    }

    #[test]
    fn pcl_wrapper() {
        let mut f = Vec::new();
        f.extend_from_slice(b"\x1b%-12345X@PJL JOB\r\n@PJL ENTER LANGUAGE=PCL\r\n");
        f.extend_from_slice(b"\x1bE\x1b&l1O\x1b*c7200x5040Y\x1b*b3WABC\x1b%1BIN;PD1,1;\x1b%0A\x0c");
        f.extend_from_slice(b"\x1bE\x1b%-12345X");
        let e = events(&f);
        assert_eq!(
            e,
            vec![
                Event::PageEnd,
                Event::Pcl,
                Event::PageEnd,
                Event::Orientation(1),
                Event::FrameX(7200.0),
                Event::FrameY(5040.0),
                Event::EnterHpgl2,
                Event::Cmd(*b"IN", b""),
                Event::Cmd(*b"PD", b"1,1"),
                Event::Pcl,
                Event::PageEnd,
                Event::PageEnd,
                Event::PageEnd,
            ]
        );
    }

    #[test]
    fn numbers_parse() {
        let mut v = Vec::new();
        numbers(b" 1,-2.5 +.5 3.;", &mut v);
        assert_eq!(v, vec![1.0, -2.5, 0.5, 3.0]);
        numbers(b"1,\"name\",2", &mut v);
        assert_eq!(v, vec![1.0, 2.0]);
        numbers(b"1,x", &mut v);
        assert_eq!(v, vec![1.0]);
        assert_eq!(parse_f64(b"-"), None);
        let big = parse_f64(b"12345678901234567890").unwrap();
        assert!((big / 12345678901234567890.0 - 1.0).abs() < 1e-12);
        assert_eq!(parse_f64(b"0.3"), Some(0.3));
    }
}

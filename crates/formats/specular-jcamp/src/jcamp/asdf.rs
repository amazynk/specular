//! JCAMP-DX data lines: AFFN numbers (free format, exponents, PAC signs as separators) and
//! the ASCII squeezed difference forms: SQZ (`@A-Ia-i`), DIF (`%J-Rj-r`) and DUP (`S-Zs`).
//!
//! A line of an `(X++(Y..Y))` table starts with its abscissa; the ordinates follow. When a
//! line ends in DIF form, the next line repeats its last ordinate first as a check (the
//! "Y-check"), which is verified and dropped.

/// One value as written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Token {
    /// An absolute value (AFFN or SQZ); `?` is NaN.
    Abs(f64),
    /// A difference from the previous value.
    Dif(f64),
    /// The previous value (or difference) occurs this many times in all.
    Dup(u32),
}

fn sqz(c: char) -> Option<(i32, bool)> {
    match c {
        '@' => Some((0, false)),
        'A'..='I' => Some((c as i32 - 'A' as i32 + 1, false)),
        'a'..='i' => Some((c as i32 - 'a' as i32 + 1, true)),
        _ => None,
    }
}

fn dif(c: char) -> Option<(i32, bool)> {
    match c {
        '%' => Some((0, false)),
        'J'..='R' => Some((c as i32 - 'J' as i32 + 1, false)),
        'j'..='r' => Some((c as i32 - 'j' as i32 + 1, true)),
        _ => None,
    }
}

fn dup(c: char) -> Option<i32> {
    match c {
        'S'..='Z' => Some(c as i32 - 'S' as i32 + 1),
        's' => Some(9),
        _ => None,
    }
}

/// Whether `c` is a pseudo-digit of any ASDF form.
pub(crate) fn is_pseudo(c: char) -> bool {
    sqz(c).is_some() || dif(c).is_some() || dup(c).is_some()
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Affn,
    Sqz,
    Dif,
    Dup,
}

/// Splits a data line (comments already removed) into tokens. `Err` names a character that
/// is not part of any form.
pub(crate) fn tokens(line: &str, out: &mut Vec<Token>) -> Result<(), char> {
    out.clear();
    // `E` is SQZ +5 in squeezed lines and an exponent in AFFN ones: a line with other
    // pseudo-digits is squeezed.
    let squeezed = line.chars().any(|c| is_pseudo(c) && c != 'E' && c != 'e');
    let chars: Vec<char> = line.chars().collect();
    let mut buf = String::new();
    let mut mode = Mode::Affn;
    let mut open = false;
    let flush = |buf: &mut String, mode: Mode, open: &mut bool, out: &mut Vec<Token>| {
        if !*open {
            return;
        }
        *open = false;
        let v: f64 = if buf == "?" {
            f64::NAN
        } else {
            buf.parse().unwrap_or(f64::NAN)
        };
        out.push(match mode {
            Mode::Affn | Mode::Sqz => Token::Abs(v),
            Mode::Dif => Token::Dif(v),
            Mode::Dup => Token::Dup(if v.is_finite() && v >= 1.0 {
                v.min(u32::MAX as f64) as u32
            } else {
                1
            }),
        });
        buf.clear();
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if c.is_ascii_digit() || c == '.' {
            if !open {
                open = true;
                mode = Mode::Affn;
            }
            buf.push(c);
            continue;
        }
        if c == ' ' || c == '\t' || c == ',' || c == ';' {
            flush(&mut buf, mode, &mut open, out);
            continue;
        }
        if c == '+' || c == '-' {
            // An exponent's sign stays with its number.
            if open && mode == Mode::Affn && (buf.ends_with('E') || buf.ends_with('e')) {
                buf.push(c);
                continue;
            }
            flush(&mut buf, mode, &mut open, out);
            open = true;
            mode = Mode::Affn;
            if c == '-' {
                buf.push('-');
            }
            continue;
        }
        if (c == 'E' || c == 'e')
            && open
            && mode == Mode::Affn
            && buf.chars().any(|d| d.is_ascii_digit())
            && !buf.contains(['E', 'e'])
        {
            let next = chars.get(i).copied();
            let after = chars.get(i + 1).copied();
            let signed =
                matches!(next, Some('+' | '-')) && after.is_some_and(|a| a.is_ascii_digit());
            let plain = !squeezed && next.is_some_and(|a| a.is_ascii_digit());
            if signed || plain {
                buf.push('E');
                continue;
            }
        }
        if c == '?' {
            flush(&mut buf, mode, &mut open, out);
            buf.push('?');
            open = true;
            mode = Mode::Affn;
            flush(&mut buf, mode, &mut open, out);
            continue;
        }
        let (m, digit, neg) = if let Some((d, n)) = sqz(c) {
            (Mode::Sqz, d, n)
        } else if let Some((d, n)) = dif(c) {
            (Mode::Dif, d, n)
        } else if let Some(d) = dup(c) {
            (Mode::Dup, d, false)
        } else {
            return Err(c);
        };
        flush(&mut buf, mode, &mut open, out);
        open = true;
        mode = m;
        if neg {
            buf.push('-');
        }
        buf.push(char::from(b'0' + digit as u8));
    }
    flush(&mut buf, mode, &mut open, out);
    Ok(())
}

/// Ordinates of an `(X++(Y..Y))` table, decoded line by line.
#[derive(Debug, Default)]
pub(crate) struct Ordinates {
    pub y: Vec<f64>,
    /// The abscissa each line starts with, and the index of its first ordinate.
    pub line_x: Vec<(f64, usize)>,
    /// The previous line ended in DIF form: this line's first ordinate is a check.
    pending_check: bool,
    last_dif: f64,
    last_was_dif: bool,
    pub y_check_failures: usize,
    pub bad_chars: Vec<char>,
    toks: Vec<Token>,
}

impl Ordinates {
    /// Decodes one line.
    pub fn line(&mut self, line: &str) {
        let mut toks = std::mem::take(&mut self.toks);
        if let Err(c) = tokens(line, &mut toks) {
            if !self.bad_chars.contains(&c) {
                self.bad_chars.push(c);
            }
            self.toks = toks;
            return;
        }
        let mut it = toks.iter().copied();
        let Some(first) = it.next() else {
            self.toks = toks;
            return;
        };
        let x = match first {
            Token::Abs(v) => v,
            _ => f64::NAN,
        };
        let mut first_y = true;
        let mut ended_dif = false;
        // A line that starts with the check value starts at the point it repeats.
        let mut start = self.y.len();
        for t in it {
            match t {
                Token::Abs(v) => {
                    if first_y && self.pending_check {
                        // The check value: the last ordinate again.
                        let last = self.y.last().copied().unwrap_or(f64::NAN);
                        if !(v == last || (v - last).abs() <= 1e-9 * last.abs().max(1.0)) {
                            self.y_check_failures += 1;
                        }
                        start = self.y.len().saturating_sub(1);
                    } else {
                        self.y.push(v);
                    }
                    self.last_was_dif = false;
                    ended_dif = false;
                }
                Token::Dif(d) => {
                    let last = self.y.last().copied().unwrap_or(0.0);
                    self.y.push(last + d);
                    self.last_dif = d;
                    self.last_was_dif = true;
                    ended_dif = true;
                }
                Token::Dup(n) => {
                    // After the check value it repeats the last ordinate already there.
                    let repeats = n.saturating_sub(1) as usize;
                    let Some(&last) = self.y.last() else {
                        continue;
                    };
                    if self.last_was_dif {
                        let mut v = last;
                        for _ in 0..repeats {
                            v += self.last_dif;
                            self.y.push(v);
                        }
                    } else {
                        for _ in 0..repeats {
                            self.y.push(last);
                        }
                    }
                }
            }
            first_y = false;
        }
        self.pending_check = ended_dif;
        self.line_x.push((x, start));
        self.toks = toks;
    }
}

/// Every number of an AFFN table (`(XY..XY)`): pairs or triples follow in order.
pub(crate) fn affn_values(text: &str, out: &mut Vec<f64>) -> Result<(), char> {
    let mut toks = Vec::new();
    for line in text.lines() {
        let line = strip_comment(line);
        tokens(line, &mut toks)?;
        for t in &toks {
            match *t {
                Token::Abs(v) => out.push(v),
                Token::Dif(_) | Token::Dup(_) => return Err('%'),
            }
        }
    }
    Ok(())
}

/// The line without a `$$` comment.
pub(crate) fn strip_comment(line: &str) -> &str {
    match line.find("$$") {
        Some(i) => &line[..i],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ys(lines: &[&str]) -> Vec<f64> {
        let mut o = Ordinates::default();
        for l in lines {
            o.line(l);
        }
        assert_eq!(o.y_check_failures, 0);
        o.y
    }

    #[test]
    fn forms() {
        assert_eq!(ys(&["1 10 20 -30"]), vec![10.0, 20.0, -30.0]);
        assert_eq!(ys(&["1+10+20-30"]), vec![10.0, 20.0, -30.0]);
        assert_eq!(ys(&["1A0B0c0"]), vec![10.0, 20.0, -30.0]);
        // DIF: 10, +10 → 20, −50 → −30.
        assert_eq!(ys(&["1A0J0n0"]), vec![10.0, 20.0, -30.0]);
        // DUP of a value and of a difference.
        assert_eq!(ys(&["1A0U"]), vec![10.0, 10.0, 10.0]);
        assert_eq!(ys(&["1A0J0T"]), vec![10.0, 20.0, 30.0]);
        // Y-check at the start of the next line is dropped.
        assert_eq!(ys(&["1A0J0", "3B0J0"]), vec![10.0, 20.0, 30.0]);
        // Exponents and missing values.
        let v = ys(&["1 1.5E+03 2e-2 ?"]);
        assert_eq!(v[..2], [1500.0, 0.02]);
        assert!(v[2].is_nan());
    }

    #[test]
    fn y_check_mismatch_is_counted() {
        let mut o = Ordinates::default();
        o.line("1A0J0");
        o.line("3B5J0");
        assert_eq!(o.y_check_failures, 1);
        assert_eq!(o.y, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn bad_characters_are_reported() {
        let mut t = Vec::new();
        assert_eq!(tokens("1 2 #", &mut t), Err('#'));
    }
}

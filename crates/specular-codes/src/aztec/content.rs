//! The characters of an Aztec symbol: five modes of 5-bit (digits 4-bit) codes with
//! latches and shifts between them, binary shift for runs of bytes, FLG(n) for FNC1 and ECI,
//! and the structured append header.

/// What a symbol holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Content {
    pub text: String,
    /// The bytes as encoded (without the structured append header).
    pub bytes: Vec<u8>,
    /// ECI designators in the order met.
    pub eci: Vec<u32>,
    /// FNC1 first: GS1 data.
    pub gs1: bool,
    /// Structured append: this symbol's position (from 1) and the number of symbols.
    pub structured: Option<(u8, u8)>,
    /// Structured append: the message's ID, if it has one.
    pub id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Upper,
    Lower,
    Mixed,
    Punct,
    Digit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tok {
    Byte(u8),
    Two(u8, u8),
    Latch(Mode),
    Shift(Mode),
    Binary,
    Flag,
}

const PUNCT: &[u8; 25] = b"!\"#$%&'()*+,-./:;<=>?[]{}";

fn token(mode: Mode, code: u16) -> Tok {
    use Mode::*;
    use Tok::*;
    let c = code as u8;
    match mode {
        Upper => match c {
            0 => Shift(Punct),
            1 => Byte(b' '),
            2..=27 => Byte(b'A' + c - 2),
            28 => Latch(Lower),
            29 => Latch(Mixed),
            30 => Latch(Digit),
            _ => Binary,
        },
        Lower => match c {
            0 => Shift(Punct),
            1 => Byte(b' '),
            2..=27 => Byte(b'a' + c - 2),
            28 => Shift(Upper),
            29 => Latch(Mixed),
            30 => Latch(Digit),
            _ => Binary,
        },
        Mixed => match c {
            0 => Shift(Punct),
            1 => Byte(b' '),
            2..=14 => Byte(c - 1),
            15..=19 => Byte(c + 12),
            20 => Byte(b'@'),
            21 => Byte(b'\\'),
            22 => Byte(b'^'),
            23 => Byte(b'_'),
            24 => Byte(b'`'),
            25 => Byte(b'|'),
            26 => Byte(b'~'),
            27 => Byte(0x7F),
            28 => Latch(Lower),
            29 => Latch(Upper),
            30 => Latch(Punct),
            _ => Binary,
        },
        Punct => match c {
            0 => Flag,
            1 => Byte(b'\r'),
            2 => Two(b'\r', b'\n'),
            3 => Two(b'.', b' '),
            4 => Two(b',', b' '),
            5 => Two(b':', b' '),
            6..=30 => Byte(PUNCT[usize::from(c - 6)]),
            _ => Latch(Upper),
        },
        Digit => match c {
            0 => Shift(Punct),
            1 => Byte(b' '),
            2..=11 => Byte(b'0' + c - 2),
            12 => Byte(b','),
            13 => Byte(b'.'),
            14 => Latch(Upper),
            _ => Shift(Upper),
        },
    }
}

struct Reader<'a> {
    bits: &'a [bool],
    at: usize,
}

impl Reader<'_> {
    fn read(&mut self, n: usize) -> Option<u16> {
        if self.at + n > self.bits.len() {
            return None;
        }
        let v = self.bits[self.at..self.at + n]
            .iter()
            .fold(0u16, |a, &b| a << 1 | u16::from(b));
        self.at += n;
        Some(v)
    }
}

/// Text of bytes in the character set an ECI names; without one, UTF-8 when the bytes read
/// as such, else ISO-8859-1 (Aztec's default), which also stands for binary (ECI 899) and
/// character sets not known.
fn text_of(bytes: &[u8], eci: Option<u32>) -> String {
    match eci {
        Some(e @ (0..=2 | 4..=13 | 15..=18 | 20..=30 | 170)) => {
            crate::qr::content::charset_text(bytes, Some(e))
        }
        None if std::str::from_utf8(bytes).is_ok() => String::from_utf8_lossy(bytes).into_owned(),
        _ => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// The structured append header being read: an optional ID between spaces, then two
/// capitals for the position and the count.
#[derive(Default)]
struct Header {
    on: bool,
    chars: Vec<u8>,
}

impl Header {
    /// Takes a character; false once the header is complete (the character is data then).
    fn take(&mut self, b: u8) -> bool {
        if !self.on {
            return false;
        }
        self.chars.push(b);
        let done = match self.chars.first() {
            Some(b' ') => {
                // " ID " then two
                let close = self.chars[1..].iter().position(|&c| c == b' ');
                close.is_some_and(|p| self.chars.len() == p + 2 + 2)
            }
            _ => self.chars.len() == 2,
        };
        if done {
            self.on = false;
        }
        true
    }
}

/// Where characters go: the structured append header first, then the content.
struct Out {
    c: Content,
    header: Header,
    /// Segments of bytes and their ECI.
    segs: Vec<(Option<u32>, Vec<u8>)>,
}

impl Out {
    fn emit(&mut self, b: u8) {
        if !self.header.take(b) {
            self.c.bytes.push(b);
            if let Some(s) = self.segs.last_mut() {
                s.1.push(b);
            }
        }
    }
}

/// Characters of the corrected, unstuffed data bits.
pub fn parse(bits: &[bool]) -> Result<Content, &'static str> {
    let mut r = Reader { bits, at: 0 };
    let mut o = Out {
        c: Content::default(),
        header: Header::default(),
        segs: vec![(None, Vec::new())],
    };
    let mut latched = Mode::Upper;
    let mut shifted: Option<Mode> = None;
    let mut codes = 0usize;
    let mut first: Option<Tok> = None;
    // The last codeword is padded with ones: a code cut short ends the data.
    'codes: loop {
        let mode = shifted.take().unwrap_or(latched);
        let n = if mode == Mode::Digit { 4 } else { 5 };
        let Some(code) = r.read(n) else { break };
        let tok = token(mode, code);
        // M/L U/L first: a structured append header follows
        match codes {
            0 => first = Some(tok),
            1 => {
                o.header.on =
                    first == Some(Tok::Latch(Mode::Mixed)) && tok == Tok::Latch(Mode::Upper)
            }
            _ => {}
        }
        codes += 1;
        match tok {
            Tok::Byte(b) => o.emit(b),
            Tok::Two(a, b) => {
                o.emit(a);
                o.emit(b);
            }
            Tok::Latch(m) => latched = m,
            Tok::Shift(m) => shifted = Some(m),
            Tok::Binary => {
                let Some(mut len) = r.read(5).map(usize::from) else {
                    break 'codes;
                };
                if len == 0 {
                    let Some(more) = r.read(11) else { break 'codes };
                    len = usize::from(more) + 31;
                }
                for _ in 0..len {
                    let Some(b) = r.read(8) else { break 'codes };
                    o.emit(b as u8);
                }
            }
            Tok::Flag => {
                let Some(n) = r.read(3) else { break 'codes };
                match n {
                    0 => {
                        // FNC1: GS1 data when first, a group separator after
                        if o.c.bytes.is_empty() && !o.c.gs1 && !o.header.on {
                            o.c.gs1 = true;
                        } else {
                            o.emit(0x1D);
                        }
                    }
                    1..=6 => {
                        let mut v = 0u32;
                        for _ in 0..n {
                            let Some(d) = r.read(4) else { break 'codes };
                            if !(2..=11).contains(&d) {
                                return Err("bad ECI digit");
                            }
                            v = v * 10 + u32::from(d - 2);
                        }
                        o.c.eci.push(v);
                        o.segs.push((Some(v), Vec::new()));
                    }
                    _ => return Err("reserved FLG(7)"),
                }
            }
        }
    }
    let Out {
        mut c,
        header,
        segs,
    } = o;
    if header.on {
        return Err("structured append header cut short");
    }
    if !header.chars.is_empty() {
        let h = &header.chars;
        let (id, pc) = if h[0] == b' ' {
            let close = h[1..].iter().position(|&x| x == b' ').ok_or("bad header")? + 1;
            (
                Some(String::from_utf8_lossy(&h[1..close]).into_owned()),
                &h[close + 1..],
            )
        } else {
            (None, &h[..])
        };
        if pc.len() != 2 || !pc.iter().all(u8::is_ascii_uppercase) {
            return Err("bad structured append header");
        }
        let (pos, total) = (pc[0] - b'A' + 1, pc[1] - b'A' + 1);
        if pos > total {
            return Err("structured append position past the count");
        }
        c.structured = Some((pos, total));
        c.id = id.filter(|s| !s.is_empty());
    }
    c.text = segs.iter().map(|(e, b)| text_of(b, *e)).collect();
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(codes: &[(u16, usize)]) -> Vec<bool> {
        let mut out = Vec::new();
        for &(v, n) in codes {
            for i in (0..n).rev() {
                out.push(v >> i & 1 == 1);
            }
        }
        out
    }

    #[test]
    fn modes_latches_and_shifts() {
        // "Ab1, x@" : A, L/L, b, D/L, 1, ',', U/L, ... built code by code
        let b = bits(&[
            (2, 5),  // A
            (28, 5), // L/L
            (3, 5),  // b
            (30, 5), // D/L
            (3, 4),  // 1
            (12, 4), // ,
            (1, 4),  // space
            (15, 4), // U/S
            (25, 5), // X
            (14, 4), // U/L
            (29, 5), // M/L
            (20, 5), // @
            (30, 5), // P/L
            (2, 5),  // CR LF
            (31, 5), // U/L
            (0, 5),  // P/S
            (6, 5),  // !
            (31, 5), // B/S
            (2, 5),  // two bytes
            (0xE9, 8),
            (0x00, 8),
            (3, 5), // B
        ]);
        let c = parse(&b).unwrap();
        assert_eq!(c.bytes, b"Ab1, X@\r\n!\xe9\0B");
        assert_eq!(c.text, "Ab1, X@\r\n!é\0B");
    }

    #[test]
    fn eci_fnc1_and_structured_append() {
        // FLG(0) first: GS1; FLG(2) 26: UTF-8 from there
        let b = bits(&[
            (0, 5),
            (0, 5),
            (0, 3),
            (3, 5), // B
            (0, 5),
            (0, 5),
            (0, 3), // FNC1 inside: GS
            (0, 5),
            (0, 5),
            (2, 3),
            (4, 4),
            (8, 4), // ECI 26
            (31, 5),
            (2, 5),
            (0xC3, 8),
            (0xA9, 8),
        ]);
        let c = parse(&b).unwrap();
        assert!(c.gs1);
        assert_eq!(c.eci, vec![26]);
        assert_eq!(c.text, "B\u{1d}é");
        // M/L U/L " ID " B C: part 2 of 3
        let b = bits(&[
            (29, 5),
            (29, 5),
            (1, 5),
            (10, 5), // I
            (5, 5),  // D
            (1, 5),
            (3, 5), // B
            (4, 5), // C
            (8, 5), // G
        ]);
        let c = parse(&b).unwrap();
        assert_eq!(c.structured, Some((2, 3)));
        assert_eq!(c.id.as_deref(), Some("ID"));
        assert_eq!(c.text, "G");
    }
}

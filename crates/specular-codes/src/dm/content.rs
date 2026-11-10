//! ECC 200 data codewords to text (ISO/IEC 16022, 5.2): ASCII (with digit pairs and upper
//! shift), C40, Text, ANSI X12, EDIFACT and Base 256, with FNC1 (GS1), the 05/06 macros,
//! structured append and ECI.

use crate::qr::content::charset_text;

/// What a symbol says.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Content {
    pub text: String,
    pub bytes: Vec<u8>,
    pub gs1: bool,
    /// Structured append: this symbol's position (1-based) and the number of symbols.
    pub structured: Option<(u8, u8)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ascii,
    C40,
    Text,
    X12,
    Edifact,
}

/// The group separator FNC1 stands for after the first position.
const GS: u8 = 0x1D;

/// Bytes gathered in one character set, then text.
struct Out {
    text: String,
    bytes: Vec<u8>,
    pending: Vec<u8>,
    eci: Option<u32>,
}

impl Out {
    fn push(&mut self, b: u8) {
        self.bytes.push(b);
        self.pending.push(b);
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            let p = std::mem::take(&mut self.pending);
            self.text
                .push_str(&charset_text(&p, Some(self.eci.unwrap_or(3))));
        }
    }
}

/// Decodes the data codewords; `Err` on a stream no encoder writes.
pub fn parse(cw: &[u8]) -> Result<Content, &'static str> {
    let mut o = Out {
        text: String::new(),
        bytes: Vec::new(),
        pending: Vec::new(),
        eci: None,
    };
    let mut c = Content::default();
    let mut trailer: Option<&[u8]> = None;
    let mut mode = Mode::Ascii;
    let mut upper = false;
    // A C40/Text shift carries over into the next pair.
    let mut shift = 0u8;
    let mut i = 0;
    let mut first = true;
    while i < cw.len() {
        match mode {
            Mode::Ascii => {
                let v = cw[i];
                i += 1;
                let at_start = first;
                first = false;
                match v {
                    0 => return Err("codeword 0"),
                    1..=128 => {
                        o.push(if upper { v - 1 + 128 } else { v - 1 });
                        upper = false;
                    }
                    129 => break,
                    130..=229 => {
                        let d = v - 130;
                        o.push(b'0' + d / 10);
                        o.push(b'0' + d % 10);
                    }
                    230 => mode = Mode::C40,
                    231 => {
                        // Base 256: its length, then the bytes, all randomised by position.
                        let un = |k: usize| -> Option<u8> {
                            let p = k + 1;
                            let r = ((149 * p) % 255 + 1) as u8;
                            cw.get(k).map(|&b| b.wrapping_sub(r))
                        };
                        let d1 = un(i).ok_or("Base 256 without a length")?;
                        i += 1;
                        let len = match d1 {
                            0 => cw.len() - i,
                            1..=249 => usize::from(d1),
                            _ => {
                                let d2 = un(i).ok_or("Base 256 length cut short")?;
                                i += 1;
                                250 * (usize::from(d1) - 249) + usize::from(d2)
                            }
                        };
                        if i + len > cw.len() {
                            return Err("Base 256 runs past the data");
                        }
                        for k in i..i + len {
                            o.push(un(k).unwrap_or(0));
                        }
                        i += len;
                    }
                    232 => {
                        if at_start {
                            c.gs1 = true;
                        } else {
                            o.push(GS);
                        }
                    }
                    233 => {
                        let a = *cw.get(i).ok_or("structured append cut short")?;
                        i += 3;
                        let (pos, total) = (a >> 4, 17 - (a & 0x0F));
                        c.structured = Some((pos + 1, total));
                    }
                    234 => {}
                    235 => upper = true,
                    236 | 237 if at_start => {
                        for &b in b"[)>\x1E" {
                            o.push(b);
                        }
                        o.push(b'0');
                        o.push(if v == 236 { b'5' } else { b'6' });
                        o.push(GS);
                        trailer = Some(b"\x1E\x04");
                    }
                    236 | 237 => return Err("a macro not at the start"),
                    238 => mode = Mode::X12,
                    239 => mode = Mode::Text,
                    240 => mode = Mode::Edifact,
                    241 => {
                        let c1 = u32::from(*cw.get(i).ok_or("ECI cut short")?);
                        i += 1;
                        let eci = if c1 <= 127 {
                            c1 - 1
                        } else {
                            let c2 = u32::from(*cw.get(i).ok_or("ECI cut short")?);
                            i += 1;
                            if c1 <= 191 {
                                (c1 - 128) * 254 + c2 - 1 + 127
                            } else {
                                let c3 = u32::from(*cw.get(i).ok_or("ECI cut short")?);
                                i += 1;
                                (c1 - 192) * 64516 + (c2 - 1) * 254 + c3 - 1 + 16383
                            }
                        };
                        o.flush();
                        o.eci = Some(eci);
                    }
                    _ => return Err("an unused codeword"),
                }
            }
            Mode::C40 | Mode::Text | Mode::X12 => {
                if i + 1 >= cw.len() || cw[i] == 254 {
                    // Unlatched (or one codeword left, which is ASCII).
                    if i < cw.len() && cw[i] == 254 {
                        i += 1;
                    }
                    mode = Mode::Ascii;
                    continue;
                }
                let v = (u32::from(cw[i]) << 8 | u32::from(cw[i + 1])).wrapping_sub(1);
                i += 2;
                let vals = [(v / 1600) as u8, ((v / 40) % 40) as u8, (v % 40) as u8];
                if mode == Mode::X12 {
                    for x in vals {
                        o.push(match x {
                            0 => b'\r',
                            1 => b'*',
                            2 => b'>',
                            3 => b' ',
                            4..=13 => b'0' + x - 4,
                            14..=39 => b'A' + x - 14,
                            _ => return Err("an X12 value out of range"),
                        });
                    }
                    continue;
                }
                for x in vals {
                    let ch: Option<u8> = match shift {
                        0 => match x {
                            0..=2 => {
                                shift = x + 1;
                                continue;
                            }
                            3 => Some(b' '),
                            4..=13 => Some(b'0' + x - 4),
                            14..=39 if mode == Mode::C40 => Some(b'A' + x - 14),
                            14..=39 => Some(b'a' + x - 14),
                            _ => return Err("a C40 value out of range"),
                        },
                        1 => Some(x),
                        2 => match x {
                            0..=14 => Some(b'!' + x),
                            15..=21 => Some(b':' + x - 15),
                            22..=26 => Some(b'[' + x - 22),
                            27 => {
                                o.push(GS);
                                None
                            }
                            30 => {
                                upper = true;
                                None
                            }
                            _ => return Err("a C40 shift 2 value out of range"),
                        },
                        _ if mode == Mode::C40 => Some(x + 96),
                        _ => match x {
                            0 => Some(b'`'),
                            1..=26 => Some(b'A' + x - 1),
                            27..=31 => Some(b'{' + x - 27),
                            _ => return Err("a Text shift 3 value out of range"),
                        },
                    };
                    shift = 0;
                    if let Some(ch) = ch {
                        o.push(if upper { ch.wrapping_add(128) } else { ch });
                        upper = false;
                    }
                }
            }
            Mode::Edifact => {
                // Four 6-bit values in three codewords; 31 returns to ASCII, and so do
                // fewer than three codewords left (as ASCII).
                if i + 3 > cw.len() {
                    mode = Mode::Ascii;
                    continue;
                }
                let mut bits = 0u32;
                let mut n = 0;
                let mut done = false;
                for k in 0..3 {
                    match cw.get(i + k) {
                        Some(&b) => {
                            bits = bits << 8 | u32::from(b);
                            n += 1;
                        }
                        None => break,
                    }
                }
                let total_bits = n * 8;
                for q in 0..total_bits / 6 {
                    let v = ((bits >> (total_bits - 6 * (q + 1))) & 0x3F) as u8;
                    if v == 0x1F {
                        // Unlatch: the rest of this codeword is padding.
                        i += (6 * (q + 1_usize)).div_ceil(8);
                        done = true;
                        break;
                    }
                    o.push(if v & 0x20 == 0 { v | 0x40 } else { v });
                }
                if done {
                    mode = Mode::Ascii;
                    continue;
                }
                i += n;
            }
        }
    }
    if let Some(t) = trailer {
        for &b in t {
            o.push(b);
        }
    }
    o.flush();
    c.text = o.text;
    c.bytes = o.bytes;
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c40_pair(a: u8, b: u8, c: u8) -> [u8; 2] {
        let v = 1600 * u32::from(a) + 40 * u32::from(b) + u32::from(c) + 1;
        [(v >> 8) as u8, v as u8]
    }

    #[test]
    fn ascii_digits_and_shift() {
        // "A", "12", upper shift + "a" (=225), pad.
        let c = parse(&[66, 142, 235, 98, 129, 77]).unwrap();
        assert_eq!(c.bytes, vec![b'A', b'1', b'2', 225]);
        assert_eq!(c.text, "A12á");
    }

    #[test]
    fn c40_text_x12() {
        // C40 "AB1" then unlatch, then ASCII "z".
        let mut cw = vec![230];
        cw.extend(c40_pair(14, 15, 5));
        cw.extend([254, 123]);
        assert_eq!(parse(&cw).unwrap().text, "AB1z");
        // Text "ab" + shift 3 "A" (value 1).
        let mut cw = vec![239];
        cw.extend(c40_pair(14, 15, 2));
        cw.extend(c40_pair(1, 3, 3));
        assert_eq!(parse(&cw).unwrap().text, "abA  ");
        // X12 "*>9".
        let mut cw = vec![238];
        cw.extend(c40_pair(1, 2, 13));
        assert_eq!(parse(&cw).unwrap().text, "*>9");
    }

    #[test]
    fn edifact_and_base256() {
        // EDIFACT "AB" then unlatch: 000001 000010 011111 → 0x04 0x27 0xC0 (pad bits).
        let mut cw = vec![240];
        let bits: u32 = (0x01 << 18) | (0x02 << 12) | (0x1F << 6);
        cw.extend([(bits >> 16) as u8, (bits >> 8) as u8, bits as u8]);
        cw.push(b'z' + 1);
        assert_eq!(parse(&cw).unwrap().text, "ABz");
        // Base 256: two bytes 0xFF 0x00, randomised by position.
        let rand = |b: u8, p: usize| b.wrapping_add(((149 * p) % 255 + 1) as u8);
        let cw = [231, rand(2, 2), rand(0xFF, 3), rand(0x00, 4)];
        assert_eq!(parse(&cw).unwrap().bytes, vec![0xFF, 0x00]);
    }

    #[test]
    fn gs1_macro_and_eci() {
        let c = parse(&[232, b'0' + 1, b'1' + 1, 232, b'9' + 1]).unwrap();
        assert!(c.gs1);
        assert_eq!(c.bytes, b"01\x1D9");
        let c = parse(&[236, b'A' + 1]).unwrap();
        assert_eq!(c.text, "[)>\u{1E}05\u{1D}A\u{1E}\u{4}");
        // ECI 26 (UTF-8): "é" as two bytes.
        let c = parse(&[241, 27, 235, 0x43 + 1, 235, 0x29 + 1]).unwrap();
        assert_eq!(c.text, "é");
    }
}

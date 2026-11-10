//! A QR symbol's data bit stream: numeric, alphanumeric, byte and kanji segments, ECI
//! character sets, structured append and FNC1 (GS1) markers.

use super::matrix::Fail;
use super::tables;

const ALNUM: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn left(&self) -> usize {
        (self.d.len() * 8).saturating_sub(self.pos)
    }
    fn read(&mut self, n: u32) -> Option<u32> {
        if self.left() < n as usize {
            return None;
        }
        let mut v = 0u32;
        for _ in 0..n {
            let b = self.d[self.pos / 8] >> (7 - self.pos % 8) & 1;
            v = (v << 1) | u32::from(b);
            self.pos += 1;
        }
        Some(v)
    }
}

/// The decoded content of a symbol.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Content {
    pub text: String,
    /// The raw bytes of byte segments (and the text's bytes of the others).
    pub bytes: Vec<u8>,
    /// The ECI designators met, in order.
    pub eci: Vec<u32>,
    /// Structured append: this symbol's position, the total, and the parity.
    pub structured: Option<(u8, u8, u8)>,
    /// FNC1 in the first position (GS1) or the second (an application indicator).
    pub gs1: bool,
    pub kanji: bool,
}

/// Text of bytes in a character set named by ECI (or guessed without one).
pub(crate) fn charset_text(bytes: &[u8], eci: Option<u32>) -> String {
    let label = match eci {
        Some(0 | 2) => "ibm437",
        Some(1 | 3) => "iso-8859-1",
        Some(n @ 4..=13) => {
            return encoding_rs::Encoding::for_label(format!("iso-8859-{}", n - 2).as_bytes())
                .unwrap_or(encoding_rs::WINDOWS_1252)
                .decode_without_bom_handling(bytes)
                .0
                .into_owned();
        }
        Some(n @ 15..=18) => {
            return encoding_rs::Encoding::for_label(format!("iso-8859-{}", n - 2).as_bytes())
                .unwrap_or(encoding_rs::WINDOWS_1252)
                .decode_without_bom_handling(bytes)
                .0
                .into_owned();
        }
        Some(20) => "shift_jis",
        Some(21) => "windows-1250",
        Some(22) => "windows-1251",
        Some(23) => "windows-1252",
        Some(24) => "windows-1256",
        Some(25) => "utf-16be",
        Some(26) => "utf-8",
        Some(27 | 170) => "us-ascii",
        Some(28) => "big5",
        Some(29) => "gbk",
        Some(30) => "euc-kr",
        _ => {
            if let Ok(s) = std::str::from_utf8(bytes) {
                return s.to_owned();
            }
            // Shift_JIS when it reads as such without errors, else ISO-8859-1 (the default)
            let (s, _, bad) = encoding_rs::SHIFT_JIS.decode(bytes);
            let wide = s.chars().filter(|&ch| u32::from(ch) >= 0x3000).count();
            if !bad && wide >= 2 {
                return s.into_owned();
            }
            return bytes.iter().map(|&b| char::from(b)).collect();
        }
    };
    if label == "iso-8859-1" {
        return bytes.iter().map(|&b| char::from(b)).collect();
    }
    if label == "ibm437" {
        // the printable ASCII part; the rest as Latin-1 (rarely used)
        return bytes.iter().map(|&b| char::from(b)).collect();
    }
    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(e) => e.decode_without_bom_handling(bytes).0.into_owned(),
        None => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Parses the data codewords of a symbol of `version`.
pub fn parse(data: &[u8], version: usize) -> Result<Content, Fail> {
    let mut r = Bits { d: data, pos: 0 };
    let cb = tables::count_bits(version);
    let mut c = Content::default();
    let mut eci: Option<u32> = None;
    // byte segments under one ECI run together (a character may span segments)
    let mut pending: Vec<u8> = Vec::new();
    let flush = |pending: &mut Vec<u8>, c: &mut Content, eci: Option<u32>| {
        if !pending.is_empty() {
            c.text.push_str(&charset_text(pending, eci));
            pending.clear();
        }
    };
    loop {
        if r.left() < 4 {
            break;
        }
        let mode = r.read(4).ok_or(Fail::Content)?;
        match mode {
            0 => break,
            1 => {
                flush(&mut pending, &mut c, eci);
                let mut n = r.read(cb[0]).ok_or(Fail::Content)? as usize;
                while n > 0 {
                    let (bits, digits) = match n {
                        1 => (4, 1),
                        2 => (7, 2),
                        _ => (10, 3),
                    };
                    let v = r.read(bits).ok_or(Fail::Content)?;
                    if v >= 10u32.pow(digits) {
                        return Err(Fail::Content);
                    }
                    let s = format!("{v:0width$}", width = digits as usize);
                    c.text.push_str(&s);
                    c.bytes.extend_from_slice(s.as_bytes());
                    n -= digits as usize;
                }
            }
            2 => {
                flush(&mut pending, &mut c, eci);
                let mut n = r.read(cb[1]).ok_or(Fail::Content)? as usize;
                let push = |k: u32, c: &mut Content| -> Result<(), Fail> {
                    let ch = *ALNUM.get(k as usize).ok_or(Fail::Content)?;
                    c.bytes.push(ch);
                    // in FNC1 mode % is GS, %% is %
                    c.text.push(char::from(ch));
                    Ok(())
                };
                while n >= 2 {
                    let v = r.read(11).ok_or(Fail::Content)?;
                    push(v / 45, &mut c)?;
                    push(v % 45, &mut c)?;
                    n -= 2;
                }
                if n == 1 {
                    let v = r.read(6).ok_or(Fail::Content)?;
                    push(v, &mut c)?;
                }
                if c.gs1 {
                    c.text = c
                        .text
                        .replace("%%", "\u{0}")
                        .replace('%', "\u{1d}")
                        .replace('\u{0}', "%");
                }
            }
            4 => {
                let n = r.read(cb[2]).ok_or(Fail::Content)? as usize;
                if r.left() < n * 8 {
                    return Err(Fail::Content);
                }
                for _ in 0..n {
                    let b = r.read(8).ok_or(Fail::Content)? as u8;
                    pending.push(b);
                    c.bytes.push(b);
                }
            }
            8 => {
                flush(&mut pending, &mut c, eci);
                c.kanji = true;
                let n = r.read(cb[3]).ok_or(Fail::Content)? as usize;
                let mut sj = Vec::with_capacity(n * 2);
                for _ in 0..n {
                    let v = r.read(13).ok_or(Fail::Content)?;
                    let w = ((v / 0xC0) << 8) | (v % 0xC0);
                    let w = if w < 0x1F00 { w + 0x8140 } else { w + 0xC140 };
                    sj.push((w >> 8) as u8);
                    sj.push(w as u8);
                }
                c.text.push_str(&encoding_rs::SHIFT_JIS.decode(&sj).0);
                c.bytes.extend_from_slice(&sj);
            }
            7 => {
                flush(&mut pending, &mut c, eci);
                let b = r.read(8).ok_or(Fail::Content)?;
                let v = if b & 0x80 == 0 {
                    b
                } else if b & 0xC0 == 0x80 {
                    ((b & 0x3F) << 8) | r.read(8).ok_or(Fail::Content)?
                } else if b & 0xE0 == 0xC0 {
                    ((b & 0x1F) << 16) | r.read(16).ok_or(Fail::Content)?
                } else {
                    return Err(Fail::Content);
                };
                eci = Some(v);
                c.eci.push(v);
            }
            3 => {
                let pos = r.read(4).ok_or(Fail::Content)? as u8;
                let total = r.read(4).ok_or(Fail::Content)? as u8 + 1;
                let parity = r.read(8).ok_or(Fail::Content)? as u8;
                c.structured = Some((pos, total, parity));
            }
            5 => c.gs1 = true,
            9 => {
                // application indicator
                let _ = r.read(8).ok_or(Fail::Content)?;
                c.gs1 = true;
            }
            _ => return Err(Fail::Content),
        }
    }
    flush(&mut pending, &mut c, eci);
    Ok(c)
}

/// Appends segments to a bit buffer (to make test symbols).
#[cfg(test)]
pub(crate) struct Writer {
    pub bits: Vec<bool>,
}

#[cfg(test)]
impl Writer {
    pub fn new() -> Writer {
        Writer { bits: Vec::new() }
    }
    pub fn put(&mut self, v: u32, n: u32) {
        for i in (0..n).rev() {
            self.bits.push(v >> i & 1 != 0);
        }
    }
    /// Pads to `cap` codewords as the standard does.
    pub fn finish(mut self, cap: usize) -> Vec<u8> {
        let room = cap * 8;
        let t = (room - self.bits.len().min(room)).min(4);
        self.put(0, t as u32);
        while !self.bits.len().is_multiple_of(8) {
            self.bits.push(false);
        }
        let mut out: Vec<u8> = self
            .bits
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | u8::from(b)))
            .collect();
        let mut pad = [0xECu8, 0x11].into_iter().cycle();
        while out.len() < cap {
            out.push(pad.next().unwrap());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_segments() {
        let mut w = Writer::new();
        // numeric "01234567" (version 1: 10-bit count)
        w.put(1, 4);
        w.put(8, 10);
        w.put(12, 10);
        w.put(345, 10);
        w.put(67, 7);
        // alphanumeric "AC-42"
        w.put(2, 4);
        w.put(5, 9);
        w.put(10 * 45 + 12, 11);
        w.put(41 * 45 + 4, 11);
        w.put(2, 6);
        // ECI 26 (UTF-8), byte "é"
        w.put(7, 4);
        w.put(26, 8);
        w.put(4, 4);
        w.put(2, 8);
        w.put(0xC3, 8);
        w.put(0xA9, 8);
        // kanji 点 (Shift_JIS 0x935F)
        w.put(8, 4);
        w.put(1, 8);
        let v = 0x935F - 0x8140;
        w.put(((v >> 8) * 0xC0) + (v & 0xFF), 13);
        let c = parse(&w.finish(40), 1).unwrap();
        assert_eq!(c.text, "01234567AC-42é点");
        assert_eq!(c.eci, vec![26]);
        assert!(c.kanji);
    }

    #[test]
    fn bytes_without_eci() {
        let mut w = Writer::new();
        w.put(4, 4);
        w.put(3, 8);
        for b in [0x41, 0xE9, 0x42] {
            w.put(b, 8);
        }
        // not UTF-8: read as ISO-8859-1
        assert_eq!(parse(&w.finish(16), 1).unwrap().text, "AéB");
    }
}

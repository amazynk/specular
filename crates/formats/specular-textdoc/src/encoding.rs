//! The character set of a text file: a byte-order mark says it; valid UTF-8 is UTF-8; DOS
//! art (`.nfo`, `.diz`) that is not UTF-8 is code page 437; anything else is guessed among
//! the legacy code pages (`specular_subs::charset`). Bytes that are not text (NULs outside
//! UTF-16, many control characters) are refused as binary.

use std::borrow::Cow;

use encoding_rs::Encoding;

/// Bytes looked at to decide.
pub const SAMPLE: usize = 64 << 10;

/// A character set the readers decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    Utf8,
    Utf16Le,
    Utf16Be,
    /// IBM PC code page 437, with its pictures for control codes (DOS art).
    Cp437,
    /// A legacy code page: Windows-125x, ISO-8859-x, KOI8, Shift_JIS, GBK, Big5, EUC.
    Legacy(&'static Encoding),
}

impl Charset {
    /// Its name as the info panel shows it.
    pub fn name(self) -> &'static str {
        match self {
            Charset::Utf8 => "UTF-8",
            Charset::Utf16Le => "UTF-16 LE",
            Charset::Utf16Be => "UTF-16 BE",
            Charset::Cp437 => "CP437 (DOS)",
            Charset::Legacy(e) => e.name(),
        }
    }

    pub fn is_utf16(self) -> bool {
        matches!(self, Charset::Utf16Le | Charset::Utf16Be)
    }

    /// Decodes `b` (no byte-order mark in it); malformed sequences become U+FFFD.
    pub fn decode(self, b: &[u8]) -> Cow<'_, str> {
        match self {
            Charset::Utf8 => String::from_utf8_lossy(b),
            Charset::Utf16Le => Cow::Owned(
                encoding_rs::UTF_16LE
                    .decode_without_bom_handling(b)
                    .0
                    .into_owned(),
            ),
            Charset::Utf16Be => Cow::Owned(
                encoding_rs::UTF_16BE
                    .decode_without_bom_handling(b)
                    .0
                    .into_owned(),
            ),
            Charset::Cp437 => Cow::Owned(b.iter().map(|&c| cp437(c)).collect()),
            Charset::Legacy(e) => e.decode_without_bom_handling(b).0,
        }
    }
}

/// What [`detect`] found: the character set and the byte-order mark's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detected {
    pub charset: Charset,
    pub bom: usize,
}

/// The characters of code page 437 above 0x7F.
const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";
/// The pictures code page 437 shows for the control codes 0x01–0x1F.
const CP437_LOW: &str = "☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼";

/// One code page 437 byte. Tab, line feed and carriage return stay themselves; NUL is a
/// space.
pub fn cp437(b: u8) -> char {
    static TABLE: std::sync::LazyLock<[char; 256]> = std::sync::LazyLock::new(|| {
        let mut t = [' '; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            *slot = (i as u8) as char;
        }
        t[0] = ' ';
        for (i, c) in CP437_LOW.chars().enumerate() {
            t[i + 1] = c;
        }
        for &b in b"\t\n\r" {
            t[usize::from(b)] = b as char;
        }
        t[0x7F] = '⌂';
        for (i, c) in CP437_HIGH.chars().enumerate() {
            t[0x80 + i] = c;
        }
        t
    });
    TABLE[usize::from(b)]
}

/// Whether `b` is valid UTF-8 but for a character cut off at its end (a sample's edge).
fn utf8_prefix(b: &[u8]) -> bool {
    match std::str::from_utf8(b) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none() && b.len() - e.valid_up_to() < 4,
    }
}

/// The character set of a file starting with `sample` (its first [`SAMPLE`] bytes or so);
/// `dos` for DOS art (`.nfo`, `.diz`), `hint` a language code for the legacy guess. `None`:
/// the bytes are not text.
pub fn detect(sample: &[u8], dos: bool, hint: Option<&str>) -> Option<Detected> {
    if let Some((enc, bom)) = Encoding::for_bom(sample) {
        let charset = if enc == encoding_rs::UTF_16LE {
            Charset::Utf16Le
        } else if enc == encoding_rs::UTF_16BE {
            Charset::Utf16Be
        } else {
            Charset::Utf8
        };
        return Some(Detected { charset, bom });
    }
    let nuls = sample.iter().filter(|&&b| b == 0).count();
    if nuls > 0 {
        // UTF-16 without a mark: one byte of most pairs is zero.
        let even = sample.iter().step_by(2).filter(|&&b| b == 0).count();
        let odd = sample
            .iter()
            .skip(1)
            .step_by(2)
            .filter(|&&b| b == 0)
            .count();
        let pairs = sample.len() / 2;
        return if pairs > 0 && odd * 10 >= pairs * 7 && even * 20 < pairs {
            Some(Detected {
                charset: Charset::Utf16Le,
                bom: 0,
            })
        } else if pairs > 0 && even * 10 >= pairs * 7 && odd * 20 < pairs {
            Some(Detected {
                charset: Charset::Utf16Be,
                bom: 0,
            })
        } else {
            None
        };
    }
    // Control characters other than layout (tab, newlines, form feed), backspace, escape
    // (terminal colours in logs) and DOS's end-of-file mark.
    let controls = sample
        .iter()
        .filter(|&&b| b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0C | 0x08 | 0x1B | 0x1A))
        .count();
    if !dos && controls * 20 > sample.len().max(1) {
        return None;
    }
    let charset = if utf8_prefix(sample) {
        Charset::Utf8
    } else if dos {
        Charset::Cp437
    } else {
        // Cut the sample at a line end so the guess does not see half a character.
        let cut = sample
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(sample.len(), |i| i + 1);
        let e = specular_subs::charset::detect(&sample[..cut], hint);
        if e == encoding_rs::UTF_8 {
            Charset::Utf8
        } else if e == encoding_rs::UTF_16LE {
            Charset::Utf16Le
        } else if e == encoding_rs::UTF_16BE {
            Charset::Utf16Be
        } else {
            Charset::Legacy(e)
        }
    };
    Some(Detected { charset, bom: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_page_437() {
        assert_eq!(CP437_HIGH.chars().count(), 128);
        assert_eq!(CP437_LOW.chars().count(), 31);
        assert_eq!(cp437(0xC9), '╔');
        assert_eq!(cp437(0xDB), '█');
        assert_eq!(cp437(0x01), '☺');
        assert_eq!(cp437(b'A'), 'A');
        assert_eq!(Charset::Cp437.decode(b"\xB0\xB1\xB2 ok\r\n"), "░▒▓ ok\r\n");
    }

    #[test]
    fn detects() {
        let d = |b: &[u8], dos| detect(b, dos, None).map(|d| (d.charset, d.bom));
        assert_eq!(d(b"\xEF\xBB\xBFhi", false), Some((Charset::Utf8, 3)));
        assert_eq!(d(b"\xFF\xFEh\0i\0", false), Some((Charset::Utf16Le, 2)));
        assert_eq!(
            d("h\0e\0l\0l\0o\0".as_bytes(), false),
            Some((Charset::Utf16Le, 0))
        );
        assert_eq!(
            d("\0h\0e\0l\0l\0o".as_bytes(), false),
            Some((Charset::Utf16Be, 0))
        );
        assert_eq!(
            d("plain ascii\n".as_bytes(), false),
            Some((Charset::Utf8, 0))
        );
        // A sample cut inside a character is still UTF-8.
        assert_eq!(d(&"שלום".as_bytes()[..7], false), Some((Charset::Utf8, 0)));
        assert_eq!(d(b"\xC9\xCD\xBB art", true), Some((Charset::Cp437, 0)));
        assert_eq!(
            d(b"caf\xE9 cr\xE8me br\xFBl\xE9e et \xE0 la carte\n", false),
            Some((Charset::Legacy(encoding_rs::WINDOWS_1252), 0))
        );
        // Binary data.
        assert_eq!(d(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR", false), None);
        assert_eq!(d(&[1, 2, 3, 4, 5, 6, 7, b'a'], false), None);
    }
}

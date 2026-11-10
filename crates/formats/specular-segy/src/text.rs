//! Textual headers: 3200 bytes of 40 card images of 80 columns, in EBCDIC (code page 037,
//! what SEG-Y rev 0 asks for) or ASCII (allowed since rev 1).

/// EBCDIC code page 037 → Unicode for the printable range `0x40..=0xFE`; control codes
/// below it (and `0xFF`) read as spaces.
const CP037_HIGH: [char; 0xC0] = [
    // 0x40
    ' ', '\u{a0}', 'â', 'ä', 'à', 'á', 'ã', 'å', 'ç', 'ñ', '¢', '.', '<', '(', '+', '|',
    // 0x50
    '&', 'é', 'ê', 'ë', 'è', 'í', 'î', 'ï', 'ì', 'ß', '!', '$', '*', ')', ';', '¬',
    // 0x60
    '-', '/', 'Â', 'Ä', 'À', 'Á', 'Ã', 'Å', 'Ç', 'Ñ', '¦', ',', '%', '_', '>', '?',
    // 0x70
    'ø', 'É', 'Ê', 'Ë', 'È', 'Í', 'Î', 'Ï', 'Ì', '`', ':', '#', '@', '\'', '=', '"',
    // 0x80
    'Ø', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', '«', '»', 'ð', 'ý', 'þ', '±',
    // 0x90
    '°', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 'ª', 'º', 'æ', '¸', 'Æ', '¤',
    // 0xA0
    'µ', '~', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '¡', '¿', 'Ð', 'Ý', 'Þ', '®',
    // 0xB0
    '^', '£', '¥', '·', '©', '§', '¶', '¼', '½', '¾', '[', ']', '¯', '¨', '´', '×',
    // 0xC0
    '{', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', '\u{ad}', 'ô', 'ö', 'ò', 'ó', 'õ',
    // 0xD0
    '}', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', '¹', 'û', 'ü', 'ù', 'ú', 'ÿ',
    // 0xE0
    '\\', '÷', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '²', 'Ô', 'Ö', 'Ò', 'Ó', 'Õ',
    // 0xF0
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '³', 'Û', 'Ü', 'Ù', 'Ú', ' ',
];

/// One EBCDIC (cp037) byte as a character; control codes as spaces.
pub fn ebcdic(b: u8) -> char {
    if b < 0x40 {
        ' '
    } else {
        CP037_HIGH[usize::from(b - 0x40)]
    }
}

/// Whether a textual header is EBCDIC rather than ASCII: the one whose reading has more
/// letters, digits and spaces wins (EBCDIC letters and digits sit at `0x81..=0xF9`, where
/// ASCII has none).
pub fn is_ebcdic(raw: &[u8]) -> bool {
    let score = |f: &dyn Fn(u8) -> char| {
        raw.iter()
            .filter(|&&b| {
                let c = f(b);
                c.is_ascii_alphanumeric() || (c == ' ' && b != 0)
            })
            .count()
    };
    let e = score(&|b| if b >= 0x40 { ebcdic(b) } else { '\0' });
    let a = score(&|b| {
        if (0x20..0x7f).contains(&b) {
            char::from(b)
        } else {
            '\0'
        }
    });
    e > a
}

/// A textual header (or extended stanza) as card images: lines of 80 columns with trailing
/// blanks removed, and trailing empty lines dropped.
pub fn decode(raw: &[u8], ebcdic_coded: bool) -> Vec<String> {
    let mut lines: Vec<String> = raw
        .chunks(80)
        .map(|card| {
            let s: String = card
                .iter()
                .map(|&b| {
                    if ebcdic_coded {
                        ebcdic(b)
                    } else if (0x20..0x7f).contains(&b) {
                        char::from(b)
                    } else if b >= 0xa0 {
                        // Latin-1 letters written by some ASCII writers.
                        char::from(b)
                    } else {
                        ' '
                    }
                })
                .collect();
            s.trim_end().to_owned()
        })
        .collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits() {
        assert_eq!(ebcdic(0xC3), 'C');
        assert_eq!(ebcdic(0x81), 'a');
        assert_eq!(ebcdic(0xE9), 'Z');
        assert_eq!(ebcdic(0xF0), '0');
        assert_eq!(ebcdic(0x40), ' ');
        assert_eq!(ebcdic(0x4B), '.');
        assert_eq!(ebcdic(0x25), ' ');
        let raw: Vec<u8> = [0xC3, 0x40, 0xF1, 0x40, 0xC3, 0xD3, 0xC9, 0xC5, 0xD5, 0xE3].to_vec();
        assert!(is_ebcdic(&raw));
        assert!(!is_ebcdic(b"C 1 CLIENT"));
        assert_eq!(decode(&raw, true), vec!["C 1 CLIENT".to_owned()]);
    }
}

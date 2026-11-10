//! Helvetica and Helvetica-Bold advance widths (the standard 14 fonts' published metrics, in
//! thousandths of the font size) for laying out markup text the way the exported PDF draws
//! it, and the WinAnsi encoding of the text drawn.

/// Widths of `' '`..=`'~'` in Helvetica.
const REGULAR: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // ' '..'/'
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // digits
    278, 278, 584, 584, 584, 556, 1015, // ':'..'@'
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667,
    611, 722, 667, 944, 667, 667, 611, // 'A'..'Z'
    278, 278, 278, 469, 556, 333, // '['..'`'
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500,
    278, 556, 500, 722, 500, 500, 500, // 'a'..'z'
    334, 260, 334, 584, // '{'..'~'
];

/// Widths of `' '`..=`'~'` in Helvetica-Bold.
const BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278,
    278, // ' '..'/'
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // digits
    333, 333, 584, 584, 584, 611, 975, // ':'..'@'
    722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667,
    611, 722, 667, 944, 667, 667, 611, // 'A'..'Z'
    333, 278, 333, 584, 556, 333, // '['..'`'
    556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556,
    333, 611, 556, 778, 556, 556, 500, // 'a'..'z'
    389, 280, 389, 584, // '{'..'~'
];

/// The advance of `c` in thousandths.
fn advance(c: char, bold: bool) -> u16 {
    let t = if bold { &BOLD } else { &REGULAR };
    match c {
        ' '..='~' => t[c as usize - 32],
        // A middle dot, as in "2026-10-24 · Name".
        '·' => 278,
        _ => 556,
    }
}

/// Width of `s` at `size`.
pub fn width(s: &str, size: f64, bold: bool) -> f64 {
    s.chars().map(|c| f64::from(advance(c, bold))).sum::<f64>() * size / 1000.0
}

/// `c` in WinAnsiEncoding (ISO 32000-1 Annex D), if it has a code.
pub fn win_ansi(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
        _ => Some(match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            _ => return None,
        }),
    }
}

/// `s` as a PDF literal string of WinAnsi codes for a content stream (`(…)`); characters
/// without a code become `?`.
pub fn literal(s: &str) -> Vec<u8> {
    let mut out = vec![b'('];
    for c in s.chars() {
        let b = win_ansi(c).unwrap_or(b'?');
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.push(b')');
    out
}

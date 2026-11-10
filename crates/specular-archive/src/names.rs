//! Entry names: decoding, normalisation and natural ordering.

use std::cmp::Ordering;

/// Natural order of `/`-separated paths, folder by folder ("a/2.jpg" < "a/10.jpg" < "b/1.jpg"),
/// with a folder's own files before its subfolders' files. `name_cmp` compares single path
/// components; the archive passes `specular_i18n::I18n::global().natural_cmp`, which makes this
/// identical to the app's folder listing (`app/specular/src/loader.rs`, `natural_path_cmp`).
pub fn natural_path_cmp(
    mut name_cmp: impl FnMut(&str, &str) -> Ordering,
    a: &str,
    b: &str,
) -> Ordering {
    let (mut ia, mut ib) = (a.split('/'), b.split('/'));
    loop {
        match (ia.next(), ib.next()) {
            (Some(x), Some(y)) => {
                // A file sorts before a folder at the same level.
                let (xf, yf) = (ia.clone().next().is_none(), ib.clone().next().is_none());
                let o = yf.cmp(&xf).then_with(|| name_cmp(x, y));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}

/// `\` to `/`, no leading `/` or `./`, no empty components, no control characters. The result
/// is only ever displayed and sorted, never used as a file-system path.
pub(crate) fn normalize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for comp in raw.split(['/', '\\']) {
        if comp.is_empty() || comp == "." {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.extend(
            comp.chars()
                .map(|c| if c.is_control() { '\u{FFFD}' } else { c }),
        );
    }
    out
}

/// Whether a raw name denotes a folder.
pub(crate) fn is_dir_name(raw: &[u8]) -> bool {
    matches!(raw.last(), Some(b'/' | b'\\'))
}

/// IBM code page 437, the ZIP default when the UTF-8 flag is clear: upper half.
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{A0}',
];

/// A ZIP name: UTF-8 when flagged or when it happens to be valid UTF-8 (many writers forget the
/// flag), else CP437.
pub(crate) fn decode_zip_name(raw: &[u8], utf8_flag: bool) -> String {
    match std::str::from_utf8(raw) {
        Ok(s) => s.to_owned(),
        Err(_) if utf8_flag => String::from_utf8_lossy(raw).into_owned(),
        Err(_) => raw
            .iter()
            .map(|&b| {
                if b < 0x80 {
                    b as char
                } else {
                    CP437_HIGH[(b - 0x80) as usize]
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize("/a//b\\c.jpg"), "a/b/c.jpg");
        assert_eq!(normalize("./x/./y.png"), "x/y.png");
        assert_eq!(normalize("../evil.jpg"), "../evil.jpg");
        assert_eq!(normalize("a\u{1}b"), "a\u{FFFD}b");
    }

    #[test]
    fn cp437() {
        assert_eq!(decode_zip_name(b"caf\x82.jpg", false), "café.jpg");
        assert_eq!(decode_zip_name("café".as_bytes(), false), "café");
    }

    #[test]
    fn path_order() {
        let cmp = |a: &str, b: &str| natural_path_cmp(|x, y| x.cmp(y), a, b);
        assert_eq!(cmp("b.jpg", "a/1.jpg"), Ordering::Less);
        assert_eq!(cmp("a/1.jpg", "b/1.jpg"), Ordering::Less);
        assert_eq!(cmp("a/1.jpg", "a/1.jpg"), Ordering::Equal);
    }
}

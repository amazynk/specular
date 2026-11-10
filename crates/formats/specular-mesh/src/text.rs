//! Byte-level helpers for the text formats (ASCII STL, OBJ, MTL, PLY headers and bodies).

/// Lines of `bytes`, split at `\n`, each without a trailing `\r`.
pub(crate) fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

/// Tokens of a line, split at ASCII white space.
pub(crate) fn tokens(line: &[u8]) -> impl Iterator<Item = &[u8]> {
    line.split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
}

/// The line without leading and trailing white space.
pub(crate) fn trim(line: &[u8]) -> &[u8] {
    line.trim_ascii()
}

pub(crate) fn f32_of(tok: &[u8]) -> Option<f32> {
    let s = std::str::from_utf8(tok).ok()?;
    s.parse::<f32>().ok().or_else(|| {
        // Some exporters write `1.#QNAN` or `-1.#IND`; read them as not-a-number so the
        // triangles that use them are dropped rather than the file refused.
        (s.contains('#')).then_some(f32::NAN)
    })
}

pub(crate) fn f64_of(tok: &[u8]) -> Option<f64> {
    std::str::from_utf8(tok).ok()?.parse::<f64>().ok()
}

pub(crate) fn i64_of(tok: &[u8]) -> Option<i64> {
    std::str::from_utf8(tok).ok()?.parse::<i64>().ok()
}

/// The text after the first token of `line` (its keyword), trimmed: a file name that may
/// hold spaces.
pub(crate) fn rest_after_keyword(line: &[u8]) -> &[u8] {
    let line = trim(line);
    match line.iter().position(|b| b.is_ascii_whitespace()) {
        Some(i) => trim(&line[i..]),
        None => &[],
    }
}

/// Lossy UTF-8 for names.
pub(crate) fn string(tok: &[u8]) -> String {
    String::from_utf8_lossy(tok).into_owned()
}

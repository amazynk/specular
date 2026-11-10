//! INI-style text headers (`[Section]` then `key = value` lines), as SkyScan logs, Nikon
//! `.xtekct` files and Volume Graphics `.vgi` files write them.

/// (section, key, value): section and key lower-case and trimmed (with spaces inside kept),
/// the value trimmed. Lines before any section have an empty section; `{…}` lines (VGI's
/// object names) start a section of that name too.
pub(crate) fn parse(data: &[u8]) -> Vec<(String, String, String)> {
    let text = String::from_utf8_lossy(data);
    let text = text.trim_start_matches('\u{feff}');
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') || t.starts_with('#') {
            continue;
        }
        if (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('{') && t.ends_with('}')) {
            section = t[1..t.len() - 1].trim().to_ascii_lowercase();
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            out.push((
                section.clone(),
                k.trim().to_ascii_lowercase(),
                v.trim().to_owned(),
            ));
        }
    }
    out
}

/// The first non-blank text of `data` (a BOM skipped), lower-case, up to `n` bytes.
pub(crate) fn start(data: &[u8], n: usize) -> String {
    let d = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    let i = d
        .iter()
        .position(|c| !c.is_ascii_whitespace())
        .unwrap_or(d.len());
    String::from_utf8_lossy(&d[i..d.len().min(i + n)]).to_ascii_lowercase()
}

/// Lookups in parsed lines.
pub(crate) struct Ini(pub Vec<(String, String, String)>);

impl Ini {
    pub fn new(data: &[u8]) -> Ini {
        Ini(parse(data))
    }

    /// The value of `key` in `section` (any section when `None`).
    pub fn get(&self, section: Option<&str>, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(s, k, _)| k == key && section.is_none_or(|x| x == s))
            .map(|(_, _, v)| v.as_str())
            .filter(|v| !v.is_empty())
    }

    pub fn num(&self, section: Option<&str>, key: &str) -> Option<f64> {
        let v = self.get(section, key)?;
        let t = v.split_whitespace().next()?;
        t.parse::<f64>().ok().filter(|x| x.is_finite())
    }

    pub fn nums(&self, section: Option<&str>, key: &str) -> Option<Vec<f64>> {
        crate::numbers(self.get(section, key)?)
    }
}

//! SerialEM `.mdoc` metadata files: the text sidecar SerialEM writes beside an MRC image
//! stack (a tilt series, a montage), from SerialEM's published description of the format.
//!
//! Lines are `Key = value`. Before the first section come the file's global values
//! (`PixelSpacing`, `ImageFile`, `ImageSize`, `DataMode`, `Voltage`…) and title lines
//! (`[T = …]`); then each section starts with a header line naming the image it describes —
//! `[ZValue = n]` (section n of the MRC file), `[MontSection = n]` or `[FrameSet = n]` — and
//! lists that image's values (`TiltAngle`, `Defocus`, `ExposureDose`, `Magnification`,
//! `DateTime`…). Values are kept as written; the few the viewer uses are read as numbers.

use std::path::{Path, PathBuf};

/// Largest `.mdoc` read.
pub const MAX_LEN: u64 = 16 << 20;

/// What a section's header names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    /// `[ZValue = n]`: section n of the image file.
    ZValue,
    /// `[MontSection = n]`: a montage's overview section.
    MontSection,
    /// `[FrameSet = n]`: a set of frames.
    FrameSet,
}

/// One section: what it describes and its values in file order.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub kind: SectionKind,
    pub index: i64,
    pub fields: Vec<(String, String)>,
}

impl Section {
    /// The value of `key` as written.
    pub fn get(&self, key: &str) -> Option<&str> {
        get(&self.fields, key)
    }

    /// The value of `key` as a finite number.
    pub fn number(&self, key: &str) -> Option<f64> {
        number(self.get(key)?)
    }

    /// The tilt angle in degrees.
    pub fn tilt(&self) -> Option<f64> {
        self.number("TiltAngle")
    }
}

/// A parsed `.mdoc` file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mdoc {
    /// The global values, in file order.
    pub global: Vec<(String, String)>,
    /// The title lines' text (inside `[T = …]`).
    pub titles: Vec<String>,
    pub sections: Vec<Section>,
}

fn get<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty())
}

fn number(s: &str) -> Option<f64> {
    s.split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

impl Mdoc {
    /// A global value as written.
    pub fn get(&self, key: &str) -> Option<&str> {
        get(&self.global, key)
    }

    /// The pixel size in Å: the global `PixelSpacing`, else the first section's.
    pub fn pixel_spacing(&self) -> Option<f64> {
        self.get("PixelSpacing")
            .and_then(number)
            .or_else(|| self.sections.iter().find_map(|s| s.number("PixelSpacing")))
            .filter(|p| *p > 0.0)
    }

    /// The `[ZValue = z]` section describing image section `z`.
    pub fn z(&self, z: usize) -> Option<&Section> {
        self.sections
            .iter()
            .find(|s| s.kind == SectionKind::ZValue && s.index == z as i64)
    }

    /// The tilt angle of each of `nz` image sections, when every one has a `[ZValue]` section
    /// with a `TiltAngle` (a tilt series).
    pub fn tilts(&self, nz: usize) -> Option<Vec<f64>> {
        (0..nz).map(|z| self.z(z)?.tilt()).collect()
    }
}

/// Parses `.mdoc` text; `None` when it has neither `Key = value` lines nor sections.
pub fn parse(text: &str) -> Option<Mdoc> {
    let mut m = Mdoc::default();
    let mut any = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let (k, v) = inner.split_once('=').unwrap_or((inner, ""));
            let (k, v) = (k.trim(), v.trim());
            let kind = match k {
                "T" => {
                    m.titles.push(v.to_owned());
                    continue;
                }
                "ZValue" => SectionKind::ZValue,
                "MontSection" => SectionKind::MontSection,
                "FrameSet" => SectionKind::FrameSet,
                _ => continue,
            };
            let Ok(index) = v.parse::<i64>() else {
                continue;
            };
            m.sections.push(Section {
                kind,
                index,
                fields: Vec::new(),
            });
            any = true;
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim().to_owned(), v.trim().to_owned());
        if k.is_empty() || k.contains(char::is_whitespace) {
            continue;
        }
        any = true;
        match m.sections.last_mut() {
            Some(s) => s.fields.push((k, v)),
            None => m.global.push((k, v)),
        }
    }
    any.then_some(m)
}

/// The `.mdoc` beside an image file: `NAME.mrc.mdoc` (SerialEM's naming), else
/// `NAME.mdoc`.
pub fn beside(image: &Path) -> Option<PathBuf> {
    let mut full = image.as_os_str().to_owned();
    full.push(".mdoc");
    let full = PathBuf::from(full);
    if full.is_file() {
        return Some(full);
    }
    let stem = image.with_extension("mdoc");
    stem.is_file().then_some(stem)
}

/// Reads and parses the `.mdoc` beside `image`, if there is one (a missing, unreadable or
/// oversized sidecar is no error).
pub fn read_beside(image: &Path) -> Option<Mdoc> {
    let p = beside(image)?;
    let meta = std::fs::metadata(&p).ok()?;
    if !meta.is_file() || meta.len() > MAX_LEN {
        return None;
    }
    let bytes = std::fs::read(&p).ok()?;
    parse(&String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "PixelSpacing = 5.4\nImageFile = TS_01.mrc\nImageSize = 924 958\n\n\
        [T = SerialEM: Digitized on a Krios    30-Nov-15  15:14:20    ]\n\n\
        [ZValue = 1]\nTiltAngle = 3.00113\nDefocus = 2.58\nDateTime = 30-Nov-15  15:22:37\n\n\
        [ZValue = 0]\nTiltAngle = 0.000999877\nExposureDose = 0\n";

    #[test]
    fn globals_titles_and_sections() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.get("ImageFile"), Some("TS_01.mrc"));
        assert_eq!(m.pixel_spacing(), Some(5.4));
        assert_eq!(m.titles.len(), 1);
        assert!(m.titles[0].starts_with("SerialEM: Digitized"));
        assert_eq!(m.sections.len(), 2);
        assert_eq!(m.tilts(2), Some(vec![0.000999877, 3.00113]));
        assert_eq!(m.tilts(3), None);
        assert_eq!(m.z(1).unwrap().get("DateTime"), Some("30-Nov-15  15:22:37"));
    }

    #[test]
    fn other_text_is_not_an_mdoc() {
        assert!(parse("just some words\nand more").is_none());
        assert!(parse("").is_none());
        // Malformed headers are skipped, not fatal.
        let m = parse("[ZValue = x]\n[ZValue = 2]\nTiltAngle = nan\n").unwrap();
        assert_eq!(m.sections.len(), 1);
        assert_eq!(m.sections[0].tilt(), None);
    }
}

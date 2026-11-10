//! Label colour tables: ITK-SNAP label description files and FreeSurfer colour LUTs, and a
//! generated palette for labels a table does not name.
//!
//! - ITK-SNAP: `IDX R G B A VIS MSH "LABEL"` per line (`A` opacity 0..1, `VIS` 0 hides).
//! - FreeSurfer (`FreeSurferColorLUT.txt` and its subsets): `No. Name R G B A` per line
//!   (`A` is transparency, 0 opaque; ignored).
//!
//! `#` starts a comment; blank lines are skipped. Either kind is told by the line: a quoted
//! name is ITK-SNAP's.

/// One entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub value: i64,
    pub name: String,
    /// sRGB.
    pub colour: [u8; 3],
    /// Shown at first (ITK-SNAP's VIS; FreeSurfer: always).
    pub visible: bool,
}

/// Which kind a table was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ItkSnap,
    FreeSurfer,
}

/// A colour table.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut {
    pub kind: Kind,
    /// Sorted by value, unique.
    pub entries: Vec<Entry>,
}

/// Most entries read.
pub const MAX_ENTRIES: usize = 65_536;

/// Why a table could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Line (1-based) that is neither kind's.
    Line(usize),
    /// No entries.
    Empty,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Line(n) => write!(f, "line {n} is not a colour table entry"),
            Error::Empty => f.write_str("no entries"),
        }
    }
}

fn channel(s: &str) -> Option<u8> {
    let v: f64 = s.parse().ok()?;
    (0.0..=255.0).contains(&v).then(|| v.round() as u8)
}

impl Lut {
    pub fn parse(text: &str) -> Result<Lut, Error> {
        let mut entries: Vec<Entry> = Vec::new();
        let mut kind = None;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim_start_matches('\u{feff}');
            let body = line.split('#').next().unwrap_or("").trim();
            if body.is_empty() {
                continue;
            }
            let bad = Error::Line(n + 1);
            let e = if let Some(q) = body.find('"') {
                let name = body[q + 1..].trim_end().trim_end_matches('"').to_owned();
                let t: Vec<&str> = body[..q].split_whitespace().collect();
                if t.len() < 4 {
                    return Err(bad);
                }
                kind.get_or_insert(Kind::ItkSnap);
                Entry {
                    value: t[0].parse().map_err(|_| bad.clone())?,
                    name,
                    colour: [
                        channel(t[1]).ok_or(bad.clone())?,
                        channel(t[2]).ok_or(bad.clone())?,
                        channel(t[3]).ok_or(bad.clone())?,
                    ],
                    visible: t.get(5).is_none_or(|v| *v != "0"),
                }
            } else {
                let t: Vec<&str> = body.split_whitespace().collect();
                if t.len() < 5 {
                    return Err(bad);
                }
                kind.get_or_insert(Kind::FreeSurfer);
                Entry {
                    value: t[0].parse().map_err(|_| bad.clone())?,
                    name: t[1].to_owned(),
                    colour: [
                        channel(t[2]).ok_or(bad.clone())?,
                        channel(t[3]).ok_or(bad.clone())?,
                        channel(t[4]).ok_or(bad.clone())?,
                    ],
                    visible: true,
                }
            };
            entries.push(e);
            if entries.len() >= MAX_ENTRIES {
                break;
            }
        }
        if entries.is_empty() {
            return Err(Error::Empty);
        }
        entries.sort_by_key(|e| e.value);
        entries.dedup_by_key(|e| e.value);
        Ok(Lut {
            kind: kind.unwrap_or(Kind::FreeSurfer),
            entries,
        })
    }

    pub fn get(&self, value: i64) -> Option<&Entry> {
        self.entries
            .binary_search_by_key(&value, |e| e.value)
            .ok()
            .map(|i| &self.entries[i])
    }
}

/// The colour of label `value` when no table names it: golden-angle hues, so neighbouring
/// values differ.
pub fn palette(value: i64) -> [u8; 3] {
    let h = (value.rem_euclid(1 << 20) as f64 * 137.507_764) % 360.0;
    // Alternate the lightness a little so close hues still part.
    let v = if value % 2 == 0 { 0.95 } else { 0.8 };
    hsv(h, 0.7, v)
}

fn hsv(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r, g, b].map(|t| ((t + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn itk_snap_files() {
        let text = "################################################\n\
# ITK-SnAP Label Description File\n\
# IDX   -R-  -G-  -B-  -A--  VIS MSH  LABEL\n\
    0     0    0    0        0  0  0    \"Clear Label\"\n\
    1   255    0    0        1  1  1    \"Liver\"\n\
    2     0  255    0        1  0  1    \"Tumour core\"\n";
        let l = Lut::parse(text).unwrap();
        assert_eq!(l.kind, Kind::ItkSnap);
        assert_eq!(l.entries.len(), 3);
        let t = l.get(2).unwrap();
        assert_eq!(
            (t.name.as_str(), t.colour, t.visible),
            ("Tumour core", [0, 255, 0], false)
        );
        assert_eq!(l.get(1).unwrap().name, "Liver");
        assert!(l.get(3).is_none());
    }

    #[test]
    fn freesurfer_files() {
        let text = "#$Id: FreeSurferColorLUT.txt\n\n#No. Label Name:                            R   G   B   A\n\n\
0   Unknown                                 0   0   0   0\n\
2   Left-Cerebral-White-Matter              245 245 245 0\n\
17  Left-Hippocampus                        220 216 20  0\n";
        let l = Lut::parse(text).unwrap();
        assert_eq!(l.kind, Kind::FreeSurfer);
        assert_eq!(l.get(17).unwrap().colour, [220, 216, 20]);
        assert_eq!(l.get(2).unwrap().name, "Left-Cerebral-White-Matter");
        assert_eq!(Lut::parse("# nothing\n"), Err(Error::Empty));
        assert_eq!(Lut::parse("1 a b\n"), Err(Error::Line(1)));
        assert_eq!(Lut::parse("1 Name 300 0 0 0\n"), Err(Error::Line(1)));
    }

    #[test]
    fn palette_parts_neighbours() {
        for v in 1..64 {
            assert_ne!(palette(v), palette(v + 1));
        }
    }
}

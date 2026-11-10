//! Chart reference files: CGATS.17 / ISO 28178 text (X-Rite and Wolf Faust reference files,
//! Argyll `.ti3`/`.cie`), written from the public descriptions of the format.
//!
//! - Keywords, then `BEGIN_DATA_FORMAT` … `END_DATA_FORMAT` naming the fields, then
//!   `BEGIN_DATA` … `END_DATA` with one patch per row. `#` starts a comment; values may be
//!   quoted.
//! - The patch name is `SAMPLE_ID` (or `SAMPLE_NAME` when there is no id, or when the ids are
//!   plain row numbers next to names).
//! - Colour: `LAB_L LAB_A LAB_B`, else `XYZ_X XYZ_Y XYZ_Z` (0–100). The first table with
//!   either is read.
//! - `ILLUMINANT` (or `MEASUREMENT_SOURCE` naming one) is D50 when absent. D65 values are
//!   adapted to D50 by Bradford; other illuminants are refused.
//! - Datacolor SpyderCHECKR values are proprietary: a file that names Datacolor or Spyder is
//!   refused until there is a decision on them.

use super::colour::{self, Lab};

/// Largest reference file read.
pub const MAX_BYTES: usize = 4 << 20;

/// One reference patch: its name and L*a*b* (D50, 2° observer).
#[derive(Clone, Debug, PartialEq)]
pub struct RefPatch {
    pub id: String,
    pub lab: Lab,
}

/// A parsed reference file.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    /// `ORIGINATOR`, `DESCRIPTOR` or the first line: what the file says it is.
    pub title: String,
    pub patches: Vec<RefPatch>,
}

/// Why a reference file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CgatsError {
    TooLarge,
    /// Not text, or no CGATS structure at all.
    NotCgats,
    /// No `BEGIN_DATA_FORMAT` … `END_DATA_FORMAT` block.
    NoFormat,
    /// No `SAMPLE_ID` or `SAMPLE_NAME` field.
    NoIds,
    /// Neither L*a*b* nor XYZ fields.
    NoColour,
    /// No `BEGIN_DATA` … `END_DATA` block, or it is empty.
    NoData,
    /// Data row `row` (1-based) has `found` values, not `expected`.
    BadRow {
        row: usize,
        found: usize,
        expected: usize,
    },
    /// Data row `row` has a value that is not a number where a number belongs.
    BadNumber {
        row: usize,
    },
    /// `NUMBER_OF_SETS` says `declared`, the data has `found` rows.
    Count {
        declared: usize,
        found: usize,
    },
    /// Two patches have the same name.
    Duplicate {
        id: String,
    },
    /// Values for an illuminant other than D50 or D65.
    Illuminant {
        name: String,
    },
    /// A Datacolor SpyderCHECKR file.
    Datacolor,
}

impl std::fmt::Display for CgatsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CgatsError::TooLarge => write!(f, "the file is too large for a reference file"),
            CgatsError::NotCgats => write!(f, "not a CGATS reference file"),
            CgatsError::NoFormat => write!(f, "no data format block"),
            CgatsError::NoIds => write!(f, "no patch names (SAMPLE_ID)"),
            CgatsError::NoColour => write!(f, "no L*a*b* or XYZ values"),
            CgatsError::NoData => write!(f, "no data rows"),
            CgatsError::BadRow {
                row,
                found,
                expected,
            } => {
                write!(f, "row {row} has {found} values, expected {expected}")
            }
            CgatsError::BadNumber { row } => {
                write!(f, "row {row} has a value that is not a number")
            }
            CgatsError::Count { declared, found } => {
                write!(f, "the file says {declared} patches but has {found}")
            }
            CgatsError::Duplicate { id } => write!(f, "patch {id} appears twice"),
            CgatsError::Illuminant { name } => write!(f, "values for illuminant {name}"),
            CgatsError::Datacolor => write!(f, "Datacolor SpyderCHECKR values"),
        }
    }
}

impl std::error::Error for CgatsError {}

/// Splits a line into tokens; quoted strings are one token (quotes removed). A `#` outside
/// quotes ends the line.
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had = false;
    for ch in line.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                had = true;
            }
            '#' if !quoted => break,
            c if c.is_whitespace() && !quoted => {
                if had || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                had = false;
            }
            c => cur.push(c),
        }
    }
    if had || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn number(s: &str) -> Option<f64> {
    // Some writers use a decimal comma.
    s.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Parses a reference file.
pub fn parse(bytes: &[u8]) -> Result<Reference, CgatsError> {
    if bytes.len() > MAX_BYTES {
        return Err(CgatsError::TooLarge);
    }
    let text = std::str::from_utf8(bytes)
        .map(str::to_owned)
        // Latin-1 files (names with accents): each byte is its code point.
        .unwrap_or_else(|_| bytes.iter().map(|&b| char::from(b)).collect());
    if text.contains('\0') {
        return Err(CgatsError::NotCgats);
    }
    let mut lines = text
        .lines()
        .map(tokens)
        .filter(|t| !t.is_empty())
        .peekable();
    let Some(first) = lines.peek().cloned() else {
        return Err(CgatsError::NotCgats);
    };
    let mut keywords: Vec<(String, String)> = Vec::new();
    let mut formats: Vec<Vec<String>> = Vec::new();
    let mut tables: Vec<(Vec<Vec<String>>, Option<usize>)> = Vec::new();
    let mut sets: Option<usize> = None;
    let mut saw_structure = false;
    while let Some(t) = lines.next() {
        let key = t[0].to_ascii_uppercase();
        match key.as_str() {
            "BEGIN_DATA_FORMAT" => {
                saw_structure = true;
                let mut fields = t[1..].to_vec();
                for t in lines.by_ref() {
                    if t[0].eq_ignore_ascii_case("END_DATA_FORMAT") {
                        break;
                    }
                    fields.extend(t);
                }
                formats.push(fields.iter().map(|f| f.to_ascii_uppercase()).collect());
            }
            "BEGIN_DATA" => {
                saw_structure = true;
                let mut rows = Vec::new();
                for t in lines.by_ref() {
                    if t[0].eq_ignore_ascii_case("END_DATA") {
                        break;
                    }
                    rows.push(t);
                }
                tables.push((rows, sets.take()));
            }
            "NUMBER_OF_SETS" => sets = t.get(1).and_then(|v| v.parse().ok()),
            _ => {
                let value = t[1..].join(" ");
                keywords.push((key, value));
            }
        }
    }
    if !saw_structure {
        return Err(CgatsError::NotCgats);
    }
    let kw = |k: &str| {
        keywords
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    };
    let all_text = keywords
        .iter()
        .map(|(a, b)| format!("{a} {b}"))
        .chain(std::iter::once(first.join(" ")))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if all_text.contains("datacolor") || all_text.contains("spyder") {
        return Err(CgatsError::Datacolor);
    }
    if formats.is_empty() {
        return Err(CgatsError::NoFormat);
    }
    let illuminant = kw("ILLUMINANT")
        .or_else(|| kw("MEASUREMENT_SOURCE"))
        .map(|s| s.to_ascii_uppercase());
    let adapt = match illuminant.as_deref() {
        None => false,
        Some(s) if s.contains("D50") || s.is_empty() => false,
        Some(s) if s.contains("D65") => true,
        // Measurement conditions (M0/M1/M2) name the instrument's light, not the values'.
        Some(s) if s.starts_with('M') && s.len() <= 3 => false,
        Some(s) => return Err(CgatsError::Illuminant { name: s.to_owned() }),
    };
    // The first format with patch names and colour, and its table.
    let mut last_err = CgatsError::NoColour;
    for (fi, fields) in formats.iter().enumerate() {
        let pos = |name: &str| fields.iter().position(|f| f == name);
        let id = match (pos("SAMPLE_ID"), pos("SAMPLE_NAME")) {
            (Some(i), _) | (None, Some(i)) => i,
            (None, None) => {
                last_err = CgatsError::NoIds;
                continue;
            }
        };
        let name = pos("SAMPLE_NAME");
        let lab = [pos("LAB_L"), pos("LAB_A"), pos("LAB_B")];
        let xyz = [pos("XYZ_X"), pos("XYZ_Y"), pos("XYZ_Z")];
        let (cols, is_lab) = match (lab, xyz) {
            ([Some(a), Some(b), Some(c)], _) => ([a, b, c], true),
            (_, [Some(a), Some(b), Some(c)]) => ([a, b, c], false),
            _ => {
                last_err = CgatsError::NoColour;
                continue;
            }
        };
        let Some((rows, declared)) = tables.get(fi).or(tables.last()) else {
            return Err(CgatsError::NoData);
        };
        if rows.is_empty() {
            return Err(CgatsError::NoData);
        }
        if let Some(d) = declared
            && *d != rows.len()
        {
            return Err(CgatsError::Count {
                declared: *d,
                found: rows.len(),
            });
        }
        // Plain row numbers as ids next to real names: the names carry the layout.
        let numeric_ids = rows
            .iter()
            .enumerate()
            .all(|(i, r)| r.get(id).and_then(|v| v.parse::<usize>().ok()) == Some(i + 1));
        let id = match name {
            Some(n) if numeric_ids && n != id && looks_like_grid_names(rows, n) => n,
            _ => id,
        };
        let mut patches: Vec<RefPatch> = Vec::with_capacity(rows.len());
        for (r, row) in rows.iter().enumerate() {
            if row.len() != fields.len() {
                return Err(CgatsError::BadRow {
                    row: r + 1,
                    found: row.len(),
                    expected: fields.len(),
                });
            }
            let v: Option<Vec<f64>> = cols.iter().map(|&c| number(&row[c])).collect();
            let Some(v) = v else {
                return Err(CgatsError::BadNumber { row: r + 1 });
            };
            let mut lab = if is_lab {
                [v[0], v[1], v[2]]
            } else {
                // XYZ 0–100 (some files 0–1: a white with Y ≤ 1.5).
                [v[0], v[1], v[2]]
            };
            if !is_lab {
                let scale = 100.0;
                lab = colour::xyz_to_lab([lab[0] / scale, lab[1] / scale, lab[2] / scale]);
            }
            let pid = row[id].trim().to_owned();
            if patches.iter().any(|p| p.id.eq_ignore_ascii_case(&pid)) {
                return Err(CgatsError::Duplicate { id: pid });
            }
            patches.push(RefPatch { id: pid, lab });
        }
        if !is_lab {
            // XYZ given as 0–1: rescale (Lab computed from Y in hundreds would be near black).
            let max_y = rows
                .iter()
                .filter_map(|r| number(&r[cols[1]]))
                .fold(0f64, f64::max);
            if max_y <= 1.5 {
                for (p, r) in patches.iter_mut().zip(rows) {
                    let v: Vec<f64> = cols.iter().filter_map(|&c| number(&r[c])).collect();
                    p.lab = colour::xyz_to_lab([v[0], v[1], v[2]]);
                }
            }
        }
        if adapt {
            for p in &mut patches {
                p.lab = colour::d65_lab_to_d50(p.lab);
            }
        }
        let title = kw("DESCRIPTOR")
            .or_else(|| kw("ORIGINATOR"))
            .map(str::to_owned)
            .unwrap_or_else(|| first.join(" "));
        return Ok(Reference { title, patches });
    }
    Err(last_err)
}

/// Whether column `n` holds grid names such as `A1` or `GS3`.
fn looks_like_grid_names(rows: &[Vec<String>], n: usize) -> bool {
    rows.iter().all(|r| {
        r.get(n).is_some_and(|s| {
            let letters = s.chars().take_while(char::is_ascii_alphabetic).count();
            letters > 0 && letters < s.len() && s[letters..].chars().all(|c| c.is_ascii_digit())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XRITE: &str = "CGATS.17\nORIGINATOR\t\"X-Rite\"\nDESCRIPTOR \"ColorChecker 24\"\n\
        NUMBER_OF_FIELDS 4\nBEGIN_DATA_FORMAT\nSAMPLE_ID\tLAB_L\tLAB_A\tLAB_B\nEND_DATA_FORMAT\n\
        NUMBER_OF_SETS 2\nBEGIN_DATA\nA1\t37.54\t14.37\t14.92\nA2\t64.66\t19.27\t17.5\nEND_DATA\n";

    #[test]
    fn xrite_style_lab() {
        let r = parse(XRITE.as_bytes()).unwrap();
        assert_eq!(r.title, "ColorChecker 24");
        assert_eq!(r.patches.len(), 2);
        assert_eq!(r.patches[1].id, "A2");
        assert_eq!(r.patches[0].lab, [37.54, 14.37, 14.92]);
    }

    #[test]
    fn wolf_faust_style_xyz_and_lab_with_comments() {
        let f = "IT8.7/2\nORIGINATOR \"Wolf Faust\"\nDESCRIPTOR \"R140501\"\n# a comment\n\
            MANUFACTURER \"Wolf Faust\"\nILLUMINANT \"D50\"\nNUMBER_OF_FIELDS 8\n\
            BEGIN_DATA_FORMAT\nSAMPLE_ID XYZ_X XYZ_Y XYZ_Z LAB_L LAB_A LAB_B STDEV_DE\n\
            END_DATA_FORMAT\nNUMBER_OF_SETS 2\nBEGIN_DATA\n\
            A01 11.45 9.69 6.76 37.29 15.74 9.32 0.3 # first\nGS0 81.2 84.2 69.1 93.5 -0.8 1.9 0.2\n\
            END_DATA\n";
        let r = parse(f.as_bytes()).unwrap();
        assert_eq!(r.title, "R140501");
        assert_eq!(r.patches[0].lab, [37.29, 15.74, 9.32]);
        assert_eq!(r.patches[1].id, "GS0");
    }

    #[test]
    fn argyll_ti3_style_xyz_only() {
        let f = "CTI3\n\nDESCRIPTOR \"Argyll Calibration Target chart information 3\"\n\
            ORIGINATOR \"Argyll target\"\nDEVICE_CLASS \"INPUT\"\nCOLOR_REP \"XYZ_RGB\"\n\n\
            NUMBER_OF_FIELDS 7\nBEGIN_DATA_FORMAT\nSAMPLE_ID XYZ_X XYZ_Y XYZ_Z RGB_R RGB_G RGB_B \n\
            END_DATA_FORMAT\n\nNUMBER_OF_SETS 1\nBEGIN_DATA\n\
            1 \"96.42\" 100.0 82.49 90.1 91.2 89.0\nEND_DATA\n";
        let r = parse(f.as_bytes()).unwrap();
        let lab = r.patches[0].lab;
        assert!((lab[0] - 100.0).abs() < 1e-3 && lab[1].abs() < 0.05 && lab[2].abs() < 0.05);
    }

    #[test]
    fn names_carry_the_layout_when_ids_are_row_numbers() {
        let f = "CGATS.17\nBEGIN_DATA_FORMAT\nSAMPLE_ID SAMPLE_NAME LAB_L LAB_A LAB_B\n\
            END_DATA_FORMAT\nBEGIN_DATA\n1 A1 37 14 14\n2 B1 65 19 17\nEND_DATA\n";
        let r = parse(f.as_bytes()).unwrap();
        assert_eq!(r.patches[1].id, "B1");
    }

    #[test]
    fn malformed_files_are_refused_with_reasons() {
        let fmt = "CGATS.17\nBEGIN_DATA_FORMAT\nSAMPLE_ID LAB_L LAB_A LAB_B\nEND_DATA_FORMAT\n";
        let cases: Vec<(String, CgatsError)> = vec![
            ("hello world".into(), CgatsError::NotCgats),
            (String::new(), CgatsError::NotCgats),
            ("CGATS.17\nBEGIN_DATA\n1 2\nEND_DATA\n".into(), CgatsError::NoFormat),
            (
                "CGATS.17\nBEGIN_DATA_FORMAT\nLAB_L LAB_A LAB_B\nEND_DATA_FORMAT\nBEGIN_DATA\n1 2 3\nEND_DATA\n".into(),
                CgatsError::NoIds,
            ),
            (
                "CGATS.17\nBEGIN_DATA_FORMAT\nSAMPLE_ID RGB_R\nEND_DATA_FORMAT\nBEGIN_DATA\nA 2\nEND_DATA\n".into(),
                CgatsError::NoColour,
            ),
            (format!("{fmt}BEGIN_DATA\nEND_DATA\n"), CgatsError::NoData),
            (
                format!("{fmt}BEGIN_DATA\nA1 1 2\nEND_DATA\n"),
                CgatsError::BadRow { row: 1, found: 3, expected: 4 },
            ),
            (
                format!("{fmt}BEGIN_DATA\nA1 1 x 2\nEND_DATA\n"),
                CgatsError::BadNumber { row: 1 },
            ),
            (
                format!("{fmt}NUMBER_OF_SETS 3\nBEGIN_DATA\nA1 1 2 3\nEND_DATA\n"),
                CgatsError::Count { declared: 3, found: 1 },
            ),
            (
                format!("{fmt}BEGIN_DATA\nA1 1 2 3\na1 1 2 3\nEND_DATA\n"),
                CgatsError::Duplicate { id: "a1".into() },
            ),
            (
                format!("ILLUMINANT \"F11\"\n{fmt}BEGIN_DATA\nA1 1 2 3\nEND_DATA\n"),
                CgatsError::Illuminant { name: "F11".into() },
            ),
            (
                format!("ORIGINATOR \"Datacolor\"\n{fmt}BEGIN_DATA\nA1 1 2 3\nEND_DATA\n"),
                CgatsError::Datacolor,
            ),
        ];
        for (text, want) in cases {
            assert_eq!(parse(text.as_bytes()), Err(want.clone()), "{text:?}");
            assert!(!want.to_string().is_empty());
        }
        assert_eq!(parse(&vec![b' '; MAX_BYTES + 1]), Err(CgatsError::TooLarge));
    }

    #[test]
    fn d65_values_are_adapted_to_d50() {
        let f = "CGATS.17\nILLUMINANT D65\nBEGIN_DATA_FORMAT\nSAMPLE_ID LAB_L LAB_A LAB_B\n\
            END_DATA_FORMAT\nBEGIN_DATA\nW 100 0 0\nEND_DATA\n";
        let r = parse(f.as_bytes()).unwrap();
        // White stays white in its own illuminant's L*a*b*.
        let lab = r.patches[0].lab;
        assert!((lab[0] - 100.0).abs() < 1e-6 && lab[1].abs() < 1e-6 && lab[2].abs() < 1e-6);
    }
}

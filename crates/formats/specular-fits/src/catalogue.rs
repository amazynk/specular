//! Source catalogues to overlay on sky images: positions and the fields of each row, from
//! a VOTable (IVOA VOTable 1.4: `TABLEDATA`, `BINARY`, `BINARY2`), a FITS table, or
//! delimited text (CSV, TSV, whitespace; VizieR's unit and dash lines are skipped).
//!
//! The position columns are found by UCD (`pos.eq.ra;meta.main`, `POS_EQ_RA_MAIN`,
//! `pos.galactic.lon`) or by name (`RA`, `RAJ2000`, `RA_ICRS`, `ALPHA_J2000`, `DEC`,
//! `DEJ2000`, `GLON`…); values in degrees, or sexagesimal (`12:34:56.7`, `12 34 56.7`,
//! `12h34m56.7s`; right ascension in hours, declination in degrees). At most
//! [`MAX_ROWS`] rows are kept.
//!
//! Equatorial positions are ICRS (FK5 J2000 taken as the same) unless the table says FK4
//! B1950: a VOTable `COOSYS` with `system="eq_FK4"` (or `FK4`) at equinox B1950, a FITS
//! table with `RADESYS = 'FK4'` or `EQUINOX = 1950`, or B1950 position column names
//! (`RAB1950`, `DEB1950`). FK4 at other equinoxes is taken as ICRS.

use crate::table::Table;
use crate::{Error, Kind, Result};

/// Most rows read from a catalogue.
pub const MAX_ROWS: usize = 200_000;

/// A catalogue: positions and every field of each row as text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalogue {
    /// The table's name, when the file gives one.
    pub name: Option<String>,
    /// Column headings (with the unit in brackets, when given).
    pub columns: Vec<String>,
    /// Longitude / latitude (degrees) of each kept row, and the row's fields.
    pub lng: Vec<f64>,
    pub lat: Vec<f64>,
    pub rows: Vec<Vec<String>>,
    /// Galactic coordinates rather than equatorial.
    pub galactic: bool,
    /// Equatorial positions in FK4 at equinox and epoch B1950 rather than ICRS.
    pub fk4: bool,
    /// The position columns.
    pub position: (usize, usize),
    /// Rows left out past [`MAX_ROWS`].
    pub dropped: usize,
}

/// One column's description.
#[derive(Clone, Debug, Default)]
struct Col {
    name: String,
    unit: String,
    ucd: String,
    /// The column's own frame is FK4 B1950 (a VOTable `COOSYS` or a B1950 name).
    b1950: bool,
}

impl Col {
    fn heading(&self) -> String {
        if self.unit.is_empty() {
            self.name.clone()
        } else {
            format!("{} [{}]", self.name, self.unit)
        }
    }
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

const RA_NAMES: &[&str] = &[
    "ra",
    "raj2000",
    "raicrs",
    "radeg",
    "rad",
    "ra2000",
    "alphaj2000",
    "alpha",
    "racen",
    "rightascension",
    "raobj",
    "ra1",
    "rab1950",
    "ra1950",
];
const DEC_NAMES: &[&str] = &[
    "dec",
    "dej2000",
    "decj2000",
    "deicrs",
    "decicrs",
    "decdeg",
    "decd",
    "dedeg",
    "de2000",
    "dec2000",
    "deltaj2000",
    "delta",
    "deccen",
    "declination",
    "decl",
    "decobj",
    "de",
    "dec1",
    "deb1950",
    "decb1950",
    "de1950",
    "dec1950",
];
const GLON_NAMES: &[&str] = &["glon", "lii", "gall", "galacticlongitude"];
const GLAT_NAMES: &[&str] = &["glat", "bii", "galb", "galacticlatitude"];

/// The longitude / latitude columns: by main UCD, UCD, then name; galactic last.
fn find_position(cols: &[Col]) -> Option<(usize, usize, bool)> {
    let ucd = |want: &[&str], main: bool| {
        cols.iter().position(|c| {
            let u = c.ucd.to_ascii_lowercase();
            let first = u.split(';').next().unwrap_or("");
            want.contains(&first) && (!main || u.contains("meta.main") || u.ends_with("_main"))
        })
    };
    for main in [true, false] {
        if let (Some(a), Some(b)) = (
            ucd(&["pos.eq.ra", "pos_eq_ra_main", "pos_eq_ra"], main),
            ucd(&["pos.eq.dec", "pos_eq_dec_main", "pos_eq_dec"], main),
        ) {
            return Some((a, b, false));
        }
    }
    if let (Some(a), Some(b)) = (
        ucd(&["pos.galactic.lon", "pos_gal_lon"], false),
        ucd(&["pos.galactic.lat", "pos_gal_lat"], false),
    ) {
        return Some((a, b, true));
    }
    let by_name = |names: &[&str]| {
        names
            .iter()
            .find_map(|n| cols.iter().position(|c| norm(&c.name) == *n))
    };
    if let (Some(a), Some(b)) = (by_name(RA_NAMES), by_name(DEC_NAMES)) {
        return Some((a, b, false));
    }
    if let (Some(a), Some(b)) = (by_name(GLON_NAMES), by_name(GLAT_NAMES)) {
        return Some((a, b, true));
    }
    None
}

/// An angle in degrees: decimal, or sexagesimal (`hours` for a right ascension unless the
/// unit says degrees).
pub fn angle(s: &str, hours: bool) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Ok(v) = t.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    let (neg, body) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let lettered = body.contains(['h', 'd', 'm', 's', '°', '\'', '"']);
    let parts: Vec<&str> = body
        .split(|c: char| c == ':' || c.is_whitespace() || "hdms°'\"".contains(c))
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() || parts.len() > 3 || (parts.len() == 1 && !lettered) {
        return None;
    }
    let mut v = 0.0;
    let mut f = 1.0;
    for p in &parts {
        let x: f64 = p.parse().ok()?;
        if !x.is_finite() || x < 0.0 {
            return None;
        }
        v += x / f;
        f *= 60.0;
    }
    let hrs = if body.contains('h') {
        true
    } else if body.contains(['d', '°']) {
        false
    } else {
        hours
    };
    let v = if hrs { v * 15.0 } else { v };
    Some(if neg { -v } else { v })
}

/// Reads a catalogue from a file's bytes.
pub fn parse(bytes: &[u8]) -> Result<Catalogue> {
    if crate::sniff(bytes) {
        return from_fits(bytes);
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let h = head.trim_start_matches('\u{feff}').trim_start();
    if h.starts_with('<') && (head.contains("VOTABLE") || head.contains("votable")) {
        return votable(bytes);
    }
    delimited(bytes)
}

/// Collects rows: positions from the chosen columns, fields as text.
struct Builder {
    cat: Catalogue,
    hours: bool,
    lat_hours: bool,
}

impl Builder {
    fn new(cols: &[Col], name: Option<String>) -> Result<Builder> {
        let (a, b, galactic) = find_position(cols)
            .ok_or_else(|| Error::Unsupported("no RA / Dec columns in the catalogue".into()))?;
        let unit = cols[a].unit.to_ascii_lowercase();
        let hours = !galactic
            && (unit.starts_with('h')
                || unit.contains("hourangle")
                || unit.is_empty()
                || unit.contains("h:m"));
        let lat_hours = false;
        let fk4 = !galactic && (cols[a].b1950 || cols[b].b1950);
        Ok(Builder {
            cat: Catalogue {
                name,
                columns: cols.iter().map(Col::heading).collect(),
                galactic,
                fk4,
                position: (a, b),
                ..Catalogue::default()
            },
            hours,
            lat_hours,
        })
    }

    /// Adds a row given as text; rows without a position are skipped.
    fn push_text(&mut self, fields: Vec<String>) {
        let (a, b) = self.cat.position;
        let pos = (
            fields.get(a).and_then(|s| angle(s, self.hours)),
            fields.get(b).and_then(|s| angle(s, self.lat_hours)),
        );
        if let (Some(l), Some(d)) = pos {
            self.push(l, d, fields);
        }
    }

    fn push(&mut self, lng: f64, lat: f64, fields: Vec<String>) {
        if !(-90.0..=90.0).contains(&lat) || !lng.is_finite() {
            return;
        }
        if self.cat.lng.len() >= MAX_ROWS {
            self.cat.dropped += 1;
            return;
        }
        self.cat.lng.push(lng.rem_euclid(360.0));
        self.cat.lat.push(lat);
        self.cat.rows.push(fields);
    }

    fn finish(self) -> Result<Catalogue> {
        if self.cat.lng.is_empty() {
            return Err(Error::Unsupported("no positions in the catalogue".into()));
        }
        Ok(self.cat)
    }
}

/// A position column named for B1950 (`RAB1950`, `_DE1950`, `RA_FK4`).
fn b1950_name(name: &str) -> bool {
    let n = norm(name);
    n.contains("1950") || n.contains("fk4")
}

/// A table header that puts its equatorial positions in FK4 B1950: `RADESYS` (or the
/// older `RADECSYS`) `'FK4'` at equinox 1950 or none, or no frame and `EQUINOX` (or
/// `EPOCH`) 1950. A J2000 or ICRS column name wins over the header.
fn header_fk4(h: &crate::Header, ra_name: &str) -> bool {
    let n = norm(ra_name);
    if n.contains("2000") || n.contains("icrs") {
        return false;
    }
    let equinox = h.f64("EQUINOX").or_else(|| h.f64("EPOCH"));
    let near_1950 = |e: f64| (e - 1950.0).abs() < 1e-6;
    match h
        .str("RADESYS")
        .or_else(|| h.str("RADECSYS"))
        .map(|s| s.trim().to_ascii_uppercase())
    {
        Some(s) if !s.is_empty() => s == "FK4" && equinox.is_none_or(near_1950),
        _ => equinox.is_some_and(near_1950),
    }
}

/// A VOTable `COOSYS` puts its positions in FK4 B1950: `system` `eq_FK4` (VOTable 1.1+)
/// or `FK4` (VOTable 1.0) with `equinox` B1950 or none.
fn coosys_fk4(system: Option<&str>, equinox: Option<&str>) -> bool {
    let fk4 = system.is_some_and(|s| {
        let s = s.trim();
        s.eq_ignore_ascii_case("eq_FK4") || s.eq_ignore_ascii_case("FK4")
    });
    let b1950 = equinox.is_none_or(|e| {
        let e = e.trim().trim_start_matches(['B', 'b']);
        e.parse::<f64>().is_ok_and(|v| (v - 1950.0).abs() < 1e-6)
    });
    fk4 && b1950
}

/// The first table HDU with positions.
fn from_fits(bytes: &[u8]) -> Result<Catalogue> {
    let f = crate::parse(bytes)?;
    let mut last = Error::Unsupported("no table in the FITS file".into());
    for hdu in f
        .hdus
        .iter()
        .filter(|h| matches!(h.kind, Kind::BinTable | Kind::Table))
    {
        let t = match Table::from_hdu(hdu) {
            Ok(t) => t,
            Err(e) => {
                last = e;
                continue;
            }
        };
        let cols: Vec<Col> = t
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| Col {
                name: c.name.clone(),
                unit: c.unit.clone(),
                ucd: hdu
                    .header
                    .str(&format!("TUCD{}", i + 1))
                    .unwrap_or("")
                    .trim()
                    .to_owned(),
                b1950: b1950_name(&c.name),
            })
            .collect();
        let mut b = match Builder::new(&cols, t.name.clone()) {
            Ok(b) => b,
            Err(e) => {
                last = e;
                continue;
            }
        };
        // The table's header may put its equatorial columns in FK4 B1950.
        if !b.cat.galactic && !b.cat.fk4 {
            b.cat.fk4 = header_fk4(&hdu.header, &cols[b.cat.position.0].name);
        }
        // Numeric position columns are degrees.
        let (ca, cb) = b.cat.position;
        if t.columns[ca].is_numeric() {
            b.hours = false;
        }
        for r in 0..t.rows_in(bytes) {
            let fields: Vec<String> = (0..t.columns.len())
                .map(|c| t.text(bytes, r, c, 8))
                .collect();
            let pos = |c: usize, h: bool| {
                if t.columns[c].is_numeric() {
                    t.f64(bytes, r, c, 0)
                } else {
                    angle(&fields[c], h)
                }
            };
            if let (Some(l), Some(d)) = (pos(ca, b.hours), pos(cb, false)) {
                b.push(l, d, fields);
            }
        }
        return b.finish();
    }
    Err(last)
}

/// Splits a delimited line (quotes allowed, doubled quotes inside).
fn split(line: &str, delim: Option<char>) -> Vec<String> {
    let Some(d) = delim else {
        return line.split_whitespace().map(str::to_owned).collect();
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            quoted = true;
        } else if c == d {
            out.push(cur.trim().to_owned());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_owned());
    out
}

/// CSV, TSV, semicolon- or bar-separated, or whitespace-separated text with a header line.
fn delimited(bytes: &[u8]) -> Result<Catalogue> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text
        .lines()
        .map(|l| l.trim_start_matches('\u{feff}'))
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header = lines
        .next()
        .ok_or_else(|| Error::Unsupported("empty catalogue".into()))?;
    let delim = ['\t', ',', ';', '|']
        .into_iter()
        .map(|d| (header.matches(d).count(), d))
        .filter(|(n, _)| *n > 0)
        .max_by_key(|(n, _)| *n)
        .map(|(_, d)| d);
    let names = split(header, delim);
    let cols: Vec<Col> = names
        .iter()
        .map(|n| {
            // "ra [deg]", "ra (deg)": the bracketed part is the unit.
            let (name, unit) = match n.find(['[', '(']) {
                Some(i) => (
                    n[..i].trim().to_owned(),
                    n[i + 1..].trim_end_matches([']', ')']).trim().to_owned(),
                ),
                None => (n.clone(), String::new()),
            };
            Col {
                b1950: b1950_name(&name),
                name,
                unit,
                ucd: String::new(),
            }
        })
        .collect();
    let mut b = Builder::new(&cols, None)?;
    // A unit line right under the header (VizieR TSV).
    let mut first = true;
    for line in lines {
        let f = split(line, delim);
        if first {
            first = false;
            let (a, _) = b.cat.position;
            if let Some(u) = f.get(a)
                && angle(u, true).is_none()
                && !u.starts_with('-')
            {
                let u = u.to_ascii_lowercase();
                b.hours = u.starts_with('h') || u.contains("h:m");
                for (i, c) in b.cat.columns.iter_mut().enumerate() {
                    if let Some(x) = f.get(i).filter(|x| !x.is_empty()) {
                        *c = format!("{c} [{x}]");
                    }
                }
                continue;
            }
        }
        b.push_text(f);
    }
    b.finish()
}

/// Standard base64, whitespace ignored; stops at padding or a bad character.
fn base64(s: &[u8]) -> Vec<u8> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in s {
        if c.is_ascii_whitespace() {
            continue;
        }
        let Some(v) = val(c) else { break };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    out
}

/// A VOTable `FIELD`: datatype, element count (`None`: variable), and description.
#[derive(Clone, Debug, Default)]
struct Field {
    col: Col,
    datatype: String,
    count: Option<usize>,
}

impl Field {
    fn elem_size(&self) -> usize {
        match self.datatype.as_str() {
            "boolean" | "unsignedByte" | "char" | "bit" => 1,
            "short" | "unicodeChar" => 2,
            "int" | "float" => 4,
            "long" | "double" | "floatComplex" => 8,
            "doubleComplex" => 16,
            _ => 1,
        }
    }
}

fn arraysize(a: Option<&str>) -> Option<usize> {
    let Some(a) = a else { return Some(1) };
    if a.ends_with('*') {
        return None;
    }
    a.split('x')
        .map(|p| p.trim().parse::<usize>().ok())
        .try_fold(1usize, |acc, n| acc.checked_mul(n?))
}

/// One binary-serialised value as text; `None` past the end of the stream.
fn binary_value(f: &Field, b: &[u8], pos: &mut usize) -> Option<String> {
    let n = match f.count {
        Some(n) => n,
        None => {
            let c = b.get(*pos..*pos + 4)?;
            *pos += 4;
            u32::from_be_bytes(c.try_into().ok()?) as usize
        }
    };
    let bytes = if f.datatype == "bit" {
        n.div_ceil(8)
    } else {
        n.checked_mul(f.elem_size())?
    };
    let d = b.get(*pos..pos.checked_add(bytes)?)?;
    *pos += bytes;
    let s = match f.datatype.as_str() {
        "char" => String::from_utf8_lossy(d.split(|&c| c == 0).next().unwrap_or(&[]))
            .trim_end()
            .to_owned(),
        "unicodeChar" => {
            let u: Vec<u16> = d
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u)
                .trim_end_matches('\0')
                .trim_end()
                .to_owned()
        }
        "bit" => (0..n)
            .map(|k| {
                if d[k / 8] & (0x80 >> (k % 8)) != 0 {
                    '1'
                } else {
                    '0'
                }
            })
            .collect(),
        _ => {
            let vals: Vec<String> = d
                .chunks_exact(f.elem_size())
                .map(|c| match f.datatype.as_str() {
                    "boolean" => match c[0] {
                        b'T' | b't' | b'1' => "T".into(),
                        b'F' | b'f' | b'0' => "F".into(),
                        _ => String::new(),
                    },
                    "unsignedByte" => c[0].to_string(),
                    "short" => i16::from_be_bytes([c[0], c[1]]).to_string(),
                    "int" => i32::from_be_bytes([c[0], c[1], c[2], c[3]]).to_string(),
                    "long" => i64::from_be_bytes(c.try_into().expect("8")).to_string(),
                    "float" => {
                        let v = f32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                        if v.is_nan() {
                            String::new()
                        } else {
                            v.to_string()
                        }
                    }
                    "double" => {
                        let v = f64::from_be_bytes(c.try_into().expect("8"));
                        if v.is_nan() {
                            String::new()
                        } else {
                            v.to_string()
                        }
                    }
                    "floatComplex" => format!(
                        "({}, {})",
                        f32::from_be_bytes([c[0], c[1], c[2], c[3]]),
                        f32::from_be_bytes([c[4], c[5], c[6], c[7]])
                    ),
                    "doubleComplex" => format!(
                        "({}, {})",
                        f64::from_be_bytes(c[..8].try_into().expect("8")),
                        f64::from_be_bytes(c[8..].try_into().expect("8"))
                    ),
                    _ => String::new(),
                })
                .collect();
            if vals.len() == 1 {
                vals.into_iter().next().unwrap_or_default()
            } else {
                format!("[{}]", vals.join(", "))
            }
        }
    };
    Some(s)
}

fn attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.local_name().as_ref() == key)
        .map(|a| {
            let raw = String::from_utf8_lossy(&a.value).into_owned();
            quick_xml::escape::unescape(&raw)
                .map(|s| s.into_owned())
                .unwrap_or(raw)
        })
}

/// A VOTable: the first `TABLE` with position columns.
fn votable(bytes: &[u8]) -> Result<Catalogue> {
    use quick_xml::events::Event;
    let mut r = quick_xml::Reader::from_reader(bytes);
    let err = |e: &dyn std::fmt::Display| Error::Malformed(format!("VOTable: {e}"));
    let mut fields: Vec<Field> = Vec::new();
    // Each `COOSYS` by ID (FK4 B1950 or not), and each field's `ref` to one.
    let mut coosys: Vec<(String, bool)> = Vec::new();
    let mut refs: Vec<Option<String>> = Vec::new();
    let mut name: Option<String> = None;
    let mut builder: Option<Builder> = None;
    let mut row: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut in_td = false;
    let mut in_stream = false;
    let mut binary2 = false;
    let mut stream = Vec::new();
    let mut last = Error::Unsupported("no table in the VOTable".into());
    loop {
        let ev = r.read_event().map_err(|e| err(&e))?;
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(ev, Event::Empty(_));
                match e.local_name().as_ref() {
                    b"TABLE" => {
                        fields.clear();
                        refs.clear();
                        name = attr(e, b"name");
                        builder = None;
                    }
                    b"COOSYS" => coosys.push((
                        attr(e, b"ID").unwrap_or_default(),
                        coosys_fk4(
                            attr(e, b"system").as_deref(),
                            attr(e, b"equinox").as_deref(),
                        ),
                    )),
                    b"FIELD" => {
                        let name = attr(e, b"name")
                            .or_else(|| attr(e, b"ID"))
                            .unwrap_or_default();
                        refs.push(attr(e, b"ref"));
                        fields.push(Field {
                            col: Col {
                                b1950: b1950_name(&name),
                                name,
                                unit: attr(e, b"unit").unwrap_or_default(),
                                ucd: attr(e, b"ucd").unwrap_or_default(),
                            },
                            datatype: attr(e, b"datatype").unwrap_or_else(|| "char".into()),
                            count: arraysize(attr(e, b"arraysize").as_deref()),
                        });
                    }
                    b"DATA" if builder.is_none() => {
                        // A field's frame: the COOSYS it refers to, or the document's
                        // only one when it refers to none.
                        let only = match coosys.as_slice() {
                            [(_, fk4)] => Some(*fk4),
                            _ => None,
                        };
                        let cols: Vec<Col> = fields
                            .iter()
                            .zip(&refs)
                            .map(|(f, r)| {
                                let declared = match r {
                                    Some(r) => coosys.iter().find(|(id, _)| id == r).map(|c| c.1),
                                    None => only,
                                };
                                let mut c = f.col.clone();
                                if let Some(fk4) = declared {
                                    c.b1950 = fk4;
                                }
                                c
                            })
                            .collect();
                        match Builder::new(&cols, name.clone()) {
                            Ok(mut b) => {
                                let (ca, _) = b.cat.position;
                                let dt = &fields[ca].datatype;
                                if dt == "float" || dt == "double" {
                                    b.hours = false;
                                }
                                builder = Some(b);
                            }
                            Err(e) => last = e,
                        }
                    }
                    b"TR" => row.clear(),
                    b"TD" => {
                        if empty {
                            row.push(String::new());
                        } else {
                            in_td = true;
                            text.clear();
                        }
                    }
                    b"BINARY" | b"BINARY2" => binary2 = e.local_name().as_ref() == b"BINARY2",
                    b"STREAM" if !empty => {
                        in_stream = true;
                        stream.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(ref t) if in_td || in_stream => {
                if in_td {
                    text.push_str(&String::from_utf8_lossy(t));
                } else {
                    stream.extend_from_slice(t);
                }
            }
            Event::CData(ref t) if in_td => text.push_str(&String::from_utf8_lossy(t)),
            Event::GeneralRef(ref g) if in_td => {
                let n = String::from_utf8_lossy(g);
                if let Ok(s) = quick_xml::escape::unescape(&format!("&{n};")) {
                    text.push_str(&s);
                }
            }
            Event::End(ref e) => match e.local_name().as_ref() {
                b"TD" => {
                    in_td = false;
                    row.push(text.trim().to_owned());
                }
                b"TR" => {
                    if let Some(b) = builder.as_mut() {
                        b.push_text(std::mem::take(&mut row));
                    }
                }
                b"STREAM" => {
                    in_stream = false;
                    if let Some(b) = builder.as_mut() {
                        decode_binary(b, &fields, &base64(&stream), binary2);
                    }
                }
                b"TABLE" => {
                    if let Some(b) = builder.take() {
                        if b.cat.lng.is_empty() {
                            last = Error::Unsupported("no positions in the catalogue".into());
                        } else {
                            return b.finish();
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    match builder {
        Some(b) => b.finish(),
        None => Err(last),
    }
}

/// Rows of a `BINARY` / `BINARY2` stream.
fn decode_binary(b: &mut Builder, fields: &[Field], data: &[u8], binary2: bool) {
    let mut pos = 0;
    let flags = fields.len().div_ceil(8);
    while pos < data.len() {
        let row_start = pos;
        let nulls = if binary2 {
            let Some(n) = data.get(pos..pos + flags) else {
                return;
            };
            pos += flags;
            n.to_vec()
        } else {
            Vec::new()
        };
        let mut row = Vec::with_capacity(fields.len());
        for (i, f) in fields.iter().enumerate() {
            let Some(v) = binary_value(f, data, &mut pos) else {
                return;
            };
            let null = nulls.get(i / 8).is_some_and(|n| n & (0x80 >> (i % 8)) != 0);
            row.push(if null { String::new() } else { v });
        }
        b.push_text(row);
        // Zero-width fields (arraysize="0") would repeat the same row forever.
        if pos == row_start {
            return;
        }
    }
}

/// The rotation from equatorial (ICRS) to galactic coordinates (the Hipparcos definition,
/// ESA SP-1200 vol. 1 §1.5.3).
const ICRS_TO_GALACTIC: [[f64; 3]; 3] = [
    [
        -0.054_875_560_416_215_4,
        -0.873_437_090_234_885,
        -0.483_835_015_548_713_2,
    ],
    [
        0.494_109_427_875_583_7,
        -0.444_829_629_960_011_2,
        0.746_982_244_497_219,
    ],
    [
        -0.867_666_149_019_004_7,
        -0.198_076_373_431_201_5,
        0.455_983_776_175_066_9,
    ],
];

fn rotate(m: &[[f64; 3]; 3], transpose: bool, lng: f64, lat: f64) -> (f64, f64) {
    let (l, b) = (lng.to_radians(), lat.to_radians());
    let v = [b.cos() * l.cos(), b.cos() * l.sin(), b.sin()];
    let at = |i: usize, j: usize| if transpose { m[j][i] } else { m[i][j] };
    let r: [f64; 3] = std::array::from_fn(|i| (0..3).map(|j| at(i, j) * v[j]).sum());
    let lng = r[1].atan2(r[0]).to_degrees().rem_euclid(360.0);
    let lat = r[2].clamp(-1.0, 1.0).asin().to_degrees();
    (lng, lat)
}

/// Galactic longitude / latitude (degrees) to ICRS right ascension / declination.
pub fn galactic_to_icrs(l: f64, b: f64) -> (f64, f64) {
    rotate(&ICRS_TO_GALACTIC, true, l, b)
}

/// ICRS right ascension / declination (degrees) to galactic longitude / latitude.
pub fn icrs_to_galactic(ra: f64, dec: f64) -> (f64, f64) {
    rotate(&ICRS_TO_GALACTIC, false, ra, dec)
}

impl Catalogue {
    /// Row `k`'s position in galactic coordinates when `galactic`, else equatorial; the
    /// equatorial frames (ICRS, FK5 J2000) are taken as one, their difference being well
    /// under an arcsecond.
    pub fn sky(&self, k: usize, galactic: bool) -> (f64, f64) {
        let (a, b) = (self.lng[k], self.lat[k]);
        match (self.galactic, galactic) {
            (true, false) => galactic_to_icrs(a, b),
            (false, true) => icrs_to_galactic(a, b),
            _ => (a, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against astropy's `SkyCoord(frame="galactic").icrs`.
    #[test]
    fn galactic_and_equatorial() {
        for (l, b, ra, dec) in [
            (0.0, 0.0, 266.4049882865447, -28.936177761791473),
            (0.0, 90.0, 192.85947789477606, 27.128252414968028),
            (120.0, -30.0, 9.838570031463801, 32.802948339960494),
            (300.5, 45.25, 191.06365451925583, -17.58782567660108),
        ] {
            let (a, d) = galactic_to_icrs(l, b);
            assert!(
                (a - ra).abs() < 2e-5 && (d - dec).abs() < 2e-5,
                "{l},{b}: {a},{d}"
            );
            let (l2, b2) = icrs_to_galactic(a, d);
            if b < 89.0 {
                assert!(
                    (l2 - l).rem_euclid(360.0).min((l - l2).rem_euclid(360.0)) < 1e-9,
                    "{l2}"
                );
            }
            assert!((b2 - b).abs() < 1e-9, "{b2}");
        }
    }

    #[test]
    fn angles() {
        assert_eq!(angle("12:30:00", true), Some(187.5));
        assert_eq!(angle("-00 30 00", false), Some(-0.5));
        assert_eq!(angle("10.5", true), Some(10.5));
        assert_eq!(angle("1h00m00s", false), Some(15.0));
        assert_eq!(angle("+45d30m", true), Some(45.5));
        assert_eq!(angle("abc", true), None);
        assert_eq!(angle("---", true), None);
    }

    #[test]
    fn csv() {
        let c = parse(b"# comment\nname,RAJ2000,DEJ2000,mag\n\"A, b\",10.5,-20.25,12\nB,12:00:00,+01:00:00,13\nC,,5,1\n").unwrap();
        assert_eq!(c.lng, vec![10.5, 180.0]);
        assert_eq!(c.lat, vec![-20.25, 1.0]);
        assert_eq!(c.rows[0][0], "A, b");
        assert!(!c.galactic);
    }

    /// FK4 B1950 positions: a VOTable COOSYS (by reference, or the only one), B1950
    /// column names, and a FITS table's RADESYS / EQUINOX.
    #[test]
    fn b1950_catalogues() {
        let vot = |coosys: &str, field_ref: &str| {
            format!(
                r#"<VOTABLE version="1.4"><RESOURCE>{coosys}<TABLE>
<FIELD name="RA" datatype="double" ucd="pos.eq.ra;meta.main" unit="deg"{field_ref}/>
<FIELD name="DE" datatype="double" ucd="pos.eq.dec;meta.main" unit="deg"{field_ref}/>
<DATA><TABLEDATA><TR><TD>10</TD><TD>20</TD></TR></TABLEDATA></DATA></TABLE></RESOURCE></VOTABLE>"#
            )
        };
        let fk4 = r#"<COOSYS ID="B1950" system="eq_FK4" equinox="B1950"/>"#;
        let fk5 = r#"<COOSYS ID="J2000" system="eq_FK5" equinox="J2000"/>"#;
        for (xml, want) in [
            (vot(fk4, r#" ref="B1950""#), true),
            (vot(fk4, ""), true),
            (vot(&format!("{fk5}{fk4}"), r#" ref="J2000""#), false),
            (vot(&format!("{fk5}{fk4}"), r#" ref="B1950""#), true),
            (vot(&format!("{fk5}{fk4}"), ""), false),
            (
                vot(r#"<COOSYS ID="c" system="FK4" equinox="B1975"/>"#, ""),
                false,
            ),
            (vot(r#"<COOSYS ID="c" system="FK4"/>"#, ""), true),
            (vot("", ""), false),
        ] {
            assert_eq!(parse(xml.as_bytes()).unwrap().fk4, want, "{xml}");
        }
        let c = parse(b"name,RAJ2000,DEJ2000,RAB1950,DEB1950\na,1,2,3,4\n").unwrap();
        assert!(!c.fk4);
        assert_eq!(c.lng, vec![1.0]);
        let c = parse(b"name,_RAB1950,_DEB1950\na,3,4\n").unwrap();
        assert!(c.fk4);
        assert_eq!(c.lng, vec![3.0]);
        let h = |cards: &[(&str, &str)]| {
            crate::Header::from_pairs(cards.iter().map(|&(k, v)| (k, v, "")))
        };
        assert!(header_fk4(&h(&[("RADESYS", "'FK4'")]), "RA"));
        assert!(header_fk4(&h(&[("RADECSYS", "'FK4'")]), "RA"));
        assert!(header_fk4(&h(&[("EQUINOX", "1950.0")]), "RA"));
        assert!(!header_fk4(&h(&[("EQUINOX", "1950.0")]), "RAJ2000"));
        assert!(!header_fk4(
            &h(&[("RADESYS", "'FK5'"), ("EQUINOX", "1950.0")]),
            "RA"
        ));
        assert!(!header_fk4(
            &h(&[("RADESYS", "'FK4'"), ("EQUINOX", "1975.0")]),
            "RA"
        ));
        assert!(!header_fk4(&h(&[]), "RA"));
    }

    #[test]
    fn vizier_tsv() {
        let c = parse(b"RAJ2000\tDEJ2000\tVmag\n\"h:m:s\"\t\"d:m:s\"\tmag\n------\t------\t---\n01 00 00\t-10 30 00\t5.0\n").unwrap();
        assert_eq!(c.lng, vec![15.0]);
        assert_eq!(c.lat, vec![-10.5]);
    }

    #[test]
    fn votable_tabledata_and_binary2() {
        let xml = br#"<?xml version="1.0"?>
<VOTABLE version="1.4"><RESOURCE><TABLE name="t">
<FIELD name="id" datatype="char" arraysize="*"/>
<FIELD name="ra" datatype="double" ucd="pos.eq.ra;meta.main" unit="deg"/>
<FIELD name="dec" datatype="double" ucd="pos.eq.dec;meta.main" unit="deg"/>
<DATA><TABLEDATA>
<TR><TD>a &amp; b</TD><TD>1.5</TD><TD>2.5</TD></TR>
<TR><TD/><TD>3</TD><TD>-4</TD></TR>
</TABLEDATA></DATA></TABLE></RESOURCE></VOTABLE>"#;
        let c = parse(xml).unwrap();
        assert_eq!(c.lng, vec![1.5, 3.0]);
        assert_eq!(c.rows[0][0], "a & b");
        assert_eq!(c.columns[1], "ra [deg]");
        // The same rows as BINARY2.
        let mut bin = Vec::new();
        for (id, ra, de) in [("ab", 1.5f64, 2.5f64), ("", 3.0, -4.0)] {
            bin.push(0u8);
            bin.extend((id.len() as u32).to_be_bytes());
            bin.extend(id.as_bytes());
            bin.extend(ra.to_be_bytes());
            bin.extend(de.to_be_bytes());
        }
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut b64 = String::new();
        for ch in bin.chunks(3) {
            let n = ch.iter().fold(0u32, |a, &x| (a << 8) | u32::from(x)) << (8 * (3 - ch.len()));
            for k in 0..=ch.len() {
                b64.push(T[((n >> (18 - 6 * k)) & 63) as usize] as char);
            }
        }
        let xml2 = String::from_utf8_lossy(xml).replace(
            &String::from_utf8_lossy(
                &xml[xml.windows(11).position(|w| w == b"<TABLEDATA>").unwrap()
                    ..xml.windows(12).position(|w| w == b"</TABLEDATA>").unwrap() + 12],
            )
            .into_owned(),
            &format!("<BINARY2><STREAM encoding=\"base64\">{b64}</STREAM></BINARY2>"),
        );
        let c2 = parse(xml2.as_bytes()).unwrap();
        assert_eq!(c2.lng, c.lng);
        assert_eq!(c2.lat, c.lat);
        assert_eq!(c2.rows[0][0], "ab");
    }
}

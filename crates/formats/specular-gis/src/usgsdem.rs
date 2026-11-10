//! USGS DEM (`.dem`), the native ASCII format of the USGS Digital Elevation Model standard.
//!
//! 1024-byte records of fixed-width Fortran fields (`I6`, `E12.6`, `D24.15`; some files end
//! each record with a line break). Record A: the quadrangle name, DEM level, elevation
//! pattern (1 = regular), planimetric reference (0 geographic in arc seconds, 1 UTM with its
//! zone, 2 State Plane), units, the four corners, the minimum and maximum elevation, the
//! spatial resolution (x, y, z) and the number of profiles, and later the vertical and
//! horizontal datum codes. Then one B record per profile (a south-to-north column): its row
//! and column, its elevation count, the ground position of its first elevation, the local
//! datum elevation, its range, then the elevations as `I6` fields (146 in the first record,
//! 170 in each record after). Elevation = value × z resolution + local datum; −32767 is a
//! void. Profiles of a UTM quadrangle start at different northings: they are placed on one
//! grid by their positions.

use crate::geotiff::Affine2;

const RECORD: usize = 1024;
/// Columns accepted at most (USGS 1° DEMs have 1201).
const MAX_SIDE: u32 = 1 << 15;
pub const VOID: i32 = -32767;

/// The parsed A record and the profiles' layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub name: String,
    pub level: u32,
    /// 0 geographic, 1 UTM, 2 State Plane.
    pub reference: u32,
    pub zone: i32,
    /// 0 radians, 1 feet, 2 metres, 3 arc seconds.
    pub ground_units: u32,
    /// 1 feet, 2 metres.
    pub elevation_units: u32,
    pub min_elevation: f64,
    pub max_elevation: f64,
    /// Spatial resolution x, y, z.
    pub resolution: [f64; 3],
    /// 1 NAD27, 2 WGS72, 3 WGS84, 4 NAD83 (blank in older files: NAD27).
    pub horizontal_datum: u32,
    pub vertical_datum: Option<u32>,
    /// Grid columns (profiles) and rows.
    pub width: u32,
    pub height: u32,
    /// Ground position of the westernmost profile and the southernmost row.
    pub x0: f64,
    pub y0: f64,
    /// (record index, column, row of its first elevation from the south, elevation count,
    /// local datum) of each profile.
    profiles: Vec<Profile>,
    /// Byte offsets of the records.
    records: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Profile {
    record: usize,
    col: u32,
    row: u32,
    count: u32,
    datum: f64,
}

fn field(r: &[u8], a: usize, b: usize) -> &str {
    r.get(a..b.min(r.len()))
        .and_then(|s| std::str::from_utf8(s).ok())
        .map_or("", str::trim)
}

fn int(r: &[u8], a: usize, b: usize) -> Option<i64> {
    field(r, a, b).parse().ok()
}

/// A Fortran real (`D` or `E` exponent).
fn real(r: &[u8], a: usize, b: usize) -> Option<f64> {
    field(r, a, b).replace(['D', 'd'], "E").parse().ok()
}

/// Whether the start of a file is a USGS DEM A record (its fixed-column codes plausible).
pub fn sniff(head: &[u8]) -> bool {
    if head.len() < 864 {
        return false;
    }
    let r = &head[..RECORD.min(head.len())];
    let ok = |a, b, lo: i64, hi: i64| int(r, a, b).is_some_and(|v| (lo..=hi).contains(&v));
    ok(144, 150, 1, 4)
        && ok(150, 156, 1, 1)
        && ok(156, 162, 0, 2)
        && ok(528, 534, 0, 3)
        && ok(534, 540, 1, 2)
        && ok(540, 546, 4, 4)
        && real(r, 816, 828).is_some_and(|v| v > 0.0)
}

/// Record offsets: every 1024 bytes, skipping line breaks between records.
fn records(data: &[u8]) -> Vec<usize> {
    let mut v = Vec::new();
    let mut at = 0;
    while at < data.len() {
        v.push(at);
        at += RECORD;
        while at < data.len() && matches!(data[at], b'\r' | b'\n') {
            at += 1;
        }
    }
    v
}

pub fn header(data: &[u8]) -> Result<Header, String> {
    if !sniff(data) {
        return Err("not a USGS DEM".into());
    }
    let recs = records(data);
    let a = &data[..RECORD.min(data.len())];
    let name = field(a, 0, 40).to_string();
    let i = |a0, b0| int(a, a0, b0).unwrap_or(0);
    let res = [
        real(a, 816, 828).unwrap_or(0.0),
        real(a, 828, 840).unwrap_or(0.0),
        real(a, 840, 852).unwrap_or(1.0),
    ];
    if !(res[0] > 0.0 && res[1] > 0.0) {
        return Err("spatial resolution missing".into());
    }
    let res = [res[0], res[1], if res[2] > 0.0 { res[2] } else { 1.0 }];
    let columns = i(858, 864).max(0) as usize;
    if columns == 0 || columns > MAX_SIDE as usize {
        return Err(format!("{columns} profiles"));
    }
    // Walk the B records.
    let mut profiles = Vec::with_capacity(columns);
    let mut k = 1;
    let mut raw: Vec<(usize, f64, f64, u32, f64)> = Vec::new();
    while raw.len() < columns && k < recs.len() {
        let r = &data[recs[k]..(recs[k] + RECORD).min(data.len())];
        let (Some(m), Some(x), Some(y)) = (int(r, 12, 18), real(r, 24, 48), real(r, 48, 72)) else {
            break;
        };
        let datum = real(r, 72, 96).unwrap_or(0.0);
        if m <= 0 || m > i64::from(MAX_SIDE) * 4 {
            return Err(format!("profile with {m} elevations"));
        }
        raw.push((k, x, y, m as u32, datum));
        let m = m as usize;
        k += 1 + if m > 146 { (m - 146).div_ceil(170) } else { 0 };
    }
    if raw.is_empty() {
        return Err("no profiles".into());
    }
    let x0 = raw.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let y0 = raw.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
    let mut top = 0u32;
    for &(record, x, y, m, datum) in &raw {
        let col = ((x - x0) / res[0]).round().max(0.0) as u32;
        let row = ((y - y0) / res[1]).round().max(0.0) as u32;
        top = top.max(row.saturating_add(m));
        profiles.push(Profile {
            record,
            col,
            row,
            count: m,
            datum,
        });
    }
    let width = profiles.iter().map(|p| p.col + 1).max().unwrap_or(0);
    if width == 0 || width > MAX_SIDE || top == 0 || top > MAX_SIDE * 4 {
        return Err(format!("{width} × {top} grid"));
    }
    let hd = i(890, 892);
    Ok(Header {
        name,
        level: i(144, 150) as u32,
        reference: i(156, 162) as u32,
        zone: i(162, 168) as i32,
        ground_units: i(528, 534) as u32,
        elevation_units: i(534, 540) as u32,
        min_elevation: real(a, 738, 762).unwrap_or(0.0),
        max_elevation: real(a, 762, 786).unwrap_or(0.0),
        resolution: res,
        horizontal_datum: if (1..=4).contains(&hd) { hd as u32 } else { 1 },
        vertical_datum: int(a, 888, 890).filter(|v| *v > 0).map(|v| v as u32),
        width,
        height: top,
        x0,
        y0,
        profiles,
        records: recs,
    })
}

impl Header {
    /// Degrees per ground unit for geographic DEMs (arc seconds), 1 otherwise.
    fn to_crs_units(&self) -> f64 {
        match (self.reference, self.ground_units) {
            (0, 3) => 1.0 / 3600.0,
            (0, 0) => 180.0 / std::f64::consts::PI,
            _ => 1.0,
        }
    }

    /// Pixel corner to ground coordinates (degrees for geographic DEMs); posts are pixel
    /// centres.
    pub fn transform(&self) -> Affine2 {
        let s = self.to_crs_units();
        let (dx, dy) = (self.resolution[0] * s, self.resolution[1] * s);
        let top = (self.y0 + f64::from(self.height - 1) * self.resolution[1]) * s;
        Affine2 {
            a: dx,
            b: 0.0,
            c: self.x0 * s - 0.5 * dx,
            d: 0.0,
            e: -dy,
            f: top + 0.5 * dy,
        }
    }

    /// The EPSG code of the reference system, when it has one.
    pub fn epsg(&self) -> Option<u32> {
        let z = self.zone.unsigned_abs();
        let south = self.zone < 0;
        match self.reference {
            0 => Some(match self.horizontal_datum {
                2 => 4322,
                3 => 4326,
                4 => 4269,
                _ => 4267,
            }),
            1 if (1..=60).contains(&z) => match (self.horizontal_datum, south) {
                (3, false) => Some(32600 + z),
                (3, true) => Some(32700 + z),
                (2, false) => Some(32200 + z),
                (2, true) => Some(32300 + z),
                (4, false) if z <= 23 => Some(26900 + z),
                (1, false) if z <= 22 => Some(26700 + z),
                _ => None,
            },
            _ => None,
        }
    }

    /// "m" or "ft".
    pub fn unit(&self) -> &'static str {
        if self.elevation_units == 1 { "ft" } else { "m" }
    }

    /// The heights, row 0 the northern edge, voids and cells no profile covers as NaN;
    /// whether records were missing.
    pub fn values(&self, data: &[u8]) -> (Vec<f32>, bool) {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![f32::NAN; w * h];
        let mut bad = self.profiles.len() < self.width as usize;
        let z = self.resolution[2];
        for p in &self.profiles {
            for e in 0..p.count as usize {
                // Field e: in the first record after the 144-byte header, then 170 per record.
                let (rec, at) = if e < 146 {
                    (p.record, 144 + 6 * e)
                } else {
                    let k = e - 146;
                    (p.record + 1 + k / 170, 6 * (k % 170))
                };
                let Some(&start) = self.records.get(rec) else {
                    bad = true;
                    break;
                };
                let r = &data[start..(start + RECORD).min(data.len())];
                let Some(v) = int(r, at, at + 6) else {
                    bad = true;
                    continue;
                };
                let row_s = p.row as usize + e;
                if row_s >= h || p.col as usize >= w {
                    continue;
                }
                let row = h - 1 - row_s;
                out[row * w + p.col as usize] = if v as i32 <= VOID {
                    f32::NAN
                } else {
                    (v as f64 * z + p.datum) as f32
                };
            }
        }
        (out, bad)
    }

    /// The info panel's lines.
    pub fn facts(&self) -> Vec<(String, String)> {
        let mut v = Vec::new();
        if !self.name.is_empty() {
            v.push(("Quadrangle".into(), self.name.clone()));
        }
        v.push(("DEM level".into(), self.level.to_string()));
        v.push((
            "Reference system".into(),
            match self.reference {
                0 => "Geographic".into(),
                1 => format!("UTM zone {}", self.zone),
                _ => format!("State Plane zone {}", self.zone),
            },
        ));
        let datum = match self.horizontal_datum {
            2 => "WGS 72",
            3 => "WGS 84",
            4 => "NAD 83",
            _ => "NAD 27",
        };
        v.push(("Horizontal datum".into(), datum.into()));
        if let Some(d) = self.vertical_datum {
            let s = match d {
                1 => "Local mean sea level",
                2 => "NGVD 29",
                3 => "NAVD 88",
                _ => "Other",
            };
            v.push(("Vertical datum".into(), s.into()));
        }
        let u = match (self.reference, self.ground_units) {
            (_, 3) => "″",
            (_, 1) => " ft",
            (_, 0) => " rad",
            _ => " m",
        };
        v.push((
            "Spacing".into(),
            format!("{}{u} × {}{u}", self.resolution[0], self.resolution[1]),
        ));
        v.push((
            "Elevation range".into(),
            format!(
                "{} – {} {}",
                self.min_elevation,
                self.max_elevation,
                self.unit()
            ),
        ));
        v
    }
}

/// Writes a regular USGS DEM (tests and fixtures): UTM zone `zone` (NAD 83), 30 m posts,
/// `cols` profiles each starting at `starts[col]` rows above the south edge with
/// `heights[col]` (metres; `None` a void). Records end with a line break when `newlines`.
pub fn write(zone: i32, starts: &[u32], heights: &[Vec<Option<i32>>], newlines: bool) -> Vec<u8> {
    let rec = |s: String| {
        let mut b = s.into_bytes();
        b.resize(RECORD, b' ');
        b
    };
    let d24 = |v: f64| format!("{:>24}", format!("{v:.15E}").replace('E', "D"));
    let mut a = format!("{:<40}{:<40}{:29}", "TEST QUAD", "", "");
    // Corner, process code, sectional indicator and origin code: blank.
    a.push_str(&" ".repeat(35));
    a.push_str(&format!("{:6}{:6}{:6}{:6}", 2, 1, 1, zone));
    a.push_str(&" ".repeat(360));
    a.push_str(&format!("{:6}{:6}{:6}", 2, 2, 4));
    for _ in 0..8 {
        a.push_str(&d24(0.0));
    }
    a.push_str(&d24(0.0));
    a.push_str(&d24(1000.0));
    a.push_str(&d24(0.0));
    a.push_str(&format!("{:6}", 0));
    a.push_str(&format!(
        "{:>12}{:>12}{:>12}",
        "0.300000E+02", "0.300000E+02", "0.100000E+01"
    ));
    a.push_str(&format!("{:6}{:6}", 1, heights.len()));
    a.push_str(&" ".repeat(888 - a.len()));
    a.push_str(&format!("{:2}{:2}", 2, 4));
    let mut out = rec(a);
    let push = |out: &mut Vec<u8>, r: Vec<u8>| {
        if newlines {
            out.push(b'\n');
        }
        out.extend_from_slice(&r);
    };
    for (c, col) in heights.iter().enumerate() {
        let x = 500_000.0 + 30.0 * c as f64;
        let y = 4_000_000.0 + 30.0 * f64::from(starts[c]);
        let mut s = format!("{:6}{:6}{:6}{:6}", 1, c + 1, col.len(), 1);
        s.push_str(&d24(x));
        s.push_str(&d24(y));
        s.push_str(&d24(0.0));
        s.push_str(&d24(0.0));
        s.push_str(&d24(1000.0));
        let mut cur = s;
        let mut cap = 146;
        for v in col {
            if cap == 0 {
                push(&mut out, rec(std::mem::take(&mut cur)));
                cap = 170;
            }
            cur.push_str(&format!("{:6}", v.unwrap_or(VOID)));
            cap -= 1;
        }
        push(&mut out, rec(cur));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staggered_profiles() {
        // Three profiles: the middle starts one row higher; the last spans two records.
        let tall: Vec<Option<i32>> = (0..200).map(Some).collect();
        let heights = vec![
            vec![Some(10), Some(11), None],
            vec![Some(20), Some(21)],
            tall,
        ];
        for nl in [false, true] {
            let f = write(13, &[0, 1, 0], &heights, nl);
            assert!(sniff(&f));
            let h = header(&f).unwrap();
            assert_eq!((h.width, h.height), (3, 200));
            assert_eq!(h.epsg(), Some(26913));
            let (v, bad) = h.values(&f);
            assert!(!bad);
            let at = |c: usize, row_from_south: usize| v[(199 - row_from_south) * 3 + c];
            assert_eq!(at(0, 0), 10.0);
            assert_eq!(at(0, 1), 11.0);
            assert!(at(0, 2).is_nan());
            assert!(at(1, 0).is_nan());
            assert_eq!(at(1, 1), 20.0);
            assert_eq!(at(2, 150), 150.0);
            assert_eq!(at(2, 199), 199.0);
            let t = h.transform();
            assert_eq!(t.c, 500_000.0 - 15.0);
            assert_eq!(t.f, 4_000_000.0 + 199.0 * 30.0 + 15.0);
        }
        assert!(!sniff(&[b' '; 2048]));
    }
}

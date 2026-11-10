//! DTED (Digital Terrain Elevation Data) levels 0, 1 and 2 (`.dt0`, `.dt1`, `.dt2`).
//!
//! An 80-byte user header label (`UHL1`: origin, posting intervals in tenths of an arc
//! second, the number of longitude lines and latitude points), a 648-byte data set
//! identification (`DSI`: level, datums, producer), a 2700-byte accuracy record (`ACC`),
//! then one record per longitude line, west to east: a sentinel (`0xAA`), the block and
//! line numbers, the heights from south to north as 16-bit signed-magnitude big-endian
//! metres, and a checksum. −32767 marks voids.
//!
//! Files copied from tape may start with 80-byte `VOL1`/`HDR1` labels before the `UHL1`;
//! partial tiles hold only some longitude lines, each placed by its own line number.

use crate::geotiff::Affine2;

const UHL: usize = 80;
const DSI: usize = 648;
const ACC: usize = 2700;
/// Where the first data record starts.
pub const DATA: usize = UHL + DSI + ACC;
/// The void value.
pub const VOID: i16 = -32767;

/// The headers.
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// Longitude lines (columns).
    pub lon_lines: u32,
    /// Latitude points per line (rows).
    pub lat_points: u32,
    /// The south-west post, degrees.
    pub origin_lon: f64,
    pub origin_lat: f64,
    /// Posting intervals, degrees.
    pub dlon: f64,
    pub dlat: f64,
    /// "DTED0", "DTED1", "DTED2", when the DSI names it.
    pub level: Option<String>,
    pub vertical_datum: Option<String>,
    pub horizontal_datum: Option<String>,
    pub producer: Option<String>,
    pub compiled: Option<String>,
    /// Absolute vertical and horizontal accuracy in metres (90% linear/circular error).
    pub vertical_accuracy: Option<u32>,
    pub horizontal_accuracy: Option<u32>,
    pub security: Option<String>,
    /// Where the `UHL1` record starts (after any tape labels).
    pub offset: usize,
}

/// Whether the bytes start with a DTED user header label.
pub fn sniff(head: &[u8]) -> bool {
    uhl_offset(head).is_some()
}

/// The offset of the `UHL1` record: 0, or after up to three 80-byte tape labels.
fn uhl_offset(d: &[u8]) -> Option<usize> {
    let uhl = |at: usize| {
        d.get(at..at + 8)
            .is_some_and(|h| &h[..4] == b"UHL1" && h[4..8].iter().all(u8::is_ascii_digit))
    };
    if uhl(0) {
        return Some(0);
    }
    let label = |at: usize| {
        d.get(at..at + 4)
            .is_some_and(|h| matches!(h, b"VOL1" | b"HDR1" | b"HDR2" | b"HDR3"))
    };
    (1..=3)
        .take_while(|&k| label((k - 1) * UHL))
        .map(|k| k * UHL)
        .find(|&at| uhl(at))
}

fn field(b: &[u8], at: usize, len: usize) -> Option<&str> {
    let s = std::str::from_utf8(b.get(at..at + len)?).ok()?.trim();
    (!s.is_empty()).then_some(s)
}

fn num(b: &[u8], at: usize, len: usize) -> Option<u32> {
    field(b, at, len)?.parse().ok()
}

/// "DDDMMSSH" → signed degrees.
fn angle(b: &[u8], at: usize) -> Option<f64> {
    let s = field(b, at, 8)?;
    // ASCII only: the byte slicing below must not split a multi-byte character.
    if s.len() != 8 || !s.is_ascii() {
        return None;
    }
    let d: f64 = s[..3].parse().ok()?;
    let m: f64 = s[3..5].parse().ok()?;
    let sec: f64 = s[5..7].parse().ok()?;
    let sign = match s.as_bytes()[7].to_ascii_uppercase() {
        b'N' | b'E' => 1.0,
        b'S' | b'W' => -1.0,
        _ => return None,
    };
    Some(sign * (d + m / 60.0 + sec / 3600.0))
}

/// Parses the headers.
pub fn header(data: &[u8]) -> Result<Header, String> {
    let offset = uhl_offset(data).ok_or("not a DTED file")?;
    let data = &data[offset..];
    let u = data.get(..UHL).ok_or("not a DTED file")?;
    let (mut lon, mut lat) = (4, 12);
    // Some writers swapped the two fields; the hemisphere letters tell.
    let hemi = |at: usize| u[at + 7].to_ascii_uppercase();
    if matches!(hemi(4), b'N' | b'S') && matches!(hemi(12), b'E' | b'W') {
        (lon, lat) = (12, 4);
    }
    let origin_lon = angle(u, lon).ok_or("bad longitude origin")?;
    let origin_lat = angle(u, lat).ok_or("bad latitude origin")?;
    let ilon = num(u, 20, 4)
        .filter(|v| *v > 0)
        .ok_or("bad longitude interval")?;
    let ilat = num(u, 24, 4)
        .filter(|v| *v > 0)
        .ok_or("bad latitude interval")?;
    let lon_lines = num(u, 47, 4).filter(|v| *v > 0).ok_or("bad line count")?;
    let lat_points = num(u, 51, 4).filter(|v| *v > 0).ok_or("bad point count")?;
    // A truncated file may lack the later headers: blanks stand in.
    let blank = [b' '; DSI + ACC];
    let rest = data.get(UHL..DATA).unwrap_or(&blank);
    let (dsi, acc) = rest.split_at(DSI);
    let has_dsi = &dsi[..3] == b"DSI";
    let has_acc = &acc[..3] == b"ACC";
    let d = |at, len| {
        has_dsi
            .then(|| field(dsi, at, len).map(str::to_string))
            .flatten()
    };
    Ok(Header {
        lon_lines,
        lat_points,
        origin_lon,
        origin_lat,
        dlon: f64::from(ilon) / 36000.0,
        dlat: f64::from(ilat) / 36000.0,
        level: d(59, 5),
        vertical_datum: d(141, 3),
        horizontal_datum: d(144, 5),
        producer: d(102, 8),
        compiled: d(159, 4),
        vertical_accuracy: num(u, 28, 4).or_else(|| has_acc.then(|| num(acc, 7, 4)).flatten()),
        horizontal_accuracy: has_acc.then(|| num(acc, 3, 4)).flatten(),
        security: field(u, 32, 3).map(str::to_string),
        offset,
    })
}

impl Header {
    /// Pixel corner to longitude/latitude (posts are pixel centres).
    pub fn transform(&self) -> Affine2 {
        Affine2 {
            a: self.dlon,
            b: 0.0,
            c: self.origin_lon - 0.5 * self.dlon,
            d: 0.0,
            e: -self.dlat,
            f: self.origin_lat + (f64::from(self.lat_points) - 0.5) * self.dlat,
        }
    }

    fn record_len(&self) -> usize {
        12 + 2 * self.lat_points as usize
    }

    /// The bytes the data records take.
    pub fn data_len(&self) -> usize {
        self.offset + DATA + self.lon_lines as usize * self.record_len()
    }

    /// The heights, row 0 the northern edge, voids as NaN; whether records were missing
    /// or damaged.
    pub fn values(&self, data: &[u8]) -> (Vec<f32>, bool) {
        let (w, h) = (self.lon_lines as usize, self.lat_points as usize);
        let mut out = vec![f32::NAN; w * h];
        let rl = self.record_len();
        let mut bad = false;
        for i in 0..w {
            let at = self.offset + DATA + i * rl;
            let Some(rec) = data.get(at..at + rl) else {
                bad = true;
                break;
            };
            if rec[0] != 0xAA {
                bad = true;
                continue;
            }
            // The record's own longitude line number places it (partial tiles skip lines).
            let col = usize::from(u16::from_be_bytes([rec[4], rec[5]]));
            let col = if col < w { col } else { i };
            if col != i {
                bad = true;
            }
            for (i, p) in rec[8..8 + 2 * h].chunks_exact(2).enumerate() {
                let raw = u16::from_be_bytes([p[0], p[1]]);
                let mut v = i32::from(raw & 0x7fff);
                if raw & 0x8000 != 0 {
                    v = -v;
                    // Some producers wrote two's complement: a "negative" height below
                    // −16000 m is that, unless it is the void value.
                    if v < -16000 && v != i32::from(VOID) {
                        v = i32::from(raw as i16);
                    }
                }
                let row = h - 1 - i;
                out[row * w + col] = if v == i32::from(VOID) {
                    f32::NAN
                } else {
                    v as f32
                };
            }
        }
        (out, bad)
    }

    /// The info panel's lines.
    pub fn facts(&self) -> Vec<(String, String)> {
        let mut v = Vec::new();
        let mut push = |k: &str, s: Option<String>| {
            if let Some(s) = s {
                v.push((k.to_string(), s));
            }
        };
        push("Level", self.level.clone());
        push(
            "Posting",
            Some(format!(
                "{}″ × {}″",
                fmt_arcsec(self.dlon * 3600.0),
                fmt_arcsec(self.dlat * 3600.0)
            )),
        );
        push("Horizontal datum", self.horizontal_datum.clone());
        push("Vertical datum", self.vertical_datum.clone());
        push(
            "Absolute vertical accuracy",
            self.vertical_accuracy.map(|a| format!("{a} m")),
        );
        push(
            "Absolute horizontal accuracy",
            self.horizontal_accuracy.map(|a| format!("{a} m")),
        );
        push("Producer", self.producer.clone());
        push("Compiled", self.compiled.clone());
        push("Security", self.security.clone());
        v
    }
}

fn fmt_arcsec(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}")
    }
}

/// Writes a DTED file (tests and fixtures): heights `values[row * cols + col]`, row 0 the
/// northern edge, `None` a void; origin the south-west post; intervals in tenths of an arc
/// second.
pub fn write(
    level: u8,
    origin: (i32, i32),
    interval: (u32, u32),
    cols: u32,
    rows: u32,
    values: &[Option<i16>],
) -> Vec<u8> {
    let ang = |v: i32, pos: u8, neg: u8| {
        let mut s = format!("{:03}0000", v.unsigned_abs()).into_bytes();
        s.push(if v < 0 { neg } else { pos });
        s
    };
    let mut d = Vec::new();
    d.extend_from_slice(b"UHL1");
    d.extend(ang(origin.1, b'E', b'W'));
    d.extend(ang(origin.0, b'N', b'S'));
    d.extend(format!("{:04}{:04}", interval.0, interval.1).bytes());
    d.extend(b"0030U  ");
    d.extend(b"            ");
    d.extend(format!("{cols:04}{rows:04}0").bytes());
    d.resize(UHL, b' ');
    let mut dsi = vec![b' '; DSI];
    dsi[..4].copy_from_slice(b"DSIU");
    dsi[59..64].copy_from_slice(format!("DTED{level}").as_bytes());
    dsi[141..144].copy_from_slice(b"MSL");
    dsi[144..149].copy_from_slice(b"WGS84");
    d.extend(dsi);
    let mut acc = vec![b' '; ACC];
    acc[..3].copy_from_slice(b"ACC");
    acc[3..7].copy_from_slice(b"0050");
    acc[7..11].copy_from_slice(b"0030");
    d.extend(acc);
    for col in 0..cols as usize {
        let start = d.len();
        d.push(0xAA);
        d.extend_from_slice(&(col as u32).to_be_bytes()[1..]);
        d.extend_from_slice(&(col as u16).to_be_bytes());
        d.extend_from_slice(&0u16.to_be_bytes());
        for i in 0..rows as usize {
            let row = rows as usize - 1 - i;
            let v = values[row * cols as usize + col].unwrap_or(VOID);
            let raw = if v < 0 {
                0x8000 | v.unsigned_abs()
            } else {
                v as u16
            };
            d.extend_from_slice(&raw.to_be_bytes());
        }
        let sum: u32 = d[start..].iter().map(|&b| u32::from(b)).sum();
        d.extend_from_slice(&sum.to_be_bytes());
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let (cols, rows) = (4u32, 5u32);
        let vals: Vec<Option<i16>> = (0..cols * rows)
            .map(|i| match i {
                6 => None,
                7 => Some(-12),
                _ => Some(i as i16 * 10),
            })
            .collect();
        let d = write(0, (47, -12), (300, 300), cols, rows, &vals);
        assert!(sniff(&d));
        let h = header(&d).unwrap();
        assert_eq!((h.lon_lines, h.lat_points), (4, 5));
        assert_eq!((h.origin_lat, h.origin_lon), (47.0, -12.0));
        assert_eq!(h.level.as_deref(), Some("DTED0"));
        assert_eq!(h.horizontal_datum.as_deref(), Some("WGS84"));
        assert_eq!(h.vertical_accuracy, Some(30));
        let t = h.transform();
        // The south-west post's centre is the origin.
        let (x, y) = t.apply(0.5, 4.5);
        assert!((x + 12.0).abs() < 1e-12 && (y - 47.0).abs() < 1e-12);
        let (v, bad) = h.values(&d);
        assert!(!bad);
        assert_eq!(v[0], 0.0);
        assert!(v[6].is_nan());
        assert_eq!(v[7], -12.0);
        assert_eq!(v[19], 190.0);
        assert_eq!(h.data_len(), d.len());
        assert!(header(&d[..40]).is_err());
    }

    #[test]
    fn tape_labels_swaps_and_partial_tiles() {
        let vals: Vec<Option<i16>> = (0..9).map(|i| Some(i as i16)).collect();
        let d = write(0, (43, -80), (300, 300), 3, 3, &vals);
        let rl = 12 + 2 * 3;
        // Tape labels before the UHL, and a file cut off inside the headers.
        let mut t = b"HDR1DMATERRAIN".to_vec();
        t.resize(UHL, b' ');
        t.extend_from_slice(&d);
        assert!(sniff(&t));
        let h = header(&t).unwrap();
        assert_eq!((h.offset, h.origin_lon, h.origin_lat), (80, -80.0, 43.0));
        assert_eq!(h.values(&t).0, header(&d).unwrap().values(&d).0);
        let cut = header(&t[..UHL + 500]).unwrap();
        assert!(cut.level.is_none() && cut.values(&t[..UHL + 500]).1);
        assert!(!sniff(b"XXXX0000UHL1"));
        // Latitude and longitude fields swapped.
        let mut s = d.clone();
        let (lon, lat) = (s[4..12].to_vec(), s[12..20].to_vec());
        s[4..12].copy_from_slice(&lat);
        s[12..20].copy_from_slice(&lon);
        let h = header(&s).unwrap();
        assert_eq!((h.origin_lon, h.origin_lat), (-80.0, 43.0));
        // Only the line for column 2 present: the others are voids.
        let mut p = d[..DATA].to_vec();
        p.extend_from_slice(&d[DATA + 2 * rl..DATA + 3 * rl]);
        let (v, bad) = header(&p).unwrap().values(&p);
        assert!(bad && v[0].is_nan() && v[1].is_nan());
        assert_eq!(v[2], 2.0);
    }

    #[test]
    fn hostile_non_ascii_origin() {
        // "04é000N": eight bytes whose `[..3]` ends inside the two-byte "é".
        let vals = vec![Some(0i16); 4];
        let mut d = write(0, (43, -80), (300, 300), 2, 2, &vals);
        d[4..12].copy_from_slice("04é000N".as_bytes());
        assert!(header(&d).is_err());
    }
}

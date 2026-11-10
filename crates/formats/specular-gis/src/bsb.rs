//! BSB/KAP raster nautical charts (NOAA, Maptech and other hydrographic offices): a text
//! header of `TAG/fields` lines (the chart's name, size and scale, its projection and
//! datum, the day palette `RGB/n,r,g,b`, reference points `REF/n,x,y,lat,lon` and the
//! border `PLY/n,lat,lon`) ended by Ctrl-Z and NUL, then the bit depth and run-length
//! coded rows, often with a row index at the end. A `.bsb` file is the chart's text
//! description, naming its panels (`K01/…,FN=name.kap`).
//!
//! The chart is placed by fitting its reference points: in Mercator (most charts) or
//! Transverse Mercator when it says so, else in longitude and latitude.

use crate::crs::{self, Crs};
use crate::geotiff::Affine2;
use crate::proj::{Ellipsoid, Method};

/// A reference point: pixel position and longitude/latitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ref {
    pub x: f64,
    pub y: f64,
    pub lon: f64,
    pub lat: f64,
}

/// A chart's header.
#[derive(Clone, Debug, Default)]
pub struct Chart {
    pub width: u32,
    pub height: u32,
    /// Bits per pixel (1–7).
    pub depth: u8,
    /// Index 0 unused (black); `RGB/n` at `n`.
    pub palette: Vec<[u8; 3]>,
    pub name: Option<String>,
    pub scale: Option<f64>,
    pub projection: Option<String>,
    /// The projection parameter (`PP=`): the latitude of true scale (Mercator) or the
    /// central meridian (Transverse Mercator).
    pub pp: Option<f64>,
    pub datum: Option<String>,
    pub refs: Vec<Ref>,
    /// The chart's border, longitude/latitude.
    pub border: Vec<[f64; 2]>,
    /// Datum shift to WGS 84 in seconds of latitude and longitude (`DTM/`).
    pub dtm: (f64, f64),
    /// Header lines as (tag, text).
    pub keys: Vec<(String, String)>,
    /// Where the row data starts (after the depth byte).
    pub data_start: usize,
}

/// Header bytes looked at for the end of the header, at most.
const MAX_HEADER: usize = 4 << 20;

/// Whether the start of a file is a BSB/KAP header.
pub fn sniff(head: &[u8]) -> bool {
    let t = &head[..head.len().min(8192)];
    let has = |p: &[u8]| t.windows(p.len()).any(|w| w == p);
    (has(b"\nBSB/") || has(b"\nNOS/") || t.starts_with(b"BSB/"))
        && (has(b"RA=") || has(b"VER/"))
        && !has(b"\nK01/")
}

/// Whether the text is a `.bsb` chart description (it names panels and holds no raster).
pub fn sniff_description(head: &[u8]) -> bool {
    let t = &head[..head.len().min(8192)];
    let has = |p: &[u8]| t.windows(p.len()).any(|w| w == p);
    (has(b"CHT/") || has(b"BSB/")) && has(b"K01/")
}

/// The first panel file a `.bsb` description names (`K01/…,FN=name.kap`).
pub fn first_panel(text: &str) -> Option<String> {
    let (_, rest) = text.split_once("K01/")?;
    let line: String = rest.lines().take(4).collect::<Vec<_>>().join(",");
    line.split(',')
        .map(str::trim)
        .find_map(|f| f.strip_prefix("FN="))
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
}

/// `k=v` fields of a header line's text (commas between, values may hold commas when the
/// next item has no `=`: `SP=Unknown,0`).
fn fields(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for item in text.split(',') {
        match item.split_once('=') {
            Some((k, v))
                if !k.trim().is_empty() && k.trim().chars().all(|c| c.is_ascii_alphanumeric()) =>
            {
                out.push((k.trim().to_ascii_uppercase(), v.trim().to_owned()))
            }
            _ => {
                if let Some(last) = out.last_mut() {
                    last.1.push(',');
                    last.1.push_str(item.trim());
                }
            }
        }
    }
    out
}

fn numbers(text: &str) -> Vec<f64> {
    text.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// Parses the header (everything up to Ctrl-Z NUL and the depth byte).
pub fn parse(data: &[u8]) -> Result<Chart, String> {
    let lim = &data[..data.len().min(MAX_HEADER)];
    let end = lim
        .windows(3)
        .position(|w| w[0] == 0x1A && w[1] == 0 && (1..=7).contains(&w[2]))
        .ok_or("no end of the chart header")?;
    let text: String = data[..end].iter().map(|&b| char::from(b)).collect();
    // Lines; a line starting with a space continues the one before.
    let mut lines: Vec<String> = Vec::new();
    for l in text.split(['\r', '\n']) {
        if l.is_empty() {
            continue;
        }
        if l.starts_with([' ', '\t'])
            && let Some(last) = lines.last_mut()
        {
            last.push(',');
            last.push_str(l.trim());
            continue;
        }
        lines.push(l.trim_end().to_owned());
    }
    let mut c = Chart {
        depth: data[end + 2],
        data_start: end + 3,
        palette: vec![[0, 0, 0]],
        ..Chart::default()
    };
    for l in &lines {
        if l.starts_with('!') {
            continue;
        }
        let Some((tag, body)) = l.split_once('/') else {
            continue;
        };
        let tag = tag.trim().to_ascii_uppercase();
        let body = body.trim().trim_start_matches(',');
        if c.keys.len() < 4096 {
            c.keys.push((tag.clone(), body.to_owned()));
        }
        match tag.as_str() {
            "BSB" | "NOS" => {
                for (k, v) in fields(body) {
                    match k.as_str() {
                        "NA" => c.name = Some(v),
                        "RA" => {
                            let n = numbers(&v);
                            // `RA=width,height` (BSB 2+), or `RA=x0,y0,x1,y1` (NOS).
                            let (w, h) = match n.len() {
                                2 => (n[0], n[1]),
                                4 => (n[2] - n[0] + 1.0, n[3] - n[1] + 1.0),
                                _ => continue,
                            };
                            c.width = w.max(0.0).min(f64::from(u32::MAX)) as u32;
                            c.height = h.max(0.0).min(f64::from(u32::MAX)) as u32;
                        }
                        _ => {}
                    }
                }
            }
            "KNP" => {
                for (k, v) in fields(body) {
                    match k.as_str() {
                        "SC" => c.scale = v.parse().ok(),
                        "PR" => c.projection = Some(v),
                        "PP" => c.pp = v.parse().ok(),
                        "GD" => c.datum = Some(v),
                        _ => {}
                    }
                }
            }
            "RGB" => {
                let n = numbers(body);
                if n.len() >= 4 && n[0] >= 1.0 && n[0] < 256.0 {
                    let i = n[0] as usize;
                    if c.palette.len() <= i {
                        c.palette.resize(i + 1, [0, 0, 0]);
                    }
                    let b = |v: f64| v.clamp(0.0, 255.0) as u8;
                    c.palette[i] = [b(n[1]), b(n[2]), b(n[3])];
                }
            }
            "REF" => {
                let n = numbers(body);
                if n.len() >= 5 && c.refs.len() < 10_000 {
                    c.refs.push(Ref {
                        x: n[1],
                        y: n[2],
                        lat: n[3],
                        lon: n[4],
                    });
                }
            }
            "PLY" => {
                let n = numbers(body);
                if n.len() >= 3 && c.border.len() < 100_000 {
                    c.border.push([n[2], n[1]]);
                }
            }
            "DTM" => {
                let n = numbers(body);
                if n.len() >= 2 {
                    c.dtm = (n[0], n[1]);
                }
            }
            _ => {}
        }
    }
    if c.width == 0 || c.height == 0 {
        return Err("the chart's size (RA=) is missing".into());
    }
    Ok(c)
}

/// A row number (7 bits a byte, high bit set on all but the last) at `p`, and where it ends.
fn varint(data: &[u8], mut p: usize) -> Option<(u64, usize)> {
    let mut num: u64 = 0;
    loop {
        let b = *data.get(p)?;
        p += 1;
        num = (num << 7) | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            return Some((num, p));
        }
        if num > u64::from(u32::MAX) {
            return None;
        }
    }
}

/// A row: its number and pixels from `p`, the index after its end NUL.
fn row(data: &[u8], mut p: usize, depth: u8, out: &mut [u8]) -> Option<(u64, usize, bool)> {
    let (num, start) = varint(data, p)?;
    p = start;
    let shift = 7 - depth;
    let count_mask = (1u32 << shift) - 1;
    let w = out.len();
    let mut x = 0usize;
    loop {
        let b = *data.get(p)?;
        p += 1;
        if b == 0 {
            // A NUL inside a row that is not full, not followed by the next row's number,
            // is a stray byte (as GDAL reads it): the row goes on.
            if x < w && varint(data, p).is_some_and(|(n, _)| n != num + 1) {
                continue;
            }
            return Some((num, p, x >= w));
        }
        let colour = (b & 0x7F) >> shift;
        let mut run = u32::from(b) & count_mask;
        let mut more = b & 0x80 != 0;
        while more {
            let b = *data.get(p)?;
            p += 1;
            run = run.saturating_mul(128).saturating_add(u32::from(b & 0x7F));
            more = b & 0x80 != 0;
        }
        let n = (run as usize).saturating_add(1).min(w.saturating_sub(x));
        out[x..x + n].fill(colour);
        x += n;
    }
}

impl Chart {
    /// The palette indices, row 0 the top; true when rows were missing or damaged (left 0).
    pub fn indices(&self, data: &[u8]) -> (Vec<u8>, bool) {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0u8; w * h];
        let depth = self.depth.clamp(1, 7);
        let mut bad = false;
        // The row index: `h` big-endian offsets, its own offset in the last four bytes.
        let index = (data.len() >= 4)
            .then(|| {
                let at = u32::from_be_bytes(data[data.len() - 4..].try_into().ok()?) as usize;
                (at >= self.data_start && at.checked_add(4 * h)? + 4 == data.len()).then_some(at)
            })
            .flatten();
        let mut done = vec![false; h];
        if let Some(at) = index {
            for (y, line) in out.chunks_exact_mut(w).enumerate() {
                let o = &data[at + 4 * y..at + 4 * y + 4];
                let off = u32::from_be_bytes([o[0], o[1], o[2], o[3]]) as usize;
                match row(data, off, depth, line) {
                    Some((_, _, full)) if off >= self.data_start => {
                        done[y] = true;
                        bad |= !full;
                    }
                    _ => bad = true,
                }
            }
        } else {
            // Rows in turn, each numbered (from 1, or from 0 in some files).
            let mut p = self.data_start;
            let mut first: Option<u64> = None;
            let mut line = vec![0u8; w];
            while p < data.len() {
                line.fill(0);
                let Some((num, next, full)) = row(data, p, depth, &mut line) else {
                    bad = true;
                    break;
                };
                p = next;
                let base = *first.get_or_insert(num.min(1));
                let y = num.wrapping_sub(base) as usize;
                if y < h {
                    out[y * w..(y + 1) * w].copy_from_slice(&line);
                    done[y] = true;
                    bad |= !full;
                }
                if done.iter().all(|d| *d) {
                    break;
                }
            }
        }
        bad |= done.iter().any(|d| !d);
        (out, bad)
    }

    /// The chart's projection for the reference points, and whether it is the chart's own.
    fn plane(&self) -> (Option<Crs>, bool) {
        let ell = Ellipsoid::WGS84;
        let pr = self
            .projection
            .as_deref()
            .unwrap_or("")
            .to_ascii_uppercase();
        if pr.starts_with("MERCATOR") {
            // The latitude of true scale only scales the plane, which the fit absorbs.
            let c = crs::from_epsg(3395).unwrap_or_else(|| {
                Crs::custom_projected(
                    "World Mercator".into(),
                    ell,
                    Method::Mercator {
                        lon0: 0.0,
                        k0: 1.0,
                        fe: 0.0,
                        fn_: 0.0,
                    },
                    crate::crs::LinearUnit::metre(),
                )
            });
            (Some(c), true)
        } else if pr.starts_with("TRANSVERSE MERCATOR") {
            let lon0 = self
                .keys
                .iter()
                .find(|(t, _)| t == "KNQ")
                .and_then(|(_, b)| {
                    fields(b)
                        .into_iter()
                        .find(|(k, _)| k == "P1")
                        .and_then(|(_, v)| v.parse().ok())
                })
                .or(self.pp);
            match lon0 {
                Some(lon0) => (
                    Some(Crs::custom_projected(
                        format!("Transverse Mercator ({lon0}°)"),
                        ell,
                        Method::TransverseMercator {
                            lat0: 0.0,
                            lon0,
                            k0: 1.0,
                            fe: 0.0,
                            fn_: 0.0,
                        },
                        crate::crs::LinearUnit::metre(),
                    )),
                    true,
                ),
                None => (crs::from_epsg(4326), false),
            }
        } else if pr.starts_with("LAMBERT CONFORMAL CONIC") {
            let p = |k: &str| {
                self.keys
                    .iter()
                    .find(|(t, _)| t == "KNQ")
                    .and_then(|(_, b)| {
                        fields(b)
                            .into_iter()
                            .find(|(n, _)| n == k)
                            .and_then(|(_, v)| v.parse::<f64>().ok())
                    })
            };
            match (self.pp, p("P2"), p("P3")) {
                (Some(lon0), Some(lat1), Some(lat2)) => (
                    Some(Crs::custom_projected(
                        format!("Lambert conformal conic ({lat1}°, {lat2}°)"),
                        ell,
                        Method::LambertConic {
                            lat0: 0.0,
                            lon0,
                            lat1,
                            lat2,
                            k0: 1.0,
                            fe: 0.0,
                            fn_: 0.0,
                        },
                        crate::crs::LinearUnit::metre(),
                    )),
                    true,
                ),
                _ => (crs::from_epsg(4326), false),
            }
        } else {
            (crs::from_epsg(4326), false)
        }
    }

    /// The reference points with the datum shift applied (longitude, latitude on WGS 84).
    pub fn wgs84_refs(&self) -> Vec<Ref> {
        self.refs
            .iter()
            .map(|r| Ref {
                lat: r.lat + self.dtm.0 / 3600.0,
                lon: r.lon + self.dtm.1 / 3600.0,
                ..*r
            })
            .collect()
    }

    /// The georeference fitted to the reference points: pixel corner to map, the CRS, the
    /// largest miss at a reference point in pixels, and whether the chart's projection was
    /// used (else longitude and latitude stand in for it).
    pub fn georef(&self) -> Option<(Affine2, Crs, f64, bool)> {
        let (crs, own) = self.plane();
        let crs = crs?;
        let refs = self.wgs84_refs();
        let pairs: Vec<([f64; 2], [f64; 2])> = refs
            .iter()
            .filter_map(|r| {
                let (x, y) = if crs.kind == crs::CrsKind::Geographic {
                    (r.lon, r.lat)
                } else {
                    crs.from_lonlat(r.lon, r.lat)?
                };
                // Pixel coordinates as given (GDAL takes them as raster coordinates too).
                Some(([r.x, r.y], [x, y]))
            })
            .collect();
        let t = Affine2::fit(&pairs)?;
        let inv = t.inverse()?;
        let miss = pairs
            .iter()
            .map(|(p, m)| {
                let (x, y) = inv.apply(m[0], m[1]);
                (x - p[0]).hypot(y - p[1])
            })
            .fold(0.0f64, f64::max);
        Some((t, crs, miss, own))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(rows: &[u8], index: bool) -> Vec<u8> {
        let mut v = b"! test\r\nVER/3.0\r\nBSB/NA=Test chart\r\n    NU=1,RA=4,2,DU=254\r\nKNP/SC=25000,GD=WGS84,PR=MERCATOR,PP=0.0\r\nRGB/1,255,0,0\r\nRGB/2,0,0,255\r\nREF/1,0,0,50.0,1.0\r\nREF/2,3,0,50.0,1.3\r\nREF/3,0,1,49.9,1.0\r\nDTM/0,0\r\n".to_vec();
        v.extend([0x1A, 0x00, 2]);
        let start = v.len();
        v.extend(rows);
        if index {
            let at = v.len() as u32;
            // Row offsets: row 1 first, then row 2.
            let r2 = start + rows.iter().position(|&b| b == 0).unwrap() + 1;
            v.extend((start as u32).to_be_bytes());
            v.extend((r2 as u32).to_be_bytes());
            v.extend(at.to_be_bytes());
        }
        v
    }

    #[test]
    fn rows_and_palette() {
        // Depth 2: colour in bits 6–5, count in bits 4–0. Row 1: colour 1 ×3, colour 2 ×1;
        // row 2: colour 2 ×4.
        let rows = [1, 0b0010_0010, 0b0100_0000, 0, 2, 0b0100_0011, 0];
        for index in [false, true] {
            let d = chart(&rows, index);
            assert!(sniff(&d));
            let c = parse(&d).unwrap();
            assert_eq!((c.width, c.height, c.depth), (4, 2, 2));
            assert_eq!(c.name.as_deref(), Some("Test chart"));
            assert_eq!(c.palette[2], [0, 0, 255]);
            let (px, bad) = c.indices(&d);
            assert!(!bad);
            assert_eq!(px, [1, 1, 1, 2, 2, 2, 2, 2]);
        }
        // A missing row is reported.
        let d = chart(&rows[..4], false);
        let (px, bad) = parse(&d).unwrap().indices(&d);
        assert!(bad);
        assert_eq!(&px[..4], [1, 1, 1, 2]);
    }

    #[test]
    fn long_runs_and_georef() {
        // Depth 2, a run continued in a second byte: (1 << 7) + 3 + 1 = 132 pixels, cut at
        // the row's width.
        let mut line = vec![0u8; 4];
        let r = row(&[1, 0b1010_0001, 0b0000_0011, 0], 0, 2, &mut line).unwrap();
        assert_eq!((r.0, r.2), (1, true));
        assert_eq!(line, [1; 4]);
        let d = chart(&[1, 0b0010_0011, 0, 2, 0b0010_0011, 0], false);
        let c = parse(&d).unwrap();
        let (t, crs, miss, own) = c.georef().unwrap();
        assert!(own && crs.epsg == Some(3395), "{:?}", crs.name);
        assert!(miss < 1e-6, "{miss}");
        let (x, y) = t.apply(0.0, 0.0);
        let (lon, lat) = crs.to_lonlat(x, y).unwrap();
        assert!((lon - 1.0).abs() < 1e-9 && (lat - 50.0).abs() < 1e-9);
    }

    #[test]
    fn description() {
        let t = "CHT/NA=Harbour,NU=12\nK01/NA=Main panel\n    FN=12345_1.KAP\nK02/NA=Inset,FN=12345_2.KAP\n";
        assert!(sniff_description(t.as_bytes()));
        assert!(!sniff(t.as_bytes()));
        assert_eq!(first_panel(t).as_deref(), Some("12345_1.KAP"));
    }
}

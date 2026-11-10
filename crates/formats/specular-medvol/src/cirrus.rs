//! Zeiss Cirrus HD-OCT raw exports (`.img`): 8-bit samples with no header, each A-scan's
//! depth samples together, then the A-scans of a B-scan, then the B-scans. The export's
//! name carries the scan ("…_Macular Cube 512x128_7-25-2012_14-20-23_OD_sn12345_cube_raw.img"):
//! A-scans per B-scan × B-scans; the depth is what fills the file (1024 on Cirrus).
//!
//! A volume is the B-scans as slices, each A-scans across and depth down, as Zeiss and
//! oct-converter show them. Cirrus cubes cover 6 × 6 mm and 2 mm of depth; the name does
//! not say so, so the spacing is nominal and the orientation the scan's own
//! ([`OrientationSource::Object`]).

use std::path::Path;

use crate::{
    DataFile, DataRef, DataType, Encoding, Error, Header, Kind, Layout, OrientationSource, Param,
    ParamValue, Result, Skip,
};

/// Depths a Cirrus export is accepted with.
const DEPTHS: [usize; 3] = [512, 1024, 2048];
/// Nominal scan area (mm, both lateral directions) and depth (mm).
const SCAN_MM: f64 = 6.0;
const DEPTH_MM: f64 = 2.0;

/// What a Cirrus export's name and size say.
#[derive(Clone, Debug, PartialEq)]
pub struct Scan {
    /// A-scans per B-scan, B-scans, samples per A-scan.
    pub ascans: usize,
    pub bscans: usize,
    pub depth: usize,
    /// "Macular Cube", "Optic Disc Cube", … as named.
    pub protocol: Option<String>,
    /// "OD" (right eye) or "OS" (left eye).
    pub eye: Option<&'static str>,
    /// Acquisition as named: "2012-07-25 14:20:23".
    pub date: Option<String>,
}

/// The `AxB` size token of a name: the first `digits x digits` pair.
fn size_token(name: &str) -> Option<(usize, usize, usize)> {
    let b = name.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'x' || b[i] == b'X') {
                let x = i;
                i += 1;
                let t = i;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if i > t && (i == b.len() || !b[i].is_ascii_alphanumeric()) {
                    let a = name[s..x].parse().ok()?;
                    let c = name[t..i].parse().ok()?;
                    return Some((a, c, s));
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// The scan a file named `path` of `len` bytes is, when it is a Cirrus raw export: an
/// `.img` named with `AxB`, for an eye (`_OD_`, `_OS_`) or as a `cube`/`raster` export, that
/// `A × B × depth` bytes fill exactly.
pub fn scan(path: &Path, len: u64) -> Option<Scan> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("img"))
    {
        return None;
    }
    let name = path.file_stem()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let eye = if lower.contains("_od_") || lower.ends_with("_od") {
        Some("OD")
    } else if lower.contains("_os_") || lower.ends_with("_os") {
        Some("OS")
    } else {
        None
    };
    if eye.is_none() && !lower.contains("cube") && !lower.contains("raster") {
        return None;
    }
    let (ascans, bscans, at) = size_token(name)?;
    if !(1..=4096).contains(&ascans) || !(1..=4096).contains(&bscans) {
        return None;
    }
    let plane = (ascans * bscans) as u64;
    let depth = DEPTHS.into_iter().find(|&d| plane * d as u64 == len)?;
    // The protocol is the words before the size, after the patient field.
    let before = name[..at].trim_end();
    let protocol = before
        .split_once('_')
        .map_or(before, |(_, p)| p)
        .trim()
        .to_owned();
    Some(Scan {
        ascans,
        bscans,
        depth,
        protocol: (!protocol.is_empty()).then_some(protocol),
        eye,
        date: date_in(name),
    })
}

/// "M-D-YYYY_H-M-S" in a name, as "YYYY-MM-DD HH:MM:SS".
fn date_in(name: &str) -> Option<String> {
    for (i, _) in name.match_indices('_') {
        let rest = &name[i + 1..];
        let mut parts = rest.splitn(3, '_');
        let (Some(d), Some(t)) = (parts.next(), parts.next()) else {
            continue;
        };
        let dn: Vec<u32> = d.split('-').filter_map(|x| x.parse().ok()).collect();
        let tn: Vec<u32> = t.split('-').filter_map(|x| x.parse().ok()).collect();
        if let ([m, day, y], [h, mi, s]) = (dn.as_slice(), tn.as_slice())
            && (1..=12).contains(m)
            && (1..=31).contains(day)
            && *y >= 1990
            && *h < 24
            && *mi < 60
            && *s < 61
        {
            return Some(format!("{y:04}-{m:02}-{day:02} {h:02}:{mi:02}:{s:02}"));
        }
    }
    None
}

/// The volume header of a Cirrus export.
pub fn header(s: &Scan) -> Result<Header> {
    let size = [s.ascans, s.depth, s.bscans];
    let samples = s
        .ascans
        .checked_mul(s.depth)
        .and_then(|n| n.checked_mul(s.bscans))
        .ok_or(Error::TooLarge)?;
    let spacing = [
        SCAN_MM / s.ascans as f64,
        DEPTH_MM / s.depth as f64,
        SCAN_MM / s.bscans as f64,
    ];
    let mut params = Vec::new();
    if let Some(e) = s.eye {
        params.push(Param {
            key: "eye",
            value: ParamValue::Text(e.to_owned()),
        });
    }
    if let Some(d) = &s.date {
        params.push(Param {
            key: "date",
            value: ParamValue::Text(d.clone()),
        });
    }
    Ok(Header {
        kind: Kind::Cirrus,
        dtype: DataType::U8,
        layout: Layout {
            size,
            // Depth samples of an A-scan together, then A-scans, then B-scans.
            stride: [s.depth, 1, s.ascans * s.depth],
            extra: Vec::new(),
            colour: None,
            samples,
        },
        affine: [
            [spacing[0], 0.0, 0.0, 0.0],
            [0.0, spacing[1], 0.0, 0.0],
            [0.0, 0.0, spacing[2], 0.0],
        ],
        orientation: OrientationSource::Object,
        dims: size.to_vec(),
        spacing,
        scale: None,
        cal: None,
        description: s.protocol.clone(),
        intent: None,
        anatomical: None,
        params,
        data: DataRef {
            file: DataFile::Attached,
            skip: Skip::Start { lines: 0, bytes: 0 },
            encoding: Encoding::Raw,
            big_endian: false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let p = Path::new("P1234_Macular Cube 512x128_7-25-2012_14-20-23_OD_sn12345_cube_raw.img");
        let s = scan(p, 512 * 128 * 1024).unwrap();
        assert_eq!((s.ascans, s.bscans, s.depth), (512, 128, 1024));
        assert_eq!(s.protocol.as_deref(), Some("Macular Cube"));
        assert_eq!(s.eye, Some("OD"));
        assert_eq!(s.date.as_deref(), Some("2012-07-25 14:20:23"));
        assert!(scan(p, 512 * 128 * 1024 + 1).is_none());
        let disc = Path::new("P9_Optic Disc Cube 200x200_1-2-2015_9-8-7_OS_sn1_cube_z.img");
        assert_eq!(scan(disc, 200 * 200 * 1024).unwrap().eye, Some("OS"));
        // A disk image, or a size without anything Cirrus about it.
        assert!(scan(Path::new("disk.img"), 1 << 20).is_none());
        assert!(scan(Path::new("map 64x64.img"), 64 * 64 * 1024).is_none());
        assert!(scan(Path::new("cube 64x64.raw"), 64 * 64 * 1024).is_none());
    }

    #[test]
    fn layout() {
        let s = scan(Path::new("x_cube 4x3_OD.img"), 4 * 3 * 512).unwrap();
        let h = header(&s).unwrap();
        assert_eq!(h.layout.size, [4, 512, 3]);
        assert_eq!(h.layout.stride, [512, 1, 2048]);
        assert_eq!(h.data_bytes(), Some(4 * 3 * 512));
    }
}

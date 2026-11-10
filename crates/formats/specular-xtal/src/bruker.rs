//! Bruker SFRM frames (SMART, APEX, PHOTON; `.sfrm` and the old numbered `.NNNN` names):
//! 80-character `KEY    :value` records in `HDRBLKS` 512-byte blocks, then little-endian
//! unsigned pixels.
//!
//! * Version 86 and below: `NPIXELB` bytes per pixel; `NOVERFL` 16-character ASCII records
//!   follow the data, each a 9-digit count and a 7-digit pixel index replacing that pixel.
//! * Version 100 and up: `NPIXELB` is "pixel bytes, underflow bytes"; `NOVERFL` is
//!   "underflows, 1-byte overflows, 2-byte overflows". After the data come the underflow table
//!   (signed), a 16-bit table replacing every 255 of 8-bit data in order, and a 32-bit table
//!   replacing every 65535 in order, each padded to 16 bytes. Stored values are counts minus
//!   a baseline (`NEXP`'s third number) unless `NOVERFL` starts with -1; with an underflow
//!   table, zeros take the table's values instead of the baseline.

use crate::geometry::Geometry;
use crate::{Coding, Error, Frame, Image, Kind, Result, Sample, bad, finite, positive, text};

/// Whether the file starts with a Bruker header (`FORMAT :` and a version number; 24 bytes
/// are enough).
pub fn sniff(head: &[u8]) -> bool {
    let Some(rest) = head.strip_prefix(b"FORMAT :") else {
        return false;
    };
    let rest = &rest[..rest.len().min(72)];
    let t = rest.trim_ascii_start();
    let digits = t.iter().take_while(|b| b.is_ascii_digit()).count();
    digits > 0 && t[digits..].iter().all(|b| *b == b' ' || *b == 0)
}

/// Where a frame's pixels and overflow tables are.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    offset: usize,
    bpp: usize,
    version: u32,
    /// v86: the ASCII overflow records.
    records: usize,
    linear: (f64, f64),
    /// v100: table counts (underflow < 0: none), underflow width, baseline.
    under: i64,
    under_bpp: usize,
    over1: usize,
    over2: usize,
    baseline: i64,
}

fn pad16(n: usize) -> usize {
    n.div_ceil(16) * 16
}

pub(crate) fn parse(data: &[u8]) -> Result<Image> {
    // The block count is in the first five blocks.
    let first = String::from_utf8_lossy(&data[..data.len().min(5 * 512)]);
    let rec = |t: &str, key: &str| -> Option<String> {
        t.as_bytes().chunks(80).find_map(|r| {
            let r = String::from_utf8_lossy(r);
            let (k, v) = r.split_at_checked(8)?;
            (k.trim_end_matches(':').trim() == key).then(|| v.trim().to_owned())
        })
    };
    let blocks = rec(&first, "HDRBLKS")
        .and_then(|v| v.split_whitespace().next()?.parse::<usize>().ok())
        .filter(|b| (1..=10_000).contains(b))
        .ok_or_else(|| bad("HDRBLKS"))?;
    let hlen = blocks * 512;
    if hlen > data.len() {
        return Err(Error::Truncated);
    }
    // Every record, repeated keys joined (TITLE, CELL, MATRIX…).
    let mut header: Vec<(String, String)> = Vec::new();
    for r in data[..hlen].chunks(80) {
        let r = String::from_utf8_lossy(r);
        let Some((k, v)) = r.split_at_checked(8) else {
            continue;
        };
        let k = k.trim_end_matches(':').trim();
        if k.is_empty() || !k.bytes().all(|b| b.is_ascii_graphic()) || k.starts_with("..") {
            continue;
        }
        let v = v.trim_matches(['\0', ' ']).trim();
        match header.iter_mut().find(|(hk, _)| hk == k) {
            Some((_, hv)) => {
                if !v.is_empty() {
                    if !hv.is_empty() {
                        hv.push(' ');
                    }
                    hv.push_str(v);
                }
            }
            None => header.push((k.to_owned(), v.to_owned())),
        }
    }
    let get = |k: &str| {
        header
            .iter()
            .find(|(hk, _)| hk == k)
            .map(|(_, v)| v.as_str())
    };
    let nums = |k: &str| -> Vec<f64> {
        get(k)
            .map(|v| crate::numbers(v).collect())
            .unwrap_or_default()
    };
    let ints = |k: &str| -> Vec<i64> {
        get(k)
            .map(|v| {
                v.split_whitespace()
                    .filter_map(|t| t.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    let version = ints("FORMAT").first().copied().unwrap_or(86);
    let version = u32::try_from(version).map_err(|_| bad("FORMAT"))?;
    let rows = *ints("NROWS").first().ok_or_else(|| bad("NROWS"))?;
    let cols = *ints("NCOLS").first().ok_or_else(|| bad("NCOLS"))?;
    let w = u32::try_from(cols).map_err(|_| bad("NCOLS"))?;
    let h = u32::try_from(rows).map_err(|_| bad("NROWS"))?;
    let pb = ints("NPIXELB");
    let bpp = usize::try_from(*pb.first().unwrap_or(&2)).unwrap_or(0);
    if !matches!(bpp, 1 | 2 | 4) {
        return Err(Error::Unsupported(format!("Bruker {bpp} bytes per pixel")));
    }
    if let Some(c) = get("COMPRES")
        && !c.is_empty()
        && !c.eq_ignore_ascii_case("none")
    {
        return Err(Error::Unsupported(format!("Bruker compression \"{c}\"")));
    }
    let novf = ints("NOVERFL");
    let mut l = Layout {
        offset: hlen,
        bpp,
        version,
        records: 0,
        linear: (1.0, 0.0),
        under: -1,
        under_bpp: 1,
        over1: 0,
        over2: 0,
        baseline: 0,
    };
    if version < 100 {
        l.records = usize::try_from(*novf.first().unwrap_or(&0)).unwrap_or(0);
        let lin = nums("LINEAR");
        if lin.len() >= 2 && lin[0] != 0.0 {
            l.linear = (lin[0], lin[1]);
        }
    } else {
        l.under = *novf.first().unwrap_or(&-1);
        l.over1 = usize::try_from(*novf.get(1).unwrap_or(&0)).unwrap_or(0);
        l.over2 = usize::try_from(*novf.get(2).unwrap_or(&0)).unwrap_or(0);
        l.under_bpp = usize::try_from(*pb.get(1).unwrap_or(&1)).unwrap_or(1);
        if !matches!(l.under_bpp, 1 | 2 | 4) {
            return Err(bad("underflow bytes per pixel"));
        }
        if l.under >= 0 {
            l.baseline = *ints("NEXP").get(2).unwrap_or(&0);
        }
    }

    let mut g = Geometry::default();
    // The goniometer: 2θ, ω, φ, χ.
    let ang = nums("ANGLES");
    if ang.len() >= 4 {
        g.two_theta = Some(ang[0]);
        g.omega = Some(ang[1]);
        g.phi = Some(ang[2]);
        g.chi = Some(ang[3]);
    }
    g.osc_start = finite(nums("START").first().copied());
    g.osc_width =
        finite(nums("INCREME").first().copied()).or_else(|| finite(nums("RANGE").first().copied()));
    g.osc_axis = match ints("AXIS").first() {
        Some(1) => Some("2θ".to_owned()),
        Some(2) => Some("ω".to_owned()),
        Some(3) => Some("φ".to_owned()),
        Some(4) => Some("χ".to_owned()),
        _ => None,
    };
    g.distance = positive(nums("DISTANC").first().copied()).map(|cm| cm * 10.0);
    g.wavelength = positive(nums("WAVELEN").first().copied());
    g.exposure = positive(nums("CUMULAT").first().copied());
    // The detector's pixels per cm (in 512 × 512 mode) are DETTYPE's second item; CENTER is
    // in 512-mode pixels with y up from the bottom row.
    let det = get("DETTYPE").unwrap_or("");
    let scale = 512.0 / f64::from(w.max(1));
    if let Some(ppcm) = positive(det.split_whitespace().nth(1).and_then(|t| t.parse().ok())) {
        let mm = 10.0 / ppcm * scale;
        g.pixel = Some((mm, mm));
    }
    let c = nums("CENTER");
    if c.len() >= 2 && (c[0] != 0.0 || c[1] != 0.0) {
        g.beam = Some((c[0] / scale, f64::from(h) - c[1] / scale));
    }
    if let Some(o) = positive(nums("CCDPARM").get(4).copied()) {
        g.overload = Some(o);
    }
    let detector = [
        get("DETTYPE").and_then(|d| d.split_whitespace().next()),
        get("MODEL"),
    ]
    .into_iter()
    .flatten()
    .filter_map(text)
    .reduce(|a, b| format!("{a} ({b})"));
    let date =
        get("CREATED").and_then(|d| text(&d.split_whitespace().collect::<Vec<_>>().join(" ")));
    let title = get("SAMPLE")
        .and_then(text)
        .or_else(|| get("TITLE").and_then(text));
    let sample = match bpp {
        1 => Sample::U8,
        2 => Sample::U16,
        _ => Sample::U32,
    };
    let sample = if version >= 100 || l.records > 0 {
        Sample::I32
    } else {
        sample
    };
    Ok(Image {
        kind: Kind::Bruker,
        width: w,
        height: h,
        sample,
        frames: vec![Frame {
            coding: Coding::Bruker(l),
            header: Vec::new(),
        }],
        header,
        geometry: g,
        detector,
        date,
        mask_negative: false,
        dummy: None,
        title,
    })
}

/// The frame's `n` pixels with overflow tables applied.
pub(crate) fn read(data: &[u8], l: &Layout, n: usize) -> Result<Vec<f32>> {
    let len = n.checked_mul(l.bpp).ok_or_else(|| bad("frame size"))?;
    let src = data
        .get(l.offset..l.offset.checked_add(len).ok_or(Error::Truncated)?)
        .ok_or(Error::Truncated)?;
    let mut v: Vec<i64> = match l.bpp {
        1 => src.iter().map(|&b| i64::from(b)).collect(),
        2 => src
            .chunks_exact(2)
            .map(|c| i64::from(u16::from_le_bytes([c[0], c[1]])))
            .collect(),
        _ => src
            .chunks_exact(4)
            .map(|c| i64::from(u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
            .collect(),
    };
    let mut at = l.offset + len;
    if l.version < 100 {
        // 16-character records: count (9), pixel index (7).
        for k in 0..l.records {
            let r = data
                .get(at + 16 * k..at + 16 * k + 16)
                .ok_or(Error::Truncated)?;
            let r = std::str::from_utf8(r).map_err(|_| bad("overflow record"))?;
            let (c, p) = r.split_at(9);
            let c: i64 = c.trim().parse().map_err(|_| bad("overflow count"))?;
            let p: usize = p.trim().parse().map_err(|_| bad("overflow pixel"))?;
            if let Some(x) = v.get_mut(p) {
                *x = c;
            }
        }
        let (s, o) = l.linear;
        if (s, o) != (1.0, 0.0) {
            return Ok(v.iter().map(|&x| (x as f64 * s + o) as f32).collect());
        }
        return Ok(v.iter().map(|&x| x as f32).collect());
    }
    at = l.offset + pad16(len);
    let table = |at: usize, count: usize, width: usize| -> Result<&[u8]> {
        // Table counts come from NOVERFL: an absurd one must not overflow the range.
        let end = count.checked_mul(width).and_then(|n| n.checked_add(at));
        data.get(at..end.ok_or(Error::Truncated)?)
            .ok_or(Error::Truncated)
    };
    let under: Vec<i64> = if l.under > 0 {
        let count = usize::try_from(l.under).map_err(|_| bad("NOVERFL"))?;
        let t = table(at, count, l.under_bpp)?;
        at += pad16(count * l.under_bpp);
        match l.under_bpp {
            1 => t.iter().map(|&b| i64::from(b as i8)).collect(),
            2 => t
                .chunks_exact(2)
                .map(|c| i64::from(i16::from_le_bytes([c[0], c[1]])))
                .collect(),
            _ => t
                .chunks_exact(4)
                .map(|c| i64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .collect(),
        }
    } else {
        Vec::new()
    };
    if l.bpp == 1 && l.over1 > 0 {
        let t = table(at, l.over1, 2)?;
        at += pad16(l.over1 * 2);
        let mut it = t
            .chunks_exact(2)
            .map(|c| i64::from(u16::from_le_bytes([c[0], c[1]])));
        for x in v.iter_mut().filter(|x| **x == 255) {
            match it.next() {
                Some(o) => *x = o,
                None => break,
            }
        }
    }
    if l.bpp < 4 && l.over2 > 0 {
        let t = table(at, l.over2, 4)?;
        let mut it = t
            .chunks_exact(4)
            .map(|c| i64::from(i32::from_le_bytes([c[0], c[1], c[2], c[3]])));
        for x in v.iter_mut().filter(|x| **x == 65535) {
            match it.next() {
                Some(o) => *x = o,
                None => break,
            }
        }
    }
    if under.is_empty() {
        if l.baseline != 0 {
            // NEXP's baseline is any integer: saturate rather than overflow.
            v.iter_mut().for_each(|x| *x = x.saturating_add(l.baseline));
        }
    } else {
        let mut it = under.into_iter();
        for x in &mut v {
            if *x == 0
                && let Some(u) = it.next()
            {
                *x = u;
                continue;
            }
            *x = x.saturating_add(l.baseline);
        }
    }
    Ok(v.iter().map(|&x| x as f32).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(recs: &[(&str, &str)]) -> Vec<u8> {
        let mut h = Vec::new();
        for (k, v) in recs {
            let mut r = format!("{k:<7}:{v}");
            r.truncate(80);
            h.extend(format!("{r:<80}").bytes());
        }
        h.resize(5 * 512, b' ');
        h
    }

    #[test]
    fn v86_overflow_records() {
        let mut d = header(&[
            ("FORMAT", "86"),
            ("VERSION", "9"),
            ("HDRBLKS", "5"),
            ("NOVERFL", "1"),
            ("NPIXELB", "2"),
            ("NROWS", "2"),
            ("NCOLS", "2"),
            ("DISTANC", "5.0"),
            ("WAVELEN", "0.71073 0.7093 0.71359"),
            ("CENTER", "256 256"),
            ("DETTYPE", "CCD-PXL 32.0 1.6"),
            ("ANGLES", "10 20 30 54.7"),
            ("AXIS", "3"),
        ]);
        assert!(sniff(&d));
        for v in [1u16, 65535, 3, 4] {
            d.extend(v.to_le_bytes());
        }
        d.extend(b"   145942      1");
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.kind, Kind::Bruker);
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, 145942.0, 3.0, 4.0]);
        let g = &im.geometry;
        assert_eq!(g.distance, Some(50.0));
        assert_eq!(g.wavelength, Some(0.71073));
        assert_eq!(g.two_theta, Some(10.0));
        assert_eq!(g.osc_axis.as_deref(), Some("φ"));
        // 512-mode centre and pixel size scaled to a 2 × 2 frame.
        assert_eq!(g.beam, Some((1.0, 1.0)));
        assert!((g.pixel.unwrap().0 - 80.0).abs() < 1e-9);
    }

    #[test]
    fn v100_tables() {
        let mut d = header(&[
            ("FORMAT", "100"),
            ("VERSION", "18"),
            ("HDRBLKS", "5"),
            ("NOVERFL", "2 2 1"),
            ("NPIXELB", "1 1"),
            ("NROWS", "2"),
            ("NCOLS", "4"),
            ("NEXP", "2 599 32 0 1"),
        ]);
        // Values minus the baseline 32; zeros take underflows; 255 → 16-bit table; its
        // 65535 → 32-bit table.
        d.extend([0u8, 1, 255, 0, 255, 5, 6, 7]);
        d.resize(d.len() + 8, 0);
        d.extend([0xfe, 0x05]); // underflows -2, 5
        d.resize(d.len() + 14, 0);
        d.extend(300u16.to_le_bytes());
        d.extend(65535u16.to_le_bytes());
        d.resize(d.len() + 12, 0);
        d.extend(100_000i32.to_le_bytes());
        d.resize(d.len() + 12, 0);
        let im = Image::parse(&d).unwrap();
        assert_eq!(
            im.frame(&d, 0).unwrap(),
            [-2.0, 33.0, 332.0, 5.0, 100_032.0, 37.0, 38.0, 39.0]
        );
    }

    mod hostile {
        use super::*;

        fn v100(novfl: &str, nexp: &str) -> Vec<u8> {
            let mut d = header(&[
                ("FORMAT", "100"),
                ("VERSION", "18"),
                ("HDRBLKS", "5"),
                ("NOVERFL", novfl),
                ("NPIXELB", "1 4"),
                ("NROWS", "2"),
                ("NCOLS", "4"),
                ("NEXP", nexp),
            ]);
            d.extend([1u8, 2, 3, 4, 5, 6, 7, 8]);
            d.resize(d.len() + 64, 0);
            d
        }

        #[test]
        fn underflow_count_overflowing_the_table_range() {
            let d = v100("4611686018427387904 0 0", "2 599 32 0 1");
            let im = Image::parse(&d).unwrap();
            assert!(im.frame(&d, 0).is_err());
        }

        #[test]
        fn baseline_at_the_integer_limit() {
            let d = v100("0 0 0", "2 599 9223372036854775807 0 1");
            let im = Image::parse(&d).unwrap();
            assert!(im.frame(&d, 0).is_ok());
        }
    }
}

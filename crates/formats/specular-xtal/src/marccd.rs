//! MarCCD / Rayonix `.mccd` frames: a TIFF file whose first 1024 bytes are the TIFF header
//! and directory, followed by the MarCCD frame header (`MMX` at 1028) and, at byte 4096, the
//! uncompressed image. The frame header's integers are in the byte order its
//! `header_byte_order` field (1234 little-endian, 4321 big-endian) says. Lengths are stored
//! ×1000 (mm, pixels, degrees), pixel sizes in nm and the wavelength in fm.

use crate::bytes::{i32_at, str_at};
use crate::geometry::Geometry;
use crate::{Coding, Error, Frame, Image, Kind, Result, Sample, bad, positive};

/// Bytes [`sniff`] needs.
pub const SNIFF_LEN: usize = 1032;
const DATA: usize = 4096;

/// Whether the file is a MarCCD TIFF (TIFF magic, `MMX` at 1028).
pub fn sniff(head: &[u8]) -> bool {
    (head.starts_with(b"II*\0") || head.starts_with(b"MM\0*"))
        && head.get(1028..1031) == Some(b"MMX")
}

pub(crate) fn parse(data: &[u8]) -> Result<Image> {
    if data.len() < DATA {
        return Err(Error::Truncated);
    }
    let big = match i32_at(data, 1052, false) {
        Some(1234) => false,
        _ if i32_at(data, 1052, true) == Some(1234) => true,
        _ => data.starts_with(b"MM"),
    };
    let int = |at: usize| i32_at(data, at, big).unwrap_or(0);
    let data_big = match int(1056) {
        4321 => true,
        1234 => false,
        _ => big,
    };
    let w = u32::try_from(int(1104)).map_err(|_| bad("nfast"))?;
    let h = u32::try_from(int(1108)).map_err(|_| bad("nslow"))?;
    let sample = match int(1112) {
        1 => Sample::U8,
        2 => Sample::U16,
        4 => Sample::U32,
        d => return Err(Error::Unsupported(format!("MarCCD depth {d}"))),
    };
    let milli = |at: usize| f64::from(int(at)) / 1000.0;
    let mut g = Geometry {
        distance: positive(Some(milli(1664))).or(positive(Some(milli(1720)))),
        ..Geometry::default()
    };
    let (bx, by) = (milli(1668), milli(1672));
    if bx > 0.0 || by > 0.0 {
        g.beam = Some((bx, by));
    }
    let (px, py) = (f64::from(int(1796)), f64::from(int(1800)));
    if px > 0.0 && py > 0.0 {
        g.pixel = Some((px / 1e6, py / 1e6));
    }
    g.wavelength = positive(Some(f64::from(int(1932)) * 1e-5));
    g.exposure = positive(Some(f64::from(int(1680)) / 1000.0));
    let angles = [
        milli(1692),
        milli(1696),
        milli(1700),
        milli(1704),
        milli(1708),
    ];
    g.two_theta = Some(angles[0]);
    g.omega = Some(angles[1]);
    g.chi = Some(angles[2]);
    g.kappa = Some(angles[3]);
    g.phi = Some(angles[4]);
    let range = milli(1760);
    let axis = int(1756);
    let names = ["2θ", "ω", "χ", "κ", "φ"];
    if let Some(&start) = usize::try_from(axis).ok().and_then(|a| angles.get(a))
        && range != 0.0
    {
        g.osc_start = Some(start);
        g.osc_width = Some(range);
        g.osc_axis = Some(names[axis as usize].to_owned());
    }
    let sat = int(1128);
    if sat > 0 {
        g.overload = Some(f64::from(sat));
    }
    let title = str_at(data, 2048, 128);
    // "MMDDhhmmYYYY.SS".
    let date = str_at(data, 2368, 32).map(|t| {
        let b = t.as_bytes();
        if b.len() >= 15 && b[..12].iter().all(u8::is_ascii_digit) {
            format!(
                "{}-{}-{} {}:{}:{}",
                &t[8..12],
                &t[0..2],
                &t[2..4],
                &t[4..6],
                &t[6..8],
                &t[13..15]
            )
        } else {
            t
        }
    });
    let mut header: Vec<(String, String)> = Vec::new();
    for (k, at, len) in [
        ("filetitle", 2048, 128),
        ("filepath", 2176, 128),
        ("filename", 2304, 64),
        ("acquire_timestamp", 2368, 32),
        ("header_timestamp", 2400, 32),
        ("save_timestamp", 2432, 32),
        ("file_comments", 2464, 512),
        ("dataset_comments", 3072, 512),
    ] {
        if let Some(v) = str_at(data, at, len) {
            header.push((k.to_owned(), v));
        }
    }
    for (k, at) in [
        ("detector_type", 1792),
        ("saturated_value", 1128),
        ("data_type", 1124),
    ] {
        header.push((k.to_owned(), int(at).to_string()));
    }
    Ok(Image {
        kind: Kind::MarCcd,
        width: w,
        height: h,
        sample,
        frames: vec![Frame {
            coding: Coding::Raw {
                offset: DATA,
                sample,
                big: data_big,
            },
            header: Vec::new(),
        }],
        header,
        geometry: g,
        detector: Some("MarCCD".to_owned()),
        date,
        mask_negative: false,
        dummy: None,
        title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_header() {
        let mut d = vec![0u8; 4096];
        d[..4].copy_from_slice(b"II*\0");
        d[1028..1031].copy_from_slice(b"MMX");
        let mut put = |at: usize, v: i32| d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(1052, 1234);
        put(1056, 1234);
        put(1104, 3);
        put(1108, 2);
        put(1112, 2);
        put(1128, 65535);
        put(1664, 150_000);
        put(1668, 1_500);
        put(1672, 1_000);
        put(1708, 10_000);
        put(1756, 4);
        put(1760, 500);
        put(1796, 79_092);
        put(1800, 79_092);
        put(1932, 103_318);
        d[2368..2383].copy_from_slice(b"101112132024.05");
        for v in [1u16, 2, 3, 4, 5, 6] {
            d.extend(v.to_le_bytes());
        }
        assert!(sniff(&d));
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.kind, Kind::MarCcd);
        assert_eq!((im.width, im.height), (3, 2));
        let g = &im.geometry;
        assert_eq!(g.distance, Some(150.0));
        assert_eq!(g.beam, Some((1.5, 1.0)));
        assert!((g.wavelength.unwrap() - 1.03318).abs() < 1e-12);
        assert!((g.pixel.unwrap().0 - 0.079092).abs() < 1e-12);
        assert_eq!(g.osc_start, Some(10.0));
        assert_eq!(g.osc_width, Some(0.5));
        assert_eq!(im.date.as_deref(), Some("2024-10-11 12:13:05"));
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }
}

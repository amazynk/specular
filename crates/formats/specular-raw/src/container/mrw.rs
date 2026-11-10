//! Minolta / Konica Minolta MRW: a chain of `\0XYZ` blocks (big-endian lengths) before the
//! sensor data. `PRD` holds the sensor size and sample packing, `WBG` the as-shot white
//! balance, `TTW` a complete TIFF (Exif, MakerNote with the previews).

use super::{Black, Codec, Packing, Previews, Sensor, Storage, Walk, fill_from_tiff_block};
use crate::cfa::Cfa;
use crate::error::{RawError, Result};
use crate::tiff::{self, be_u16, be_u32};
use crate::{RawFormat, RawInfo, SensorCompression};
use std::ops::Range;

pub(super) fn is_mrw(d: &[u8]) -> bool {
    d.starts_with(b"\0MRM")
}

/// Block name and data range of each header block.
fn blocks(d: &[u8], end: usize) -> Vec<([u8; 4], Range<usize>)> {
    let mut out = Vec::new();
    let mut p = 8usize;
    while p + 8 <= end && out.len() < 64 {
        let Some(len) = be_u32(d, p + 4) else { break };
        let start = p + 8;
        let stop = start.saturating_add(len as usize).min(end);
        out.push(([d[p], d[p + 1], d[p + 2], d[p + 3]], start..stop));
        p = start.saturating_add(len as usize);
    }
    out
}

pub(super) fn parse(d: &[u8]) -> Result<(RawInfo, Option<Sensor>)> {
    let data = be_u32(d, 4)
        .map(|l| 8usize.saturating_add(l as usize))
        .ok_or(RawError::Malformed("MRW header"))?;
    let end = data.min(d.len());
    let mut info = RawInfo {
        format: Some(RawFormat::Mrw),
        ..Default::default()
    };
    let mut pv = Previews::default();
    let mut prd = None;
    let mut wbg = None;
    for (name, r) in blocks(d, end) {
        match &name {
            b"\0PRD" if r.len() >= 24 => prd = Some(r.start),
            b"\0WBG" if r.len() >= 12 => wbg = Some(r.start),
            b"\0TTW" => {
                fill_from_tiff_block(&mut info, d, r.clone());
                tiff_previews(&mut pv, d, r.clone());
                info.tiff_blocks.push(r);
            }
            _ => {}
        }
    }
    if info.make.is_empty() {
        info.make = "Minolta".into();
    }
    if info.orientation == 0 {
        info.orientation = 1;
    }
    info.previews = pv.finish(info.orientation);
    let sensor = prd.map(|p| sensor(d, p, wbg, data, &info.model));
    info.sensor = sensor.as_ref().map(Sensor::info);
    Ok((info, sensor))
}

/// Previews of the embedded TIFF: IFD JPEGs and the Minolta MakerNote thumbnail (0x0081) and
/// preview (0x0088 offset from the TIFF start, 0x0089 length).
pub(super) fn tiff_previews(pv: &mut Previews, d: &[u8], block: Range<usize>) {
    let Some(b) = d.get(block.clone()) else {
        return;
    };
    let Some((le, first)) = tiff::header(b, 0) else {
        return;
    };
    let w = Walk::new(b, le, first);
    let mut local = Previews::default();
    for ifd in &w.main {
        local.add_ifd(b, ifd, "IFD chain");
    }
    if let Some(mn) = &w.maker {
        if let (Some(o), Some(l)) = (mn.uint(b, 0x0088), mn.uint(b, 0x0089)) {
            let s = o as usize;
            local.jpeg(b, s..s.saturating_add(l as usize), "MakerNote PreviewImage");
        }
        if let Some(e) = mn.get(0x0081) {
            local.jpeg(b, e.pos..e.pos + e.byte_len(), "MakerNote ThumbnailImage");
        }
    }
    for mut p in local.list {
        p.range = p.range.start + block.start..p.range.end + block.start;
        if !pv.seen(p.range.start) {
            pv.list.push(p);
        }
    }
}

fn sensor(d: &[u8], prd: usize, wbg: Option<usize>, data: usize, model: &str) -> Sensor {
    let h = be_u16(d, prd + 8).unwrap_or(0) as u32;
    let w = be_u16(d, prd + 10).unwrap_or(0) as u32;
    let stored = d.get(prd + 16).copied().unwrap_or(0);
    let bits = d.get(prd + 17).copied().unwrap_or(12) as u32;
    let method = d.get(prd + 18).copied().unwrap_or(0);
    let pattern = be_u16(d, prd + 22).unwrap_or(1);
    let mut s = Sensor::new(w, h, bits.clamp(8, 16), SensorCompression::Uncompressed);
    s.cfa = Cfa::bayer(if pattern == 4 { "GBRG" } else { "RGGB" });
    s.black = Black::Levels([0.0; 4]);
    if let Some(p) = wbg {
        // Four coefficients after four scale bytes: R, G, G, B (G, B, R, G on the A200).
        let c: Vec<f32> = (0..4)
            .map(|i| be_u16(d, p + 4 + 2 * i).unwrap_or(0) as f32)
            .collect();
        let (r, g, b) = if model.contains("A200") {
            (c[2], c[0], c[1])
        } else {
            (c[0], c[1], c[3])
        };
        if r > 0.0 && g > 0.0 && b > 0.0 {
            s.wb = Some([r / g, 1.0, b / g]);
        }
    }
    let packing = match (method, stored) {
        (0x59, _) => Packing::Msb(bits),
        (0x52, 16) => Packing::U16Be,
        (0x52, 8) => Packing::U8,
        _ => return s.unsupported("MRW sample packing"),
    };
    if w == 0 || h == 0 || w > 65535 || h > 65535 {
        return s.unsupported("MRW sensor size");
    }
    let row = match packing {
        Packing::Msb(b) => (w as usize * b as usize).div_ceil(8),
        Packing::U16Be => w as usize * 2,
        _ => w as usize,
    };
    let start = data.min(d.len());
    s.storage = Storage::Blocks {
        blocks: std::iter::once(start..data.saturating_add(row * h as usize)).collect(),
        tile_w: w,
        tile_h: h,
        codec: Codec::Raw {
            packing,
            row_bytes: 0,
        },
    };
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(packed: bool) -> Vec<u8> {
        let (w, h) = (4u16, 2u16);
        let mut prd = b"21810002".to_vec();
        for v in [h, w, h, w] {
            prd.extend(v.to_be_bytes());
        }
        prd.extend([
            if packed { 12 } else { 16 },
            12,
            if packed { 0x59 } else { 0x52 },
            0,
        ]);
        prd.extend([0, 0, 0, 1]);
        let mut wbg = vec![2, 2, 2, 2];
        for v in [512u16, 256, 256, 384] {
            wbg.extend(v.to_be_bytes());
        }
        let mut hdr = Vec::new();
        for (n, b) in [(b"\0PRD", &prd), (b"\0WBG", &wbg)] {
            hdr.extend(n);
            hdr.extend((b.len() as u32).to_be_bytes());
            hdr.extend(b.iter());
        }
        let mut f = b"\0MRM".to_vec();
        f.extend((hdr.len() as u32).to_be_bytes());
        f.extend(hdr);
        let samples: Vec<u16> = (0..8).map(|i| 100 * i + 7).collect();
        if packed {
            for p in samples.chunks(2) {
                f.extend([
                    (p[0] >> 4) as u8,
                    ((p[0] & 15) << 4 | p[1] >> 8) as u8,
                    p[1] as u8,
                ]);
            }
        } else {
            for v in samples {
                f.extend(v.to_be_bytes());
            }
        }
        f
    }

    #[test]
    fn packed_and_unpacked() {
        for packed in [true, false] {
            let f = file(packed);
            assert!(is_mrw(&f));
            let raw = crate::parse(&f).unwrap();
            let info = raw.info();
            assert_eq!(info.format, Some(RawFormat::Mrw));
            let img = raw.decode_raw(&f, &Default::default()).unwrap();
            assert_eq!((img.width, img.height), (4, 2));
            assert_eq!(img.data, (0..8).map(|i| 100 * i + 7).collect::<Vec<u16>>());
            assert_eq!(img.wb, Some([2.0, 1.0, 1.5]));
            assert!(!img.truncated);
        }
    }

    #[test]
    fn truncated_and_hostile() {
        let f = file(true);
        for n in [4, 8, 20, 40, f.len() - 3] {
            let _ = crate::parse(&f[..n]).map(|r| r.decode_raw(&f[..n], &Default::default()));
        }
        let mut big = f.clone();
        big[24..28].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        let _ = crate::parse(&big).map(|r| r.decode_raw(&big, &Default::default()));
    }
}

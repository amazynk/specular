//! Binary Visio (`.vsd`, `.vss`, `.vst`): the preview in the compound file's summary
//! information (`PIDSI_THUMBNAIL`, clipboard data).

use specular_cfb::Cfb;

use crate::{Error, Preview, PreviewKind};

const PIDSI_THUMBNAIL: u32 = 0x11;
const VT_CF: u32 = 0x47;
const CF_METAFILEPICT: u32 = 3;
const CF_DIB: u32 = 8;
const CF_ENHMETAFILE: u32 = 14;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

/// The clipboard data of `PIDSI_THUMBNAIL` in a property set stream: (format, bytes).
pub(crate) fn thumbnail_property(ps: &[u8]) -> Option<(u32, &[u8])> {
    if u16_at(ps, 0)? != 0xFFFE {
        return None;
    }
    let sections = u32_at(ps, 24)?.min(16) as usize;
    for i in 0..sections {
        let off = u32_at(ps, 28 + i * 20 + 16)? as usize;
        let sec = ps.get(off..)?;
        let count = u32_at(sec, 4)?.min(4096) as usize;
        for j in 0..count {
            let id = u32_at(sec, 8 + j * 8)?;
            let at = u32_at(sec, 12 + j * 8)? as usize;
            if id != PIDSI_THUMBNAIL {
                continue;
            }
            let p = sec.get(at..)?;
            if u32_at(p, 0)? & 0xFFFF != VT_CF {
                return None;
            }
            let size = u32_at(p, 4)? as usize;
            let tag = u32_at(p, 8)? as i32;
            if tag != -1 || size < 8 {
                return None;
            }
            let cf = u32_at(p, 12)?;
            let data = p.get(16..(8 + size).min(p.len()))?;
            return Some((cf, data));
        }
    }
    None
}

/// A standalone WMF from `CF_METAFILEPICT` data: the METAFILEPICT header dropped and an
/// Aldus placeable header added (its bounds from the window origin and extent records).
pub(crate) fn placeable_wmf(data: &[u8]) -> Option<Vec<u8>> {
    let is_header =
        |o: usize| matches!(u16_at(data, o), Some(1 | 2)) && u16_at(data, o + 2) == Some(9);
    let (start, ext) = if is_header(8) {
        (8, (u16_at(data, 2)? as i16, u16_at(data, 4)? as i16))
    } else if is_header(16) {
        (16, (u32_at(data, 4)? as i16, u32_at(data, 8)? as i16))
    } else if is_header(0) {
        (0, (0, 0))
    } else {
        return None;
    };
    let wmf = &data[start..];
    // Window origin and extent.
    let (mut org, mut wext) = ((0i16, 0i16), None);
    let mut o = 18;
    for _ in 0..100_000 {
        let Some(size) = u32_at(wmf, o) else { break };
        let Some(func) = u16_at(wmf, o + 4) else {
            break;
        };
        if size < 3 || func == 0 {
            break;
        }
        match func {
            0x020B => org = (u16_at(wmf, o + 8)? as i16, u16_at(wmf, o + 6)? as i16),
            0x020C => wext = Some((u16_at(wmf, o + 8)? as i16, u16_at(wmf, o + 6)? as i16)),
            _ => {}
        }
        o += size as usize * 2;
    }
    let (wx, wy) = wext.unwrap_or((1000, 1000));
    let (l, t) = org;
    let r = l.saturating_add(wx);
    let b = t.saturating_add(wy);
    // Logical units per inch from the HIMETRIC picture size when known.
    let inch = if ext.0 > 0 {
        ((f64::from(wx).abs() * 2540.0 / f64::from(ext.0)).round() as u32).clamp(1, 65535) as u16
    } else {
        1440
    };
    let mut out = Vec::with_capacity(22 + wmf.len());
    out.extend_from_slice(&0x9AC6_CDD7u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    for v in [l.min(r), t.min(b), l.max(r), t.max(b)] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&inch.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    let mut sum = 0u16;
    for c in out.chunks_exact(2) {
        sum ^= u16::from_le_bytes([c[0], c[1]]);
    }
    out.extend_from_slice(&sum.to_le_bytes());
    out.extend_from_slice(wmf);
    Some(out)
}

pub(crate) fn preview(data: &[u8]) -> Result<Preview, Error> {
    let cfb = Cfb::open(data).map_err(|_| Error::NotVisio)?;
    if cfb.exists("EncryptedPackage") || cfb.exists("EncryptionInfo") {
        return Err(Error::Unsupported(
            "password-protected Visio drawing: opening it needs the password, which is not supported"
                .into(),
        ));
    }
    if !cfb.exists("VisioDocument") {
        return Err(Error::NotVisio);
    }
    let none = || {
        Error::Unsupported(
            "binary Visio drawing without an embedded preview (only VSDX pages are drawn)".into(),
        )
    };
    let ps = cfb.read("\u{5}SummaryInformation").map_err(|_| none())?;
    let (cf, bytes) = thumbnail_property(&ps).ok_or_else(none)?;
    let (kind, bytes) = match cf {
        CF_METAFILEPICT => (PreviewKind::Wmf, placeable_wmf(bytes).ok_or_else(none)?),
        CF_ENHMETAFILE => (PreviewKind::Emf, bytes.to_vec()),
        CF_DIB => (PreviewKind::Dib, bytes.to_vec()),
        _ => {
            if bytes.starts_with(b"\x89PNG") {
                (PreviewKind::Png, bytes.to_vec())
            } else if bytes.starts_with(&[0xFF, 0xD8]) {
                (PreviewKind::Jpeg, bytes.to_vec())
            } else {
                return Err(none());
            }
        }
    };
    if bytes.is_empty() {
        return Err(none());
    }
    Ok(Preview { kind, bytes })
}

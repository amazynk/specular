//! JPEG XS in HEIF (ISO/IEC 21122-3, `image/jxsi`): the primary item is a `jxs1` coded image
//! item whose data is one JPEG XS codestream. The container is read with the in-tree HEIF
//! parser (`specular-heif`); `ispe`, `colr` (nclx), `irot`, `imir` and `clap` are honoured and
//! a `jxpl` profile and level property is reported. Grids of `jxs1` tiles and other derived
//! primary items are refused in words.

use crate::file::{Cicp, FileInfo};
use crate::{Error, Result};
use heif::{Colr, HeifFile, ItemProperties, Property};
use std::borrow::Cow;

/// The HEIF item type of a JPEG XS coded image.
pub const ITEM_TYPE: [u8; 4] = *b"jxs1";

/// A crop of the decoded picture (`clap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

fn be32(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn be16(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

/// Child boxes of `d` as (type, body); stops at the first box that does not fit (a probe
/// sees only the head of the file).
fn children(d: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let size = be32(d, at)? as usize;
        let kind: [u8; 4] = d.get(at + 4..at + 8)?.try_into().ok()?;
        let (hdr, size) = match size {
            0 => (8, d.len() - at),
            1 => {
                let big = d.get(at + 8..at + 16)?;
                let s = u64::from_be_bytes(big.try_into().ok()?);
                (16, usize::try_from(s).ok()?)
            }
            s => (8, s),
        };
        if size < hdr {
            return None;
        }
        let body = &d[at + hdr..(at + size).min(d.len())];
        at = at.checked_add(size)?;
        Some((kind, body))
    })
}

/// Whether `head` is a HEIF file whose primary item is a JPEG XS image (looked up in the
/// `meta` box, which has to lie within `head`).
pub fn is_heif(head: &[u8]) -> bool {
    if head.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let mut boxes = children(head);
    let Some((_, ftyp)) = boxes.next() else {
        return false;
    };
    let brands = ftyp
        .get(..4)
        .into_iter()
        .chain(ftyp.get(8..).unwrap_or_default().chunks_exact(4));
    if !brands
        .into_iter()
        .any(|b| matches!(b, b"mif1" | b"mif2" | b"msf1" | b"miaf"))
    {
        return false;
    }
    let Some((_, meta)) = boxes.find(|(k, _)| k == b"meta") else {
        return false;
    };
    let Some(meta) = meta.get(4..) else {
        return false;
    };
    let mut primary = None;
    let mut types: Vec<(u32, [u8; 4])> = Vec::new();
    for (kind, body) in children(meta) {
        match &kind {
            b"pitm" => {
                primary = match body.first() {
                    Some(0) => be16(body, 4).map(u32::from),
                    Some(_) => be32(body, 4),
                    None => None,
                }
            }
            b"iinf" => {
                let skip = if body.first() == Some(&0) { 6 } else { 8 };
                for (k, infe) in children(body.get(skip..).unwrap_or_default()).take(4096) {
                    if &k != b"infe" {
                        continue;
                    }
                    let entry = match infe.first() {
                        Some(2) => be16(infe, 4)
                            .map(u32::from)
                            .zip(infe.get(8..12).and_then(|t| t.try_into().ok())),
                        Some(3) => {
                            be32(infe, 4).zip(infe.get(10..14).and_then(|t| t.try_into().ok()))
                        }
                        _ => None,
                    };
                    types.extend(entry);
                }
            }
            _ => {}
        }
    }
    primary.is_some_and(|p| types.iter().any(|&(id, t)| id == p && t == ITEM_TYPE))
}

fn heif_err(e: heif::HeifError) -> Error {
    Error::Malformed(format!("HEIF container: {e}"))
}

/// The codestream of the primary `jxs1` item, what the container says about it, and the
/// crop its `clap` asks for.
pub fn parse(data: &[u8]) -> Result<(Cow<'_, [u8]>, FileInfo, Option<Crop>)> {
    let file = HeifFile::parse_borrowed(data).map_err(heif_err)?;
    let item = file.primary_item().map_err(heif_err)?.clone();
    let id = item.id;
    if item.item_type != ITEM_TYPE {
        let t = String::from_utf8_lossy(&item.item_type).trim().to_owned();
        return Err(
            if t == "grid" || t == "iovl" || t == "iden" || t == "tili" {
                Error::Unsupported(format!(
                    "HEIF images assembled from JPEG XS tiles ('{t}') are not supported yet"
                ))
            } else {
                Error::NotRecognised
            },
        );
    }
    if item.protection_index != 0 {
        return Err(Error::Unsupported(
            "the JPEG XS image in this HEIF file is encrypted".into(),
        ));
    }
    let meta = file.meta().map_err(heif_err)?;
    let props = ItemProperties::resolve(meta, id).map_err(heif_err)?;
    // The JPEG XS configuration properties are understood: the codestream describes itself.
    let unknown: Vec<String> = props
        .unsupported_essential()
        .iter()
        .filter(|t| !t.starts_with(b"jx"))
        .map(|t| String::from_utf8_lossy(t).into_owned())
        .collect();
    if !unknown.is_empty() {
        return Err(Error::Unsupported(format!(
            "the JPEG XS item needs HEIF properties this viewer does not know ({})",
            unknown.join(", ")
        )));
    }
    let mut info = FileInfo {
        brand: data
            .get(8..12)
            .and_then(|b| b.try_into().ok())
            .unwrap_or(*b"mif1"),
        ..FileInfo::default()
    };
    if let Some(Colr::Nclx {
        primaries,
        transfer,
        matrix,
        full_range,
    }) = props.nclx()
    {
        info.cicp = Some(Cicp {
            primaries: *primaries,
            transfer: *transfer,
            matrix: *matrix,
            full_range: *full_range,
        });
    }
    for (_, raw, _) in meta.properties_of(id) {
        if &raw.box_type == b"jxpl" && raw.body.len() >= 4 {
            // A full box or a plain one: the last four bytes are Ppih and Plev.
            let b = &raw.body[raw.body.len() - 4..];
            info.profile_level = Some((
                u16::from_be_bytes([b[0], b[1]]),
                u16::from_be_bytes([b[2], b[3]]),
            ));
        }
    }
    info.orientation = orientation(props.transformative().map(|e| &e.property));
    // Borrowed when the item is one span of the file; copied when split into extents or kept
    // in `idat`.
    let cs: Cow<'_, [u8]> = match file.item_file_spans(id).ok().as_deref() {
        Some(&[(a, b)]) if a <= b && b <= data.len() => Cow::Borrowed(&data[a..b]),
        _ => Cow::Owned(file.item_data_owned(id).map_err(heif_err)?),
    };
    if !cs.starts_with(&[0xFF, 0x10]) {
        return Err(Error::Malformed(
            "the JPEG XS item does not hold a codestream".into(),
        ));
    }
    info.codestream = (0, cs.len());
    let crop = props.clap().and_then(|c| {
        let ispe = props.ispe()?;
        let r = c.resolve(ispe.width, ispe.height).ok()?;
        let full = r.x == 0 && r.y == 0 && r.width == ispe.width && r.height == ispe.height;
        (!full && r.width > 0 && r.height > 0).then_some(Crop {
            x: r.x,
            y: r.y,
            w: r.width,
            h: r.height,
        })
    });
    Ok((cs, info, crop))
}

/// Composes `irot` and `imir` (in association order) into an EXIF orientation (1..=8).
fn orientation<'a>(props: impl Iterator<Item = &'a Property>) -> u8 {
    // The shown image is mirror^f of rotate-clockwise^r of the stored one.
    let (mut r, mut f) = (0u8, false);
    for p in props {
        match p {
            Property::Irot(a) => {
                // irot turns anti-clockwise.
                let k = (4 - (a.angle & 3)) & 3;
                r = if f { (r + 4 - k) & 3 } else { (r + k) & 3 };
            }
            // Axis 1 mirrors left and right; axis 0 top and bottom (a left-right mirror
            // after a half turn).
            Property::Imir(m) => {
                f = !f;
                if m.axis == 0 {
                    r = (r + 2) & 3;
                }
            }
            _ => {}
        }
    }
    match (r, f) {
        (0, false) => 1,
        (0, true) => 2,
        (2, false) => 3,
        (2, true) => 4,
        (1, true) => 5,
        (1, false) => 6,
        (3, true) => 7,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heif::{Imir, Irot};

    #[test]
    fn orientations() {
        let o = |v: &[Property]| orientation(v.iter());
        assert_eq!(o(&[]), 1);
        assert_eq!(o(&[Property::Irot(Irot::new(1))]), 8);
        assert_eq!(o(&[Property::Irot(Irot::new(2))]), 3);
        assert_eq!(o(&[Property::Irot(Irot::new(3))]), 6);
        assert_eq!(o(&[Property::Imir(Imir::new(1))]), 2);
        assert_eq!(o(&[Property::Imir(Imir::new(0))]), 4);
        assert_eq!(
            o(&[Property::Irot(Irot::new(3)), Property::Imir(Imir::new(1))]),
            5
        );
        assert_eq!(
            o(&[Property::Irot(Irot::new(1)), Property::Imir(Imir::new(1))]),
            7
        );
    }
}

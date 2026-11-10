//! The JPEG XS image file (ISO/IEC 21122-3 Annex A): a box file opened by the signature box,
//! the `jxs ` brand, a header superbox (image header, colour specification) and the
//! contiguous codestream box; optionally a video support superbox with the profile and level
//! (`jxpl`) and video information (`jpvi`) boxes.

use crate::{Error, Result};

/// The 12-byte signature box that opens every JPEG XS file.
pub const SIGNATURE: [u8; 12] = [0, 0, 0, 12, b'J', b'X', b'S', b' ', 0x0D, 0x0A, 0x87, 0x0A];

/// Colour description of a CICP colour specification box (Rec. ITU-T H.273 code points).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cicp {
    pub primaries: u16,
    pub transfer: u16,
    pub matrix: u16,
    pub full_range: bool,
}

/// What a JPEG XS file says around its codestream.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileInfo {
    pub brand: [u8; 4],
    pub cicp: Option<Cicp>,
    /// Profile and level from the `jxpl` box.
    pub profile_level: Option<(u16, u16)>,
    /// Maximum bit rate (Mbit/s) and the frame rate field of the `jpvi` box.
    pub video: Option<(u32, u32)>,
    /// Byte range of the codestream (of the item data, for HEIF).
    pub codestream: (usize, usize),
    /// EXIF orientation from HEIF `irot`/`imir` (0: none given).
    pub orientation: u8,
}

struct Box<'a> {
    kind: [u8; 4],
    body: &'a [u8],
    /// Offset of `body` in the buffer the boxes were read from.
    at: usize,
}

fn boxes(d: &[u8], base: usize) -> impl Iterator<Item = Result<Box<'_>>> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        if at >= d.len() {
            return None;
        }
        let rest = &d[at..];
        if rest.len() < 8 {
            at = d.len();
            return Some(Err(Error::Truncated(
                "the file ends inside a box header".into(),
            )));
        }
        let len = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as u64;
        let kind = [rest[4], rest[5], rest[6], rest[7]];
        let (hdr, len) = match len {
            0 => (8u64, rest.len() as u64),
            1 => {
                let Some(x) = rest.get(8..16) else {
                    at = d.len();
                    return Some(Err(Error::Truncated(
                        "the file ends inside a box header".into(),
                    )));
                };
                let mut b = [0u8; 8];
                b.copy_from_slice(x);
                (16, u64::from_be_bytes(b))
            }
            2..=7 => {
                at = d.len();
                return Some(Err(Error::Malformed(format!("a box of length {len}"))));
            }
            n => (8, n),
        };
        if len < hdr {
            at = d.len();
            return Some(Err(Error::Malformed(
                "a box shorter than its header".into(),
            )));
        }
        let end = (len as usize).min(rest.len());
        let truncated = (len as usize) > rest.len();
        let b = Box {
            kind,
            body: &rest[hdr as usize..end],
            at: base + at + hdr as usize,
        };
        at = if truncated {
            d.len()
        } else {
            at + len as usize
        };
        Some(Ok(b))
    })
}

/// Whether `head` starts a JPEG XS file.
pub fn is_file(head: &[u8]) -> bool {
    head.starts_with(&SIGNATURE)
}

/// Finds the codestream and the colour facts of a JPEG XS file.
pub fn parse(d: &[u8]) -> Result<FileInfo> {
    if !is_file(d) {
        return Err(Error::NotRecognised);
    }
    let mut info = FileInfo::default();
    let mut found = false;
    let mut have_ftyp = false;
    for b in boxes(d, 0).skip(1) {
        let b = b?;
        match &b.kind {
            b"ftyp" => {
                have_ftyp = true;
                if b.body.len() >= 4 {
                    info.brand.copy_from_slice(&b.body[..4]);
                }
                let compatible = b.body.len() >= 8
                    && (b.body[..4] == *b"jxs "
                        || b.body[8..].chunks_exact(4).any(|c| c == b"jxs "));
                if !compatible {
                    return Err(Error::Unsupported(
                        "this JPEG XS file is not marked as readable by plain JPEG XS readers (no 'jxs ' brand)".into(),
                    ));
                }
            }
            b"jxsh" | b"jp2h" => {
                for c in boxes(b.body, b.at) {
                    let c = c?;
                    if &c.kind == b"colr" && c.body.len() >= 10 && c.body[0] == 5 {
                        let u = |i: usize| u16::from_be_bytes([c.body[i], c.body[i + 1]]);
                        info.cicp.get_or_insert(Cicp {
                            primaries: u(3),
                            transfer: u(5),
                            matrix: u(7),
                            full_range: c.body[9] & 0x80 != 0,
                        });
                    }
                }
            }
            b"jpvs" => {
                for c in boxes(b.body, b.at) {
                    let c = c?;
                    match &c.kind {
                        b"jxpl" if c.body.len() >= 4 => {
                            info.profile_level = Some((
                                u16::from_be_bytes([c.body[0], c.body[1]]),
                                u16::from_be_bytes([c.body[2], c.body[3]]),
                            ));
                        }
                        b"jpvi" if c.body.len() >= 8 => {
                            let u = |i: usize| {
                                u32::from_be_bytes([
                                    c.body[i],
                                    c.body[i + 1],
                                    c.body[i + 2],
                                    c.body[i + 3],
                                ])
                            };
                            info.video = Some((u(0), u(4)));
                        }
                        _ => {}
                    }
                }
            }
            b"jp2c" if !found => {
                found = true;
                info.codestream = (b.at, b.at + b.body.len());
            }
            _ => {}
        }
    }
    if !have_ftyp {
        return Err(Error::Malformed(
            "a JPEG XS file without its file type box".into(),
        ));
    }
    if !found {
        return Err(Error::Truncated(
            "the JPEG XS file has no codestream".into(),
        ));
    }
    Ok(info)
}

/// The frame rate of a `jpvi` frame rate field, in frames per second.
pub fn frame_rate(frat: u32) -> Option<f64> {
    let num = frat & 0xFFFF;
    if num == 0 {
        return None;
    }
    match (frat >> 24) & 0x3F {
        1 => Some(num as f64),
        2 => Some(num as f64 * 1000.0 / 1001.0),
        _ => None,
    }
}

/// Wraps a codestream in a minimal JPEG XS file (for tests and fixtures).
pub fn wrap(codestream: &[u8], cicp: Cicp, profile_level: Option<(u16, u16)>) -> Vec<u8> {
    fn put(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
    }
    let mut f = SIGNATURE.to_vec();
    put(&mut f, b"ftyp", b"jxs \0\0\0\0jxs ");
    let mut h = Vec::new();
    // Image header: height, width, components, bit depth - 1, compression type 12.
    let mut ihdr = [0u8; 14];
    if let Ok(hd) = crate::header::parse(codestream) {
        ihdr[0..4].copy_from_slice(&hd.height.to_be_bytes());
        ihdr[4..8].copy_from_slice(&hd.width.to_be_bytes());
        ihdr[8..10].copy_from_slice(&(hd.components.len() as u16).to_be_bytes());
        ihdr[10] = hd.depth().saturating_sub(1);
    }
    ihdr[11] = 12;
    put(&mut h, b"ihdr", &ihdr);
    let mut colr = vec![5, 0, 0];
    colr.extend_from_slice(&cicp.primaries.to_be_bytes());
    colr.extend_from_slice(&cicp.transfer.to_be_bytes());
    colr.extend_from_slice(&cicp.matrix.to_be_bytes());
    colr.push(if cicp.full_range { 0x80 } else { 0 });
    put(&mut h, b"colr", &colr);
    put(&mut f, b"jp2h", &h);
    if let Some((p, l)) = profile_level {
        let mut v = Vec::new();
        let mut jpvi = Vec::new();
        jpvi.extend_from_slice(&1000u32.to_be_bytes());
        jpvi.extend_from_slice(&(0x0100_0000u32 | 25).to_be_bytes());
        jpvi.extend_from_slice(&[0; 6]);
        put(&mut v, b"jpvi", &jpvi);
        let mut jxpl = p.to_be_bytes().to_vec();
        jxpl.extend_from_slice(&l.to_be_bytes());
        put(&mut v, b"jxpl", &jxpl);
        put(&mut f, b"jpvs", &v);
    }
    put(&mut f, b"jp2c", codestream);
    f
}

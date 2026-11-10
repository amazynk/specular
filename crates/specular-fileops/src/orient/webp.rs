//! WebP: Orientation in the `EXIF` chunk, which needs the extended (`VP8X`) layout with its
//! Exif flag; a simple file gets a `VP8X` header made from its frame's size.

use super::tiff;

struct Chunk {
    ty: [u8; 4],
    /// Payload range.
    data: std::ops::Range<usize>,
}

fn chunks(d: &[u8]) -> Option<Vec<Chunk>> {
    if d.get(..4)? != b"RIFF" || d.get(8..12)? != b"WEBP" {
        return None;
    }
    let riff_end = (u32::from_le_bytes(d[4..8].try_into().ok()?) as usize + 8).min(d.len());
    let mut out = Vec::new();
    let mut i = 12;
    while i + 8 <= riff_end {
        let ty: [u8; 4] = d[i..i + 4].try_into().ok()?;
        let len = u32::from_le_bytes(d[i + 4..i + 8].try_into().ok()?) as usize;
        let data = i + 8..(i + 8).checked_add(len)?;
        if data.end > riff_end {
            return None;
        }
        i = data.end + (len & 1);
        out.push(Chunk { ty, data });
    }
    Some(out)
}

/// The Exif block of an EXIF chunk: some writers keep JPEG's `Exif\0\0` prefix.
fn exif_tiff(p: &[u8]) -> (&[u8], usize) {
    if p.starts_with(b"Exif\0\0") {
        (&p[6..], 6)
    } else {
        (p, 0)
    }
}

pub fn orientation(d: &[u8]) -> Option<u8> {
    let c = chunks(d)?;
    let e = c.iter().find(|c| &c.ty == b"EXIF")?;
    tiff::orientation(exif_tiff(&d[e.data.clone()]).0)
}

fn put_chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(ty);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

/// Canvas size and alpha of a simple file's frame (`VP8 ` or `VP8L`).
fn frame_facts(d: &[u8], c: &Chunk) -> Option<(u32, u32, bool)> {
    let p = &d[c.data.clone()];
    match &c.ty {
        b"VP8 " => {
            // Frame tag (3 bytes), start code 9d 01 2a, then 14-bit width and height.
            if p.get(3..6)? != [0x9d, 0x01, 0x2a] {
                return None;
            }
            let w = u32::from(u16::from_le_bytes([p[6], p[7]]) & 0x3FFF);
            let h = u32::from(u16::from_le_bytes([*p.get(8)?, *p.get(9)?]) & 0x3FFF);
            Some((w, h, false))
        }
        b"VP8L" => {
            if *p.first()? != 0x2f {
                return None;
            }
            let bits = u32::from_le_bytes(p.get(1..5)?.try_into().ok()?);
            let w = (bits & 0x3FFF) + 1;
            let h = ((bits >> 14) & 0x3FFF) + 1;
            Some((w, h, bits >> 28 & 1 == 1))
        }
        _ => None,
    }
}

/// The file with Orientation `o`.
pub fn set_orientation(d: &[u8], o: u8) -> Option<Vec<u8>> {
    let cs = chunks(d)?;
    let mut exif = match cs.iter().find(|c| &c.ty == b"EXIF") {
        Some(e) => {
            let p = &d[e.data.clone()];
            let (t, skip) = exif_tiff(p);
            let mut t = t.to_vec();
            tiff::set_orientation(&mut t, o)?;
            let mut v = p[..skip].to_vec();
            v.extend_from_slice(&t);
            v
        }
        None => tiff::minimal(o),
    };
    let mut body = Vec::with_capacity(d.len() + exif.len() + 32);
    let has_vp8x = cs.first().is_some_and(|c| &c.ty == b"VP8X");
    if !has_vp8x {
        let first = cs.first()?;
        let (w, h, alpha) = frame_facts(d, first)?;
        let mut x = [0u8; 10];
        x[0] = 0x08 | if alpha { 0x10 } else { 0 };
        x[4..7].copy_from_slice(&(w - 1).to_le_bytes()[..3]);
        x[7..10].copy_from_slice(&(h - 1).to_le_bytes()[..3]);
        put_chunk(&mut body, b"VP8X", &x);
    }
    let mut written = false;
    for c in &cs {
        let p = &d[c.data.clone()];
        match &c.ty {
            b"VP8X" => {
                let mut x = p.to_vec();
                *x.first_mut()? |= 0x08;
                put_chunk(&mut body, b"VP8X", &x);
            }
            b"EXIF" => {
                put_chunk(&mut body, b"EXIF", &std::mem::take(&mut exif));
                written = true;
            }
            // EXIF goes after the image data and before XMP.
            b"XMP " if !written => {
                put_chunk(&mut body, b"EXIF", &std::mem::take(&mut exif));
                written = true;
                put_chunk(&mut body, &c.ty, p);
            }
            ty => put_chunk(&mut body, ty, p),
        }
    }
    if !written {
        put_chunk(&mut body, b"EXIF", &exif);
    }
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32 + 4).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    // Anything after the RIFF (some tools append data) is kept as it was.
    let riff_end = u32::from_le_bytes(d[4..8].try_into().ok()?) as usize + 8;
    if riff_end < d.len() {
        out.extend_from_slice(&d[riff_end..]);
    }
    Some(out)
}

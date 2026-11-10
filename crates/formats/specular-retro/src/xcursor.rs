//! X cursor files (libXcursor's format; cursor themes' `cursors/` folders, no extension).
//!
//! "Xcur", the header size, version and table-of-contents length (little-endian u32s), then
//! (type, subtype, position) entries. Image chunks (type 0xFFFD0002, subtype the nominal
//! size) hold a 36-byte header (size, type, nominal size, version, width, height, hot spot,
//! frame delay) and premultiplied ARGB pixels.
//!
//! Each nominal size is a page, largest first; an animated cursor shows its first frame.

use crate::*;

const MAGIC: &[u8; 4] = b"Xcur";
const IMAGE: u32 = 0xfffd_0002;
/// Table entries read at most.
const MAX_TOC: u32 = 1 << 16;
/// libXcursor's own size limit.
const MAX_SIDE: u32 = 0x7fff;

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(MAGIC) && u32le(head, 4).is_some_and(|h| (16..=1024).contains(&h))
}

/// An image chunk: nominal size, width, height, hot spot, delay, pixel offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub nominal: u32,
    pub width: u32,
    pub height: u32,
    pub hot: (u32, u32),
    pub delay_ms: u32,
    offset: usize,
}

/// The image chunks, in file order.
pub fn chunks(data: &[u8]) -> Result<Vec<Chunk>> {
    if !probe(data) {
        return Err(Error::NotRecognised);
    }
    let ntoc = u32le(data, 12).ok_or_else(|| truncated("header"))?;
    if ntoc > MAX_TOC {
        return Err(Error::Malformed(format!("{ntoc} table entries")));
    }
    let hsize = u32le(data, 4).unwrap_or(16) as usize;
    let mut out = Vec::new();
    for i in 0..ntoc as usize {
        let e = hsize + i * 12;
        let (Some(t), Some(pos)) = (u32le(data, e), u32le(data, e + 8)) else {
            break;
        };
        if t != IMAGE {
            continue;
        }
        let p = pos as usize;
        let (Some(hs), Some(ct), Some(nominal), Some(w), Some(h)) = (
            u32le(data, p),
            u32le(data, p + 4),
            u32le(data, p + 8),
            u32le(data, p + 16),
            u32le(data, p + 20),
        ) else {
            continue;
        };
        if ct != IMAGE || hs < 36 || w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
            continue;
        }
        out.push(Chunk {
            nominal,
            width: w,
            height: h,
            hot: (
                u32le(data, p + 24).unwrap_or(0),
                u32le(data, p + 28).unwrap_or(0),
            ),
            delay_ms: u32le(data, p + 32).unwrap_or(0),
            offset: p + hs as usize,
        });
    }
    if out.is_empty() {
        return Err(Error::Malformed("no cursor images".into()));
    }
    Ok(out)
}

/// The pages: the first image of each nominal size, largest first, with the frame count.
pub fn pages(data: &[u8]) -> Result<Vec<(Chunk, u32)>> {
    let all = chunks(data)?;
    let mut pages: Vec<(Chunk, u32)> = Vec::new();
    for c in all {
        match pages.iter_mut().find(|(p, _)| p.nominal == c.nominal) {
            Some((_, n)) => *n += 1,
            None => pages.push((c, 1)),
        }
    }
    pages.sort_by_key(|(c, _)| std::cmp::Reverse((c.nominal, c.width)));
    Ok(pages)
}

pub fn info(data: &[u8], page: u32) -> Result<Info> {
    let p = pages(data)?;
    let n = p.len() as u32;
    let (c, frames) = *p
        .get(page as usize)
        .ok_or_else(|| Error::Malformed("no such page".into()))?;
    check_dims(c.width, c.height)?;
    let mut coding = format!("{} px cursor, hot spot {},{}", c.nominal, c.hot.0, c.hot.1);
    if frames > 1 {
        coding.push_str(&format!(", {frames} frames (first shown)"));
    }
    let mut i = Info::new(c.width, c.height, PixelFormat::Rgba8, Model::Rgb, 8, coding).alpha(true);
    i.pages = n;
    Ok(i)
}

pub fn decode(data: &[u8], page: u32) -> Result<Image> {
    let p = pages(data)?;
    let (c, _) = *p
        .get(page as usize)
        .ok_or_else(|| Error::Malformed("no such page".into()))?;
    check_dims(c.width, c.height)?;
    let mut img = Image::filled(c.width, c.height, PixelFormat::Rgba8, 0);
    let src = data.get(c.offset..).unwrap_or(&[]);
    let n = c.width as usize * c.height as usize;
    img.truncated = src.len() < n * 4;
    for (o, px) in img.data.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        // Little-endian ARGB: B, G, R, A in memory; premultiplied.
        let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
        let un = |v: u8| {
            if a == 0 {
                0
            } else {
                ((u32::from(v) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
            }
        };
        o.copy_from_slice(&[un(r), un(g), un(b), a]);
    }
    Ok(img)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A cursor file: (nominal, width, height, premultiplied ARGB pixels) per image.
    pub fn build(images: &[(u32, u32, u32, Vec<u32>)]) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(MAGIC);
        for v in [16u32, 0x10000, images.len() as u32] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        let mut pos = 16 + 12 * images.len();
        for (nom, w, h, _) in images {
            for v in [IMAGE, *nom, pos as u32] {
                d.extend_from_slice(&v.to_le_bytes());
            }
            pos += 36 + (*w * *h * 4) as usize;
        }
        for (nom, w, h, px) in images {
            for v in [36u32, IMAGE, *nom, 1, *w, *h, 1, 2, 50] {
                d.extend_from_slice(&v.to_le_bytes());
            }
            for p in px {
                d.extend_from_slice(&p.to_le_bytes());
            }
        }
        d
    }

    #[test]
    fn sizes_as_pages() {
        let small = (24, 2, 1, vec![0xff00_00ff, 0x8080_0000]);
        let big = (32, 2, 2, vec![0xffff_ffff; 4]);
        let f = build(&[small.clone(), big, small]);
        assert!(probe(&f));
        let i = info(&f, 0).unwrap();
        assert_eq!((i.width, i.height, i.pages), (2, 2, 2));
        let i = info(&f, 1).unwrap();
        assert!(i.coding.contains("2 frames"), "{}", i.coding);
        let img = decode(&f, 1).unwrap();
        assert_eq!(&img.data[..4], &[0, 0, 255, 255]);
        // 0x80800000: alpha 128, red 128 premultiplied → 255.
        assert_eq!(&img.data[4..8], &[255, 0, 0, 128]);
        assert!(decode(&f, 2).is_err());
        assert!(decode(&f[..60], 0).is_err() || decode(&f[..60], 0).unwrap().truncated);
    }
}

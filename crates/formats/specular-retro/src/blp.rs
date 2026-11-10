//! Blizzard BLP textures (Warcraft III, World of Warcraft), from the public format notes.
//!
//! BLP1: "BLP1", compression (0 JPEG, 1 palette), alpha bits, width, height, picture type,
//! has-mipmaps, then 16 mip offsets and 16 sizes (little-endian u32s). A palette file has
//! 256 BGRA entries next; the first mip is width × height indices, then (8-bit alpha) as
//! many alpha bytes. A JPEG file has a shared JPEG header (its length, then the bytes) that
//! each mip's data completes; its four channels are blue, green, red and alpha, stored
//! without colour transform.
//!
//! BLP2: "BLP2", type (0 JPEG, 1 other), encoding (1 palette, 2 DXT, 3 BGRA), alpha depth
//! (0, 1, 4, 8), alpha encoding (0 DXT1, 1 DXT3, 7 DXT5), has-mipmaps, width, height, 16 mip
//! offsets and sizes, a 256-entry BGRA palette. Palette alpha is packed after the indices,
//! lowest bits first.
//!
//! Palette and BGRA mips decode here; DXT blocks and JPEG data are handed back
//! ([`Mip`]) for the caller's texture and JPEG decoders.

use crate::*;

/// What the first mip level holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mip {
    /// Decoded pixels.
    Pixels(Image),
    /// BC1/BC2/BC3 blocks (`kind` 1, 2 or 3) for a `width × height` image.
    Dxt {
        kind: u8,
        width: u32,
        height: u32,
        blocks: Vec<u8>,
    },
    /// A complete JPEG stream (shared header + the mip's data) whose four components are
    /// B, G, R, A; `alpha` when the alpha component is meant.
    Jpeg { jpeg: Vec<u8>, alpha: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Enc {
    Jpeg,
    Palette,
    Dxt(u8),
    Bgra,
}

struct Header {
    v2: bool,
    enc: Enc,
    alpha_bits: u32,
    width: u32,
    height: u32,
    offset: usize,
    size: usize,
    palette: usize,
}

pub fn probe(head: &[u8]) -> bool {
    head.starts_with(b"BLP1") || head.starts_with(b"BLP2")
}

fn header(d: &[u8]) -> Result<Header> {
    let v2 = if d.starts_with(b"BLP2") {
        true
    } else if d.starts_with(b"BLP1") {
        false
    } else if d.starts_with(b"BLP0") {
        return Err(Error::Unsupported(
            "BLP0 (mip levels in separate files)".into(),
        ));
    } else {
        return Err(Error::NotRecognised);
    };
    let t = || truncated("header");
    let (enc, alpha_bits, width, height, mips) = if v2 {
        let kind = u32le(d, 4).ok_or_else(t)?;
        let (e, ab, ae) = (
            *d.get(8).ok_or_else(t)?,
            *d.get(9).ok_or_else(t)?,
            *d.get(10).ok_or_else(t)?,
        );
        let enc = match (kind, e) {
            (0, _) => Enc::Jpeg,
            (_, 1) => Enc::Palette,
            (_, 2) => Enc::Dxt(match ae {
                0 => 1,
                1 => 2,
                7 => 3,
                _ => return Err(Error::Unsupported(format!("DXT alpha encoding {ae}"))),
            }),
            (_, 3) => Enc::Bgra,
            (_, e) => return Err(Error::Unsupported(format!("BLP2 encoding {e}"))),
        };
        (
            enc,
            u32::from(ab),
            u32le(d, 12).ok_or_else(t)?,
            u32le(d, 16).ok_or_else(t)?,
            20,
        )
    } else {
        let enc = match u32le(d, 4).ok_or_else(t)? {
            0 => Enc::Jpeg,
            1 => Enc::Palette,
            c => return Err(Error::Unsupported(format!("BLP1 compression {c}"))),
        };
        (
            enc,
            u32le(d, 8).ok_or_else(t)?,
            u32le(d, 12).ok_or_else(t)?,
            u32le(d, 16).ok_or_else(t)?,
            28,
        )
    };
    check_dims(width, height)?;
    if !matches!(alpha_bits, 0 | 1 | 4 | 8) {
        return Err(Error::Unsupported(format!("{alpha_bits}-bit alpha")));
    }
    let offset = u32le(d, mips).ok_or_else(t)? as usize;
    let size = u32le(d, mips + 64).ok_or_else(t)? as usize;
    Ok(Header {
        v2,
        enc,
        alpha_bits,
        width,
        height,
        offset,
        size,
        palette: mips + 128,
    })
}

pub fn info(d: &[u8]) -> Result<Info> {
    let h = header(d)?;
    let v = if h.v2 { "BLP2" } else { "BLP1" };
    let (model, what) = match h.enc {
        Enc::Jpeg => (Model::Rgb, "JPEG".to_string()),
        Enc::Palette => (Model::Palette, "8-bit palette".to_string()),
        Enc::Dxt(k) => (Model::Rgb, format!("DXT{}", [1, 3, 5][k as usize - 1])),
        Enc::Bgra => (Model::Rgb, "8-bit BGRA".to_string()),
    };
    let alpha = h.alpha_bits > 0;
    let mut coding = format!("{v}, {what}");
    if alpha {
        coding.push_str(&format!(", {}-bit alpha", h.alpha_bits));
    }
    Ok(Info::new(h.width, h.height, PixelFormat::Rgba8, model, 8, coding).alpha(alpha))
}

/// The first mip level.
pub fn decode(d: &[u8]) -> Result<Mip> {
    let h = header(d)?;
    let (w, ht) = (h.width as usize, h.height as usize);
    let n = w * ht;
    let mip = d.get(h.offset..).unwrap_or(&[]);
    let mip = &mip[..h.size.min(mip.len())];
    match h.enc {
        Enc::Jpeg => {
            let hl = u32le(d, h.palette).ok_or_else(|| truncated("JPEG header"))? as usize;
            if hl > 1 << 20 {
                return Err(Error::Malformed("JPEG header length".into()));
            }
            let head = d
                .get(h.palette + 4..h.palette + 4 + hl)
                .ok_or_else(|| truncated("JPEG header"))?;
            let mut jpeg = head.to_vec();
            jpeg.extend_from_slice(mip);
            Ok(Mip::Jpeg {
                jpeg,
                alpha: h.alpha_bits > 0,
            })
        }
        Enc::Dxt(kind) => {
            let block = if kind == 1 { 8 } else { 16 };
            let need = w.div_ceil(4) * ht.div_ceil(4) * block;
            check_payload(mip.len(), need as u64, 1)?;
            let mut blocks = mip[..need.min(mip.len())].to_vec();
            blocks.resize(need, 0);
            Ok(Mip::Dxt {
                kind,
                width: h.width,
                height: h.height,
                blocks,
            })
        }
        Enc::Bgra => {
            check_payload(mip.len(), n as u64 * 4, 1)?;
            let mut img = Image::filled(h.width, h.height, PixelFormat::Rgba8, GREY);
            img.truncated = mip.len() < n * 4;
            for (o, p) in img.data.chunks_exact_mut(4).zip(mip.chunks_exact(4)) {
                let a = if h.alpha_bits == 0 { 255 } else { p[3] };
                o.copy_from_slice(&[p[2], p[1], p[0], a]);
            }
            Ok(Mip::Pixels(img))
        }
        Enc::Palette => {
            let pal_bytes = d
                .get(h.palette..h.palette + 1024)
                .ok_or_else(|| truncated("palette"))?;
            let pal: Vec<[u8; 4]> = pal_bytes
                .chunks_exact(4)
                .map(|c| [c[2], c[1], c[0], 255])
                .collect();
            check_payload(mip.len(), n as u64, 1)?;
            let k = mip.len().min(n);
            let mut img = Image::from_indices(h.width, h.height, &mip[..k], &pal);
            let alpha = &mip[k..];
            let bits = h.alpha_bits as usize;
            if bits > 0 {
                for (i, px) in img.data.chunks_exact_mut(4).enumerate() {
                    let bit = i * bits;
                    let a = alpha.get(bit / 8).map_or(255, |&b| {
                        let v = u32::from(b >> (bit % 8)) & ((1 << bits) - 1);
                        scale_to_8(v, (1 << bits) - 1)
                    });
                    px[3] = a;
                }
            }
            img.truncated = k < n || alpha.len() < (n * bits).div_ceil(8);
            Ok(Mip::Pixels(img))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blp2(enc: u8, alpha: u8, ae: u8, w: u32, h: u32, pal: &[u8], mip: &[u8]) -> Vec<u8> {
        let mut f = b"BLP2".to_vec();
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&[enc, alpha, ae, 0]);
        f.extend_from_slice(&w.to_le_bytes());
        f.extend_from_slice(&h.to_le_bytes());
        let off = 20 + 128 + 1024;
        let mut mips = [0u32; 32];
        mips[0] = off as u32;
        mips[16] = mip.len() as u32;
        for m in mips {
            f.extend_from_slice(&m.to_le_bytes());
        }
        let mut p = pal.to_vec();
        p.resize(1024, 0);
        f.extend_from_slice(&p);
        f.extend_from_slice(mip);
        f
    }

    #[test]
    fn palette_with_4bit_alpha() {
        let pal = [10, 20, 30, 0, 1, 2, 3, 0];
        // 4 pixels: indices 0,1,0,1; alpha nibbles 0xF, 0x0, 0x8, 0x1 (low first).
        let f = blp2(1, 4, 0, 2, 2, &pal, &[0, 1, 0, 1, 0x0f, 0x18]);
        assert!(probe(&f));
        assert_eq!(info(&f).unwrap().coding, "BLP2, 8-bit palette, 4-bit alpha");
        let Mip::Pixels(img) = decode(&f).unwrap() else {
            panic!()
        };
        assert!(!img.truncated);
        assert_eq!(
            img.data,
            [30, 20, 10, 255, 3, 2, 1, 0, 30, 20, 10, 136, 3, 2, 1, 17]
        );
    }

    #[test]
    fn dxt_and_bgra() {
        let f = blp2(2, 8, 7, 5, 4, &[], &[7u8; 32]);
        assert_eq!(info(&f).unwrap().coding, "BLP2, DXT5, 8-bit alpha");
        let Mip::Dxt { kind, blocks, .. } = decode(&f).unwrap() else {
            panic!()
        };
        assert_eq!((kind, blocks.len()), (3, 32));
        let f = blp2(3, 0, 0, 1, 1, &[], &[1, 2, 3, 4]);
        assert_eq!(
            decode(&f).unwrap(),
            Mip::Pixels(Image {
                width: 1,
                height: 1,
                format: PixelFormat::Rgba8,
                data: vec![3, 2, 1, 255],
                truncated: false,
            })
        );
        assert!(matches!(header(b"BLP0aaaa"), Err(Error::Unsupported(_))));
    }
}

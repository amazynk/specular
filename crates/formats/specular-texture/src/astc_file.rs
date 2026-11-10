//! The `.astc` file written by astcenc and the ARM tools: a 16-byte header (magic 0x5CA1AB13,
//! block width, height and depth, then the image width, height and depth as 24-bit
//! little-endian numbers) followed by the blocks, row by row (and, for 3D footprints, layer of
//! blocks by layer). There is no mip chain and no colour-space or profile field: the image is
//! decoded as linear LDR unless one of its blocks uses an HDR endpoint mode, when the HDR
//! profile is used. A 3D image's depth slices are its images (pages); each decodes from the
//! layer of blocks holding it.

use crate::{Container, Error, Format, Layout, Level, Result, Supercompression, Texture, astc};

pub(crate) const MAGIC: [u8; 4] = [0x13, 0xAB, 0xA1, 0x5C];
const HEADER: usize = 16;
/// Blocks scanned for HDR endpoint modes (sampled evenly beyond this).
const HDR_SCAN: usize = 1 << 16;

fn u24(d: &[u8], p: usize) -> u32 {
    u32::from(d[p]) | u32::from(d[p + 1]) << 8 | u32::from(d[p + 2]) << 16
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER {
        return Err(Error::Truncated("ASTC header"));
    }
    let (bw, bh, bd) = (d[4], d[5], d[6]);
    let (width, height, depth) = (u24(d, 7), u24(d, 10), u24(d, 13).max(1));
    let bd = bd.max(1);
    if !astc::valid_footprint_3d(bw.into(), bh.into(), bd.into()) {
        return Err(Error::Malformed("ASTC footprint"));
    }
    if bd == 1 && depth > 1 {
        return Err(Error::Malformed("2D ASTC blocks with depth"));
    }
    let blocks = (width.div_ceil(u32::from(bw).max(1)) as usize)
        .saturating_mul(height.div_ceil(u32::from(bh).max(1)) as usize)
        .saturating_mul(depth.div_ceil(u32::from(bd)) as usize);
    let size = blocks.saturating_mul(16);
    let end = HEADER.saturating_add(size).min(d.len());
    let data = &d[HEADER..end];
    let n = data.len() / 16;
    let step = (n / HDR_SCAN).max(1);
    let hdr = (0..n)
        .step_by(step)
        .any(|i| astc::block_is_hdr(&data[i * 16..i * 16 + 16], bd.into()));
    Ok(Texture {
        container: Container::Astc,
        format: Format::Astc {
            w: bw,
            h: bh,
            d: bd,
            hdr,
        },
        width,
        height,
        depth,
        layers: 1,
        faces: 1,
        srgb: false,
        flip_y: false,
        premultiplied: false,
        levels: vec![Level {
            width,
            height,
            range: HEADER..end,
            uncompressed_len: size,
        }],
        supercompression: Supercompression::None,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 1,
        basis: None,
        seek: None,
    })
}

#[cfg(test)]
mod tests {
    use crate::{DecodeOptions, Texture};

    /// A `w`×`h` 4×4 file of LDR void-extent blocks of colour `c` (RGBA16).
    pub(crate) fn solid(w: u32, h: u32, c: [u16; 4]) -> Vec<u8> {
        let mut d = super::MAGIC.to_vec();
        d.extend_from_slice(&[4, 4, 1]);
        for v in [w, h, 1] {
            d.extend_from_slice(&v.to_le_bytes()[..3]);
        }
        let mut b = [0u8; 16];
        // Void extent (0x1FC), LDR, all coordinates set (no extent).
        let bits: u128 = 0x1fc | 3 << 10 | ((1u128 << 52) - 1) << 12;
        b.copy_from_slice(&bits.to_le_bytes());
        for (k, v) in c.iter().enumerate() {
            b[8 + 2 * k..10 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
        for _ in 0..w.div_ceil(4) * h.div_ceil(4) {
            d.extend_from_slice(&b);
        }
        d
    }

    #[test]
    fn decodes_a_solid_file() {
        let d = solid(10, 6, [0xff00, 0x8000, 0x1000, 0xffff]);
        let t = Texture::open(&d).unwrap();
        assert_eq!((t.width, t.height), (10, 6));
        assert_eq!(t.description(), "ASTC 4×4");
        let out = t.decode(&d, &DecodeOptions::default()).unwrap();
        assert!(!out.truncated);
        assert_eq!(out.data.len(), 10 * 6 * 4);
        assert!(out.data.chunks(4).all(|p| p == [0xff, 0x80, 0x10, 0xff]));
    }

    fn fnv(h: &mut u64, bytes: &[u8]) {
        for &b in bytes {
            *h = (*h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3);
        }
    }

    /// 3D footprints: 23×19×7 volumes encoded by astcenc (pinned as in
    /// tools/corpus/texture/astc.sh; `-cl` / `-ch`, `-zdim 7`, medium quality) from slices of
    /// a colour field with alpha and noise and an HDR field up to ~20. The hashes are of
    /// astcenc's own decode (`-dl`: RGBA8, `-dh`: RGBA half floats) of every slice; ours,
    /// slice by slice through the pages (LDR) and the block decoder (HDR), must be the same
    /// bits.
    #[test]
    fn three_d_matches_astcenc() {
        const FILES: [(&str, u64); 9] = [
            ("ldr_3x3x3.astc", 0x482b_8e8e_5cf0_213a),
            ("ldr_4x4x3.astc", 0xf6a3_6c5a_3ccd_f35b),
            ("ldr_4x4x4.astc", 0x07b9_329d_2f0b_f34b),
            ("ldr_5x5x4.astc", 0xf829_c324_0cbd_0d73),
            ("ldr_6x5x5.astc", 0x28ab_7b67_31e7_289d),
            ("ldr_6x6x6.astc", 0xb778_111f_0bc3_f639),
            ("hdr_3x3x3.astc", 0xd9cf_1f35_6fd9_7f72),
            ("hdr_4x4x4.astc", 0x6b22_17e4_d569_5baf),
            ("hdr_6x6x6.astc", 0x3820_c981_912d_6c64),
        ];
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/astc3d/");
        for (name, want) in FILES {
            let d = std::fs::read(format!("{dir}{name}")).unwrap();
            let mut t = Texture::open(&d).unwrap();
            assert_eq!(
                (t.width, t.height, t.depth, t.image_count()),
                (23, 19, 7, 7)
            );
            let crate::Format::Astc { w, h, d: bd, hdr } = t.format else {
                panic!("{name}: {:?}", t.format);
            };
            assert_eq!(hdr, name.starts_with("hdr"), "{name}");
            let mut hash = 0xcbf2_9ce4_8422_2325u64;
            if hdr {
                let (bw, bh, bd) = (usize::from(w), usize::from(h), usize::from(bd));
                let (bx, by) = (23usize.div_ceil(bw), 19usize.div_ceil(bh));
                let mut px = vec![0u16; bw * bh * 4];
                for z in 0..7 {
                    for y in 0..19 {
                        for x in 0..23 {
                            let b = ((z / bd) * by + y / bh) * bx + x / bw;
                            let blk = &d[16 + b * 16..][..16];
                            crate::astc::decode_hdr(blk, (bw, bh, bd), z % bd, &mut px, bw * 4);
                            let o = ((y % bh) * bw + x % bw) * 4;
                            for v in &px[o..o + 4] {
                                fnv(&mut hash, &v.to_le_bytes());
                            }
                        }
                    }
                }
            }
            for z in 0..7 {
                assert!(t.select_image(z));
                let out = t.decode(&d, &DecodeOptions::default()).unwrap();
                assert!(!out.truncated, "{name} slice {z}");
                if !hdr {
                    fnv(&mut hash, &out.data);
                }
            }
            assert_eq!(hash, want, "{name}");
        }
    }

    #[test]
    fn truncated_and_malformed_files_fail_cleanly() {
        let d = solid(64, 64, [0, 0, 0, 0xffff]);
        for n in [0, 4, 15, 16, 17, 100, d.len() - 1] {
            if let Ok(t) = Texture::open(&d[..n]) {
                let out = t.decode(&d[..n], &DecodeOptions::default()).unwrap();
                assert!(out.truncated);
            }
        }
        let mut bad = d.clone();
        bad[4] = 7; // 7×4 is not a footprint
        assert!(Texture::open(&bad).is_err());
        let mut big = d.clone();
        big[7..10].copy_from_slice(&[0xff, 0xff, 0xff]);
        assert!(Texture::open(&big).is_err());
        // 4×4×7 is not a footprint; 2D blocks cannot cover several slices.
        let mut deep = d.clone();
        deep[6] = 7;
        assert!(Texture::open(&deep).is_err());
        let mut slices = d;
        slices[13] = 2;
        assert!(Texture::open(&slices).is_err());
    }
}

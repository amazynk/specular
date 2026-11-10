//! KTX2 container: header, level index, key/value data (orientation) and supercompression
//! (none, Zstandard, zlib, BasisLZ; vkFormat 0 is Basis Universal ETC1S or UASTC by the DFD).
//! Within a level, images are ordered layer, face, z slice.

use crate::{
    Basis2, BasisGlobal, Container, Error, Format, Layout, Level, Result, Supercompression,
    Texture, basis,
};

pub(crate) const IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
const HEADER: usize = 80;

fn u32_at(d: &[u8], p: usize) -> u32 {
    d.get(p..p + 4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn u64_at(d: &[u8], p: usize) -> u64 {
    d.get(p..p + 8)
        .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
}

fn masked(bytes: u8, masks: [u32; 4]) -> Format {
    Format::Masked {
        bytes,
        masks,
        grey: false,
    }
}

/// The ASTC 2D footprints in VkFormat (and GL, PVR) order.
pub(crate) const ASTC_FOOTPRINTS: [(u8, u8); 14] = [
    (4, 4),
    (5, 4),
    (5, 5),
    (6, 5),
    (6, 6),
    (8, 5),
    (8, 6),
    (8, 8),
    (10, 5),
    (10, 6),
    (10, 8),
    (10, 10),
    (12, 10),
    (12, 12),
];

/// ASTC format of footprint `i` of [`ASTC_FOOTPRINTS`].
pub(crate) fn astc(i: usize, hdr: bool) -> Format {
    let (w, h) = ASTC_FOOTPRINTS[i];
    Format::Astc { w, h, d: 1, hdr }
}

/// VkFormat → (format, sRGB).
pub(crate) fn vk(code: u32) -> Option<(Format, bool)> {
    use Format::*;
    let srgb = matches!(
        code,
        15 | 22
            | 29
            | 36
            | 43
            | 50
            | 132
            | 134
            | 136
            | 138
            | 146
            | 148
            | 150
            | 152
            | 1_000_054_004
            | 1_000_054_005
    ) || (157..=184).contains(&code) && code.is_multiple_of(2);
    let f = match code {
        2 => masked(2, [0xf000, 0xf00, 0xf0, 0xf]),
        3 => masked(2, [0xf0, 0xf00, 0xf000, 0xf]),
        4 => masked(2, [0xf800, 0x7e0, 0x1f, 0]),
        5 => masked(2, [0x1f, 0x7e0, 0xf800, 0]),
        6 => masked(2, [0xf800, 0x7c0, 0x3e, 0x1]),
        7 => masked(2, [0x3e, 0x7c0, 0xf800, 0x1]),
        8 => masked(2, [0x7c00, 0x3e0, 0x1f, 0x8000]),
        9 | 15 => Masked {
            bytes: 1,
            masks: [0xff, 0, 0, 0],
            grey: true,
        },
        10 => Snorm8 { channels: 1 },
        16 | 22 => masked(2, [0xff, 0xff00, 0, 0]),
        17 => Snorm8 { channels: 2 },
        23 | 29 => masked(3, [0xff, 0xff00, 0xff_0000, 0]),
        30 | 36 => masked(3, [0xff_0000, 0xff00, 0xff, 0]),
        37 | 43 => masked(4, [0xff, 0xff00, 0xff_0000, 0xff00_0000]),
        38 => Snorm8 { channels: 4 },
        44 | 50 => masked(4, [0xff_0000, 0xff00, 0xff, 0xff00_0000]),
        58 => masked(4, [0x3ff0_0000, 0xffc00, 0x3ff, 0xc000_0000]),
        64 => masked(4, [0x3ff, 0xffc00, 0x3ff0_0000, 0xc000_0000]),
        70 => Unorm16 { channels: 1 },
        77 => Unorm16 { channels: 2 },
        84 => Unorm16 { channels: 3 },
        91 => Unorm16 { channels: 4 },
        76 => Half { channels: 1 },
        83 => Half { channels: 2 },
        90 => Half { channels: 3 },
        97 => Half { channels: 4 },
        100 => Float { channels: 1 },
        103 => Float { channels: 2 },
        106 => Float { channels: 3 },
        109 => Float { channels: 4 },
        122 => R11G11B10F,
        123 => Rgb9e5,
        131 | 132 => Bc1 { alpha: false },
        133 | 134 => Bc1 { alpha: true },
        135 | 136 => Bc2,
        137 | 138 => Bc3,
        139 => Bc4 { signed: false },
        140 => Bc4 { signed: true },
        141 => Bc5 { signed: false },
        142 => Bc5 { signed: true },
        143 => Bc6h { signed: false },
        144 => Bc6h { signed: true },
        145 | 146 => Bc7,
        147 | 148 => Etc2Rgb,
        149 | 150 => Etc2Rgba1,
        151 | 152 => Etc2Rgba,
        153 => EacR11 { signed: false },
        154 => EacR11 { signed: true },
        155 => EacRg11 { signed: false },
        156 => EacRg11 { signed: true },
        157..=184 => astc(((code - 157) / 2) as usize, false),
        1_000_066_000..=1_000_066_013 => astc((code - 1_000_066_000) as usize, true),
        1_000_054_000 | 1_000_054_004 => Pvrtc {
            two_bpp: true,
            alpha: true,
        },
        1_000_054_001 | 1_000_054_005 => Pvrtc {
            two_bpp: false,
            alpha: true,
        },
        _ => return None,
    };
    Some((f, srgb))
}

fn vk_name(code: u32) -> String {
    match code {
        0 => "Basis Universal / undefined format".into(),
        1_000_054_002 | 1_000_054_003 | 1_000_054_006 | 1_000_054_007 => "PVRTC2".into(),
        _ => format!("VkFormat {code}"),
    }
}

/// Whether the key/value data says rows go bottom-up.
fn flip_y(kvd: &[u8]) -> bool {
    let mut p = 0;
    while p + 4 <= kvd.len() {
        let len = u32_at(kvd, p) as usize;
        let Some(kv) = kvd.get(p + 4..(p + 4).saturating_add(len)) else {
            break;
        };
        if let Some(z) = kv.iter().position(|&b| b == 0)
            && &kv[..z] == b"KTXorientation"
        {
            return kv.get(z + 2) == Some(&b'u');
        }
        p += 4 + len.div_ceil(4) * 4;
    }
    false
}

/// Whether the data format descriptor's basic block sets KHR_DF_FLAG_ALPHA_PREMULTIPLIED.
/// The DFD is a total size word, then blocks: the basic one (Khronos vendor 0, type 0) has
/// its colour model, primaries, transfer and flags bytes in its third word.
fn dfd_premultiplied(dfd: &[u8]) -> bool {
    let word0 = u32_at(dfd, 4);
    // vendorId in bits 0..17, descriptorType in bits 17..32.
    dfd.len() >= 16 && word0 & 0x1_ffff == 0 && word0 >> 17 == 0 && dfd[15] & 1 != 0
}

/// The format of vkFormat 0 (Basis Universal) from the DFD's basic block: colour model 163
/// ETC1S (BasisLZ only) or 166 UASTC, its sample channel ids, and the sRGB transfer function.
fn basis_format(dfd: &[u8], s: Supercompression) -> Result<(Format, bool)> {
    if dfd.len() < 28 {
        return Err(Error::Truncated("KTX2 data format descriptor"));
    }
    let (model, transfer) = (dfd[12], dfd[14]);
    let block_size = usize::from(u16::from_le_bytes([dfd[10], dfd[11]]));
    let channels: Vec<u8> = (0..block_size.saturating_sub(24) / 16)
        .filter_map(|i| dfd.get(4 + 24 + 16 * i + 3).map(|b| b & 0x0f))
        .collect();
    let srgb = transfer == 2;
    let format = match (model, s) {
        // Channel ids: 0 RGB, 3 RRR, 4 GGG, 15 AAA.
        (163, Supercompression::BasisLz) => Format::Etc1s {
            alpha: channels.contains(&15),
            rg: channels.first() == Some(&3) && channels.get(1) == Some(&4),
        },
        (163, _) => return Err(Error::Malformed("KTX2 ETC1S without BasisLZ")),
        (166, Supercompression::BasisLz) => {
            return Err(Error::Malformed("KTX2 UASTC with BasisLZ"));
        }
        (166 | 167, Supercompression::Basis2(_)) => {
            return Err(Error::Malformed(
                "KTX2 Basis Universal model with another codec's supercompression",
            ));
        }
        // Channel ids: 0 RGB, 3 RGBA, 4 RRR, 5 RRRG, 6 RG.
        (166, _) => Format::Uastc {
            alpha: matches!(channels.first(), Some(3 | 5)),
            rg: channels.first() == Some(&5),
        },
        // UASTC HDR 4×4 blocks are ASTC HDR 4×4 blocks (a subset of the modes: the Basis
        // transcoder turns them into ASTC by copying).
        (167, Supercompression::BasisLz) => {
            return Err(Error::Malformed("KTX2 UASTC HDR with BasisLZ"));
        }
        (167, _) => Format::Astc {
            w: 4,
            h: 4,
            d: 1,
            hdr: true,
        },
        (168, Supercompression::Basis2(Basis2::Hdr6x6i)) => Format::Astc {
            w: 6,
            h: 6,
            d: 1,
            hdr: true,
        },
        (168, _) => {
            return Err(Error::Malformed(
                "KTX2 UASTC HDR 6×6 intermediate without its supercompression",
            ));
        }
        // XUASTC LDR: the ASTC footprint is in the DFD's texel block dimensions (minus one).
        (169, Supercompression::Basis2(Basis2::Xuastc)) => {
            let (w, h) = (dfd[16] + 1, dfd[17] + 1);
            if !crate::astc::valid_footprint(usize::from(w), usize::from(h)) {
                return Err(Error::Malformed("KTX2 XUASTC LDR block size"));
            }
            Format::Astc {
                w,
                h,
                d: 1,
                hdr: false,
            }
        }
        (169, _) => {
            return Err(Error::Malformed(
                "KTX2 XUASTC LDR without its supercompression",
            ));
        }
        (170, Supercompression::Basis2(Basis2::Xubc7)) => Format::Bc7,
        (170, _) => return Err(Error::Malformed("KTX2 XUBC7 without its supercompression")),
        (m, _) => {
            return Err(Error::Unsupported(format!(
                "KTX2 vkFormat 0 with colour model {m}"
            )));
        }
    };
    Ok((format, srgb))
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER {
        return Err(Error::Truncated("KTX2 header"));
    }
    let code = u32_at(d, 12);
    let width = u32_at(d, 20);
    let height = u32_at(d, 24).max(1);
    let depth = u32_at(d, 28).max(1);
    let layers = u32_at(d, 32).max(1);
    let faces = u32_at(d, 36).max(1);
    let level_count = u32_at(d, 40).max(1);
    let scheme = u32_at(d, 44);
    let dfd_off = u32_at(d, 48) as usize;
    let dfd_len = u32_at(d, 52) as usize;
    let kvd_off = u32_at(d, 56) as usize;
    let kvd_len = u32_at(d, 60) as usize;
    let supercompression = match scheme {
        0 => Supercompression::None,
        2 => Supercompression::Zstd,
        3 => Supercompression::Zlib,
        1 => Supercompression::BasisLz,
        4 => Supercompression::Basis2(Basis2::Hdr6x6i),
        5 => Supercompression::Basis2(Basis2::Xuastc),
        6 => Supercompression::Basis2(Basis2::Xubc7),
        s => return Err(Error::Unsupported(format!("KTX2 supercompression {s}"))),
    };
    let dfd = d
        .get(dfd_off..dfd_off.saturating_add(dfd_len))
        .unwrap_or(&[]);
    // basisu v2.0 stored UASTC HDR 6×6i and XUASTC LDR under the BasisLZ scheme, with an
    // 8-byte seek table entry (offset, length) per image.
    let (supercompression, seek_entry) = match (supercompression, dfd.get(12)) {
        (Supercompression::BasisLz, Some(168)) if code == 0 => {
            (Supercompression::Basis2(Basis2::Hdr6x6i), 8)
        }
        (Supercompression::BasisLz, Some(169)) if code == 0 => {
            (Supercompression::Basis2(Basis2::Xuastc), 8)
        }
        (s, _) => (s, 12),
    };
    let (format, srgb) = match (code, supercompression) {
        (0, _) => basis_format(dfd, supercompression)?,
        (_, Supercompression::BasisLz) => {
            return Err(Error::Malformed("KTX2 BasisLZ with a vkFormat"));
        }
        (_, Supercompression::Basis2(_)) => {
            return Err(Error::Malformed(
                "KTX2 Basis Universal supercompression with a vkFormat",
            ));
        }
        _ => vk(code).ok_or_else(|| Error::Unsupported(vk_name(code)))?,
    };
    if level_count > 32 {
        return Err(Error::Malformed("KTX2 level count"));
    }
    let basis = if supercompression == Supercompression::BasisLz {
        if depth > 1 {
            return Err(Error::Unsupported("KTX2 BasisLZ 3D texture".into()));
        }
        let (sgd_off, sgd_len) = (u64_at(d, 64), u64_at(d, 72));
        let sgd = usize::try_from(sgd_off)
            .ok()
            .zip(usize::try_from(sgd_len).ok())
            .and_then(|(o, l)| d.get(o..o.checked_add(l)?))
            .ok_or(Error::Truncated("KTX2 supercompression global data"))?;
        let images = (level_count as usize)
            .saturating_mul(layers as usize)
            .saturating_mul(faces as usize);
        let g = basis::etc1s::Etc1sGlobal::parse(sgd, images)?;
        Some(BasisGlobal(std::sync::Arc::new(g)))
    } else {
        None
    };
    let seek = if let Supercompression::Basis2(_) = supercompression {
        let images = (level_count as usize)
            .saturating_mul(layers as usize)
            .saturating_mul(faces as usize);
        Some(seek_table(d, images, seek_entry)?)
    } else {
        None
    };
    // ETC1S blocks carry an alpha block when the images have alpha slices.
    let format = match (format, &basis) {
        (Format::Etc1s { rg, .. }, Some(g)) => Format::Etc1s {
            alpha: g.0.has_alpha(),
            rg,
        },
        _ => format,
    };
    if d.len() < HEADER + 24 * level_count as usize {
        return Err(Error::Truncated("KTX2 level index"));
    }
    // As DDS: float formats would need the division in linear light, so stay as stored.
    let premultiplied = format.has_alpha()
        && !format.is_float()
        && d.get(dfd_off..dfd_off.saturating_add(dfd_len))
            .is_some_and(dfd_premultiplied);
    let flip = d
        .get(kvd_off..kvd_off.saturating_add(kvd_len))
        .is_some_and(flip_y);
    let mut levels = Vec::new();
    for k in 0..level_count {
        let p = HEADER + 24 * k as usize;
        let (off, len, ulen) = (u64_at(d, p), u64_at(d, p + 8), u64_at(d, p + 16));
        if off >= d.len() as u64 {
            break;
        }
        let end = off.saturating_add(len).min(d.len() as u64);
        let ulen = if supercompression == Supercompression::None {
            len
        } else {
            ulen
        };
        levels.push(Level {
            width: (width >> k).max(1),
            height: (height >> k).max(1),
            range: off as usize..end as usize,
            uncompressed_len: usize::try_from(ulen).unwrap_or(usize::MAX),
        });
    }
    Ok(Texture {
        premultiplied,
        container: Container::Ktx2,
        format,
        width,
        height,
        depth,
        layers,
        faces,
        srgb,
        flip_y: flip,
        levels,
        supercompression,
        layout: Layout::PerLevel,
        image: 0,
        row_align: 1,
        basis,
        seek,
    })
}

/// The seek table of the Basis Universal intermediate formats in the supercompression global
/// data: per image, its offset from the level start, its length and (`entry` 12) a profile
/// word.
fn seek_table(d: &[u8], images: usize, entry: usize) -> Result<std::sync::Arc<[(usize, usize)]>> {
    let (off, len) = (u64_at(d, 64), u64_at(d, 72));
    let sgd = usize::try_from(off)
        .ok()
        .zip(usize::try_from(len).ok())
        .and_then(|(o, l)| d.get(o..o.checked_add(l)?))
        .ok_or(Error::Truncated("KTX2 supercompression global data"))?;
    if sgd.len() < images.saturating_mul(entry) {
        return Err(Error::Truncated("KTX2 seek table"));
    }
    Ok(sgd
        .chunks_exact(entry)
        .take(images)
        .map(|e| (u32_at(e, 0) as usize, u32_at(e, 4) as usize))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UASTC HDR 4×4 and ASTC HDR 6×6 written by basisu (tests/data, see tests/hdr_basis.rs):
    /// every texel's half-float colour bits equal astcenc's `-dh` decode of the same blocks.
    #[test]
    fn basis_hdr_blocks_match_the_transcoder() {
        use std::io::Read;
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
        for (name, b) in [("hdr4", 4usize), ("hdr6", 6)] {
            let d = std::fs::read(format!("{dir}{name}.ktx2")).unwrap();
            let refs = std::fs::read(format!("{dir}{name}.rgbah")).unwrap();
            let t = parse(&d).unwrap();
            assert_eq!(
                t.format,
                Format::Astc {
                    w: b as u8,
                    h: b as u8,
                    d: 1,
                    hdr: true
                }
            );
            let l = &t.levels[0];
            let src = &d[l.range.clone()];
            let blocks = match t.supercompression {
                Supercompression::Zstd => {
                    let mut v = Vec::new();
                    ruzstd::decoding::StreamingDecoder::new(src)
                        .unwrap()
                        .read_to_end(&mut v)
                        .unwrap();
                    v
                }
                _ => src.to_vec(),
            };
            let (w, h) = (t.width as usize, t.height as usize);
            let bx = w.div_ceil(b);
            let mut px = vec![0u16; b * b * 4];
            let mut diff = 0;
            for y in 0..h {
                for x in 0..w {
                    let blk = &blocks[((y / b) * bx + x / b) * 16..][..16];
                    crate::astc::decode_hdr(blk, (b, b, 1), 0, &mut px, b * 4);
                    let o = ((y % b) * b + x % b) * 4;
                    for c in 0..3 {
                        let r = (y * w + x) * 4 + c;
                        let want = u16::from_le_bytes([refs[2 * r], refs[2 * r + 1]]);
                        diff += usize::from(px[o + c] != want);
                    }
                }
            }
            assert_eq!(diff, 0, "{name}: {diff} channels differ");
        }
        // The same UASTC HDR 4×4 blocks labelled the Basis way (vkFormat 0, DFD colour model
        // 167) read as ASTC HDR 4×4 too.
        let mut d = std::fs::read(format!("{dir}hdr4.ktx2")).unwrap();
        d[12..16].fill(0);
        let dfd = u32_at(&d, 48) as usize;
        d[dfd + 12] = 167;
        let t = parse(&d).unwrap();
        assert_eq!(
            t.format,
            Format::Astc {
                w: 4,
                h: 4,
                d: 1,
                hdr: true
            }
        );
    }

    /// A basic DFD block (Khronos, type 0): total size, two header words, then colour model,
    /// primaries, transfer and flags.
    fn with_dfd(flags: u8) -> Vec<u8> {
        let mut d = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fuzz/seeds/rgba8_zlib.ktx2"
        ))
        .unwrap();
        let off = d.len() as u32;
        let mut dfd = Vec::new();
        dfd.extend_from_slice(&28u32.to_le_bytes());
        dfd.extend_from_slice(&0u32.to_le_bytes());
        dfd.extend_from_slice(&(2 | (24 << 16) as u32).to_le_bytes());
        dfd.extend_from_slice(&[1, 1, 2, flags]);
        dfd.extend_from_slice(&[0; 12]);
        d.extend_from_slice(&dfd);
        d[48..52].copy_from_slice(&off.to_le_bytes());
        d[52..56].copy_from_slice(&(dfd.len() as u32).to_le_bytes());
        d
    }

    #[test]
    fn reads_the_dfd_premultiplied_flag() {
        assert!(!parse(&with_dfd(0)).unwrap().premultiplied);
        assert!(parse(&with_dfd(1)).unwrap().premultiplied);
        // Another vendor's block is not the basic descriptor.
        let mut d = with_dfd(1);
        let off = u32_at(&d, 48) as usize;
        d[off + 4] = 1;
        assert!(!parse(&d).unwrap().premultiplied);
    }
}

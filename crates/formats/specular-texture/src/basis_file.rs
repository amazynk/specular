//! The `.basis` file of Basis Universal (format version 0x13): a 77-byte packed little-endian
//! header, a table of 23-byte slice descriptors, the ETC1S codebooks and Huffman tables, and the
//! slices. Each slice is one mip level of one image; ETC1S images with alpha have a second
//! slice (flagged) holding alpha in its green channel, as KTX2 BasisLZ does. The ETC1S
//! codebooks, tables and slice bitstreams are the ones KTX2 BasisLZ carries, so they go through
//! the same transcoder ([`crate::basis::etc1s`]); UASTC slices are plain 16-byte blocks.
//!
//! Texture types: 2D images (an array, shown as pages), 2D arrays, cube map arrays (six faces
//! per image), video frames (I- and P-frames, shown as pages) and volumes (slices as pages).
//! Images of different sizes in one file are not paged: only the first is shown.

use std::sync::Arc;

use crate::basis::etc1s::{Etc1sGlobal, ImageDesc};
use crate::{
    Basis2, BasisGlobal, Container, Error, Format, Layout, Level, Result, Supercompression, Texture,
};

const HEADER: usize = 77;
/// ASTC footprints of the XUASTC LDR texture formats 5…18.
const XUASTC_BLOCKS: [(u32, u32); 14] = [
    (4, 4),
    (5, 4),
    (5, 5),
    (6, 5),
    (6, 6),
    (8, 5),
    (8, 6),
    (10, 5),
    (10, 6),
    (8, 8),
    (10, 8),
    (10, 10),
    (12, 10),
    (12, 12),
];
const SLICE: usize = 23;
const MAX_LEVELS: usize = 17;
/// The ETC1S P-frame flag of [`ImageDesc`] (KTX2's `IS_P_FRAME`).
const P_FRAME: u32 = 2;

/// Whether the bytes start like a `.basis` file: the signature "sB" and the header size.
pub(crate) fn is_basis(d: &[u8]) -> bool {
    d.len() >= 6 && d.starts_with(b"sB") && d[4] == HEADER as u8 && d[5] == 0
}

fn u16_at(d: &[u8], p: usize) -> usize {
    usize::from(u16::from_le_bytes([d[p], d[p + 1]]))
}

fn u24_at(d: &[u8], p: usize) -> usize {
    usize::from(d[p]) | usize::from(d[p + 1]) << 8 | usize::from(d[p + 2]) << 16
}

fn u32_at(d: &[u8], p: usize) -> usize {
    u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]) as usize
}

#[derive(Clone, Copy)]
struct Slice {
    image: usize,
    level: usize,
    alpha: bool,
    iframe: bool,
    width: u32,
    height: u32,
    blocks: (usize, usize),
    range: (usize, usize),
}

fn section(d: &[u8], off: usize, len: usize) -> Result<&[u8]> {
    off.checked_add(len)
        .and_then(|end| d.get(off..end))
        .ok_or(Error::Truncated("Basis codebooks"))
}

pub(crate) fn parse(d: &[u8]) -> Result<Texture> {
    if d.len() < HEADER || !is_basis(d) {
        return Err(Error::Truncated("Basis header"));
    }
    let version = u16_at(d, 2);
    if version != 0x13 {
        return Err(Error::Unsupported(format!(
            "Basis file version {version:#x}"
        )));
    }
    let total_slices = u24_at(d, 14);
    let tex_format = d[20];
    let flags = u16_at(d, 21);
    let tex_type = d[23];
    // 0 ETC1S, 1 UASTC LDR 4×4, 2 UASTC HDR 4×4 (ASTC HDR 4×4 blocks, a subset of the
    // modes), 3 ASTC HDR 6×6 (plain blocks); the intermediate codecs, one stream per slice:
    // 4 UASTC HDR 6×6i (to ASTC HDR 6×6), 5–18 XUASTC LDR (to ASTC LDR of the same
    // footprint), 33 XUBC7 (to BC7).
    let (etc1s, (bw, bh)) = match tex_format {
        0 => (true, (4, 4)),
        1 | 2 => (false, (4, 4)),
        3 | 4 => (false, (6, 6)),
        5..=18 => (false, XUASTC_BLOCKS[usize::from(tex_format - 5)]),
        33 => (false, (4, 4)),
        f => return Err(Error::Unsupported(format!("Basis texture format {f}"))),
    };
    let codec = match tex_format {
        4 => Some(Basis2::Hdr6x6i),
        5..=18 => Some(Basis2::Xuastc),
        33 => Some(Basis2::Xubc7),
        _ => None,
    };
    if etc1s && flags & 8 != 0 {
        return Err(Error::Unsupported(
            "Basis ETC1S with an external codebook".into(),
        ));
    }
    let slice_ofs = u32_at(d, 65);
    let slice_table = slice_ofs
        .checked_add(total_slices.saturating_mul(SLICE))
        .and_then(|end| d.get(slice_ofs..end))
        .ok_or(Error::Truncated("Basis slice table"))?;
    if total_slices == 0 {
        return Err(Error::Truncated("no image data"));
    }
    let video = tex_type == 3;
    let slices: Vec<Slice> = slice_table
        .chunks_exact(SLICE)
        .map(|s| Slice {
            image: u24_at(s, 0),
            level: usize::from(s[3]),
            alpha: s[4] & 1 != 0,
            iframe: s[4] & 2 != 0,
            width: u16_at(s, 5) as u32,
            height: u16_at(s, 7) as u32,
            blocks: (u16_at(s, 9), u16_at(s, 11)),
            range: (u32_at(s, 13), u32_at(s, 17)),
        })
        .collect();
    if slices.iter().any(|s| s.level >= MAX_LEVELS) {
        return Err(Error::Malformed("Basis mip level"));
    }
    let images_in_file = slices.iter().map(|s| s.image + 1).max().unwrap_or(1);
    if images_in_file > total_slices {
        return Err(Error::Malformed("Basis image index"));
    }
    // The colour slice of (image, level): the first listed. Indexed, as the checks below look
    // up every image and the table can hold millions of slices.
    let mut colour_of = std::collections::HashMap::with_capacity(slices.len());
    for s in slices.iter().filter(|s| !(etc1s && s.alpha)) {
        colour_of.entry((s.image, s.level)).or_insert(s);
    }
    let colour = |image: usize, level: usize| colour_of.get(&(image, level)).copied();
    let first = colour(0, 0).ok_or(Error::Truncated("no image data"))?;
    let (width, height) = (first.width, first.height);
    let levels_n = (0..MAX_LEVELS)
        .take_while(|&k| colour(0, k).is_some())
        .count();
    let level_size = |k: usize| colour(0, k).map(|s| (s.width, s.height));
    for k in 0..levels_n {
        let s = colour(0, k).ok_or(Error::Truncated("Basis level"))?;
        if s.blocks
            != (
                s.width.div_ceil(bw) as usize,
                s.height.div_ceil(bh) as usize,
            )
        {
            return Err(Error::Malformed("Basis slice block count"));
        }
    }
    // Page only images whose levels all match the first image's.
    let same = |i: usize| {
        (0..levels_n).all(|k| colour(i, k).map(|s| (s.width, s.height)) == level_size(k))
    };
    let mut images = if (1..images_in_file).all(same) {
        images_in_file
    } else {
        1
    };
    let mut layout = Layout::PerLevel;
    let mut levels = Vec::with_capacity(levels_n);
    let mut seek = None;
    let basis = if codec.is_some() {
        // Each slice is its own stream: the seek table holds them level-major, image by image.
        let mut table = Vec::with_capacity(levels_n * images);
        for k in 0..levels_n {
            for i in 0..images {
                let s = colour(i, k).ok_or(Error::Truncated("Basis level"))?;
                table.push(s.range);
            }
            let (w, h) = level_size(k).unwrap_or((1, 1));
            levels.push(Level {
                width: w,
                height: h,
                range: 0..d.len(),
                uncompressed_len: 0,
            });
        }
        seek = Some(Arc::from(table));
        None
    } else if etc1s {
        let mut descs = vec![ImageDesc::default(); levels_n * images];
        for s in &slices {
            if s.image >= images || s.level >= levels_n {
                continue;
            }
            let d = &mut descs[s.level * images + s.image];
            if s.alpha {
                d.alpha = s.range;
            } else {
                d.rgb = s.range;
                if video && !s.iframe {
                    d.flags |= P_FRAME;
                }
            }
        }
        let ep = section(d, u32_at(d, 41), u24_at(d, 45))?;
        let sel = section(d, u32_at(d, 50), u24_at(d, 54))?;
        let tables = section(d, u32_at(d, 57), u32_at(d, 61))?;
        let g = Etc1sGlobal::from_parts(u16_at(d, 39), u16_at(d, 48), ep, sel, tables, descs)?;
        for k in 0..levels_n {
            let (w, h) = level_size(k).unwrap_or((1, 1));
            levels.push(Level {
                width: w,
                height: h,
                range: 0..d.len(),
                uncompressed_len: 0,
            });
        }
        Some(BasisGlobal(Arc::new(g)))
    } else {
        // UASTC: the images' slices of a level must be evenly spaced (they are, written image
        // by image) to be paged; otherwise only the first image is shown.
        let at = |i: usize, k: usize| colour(i, k).map(|s| s.range.0);
        let chain = if images > 1 {
            at(1, 0).zip(at(0, 0)).and_then(|(b, a)| b.checked_sub(a))
        } else {
            None
        };
        match chain {
            Some(c)
                if c > 0
                    && (0..images).all(|i| {
                        (0..levels_n).all(|k| at(i, k) == at(0, k).map(|o| o + i * c))
                    }) =>
            {
                layout = Layout::PerChain { chain: c as u64 };
            }
            _ => images = 1,
        }
        for k in 0..levels_n {
            let s = colour(0, k).ok_or(Error::Truncated("Basis level"))?;
            let start = s.range.0.min(d.len());
            let end = start.saturating_add(s.range.1).min(d.len());
            levels.push(Level {
                width: s.width,
                height: s.height,
                range: start..end,
                uncompressed_len: s.range.1,
            });
        }
        None
    };
    let alpha = flags & 4 != 0;
    let format = if etc1s {
        Format::Etc1s {
            alpha: basis.as_ref().is_some_and(|g| g.0.has_alpha()),
            rg: false,
        }
    } else if tex_format == 1 {
        Format::Uastc { alpha, rg: false }
    } else if codec == Some(Basis2::Xubc7) {
        Format::Bc7
    } else {
        Format::Astc {
            w: bw as u8,
            h: bh as u8,
            d: 1,
            hdr: tex_format < 5,
        }
    };
    let (layers, faces, depth) = match tex_type {
        2 if images.is_multiple_of(6) => (images as u32 / 6, 6, 1),
        _ => (images as u32, 1, 1),
    };
    Ok(Texture {
        container: Container::Basis,
        format,
        width,
        height,
        depth,
        layers,
        faces,
        srgb: flags & 16 != 0,
        flip_y: flags & 2 != 0,
        premultiplied: false,
        levels,
        supercompression: if etc1s {
            Supercompression::BasisLz
        } else if let Some(c) = codec {
            Supercompression::Basis2(c)
        } else {
            Supercompression::None
        },
        layout,
        image: 0,
        row_align: 1,
        basis,
        seek,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodeOptions;

    /// A UASTC `.basis` file of `images` images, each `w`×`h` with one level, every block a
    /// solid-colour UASTC block (mode 8, void extent) of `c`.
    fn uastc_file(w: u16, h: u16, images: usize, c: [u8; 4]) -> Vec<u8> {
        let (bx, by) = (usize::from(w).div_ceil(4), usize::from(h).div_ceil(4));
        let mut block = [0u8; 16];
        // UASTC mode 8 (solid colour): mode code 0b10111 in 5 bits, then RGBA 8 bits each.
        let bits: u128 = 0b1_0111
            | u128::from(c[0]) << 5
            | u128::from(c[1]) << 13
            | u128::from(c[2]) << 21
            | u128::from(c[3]) << 29;
        block.copy_from_slice(&bits.to_le_bytes());
        let slice_ofs = HEADER;
        let data_ofs = HEADER + images * SLICE;
        let size = bx * by * 16;
        let mut d = vec![0u8; HEADER];
        d[..2].copy_from_slice(b"sB");
        d[2..4].copy_from_slice(&0x13u16.to_le_bytes());
        d[4] = HEADER as u8;
        d[14..17].copy_from_slice(&(images as u32).to_le_bytes()[..3]);
        d[17..20].copy_from_slice(&(images as u32).to_le_bytes()[..3]);
        d[20] = 1;
        d[23] = 1;
        d[65..69].copy_from_slice(&(slice_ofs as u32).to_le_bytes());
        for i in 0..images {
            let mut s = [0u8; SLICE];
            s[..3].copy_from_slice(&(i as u32).to_le_bytes()[..3]);
            s[5..7].copy_from_slice(&w.to_le_bytes());
            s[7..9].copy_from_slice(&h.to_le_bytes());
            s[9..11].copy_from_slice(&(bx as u16).to_le_bytes());
            s[11..13].copy_from_slice(&(by as u16).to_le_bytes());
            s[13..17].copy_from_slice(&((data_ofs + i * size) as u32).to_le_bytes());
            s[17..21].copy_from_slice(&(size as u32).to_le_bytes());
            d.extend_from_slice(&s);
        }
        for _ in 0..images * bx * by {
            d.extend_from_slice(&block);
        }
        d
    }

    #[test]
    fn decodes_a_uastc_file_and_pages_its_images() {
        let d = uastc_file(9, 5, 3, [10, 200, 30, 255]);
        let mut t = Texture::open(&d).unwrap();
        assert_eq!((t.width, t.height, t.image_count()), (9, 5, 3));
        assert_eq!(t.description(), "Basis UASTC RGB, 3 layers");
        assert!(t.select_image(2));
        let out = t.decode(&d, &DecodeOptions::default()).unwrap();
        assert!(!out.truncated);
        assert_eq!(out.data.len(), 9 * 5 * 4);
        assert!(
            out.data.chunks(4).all(|p| p == [10, 200, 30, 255]),
            "{:?}",
            &out.data[..8]
        );
    }

    #[test]
    fn malformed_files_fail_cleanly() {
        let d = uastc_file(64, 64, 2, [1, 2, 3, 255]);
        for n in [0, 6, 76, 77, 80, 100, 123, 200, d.len() - 1] {
            if let Ok(t) = Texture::open(&d[..n]) {
                let _ = t.decode(&d[..n], &DecodeOptions::default());
            }
        }
        let mut bad = d.clone();
        bad[HEADER + 3] = 40; // level index
        assert!(Texture::open(&bad).is_err());
        let mut bad = d.clone();
        bad[HEADER + 9] = 99; // block count
        assert!(Texture::open(&bad).is_err());
        let mut bad = d.clone();
        bad[2] = 0x10;
        assert!(Texture::open(&bad).is_err());
        let mut bad = d.clone();
        bad[20] = 0; // ETC1S without codebooks
        assert!(Texture::open(&bad).is_err());
        let mut bad = d;
        bad[14..17].copy_from_slice(&[0xff, 0xff, 0xff]);
        assert!(Texture::open(&bad).is_err());
    }

    fn le(d: &[u8], p: usize, n: usize) -> usize {
        (0..n).fold(0, |a, i| a | usize::from(d[p + i]) << (8 * i))
    }

    /// Repacks a KTX2 Basis file (BasisLZ ETC1S, or UASTC without supercompression) as a
    /// `.basis` file: the codebooks and slices are the same bytes, so the result must decode
    /// exactly as basisu decoded the KTX2 file. `None` for other KTX2 files.
    fn ktx2_to_basis(k: &[u8]) -> Option<Vec<u8>> {
        let (w, h) = (le(k, 20, 4), le(k, 24, 4).max(1));
        let (layers, faces) = (le(k, 32, 4).max(1), le(k, 36, 4).max(1));
        let (levels, scheme) = (le(k, 40, 4).max(1), le(k, 44, 4));
        let dfd = le(k, 48, 4);
        let model = k[dfd + 12];
        let images = layers * faces;
        let etc1s = match (model, scheme) {
            (163, 1) => true,
            (166, 0) => false,
            _ => return None,
        };
        let mut slices = Vec::new(); // (image, level, alpha, w, h, offset, len, iframe)
        let (sgd_off, sgd_len) = (le(k, 64, 8), le(k, 72, 8));
        let sgd = &k[sgd_off..sgd_off + sgd_len];
        // Texture video: KTX2 flags P-frames per image, .basis flags I-frames per slice.
        let mut video = false;
        for l in 0..levels {
            let (off, len) = (le(k, 80 + l * 24, 8), le(k, 88 + l * 24, 8));
            let (lw, lh) = ((w >> l).max(1), (h >> l).max(1));
            for i in 0..images {
                if etc1s {
                    let d = 20 + (l * images + i) * 20;
                    let p_frame = le(sgd, d, 4) & 2 != 0;
                    video |= p_frame;
                    slices.push((
                        i,
                        l,
                        false,
                        lw,
                        lh,
                        off + le(sgd, d + 4, 4),
                        le(sgd, d + 8, 4),
                        !p_frame,
                    ));
                    if le(sgd, d + 16, 4) != 0 {
                        slices.push((
                            i,
                            l,
                            true,
                            lw,
                            lh,
                            off + le(sgd, d + 12, 4),
                            le(sgd, d + 16, 4),
                            !p_frame,
                        ));
                    }
                } else {
                    let size = len / images;
                    slices.push((i, l, false, lw, lh, off + i * size, size, true));
                }
            }
        }
        let base = HEADER + slices.len() * SLICE;
        let mut d = vec![0u8; HEADER];
        let put = |d: &mut Vec<u8>, p: usize, n: usize, v: usize| {
            d[p..p + n].copy_from_slice(&v.to_le_bytes()[..n]);
        };
        d[..2].copy_from_slice(b"sB");
        put(&mut d, 2, 2, 0x13);
        put(&mut d, 4, 2, HEADER);
        put(&mut d, 14, 3, slices.len());
        put(&mut d, 17, 3, images);
        d[20] = u8::from(!etc1s);
        let alpha = slices.iter().any(|s| s.2) || (!etc1s && matches!(k[dfd + 28 + 3] & 15, 3 | 5));
        put(
            &mut d,
            21,
            2,
            usize::from(etc1s) | if alpha { 4 } else { 0 },
        );
        d[23] = if video {
            3
        } else if faces == 6 {
            2
        } else {
            1
        };
        if etc1s {
            let (ne, ns) = (le(sgd, 0, 2), le(sgd, 2, 2));
            let (le_, ls, lt) = (le(sgd, 4, 4), le(sgd, 8, 4), le(sgd, 12, 4));
            let at = sgd_off + 20 + levels * images * 20;
            put(&mut d, 39, 2, ne);
            put(&mut d, 41, 4, base + at);
            put(&mut d, 45, 3, le_);
            put(&mut d, 48, 2, ns);
            put(&mut d, 50, 4, base + at + le_);
            put(&mut d, 54, 3, ls);
            put(&mut d, 57, 4, base + at + le_ + ls);
            put(&mut d, 61, 4, lt);
        }
        put(&mut d, 65, 4, HEADER);
        for &(i, l, a, sw, sh, off, len, iframe) in &slices {
            let mut e = vec![0u8; SLICE];
            put(&mut e, 0, 3, i);
            e[3] = l as u8;
            e[4] = u8::from(a) | if video && iframe { 2 } else { 0 };
            put(&mut e, 5, 2, sw);
            put(&mut e, 7, 2, sh);
            put(&mut e, 9, 2, sw.div_ceil(4));
            put(&mut e, 11, 2, sh.div_ceil(4));
            put(&mut e, 13, 4, base + off);
            put(&mut e, 17, 4, len);
            d.extend_from_slice(&e);
        }
        d.extend_from_slice(k);
        Some(d)
    }

    /// Every Basis KTX2 of the corpus with basisu references (tools/corpus/texture/basis.sh),
    /// repacked as `.basis`: the top level of each paged image must match basisu bit for bit.
    #[test]
    fn repacked_ktx2_matches_basisu() {
        let Some(root) = crate::test_corpus("") else {
            return;
        };
        let Ok(dir) = std::fs::read_dir(root.join("basis/ref")) else {
            return;
        };
        let mut srcs: Vec<_> = dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "src"))
            .collect();
        srcs.sort();
        let (mut files, mut images, mut bad) = (0, 0, Vec::new());
        for p in srcs {
            let stem = p.file_stem().unwrap().to_str().unwrap().to_owned();
            let src = std::fs::read_to_string(&p).unwrap();
            let k = std::fs::read(root.join(src.trim())).unwrap();
            let Some(b) = ktx2_to_basis(&k) else { continue };
            let mut t = match Texture::open(&b) {
                Ok(t) => t,
                Err(e) => {
                    bad.push(format!("{stem}: {e}"));
                    continue;
                }
            };
            files += 1;
            for i in 0..t.image_count() {
                t.select_image(i);
                let (a, f, _) = t.image_position();
                let r = root.join(format!("basis/ref/{stem}.l0.a{a}.f{f}.rgba"));
                let Ok(want) = std::fs::read(&r) else {
                    continue;
                };
                let got = t.decode(&b, &DecodeOptions::default()).unwrap();
                images += 1;
                if got.data != want {
                    bad.push(format!("{stem} image {i}"));
                }
            }
        }
        eprintln!(
            "basis repack: {files} files, {images} images, {} bad",
            bad.len()
        );
        assert!(bad.is_empty(), "{bad:#?}");
    }
}

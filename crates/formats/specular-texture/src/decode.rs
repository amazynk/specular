//! Level selection, block/pixel decode into RGBA8 (parallel by block rows), and the residual
//! box filter.

use std::borrow::Cow;
use std::io::Read;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::{
    DecodeOptions, Decoded, Error, Format, Layout, MAX_OUTPUT_BYTES, Region, Result,
    Supercompression, Texture, astc, basis, bcn, etc, pvrtc,
};

// ---------------------------------------------------------------------------------------------
// Lookup tables
// ---------------------------------------------------------------------------------------------

fn half_to_f32(h: u16) -> f32 {
    let neg = h & 0x8000 != 0;
    let e = u32::from((h >> 10) & 0x1f);
    let m = u32::from(h & 0x3ff);
    let v = match e {
        0 => m as f32 * (1.0 / 16_777_216.0),
        31 => {
            if m == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
        _ => f32::from_bits(((e + 112) << 23) | (m << 13)),
    };
    if neg { -v } else { v }
}

/// Linear light (any float) → sRGB-encoded [0, 1]; NaN and negatives → 0.
fn lin_to_srgb(v: f32) -> f32 {
    if v.is_nan() || v <= 0.0 {
        0.0
    } else if v >= 1.0 {
        1.0
    } else if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Half float bits → sRGB 8-bit (colour channels).
fn half_srgb() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        (0..=u16::MAX)
            .map(|h| (lin_to_srgb(half_to_f32(h)) * 255.0 + 0.5) as u8)
            .collect()
    })
}

/// Half float bits → 8-bit alpha (clamped, linear).
fn half_alpha() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| (0..=u16::MAX).map(|h| unit8(half_to_f32(h))).collect())
}

/// 16-bit linear → sRGB 8-bit.
fn lin16_srgb() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        (0..=u16::MAX)
            .map(|v| (lin_to_srgb(f32::from(v) / 65535.0) * 255.0 + 0.5) as u8)
            .collect()
    })
}

/// sRGB 8-bit → 16-bit linear.
fn srgb_lin16() -> &'static [u16; 256] {
    static T: OnceLock<[u16; 256]> = OnceLock::new();
    T.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.0;
            let l = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            (l * 65535.0 + 0.5) as u16
        })
    })
}

/// Clamp to [0, 1] and quantise (NaN → 0).
fn unit8(v: f32) -> u8 {
    if v.is_nan() {
        0
    } else {
        (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    }
}

fn f32_srgb(v: f32) -> u8 {
    let i = if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0) * 65535.0 + 0.5
    };
    lin16_srgb()[i as usize]
}

fn snorm8(v: u8) -> u8 {
    let s = i32::from((v as i8).max(-127));
    (((s + 127) * 255 + 127) / 254) as u8
}

// ---------------------------------------------------------------------------------------------
// Pixel formats
// ---------------------------------------------------------------------------------------------

/// Expand 1–4 channel values into RGBA (1 → grey, 2 → (R, G, 0), 3 → opaque RGB).
#[inline]
fn put(dst: &mut [u8], c: &[u8]) {
    let px = match *c {
        [l] => [l, l, l, 255],
        [r, g] => [r, g, 0, 255],
        [r, g, b] => [r, g, b, 255],
        [r, g, b, a, ..] => [r, g, b, a],
        [] => [0, 0, 0, 255],
    };
    dst[..4].copy_from_slice(&px);
}

/// Convert `dst.len() / 4` pixels of `src` into RGBA8.
fn convert_row(f: Format, src: &[u8], dst: &mut [u8]) {
    let n = dst.len() / 4;
    match f {
        Format::Masked { bytes, masks, grey } => {
            masked_row(bytes, masks, grey, &src[..n * bytes as usize], dst)
        }
        Format::Unorm16 { channels } => {
            let ch = channels as usize;
            let mut c = [0u8; 4];
            for (s, d) in src.chunks_exact(2 * ch).zip(dst.chunks_exact_mut(4)) {
                for (k, v) in s.chunks_exact(2).enumerate() {
                    let v = u32::from(u16::from_le_bytes([v[0], v[1]]));
                    c[k] = ((v * 255 + 32767) / 65535) as u8;
                }
                put(d, &c[..ch]);
            }
        }
        Format::Snorm8 { channels } => {
            let ch = channels as usize;
            let mut c = [0u8; 4];
            for (s, d) in src.chunks_exact(ch).zip(dst.chunks_exact_mut(4)) {
                for (k, &v) in s.iter().enumerate() {
                    c[k] = snorm8(v);
                }
                put(d, &c[..ch]);
            }
        }
        Format::Half { channels } => {
            let (hs, ha) = (half_srgb(), half_alpha());
            let ch = channels as usize;
            let mut c = [0u8; 4];
            for (s, d) in src.chunks_exact(2 * ch).zip(dst.chunks_exact_mut(4)) {
                for (k, v) in s.chunks_exact(2).enumerate() {
                    let h = u16::from_le_bytes([v[0], v[1]]) as usize;
                    c[k] = if k == 3 { ha[h] } else { hs[h] };
                }
                put(d, &c[..ch]);
            }
        }
        Format::Float { channels } => {
            let ch = channels as usize;
            let mut c = [0u8; 4];
            for (s, d) in src.chunks_exact(4 * ch).zip(dst.chunks_exact_mut(4)) {
                for (k, v) in s.chunks_exact(4).enumerate() {
                    let x = f32::from_le_bytes([v[0], v[1], v[2], v[3]]);
                    c[k] = if k == 3 { unit8(x) } else { f32_srgb(x) };
                }
                put(d, &c[..ch]);
            }
        }
        Format::R11G11B10F => {
            let hs = half_srgb();
            for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                // Same exponent bias as half; 6- and 5-bit mantissas, no sign.
                let r = ((v & 0x7ff) << 4) as usize;
                let g = (((v >> 11) & 0x7ff) << 4) as usize;
                let b = (((v >> 22) & 0x3ff) << 5) as usize;
                d.copy_from_slice(&[hs[r], hs[g], hs[b], 255]);
            }
        }
        Format::Rgb9e5 => {
            for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                let v = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                let scale = f32::from_bits(((v >> 27) + 127 - 15 - 9) << 23);
                let c = |m: u32| f32_srgb((m & 0x1ff) as f32 * scale);
                d.copy_from_slice(&[c(v), c(v >> 9), c(v >> 18), 255]);
            }
        }
        // Block formats never reach here.
        _ => dst.fill(0),
    }
}

fn masked_row(bytes: u8, masks: [u32; 4], grey: bool, src: &[u8], dst: &mut [u8]) {
    const RGBA: [u32; 4] = [0xff, 0xff00, 0xff_0000, 0xff00_0000];
    const BGRA: [u32; 4] = [0xff_0000, 0xff00, 0xff, 0xff00_0000];
    match (bytes, masks, grey) {
        (4, RGBA, false) => dst.copy_from_slice(src),
        (4, [0xff, 0xff00, 0xff_0000, 0], false) => {
            for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                d.copy_from_slice(&[s[0], s[1], s[2], 255]);
            }
        }
        (4, BGRA, false) => {
            for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
            }
        }
        (4, [0xff_0000, 0xff00, 0xff, 0], false) => {
            for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
                d.copy_from_slice(&[s[2], s[1], s[0], 255]);
            }
        }
        (3, [0xff, 0xff00, 0xff_0000, 0], false) => {
            for (s, d) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
                d.copy_from_slice(&[s[0], s[1], s[2], 255]);
            }
        }
        (3, [0xff_0000, 0xff00, 0xff, 0], false) => {
            for (s, d) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)) {
                d.copy_from_slice(&[s[2], s[1], s[0], 255]);
            }
        }
        (1, [0xff, 0, 0, 0], true) => {
            for (&l, d) in src.iter().zip(dst.chunks_exact_mut(4)) {
                d.copy_from_slice(&[l, l, l, 255]);
            }
        }
        _ => {
            // Generic: shift and rescale each mask to 8 bits.
            let ch: [(u32, u32, u64); 4] = masks.map(|m| {
                if m == 0 {
                    (0, 0, 0)
                } else {
                    let sh = m.trailing_zeros();
                    (m, sh, u64::from(m >> sh))
                }
            });
            let get = |v: u32, k: usize, default: u8| -> u8 {
                let (m, sh, max) = ch[k];
                if m == 0 {
                    default
                } else {
                    let x = u64::from((v & m) >> sh);
                    ((x * 255 + max / 2) / max) as u8
                }
            };
            let b = bytes as usize;
            for (s, d) in src.chunks_exact(b).zip(dst.chunks_exact_mut(4)) {
                let mut le = [0u8; 4];
                le[..b].copy_from_slice(s);
                let v = u32::from_le_bytes(le);
                let a = get(v, 3, 255);
                let px = if grey {
                    let l = get(v, 0, 0);
                    [l, l, l, a]
                } else {
                    [get(v, 0, 0), get(v, 1, 0), get(v, 2, 0), a]
                };
                d.copy_from_slice(&px);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------------------------

/// Largest block footprint (ASTC 12×12).
const MAX_BLOCK: usize = 12;

/// Visit the texels of a bw×bh block at the start of `out` (row pitch `pitch`): raster index
/// and RGBA8 slot.
#[inline]
fn each_texel(
    out: &mut [u8],
    pitch: usize,
    bw: usize,
    bh: usize,
    put: &mut dyn FnMut(usize, &mut [u8]),
) {
    for y in 0..bh {
        for x in 0..bw {
            let o = y * pitch + x * 4;
            put(y * bw + x, &mut out[o..o + 4]);
        }
    }
}

/// Decode one block of `f` into RGBA8 at the start of `out`, row pitch `pitch` bytes. `srgb`
/// selects the ASTC LDR sRGB decode; `z` is the slice of a 3D ASTC block to give.
fn decode_block(f: Format, srgb: bool, src: &[u8], z: usize, out: &mut [u8], pitch: usize) {
    let (bw, bh, _) = f.block();
    let (bw, bh) = (bw as usize, bh as usize);
    let each =
        |out: &mut [u8], put: &mut dyn FnMut(usize, &mut [u8])| each_texel(out, pitch, bw, bh, put);
    match f {
        Format::Bc1 { alpha } => {
            bcn::bc1(src, out, pitch);
            if !alpha {
                each(out, &mut |_, p| p[3] = 255);
            }
        }
        Format::Bc2 => bcn::bc2(src, out, pitch),
        Format::Bc3 => bcn::bc3(src, out, pitch),
        Format::Bc7 => bcn::bc7(src, out, pitch),
        Format::Bc4 { signed } => {
            let mut t = [0u8; 16];
            bcn::bc4(src, &mut t, 4, signed);
            each(out, &mut |i, p| {
                let l = if signed { snorm8(t[i]) } else { t[i] };
                p.copy_from_slice(&[l, l, l, 255]);
            });
        }
        Format::Bc5 { signed } => {
            let mut t = [0u8; 32];
            bcn::bc5(src, &mut t, 8, signed);
            each(out, &mut |i, p| {
                let (r, g) = (t[2 * i], t[2 * i + 1]);
                let (r, g) = if signed {
                    (snorm8(r), snorm8(g))
                } else {
                    (r, g)
                };
                p.copy_from_slice(&[r, g, 0, 255]);
            });
        }
        Format::Bc6h { signed } => {
            let mut t = [0u16; 48];
            bcn::bc6h_half(src, &mut t, 12, signed);
            let hs = half_srgb();
            each(out, &mut |i, p| {
                let v = &t[3 * i..3 * i + 3];
                p.copy_from_slice(&[hs[v[0] as usize], hs[v[1] as usize], hs[v[2] as usize], 255]);
            });
        }
        Format::Etc1 | Format::Etc2Rgb => etc::etc2_rgb(src, false, out, pitch),
        Format::Etc2Rgba1 => etc::etc2_rgb(src, true, out, pitch),
        Format::Etc2Rgba => {
            etc::etc2_rgb(&src[8..16], false, out, pitch);
            etc::eac_alpha(&src[..8], out, pitch);
        }
        Format::EacR11 { signed } => {
            let v = etc::eac11(src, signed);
            each(out, &mut |i, p| {
                let l = etc::eac_to_8(v[i], signed);
                p.copy_from_slice(&[l, l, l, 255]);
            });
        }
        Format::EacRg11 { signed } => {
            let (r, g) = (
                etc::eac11(&src[..8], signed),
                etc::eac11(&src[8..16], signed),
            );
            each(out, &mut |i, p| {
                p.copy_from_slice(&[
                    etc::eac_to_8(r[i], signed),
                    etc::eac_to_8(g[i], signed),
                    0,
                    255,
                ])
            });
        }
        Format::Astc { hdr: false, d, .. } => {
            astc::decode_ldr(src, bw, bh, usize::from(d), z, srgb, out, pitch);
        }
        Format::Astc { hdr: true, d, .. } => {
            let mut t = [0u16; MAX_BLOCK * MAX_BLOCK * 4];
            astc::decode_hdr(src, (bw, bh, usize::from(d)), z, &mut t, bw * 4);
            let (hs, ha) = (half_srgb(), half_alpha());
            each(out, &mut |i, p| {
                let v = &t[4 * i..4 * i + 4];
                p.copy_from_slice(&[
                    hs[v[0] as usize],
                    hs[v[1] as usize],
                    hs[v[2] as usize],
                    ha[v[3] as usize],
                ]);
            });
        }
        Format::Etc1s { alpha, rg } => {
            basis::etc1s::decode_block(src, alpha, out, pitch);
            if rg {
                each(out, &mut |_, p| p.copy_from_slice(&[p[0], p[3], 0, 255]));
            }
        }
        Format::Uastc { alpha, rg } => {
            basis::uastc::decode_block(src, out, pitch);
            if rg {
                each(out, &mut |_, p| p.copy_from_slice(&[p[0], p[3], 0, 255]));
            } else if !alpha {
                each(out, &mut |_, p| p[3] = 255);
            }
        }
        _ => each(out, &mut |_, p| p.fill(0)),
    }
}

/// Decodes a bare array of BCn blocks (BC1–BC7, rows of blocks covering `width`×`height`
/// rounded up to whole blocks) to tightly packed RGBA8, cropped to `width`×`height`, block rows
/// in parallel. Conversions as for whole textures (BC4 as grey, BC6H clamped and sRGB
/// encoded). For texture-coded video frames (HAP), which carry no container header.
pub fn decode_blocks(f: Format, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    if !matches!(
        f,
        Format::Bc1 { .. }
            | Format::Bc2
            | Format::Bc3
            | Format::Bc4 { .. }
            | Format::Bc5 { .. }
            | Format::Bc6h { .. }
            | Format::Bc7
    ) {
        return Err(Error::Unsupported(format!("{f:?} as bare blocks")));
    }
    if width == 0 || height == 0 || width > crate::MAX_DIMENSION || height > crate::MAX_DIMENSION {
        return Err(Error::TooLarge);
    }
    let (w, h) = (width as usize, height as usize);
    let (bx, by) = (w.div_ceil(4), h.div_ceil(4));
    let bb = f.block().2;
    let need = bx * by * bb;
    if data.len() < need {
        return Err(Error::Truncated("texture blocks"));
    }
    let _held = specular_core::budget::reserve_n(w as u64 * h as u64, 4)?;
    let stride = w * 4;
    let mut out = vec![0u8; stride * h];
    out.par_chunks_mut(stride * 4)
        .enumerate()
        .for_each(|(row, dst)| {
            let rows = dst.len() / stride;
            let mut tmp = [0u8; 16 * 4];
            let src = &data[row * bx * bb..(row + 1) * bx * bb];
            for (i, block) in src.chunks_exact(bb).enumerate() {
                decode_block(f, false, block, 0, &mut tmp, 16);
                let cols = (w - i * 4).min(4);
                for y in 0..rows {
                    let o = y * stride + i * 16;
                    dst[o..o + cols * 4].copy_from_slice(&tmp[y * 16..y * 16 + cols * 4]);
                }
            }
        });
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------------------------

struct Plan {
    /// Output region (scaled coordinates).
    out: Region,
    /// Chosen mip level and its stored size.
    level: usize,
    lw: u32,
    lh: u32,
    /// Virtual level size (ceil(W / 2^k)); the stored level may be one pixel smaller.
    vw: u32,
    vh: u32,
    /// Residual box factor on top of the level.
    rf: u32,
    /// Stored rectangle decoded from the level.
    tx0: u32,
    tx1: u32,
    ty0: u32,
    ty1: u32,
}

fn plan(t: &Texture, o: &DecodeOptions) -> Result<Plan> {
    let s = o.scale.max(1);
    let (ow, oh) = (t.width.div_ceil(s), t.height.div_ceil(s));
    let out = o.region.unwrap_or(Region {
        x: 0,
        y: 0,
        w: ow,
        h: oh,
    });
    if out.w == 0
        || out.h == 0
        || u64::from(out.x) + u64::from(out.w) > u64::from(ow)
        || u64::from(out.y) + u64::from(out.h) > u64::from(oh)
    {
        return Err(Error::BadRegion);
    }
    if u64::from(out.w) * u64::from(out.h) * 4 > MAX_OUTPUT_BYTES {
        return Err(Error::TooLarge);
    }
    // Largest usable mip level whose factor divides the scale.
    let mut level = 0;
    for k in (1..t.levels.len().min(32)).rev() {
        let f = 1u32 << k;
        if !s.is_multiple_of(f) {
            continue;
        }
        let l = &t.levels[k];
        if l.width.abs_diff(t.width.div_ceil(f)) <= 1
            && l.height.abs_diff(t.height.div_ceil(f)) <= 1
        {
            level = k;
            break;
        }
    }
    let l = &t.levels[level];
    let (vw, vh) = (t.width.div_ceil(1 << level), t.height.div_ceil(1 << level));
    let rf = s >> level;
    let (lw, lh) = (l.width.max(1), l.height.max(1));
    let span = |o0: u32, n: u32, v: u32, stored: u32| {
        let s0 = (u64::from(o0) * u64::from(rf)).min(u64::from(v)) as u32;
        let s1 = ((u64::from(o0) + u64::from(n)) * u64::from(rf)).min(u64::from(v)) as u32;
        let t0 = s0.min(stored - 1);
        let t1 = s1.min(stored).max(t0 + 1);
        (t0, t1)
    };
    let (tx0, tx1) = span(out.x, out.w, vw, lw);
    let (ty0, ty1) = span(out.y, out.h, vh, lh);
    if u64::from(tx1 - tx0) * u64::from(ty1 - ty0) * 4 > MAX_OUTPUT_BYTES {
        return Err(Error::TooLarge);
    }
    Ok(Plan {
        out,
        level,
        lw,
        lh,
        vw,
        vh,
        rf,
        tx0,
        tx1,
        ty0,
        ty1,
    })
}

pub(crate) fn memory_estimate(t: &Texture, o: &DecodeOptions) -> u64 {
    let Ok(p) = plan(t, o) else { return 0 };
    let out = u64::from(p.out.w) * u64::from(p.out.h) * 4;
    let buf = u64::from(p.tx1 - p.tx0) * u64::from(p.ty1 - p.ty0) * 4;
    let level = if t.supercompression == Supercompression::None {
        0
    } else if matches!(
        t.supercompression,
        Supercompression::BasisLz | Supercompression::Basis2(_)
    ) {
        // One transcoded image (a P-frame also holds the frame before it).
        t.image_size(p.lw, p.lh).saturating_mul(2)
    } else {
        t.image_size(p.lw, p.lh)
            .saturating_mul(t.image_in_level(p.level).saturating_add(1))
    };
    let buf = if p.rf == 1 && p.tx1 - p.tx0 == p.out.w && p.ty1 - p.ty0 == p.out.h {
        0
    } else {
        buf
    };
    out.saturating_add(buf).saturating_add(level)
}

/// Bytes of the selected image of a level: borrowed, or decompressed (reserved in `res` from
/// the memory budget); and whether some are missing.
fn level_bytes<'a>(
    t: &Texture,
    data: &'a [u8],
    p: &Plan,
    res: &mut specular_core::Reservation,
) -> Result<(Cow<'a, [u8]>, bool)> {
    let l = &t.levels[p.level];
    let want = t.image_size(p.lw, p.lh);
    let index = t.image_in_level(p.level);
    if t.supercompression == Supercompression::None {
        let (start, end) = match t.layout {
            Layout::PerLevel => (
                (l.range.start as u64).saturating_add(index.saturating_mul(want)),
                l.range.end as u64,
            ),
            Layout::PerChain { chain } => {
                // Chains (layer, face) are `chain` bytes apart; slices follow in the level.
                let dk = u64::from((t.depth >> p.level).max(1));
                let (lf, slice) = (index / dk, index % dk);
                let s = (l.range.start as u64)
                    .saturating_add(lf.saturating_mul(chain))
                    .saturating_add(slice.saturating_mul(want));
                (s, data.len() as u64)
            }
        };
        let end = end.min(data.len() as u64).min(start.saturating_add(want));
        let s = data
            .get(start as usize..end.max(start) as usize)
            .unwrap_or(&[]);
        return Ok((Cow::Borrowed(s), (s.len() as u64) < want));
    }
    let src = data.get(l.range.clone()).unwrap_or(&[]);
    if let Some(g) = &t.basis {
        if want > MAX_OUTPUT_BYTES {
            return Err(Error::TooLarge);
        }
        // Descriptors run level-major: each level holds layers × faces images.
        let per_level = u64::from(t.layers) * u64::from(t.faces);
        let desc = (p.level as u64 * per_level + index) as usize;
        let (bw, bh, _) = t.format.block();
        let (bx, by) = (p.lw.div_ceil(bw) as usize, p.lh.div_ceil(bh) as usize);
        res.grow(want)?;
        return match g.0.transcode(src, desc, bx, by) {
            Ok(v) => Ok((Cow::Owned(v), false)),
            Err(Error::Truncated(_)) => Ok((Cow::Borrowed(&[]), true)),
            Err(e) => Err(e),
        };
    }
    if let (Supercompression::Basis2(codec), Some(seek)) = (t.supercompression, &t.seek) {
        if want > MAX_OUTPUT_BYTES {
            return Err(Error::TooLarge);
        }
        let per_level = u64::from(t.layers) * u64::from(t.faces);
        let desc = (p.level as u64 * per_level + index) as usize;
        let Some(&(o, n)) = seek.get(desc) else {
            return Ok((Cow::Borrowed(&[]), true));
        };
        let Some(slice) = o.checked_add(n).and_then(|e| src.get(o..e)) else {
            return Ok((Cow::Borrowed(&[]), true));
        };
        res.grow(want)?;
        return match basis::transcode2(codec, slice, p.lw, p.lh, t.format) {
            Ok(v) => Ok((Cow::Owned(v), false)),
            Err(Error::Truncated(_)) => Ok((Cow::Borrowed(&[]), true)),
            Err(e) => Err(e),
        };
    }
    let at = index.saturating_mul(want);
    // Earlier images are inflated too (and dropped): bound that work.
    if at.saturating_add(want) > 4 * MAX_OUTPUT_BYTES {
        return Err(Error::TooLarge);
    }
    let (want, at) = (want as usize, at as usize);
    // Images are inflated from the level start: the selected one ends at `at + want`.
    let need = (at + want).min(l.uncompressed_len);
    let mut out = match t.supercompression {
        Supercompression::Zstd => {
            // Grows with the data actually produced, up to `need`, reserved as it grows.
            let mut out = Vec::new();
            if let Ok(dec) = ruzstd::decoding::StreamingDecoder::new(src) {
                let mut dec = dec.take(need as u64);
                let base = res.bytes();
                loop {
                    let start = out.len();
                    out.resize(start + (start / 2).clamp(1 << 20, 64 << 20), 0);
                    res.grow_to(base + out.len() as u64)?;
                    match dec.read(&mut out[start..]) {
                        Ok(n) if n > 0 => out.truncate(start + n),
                        _ => {
                            out.truncate(start);
                            break;
                        }
                    }
                }
            }
            out
        }
        Supercompression::Zlib => {
            // Deflate expands at most ~1032:1, so a declared length beyond that is not
            // allocated.
            let len = need.min(src.len().saturating_mul(1032).saturating_add(64));
            res.grow(len as u64)?;
            let mut out = vec![0u8; len];
            let (got, _) = zlib_rs::decompress_slice(
                &mut out,
                src,
                zlib_rs::InflateConfig { window_bits: 15 },
            );
            let filled = got.len();
            out.truncate(filled);
            out
        }
        Supercompression::None | Supercompression::BasisLz | Supercompression::Basis2(_) => {
            unreachable!("handled above")
        }
    };
    out.drain(..at.min(out.len()));
    let filled = out.len();
    Ok((Cow::Owned(out), filled < want))
}

fn cancelled(c: Option<&AtomicBool>) -> bool {
    c.is_some_and(|c| c.load(Ordering::Relaxed))
}

// ---------------------------------------------------------------------------------------------
// Decode
// ---------------------------------------------------------------------------------------------

pub(crate) fn decode(t: &Texture, data: &[u8], o: &DecodeOptions) -> Result<Decoded> {
    let p = plan(t, o)?;
    let (tw, th) = ((p.tx1 - p.tx0) as usize, (p.ty1 - p.ty0) as usize);
    // The RGBA output, then the transcoded or inflated level, in the memory budget.
    let mut res = specular_core::budget::reserve_n(tw as u64 * th as u64, 4)?;
    let (level, mut truncated) = level_bytes(t, data, &p, &mut res)?;
    let level: &[u8] = &level;
    if cancelled(o.cancel) {
        return Err(Error::Cancelled);
    }
    let f = t.format;
    let (bw, bh, bb) = f.block();
    let z = t.slice_in_block(p.level);
    let stride = tw * 4;
    let mut buf = vec![0u8; stride * th];
    let missing = AtomicBool::new(false);
    let stop = AtomicBool::new(false);

    // Chunks aligned to block rows in level coordinates, ~16K pixels each.
    let step = bh as usize * (16_384 / (tw * bh as usize).max(1)).max(1);
    let first = (step - p.ty0 as usize % step).min(th);
    let (head, rest) = buf.split_at_mut(first * stride);
    let chunks: Vec<(usize, &mut [u8])> = std::iter::once((p.ty0 as usize, head))
        .chain(
            rest.chunks_mut(step * stride)
                .enumerate()
                .map(|(i, c)| (p.ty0 as usize + first + i * step, c)),
        )
        .filter(|(_, c)| !c.is_empty())
        .collect();

    let work = |(y0, out): (usize, &mut [u8])| {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if cancelled(o.cancel) {
            stop.store(true, Ordering::Relaxed);
            return;
        }
        let rows = out.len() / stride;
        if let Format::Pvrtc { two_bpp, alpha } = f {
            // Texels blend the colours of neighbouring blocks: decode by texel windows.
            let rect = (p.tx0 as usize, y0, p.tx1 as usize, y0 + rows);
            pvrtc::decode_rect(level, p.lw, p.lh, two_bpp, rect, out, stride);
            if !alpha {
                for px in out.chunks_exact_mut(4) {
                    px[3] = 255;
                }
            }
        } else if f.is_compressed() {
            let (bw, bh) = (bw as usize, bh as usize);
            let per_row = (p.lw as usize).div_ceil(bw);
            let (tx0, tx1) = (p.tx0 as usize, p.tx1 as usize);
            let mut px = [0u8; MAX_BLOCK * MAX_BLOCK * 4];
            let pitch = bw * 4;
            for by in y0 / bh..=(y0 + rows - 1) / bh {
                for bx in tx0 / bw..tx1.div_ceil(bw) {
                    let off = (by * per_row + bx) * bb;
                    let Some(src) = level.get(off..off + bb) else {
                        missing.store(true, Ordering::Relaxed);
                        continue;
                    };
                    let xs = (bx * bw).max(tx0);
                    let xe = (bx * bw + bw).min(tx1);
                    // Whole block inside the chunk: decode straight into the output.
                    if xs == bx * bw && xe == xs + bw && by * bh >= y0 && by * bh + bh <= y0 + rows
                    {
                        let d = (by * bh - y0) * stride + (xs - tx0) * 4;
                        decode_block(f, t.srgb, src, z, &mut out[d..], stride);
                        continue;
                    }
                    decode_block(f, t.srgb, src, z, &mut px, pitch);
                    for ry in 0..bh {
                        let y = by * bh + ry;
                        if y < y0 || y >= y0 + rows {
                            continue;
                        }
                        let s = ry * pitch + (xs - bx * bw) * 4;
                        let d = (y - y0) * stride + (xs - tx0) * 4;
                        out[d..d + (xe - xs) * 4].copy_from_slice(&px[s..s + (xe - xs) * 4]);
                    }
                }
            }
        } else {
            let row_bytes = t.row_bytes(p.lw) as usize;
            for (r, dst) in out.chunks_exact_mut(stride).enumerate() {
                let start = (y0 + r) * row_bytes + p.tx0 as usize * bb;
                let avail = level.len().saturating_sub(start) / bb;
                let n = avail.min(tw);
                if n < tw {
                    missing.store(true, Ordering::Relaxed);
                }
                if n > 0 {
                    convert_row(f, &level[start..start + n * bb], &mut dst[..n * 4]);
                }
            }
        }
        if t.premultiplied {
            unpremultiply(out);
        }
    };
    if o.parallel && chunks.len() > 1 {
        chunks.into_par_iter().for_each(work);
    } else {
        chunks.into_iter().for_each(work);
    }
    if stop.load(Ordering::Relaxed) || cancelled(o.cancel) {
        return Err(Error::Cancelled);
    }
    truncated |= missing.load(Ordering::Relaxed);

    let (ow, oh) = (p.out.w as usize, p.out.h as usize);
    if p.rf == 1 && tw == ow && th == oh {
        return Ok(Decoded {
            width: p.out.w,
            height: p.out.h,
            origin: (p.out.x, p.out.y),
            stride,
            data: buf,
            truncated,
        });
    }
    let data = resample(&buf, stride, &p, o)?;
    Ok(Decoded {
        width: p.out.w,
        height: p.out.h,
        origin: (p.out.x, p.out.y),
        stride: ow * 4,
        data,
        truncated,
    })
}

/// Premultiplied RGBA8 to straight, in place: colour × 255 / alpha, rounded and clamped (a
/// lossy encoder can push colour above alpha). Transparent pixels keep their (zero) colour.
fn unpremultiply(px: &mut [u8]) {
    for p in px.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

/// Map the decoded stored rectangle to the output region: clamp to the stored edge, and box
/// filter by the residual factor in linear light, alpha weighted.
fn resample(buf: &[u8], stride: usize, p: &Plan, o: &DecodeOptions) -> Result<Vec<u8>> {
    let (ow, oh) = (p.out.w as usize, p.out.h as usize);
    let rf = p.rf as usize;
    let mut out = vec![0u8; ow * oh * 4];
    // Source columns (virtual) for each output column, as stored offsets in `buf`.
    let col = |v: usize, lw: u32, t0: u32| v.min(lw as usize - 1) - t0 as usize;
    let (to_lin, to_srgb) = (srgb_lin16(), lin16_srgb());
    let stop = AtomicBool::new(false);
    let row = |(oy, dst): (usize, &mut [u8])| {
        if oy % 16 == 0 && cancelled(o.cancel) {
            stop.store(true, Ordering::Relaxed);
        }
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let vy0 = (p.out.y as usize + oy) * rf;
        let vy1 = (vy0 + rf).min(p.vh as usize);
        for (ox, d) in dst.chunks_exact_mut(4).enumerate() {
            let vx0 = (p.out.x as usize + ox) * rf;
            let vx1 = (vx0 + rf).min(p.vw as usize);
            if rf == 1 {
                let s = col(vy0, p.lh, p.ty0) * stride + col(vx0, p.lw, p.tx0) * 4;
                d.copy_from_slice(&buf[s..s + 4]);
                continue;
            }
            let (mut sr, mut sg, mut sb, mut sw, mut sa) = (0u64, 0u64, 0u64, 0u64, 0u64);
            let mut n = 0u64;
            for vy in vy0..vy1 {
                let r = col(vy, p.lh, p.ty0) * stride;
                for vx in vx0..vx1 {
                    let s = r + col(vx, p.lw, p.tx0) * 4;
                    let px = &buf[s..s + 4];
                    let w = u64::from(px[3]) * 256 + 1;
                    sr += w * u64::from(to_lin[px[0] as usize]);
                    sg += w * u64::from(to_lin[px[1] as usize]);
                    sb += w * u64::from(to_lin[px[2] as usize]);
                    sw += w;
                    sa += u64::from(px[3]);
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let c = |s: u64| to_srgb[((s + sw / 2) / sw) as usize];
            d.copy_from_slice(&[c(sr), c(sg), c(sb), ((sa + n / 2) / n) as u8]);
        }
    };
    if o.parallel && oh > 1 {
        out.par_chunks_exact_mut(ow * 4).enumerate().for_each(row);
    } else {
        out.chunks_exact_mut(ow * 4).enumerate().for_each(row);
    }
    if stop.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_blocks_crop_and_refuse() {
        // 6×5 in 2×2 BC1 blocks: red, blue / green, white (solid colour0, indices 0).
        let solid = |c: u16| [c as u8, (c >> 8) as u8, 0, 0, 0, 0, 0, 0];
        let data: Vec<u8> = [0xF800, 0x001F, 0x07E0, 0xFFFF]
            .into_iter()
            .flat_map(solid)
            .collect();
        let px = decode_blocks(Format::Bc1 { alpha: false }, &data, 6, 5).unwrap();
        assert_eq!(px.len(), 6 * 5 * 4);
        let at = |x: usize, y: usize| &px[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4];
        assert_eq!(at(0, 0), [255, 0, 0, 255]);
        assert_eq!(at(5, 3), [0, 0, 255, 255]);
        assert_eq!(at(3, 4), [0, 255, 0, 255]);
        assert_eq!(at(5, 4), [255, 255, 255, 255]);
        assert!(matches!(
            decode_blocks(Format::Bc1 { alpha: false }, &data[..24], 6, 5),
            Err(Error::Truncated(_))
        ));
        assert!(decode_blocks(Format::Etc1, &data, 6, 5).is_err());
        assert!(decode_blocks(Format::Bc3, &[], 0, 5).is_err());
    }

    #[test]
    fn half_conversion() {
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0xc000), -2.0);
        assert_eq!(half_to_f32(0x0001), 2f32.powi(-24));
        assert_eq!(half_srgb()[0x3c00], 255);
        assert_eq!(half_srgb()[0x3800], 188); // 0.5 linear
        assert_eq!(half_srgb()[0xbc00], 0);
        assert_eq!(half_alpha()[0x3800], 128);
    }

    #[test]
    fn srgb_roundtrip_is_exact() {
        let (a, b) = (srgb_lin16(), lin16_srgb());
        for i in 0..256 {
            assert_eq!(b[a[i] as usize] as usize, i);
        }
    }

    #[test]
    fn unpremultiply_divides_partial_alpha() {
        let mut px = [64, 32, 0, 128, 9, 9, 9, 0, 200, 100, 50, 255, 90, 10, 0, 64];
        unpremultiply(&mut px);
        // 64·255/128 = 127.5 → 128; colour above alpha (lossy) clamps.
        assert_eq!(
            px,
            [
                128, 64, 0, 128, 9, 9, 9, 0, 200, 100, 50, 255, 255, 40, 0, 64
            ]
        );
    }

    #[test]
    fn snorm() {
        assert_eq!(snorm8(0x81), 0);
        assert_eq!(snorm8(0x80), 0);
        assert_eq!(snorm8(0), 128);
        assert_eq!(snorm8(127), 255);
    }

    #[test]
    fn masked_generic() {
        // RGB565: pure red, green, blue.
        let src = [0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00];
        let mut dst = [0u8; 12];
        masked_row(2, [0xf800, 0x07e0, 0x001f, 0], false, &src, &mut dst);
        assert_eq!(dst, [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
    }
}

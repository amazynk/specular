//! Video YCbCr: how the planes of a [`PixelLayout::Yuv`] raster are coded, and the CPU
//! reference conversion to RGB (the GPU does the same in `convert.wgsl` of `specular-render`;
//! a test there pins the two together).
//!
//! A video frame keeps its decoder's planes (Y full size, Cb and Cr subsampled by the planes'
//! `h_div` / `v_div`) and is converted where it is shown, so playback uploads 1.5 bytes per
//! pixel (3 for 10-bit) instead of 4 or 8, and the CPU does no per-pixel work.

use crate::hdr::{HdrEncoding, Primaries, Transfer};
use crate::{PixelLayout, Raster, RasterPlane};

/// The YCbCr → R'G'B' matrix (H.273 matrix coefficients).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum YuvMatrix {
    /// BT.601 (H.273 MC 5 and 6).
    Bt601,
    /// BT.709 (MC 1).
    Bt709,
    /// BT.2020 / BT.2100 non-constant luminance (MC 9; constant luminance, MC 10, is shown
    /// through the same matrix).
    Bt2020,
    /// SMPTE 240M (MC 7): Kr 0.212, Kb 0.087, nearer BT.709 than BT.601.
    Smpte240,
    /// FCC 73.682 (MC 4): Kr 0.30, Kb 0.11.
    Fcc,
    /// Identity (MC 0): the planes are G, B, R ("RGB" H.264, HEVC, AV1 and VP9 video), each
    /// coded like luma.
    Identity,
    /// YCgCo (MC 8): Cb carries Cg, Cr carries Co.
    YCgCo,
}

impl YuvMatrix {
    /// From an H.273 `matrix_coefficients` code; unspecified or unknown codes (2 and others)
    /// fall back to what players assume: BT.709 for HD and larger (height ≥ 720), BT.601 for
    /// SD.
    pub fn from_h273(code: Option<u16>, height: u32) -> YuvMatrix {
        match code {
            Some(0) => YuvMatrix::Identity,
            Some(1) => YuvMatrix::Bt709,
            Some(4) => YuvMatrix::Fcc,
            Some(5 | 6) => YuvMatrix::Bt601,
            Some(7) => YuvMatrix::Smpte240,
            Some(8) => YuvMatrix::YCgCo,
            Some(9 | 10) => YuvMatrix::Bt2020,
            _ if height >= 720 => YuvMatrix::Bt709,
            _ => YuvMatrix::Bt601,
        }
    }

    /// Luma weights of red and blue (Kr, Kb); BT.601's for identity and YCgCo, whose
    /// [`YuvMatrix::to_rgb`] does not use them.
    pub fn kr_kb(self) -> (f32, f32) {
        match self {
            YuvMatrix::Bt709 => (0.2126, 0.0722),
            YuvMatrix::Bt2020 => (0.2627, 0.0593),
            YuvMatrix::Smpte240 => (0.212, 0.087),
            YuvMatrix::Fcc => (0.30, 0.11),
            YuvMatrix::Bt601 | YuvMatrix::Identity | YuvMatrix::YCgCo => (0.299, 0.114),
        }
    }

    /// Compact code (0..=6) for uniforms and caches; `convert.wgsl` reads the same codes.
    pub fn code(self) -> u32 {
        match self {
            YuvMatrix::Bt601 => 0,
            YuvMatrix::Bt709 => 1,
            YuvMatrix::Bt2020 => 2,
            YuvMatrix::Smpte240 => 3,
            YuvMatrix::Fcc => 4,
            YuvMatrix::Identity => 5,
            YuvMatrix::YCgCo => 6,
        }
    }

    /// The row-major 3×3 matrix taking (Y', Cb, Cr) to R'G'B', with Y' in 0..1 and Cb, Cr
    /// in −½..½ (in 0..1 for [`YuvMatrix::Identity`], whose chroma is coded like luma: see
    /// [`YuvEncoding::levels`]).
    pub fn to_rgb(self) -> [[f32; 3]; 3] {
        match self {
            YuvMatrix::Identity => return [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            YuvMatrix::YCgCo => return [[1.0, -1.0, 1.0], [1.0, 1.0, 0.0], [1.0, -1.0, -1.0]],
            _ => {}
        }
        let (kr, kb) = self.kr_kb();
        let kg = 1.0 - kr - kb;
        let (rv, bu) = (2.0 * (1.0 - kr), 2.0 * (1.0 - kb));
        [
            [1.0, 0.0, rv],
            [1.0, -bu * kb / kg, -rv * kr / kg],
            [1.0, bu, 0.0],
        ]
    }
}

/// How the planes of a [`PixelLayout::Yuv`] raster are coded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct YuvEncoding {
    pub matrix: YuvMatrix,
    /// Full ("PC") range; otherwise limited ("TV"): 16–235 luma and 16–240 chroma at 8 bits.
    pub full_range: bool,
    /// Significant bits per sample, 8 to 16. Above 8 each sample is a native-endian `u16`.
    pub bits: u8,
    /// Subsampled chroma sits on the even luma columns (MPEG-2 / H.264 / HEVC "left", the
    /// video default); otherwise midway between them (JPEG, "centre").
    pub cosited_x: bool,
    /// Vertically subsampled chroma sits on the even luma rows (H.273 chroma sample location
    /// "top-left" or "top", AV1 "colocated"); otherwise midway between them (the default).
    pub cosited_y: bool,
    /// Vertically subsampled chroma sits on the last luma row it covers (H.273 chroma sample
    /// location "bottom-left" or "bottom", 4 and 5); `cosited_y` wins when both are set.
    pub bottom_y: bool,
    /// Horizontally subsampled chroma sits on the last luma column it covers (co-sited
    /// chroma of a picture mirrored or turned by 180°); `cosited_x` wins when both are set.
    pub right_x: bool,
    /// A fourth, full-size plane of straight alpha follows, `bits` wide, full range.
    pub alpha: bool,
    /// Transfer and primaries of the R'G'B' the matrix gives.
    pub colour: HdrEncoding,
}

impl YuvEncoding {
    /// A compact form for caches; [`YuvEncoding::from_bytes`] reads it back.
    pub fn to_bytes(self) -> [u8; 4] {
        let flags = u8::from(self.full_range)
            | u8::from(self.cosited_x) << 1
            | u8::from(self.cosited_y) << 2
            | u8::from(self.bottom_y) << 3
            | u8::from(self.right_x) << 4
            | u8::from(self.alpha) << 5;
        [
            self.matrix.code() as u8,
            flags,
            self.bits,
            self.colour.code(),
        ]
    }

    /// The encoding of [`YuvEncoding::to_bytes`]; `None` for bytes it never writes.
    pub fn from_bytes(b: [u8; 4]) -> Option<YuvEncoding> {
        let matrix = [
            YuvMatrix::Bt601,
            YuvMatrix::Bt709,
            YuvMatrix::Bt2020,
            YuvMatrix::Smpte240,
            YuvMatrix::Fcc,
            YuvMatrix::Identity,
            YuvMatrix::YCgCo,
        ]
        .into_iter()
        .find(|m| m.code() == u32::from(b[0]))?;
        if b[1] >= 64 || !(8..=16).contains(&b[2]) {
            return None;
        }
        let flag = |i: u8| b[1] & (1 << i) != 0;
        Some(YuvEncoding {
            matrix,
            full_range: flag(0),
            cosited_x: flag(1),
            cosited_y: flag(2),
            bottom_y: flag(3),
            right_x: flag(4),
            alpha: flag(5),
            bits: b[2],
            colour: HdrEncoding::from_code(b[3])?,
        })
    }

    /// Bytes per sample: 1 at 8 bits, 2 above.
    pub fn sample_bytes(self) -> usize {
        if self.bits > 8 { 2 } else { 1 }
    }

    /// (offset, scale) mapping a luma sample to 0..1 and (offset, scale) mapping a chroma
    /// sample to −½..½ (to 0..1, like luma, for [`YuvMatrix::Identity`]):
    /// `(v − offset) × scale`.
    pub fn levels(self) -> ([f32; 2], [f32; 2]) {
        let b = self.bits.clamp(8, 16) as i32;
        let unit = (1u32 << (b - 8)) as f32;
        let (luma, chroma) = if self.full_range {
            let max = ((1u32 << b) - 1) as f32;
            ([0.0, 1.0 / max], [128.0 * unit, 1.0 / max])
        } else {
            (
                [16.0 * unit, 1.0 / (219.0 * unit)],
                [128.0 * unit, 1.0 / (224.0 * unit)],
            )
        };
        if self.matrix == YuvMatrix::Identity {
            (luma, luma)
        } else {
            (luma, chroma)
        }
    }

    /// The layout of the R'G'B' this converts to on the CPU ([`to_rgb`]): RGBA8 for 8-bit SDR
    /// sRGB-like video in BT.709 primaries, otherwise 16-bit RGBA with the transfer and
    /// primaries attached.
    pub fn rgb_layout(self) -> PixelLayout {
        if self.bits <= 8 && self.colour == HdrEncoding::SRGB {
            PixelLayout::Rgba
        } else {
            PixelLayout::Rgba16(self.colour)
        }
    }
}

/// The transfer and primaries of H.273 `colour_primaries` / `transfer_characteristics`
/// codes: PQ (16) and HLG (18) are HDR; everything else is shown as SDR (the BT.709 /
/// BT.1886 curve goes to the display as is, like every player does).
pub fn h273_colour(primaries: Option<u16>, transfer: Option<u16>) -> HdrEncoding {
    let transfer = match transfer {
        Some(16) => Transfer::Pq,
        Some(18) => Transfer::Hlg,
        _ => Transfer::Srgb,
    };
    let primaries = match primaries {
        Some(9) => Primaries::Bt2020,
        Some(11 | 12) => Primaries::DisplayP3,
        // HDR signalled without primaries is BT.2100.
        None | Some(2) if transfer != Transfer::Srgb => Primaries::Bt2020,
        _ => Primaries::Bt709,
    };
    HdrEncoding {
        transfer,
        primaries,
    }
}

/// Where along one axis a chroma sample sits for output pixel `i`: the two samples either
/// side (clamped to the plane's `n`) and the weight of the second. `div` is the plane's
/// subsampling on the axis; `site` the offset of chroma sample 0 from luma sample 0 in luma
/// samples (`(div − 1) / 2` centred, 0 co-sited, `div − 1` on the last luma sample).
fn tap(i: usize, n: u32, div: u8, site: f32) -> (usize, usize, f32) {
    let last = n.saturating_sub(1) as usize;
    if div <= 1 {
        let c = i.min(last);
        return (c, c, 0.0);
    }
    let f = ((i as f32 - site) / f32::from(div)).max(0.0);
    let c0 = f.floor() as usize;
    let w = f - c0 as f32;
    if c0 >= last {
        (last, last, 0.0)
    } else {
        (c0, c0 + 1, w)
    }
}

impl YuvEncoding {
    /// Offsets of chroma sample 0 from luma sample 0, in luma samples, for chroma
    /// subsampled by `h_div` × `v_div`: the siting [`to_rgb`] and the shader interpolate at.
    pub fn chroma_site(self, h_div: u8, v_div: u8) -> (f32, f32) {
        let (hd, vd) = (f32::from(h_div.max(1)), f32::from(v_div.max(1)));
        let x = if self.cosited_x {
            0.0
        } else if self.right_x {
            hd - 1.0
        } else {
            (hd - 1.0) / 2.0
        };
        let y = if self.cosited_y {
            0.0
        } else if self.bottom_y {
            vd - 1.0
        } else {
            (vd - 1.0) / 2.0
        };
        (x, y)
    }
}

/// Samples of one row of a plane as `f32` codes: the first `out.len()` (at most the plane's
/// width; the rest of `out` is left alone).
fn row_codes(p: &RasterPlane, wide: bool, y: usize, out: &mut [f32]) {
    let y = y.min(p.height.saturating_sub(1) as usize);
    let n = (p.width as usize).min(out.len());
    let start = y * p.stride;
    let out = &mut out[..n];
    if wide {
        let row = p.data.get(start..start + 2 * n).unwrap_or(&[]);
        for (o, b) in out.iter_mut().zip(row.as_chunks::<2>().0) {
            *o = f32::from(u16::from_ne_bytes(*b));
        }
    } else {
        let row = p.data.get(start..start + n).unwrap_or(&[]);
        for (o, &b) in out.iter_mut().zip(row) {
            *o = f32::from(b);
        }
    }
}

/// How one chroma plane is spread across the output columns.
enum Across {
    /// One chroma sample per output column.
    Same,
    /// Two output columns per sample: column 2k is `p` of sample k − 1 and 1 − `p` of sample
    /// k, column 2k + 1 is 1 − `q` of sample k and `q` of sample k + 1 (centred siting ¼ and
    /// ¼, co-sited 0 and ½, on the right ½ and 0).
    Two { p: f32, q: f32 },
    /// Any other subsampling: the taps of each output column.
    Taps(Vec<(usize, usize, f32)>),
}

/// One chroma plane's taps across the output columns and down the output rows.
struct ChromaTaps {
    across: Across,
    y: Vec<(usize, usize, f32)>,
    /// Chroma columns the output reads. The padded rows hold these plus one either side:
    /// sample −1 and samples past the plane's last repeat its edge (the clamp of [`tap`]).
    need: usize,
}

impl ChromaTaps {
    fn new(p: &RasterPlane, e: YuvEncoding, w: usize, h: usize) -> ChromaTaps {
        let (sx, sy) = e.chroma_site(p.h_div, p.v_div);
        let (across, need) = match p.h_div {
            0 | 1 => (Across::Same, w),
            2 => (
                Across::Two {
                    p: sx / 2.0,
                    q: (1.0 - sx) / 2.0,
                },
                w.div_ceil(2),
            ),
            _ => (
                Across::Taps((0..w).map(|i| tap(i, p.width, p.h_div, sx)).collect()),
                (p.width as usize).max(1),
            ),
        };
        ChromaTaps {
            across,
            y: (0..h).map(|i| tap(i, p.height, p.v_div, sy)).collect(),
            need,
        }
    }

    /// Length of the padded rows [`ChromaTaps::row`] works in.
    fn padded(&self) -> usize {
        self.need + 2
    }

    /// Chroma row `y` of the plane as codes into `buf` (`padded()` long): sample j at j + 1,
    /// the edges repeated.
    fn fill(&self, p: &RasterPlane, wide: bool, y: usize, buf: &mut [f32]) {
        let n = (p.width as usize).clamp(1, self.need + 1);
        row_codes(p, wide, y, &mut buf[1..=n]);
        let last = buf[n];
        buf[n + 1..].fill(last);
        buf[0] = buf[1];
    }

    /// Output row `row` of the plane's interpolated chroma codes: `out` (the output width),
    /// or for [`Across::Same`] a slice of `a`; `a` and `b` are scratch rows `padded()` long.
    fn row<'s>(
        &self,
        p: &RasterPlane,
        wide: bool,
        row: usize,
        a: &'s mut [f32],
        b: &mut [f32],
        out: &'s mut [f32],
    ) -> &'s [f32] {
        let (y0, y1, wy) = self.y[row];
        self.fill(p, wide, y0, a);
        if wy != 0.0 {
            self.fill(p, wide, y1, b);
            for (s, &t) in a.iter_mut().zip(b.iter()) {
                *s += (t - *s) * wy;
            }
        }
        let w = out.len();
        match &self.across {
            Across::Same => &a[1..=w],
            Across::Two { p, q } => {
                upsample2(a, out, *p, *q);
                out
            }
            Across::Taps(t) => {
                let a = &a[1..];
                for (o, &(x0, x1, wx)) in out.iter_mut().zip(t) {
                    let (l, r) = (a[x0], a[x1]);
                    *o = l + (r - l) * wx;
                }
                out
            }
        }
    }
}

type F8 = std::simd::Simd<f32, 8>;
type U8x = std::simd::Simd<u32, 8>;

/// Doubles a padded chroma row across (see [`Across::Two`]; `src[k]` is sample k − 1).
fn upsample2(src: &[f32], out: &mut [f32], p: f32, q: f32) {
    use std::simd::StdFloat;
    let n = out.len().div_ceil(2);
    let mut k = 0;
    let s = F8::splat;
    let (pv, p1, qv, q1) = (s(p), s(1.0 - p), s(q), s(1.0 - q));
    while k + 8 <= n && 2 * k + 16 <= out.len() && k + 10 <= src.len() {
        let a = F8::from_slice(&src[k..k + 8]);
        let b = F8::from_slice(&src[k + 1..k + 9]);
        let c = F8::from_slice(&src[k + 2..k + 10]);
        let (lo, hi) = a.mul_add(pv, b * p1).interleave(c.mul_add(qv, b * q1));
        lo.copy_to_slice(&mut out[2 * k..2 * k + 8]);
        hi.copy_to_slice(&mut out[2 * k + 8..2 * k + 16]);
        k += 8;
    }
    for k in k..n {
        let (a, b, c) = (src[k], src[k + 1], src[k + 2]);
        out[2 * k] = a.mul_add(p, b * (1.0 - p));
        if let Some(o) = out.get_mut(2 * k + 1) {
            *o = c.mul_add(q, b * (1.0 - q));
        }
    }
}

/// Output codes as affine functions of the Y', Cb and Cr codes: channel c is
/// `k[c] · (Y', Cb, Cr) + off[c]`, clamped to 0..=`q`; alpha is its code × `a_scale`.
#[derive(Clone, Copy)]
struct Affine {
    k: [[f32; 3]; 3],
    off: [f32; 3],
    q: f32,
    a_scale: f32,
}

impl Affine {
    fn new(e: YuvEncoding, wide_out: bool) -> Affine {
        let ([yo, ys], [co, cs]) = e.levels();
        let m = e.matrix.to_rgb();
        let q = if wide_out { 65535.0 } else { 255.0 };
        let max = ((1u32 << e.bits.clamp(8, 16)) - 1) as f32;
        let mut k = [[0.0; 3]; 3];
        let mut off = [0.0; 3];
        for c in 0..3 {
            k[c] = [m[c][0] * ys * q, m[c][1] * cs * q, m[c][2] * cs * q];
            off[c] = -(k[c][0] * yo + (k[c][1] + k[c][2]) * co);
        }
        Affine {
            k,
            off,
            q,
            a_scale: q / max,
        }
    }
}

/// Rounds codes already clamped to 0..=65535 to the nearest integer, halves up (as the
/// shader's unorm output and the per-pixel reference): adding 2²³ to the floor leaves the
/// integer in the low mantissa bits, without the range checks of a saturating cast.
#[inline(always)]
fn round_codes(v: F8) -> U8x {
    use std::simd::StdFloat;
    use std::simd::num::SimdFloat;
    ((v + F8::splat(0.5)).floor() + F8::splat(8_388_608.0)).to_bits() & U8x::splat(0xffff)
}

#[inline(always)]
fn round_code(v: f32) -> u32 {
    ((v + 0.5).floor() + 8_388_608.0).to_bits() & 0xffff
}

/// One row of pixels from luma `l`, chroma `u` and `v` and alpha `al` codes (the output width;
/// `al` unread unless `ALPHA`) into RGBA8 or, when `WIDE`, native-endian RGBA16.
fn pixels<const WIDE: bool, const ALPHA: bool>(
    c: &Affine,
    l: &[f32],
    u: &[f32],
    v: &[f32],
    al: &[f32],
    out: &mut [u8],
) {
    use std::simd::StdFloat;
    use std::simd::num::SimdFloat;
    let bpp = if WIDE { 8 } else { 4 };
    let w = out.len() / bpp;
    let (l, u, v) = (&l[..w], &u[..w], &v[..w]);
    let al = if ALPHA { &al[..w] } else { &[][..] };
    let s = F8::splat;
    let (zero, q) = (s(0.0), s(c.q));
    let k = c.k.map(|r| r.map(s));
    let off = c.off.map(s);
    let a_scale = s(c.a_scale);
    let chunks = w / 8;
    for i in 0..chunks {
        let x = i * 8;
        let lv = F8::from_slice(&l[x..x + 8]);
        let uv = F8::from_slice(&u[x..x + 8]);
        let vv = F8::from_slice(&v[x..x + 8]);
        let ch = |r: usize| {
            let t = lv.mul_add(k[r][0], off[r]);
            let t = uv.mul_add(k[r][1], t);
            round_codes(vv.mul_add(k[r][2], t).simd_max(zero).simd_min(q))
        };
        let (r, g, b) = (ch(0), ch(1), ch(2));
        let a = if ALPHA {
            round_codes((F8::from_slice(&al[x..x + 8]) * a_scale).simd_min(q))
        } else {
            U8x::splat(c.q as u32)
        };
        if WIDE {
            use std::simd::num::SimdUint;
            use std::simd::{Simd, ToBytes};
            let [r, g, b, a] = [r, g, b, a].map(|c| c.cast::<u64>());
            #[cfg(target_endian = "little")]
            let px: Simd<u64, 8> = r | g << 16 | b << 32 | a << 48;
            #[cfg(target_endian = "big")]
            let px: Simd<u64, 8> = r << 48 | g << 32 | b << 16 | a;
            px.to_ne_bytes().copy_to_slice(&mut out[x * 8..x * 8 + 64]);
        } else {
            use std::simd::ToBytes;
            let px = r | g << 8 | b << 16 | a << 24;
            px.to_le_bytes().copy_to_slice(&mut out[x * 4..x * 4 + 32]);
        }
    }
    for x in chunks * 8..w {
        let ch = |r: usize| {
            let t = l[x].mul_add(c.k[r][0], c.off[r]);
            let t = u[x].mul_add(c.k[r][1], t);
            round_code(v[x].mul_add(c.k[r][2], t).clamp(0.0, c.q))
        };
        let a = if ALPHA {
            round_code((al[x] * c.a_scale).min(c.q))
        } else {
            c.q as u32
        };
        let px = &mut out[x * bpp..(x + 1) * bpp];
        for (j, val) in [ch(0), ch(1), ch(2), a].into_iter().enumerate() {
            if WIDE {
                px[2 * j..2 * j + 2].copy_from_slice(&(val as u16).to_ne_bytes());
            } else {
                px[j] = val as u8;
            }
        }
    }
}

/// Converts a [`PixelLayout::Yuv`] raster to R'G'B' on the CPU: RGBA8 or native-endian
/// 16-bit RGBA ([`YuvEncoding::rgb_layout`]), straight alpha from the fourth plane when the
/// encoding has one (opaque otherwise). Subsampled chroma is interpolated bilinearly at the
/// encoding's siting, like the shader (`convert.wgsl`, to float rounding): float, eight
/// pixels a step, the levels and matrix folded into one affine map per channel, halved
/// chroma doubled across with fixed weights per siting (other subsampling by taps). Rows
/// run in parallel on the current rayon pool. `None` for other layouts or missing planes.
pub fn to_rgb(r: &Raster) -> Option<Raster> {
    to_rgb_with(r, true)
}

/// [`to_rgb`], on the calling thread only unless `parallel`.
pub fn to_rgb_with(r: &Raster, parallel: bool) -> Option<Raster> {
    let PixelLayout::Yuv(e) = r.layout else {
        return None;
    };
    let [y, cb, cr] = r.planes.get(..3)? else {
        return None;
    };
    let alpha = if e.alpha {
        Some(r.planes.get(3)?)
    } else {
        None
    };
    let layout = e.rgb_layout();
    let wide_out = layout != PixelLayout::Rgba;
    let wide_in = e.bits > 8;
    let (w, h) = (r.width as usize, r.height as usize);
    let bpp = if wide_out { 8 } else { 4 };
    let affine = Affine::new(e, wide_out);
    let tb = ChromaTaps::new(cb, e, w, h);
    let tr = ChromaTaps::new(cr, e, w, h);
    let mut data = vec![0u8; w * h * bpp];
    let kernel = match (wide_out, alpha.is_some()) {
        (false, false) => pixels::<false, false>,
        (false, true) => pixels::<false, true>,
        (true, false) => pixels::<true, false>,
        (true, true) => pixels::<true, true>,
    };
    let convert_row = |row: usize, out: &mut [u8], s: &mut Scratch| {
        row_codes(y, wide_in, row, &mut s.l);
        if let Some(p) = alpha {
            row_codes(p, wide_in, row, &mut s.alpha);
        }
        let u = tb.row(cb, wide_in, row, &mut s.ua, &mut s.ub, &mut s.u);
        let v = tr.row(cr, wide_in, row, &mut s.va, &mut s.vb, &mut s.v);
        kernel(&affine, &s.l, u, v, &s.alpha, out);
    };
    let new_scratch = || Scratch::new(w, alpha.is_some(), tb.padded(), tr.padded());
    let row_bytes = w * bpp;
    // A band of rows per task: small images stay on the calling thread.
    if w == 0 || h == 0 {
        // Nothing to convert.
    } else if parallel && w * h >= 1 << 16 {
        use rayon::prelude::*;
        data.par_chunks_mut(row_bytes * 16)
            .enumerate()
            .for_each_init(new_scratch, |s, (band, rows)| {
                for (i, out) in rows.chunks_exact_mut(row_bytes).enumerate() {
                    convert_row(band * 16 + i, out, s);
                }
            });
    } else {
        let mut s = new_scratch();
        for (row, out) in data.chunks_exact_mut(row_bytes).enumerate() {
            convert_row(row, out, &mut s);
        }
    }
    Some(Raster {
        width: r.width,
        height: r.height,
        origin: r.origin,
        scale: r.scale,
        layout,
        planes: vec![RasterPlane {
            width: r.width,
            height: r.height,
            stride: row_bytes,
            data,
            h_div: 1,
            v_div: 1,
        }],
        truncated: r.truncated,
    })
}

/// One pixel of a [`PixelLayout::Yuv`] raster (raster coordinates) as [`to_rgb`] converts
/// it: R'G'B' 0..1 (clamped) and straight alpha. `None` outside the raster or for another
/// layout.
pub fn pixel(r: &Raster, x: u32, y: u32) -> Option<[f32; 4]> {
    let PixelLayout::Yuv(e) = r.layout else {
        return None;
    };
    if x >= r.width || y >= r.height {
        return None;
    }
    let [yp, cb, cr] = r.planes.get(..3)? else {
        return None;
    };
    let wide = e.bits > 8;
    let code = |p: &RasterPlane, x: usize, y: usize| -> Option<f32> {
        let x = x.min(p.width.saturating_sub(1) as usize);
        let o = y.min(p.height.saturating_sub(1) as usize) * p.stride;
        Some(if wide {
            let b = p.data.get(o + 2 * x..o + 2 * x + 2)?;
            f32::from(u16::from_ne_bytes([b[0], b[1]]))
        } else {
            f32::from(*p.data.get(o + x)?)
        })
    };
    let chroma = |p: &RasterPlane| -> Option<f32> {
        let (ox, oy) = e.chroma_site(p.h_div, p.v_div);
        let (x0, x1, wx) = tap(x as usize, p.width, p.h_div, ox);
        let (y0, y1, wy) = tap(y as usize, p.height, p.v_div, oy);
        let (a, b) = (code(p, x0, y0)?, code(p, x1, y0)?);
        let (c, d) = (code(p, x0, y1)?, code(p, x1, y1)?);
        let top = a + (b - a) * wx;
        let bottom = c + (d - c) * wx;
        Some(top + (bottom - top) * wy)
    };
    let ([yo, ys], [co, cs]) = e.levels();
    let l = (code(yp, x as usize, y as usize)? - yo) * ys;
    let u = (chroma(cb)? - co) * cs;
    let v = (chroma(cr)? - co) * cs;
    let m = e.matrix.to_rgb();
    let c = |k: [f32; 3]| (k[0] * l + k[1] * u + k[2] * v).clamp(0.0, 1.0);
    let a = match (e.alpha, r.planes.get(3)) {
        (true, Some(p)) => code(p, x as usize, y as usize)? / ((1u32 << e.bits) - 1) as f32,
        _ => 1.0,
    };
    Some([c(m[0]), c(m[1]), c(m[2]), a])
}

/// Per-thread rows for [`to_rgb`]: luma, alpha and the two chroma planes interpolated (the
/// output width), and each chroma plane's padded scratch rows.
struct Scratch {
    l: Vec<f32>,
    alpha: Vec<f32>,
    u: Vec<f32>,
    v: Vec<f32>,
    ua: Vec<f32>,
    ub: Vec<f32>,
    va: Vec<f32>,
    vb: Vec<f32>,
}

impl Scratch {
    fn new(w: usize, alpha: bool, pu: usize, pv: usize) -> Scratch {
        Scratch {
            l: vec![0.0; w],
            alpha: vec![0.0; if alpha { w } else { 0 }],
            u: vec![0.0; w],
            v: vec![0.0; w],
            ua: vec![0.0; pu],
            ub: vec![0.0; pu],
            va: vec![0.0; pv],
            vb: vec![0.0; pv],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `bits`-wide sample of `plane` at (x, y), clamped to the plane: the per-pixel
    /// reference of [`to_rgb`].
    fn sample(p: &RasterPlane, wide: bool, x: usize, y: usize) -> f32 {
        let x = x.min(p.width.saturating_sub(1) as usize);
        let y = y.min(p.height.saturating_sub(1) as usize);
        if wide {
            let o = y * p.stride + 2 * x;
            p.data
                .get(o..o + 2)
                .map_or(0.0, |b| u16::from_ne_bytes([b[0], b[1]]) as f32)
        } else {
            p.data.get(y * p.stride + x).map_or(0.0, |&v| v as f32)
        }
    }

    /// A chroma sample at luma position (x, y), bilinearly interpolated with the encoding's
    /// siting, written out like the shader's `yuv_chroma`.
    fn chroma(p: &RasterPlane, e: YuvEncoding, x: usize, y: usize) -> f32 {
        let wide = e.bits > 8;
        let (hd, vd) = (p.h_div.max(1) as f32, p.v_div.max(1) as f32);
        let fx = if p.h_div <= 1 {
            x as f32
        } else if e.cosited_x {
            x as f32 / hd
        } else if e.right_x {
            (x as f32 + 1.0) / hd - 1.0
        } else {
            (x as f32 + 0.5) / hd - 0.5
        };
        let fy = if p.v_div <= 1 {
            y as f32
        } else if e.cosited_y {
            y as f32 / vd
        } else if e.bottom_y {
            (y as f32 + 1.0) / vd - 1.0
        } else {
            (y as f32 + 0.5) / vd - 0.5
        };
        let (fx, fy) = (fx.max(0.0), fy.max(0.0));
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
        let a = sample(p, wide, x0, y0);
        let b = sample(p, wide, x0 + 1, y0);
        let c = sample(p, wide, x0, y0 + 1);
        let d = sample(p, wide, x0 + 1, y0 + 1);
        let top = a + (b - a) * ax;
        let bottom = c + (d - c) * ax;
        top + (bottom - top) * ay
    }

    /// The per-pixel reference conversion of one pixel to 0..1 R'G'B' and alpha.
    fn reference(r: &Raster, x: usize, y: usize) -> [f32; 4] {
        let PixelLayout::Yuv(e) = r.layout else {
            unreachable!()
        };
        let wide = e.bits > 8;
        let ([yo, ys], [co, cs]) = e.levels();
        let m = e.matrix.to_rgb();
        let l = (sample(&r.planes[0], wide, x, y) - yo) * ys;
        let u = (chroma(&r.planes[1], e, x, y) - co) * cs;
        let v = (chroma(&r.planes[2], e, x, y) - co) * cs;
        let max = ((1u32 << e.bits) - 1) as f32;
        let a = if e.alpha {
            sample(&r.planes[3], wide, x, y) / max
        } else {
            1.0
        };
        let c = |k: [f32; 3]| (k[0] * l + k[1] * u + k[2] * v).clamp(0.0, 1.0);
        [c(m[0]), c(m[1]), c(m[2]), a]
    }

    /// The row-wise conversion against the per-pixel reference: every siting (right and
    /// bottom included), 4:2:0, 4:2:2, 4:4:4 and 4:1:1, odd sizes, 8 and 10 bits, alpha, on
    /// both the single-threaded and the banded path.
    #[test]
    fn rows_match_the_per_pixel_reference() {
        let mut seed = 7u32;
        let mut rnd = |n: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 8) % n
        };
        for (w, h) in [(7u32, 5u32), (300, 230)] {
            for (hd, vd) in [(2u8, 2u8), (2, 1), (1, 1), (4, 1)] {
                for bits in [8u8, 10] {
                    for flags in 0..16u32 {
                        let e = YuvEncoding {
                            matrix: YuvMatrix::Bt709,
                            full_range: flags & 1 != 0,
                            bits,
                            cosited_x: flags & 2 != 0,
                            right_x: flags & 4 != 0,
                            cosited_y: flags & 8 != 0,
                            bottom_y: flags & 4 != 0,
                            alpha: flags & 1 != 0,
                            colour: HdrEncoding::SRGB,
                        };
                        let wide = bits > 8;
                        let max = (1u32 << bits) - 1;
                        let mut mk = |pw: u32, ph: u32, hd: u8, vd: u8| {
                            let v: Vec<u16> = (0..pw * ph).map(|_| rnd(max + 1) as u16).collect();
                            RasterPlane {
                                h_div: hd,
                                v_div: vd,
                                ..plane(pw, ph, 1, &v, wide)
                            }
                        };
                        let (cw, ch) = (w.div_ceil(hd as u32), h.div_ceil(vd as u32));
                        let mut planes =
                            vec![mk(w, h, 1, 1), mk(cw, ch, hd, vd), mk(cw, ch, hd, vd)];
                        if e.alpha {
                            planes.push(mk(w, h, 1, 1));
                        }
                        let r = Raster {
                            width: w,
                            height: h,
                            origin: (0, 0),
                            scale: 1,
                            layout: PixelLayout::Yuv(e),
                            planes,
                            truncated: false,
                        };
                        let out = to_rgb(&r).unwrap();
                        let (q, bpp) = if wide { (65535.0, 8) } else { (255.0, 4) };
                        let p = &out.planes[0];
                        for y in 0..h as usize {
                            for x in 0..w as usize {
                                let want = reference(&r, x, y);
                                let one = pixel(&r, x as u32, y as u32).unwrap();
                                for c in 0..4 {
                                    assert!((one[c] - want[c]).abs() < 1e-4, "pixel ({x}, {y})");
                                }
                                let px = &p.data[y * p.stride + x * bpp..][..bpp];
                                for c in 0..4 {
                                    let got = if wide {
                                        u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]) as f32
                                    } else {
                                        px[c] as f32
                                    };
                                    let d = (got - want[c] * q).abs();
                                    assert!(
                                        d <= 0.51 + q * 2e-5,
                                        "{w}×{h} {hd}×{vd} {bits}-bit flags {flags} ({x}, {y}) \
                                         channel {c}: {got} against {}",
                                        want[c] * q
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Wider depths, HDR-labelled 8-bit (16-bit output), every matrix and odd widths around
    /// the 8-pixel vector steps, against the per-pixel reference.
    #[test]
    fn depths_matrices_and_widths_match_the_reference() {
        let mut seed = 11u32;
        let mut rnd = |n: u32| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 8) % n
        };
        let matrices = [
            YuvMatrix::Bt601,
            YuvMatrix::Bt709,
            YuvMatrix::Bt2020,
            YuvMatrix::Smpte240,
            YuvMatrix::Fcc,
            YuvMatrix::Identity,
            YuvMatrix::YCgCo,
        ];
        let pq = h273_colour(Some(9), Some(16));
        let mut case = 0usize;
        for (w, h) in [(1u32, 1u32), (15, 3), (17, 4), (33, 7), (64, 2)] {
            for (hd, vd) in [(2u8, 2u8), (2, 1), (1, 1), (4, 1), (1, 2)] {
                for (bits, colour) in [(8u8, pq), (12, pq), (16, HdrEncoding::SRGB), (9, pq)] {
                    for flags in 0..16u32 {
                        case += 1;
                        let e = YuvEncoding {
                            matrix: matrices[case % matrices.len()],
                            full_range: flags & 1 != 0,
                            bits,
                            cosited_x: flags & 2 != 0,
                            right_x: flags & 4 != 0,
                            cosited_y: flags & 8 != 0,
                            bottom_y: flags & 4 != 0,
                            alpha: flags & 1 != 0,
                            colour,
                        };
                        let wide = bits > 8;
                        let max = (1u32 << bits) - 1;
                        let mut mk = |pw: u32, ph: u32, hd: u8, vd: u8| {
                            let v: Vec<u16> = (0..pw * ph).map(|_| rnd(max + 1) as u16).collect();
                            RasterPlane {
                                h_div: hd,
                                v_div: vd,
                                ..plane(pw, ph, 1, &v, wide)
                            }
                        };
                        let (cw, ch) = (w.div_ceil(hd as u32), h.div_ceil(vd as u32));
                        let mut planes =
                            vec![mk(w, h, 1, 1), mk(cw, ch, hd, vd), mk(cw, ch, hd, vd)];
                        if e.alpha {
                            planes.push(mk(w, h, 1, 1));
                        }
                        let r = Raster {
                            width: w,
                            height: h,
                            origin: (0, 0),
                            scale: 1,
                            layout: PixelLayout::Yuv(e),
                            planes,
                            truncated: false,
                        };
                        let out = to_rgb_with(&r, false).unwrap();
                        assert_eq!(out.layout, PixelLayout::Rgba16(colour));
                        let p = &out.planes[0];
                        for y in 0..h as usize {
                            for x in 0..w as usize {
                                let want = reference(&r, x, y);
                                let px = &p.data[y * p.stride + x * 8..][..8];
                                for c in 0..4 {
                                    let got = u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]);
                                    let d = (f32::from(got) - want[c] * 65535.0).abs();
                                    assert!(
                                        d <= 0.51 + 65535.0 * 2e-5,
                                        "{w}×{h} {hd}×{vd} {bits}-bit {:?} flags {flags} \
                                         ({x}, {y}) channel {c}: {got} against {}",
                                        e.matrix,
                                        want[c] * 65535.0
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn plane(w: u32, h: u32, div: u8, v: &[u16], wide: bool) -> RasterPlane {
        let data: Vec<u8> = if wide {
            v.iter().flat_map(|s| s.to_ne_bytes()).collect()
        } else {
            v.iter().map(|&s| s as u8).collect()
        };
        RasterPlane {
            width: w,
            height: h,
            stride: w as usize * if wide { 2 } else { 1 },
            data,
            h_div: div,
            v_div: div,
        }
    }

    fn raster(e: YuvEncoding, y: u16, cb: u16, cr: u16) -> Raster {
        let wide = e.bits > 8;
        Raster {
            width: 4,
            height: 2,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Yuv(e),
            planes: vec![
                plane(4, 2, 1, &[y; 8], wide),
                plane(2, 1, 2, &[cb; 2], wide),
                plane(2, 1, 2, &[cr; 2], wide),
            ],
            truncated: false,
        }
    }

    fn enc(matrix: YuvMatrix, full_range: bool, bits: u8) -> YuvEncoding {
        YuvEncoding {
            matrix,
            full_range,
            bits,
            cosited_x: true,
            cosited_y: false,
            bottom_y: false,
            right_x: false,
            alpha: false,
            colour: HdrEncoding::SRGB,
        }
    }

    #[test]
    fn limited_range_black_white_and_primaries() {
        let e = enc(YuvMatrix::Bt709, false, 8);
        let rgb = |y, cb, cr| to_rgb(&raster(e, y, cb, cr)).unwrap().planes[0].data[..4].to_vec();
        assert_eq!(rgb(16, 128, 128), [0, 0, 0, 255]);
        assert_eq!(rgb(235, 128, 128), [255, 255, 255, 255]);
        // BT.709 red: Y' 63, Cb 102, Cr 240 (limited range, 8 bits).
        let red = rgb(63, 102, 240);
        assert!(red[0] >= 253 && red[1] <= 2 && red[2] <= 2, "{red:?}");
        // The same code values through BT.601 are not pure red.
        let e601 = enc(YuvMatrix::Bt601, false, 8);
        let other = to_rgb(&raster(e601, 63, 102, 240)).unwrap().planes[0].data[0];
        assert!(other < 240, "{other}");
    }

    #[test]
    fn full_range_and_ten_bits() {
        let e = enc(YuvMatrix::Bt601, true, 8);
        let px = to_rgb(&raster(e, 255, 128, 128)).unwrap();
        assert_eq!(&px.planes[0].data[..4], &[255, 255, 255, 255]);
        // 10-bit limited BT.2020 PQ: 16-bit output with the encoding attached.
        let mut e = enc(YuvMatrix::Bt2020, false, 10);
        e.colour = h273_colour(Some(9), Some(16));
        let px = to_rgb(&raster(e, 940, 512, 512)).unwrap();
        assert_eq!(px.layout, PixelLayout::Rgba16(e.colour));
        let r = u16::from_ne_bytes([px.planes[0].data[0], px.planes[0].data[1]]);
        assert_eq!(r, 65535);
        let px = to_rgb(&raster(e, 64, 512, 512)).unwrap();
        assert_eq!(&px.planes[0].data[..2], &[0, 0]);
    }

    #[test]
    fn matrices_and_colour_from_h273() {
        assert_eq!(YuvMatrix::from_h273(None, 1080), YuvMatrix::Bt709);
        assert_eq!(YuvMatrix::from_h273(Some(2), 480), YuvMatrix::Bt601);
        assert_eq!(YuvMatrix::from_h273(Some(6), 2160), YuvMatrix::Bt601);
        assert_eq!(YuvMatrix::from_h273(Some(9), 480), YuvMatrix::Bt2020);
        let hlg = h273_colour(Some(9), Some(18));
        assert_eq!(
            (hlg.transfer, hlg.primaries),
            (Transfer::Hlg, Primaries::Bt2020)
        );
        assert_eq!(h273_colour(Some(1), Some(1)), HdrEncoding::SRGB);
        assert_eq!(h273_colour(None, Some(16)).primaries, Primaries::Bt2020);
        // SMPTE 240M is its own matrix (it was shown through BT.601, up to
        // 33 of 255 off on saturated colours), as are FCC, identity and YCgCo.
        assert_eq!(YuvMatrix::from_h273(Some(7), 480), YuvMatrix::Smpte240);
        assert_eq!(YuvMatrix::from_h273(Some(4), 1080), YuvMatrix::Fcc);
        assert_eq!(YuvMatrix::from_h273(Some(0), 480), YuvMatrix::Identity);
        assert_eq!(YuvMatrix::from_h273(Some(8), 480), YuvMatrix::YCgCo);
        // Each matrix maps grey to grey.
        for m in [
            YuvMatrix::Bt601,
            YuvMatrix::Bt709,
            YuvMatrix::Bt2020,
            YuvMatrix::Smpte240,
            YuvMatrix::Fcc,
        ] {
            let k = m.to_rgb();
            for row in k {
                assert!((row[0] - 1.0).abs() < 1e-6);
            }
            // White: Y 1, no chroma; and Kr R + Kg G + Kb B of the red row is Y.
            let (kr, kb) = m.kr_kb();
            let (u, v) = (0.1f32, -0.2f32);
            let r = k[0][0] * 0.5 + k[0][2] * v;
            let g = k[1][0] * 0.5 + k[1][1] * u + k[1][2] * v;
            let b = k[2][0] * 0.5 + k[2][1] * u;
            assert!((kr * r + (1.0 - kr - kb) * g + kb * b - 0.5).abs() < 1e-5);
        }
        let codes: std::collections::HashSet<u32> = [
            YuvMatrix::Bt601,
            YuvMatrix::Bt709,
            YuvMatrix::Bt2020,
            YuvMatrix::Smpte240,
            YuvMatrix::Fcc,
            YuvMatrix::Identity,
            YuvMatrix::YCgCo,
        ]
        .map(YuvMatrix::code)
        .into();
        assert_eq!(codes.len(), 7);
        assert!(
            codes.iter().all(|&c| c < 8),
            "three bits in the shader's flags"
        );
    }

    /// "RGB" video (H.273 matrix 0: libx264rgb, x265 and libaom with gbrp)
    /// was shown through BT.601, 135 of 255 off. Its planes are G, B, R, each coded like luma.
    #[test]
    fn identity_matrix_planes_are_gbr() {
        let rgb = |e: YuvEncoding, g, b, r| {
            to_rgb(&raster(e, g, b, r)).unwrap().planes[0].data[..4].to_vec()
        };
        let full = YuvEncoding {
            cosited_x: false,
            ..enc(YuvMatrix::Identity, true, 8)
        };
        assert_eq!(rgb(full, 200, 30, 90), [90, 200, 30, 255]);
        // Limited range: every plane 16..235.
        let tv = enc(YuvMatrix::Identity, false, 8);
        assert_eq!(rgb(tv, 235, 16, 126), [128, 255, 0, 255]);
        // YCgCo: Y 128, Cg 128 + 64, Co 128: green above grey.
        let px = rgb(enc(YuvMatrix::YCgCo, true, 8), 128, 192, 128);
        assert!(px[1] > 180 && px[0] < 80 && px[2] < 80, "{px:?}");
    }

    /// Vertical siting: co-sited chroma rows sit on the even luma rows, so luma row 0 takes
    /// chroma row 0 alone and row 1 the mean of rows 0 and 1; centred siting takes a quarter
    /// of the way there.
    #[test]
    fn vertical_siting() {
        let e = YuvEncoding {
            cosited_y: true,
            bottom_y: false,
            ..enc(YuvMatrix::Bt601, true, 8)
        };
        let p = plane(1, 2, 2, &[0, 200], false);
        let p = RasterPlane { h_div: 1, ..p };
        assert_eq!(chroma(&p, e, 0, 0), 0.0);
        assert_eq!(chroma(&p, e, 0, 1), 100.0);
        let centred = YuvEncoding {
            cosited_y: false,
            bottom_y: false,
            ..e
        };
        assert_eq!(chroma(&p, centred, 0, 0), 0.0);
        assert_eq!(chroma(&p, centred, 0, 1), 50.0);
        // Bottom siting (H.273 4 and 5): chroma row j on luma row 2j + 1, so luma
        // row 1 takes row 0 alone and row 2 the mean of rows 0 and 1.
        let bottom = YuvEncoding {
            bottom_y: true,
            ..centred
        };
        let p3 = plane(1, 3, 2, &[0, 200, 200], false);
        let p3 = RasterPlane { h_div: 1, ..p3 };
        assert_eq!(chroma(&p3, bottom, 0, 1), 0.0);
        assert_eq!(chroma(&p3, bottom, 0, 2), 100.0);
        assert_eq!(chroma(&p3, bottom, 0, 3), 200.0);
    }

    /// 4:1:1 (DV NTSC) and 4:4:0 (VP9 profile 1) interpolate, not repeat: centred 4:1:1
    /// chroma sample 0 sits at luma column 1.5, so columns 1 and 2 are a quarter of the way
    /// past it towards sample 1 and back; co-sited it sits on column 0.
    #[test]
    fn bytes_round_trip() {
        let mut e = enc(YuvMatrix::YCgCo, true, 10);
        e.colour = h273_colour(Some(9), Some(16));
        for flags in 0..32u8 {
            let e = YuvEncoding {
                cosited_x: flags & 1 != 0,
                cosited_y: flags & 2 != 0,
                bottom_y: flags & 4 != 0,
                right_x: flags & 8 != 0,
                alpha: flags & 16 != 0,
                ..e
            };
            assert_eq!(YuvEncoding::from_bytes(e.to_bytes()), Some(e));
        }
        assert_eq!(YuvEncoding::from_bytes([7, 0, 8, 0]), None);
        assert_eq!(YuvEncoding::from_bytes([0, 0, 7, 0]), None);
    }

    #[test]
    fn chroma_411_and_440_interpolate() {
        let e = enc(YuvMatrix::Bt601, true, 8);
        let mut p = plane(2, 1, 4, &[0, 200], false);
        p.v_div = 1;
        assert_eq!(chroma(&p, e, 0, 0), 0.0);
        assert_eq!(chroma(&p, e, 2, 0), 100.0);
        assert_eq!(chroma(&p, e, 4, 0), 200.0);
        let centred = YuvEncoding {
            cosited_x: false,
            ..e
        };
        assert_eq!(chroma(&p, centred, 1, 0), 0.0);
        assert_eq!(chroma(&p, centred, 2, 0), 25.0);
        assert_eq!(chroma(&p, centred, 5, 0), 175.0);
        // 4:4:0, centred rows: luma row 1 a quarter of the way to chroma row 1.
        let mut q = plane(1, 2, 2, &[0, 200], false);
        q.h_div = 1;
        assert_eq!(chroma(&q, e, 0, 1), 50.0);
        assert_eq!(chroma(&q, e, 0, 2), 150.0);
    }
}

//! YUV → 8-bit RGB/Gray conversion fused with clean-aperture crop and 1/2/1/4/1/8 box
//! downscaling, plus the alpha plane and un-premultiplication.
//!
//! Downscaling averages the Y samples of each `s`×`s` box and the chroma samples covering the
//! same box, then converts once. YUV→RGB is affine, so for SDR this equals averaging RGB
//! (minus clamping effects) at a fraction of the cost. For PQ/HLG the average is taken in the
//! (non-linear) signal domain before tone mapping — a documented approximation.
//!
//! 4:2:0 and 4:2:2 chroma is upsampled bilinearly, as libavif does by default (libyuv's
//! bilinear filter, or its built-in one, which this matches exactly). Siting follows the AV1
//! sequence header: CSP_UNKNOWN (nearly every file) and 4:2:2 are taken as centred between
//! the luma samples, as libavif, libyuv and swscale do; CSP_VERTICAL puts chroma on the even
//! luma columns and CSP_COLOCATED on the even columns and rows (libavif and libheif ignore
//! both). HEIC differs on purpose: specular-heic replicates chroma like libheif's default.
//!
//! HDR tone mapping (PQ, transfer 16, and HLG, transfer 18) is intentionally simple:
//! the signal is linearised (PQ EOTF, or HLG inverse OETF + OOTF for a 1000 nit display),
//! BT.2020 primaries are converted to BT.709, luminance is normalised to 203 nits reference
//! white, compressed with an extended Reinhard curve on max(R,G,B) whose white point is the
//! content peak (`clli` MaxCLL or 1000 nits), then sRGB-encoded.

use rayon::prelude::*;

use crate::Error;
use crate::backend::Planes;

/// Rows per band between cancellation checks.
const BAND: usize = 32;

#[derive(Clone, Copy, Debug)]
enum Matrix {
    Ycbcr { kr: f32, kb: f32 },
    Identity,
    YCgCo,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Tone {
    Sdr,
    Pq {
        peak: f32,
    },
    Hlg,
    /// No transfer or gamut change: the RGB signal itself (for 16-bit output, which keeps
    /// PQ/HLG code values for the renderer to linearise).
    Wide,
}

/// Per-picture conversion constants.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Conv {
    ya: f32,
    yb: f32,
    ca: f32,
    cb: f32,
    matrix: Matrix,
    tone: Tone,
    /// Convert BT.2020 primaries to BT.709 (HDR paths only).
    bt2020: bool,
}

impl Conv {
    /// `matrix`/`full_range`/`primaries` are CICP values; `bits` the significant bit depth.
    pub(crate) fn new(matrix: u16, full_range: bool, primaries: u16, bits: u8, tone: Tone) -> Conv {
        let bits = bits.clamp(8, 16);
        let max = ((1u32 << bits) - 1) as f32;
        let k = (1u32 << (bits - 8)) as f32;
        let (ya, yb, ca, cb) = if full_range {
            let mid = (1u32 << (bits - 1)) as f32;
            (1.0 / max, 0.0, 1.0 / max, -mid / max)
        } else {
            (
                1.0 / (219.0 * k),
                -16.0 / 219.0,
                1.0 / (224.0 * k),
                -128.0 / 224.0,
            )
        };
        let m = match matrix {
            0 => Matrix::Identity,
            8 => Matrix::YCgCo,
            1 => Matrix::Ycbcr {
                kr: 0.2126,
                kb: 0.0722,
            },
            4 => Matrix::Ycbcr { kr: 0.30, kb: 0.11 },
            7 => Matrix::Ycbcr {
                kr: 0.212,
                kb: 0.087,
            },
            9 | 10 => Matrix::Ycbcr {
                kr: 0.2627,
                kb: 0.0593,
            },
            // 5, 6 (BT.601) and unspecified/other: BT.601, as libavif does.
            _ => Matrix::Ycbcr {
                kr: 0.299,
                kb: 0.114,
            },
        };
        Conv {
            ya,
            yb,
            ca,
            cb,
            matrix: m,
            tone,
            bt2020: primaries == 9,
        }
    }

    /// The same conversion with another tone mapping.
    pub(crate) fn with_tone(self, tone: Tone) -> Conv {
        Conv { tone, ..self }
    }

    /// Gray conversion for alpha planes / monochrome: same range handling, no tone mapping.
    pub(crate) fn alpha(full_range: bool, bits: u8) -> Conv {
        Conv::new(1, full_range, 1, bits, Tone::Sdr)
    }

    /// The RGB signal (0..1 nominal, unclamped) of a YUV sample.
    #[inline(always)]
    fn rgb_signal(&self, y: f32, u: f32, v: f32) -> [f32; 3] {
        let (r, g, b) = match self.matrix {
            Matrix::Identity => {
                let f = |x: f32| x * self.ya + self.yb;
                (f(v), f(y), f(u))
            }
            Matrix::YCgCo => {
                let y = y * self.ya + self.yb;
                let cg = u * self.ca + self.cb;
                let co = v * self.ca + self.cb;
                let t = y - cg;
                (t + co, y + cg, t - co)
            }
            Matrix::Ycbcr { kr, kb } => {
                let y = y * self.ya + self.yb;
                let cb = u * self.ca + self.cb;
                let cr = v * self.ca + self.cb;
                let r = y + 2.0 * (1.0 - kr) * cr;
                let b = y + 2.0 * (1.0 - kb) * cb;
                let g = (y - kr * r - kb * b) / (1.0 - kr - kb);
                (r, g, b)
            }
        };
        [r, g, b]
    }

    /// [`rgb_signal`](Self::rgb_signal) as an affine map of the raw samples, with the chroma
    /// inputs taken `chroma_scale` times larger (for weighted sums): `rgb = m · (y, u, v) + o`.
    fn affine(&self, chroma_scale: f32) -> Affine {
        let o = self.rgb_signal(0.0, 0.0, 0.0);
        let col = |y, u, v| {
            let p = self.rgb_signal(y, u, v);
            [p[0] - o[0], p[1] - o[1], p[2] - o[2]]
        };
        let (cy, cu, cv) = (col(1.0, 0.0, 0.0), col(0.0, 1.0, 0.0), col(0.0, 0.0, 1.0));
        let m = [0, 1, 2].map(|c| [cy[c], cu[c] / chroma_scale, cv[c] / chroma_scale]);
        Affine { m, o }
    }

    #[inline(always)]
    fn rgb(&self, y: f32, u: f32, v: f32) -> [u8; 3] {
        let [r, g, b] = self.rgb_signal(y, u, v);
        match self.tone {
            Tone::Sdr | Tone::Wide => [q(r), q(g), q(b)],
            _ => self.hdr([r, g, b]),
        }
    }

    #[inline(always)]
    fn gray(&self, y: f32) -> u8 {
        let y = y * self.ya + self.yb;
        match self.tone {
            Tone::Sdr | Tone::Wide => q(y),
            _ => self.hdr([y, y, y])[0],
        }
    }

    /// [`rgb`](Self::rgb) as 16-bit signal values (no tone mapping).
    #[inline(always)]
    fn rgb16(&self, y: f32, u: f32, v: f32) -> [u16; 3] {
        self.rgb_signal(y, u, v).map(q16)
    }

    #[inline(always)]
    fn gray16(&self, y: f32) -> u16 {
        q16(y * self.ya + self.yb)
    }

    #[cold]
    fn hdr(&self, e: [f32; 3]) -> [u8; 3] {
        let e = e.map(|x| x.clamp(0.0, 1.0));
        // Linear light in nits.
        let (mut lin, peak) = match self.tone {
            Tone::Pq { peak } => (e.map(pq_eotf), peak),
            _ => (hlg_to_display(e), 1000.0),
        };
        if self.bt2020 {
            let [r, g, b] = lin;
            lin = [
                1.6605 * r - 0.5876 * g - 0.0728 * b,
                -0.1246 * r + 1.1329 * g - 0.0083 * b,
                -0.0182 * r - 0.1006 * g + 1.1187 * b,
            ];
        }
        const REF_WHITE: f32 = 203.0;
        let lin = lin.map(|x| x.max(0.0) / REF_WHITE);
        let w = (peak / REF_WHITE).max(1.0);
        let m = lin[0].max(lin[1]).max(lin[2]);
        let scale = if m > 0.0 {
            (m * (1.0 + m / (w * w)) / (1.0 + m)) / m
        } else {
            0.0
        };
        lin.map(|x| q(srgb_oetf(x * scale)))
    }
}

/// YUV → RGB signal as one affine map (every supported matrix is affine).
#[derive(Clone, Copy, Debug)]
struct Affine {
    m: [[f32; 3]; 3],
    o: [f32; 3],
}

#[inline(always)]
fn q(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

#[inline(always)]
fn q16(x: f32) -> u16 {
    (x.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16
}

fn pq_eotf(e: f32) -> f32 {
    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 4096.0 * 128.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 4096.0 * 32.0;
    const C3: f32 = 2392.0 / 4096.0 * 32.0;
    let p = e.powf(1.0 / M2);
    let num = (p - C1).max(0.0);
    let den = C2 - C3 * p;
    if den <= 0.0 {
        return 10000.0;
    }
    10000.0 * (num / den).powf(1.0 / M1)
}

/// HLG signal → display light (nits) for a 1000 nit display (BT.2100 OOTF, gamma 1.2).
fn hlg_to_display(e: [f32; 3]) -> [f32; 3] {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_7;
    let inv = |x: f32| {
        if x <= 0.5 {
            x * x / 3.0
        } else {
            (((x - C) / A).exp() + B) / 12.0
        }
    };
    let s = e.map(inv);
    let ys = 0.2627 * s[0] + 0.6780 * s[1] + 0.0593 * s[2];
    let g = if ys > 0.0 { ys.powf(0.2) } else { 0.0 };
    s.map(|x| 1000.0 * g * x)
}

fn srgb_oetf(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Source rectangle (in picture pixels) and scale factor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Region {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    pub scale: usize,
}

impl Region {
    pub(crate) fn out_w(&self) -> usize {
        self.w.div_ceil(self.scale)
    }
    pub(crate) fn out_h(&self) -> usize {
        self.h.div_ceil(self.scale)
    }
}

/// Destination: a canvas of `width`×`height` pixels with `channels` (1, 3 or 4) samples
/// each, one byte per sample, or with `wide` two (native-endian `u16`, the signal without
/// tone mapping); the converted region lands at (`x`, `y`) output pixels and is clipped to
/// the canvas.
pub(crate) struct Dst<'a> {
    pub data: &'a mut [u8],
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    pub wide: bool,
    pub x: usize,
    pub y: usize,
}

/// What to write: the colour channels (gray, or RGB) or the alpha channel (index 3).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Color,
    Alpha,
}

trait Sample: Copy + Send + Sync {
    fn at(row: &[u8], x: usize) -> u32;
    /// Samples `x0..x0 + n` of `row`.
    fn span(row: &[u8], x0: usize, n: usize) -> impl Iterator<Item = u32>;
}

#[derive(Clone, Copy)]
struct U8;
impl Sample for U8 {
    #[inline(always)]
    fn at(row: &[u8], x: usize) -> u32 {
        u32::from(row[x])
    }
    #[inline(always)]
    fn span(row: &[u8], x0: usize, n: usize) -> impl Iterator<Item = u32> {
        row[x0..x0 + n].iter().map(|&b| u32::from(b))
    }
}

#[derive(Clone, Copy)]
struct U16;
impl Sample for U16 {
    #[inline(always)]
    fn at(row: &[u8], x: usize) -> u32 {
        u32::from(u16::from_ne_bytes([row[2 * x], row[2 * x + 1]]))
    }
    #[inline(always)]
    fn span(row: &[u8], x0: usize, n: usize) -> impl Iterator<Item = u32> {
        row[2 * x0..2 * (x0 + n)]
            .chunks_exact(2)
            .map(|b| u32::from(u16::from_ne_bytes([b[0], b[1]])))
    }
}

/// The chroma sample under luma position `x` on an axis subsampled by two (`n` chroma
/// samples), its neighbour, and the first one's bilinear weight out of 4 (the neighbour gets
/// the rest). Centred siting (between two luma samples): 3/4 + 1/4 of the nearer neighbour,
/// as libavif and libyuv do. Co-sited (on the even luma samples): the sample itself, or the
/// mean of the two either side of an odd position. Edges repeat the outermost sample.
#[inline(always)]
fn taps(x: usize, n: usize, cosited: bool) -> (usize, usize, u32) {
    let c = x >> 1;
    // Co-sited even positions give the left neighbour weight 0.
    if x & 1 == 0 {
        (c, c.saturating_sub(1), if cosited { 4 } else { 3 })
    } else {
        (c, (c + 1).min(n - 1), if cosited { 2 } else { 3 })
    }
}

/// Converts `region` of picture `p` into `dst`.
pub(crate) fn convert(
    p: &Planes<'_>,
    conv: &Conv,
    region: Region,
    dst: &mut Dst<'_>,
    target: Target,
    parallel: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<(), Error> {
    validate(p, region, target)?;
    match (p.hbd, dst.wide) {
        (true, false) => run::<U16, false>(p, conv, region, dst, target, parallel, cancel),
        (false, false) => run::<U8, false>(p, conv, region, dst, target, parallel, cancel),
        (true, true) => run::<U16, true>(p, conv, region, dst, target, parallel, cancel),
        (false, true) => run::<U8, true>(p, conv, region, dst, target, parallel, cancel),
    }
}

fn validate(p: &Planes<'_>, r: Region, target: Target) -> Result<(), Error> {
    let bad = || Error::Decoder("decoded picture geometry is inconsistent".into());
    if r.scale == 0 || r.w == 0 || r.h == 0 {
        return Err(bad());
    }
    if r.x + r.w > p.width as usize || r.y + r.h > p.height as usize {
        return Err(Error::Malformed(
            "region outside the decoded picture".into(),
        ));
    }
    let bytes = if p.hbd { 2 } else { 1 };
    let need = |plane: usize, w: usize, h: usize| -> bool {
        let stride = p.strides[plane];
        stride >= w * bytes && p.planes[plane].len() >= (h - 1) * stride + w * bytes
    };
    let (w, h) = (p.width as usize, p.height as usize);
    if !need(0, w, h) {
        return Err(bad());
    }
    if !p.mono && target == Target::Color {
        let cw = if p.ssx { w.div_ceil(2) } else { w };
        let ch = if p.ssy { h.div_ceil(2) } else { h };
        if !need(1, cw, ch) || !need(2, cw, ch) {
            return Err(bad());
        }
    }
    Ok(())
}

fn run<T: Sample, const W: bool>(
    p: &Planes<'_>,
    conv: &Conv,
    region: Region,
    dst: &mut Dst<'_>,
    target: Target,
    parallel: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if dst.x >= dst.width || dst.y >= dst.height {
        return Ok(());
    }
    let ch = dst.channels;
    let bps = if W { 2 } else { 1 };
    let stride = dst.width * ch * bps;
    let out_w = region.out_w().min(dst.width - dst.x);
    let out_h = region.out_h().min(dst.height - dst.y);
    let rows = dst
        .data
        .get_mut(dst.y * stride..(dst.y + out_h) * stride)
        .ok_or_else(|| Error::Decoder("destination too small".into()))?;
    let x0 = dst.x * ch * bps;
    let job = |(i, row): (usize, &mut [u8]), acc: &mut Vec<u32>| {
        let out = &mut row[x0..x0 + out_w * ch * bps];
        if region.scale == 1 {
            row_1x::<T, W>(p, conv, region, i, out, ch, target, acc);
        } else {
            row_box::<T, W>(p, conv, region, i, out, ch, target, acc);
        }
    };
    for (b, band) in rows.chunks_mut(stride * BAND).enumerate() {
        if cancel() {
            return Err(Error::Cancelled);
        }
        let base = b * BAND;
        if parallel {
            band.par_chunks_mut(stride)
                .enumerate()
                .for_each_init(Vec::new, |acc, (i, row)| job((base + i, row), acc));
        } else {
            let mut acc = Vec::new();
            for (i, row) in band.chunks_mut(stride).enumerate() {
                job((base + i, row), &mut acc);
            }
        }
    }
    Ok(())
}

#[inline(always)]
fn row_of<'a>(p: &Planes<'a>, plane: usize, y: usize) -> &'a [u8] {
    &p.planes[plane][y * p.strides[plane]..]
}

#[inline(always)]
fn put16(out: &mut [u8], k: usize, v: u16) {
    out[2 * k..2 * k + 2].copy_from_slice(&v.to_ne_bytes());
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn put<const W: bool>(
    out: &mut [u8],
    i: usize,
    ch: usize,
    target: Target,
    conv: &Conv,
    mono: bool,
    y: f32,
    u: f32,
    v: f32,
) {
    if W {
        match target {
            Target::Alpha => put16(out, i * ch + 3, conv.gray16(y)),
            Target::Color => {
                let rgb = if mono {
                    [conv.gray16(y); 3]
                } else {
                    conv.rgb16(y, u, v)
                };
                for (c, s) in rgb.into_iter().enumerate().take(ch.min(3)) {
                    put16(out, i * ch + c, s);
                }
            }
        }
        return;
    }
    match target {
        Target::Alpha => out[i * ch + 3] = conv.gray(y),
        Target::Color => {
            if ch == 1 {
                out[i] = conv.gray(y);
            } else if mono {
                let g = conv.gray(y);
                out[i * ch] = g;
                out[i * ch + 1] = g;
                out[i * ch + 2] = g;
            } else {
                let [r, g, b] = conv.rgb(y, u, v);
                out[i * ch] = r;
                out[i * ch + 1] = g;
                out[i * ch + 2] = b;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn row_1x<T: Sample, const W: bool>(
    p: &Planes<'_>,
    conv: &Conv,
    r: Region,
    oy: usize,
    out: &mut [u8],
    ch: usize,
    target: Target,
    acc: &mut Vec<u32>,
) {
    let sy = r.y + oy;
    let yr = row_of(p, 0, sy);
    let mono = p.mono || target == Target::Alpha || ch == 1;
    let n = out.len() / (ch * if W { 2 } else { 1 });
    if mono {
        for i in 0..n {
            let y = T::at(yr, r.x + i) as f32;
            put::<W>(out, i, ch, target, conv, true, y, 0.0, 0.0);
        }
        return;
    }
    let (cu, cv) = chroma_row::<T>(p, r.x, n, sy, acc);
    if !W && matches!(conv.tone, Tone::Sdr | Tone::Wide) {
        // The common case, branch-free per pixel so that it vectorises.
        let a = conv.affine(16.0);
        match ch {
            3 => rgb_span::<T, 3>(yr, r.x, cu, cv, out, &a),
            _ => rgb_span::<T, 4>(yr, r.x, cu, cv, out, &a),
        }
        return;
    }
    for (i, (&u, &v)) in cu.iter().zip(cv).enumerate() {
        let y = T::at(yr, r.x + i) as f32;
        let (u, v) = (u as f32 * (1.0 / 16.0), v as f32 * (1.0 / 16.0));
        put::<W>(out, i, ch, target, conv, false, y, u, v);
    }
}

/// 8-bit RGB(A) of luma samples `x0..` and upsampled chroma; alpha is left alone.
#[inline(always)]
fn rgb_span<T: Sample, const CH: usize>(
    yr: &[u8],
    x0: usize,
    cu: &[u32],
    cv: &[u32],
    out: &mut [u8],
    a: &Affine,
) {
    let px = out.chunks_exact_mut(CH).zip(cu.iter().zip(cv));
    for ((o, (&u, &v)), y) in px.zip(T::span(yr, x0, cu.len())) {
        let (y, u, v) = (y as f32, u as f32, v as f32);
        for ((d, m), off) in o.iter_mut().zip(&a.m).zip(a.o) {
            *d = q(m[0] * y + m[1] * u + m[2] * v + off);
        }
    }
}

/// Bilinear chroma of luma row `sy`, columns `x0..x0 + n`, honouring the chroma siting: two
/// spans (U, V) of weighted sums out of 16, kept in `acc`.
fn chroma_row<'a, T: Sample>(
    p: &Planes<'_>,
    x0: usize,
    n: usize,
    sy: usize,
    acc: &'a mut Vec<u32>,
) -> (&'a [u32], &'a [u32]) {
    let (cy0, cy1, wy0) = if p.ssy {
        let chh = (p.height as usize).div_ceil(2).max(1);
        taps(sy, chh, p.cosited_y)
    } else {
        (sy, sy, 4)
    };
    let wy1 = 4 - wy0;
    let rows = [
        (row_of(p, 1, cy0), row_of(p, 1, cy1)),
        (row_of(p, 2, cy0), row_of(p, 2, cy1)),
    ];
    acc.clear();
    if !p.ssx {
        acc.resize(2 * n, 0);
        let (au, av) = acc.split_at_mut(n);
        for (dst, (r0, r1)) in [au, av].into_iter().zip(rows) {
            for ((d, a), b) in dst
                .iter_mut()
                .zip(T::span(r0, x0, n))
                .zip(T::span(r1, x0, n))
            {
                *d = 4 * (a * wy0 + b * wy1);
            }
        }
        return (&acc[..n], &acc[n..2 * n]);
    }
    // Chroma columns c_lo..=c_hi lie under the span. Blend their rows (plus one clamped
    // column either side) into `vb`, then interpolate each pair of luma columns.
    let cw = (p.width as usize).div_ceil(2).max(1);
    let (c_lo, c_hi) = (x0 >> 1, (x0 + n - 1) >> 1);
    let m = c_hi - c_lo + 1;
    let (we, wo) = if p.cosited_x { (4, 2) } else { (3, 3) };
    acc.resize(2 * (m + 2) + 4 * m, 0);
    let (vbs, ups) = acc.split_at_mut(2 * (m + 2));
    for (k, (r0, r1)) in rows.into_iter().enumerate() {
        let vb = &mut vbs[k * (m + 2)..(k + 1) * (m + 2)];
        let blend = |c: usize| T::at(r0, c) * wy0 + T::at(r1, c) * wy1;
        vb[0] = blend(c_lo.saturating_sub(1));
        for ((d, a), b) in vb[1..=m]
            .iter_mut()
            .zip(T::span(r0, c_lo, m))
            .zip(T::span(r1, c_lo, m))
        {
            *d = a * wy0 + b * wy1;
        }
        vb[m + 1] = blend((c_hi + 1).min(cw - 1));
        let up = &mut ups[k * 2 * m..(k + 1) * 2 * m];
        for (pair, w) in up.chunks_exact_mut(2).zip(vb.windows(3)) {
            pair[0] = we * w[1] + (4 - we) * w[0];
            pair[1] = wo * w[1] + (4 - wo) * w[2];
        }
    }
    let off = x0 & 1;
    (&ups[off..off + n], &ups[2 * m + off..2 * m + off + n])
}

/// Vertical weight (out of 4) that luma row `yy` gives chroma row `cy`.
#[inline]
fn tap(yy: usize, sub: usize, n: usize, cy: usize, cosited: bool) -> u32 {
    if sub == 0 {
        return if yy == cy { 4 } else { 0 };
    }
    let (c, nb, w) = taps(yy, n, cosited);
    u32::from(c == cy) * w + u32::from(nb == cy) * (4 - w)
}

#[allow(clippy::too_many_arguments)]
fn row_box<T: Sample, const W: bool>(
    p: &Planes<'_>,
    conv: &Conv,
    r: Region,
    oy: usize,
    out: &mut [u8],
    ch: usize,
    target: Target,
    acc: &mut Vec<u32>,
) {
    let s = r.scale;
    let ow = out.len() / (ch * if W { 2 } else { 1 });
    let y0 = r.y + oy * s;
    let y1 = (y0 + s).min(r.y + r.h);
    let x_end = r.x + r.w;
    // Column sums of Y over the box rows. The sums are zipped with the row's samples (no
    // indexing): indexed, a profile-guided build kept a bounds check per sample and left
    // the loop scalar (AVIF thumbnails +7 % there).
    acc.clear();
    acc.resize(r.w * 5 + 4, 0);
    let (ya, rest) = acc.split_at_mut(r.w);
    let (rest, uvh) = rest.split_at_mut(2 * (r.w + 2));
    let (uh, vh) = uvh.split_at_mut(r.w);
    for yy in y0..y1 {
        let row = row_of(p, 0, yy);
        for (a, v) in ya.iter_mut().zip(T::span(row, r.x, r.w)) {
            *a += v;
        }
    }
    let rows = (y1 - y0) as u32;
    let mono = p.mono || target == Target::Alpha || ch == 1;
    // Chroma: the box average of the bilinear upsampled chroma, so scaled output matches
    // box-downscaling the 1:1 output. Each luma row/column gives its chroma sample and the
    // neighbour the weights of [`taps`] (4 to itself when not subsampled).
    let (sxs, sys) = (usize::from(p.ssx), usize::from(p.ssy));
    let cwid = ((p.width as usize + sxs) >> sxs).max(1);
    let chgt = ((p.height as usize + sys) >> sys).max(1);
    let (ua, va) = rest.split_at_mut(r.w + 2);
    let cosited_x = p.cosited_x;
    let cx_base = (r.x >> sxs).saturating_sub(sxs);
    if !mono {
        let cy_lo = (y0 >> sys).saturating_sub(sys);
        let cy_hi = (((y1 - 1) >> sys) + 1 + sys).min(chgt);
        let cx_hi = (((x_end - 1) >> sxs) + 1 + sxs).min(cwid);
        let ncol = cx_hi - cx_base;
        for cy in cy_lo..cy_hi {
            let w = (y0..y1)
                .map(|yy| tap(yy, sys, chgt, cy, p.cosited_y))
                .sum::<u32>();
            if w == 0 {
                continue;
            }
            let ur = T::span(row_of(p, 1, cy), cx_base, ncol);
            let vr = T::span(row_of(p, 2, cy), cx_base, ncol);
            for (a, u) in ua[..ncol].iter_mut().zip(ur) {
                *a += w * u;
            }
            for (a, v) in va[..ncol].iter_mut().zip(vr) {
                *a += w * v;
            }
        }
        // The chroma of each luma column (horizontal taps), so that a box below is a plain
        // sum like the luma one.
        for (k, (hu, hv)) in uh.iter_mut().zip(vh.iter_mut()).enumerate() {
            let x = r.x + k;
            let (c, n, w) = if sxs == 0 {
                (x, x, 4)
            } else {
                taps(x, cwid, cosited_x)
            };
            let (c, n) = (c - cx_base, n - cx_base);
            *hu = w * ua[c] + (4 - w) * ua[n];
            *hv = w * va[c] + (4 - w) * va[n];
        }
    }
    // Boxes of `s` columns (the last one may be narrower), as many as the row has room for.
    let boxes = ya.chunks(s).zip(uh.chunks(s).zip(vh.chunks(s)));
    for (i, (yb, (ub, vb))) in boxes.take(ow).enumerate() {
        let ys: u32 = yb.iter().sum();
        let lum = yb.len() as u32;
        let y = ys as f32 / (rows * lum) as f32;
        if mono {
            put::<W>(out, i, ch, target, conv, true, y, 0.0, 0.0);
        } else {
            // At most 4 · (4 · 8 rows) · 4095 · 8 columns: no overflow.
            let us: u32 = ub.iter().sum();
            let vs: u32 = vb.iter().sum();
            let d = (16 * rows * lum) as f32;
            put::<W>(
                out,
                i,
                ch,
                target,
                conv,
                false,
                y,
                us as f32 / d,
                vs as f32 / d,
            );
        }
    }
}

/// Converts premultiplied RGBA to straight alpha in place.
pub(crate) fn unpremultiply(data: &mut [u8], parallel: bool) {
    let f = |px: &mut [u8]| {
        let a = u32::from(px[3]);
        if a == 0 {
            px[..3].fill(0);
        } else if a < 255 {
            for c in &mut px[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    };
    if parallel {
        data.par_chunks_mut(4 * 4096)
            .for_each(|c| c.chunks_exact_mut(4).for_each(f));
    } else {
        data.chunks_exact_mut(4).for_each(f);
    }
}

/// Converts premultiplied 16-bit RGBA (native-endian) to straight alpha in place.
pub(crate) fn unpremultiply16(data: &mut [u8], parallel: bool) {
    let f = |px: &mut [u8]| {
        let rd = |px: &[u8], c: usize| u32::from(u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]));
        let a = rd(px, 3);
        for c in 0..3 {
            let v = (rd(px, c) * 65535 + a / 2)
                .checked_div(a)
                .map_or(0, |v| v.min(65535) as u16);
            px[2 * c..2 * c + 2].copy_from_slice(&v.to_ne_bytes());
        }
    };
    if parallel {
        data.par_chunks_mut(8 * 4096)
            .for_each(|c| c.chunks_exact_mut(8).for_each(f));
    } else {
        data.chunks_exact_mut(8).for_each(f);
    }
}

/// [`downscale`] for 16-bit samples (native-endian `u16`, two bytes each).
pub(crate) fn downscale16(
    src: &[u8],
    width: usize,
    ch: usize,
    region: Region,
    weighted: bool,
    parallel: bool,
) -> Vec<u8> {
    let weighted = weighted && ch == 4;
    let s = region.scale;
    let (ow, oh) = (region.out_w(), region.out_h());
    let mut out = vec![0u8; ow * oh * ch * 2];
    let rd = |i: usize| u64::from(u16::from_ne_bytes([src[2 * i], src[2 * i + 1]]));
    let job = |(oy, row): (usize, &mut [u8])| {
        let y0 = region.y + oy * s;
        let y1 = (y0 + s).min(region.y + region.h);
        let mut wr = |k: usize, v: u64| {
            row[2 * k..2 * k + 2].copy_from_slice(&(v.min(65535) as u16).to_ne_bytes());
        };
        for ox in 0..ow {
            let x0 = region.x + ox * s;
            let x1 = (x0 + s).min(region.x + region.w);
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            let mut sum = [0u64; 4];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let base = (yy * width + xx) * ch;
                    if weighted {
                        let a = rd(base + 3);
                        for (c, s) in sum.iter_mut().enumerate().take(3) {
                            *s += rd(base + c) * a;
                        }
                        sum[3] += a;
                    } else {
                        for (c, s) in sum.iter_mut().enumerate().take(ch) {
                            *s += rd(base + c);
                        }
                    }
                }
            }
            if weighted {
                for c in 0..3 {
                    wr(
                        ox * 4 + c,
                        (sum[c] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0),
                    );
                }
                wr(ox * 4 + 3, (sum[3] + n / 2) / n);
            } else {
                for (c, &v) in sum.iter().enumerate().take(ch) {
                    wr(ox * ch + c, (v + n / 2) / n);
                }
            }
        }
    };
    if ow == 0 || oh == 0 {
        return out;
    }
    if parallel {
        out.par_chunks_mut(ow * ch * 2).enumerate().for_each(job);
    } else {
        out.chunks_mut(ow * ch * 2).enumerate().for_each(job);
    }
    out
}

/// Box-downscales (and crops) an interleaved 8-bit buffer; used when a grid's tiles do not
/// align with the scale factor, a crop applies to a whole grid canvas, or straight alpha is
/// reduced. `weighted` (RGBA with straight alpha): colour is averaged weighted by alpha.
pub(crate) fn downscale(
    src: &[u8],
    width: usize,
    ch: usize,
    region: Region,
    weighted: bool,
    parallel: bool,
) -> Vec<u8> {
    let weighted = weighted && ch == 4;
    let s = region.scale;
    let (ow, oh) = (region.out_w(), region.out_h());
    let mut out = vec![0u8; ow * oh * ch];
    let job = |(oy, row): (usize, &mut [u8])| {
        let y0 = region.y + oy * s;
        let y1 = (y0 + s).min(region.y + region.h);
        for ox in 0..ow {
            let x0 = region.x + ox * s;
            let x1 = (x0 + s).min(region.x + region.w);
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            if weighted {
                let mut sum = [0u32; 4];
                for yy in y0..y1 {
                    let base = yy * width * 4;
                    for xx in x0..x1 {
                        let p = &src[base + xx * 4..base + xx * 4 + 4];
                        let a = u32::from(p[3]);
                        for c in 0..3 {
                            sum[c] += u32::from(p[c]) * a;
                        }
                        sum[3] += a;
                    }
                }
                let px = &mut row[ox * 4..ox * 4 + 4];
                for c in 0..3 {
                    px[c] = (sum[c] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
                }
                px[3] = ((sum[3] + n / 2) / n) as u8;
                continue;
            }
            for c in 0..ch {
                let mut sum = 0u32;
                for yy in y0..y1 {
                    let base = yy * width * ch;
                    for xx in x0..x1 {
                        sum += u32::from(src[base + xx * ch + c]);
                    }
                }
                row[ox * ch + c] = ((sum + n / 2) / n) as u8;
            }
        }
    };
    if ow == 0 || oh == 0 {
        return out;
    }
    if parallel {
        out.par_chunks_mut(ow * ch).enumerate().for_each(job);
    } else {
        out.chunks_mut(ow * ch).enumerate().for_each(job);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduced_straight_alpha_has_no_fringe() {
        // 4×1: two opaque red pixels, two transparent ones hiding white.
        let src = [
            255, 0, 0, 255, 255, 0, 0, 255, 255, 255, 255, 0, 255, 255, 255, 0,
        ];
        let r = Region {
            x: 1,
            y: 0,
            w: 2,
            h: 1,
            scale: 2,
        };
        // The middle two: red at alpha ½ — not pink.
        assert_eq!(downscale(&src, 4, 4, r, true, false), [255, 0, 0, 128]);
        assert_eq!(downscale(&src, 4, 4, r, false, false), [255, 128, 128, 128]);
        let clear = [9, 9, 9, 0, 9, 9, 9, 0];
        let r = Region {
            x: 0,
            y: 0,
            w: 2,
            h: 1,
            scale: 2,
        };
        assert_eq!(downscale(&clear, 2, 4, r, true, false), [0, 0, 0, 0]);
    }

    #[test]
    fn chroma_taps_follow_the_siting() {
        // Centred: 3/4 nearer sample + 1/4 the other, edges clamped.
        assert_eq!(taps(0, 4, false), (0, 0, 3));
        assert_eq!(taps(1, 4, false), (0, 1, 3));
        assert_eq!(taps(2, 4, false), (1, 0, 3));
        assert_eq!(taps(7, 4, false), (3, 3, 3));
        // Co-sited: even outputs are the sample itself, odd ones the mean of two.
        assert_eq!(taps(0, 4, true).2, 4);
        assert_eq!(taps(4, 4, true), (2, 1, 4));
        assert_eq!(taps(5, 4, true), (2, 3, 2));
        assert_eq!(taps(7, 4, true), (3, 3, 2));
    }

    #[test]
    fn affine_matches_rgb_signal() {
        for (mc, full, bits) in [(6, false, 8), (1, true, 10), (0, true, 8), (8, false, 12)] {
            let c = Conv::new(mc, full, 1, bits, Tone::Sdr);
            let a = c.affine(1.0);
            let k = f32::from(1u16 << (bits - 8));
            for (y, u, v) in [
                (16.0, 128.0, 128.0),
                (200.0, 60.0, 190.0),
                (90.0, 240.0, 20.0),
            ] {
                let (y, u, v) = (y * k, u * k, v * k);
                let want = c.rgb_signal(y, u, v);
                for (ch, w) in want.into_iter().enumerate() {
                    let got = a.m[ch][0] * y + a.m[ch][1] * u + a.m[ch][2] * v + a.o[ch];
                    assert!((got - w).abs() < 1e-3, "mc {mc} ch {ch}: {got} vs {w}");
                }
            }
        }
    }

    #[test]
    fn bt601_limited_gray_and_red() {
        let c = Conv::new(6, false, 1, 8, Tone::Sdr);
        assert_eq!(c.rgb(16.0, 128.0, 128.0), [0, 0, 0]);
        assert_eq!(c.rgb(235.0, 128.0, 128.0), [255, 255, 255]);
        let [r, g, b] = c.rgb(81.0, 90.0, 240.0);
        assert!(r >= 253 && g <= 2 && b <= 2, "{r} {g} {b}");
    }

    #[test]
    fn ten_bit_full_range() {
        let c = Conv::new(1, true, 1, 10, Tone::Sdr);
        assert_eq!(c.rgb(1023.0, 512.0, 512.0), [255, 255, 255]);
        assert_eq!(c.gray(0.0), 0);
    }

    #[test]
    fn pq_tone_map_monotonic() {
        let c = Conv::new(9, false, 9, 10, Tone::Pq { peak: 1000.0 });
        let a = c.gray(400.0);
        let b = c.gray(600.0);
        let d = c.gray(900.0);
        assert!(a < b && b < d, "{a} {b} {d}");
    }

    #[test]
    fn wide_output_keeps_the_signal() {
        // 10-bit limited-range PQ: the 16-bit output is the RGB signal, not tone mapped.
        let c = Conv::new(9, false, 9, 10, Tone::Wide);
        assert_eq!(c.rgb16(940.0, 512.0, 512.0), [65535; 3]);
        assert_eq!(c.rgb16(64.0, 512.0, 512.0), [0; 3]);
        let mid = c.gray16(502.0);
        assert!((mid as f32 / 65535.0 - 0.5).abs() < 1e-3, "{mid}");
        let mut px = [0u8; 8];
        for (i, v) in [100u16, 200, 0, 32768].iter().enumerate() {
            px[2 * i..2 * i + 2].copy_from_slice(&v.to_ne_bytes());
        }
        unpremultiply16(&mut px, false);
        let rd = |i: usize| u16::from_ne_bytes([px[2 * i], px[2 * i + 1]]);
        assert_eq!([rd(0), rd(1), rd(2), rd(3)], [200, 400, 0, 32768]);
        let r = Region {
            x: 0,
            y: 0,
            w: 2,
            h: 1,
            scale: 2,
        };
        let src: Vec<u8> = [1000u16, 3000]
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect();
        assert_eq!(
            downscale16(&src, 2, 1, r, false, false),
            2000u16.to_ne_bytes()
        );
    }

    #[test]
    fn unpremultiply_basic() {
        let mut px = [64, 32, 0, 128, 10, 10, 10, 0];
        unpremultiply(&mut px, false);
        assert_eq!(px, [128, 64, 0, 128, 0, 0, 0, 0]);
    }
}

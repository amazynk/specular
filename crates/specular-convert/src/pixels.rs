//! From a decoded raster to output samples: every layout becomes linear-light BT.709 RGBA with
//! premultiplied alpha (through the source's ICC profile when converting, chroma upsampled
//! bilinearly, HDR tone-mapped unless it is kept), is resampled with a separable Lanczos-3
//! filter streaming a window of rows (never the whole image in floats), then encoded with
//! the sRGB curve (or PQ for kept HDR) to 8 or 16 bits, and finally turned upright.

use specular_color::SourceLut;
use specular_core::hdr::{self, HdrEncoding, Primaries};
use specular_core::{CancelToken, Orientation, PixelLayout, Raster};
use std::collections::VecDeque;
use std::sync::OnceLock;

/// How the raster's encoded values become linear BT.709.
pub enum ToLinear {
    /// The sRGB curve (sRGB files, untagged files, and profiles kept as they are).
    Srgb,
    /// The source profile's transform (converting to sRGB).
    Lut(SourceLut),
}

/// Transfer function of the output samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutTransfer {
    Srgb,
    /// BT.2100 PQ in BT.2020 primaries (HDR kept, 16 bits).
    Pq,
}

/// What [`render`] produces.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    /// Output size before orientation (in the raster's stored orientation).
    pub width: u32,
    pub height: u32,
    /// One grey channel instead of RGB.
    pub gray: bool,
    /// Keep an alpha channel (dropped again when the result is fully opaque); otherwise
    /// transparency is flattened onto white.
    pub alpha: bool,
    pub sixteen: bool,
    pub transfer: OutTransfer,
    /// Tone-map HDR content to SDR.
    pub tone_map: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Samples {
    Eight(Vec<u8>),
    Sixteen(Vec<u16>),
}

/// Interleaved, upright output samples.
#[derive(Clone, Debug, PartialEq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    /// 1 (grey), 2 (grey + alpha), 3 (RGB) or 4 (RGBA).
    pub channels: usize,
    pub samples: Samples,
}

impl Pixels {
    pub fn has_alpha(&self) -> bool {
        self.channels.is_multiple_of(2)
    }
    pub fn gray(&self) -> bool {
        self.channels <= 2
    }
}

pub struct Cancelled;

/// Renders `raster` (stored orientation) into upright output samples.
pub fn render(
    raster: &Raster,
    to_linear: &ToLinear,
    target: &Target,
    orientation: Orientation,
    cancel: &CancelToken,
) -> Result<Pixels, Cancelled> {
    // A video frame's YCbCr planes go to R'G'B' first (movies decode to RGB for
    // conversion; this covers a frame handed over as decoded).
    let rgb;
    let raster = match raster.layout {
        PixelLayout::Yuv(_) => {
            rgb = specular_core::yuv::to_rgb(raster).unwrap_or_else(|| raster.clone());
            &rgb
        }
        _ => raster,
    };
    let src = Source::new(raster, to_linear, target.tone_map);
    let (iw, ih) = (raster.width as usize, raster.height as usize);
    let (ow, oh) = (target.width.max(1) as usize, target.height.max(1) as usize);
    let mut q = Quantizer::new(target, raster.layout.has_alpha(), ow, oh);
    let mut row = vec![0f32; iw * 4];
    if (ow, oh) == (iw, ih) {
        for y in 0..ih {
            if y % 64 == 0 && cancel.is_cancelled() {
                return Err(Cancelled);
            }
            src.row(y, &mut row);
            q.push(&row);
        }
    } else {
        let hx = taps(iw, ow);
        let vy = taps(ih, oh);
        let mut cache: VecDeque<(usize, Vec<f32>)> = VecDeque::new();
        let mut acc = vec![0f32; ow * 4];
        for (oy, t) in vy.iter().enumerate() {
            if oy % 16 == 0 && cancel.is_cancelled() {
                return Err(Cancelled);
            }
            while cache.front().is_some_and(|(y, _)| *y < t.start) {
                cache.pop_front();
            }
            let need_end = t.start + t.w.len();
            let mut next = cache.back().map_or(t.start, |(y, _)| y + 1).max(t.start);
            while next < need_end {
                src.row(next, &mut row);
                let mut h = vec![0f32; ow * 4];
                resample_row(&row, &hx, &mut h);
                cache.push_back((next, h));
                next += 1;
            }
            acc.fill(0.0);
            for (j, &w) in t.w.iter().enumerate() {
                let (y, h) = &cache[t.start + j - cache[0].0];
                debug_assert_eq!(*y, t.start + j);
                for (a, v) in acc.iter_mut().zip(h) {
                    *a += w * v;
                }
            }
            q.push(&acc);
        }
    }
    let (channels, samples) = q.finish();
    let (w, h, samples) = match samples {
        Samples::Eight(v) => {
            let (w, h, v) = orient(ow, oh, channels, &v, orientation);
            (w, h, Samples::Eight(v))
        }
        Samples::Sixteen(v) => {
            let (w, h, v) = orient(ow, oh, channels, &v, orientation);
            (w, h, Samples::Sixteen(v))
        }
    };
    Ok(Pixels {
        width: w as u32,
        height: h as u32,
        channels,
        samples,
    })
}

/// The raster at its own size as linear-light BT.709 RGBA, premultiplied, unclipped (wide
/// gamut values come out negative or above 1 instead of clipping), through `to_linear`
/// (the source's profile). HDR keeps its light above 1 unless `tone_map`. Stored
/// orientation, rows top to bottom.
pub fn linear_rgba(
    raster: &Raster,
    to_linear: &ToLinear,
    tone_map: bool,
    cancel: &CancelToken,
) -> Result<Vec<f32>, Cancelled> {
    let rgb;
    let raster = match raster.layout {
        PixelLayout::Yuv(_) => {
            rgb = specular_core::yuv::to_rgb(raster).unwrap_or_else(|| raster.clone());
            &rgb
        }
        _ => raster,
    };
    let src = Source::new(raster, to_linear, tone_map);
    let (w, h) = (raster.width as usize, raster.height as usize);
    let mut out = vec![0f32; w * h * 4];
    for (y, row) in out.chunks_exact_mut(w * 4).enumerate() {
        if y % 64 == 0 && cancel.is_cancelled() {
            return Err(Cancelled);
        }
        src.row(y, row);
    }
    Ok(out)
}

/// sRGB curve table (encoded → linear) at 4096 intervals, interpolated.
fn srgb_table() -> &'static [f32] {
    static T: OnceLock<Vec<f32>> = OnceLock::new();
    T.get_or_init(|| {
        (0..=4096)
            .map(|i| hdr::srgb_to_linear(i as f32 / 4096.0))
            .collect()
    })
}

/// Linear interpolation in a table covering 0..=1.
fn interp(t: &[f32], x: f32) -> f32 {
    let n = (t.len() - 1) as f32;
    let p = x.clamp(0.0, 1.0) * n;
    let i = (p as usize).min(t.len() - 2);
    let f = p - i as f32;
    t[i] + (t[i + 1] - t[i]) * f
}

/// How a pixel's samples are stored.
#[derive(Clone, Copy)]
enum Kind {
    Gray,
    YCbCr,
    Rgb,
    Rgba,
    /// 16-bit samples coded like the 8-bit ones (sRGB curve or ICC profile).
    Rgba16Sdr,
    /// 16-bit samples with their own transfer and primaries.
    Rgba16(HdrEncoding),
    Rgba16F(Primaries),
    Gray16,
    GrayAlpha16,
}

/// Per output column (or row) of a subsampled plane: the two samples and the weight of the
/// second (centred chroma siting).
#[derive(Clone, Copy)]
struct Tap2 {
    i0: usize,
    i1: usize,
    f: f32,
}

fn tap2(n: usize, div: usize, size: usize) -> Vec<Tap2> {
    let last = size.max(1) - 1;
    (0..n)
        .map(|x| {
            if div <= 1 {
                let i = x.min(last);
                return Tap2 {
                    i0: i,
                    i1: i,
                    f: 0.0,
                };
            }
            let c = (x as f32 + 0.5) / div as f32 - 0.5;
            if c <= 0.0 {
                return Tap2 {
                    i0: 0,
                    i1: 0,
                    f: 0.0,
                };
            }
            let i0 = (c as usize).min(last);
            Tap2 {
                i0,
                i1: (i0 + 1).min(last),
                f: c - c.floor(),
            }
        })
        .collect()
}

struct Source<'a> {
    r: &'a Raster,
    kind: Kind,
    /// Encoded → linear, one table per channel (the sRGB curve, or the profile's curves).
    curves: [Vec<f32>; 3],
    /// Matrix after the curves (profile with matrix/TRC), rows with offsets.
    matrix: Option<[f32; 12]>,
    /// A 3D table instead of curves and matrix (LUT-based profiles), RGB f32, red fastest.
    cube: Option<Vec<[f32; 3]>>,
    /// Column / row taps of planes 1 and 2 (chroma) and 0.
    cols: Vec<Vec<Tap2>>,
    rows: Vec<Vec<Tap2>>,
    tone: Option<f32>,
    /// Float rasters: the factor their values are shown at ([`hdr::float_exposure`]).
    exposure: f32,
}

impl<'a> Source<'a> {
    fn new(r: &'a Raster, to_linear: &ToLinear, tone_map: bool) -> Source<'a> {
        let kind = match r.layout {
            PixelLayout::Gray => Kind::Gray,
            PixelLayout::YCbCr => Kind::YCbCr,
            PixelLayout::Rgb => Kind::Rgb,
            PixelLayout::Rgba => Kind::Rgba,
            PixelLayout::Rgba16(e) if e == HdrEncoding::SRGB => Kind::Rgba16Sdr,
            PixelLayout::Rgba16(e) => Kind::Rgba16(e),
            PixelLayout::Rgba16F(p) => Kind::Rgba16F(p),
            PixelLayout::Gray16 => Kind::Gray16,
            PixelLayout::GrayAlpha16 => Kind::GrayAlpha16,
            // Converted to R'G'B' in `render`; a malformed one reads as grey.
            PixelLayout::Yuv(_) => Kind::Gray,
        };
        let (exposure, peak) = hdr::float_exposure(r);
        let srgb = srgb_table().to_vec();
        let (mut curves, mut matrix, mut cube) = ([srgb.clone(), srgb.clone(), srgb], None, None);
        match to_linear {
            ToLinear::Srgb => {}
            ToLinear::Lut(SourceLut::Curves { data, matrix: m }) => {
                for (c, curve) in curves.iter_mut().enumerate() {
                    *curve = data
                        .chunks_exact(4)
                        .map(|t| hdr::f16_to_f32(t[c]))
                        .collect();
                }
                matrix = Some(*m);
            }
            ToLinear::Lut(SourceLut::Cube { data }) => {
                cube = Some(
                    data.chunks_exact(4)
                        .map(|t| [t[0], t[1], t[2]].map(hdr::f16_to_f32))
                        .collect(),
                );
            }
        }
        let (w, h) = (r.width as usize, r.height as usize);
        let cols = r
            .planes
            .iter()
            .map(|p| tap2(w, p.h_div as usize, p.width as usize))
            .collect();
        let rows = r
            .planes
            .iter()
            .map(|p| tap2(h, p.v_div as usize, p.height as usize))
            .collect();
        Source {
            r,
            kind,
            curves,
            matrix,
            cube,
            cols,
            rows,
            // Float data as the viewer shows it: its own peak (so 1.0 exports as white when
            // nothing is brighter; it came out 243 against a nominal 1000-nit peak) and an
            // exposure for data far above 1 (the visual review). PQ/HLG keep the
            // nominal peak.
            tone: match r.layout {
                PixelLayout::Rgba16F(_) => tone_map.then_some(peak),
                _ => (tone_map && r.layout.is_hdr()).then(|| hdr::peak_from_nits(None)),
            },
            exposure,
        }
    }

    /// Encoded (0..1) RGB → linear BT.709.
    fn linear(&self, e: [f32; 3]) -> [f32; 3] {
        if let Some(cube) = &self.cube {
            return trilinear(cube, e);
        }
        let l = [
            interp(&self.curves[0], e[0]),
            interp(&self.curves[1], e[1]),
            interp(&self.curves[2], e[2]),
        ];
        match self.matrix {
            None => l,
            Some(m) => [
                m[0] * l[0] + m[1] * l[1] + m[2] * l[2] + m[3],
                m[4] * l[0] + m[5] * l[1] + m[6] * l[2] + m[7],
                m[8] * l[0] + m[9] * l[1] + m[10] * l[2] + m[11],
            ],
        }
    }

    fn sample(&self, p: usize, x: usize, y: usize) -> f32 {
        let pl = &self.r.planes[p];
        let (c, r) = (self.cols[p][x], self.rows[p][y]);
        let at = |i: usize, j: usize| pl.data[j * pl.stride + i] as f32;
        if c.f == 0.0 && r.f == 0.0 {
            return at(c.i0, r.i0);
        }
        let top = at(c.i0, r.i0) * (1.0 - c.f) + at(c.i1, r.i0) * c.f;
        let bot = at(c.i0, r.i1) * (1.0 - c.f) + at(c.i1, r.i1) * c.f;
        top * (1.0 - r.f) + bot * r.f
    }

    /// Row `y` as linear premultiplied RGBA.
    fn row(&self, y: usize, out: &mut [f32]) {
        let w = self.r.width as usize;
        let p0 = &self.r.planes[0];
        let line = |bpp: usize| {
            let start = y * p0.stride;
            &p0.data[start..start + w * bpp]
        };
        let u16s = |b: &[u8]| -> Vec<u16> {
            b.chunks_exact(2)
                .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                .collect()
        };
        const K8: f32 = 1.0 / 255.0;
        const K16: f32 = 1.0 / 65535.0;
        for x in 0..w {
            let (rgb, a) = match self.kind {
                Kind::Gray => {
                    let v = self.sample(0, x, y) * K8;
                    (self.linear([v; 3]), 1.0)
                }
                Kind::YCbCr => {
                    let yy = self.sample(0, x, y);
                    let cb = self.sample(1, x, y) - 128.0;
                    let cr = self.sample(2, x, y) - 128.0;
                    let e = [
                        yy + 1.402 * cr,
                        yy - 0.344_136 * cb - 0.714_136 * cr,
                        yy + 1.772 * cb,
                    ]
                    .map(|v| (v * K8).clamp(0.0, 1.0));
                    (self.linear(e), 1.0)
                }
                Kind::Rgb => {
                    let e = [0, 1, 2].map(|p| self.sample(p, x, y) * K8);
                    (self.linear(e), 1.0)
                }
                Kind::Rgba => {
                    let px = &line(4)[x * 4..x * 4 + 4];
                    let e = [px[0], px[1], px[2]].map(|v| v as f32 * K8);
                    (self.linear(e), px[3] as f32 * K8)
                }
                Kind::Rgba16Sdr | Kind::Rgba16(_) | Kind::Rgba16F(_) => {
                    let s = u16s(&line(8)[x * 8..x * 8 + 8]);
                    let rgb = match self.kind {
                        Kind::Rgba16Sdr => self.linear([s[0], s[1], s[2]].map(|v| v as f32 * K16)),
                        Kind::Rgba16(e) => wide_linear(e, [s[0], s[1], s[2]]),
                        Kind::Rgba16F(p) => {
                            hdr::mat_mul(hdr::to_bt709(p), [s[0], s[1], s[2]].map(hdr::f16_to_f32))
                                .map(|v| {
                                    if v.is_finite() {
                                        v.max(0.0) * self.exposure
                                    } else {
                                        0.0
                                    }
                                })
                        }
                        _ => unreachable!(),
                    };
                    let a = if let Kind::Rgba16F(_) = self.kind {
                        hdr::f16_to_f32(s[3])
                    } else {
                        s[3] as f32 * K16
                    };
                    (rgb, a)
                }
                Kind::Gray16 => {
                    let s = u16s(&line(2)[x * 2..x * 2 + 2]);
                    (self.linear([s[0] as f32 * K16; 3]), 1.0)
                }
                Kind::GrayAlpha16 => {
                    let s = u16s(&line(4)[x * 4..x * 4 + 4]);
                    (self.linear([s[0] as f32 * K16; 3]), s[1] as f32 * K16)
                }
            };
            let rgb = match self.tone {
                Some(peak) => {
                    let rgb = rgb.map(|v| v.max(0.0));
                    let m = rgb[0].max(rgb[1]).max(rgb[2]);
                    let k = hdr::tone_map_scale(m, peak, 1.0);
                    rgb.map(|v| v * k)
                }
                None => rgb,
            };
            let a = if a.is_finite() {
                a.clamp(0.0, 1.0)
            } else {
                1.0
            };
            let o = &mut out[x * 4..x * 4 + 4];
            o[0] = rgb[0] * a;
            o[1] = rgb[1] * a;
            o[2] = rgb[2] * a;
            o[3] = a;
        }
    }
}

fn trilinear(cube: &[[f32; 3]], e: [f32; 3]) -> [f32; 3] {
    let n = specular_color::gpu::CUBE_SIZE;
    let s = (n - 1) as f32;
    let p = e.map(|v| v.clamp(0.0, 1.0) * s);
    let i = p.map(|v| (v as usize).min(n - 2));
    let f = [p[0] - i[0] as f32, p[1] - i[1] as f32, p[2] - i[2] as f32];
    let at = |r: usize, g: usize, b: usize| cube[(b * n + g) * n + r];
    let mut out = [0f32; 3];
    for (db, wb) in [(0, 1.0 - f[2]), (1, f[2])] {
        for (dg, wg) in [(0, 1.0 - f[1]), (1, f[1])] {
            for (dr, wr) in [(0, 1.0 - f[0]), (1, f[0])] {
                let w = wr * wg * wb;
                if w == 0.0 {
                    continue;
                }
                let v = at(i[0] + dr, i[1] + dg, i[2] + db);
                for c in 0..3 {
                    out[c] += w * v[c];
                }
            }
        }
    }
    out
}

/// Filter taps of one output sample: input indices `start..start + w.len()`.
pub struct Taps {
    pub start: usize,
    pub w: Vec<f32>,
}

fn lanczos3(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        return 1.0;
    }
    if x >= 3.0 {
        return 0.0;
    }
    let px = std::f32::consts::PI * x;
    3.0 * px.sin() * (px / 3.0).sin() / (px * px)
}

/// Lanczos-3 taps mapping `inp` samples onto `out` (pixel centres aligned, the kernel
/// stretched by the reduction factor when shrinking, edges clamped).
pub fn taps(inp: usize, out: usize) -> Vec<Taps> {
    let f = inp as f64 / out as f64;
    let fs = f.max(1.0);
    let r = 3.0 * fs;
    (0..out)
        .map(|o| {
            let c = (o as f64 + 0.5) * f - 0.5;
            let lo = (c - r).ceil() as isize;
            let hi = (c + r).floor() as isize;
            let clamp = |i: isize| i.clamp(0, inp as isize - 1) as usize;
            let start = clamp(lo);
            let end = clamp(hi);
            let mut w = vec![0f32; end - start + 1];
            for i in lo..=hi {
                let k = lanczos3(((i as f64 - c) / fs) as f32);
                w[clamp(i) - start] += k;
            }
            let sum: f32 = w.iter().sum();
            if sum.abs() > 1e-6 {
                w.iter_mut().for_each(|v| *v /= sum);
            }
            Taps { start, w }
        })
        .collect()
}

fn resample_row(row: &[f32], taps: &[Taps], out: &mut [f32]) {
    for (o, t) in out.chunks_exact_mut(4).zip(taps) {
        let mut a = [0f32; 4];
        for (j, &w) in t.w.iter().enumerate() {
            let p = (t.start + j) * 4;
            for c in 0..4 {
                a[c] += w * row[p + c];
            }
        }
        o.copy_from_slice(&a);
    }
}

/// Linear-light thresholds between consecutive 8-bit sRGB codes.
fn srgb8_thresholds() -> &'static [f32; 255] {
    static T: OnceLock<[f32; 255]> = OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|k| hdr::srgb_to_linear((k as f32 + 0.5) / 255.0)))
}

/// Linear (0..1) to the nearest 8-bit sRGB code.
pub(crate) fn encode8(v: f32) -> u8 {
    let t = srgb8_thresholds();
    t.partition_point(|&th| th <= v) as u8
}

fn encode16(v: f32) -> u16 {
    (hdr::linear_to_srgb(v.clamp(0.0, 1.0)) * 65535.0 + 0.5) as u16
}

/// 16-bit HDR samples to linear BT.709 in units of reference white, without clipping
/// what lies outside BT.709 (negative components) so that kept HDR keeps its gamut.
fn wide_linear(enc: HdrEncoding, rgb: [u16; 3]) -> [f32; 3] {
    let n = rgb.map(|v| v as f32 / 65535.0);
    let lin = match enc.transfer {
        hdr::Transfer::Srgb => n.map(hdr::srgb_to_linear),
        hdr::Transfer::Linear => n,
        hdr::Transfer::Pq => n.map(hdr::pq_to_linear),
        hdr::Transfer::Hlg => hdr::hlg_to_linear(n),
    };
    hdr::mat_mul(hdr::to_bt709(enc.primaries), lin)
}

/// Linear BT.709 in units of reference white → PQ-coded BT.2020, 16 bits.
fn encode_pq(rgb: [f32; 3]) -> [u16; 3] {
    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 4096.0 * 128.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 4096.0 * 32.0;
    const C3: f32 = 2392.0 / 4096.0 * 32.0;
    let wide = hdr::mat_mul(hdr::from_bt709(Primaries::Bt2020), rgb);
    wide.map(|v| {
        let y = (v * hdr::REFERENCE_WHITE_NITS / 10_000.0).clamp(0.0, 1.0);
        let p = y.powf(M1);
        let e = ((C1 + C2 * p) / (1.0 + C3 * p)).powf(M2);
        (e.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16
    })
}

struct Quantizer {
    target: Target,
    /// The source has alpha (flatten onto white when the output has none).
    src_alpha: bool,
    channels: usize,
    out8: Vec<u8>,
    out16: Vec<u16>,
    opaque: bool,
}

impl Quantizer {
    fn new(target: &Target, src_alpha: bool, w: usize, h: usize) -> Quantizer {
        let alpha = target.alpha && src_alpha;
        let channels = if target.gray { 1 } else { 3 } + usize::from(alpha);
        let n = w * h * channels;
        let (out8, out16) = if target.sixteen || target.transfer == OutTransfer::Pq {
            (Vec::new(), Vec::with_capacity(n))
        } else {
            (Vec::with_capacity(n), Vec::new())
        };
        Quantizer {
            target: Target { alpha, ..*target },
            src_alpha,
            channels,
            out8,
            out16,
            opaque: true,
        }
    }

    fn push(&mut self, row: &[f32]) {
        let t = self.target;
        let wide = t.sixteen || t.transfer == OutTransfer::Pq;
        for px in row.chunks_exact(4) {
            let a = px[3].clamp(0.0, 1.0);
            let mut rgb = [px[0], px[1], px[2]];
            if self.src_alpha && !t.alpha {
                // Over white, in linear light (premultiplied: c + (1 − a)·white).
                rgb = rgb.map(|c| c + (1.0 - a));
            } else if self.src_alpha {
                rgb = if a > 0.0 {
                    rgb.map(|c| c / a)
                } else {
                    [0.0; 3]
                };
            }
            // Kept HDR goes back to BT.2020 before clipping: colours outside BT.709 survive.
            let rgb = if t.transfer == OutTransfer::Pq && !t.gray {
                rgb.map(|v| if v.is_finite() { v } else { 0.0 })
            } else {
                rgb.map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 })
            };
            let gray = || 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
            if wide {
                match t.transfer {
                    OutTransfer::Pq => {
                        let c = encode_pq(rgb);
                        if t.gray {
                            self.out16.push(encode_pq([gray(); 3])[1]);
                        } else {
                            self.out16.extend_from_slice(&c);
                        }
                    }
                    OutTransfer::Srgb => {
                        if t.gray {
                            self.out16.push(encode16(gray()));
                        } else {
                            self.out16.extend(rgb.map(encode16));
                        }
                    }
                }
                if t.alpha {
                    let q = (a * 65535.0 + 0.5) as u16;
                    self.opaque &= q == 65535;
                    self.out16.push(q);
                }
            } else {
                if t.gray {
                    self.out8.push(encode8(gray()));
                } else {
                    self.out8.extend(rgb.map(encode8));
                }
                if t.alpha {
                    let q = (a * 255.0 + 0.5) as u8;
                    self.opaque &= q == 255;
                    self.out8.push(q);
                }
            }
        }
    }

    /// The channel count and samples, alpha dropped when everything is opaque.
    fn finish(self) -> (usize, Samples) {
        let wide = self.target.sixteen || self.target.transfer == OutTransfer::Pq;
        let (mut c, mut s8, mut s16) = (self.channels, self.out8, self.out16);
        if self.target.alpha && self.opaque {
            let keep = c - 1;
            if wide {
                s16 = s16
                    .chunks_exact(c)
                    .flat_map(|p| p[..keep].iter().copied())
                    .collect();
            } else {
                s8 = s8
                    .chunks_exact(c)
                    .flat_map(|p| p[..keep].iter().copied())
                    .collect();
            }
            c = keep;
        }
        (
            c,
            if wide {
                Samples::Sixteen(s16)
            } else {
                Samples::Eight(s8)
            },
        )
    }
}

/// Applies an EXIF orientation to interleaved samples (5 is the transpose, 7 the
/// transverse).
pub(crate) fn orient<T: Copy + Default>(
    w: usize,
    h: usize,
    c: usize,
    src: &[T],
    o: Orientation,
) -> (usize, usize, Vec<T>) {
    if o == Orientation::NORMAL || o.0 == 0 || o.0 > 8 {
        return (w, h, src.to_vec());
    }
    let (dw, dh) = if o.swaps_axes() { (h, w) } else { (w, h) };
    let mut out = vec![T::default(); src.len()];
    for y in 0..h {
        for x in 0..w {
            // Where stored (x, y) is displayed, straight from the EXIF definitions.
            let (dx, dy) = match o.0 {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (h - 1 - y, x),
                7 => (h - 1 - y, w - 1 - x),
                8 => (y, w - 1 - x),
                _ => (x, y),
            };
            let s = (y * w + x) * c;
            let d = (dy * dw + dx) * c;
            out[d..d + c].copy_from_slice(&src[s..s + c]);
        }
    }
    (dw, dh, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::RasterPlane;

    fn rgba_raster(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Raster {
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend(f(x, y));
            }
        }
        Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba,
            planes: vec![RasterPlane {
                width: w,
                height: h,
                stride: w as usize * 4,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        }
    }

    fn target(w: u32, h: u32) -> Target {
        Target {
            width: w,
            height: h,
            gray: false,
            alpha: true,
            sixteen: false,
            transfer: OutTransfer::Srgb,
            tone_map: true,
        }
    }

    fn eight(p: &Pixels) -> &[u8] {
        match &p.samples {
            Samples::Eight(v) => v,
            Samples::Sixteen(_) => panic!("16-bit"),
        }
    }

    /// A half-float raster of `w`×1 pixels, grey `f(x)`.
    fn half_row(w: u32, f: impl Fn(u32) -> f32) -> Raster {
        let mut data = Vec::new();
        for x in 0..w {
            let v = specular_color::math::f16_bits(f(x));
            for c in [v, v, v, specular_color::math::f16_bits(1.0)] {
                data.extend(c.to_ne_bytes());
            }
        }
        Raster {
            width: w,
            height: 1,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba16F(hdr::Primaries::Bt709),
            planes: vec![RasterPlane {
                width: w,
                height: 1,
                stride: w as usize * 8,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        }
    }

    /// Float images as the viewer shows them (the visual review): diffuse white
    /// 1.0 with nothing brighter exports as 255 (it was 243 against a nominal 1000-nit
    /// peak), and data far above 1 (a PFM of 0–255 values, all white before) is exposed down.
    #[test]
    fn float_white_and_auto_exposure() {
        let render1 = |r: &Raster| {
            let p = render(
                r,
                &ToLinear::Srgb,
                &target(r.width, 1),
                Orientation::NORMAL,
                &CancelToken::new(),
            )
            .ok()
            .unwrap();
            eight(&p).to_vec()
        };
        let white = render1(&half_row(4, |_| 1.0));
        assert_eq!(&white[..3], &[255, 255, 255]);
        let ramp = render1(&half_row(256, |x| x as f32));
        let mid = ramp[128 * 4];
        assert!(mid > 60 && mid < 230, "mid grey {mid}");
        assert!(ramp[4] < 40, "near black {}", ramp[4]);
        // Light above white keeps exposure 1 and is tone-mapped from its own peak.
        let hdr = render1(&half_row(64, |x| x as f32 / 16.0));
        assert!(
            hdr[16 * 4] > 200 && hdr[16 * 4] < 255,
            "1.0 under a 4.0 peak: {}",
            hdr[16 * 4]
        );
    }

    #[test]
    fn encode8_is_exact() {
        for k in 0..=255u8 {
            let lin = hdr::srgb_to_linear(k as f32 / 255.0);
            assert_eq!(encode8(lin), k);
        }
        assert_eq!(encode8(-1.0), 0);
        assert_eq!(encode8(2.0), 255);
    }

    #[test]
    fn identity_is_lossless_and_opaque_alpha_drops() {
        let r = rgba_raster(7, 5, |x, y| [(x * 30) as u8, (y * 50) as u8, 200, 255]);
        let p = render(
            &r,
            &ToLinear::Srgb,
            &target(7, 5),
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        assert_eq!(p.channels, 3);
        let s = eight(&p);
        for y in 0..5 {
            for x in 0..7 {
                let i = (y * 7 + x) * 3;
                assert_eq!(&s[i..i + 3], &[(x * 30) as u8, (y * 50) as u8, 200]);
            }
        }
    }

    #[test]
    fn downscale_averages_in_linear_light() {
        // A fine black/white checkerboard halves to the linear mean (sRGB 188), not to 128.
        let r = rgba_raster(64, 64, |x, y| {
            let v = if (x + y) % 2 == 0 { 255 } else { 0 };
            [v, v, v, 255]
        });
        let p = render(
            &r,
            &ToLinear::Srgb,
            &target(16, 16),
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        assert_eq!((p.width, p.height), (16, 16));
        for &v in eight(&p) {
            // (Lanczos lets a little of the pattern's frequency through.)
            assert!((184..=192).contains(&v), "{v}");
        }
        // Flat colour stays flat (weights normalised, edges clamped).
        let r = rgba_raster(50, 30, |_, _| [10, 120, 240, 255]);
        let p = render(
            &r,
            &ToLinear::Srgb,
            &target(17, 11),
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        for px in eight(&p).chunks_exact(3) {
            assert_eq!(px, [10, 120, 240]);
        }
    }

    #[test]
    fn transparency_kept_or_flattened() {
        let r = rgba_raster(4, 4, |x, _| {
            if x < 2 {
                [255, 0, 0, 0]
            } else {
                [0, 0, 255, 255]
            }
        });
        let p = render(
            &r,
            &ToLinear::Srgb,
            &target(4, 4),
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        assert_eq!(p.channels, 4);
        assert_eq!(&eight(&p)[..8], &[0, 0, 0, 0, 0, 0, 0, 0]);
        let mut t = target(4, 4);
        t.alpha = false;
        let p = render(
            &r,
            &ToLinear::Srgb,
            &t,
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        assert_eq!(p.channels, 3);
        assert_eq!(&eight(&p)[..6], &[255, 255, 255, 255, 255, 255]);
        assert_eq!(&eight(&p)[6..9], &[0, 0, 255]);
    }

    #[test]
    fn orientation_matches_the_view() {
        // 3×2: a b c / d e f.
        let src: Vec<u8> = (0..6).collect();
        let (w, h, o) = orient(3, 2, 1, &src, Orientation(6));
        // 90° clockwise: d a / e b / f c.
        assert_eq!((w, h), (2, 3));
        assert_eq!(o, [3, 0, 4, 1, 5, 2]);
        let (_, _, o) = orient(3, 2, 1, &src, Orientation(8));
        assert_eq!(o, [2, 5, 1, 4, 0, 3]);
        let (_, _, o) = orient(3, 2, 1, &src, Orientation(3));
        assert_eq!(o, [5, 4, 3, 2, 1, 0]);
        let (_, _, o) = orient(3, 2, 1, &src, Orientation(2));
        assert_eq!(o, [2, 1, 0, 5, 4, 3]);
        // Transpose (5): a d / b e / c f; transverse (7): f c / e b / d a.
        let (_, _, o) = orient(3, 2, 1, &src, Orientation(5));
        assert_eq!(o, [0, 3, 1, 4, 2, 5]);
        let (_, _, o) = orient(3, 2, 1, &src, Orientation(7));
        assert_eq!(o, [5, 2, 4, 1, 3, 0]);
        let (_, _, o) = orient(3, 2, 1, &src, Orientation(4));
        assert_eq!(o, [3, 4, 5, 0, 1, 2]);
    }

    #[test]
    fn ycbcr_420_and_hdr_paths() {
        // 4:2:0 neutral grey: Y 100, Cb = Cr = 128 everywhere.
        let plane = |w: u32, h: u32, v: u8, d: u8| RasterPlane {
            width: w,
            height: h,
            stride: w as usize,
            data: vec![v; (w * h) as usize],
            h_div: d,
            v_div: d,
        };
        let r = Raster {
            width: 5,
            height: 3,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::YCbCr,
            planes: vec![
                plane(5, 3, 100, 1),
                plane(3, 2, 128, 2),
                plane(3, 2, 128, 2),
            ],
            truncated: false,
        };
        let mut t = target(5, 3);
        t.gray = true;
        let p = render(
            &r,
            &ToLinear::Srgb,
            &t,
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        assert_eq!(p.channels, 1);
        assert!(eight(&p).iter().all(|&v| v == 100));

        // A PQ pixel at 1000 nits: tone-mapped to SDR white-ish, or kept as PQ.
        let pq1000 = (0.751_827_f32 * 65535.0) as u16;
        let mut data = Vec::new();
        for v in [pq1000, pq1000, pq1000, 65535] {
            data.extend(v.to_ne_bytes());
        }
        let r = Raster {
            width: 1,
            height: 1,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba16(HdrEncoding {
                transfer: hdr::Transfer::Pq,
                primaries: Primaries::Bt2020,
            }),
            planes: vec![RasterPlane {
                width: 1,
                height: 1,
                stride: 8,
                data,
                h_div: 1,
                v_div: 1,
            }],
            truncated: false,
        };
        let p = render(
            &r,
            &ToLinear::Srgb,
            &target(1, 1),
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        assert!(eight(&p).iter().all(|&v| v >= 250), "{:?}", p.samples);
        let mut t = target(1, 1);
        t.tone_map = false;
        t.transfer = OutTransfer::Pq;
        let p = render(
            &r,
            &ToLinear::Srgb,
            &t,
            Orientation::NORMAL,
            &CancelToken::new(),
        )
        .ok()
        .unwrap();
        let Samples::Sixteen(s) = &p.samples else {
            panic!()
        };
        for &v in s {
            assert!((v as i32 - pq1000 as i32).abs() < 40, "{v} vs {pq1000}");
        }
    }

    #[test]
    fn cancel_stops() {
        let r = rgba_raster(64, 256, |_, _| [1, 2, 3, 255]);
        let c = CancelToken::new();
        c.cancel();
        assert!(
            render(
                &r,
                &ToLinear::Srgb,
                &target(32, 128),
                Orientation::NORMAL,
                &c
            )
            .is_err()
        );
    }
}

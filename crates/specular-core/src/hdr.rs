//! High dynamic range: sample encodings, transfer functions, gain maps and the tone-mapping
//! operator.
//!
//! Scale convention used everywhere in Specular: linear light with **1.0 = SDR reference
//! white = 203 cd/m²** (ITU-R BT.2408). A display's *headroom* is its peak in the same unit
//! (1.0 for an SDR display, 2.0 for a display that can show twice SDR white, ...).
//!
//! The functions here are the CPU reference for the GPU shaders (`convert.wgsl`,
//! `image.wgsl` in `specular-render`); unit tests pin both to the same numbers.

use crate::{PixelLayout, Raster};

/// SDR reference white in cd/m² (BT.2408). PQ and HLG signals are divided by this.
pub const REFERENCE_WHITE_NITS: f32 = 203.0;

/// Content peak assumed for PQ/HLG images without light-level metadata (cd/m²).
pub const DEFAULT_HDR_PEAK_NITS: f32 = 1000.0;

/// Transfer function of 16-bit integer samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transfer {
    /// The sRGB curve (IEC 61966-2-1).
    Srgb,
    /// Linear light, 1.0 = reference white.
    Linear,
    /// SMPTE ST 2084 perceptual quantiser (absolute: 1.0 = 10000 cd/m²).
    Pq,
    /// ARIB STD-B67 / BT.2100 hybrid log-gamma (relative, 1000 cd/m² reference display).
    Hlg,
}

/// Colour primaries (all with a D65 white point).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primaries {
    /// BT.709 / sRGB.
    Bt709,
    /// Display P3.
    DisplayP3,
    /// BT.2020 / BT.2100.
    Bt2020,
}

/// How the samples of a high-bit-depth raster are encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HdrEncoding {
    pub transfer: Transfer,
    pub primaries: Primaries,
}

impl HdrEncoding {
    pub const SRGB: HdrEncoding = HdrEncoding {
        transfer: Transfer::Srgb,
        primaries: Primaries::Bt709,
    };
    pub const LINEAR: HdrEncoding = HdrEncoding {
        transfer: Transfer::Linear,
        primaries: Primaries::Bt709,
    };

    /// Whether values above reference white can occur.
    pub fn is_hdr(self) -> bool {
        matches!(
            self.transfer,
            Transfer::Pq | Transfer::Hlg | Transfer::Linear
        )
    }

    /// Compact code (for caches): transfer in bits 2..4, primaries in bits 0..2.
    pub fn code(self) -> u8 {
        let t = match self.transfer {
            Transfer::Srgb => 0,
            Transfer::Linear => 1,
            Transfer::Pq => 2,
            Transfer::Hlg => 3,
        };
        t << 2 | primaries_code(self.primaries)
    }

    /// Inverse of [`HdrEncoding::code`]; `None` for codes it never produces.
    pub fn from_code(c: u8) -> Option<HdrEncoding> {
        if c > 15 {
            return None;
        }
        let transfer = match c >> 2 {
            0 => Transfer::Srgb,
            1 => Transfer::Linear,
            2 => Transfer::Pq,
            _ => Transfer::Hlg,
        };
        Some(HdrEncoding {
            transfer,
            primaries: primaries_of(c & 3)?,
        })
    }
}

/// Compact code of a primaries value (0..=2).
pub fn primaries_code(p: Primaries) -> u8 {
    match p {
        Primaries::Bt709 => 0,
        Primaries::DisplayP3 => 1,
        Primaries::Bt2020 => 2,
    }
}

/// Inverse of [`primaries_code`].
pub fn primaries_of(c: u8) -> Option<Primaries> {
    Some(match c {
        0 => Primaries::Bt709,
        1 => Primaries::DisplayP3,
        2 => Primaries::Bt2020,
        _ => return None,
    })
}

/// Row-major 3×3 matrix converting linear RGB in `p` to linear BT.709.
pub fn to_bt709(p: Primaries) -> [[f32; 3]; 3] {
    match p {
        Primaries::Bt709 => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        Primaries::DisplayP3 => [
            [1.224_94, -0.224_94, 0.0],
            [-0.042_057, 1.042_057, 0.0],
            [-0.019_638, -0.078_636, 1.098_274],
        ],
        Primaries::Bt2020 => [
            [1.660_491, -0.587_641, -0.072_850],
            [-0.124_551, 1.132_9, -0.008_349],
            [-0.018_151, -0.100_579, 1.118_73],
        ],
    }
}

/// Row-major 3×3 matrix converting linear BT.709 to linear RGB in `p` (inverse of
/// [`to_bt709`]).
pub fn from_bt709(p: Primaries) -> [[f32; 3]; 3] {
    let m: M3 = to_bt709(p).map(|r| r.map(f64::from));
    m3_inv(m)
        .expect("primaries matrices are invertible")
        .map(|r| r.map(|v| v as f32))
}

/// IEEE 754 binary16 bits to f32 (exact).
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h as u32) & 0x8000) << 16;
    let exp = ((h >> 10) & 0x1f) as u32;
    let man = (h & 0x3ff) as u32;
    let bits = match exp {
        0 => {
            if man == 0 {
                sign
            } else {
                // Subnormal: value = man × 2^-24.
                let v = man as f32 * (1.0 / 16_777_216.0);
                return if sign != 0 { -v } else { v };
            }
        }
        31 => sign | 0x7f80_0000 | (man << 13),
        _ => sign | ((exp + 112) << 23) | (man << 13),
    };
    f32::from_bits(bits)
}

/// f32 to IEEE 754 binary16 bits, rounding to nearest even; overflow gives infinity.
pub fn f32_to_f16(f: f32) -> u16 {
    let x = f.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let man = x & 0x7f_ffff;
    if exp == 0xff {
        // Inf or NaN (keep NaN quiet and non-zero).
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 31 {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // Subnormal half: shift the full mantissa (with the implicit 1) into place.
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let rest = m & ((1 << shift) - 1);
        let mut v = m >> shift;
        if rest > half || (rest == half && v & 1 == 1) {
            v += 1;
        }
        return sign | v as u16;
    }
    let mut v = ((e as u32) << 10) | (man >> 13);
    let rest = man & 0x1fff;
    if rest > 0x1000 || (rest == 0x1000 && v & 1 == 1) {
        v += 1; // may carry into the exponent, which is the correct rounding
    }
    sign | v as u16
}

/// sRGB electro-optical transfer (encoded 0..1 to linear).
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Inverse of [`srgb_to_linear`]; values above 1 are extended with the same curve.
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// PQ (ST 2084) signal 0..1 to linear in units of reference white.
pub fn pq_to_linear(e: f32) -> f32 {
    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 4096.0 * 128.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 4096.0 * 32.0;
    const C3: f32 = 2392.0 / 4096.0 * 32.0;
    let p = e.clamp(0.0, 1.0).powf(1.0 / M2);
    let y = ((p - C1).max(0.0) / (C2 - C3 * p)).powf(1.0 / M1);
    y * (10_000.0 / REFERENCE_WHITE_NITS)
}

/// HLG inverse OETF: signal 0..1 to normalised scene light 0..1.
pub fn hlg_inverse_oetf(e: f32) -> f32 {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_7;
    let e = e.clamp(0.0, 1.0);
    if e <= 0.5 {
        e * e / 3.0
    } else {
        (((e - C) / A).exp() + B) / 12.0
    }
}

/// HLG signal (BT.2020 RGB) to display linear light in units of reference white, using the
/// BT.2100 OOTF for a 1000 cd/m² display (system gamma 1.2).
pub fn hlg_to_linear(rgb: [f32; 3]) -> [f32; 3] {
    let s = rgb.map(hlg_inverse_oetf);
    let ys = 0.2627 * s[0] + 0.6780 * s[1] + 0.0593 * s[2];
    let k = DEFAULT_HDR_PEAK_NITS / REFERENCE_WHITE_NITS * ys.max(0.0).powf(0.2);
    s.map(|c| c * k)
}

/// Decode a 16-bit sample triple to linear BT.709 in units of reference white.
pub fn decode16(enc: HdrEncoding, rgb: [u16; 3]) -> [f32; 3] {
    let n = rgb.map(|v| v as f32 / 65535.0);
    let lin = match enc.transfer {
        Transfer::Srgb => n.map(srgb_to_linear),
        Transfer::Linear => n,
        Transfer::Pq => n.map(pq_to_linear),
        Transfer::Hlg => hlg_to_linear(n),
    };
    mat_mul(to_bt709(enc.primaries), lin).map(|v| v.max(0.0))
}

pub fn mat_mul(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

type M3 = [[f64; 3]; 3];

fn m3_mul(a: M3, b: M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn m3_inv(m: M3) -> Option<M3> {
    let c = |r: usize, s: usize| {
        let (r1, r2, s1, s2) = ((r + 1) % 3, (r + 2) % 3, (s + 1) % 3, (s + 2) % 3);
        m[r1][s1] * m[r2][s2] - m[r1][s2] * m[r2][s1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    // inv[i][j] = cofactor(j, i) / det.
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| c(j, i) / det)
    }))
}

fn xy_to_xyz([x, y]: [f64; 2]) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

/// RGB → XYZ for primaries and white given as CIE xy (`[r, g, b, white]`).
fn rgb_to_xyz(c: [[f64; 2]; 4]) -> Option<M3> {
    if c.iter()
        .any(|p| !p[0].is_finite() || !p[1].is_finite() || p[1] <= 1e-6)
    {
        return None;
    }
    let p = [xy_to_xyz(c[0]), xy_to_xyz(c[1]), xy_to_xyz(c[2])];
    let m: M3 = std::array::from_fn(|i| [p[0][i], p[1][i], p[2][i]]);
    let w = xy_to_xyz(c[3]);
    let inv = m3_inv(m)?;
    let s: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| inv[i][k] * w[k]).sum());
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| m[i][j] * s[j])
    }))
}

const BT709_XY: [[f64; 2]; 4] = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06], [0.3127, 0.3290]];

/// Row-major matrix taking linear RGB with arbitrary chromaticities (CIE xy of red, green,
/// blue and white, as in OpenEXR's `chromaticities` or Radiance's `PRIMARIES`) to linear
/// BT.709 / D65, adapting the white point with the Bradford transform. `None` for degenerate
/// input. Identity (within rounding) for BT.709 chromaticities.
pub fn chromaticities_to_bt709(c: [[f32; 2]; 4]) -> Option<[[f32; 3]; 3]> {
    let c: [[f64; 2]; 4] = c.map(|p| p.map(f64::from));
    let src = rgb_to_xyz(c)?;
    let dst_inv = m3_inv(rgb_to_xyz(BT709_XY)?)?;
    const BRADFORD: M3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let cone = |w: [f64; 2]| {
        let w = xy_to_xyz(w);
        let r: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| BRADFORD[i][k] * w[k]).sum());
        r
    };
    let (ws, wd) = (cone(c[3]), cone(BT709_XY[3]));
    let scale: M3 =
        std::array::from_fn(|i| std::array::from_fn(|j| if i == j { wd[i] / ws[i] } else { 0.0 }));
    let adapt = m3_mul(m3_inv(BRADFORD)?, m3_mul(scale, BRADFORD));
    let m = m3_mul(dst_inv, m3_mul(adapt, src));
    m.iter()
        .flatten()
        .all(|v| v.is_finite())
        .then(|| m.map(|r| r.map(|v| v as f32)))
}

/// CIE XYZ (D65-relative, Y = 1 white) → linear BT.709.
pub fn xyz_to_bt709() -> [[f32; 3]; 3] {
    let inv = m3_inv(rgb_to_xyz(BT709_XY).expect("BT.709")).expect("invertible");
    inv.map(|r| r.map(|v| v as f32))
}

/// The [`Primaries`] matching chromaticities within `tolerance` in xy, if any.
pub fn primaries_matching(c: [[f32; 2]; 4], tolerance: f32) -> Option<Primaries> {
    const D65: [f32; 2] = [0.3127, 0.3290];
    let known = [
        (
            Primaries::Bt709,
            [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06], D65],
        ),
        (
            Primaries::DisplayP3,
            [[0.68, 0.32], [0.265, 0.69], [0.15, 0.06], D65],
        ),
        (
            Primaries::Bt2020,
            [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046], D65],
        ),
    ];
    known.into_iter().find_map(|(p, k)| {
        let close = c
            .iter()
            .zip(&k)
            .all(|(a, b)| (a[0] - b[0]).abs() <= tolerance && (a[1] - b[1]).abs() <= tolerance);
        close.then_some(p)
    })
}

/// Median of the brightest channel (four stops over diffuse white) above which a float image
/// is taken for data in other units (0–255 or counts) rather than light.
pub const AUTO_EXPOSURE_MEDIAN: f32 = 16.0;

/// How a float ([`PixelLayout::Rgba16F`]) raster is shown: the exposure factor and the
/// content peak after it (units of reference white, at least 1). Light (1.0 = diffuse white)
/// keeps exposure 1 and its own peak, so an image within 0–1 is not tone-mapped and 1.0
/// shows as white. Data whose median is above [`AUTO_EXPOSURE_MEDIAN`] (a PFM of 0–255
/// values showed all white) is scaled so its 99.5th percentile is reference white.
/// Other layouts: `(1, 1)`.
pub fn float_exposure(r: &Raster) -> (f32, f32) {
    if !matches!(r.layout, PixelLayout::Rgba16F(_)) {
        return (1.0, 1.0);
    }
    let Some(p) = r.planes.first() else {
        return (1.0, 1.0);
    };
    let (w, h) = (r.width as usize, r.height as usize);
    let n = w * h;
    if n == 0 || p.stride < w * 8 {
        return (1.0, 1.0);
    }
    let step = n.div_ceil(1 << 16).max(1);
    let mut peak = 0f32;
    let mut s: Vec<f32> = Vec::with_capacity(n / step + 1);
    for i in (0..n).step_by(step) {
        let (y, x) = (i / w, i % w);
        let o = y * p.stride + x * 8;
        let Some(px) = p.data.get(o..o + 6) else {
            continue;
        };
        let m = (0..3)
            .map(|c| f16_to_f32(u16::from_ne_bytes([px[2 * c], px[2 * c + 1]])))
            .filter(|v| v.is_finite())
            .fold(0f32, f32::max);
        peak = peak.max(m);
        s.push(m);
    }
    // The peak over every pixel (the sample above may miss a highlight).
    for line in p.data.chunks(p.stride).take(h) {
        for px in line[..(w * 8).min(line.len())].chunks_exact(8) {
            for c in 0..3 {
                let v = f16_to_f32(u16::from_ne_bytes([px[2 * c], px[2 * c + 1]]));
                if v.is_finite() {
                    peak = peak.max(v);
                }
            }
        }
    }
    if s.is_empty() {
        return (1.0, 1.0);
    }
    s.sort_by(f32::total_cmp);
    let median = s[s.len() / 2];
    let exposure = if median > AUTO_EXPOSURE_MEDIAN {
        let p995 = s[((s.len() - 1) as f32 * 0.995).round() as usize];
        if p995 > 0.0 { 1.0 / p995 } else { 1.0 }
    } else {
        1.0
    };
    (exposure, (peak * exposure).max(1.0))
}

/// Content peak (units of reference white) for a light level in cd/m², or the default.
pub fn peak_from_nits(nits: Option<f32>) -> f32 {
    let n = nits
        .filter(|n| n.is_finite() && *n > 0.0)
        .unwrap_or(DEFAULT_HDR_PEAK_NITS);
    (n / REFERENCE_WHITE_NITS).max(1.0)
}

/// The tone-mapping operator, applied to the largest of R, G, B (so hue and saturation are
/// kept) only when the content peak `content` exceeds the display peak `display`:
///
/// * below the knee `k = 0.8 · display` values pass through unchanged;
/// * above it, `y = k + t / (1 + t·(1/s − 1/r))` with `t = x − k`, `s = display − k`,
///   `r = content − k` — a Reinhard-style shoulder with slope 1 at the knee (C1-continuous)
///   that maps the content peak exactly onto the display peak. Values above the declared
///   peak are clipped to the display peak.
///
/// Returns the factor to multiply the RGB triple by.
pub fn tone_map_scale(max_rgb: f32, content: f32, display: f32) -> f32 {
    if content <= display || max_rgb <= 0.0 {
        return 1.0;
    }
    let k = 0.8 * display;
    if max_rgb <= k {
        return 1.0;
    }
    let t = max_rgb - k;
    let s = display - k;
    let r = content - k;
    let y = (k + t / (1.0 + t * (1.0 / s - 1.0 / r))).min(display);
    y / max_rgb
}

/// One wide pixel ([`PixelLayout::pixel_bytes`] bytes of a wide plane: 8 of
/// [`PixelLayout::Rgba16`] / [`PixelLayout::Rgba16F`], 2 of [`PixelLayout::Gray16`], 4 of
/// [`PixelLayout::GrayAlpha16`]) to straight 8-bit sRGB RGBA, tone-mapped for an SDR display
/// from `content_peak` (units of reference white) when the layout is HDR; SDR layouts (16-bit
/// sRGB-coded samples) are only re-encoded. Other layouts give black.
pub fn wide_to_srgb8(layout: PixelLayout, px: &[u8], content_peak: f32) -> [u8; 4] {
    wide_to_srgb8_exposed(layout, px, content_peak, 1.0)
}

/// [`wide_to_srgb8`] with the linear values multiplied by `exposure` first (float rasters:
/// [`float_exposure`]; `content_peak` is then the peak after it).
pub fn wide_to_srgb8_exposed(
    layout: PixelLayout,
    px: &[u8],
    content_peak: f32,
    exposure: f32,
) -> [u8; 4] {
    let peak = if layout.is_hdr() { content_peak } else { 1.0 };
    match wide_linear(layout, px) {
        Some((rgb, a)) => linear_to_srgb8(rgb.map(|v| v * exposure), a, peak),
        None => [0, 0, 0, 255],
    }
}

/// One wide pixel as linear BT.709 RGB (1.0 = reference white, not negative) and alpha
/// (0..1); `None` for 8-bit layouts.
fn wide_linear(layout: PixelLayout, px: &[u8]) -> Option<([f32; 3], f32)> {
    let u = |i: usize| u16::from_ne_bytes([px[2 * i], px[2 * i + 1]]);
    Some(match layout {
        PixelLayout::Rgba16(e) => (decode16(e, [u(0), u(1), u(2)]), u(3) as f32 / 65535.0),
        PixelLayout::Gray16 => (decode16(HdrEncoding::SRGB, [u(0); 3]), 1.0),
        PixelLayout::GrayAlpha16 => (
            decode16(HdrEncoding::SRGB, [u(0); 3]),
            u(1) as f32 / 65535.0,
        ),
        PixelLayout::Rgba16F(p) => {
            let lin = [u(0), u(1), u(2)].map(|h| {
                let v = f16_to_f32(h);
                if v.is_finite() { v } else { 0.0 }
            });
            (
                mat_mul(to_bt709(p), lin).map(|v| v.max(0.0)),
                f16_to_f32(u(3)).clamp(0.0, 1.0),
            )
        }
        _ => return None,
    })
}

/// Linear BT.709 RGB and alpha to 8-bit sRGB RGBA, tone-mapped for an SDR display.
fn linear_to_srgb8(rgb: [f32; 3], a: f32, content_peak: f32) -> [u8; 4] {
    let m = rgb[0].max(rgb[1]).max(rgb[2]);
    let k = tone_map_scale(m, content_peak, 1.0);
    let q = |v: f32| (linear_to_srgb((v * k).clamp(0.0, 1.0)) * 255.0 + 0.5) as u8;
    [q(rgb[0]), q(rgb[1]), q(rgb[2]), (a * 255.0 + 0.5) as u8]
}

/// The SDR rendition of a wide base with a gain map (thumbnails): every pixel linearised,
/// the map applied at the weight for an SDR display ([`GainMapParams::weight`] of 1.0, so
/// −1 for an HDR base: the full map towards the SDR alternate), what is left above
/// reference white tone mapped as the renderer does, then 8-bit sRGB RGBA. The map is
/// sampled bilinearly over the whole image, as the shader samples it; `base` must be a
/// whole-image raster (any scale). `content_peak` is the base's own peak (units of
/// reference white). `None` unless `base` is one wide plane and the map RGBA8.
pub fn sdr_rendition(base: &Raster, gm: &GainMap, content_peak: f32) -> Option<Raster> {
    let (bp, mp) = (base.planes.first()?, gm.map.planes.first()?);
    let (w, h) = (base.width as usize, base.height as usize);
    let (mw, mh) = (gm.map.width as usize, gm.map.height as usize);
    let bpp = base.layout.pixel_bytes();
    if !base.layout.is_wide()
        || gm.map.layout != PixelLayout::Rgba
        || mw == 0
        || mh == 0
        || bp.stride < w * bpp
        || bp.data.len() < bp.stride * h.saturating_sub(1) + w * bpp
        || mp.stride < mw * 4
        || mp.data.len() < mp.stride * (mh - 1) + mw * 4
    {
        return None;
    }
    let p = &gm.params;
    let weight = p.weight(1.0);
    let peak = p.peak(content_peak, weight);
    // Bilinear taps of one axis: (index 0, index 1, fraction of index 1).
    let taps = |n: usize, m: usize| -> Vec<(usize, usize, f32)> {
        (0..n)
            .map(|i| {
                let c = ((i as f32 + 0.5) * m as f32 / n as f32 - 0.5).clamp(0.0, (m - 1) as f32);
                let i0 = c as usize;
                (i0, (i0 + 1).min(m - 1), c - i0 as f32)
            })
            .collect()
    };
    let (tx, ty) = (taps(w, mw), taps(h, mh));
    let map = |x: usize, y: usize, c: usize| f32::from(mp.data[y * mp.stride + x * 4 + c]) / 255.0;
    let mut out = vec![0u8; w * h * 4];
    for (y, row) in out.chunks_exact_mut(w * 4).enumerate() {
        let (y0, y1, fy) = ty[y];
        let src = &bp.data[y * bp.stride..];
        for (x, o) in row.chunks_exact_mut(4).enumerate() {
            let (rgb, a) = wide_linear(base.layout, &src[x * bpp..(x + 1) * bpp])?;
            let (x0, x1, fx) = tx[x];
            let g = std::array::from_fn(|c| {
                let top = map(x0, y0, c) * (1.0 - fx) + map(x1, y0, c) * fx;
                let bot = map(x0, y1, c) * (1.0 - fx) + map(x1, y1, c) * fx;
                top * (1.0 - fy) + bot * fy
            });
            o.copy_from_slice(&linear_to_srgb8(p.apply_rgb(rgb, g, weight), a, peak));
        }
    }
    Some(Raster {
        width: base.width,
        height: base.height,
        origin: base.origin,
        scale: base.scale,
        layout: PixelLayout::Rgba,
        planes: vec![crate::RasterPlane {
            width: base.width,
            height: base.height,
            stride: w * 4,
            data: out,
            h_div: 1,
            v_div: 1,
        }],
        truncated: base.truncated,
    })
}

/// Gain-map metadata (ISO 21496-1 / Adobe hdrgm / Apple), per channel, in the units the
/// formulas want.
///
/// Applying the map to a linear base pixel `b` (per channel, in the application space
/// [`GainMapParams::space`]):
///
/// ```text
/// g' = g^(1/gamma)                       (g = the map sample, 0..1)
/// L  = mix(gain_min, gain_max, g')       (log2 units)
/// out = (b + base_offset) · 2^(L·W) − alt_offset
/// W  = sign(A − B) · clamp((log2 H − B) / (A − B), 0, 1)
/// ```
///
/// with `B`, `A` the base and alternate headrooms (ISO 21496-1 formula 3). The map always
/// holds the gain from the SDR towards the HDR rendition, so the weight is negative when the
/// base is the HDR rendition: an SDR display (`H = 1`) then gets `W = −1`, the alternate
/// fully mapped down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GainMapParams {
    /// log2 of the minimum gain.
    pub gain_min: [f32; 3],
    /// log2 of the maximum gain.
    pub gain_max: [f32; 3],
    pub gamma: [f32; 3],
    /// Offset added to the base rendition.
    pub base_offset: [f32; 3],
    /// Offset subtracted from the alternate rendition.
    pub alt_offset: [f32; 3],
    /// log2 headroom at which the base rendition is shown unchanged.
    pub base_headroom: f32,
    /// log2 headroom at which the alternate rendition is fully shown.
    pub alt_headroom: f32,
    /// The base image is the HDR rendition (the map tone-maps down to SDR):
    /// `base_headroom > alt_headroom`.
    pub base_is_hdr: bool,
    /// Primaries of the space the gains are applied in (ISO 21496-1 Annex B: the base's or
    /// the alternate's colour space). Only matters when the channels differ (a colour map
    /// or per-channel metadata): equal gains on R, G and B commute with a change of
    /// primaries, which keeps white, so decoders leave BT.709 for single-channel maps.
    pub space: Primaries,
}

impl Default for GainMapParams {
    fn default() -> GainMapParams {
        GainMapParams {
            gain_min: [0.0; 3],
            gain_max: [1.0; 3],
            gamma: [1.0; 3],
            base_offset: [1.0 / 64.0; 3],
            alt_offset: [1.0 / 64.0; 3],
            base_headroom: 0.0,
            alt_headroom: 1.0,
            base_is_hdr: false,
            space: Primaries::Bt709,
        }
    }
}

impl GainMapParams {
    /// Signed weight of the map for a display headroom (linear, 1.0 = SDR): in 0..=1 for an
    /// SDR base, in −1..=0 for an HDR base.
    pub fn weight(&self, headroom: f32) -> f32 {
        let h = headroom.max(1.0).log2();
        let d = self.alt_headroom - self.base_headroom;
        if d.abs() < 1e-6 {
            // Degenerate metadata (ISO 21496-1 forbids it): all or nothing.
            return if !self.base_is_hdr && h >= self.alt_headroom {
                1.0
            } else {
                0.0
            };
        }
        d.signum() * ((h - self.base_headroom) / d).clamp(0.0, 1.0)
    }

    /// Apply the map to one linear base channel with gain sample `g` (0..1).
    pub fn apply(&self, c: usize, base: f32, g: f32, weight: f32) -> f32 {
        self.gain(c, base, g, weight).max(0.0)
    }

    /// [`apply`](Self::apply) without the final clamp at zero.
    fn gain(&self, c: usize, base: f32, g: f32, weight: f32) -> f32 {
        let gamma = if self.gamma[c] > 0.0 {
            self.gamma[c]
        } else {
            1.0
        };
        let g = g.clamp(0.0, 1.0).powf(1.0 / gamma);
        let l = self.gain_min[c] + (self.gain_max[c] - self.gain_min[c]) * g;
        (base + self.base_offset[c]) * (l * weight).exp2() - self.alt_offset[c]
    }

    /// Apply the map to a linear BT.709 pixel with per-channel gain samples `g`: converted
    /// into the application space, mapped per channel, converted back. The reference for
    /// the shader (`image.wgsl`, `apply_hdr`).
    pub fn apply_rgb(&self, rgb: [f32; 3], g: [f32; 3], weight: f32) -> [f32; 3] {
        if !self.converts() {
            return std::array::from_fn(|c| self.apply(c, rgb[c], g[c], weight));
        }
        let x = mat_mul(from_bt709(self.space), rgb);
        // Clamped only once back in BT.709, as the shader does.
        let y = std::array::from_fn(|c| self.gain(c, x[c], g[c], weight));
        mat_mul(to_bt709(self.space), y).map(|v| v.max(0.0))
    }

    /// Whether the gains are applied in another space than linear BT.709.
    pub fn converts(&self) -> bool {
        self.space != Primaries::Bt709
    }

    /// Peak of the rendition shown at `weight` (units of reference white), for a base whose
    /// own peak is `base_peak`.
    pub fn peak(&self, base_peak: f32, weight: f32) -> f32 {
        let p = if weight >= 0.0 {
            let mx = self.gain_max.iter().copied().fold(f32::MIN, f32::max);
            base_peak * (mx * weight).exp2()
        } else {
            // An HDR base mapped down: its headroom moves towards the alternate's, and the
            // rendition is made for the headroom `B + (B − A)·W` (log2), whatever peak the
            // base declares (MaxCLL above its own headroom would tone map the SDR rendition
            // a second time).
            let d = self.base_headroom - self.alt_headroom;
            let scaled = base_peak * (d * weight).exp2();
            scaled.min((self.base_headroom + d * weight).exp2())
        };
        p.max(1.0)
    }

    /// Sanity limits so a hostile file cannot produce NaN/inf in the shader.
    pub fn is_sane(&self) -> bool {
        let fin = |a: &[f32; 3], lo: f32, hi: f32| {
            a.iter().all(|v| v.is_finite() && (lo..=hi).contains(v))
        };
        fin(&self.gain_min, -32.0, 32.0)
            && fin(&self.gain_max, -32.0, 32.0)
            && fin(&self.gamma, 1e-3, 1e3)
            && fin(&self.base_offset, -1.0, 1.0)
            && fin(&self.alt_offset, -1.0, 1.0)
            && self.base_headroom.is_finite()
            && self.alt_headroom.is_finite()
            && (-32.0..=32.0).contains(&self.base_headroom)
            && (-32.0..=32.0).contains(&self.alt_headroom)
    }
}

/// A gain map: the secondary image (an RGBA8 raster whose R, G, B are the per-channel map
/// samples; a single-channel map is replicated) and its metadata. The map covers the whole
/// base image, usually at a lower resolution.
#[derive(Clone, Debug)]
pub struct GainMap {
    pub map: Raster,
    pub params: GainMapParams,
    /// Where the metadata came from (for the info panel), e.g. "ISO 21496-1".
    pub source: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_round_trip() {
        for h in 0..=u16::MAX {
            let f = f16_to_f32(h);
            if f.is_nan() {
                assert!(f32_to_f16(f) & 0x7c00 == 0x7c00 && f32_to_f16(f) & 0x3ff != 0);
                continue;
            }
            assert_eq!(f32_to_f16(f), h, "{h:#06x} -> {f}");
        }
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(65504.0), 0x7bff);
        assert_eq!(f32_to_f16(1e6), 0x7c00);
        assert_eq!(f32_to_f16(1.0 + 1.0 / 2048.0), 0x3c00); // tie to even
        assert_eq!(f32_to_f16(1.0 + 3.0 / 2048.0), 0x3c02);
        assert_eq!(f16_to_f32(0x0001), 2f32.powi(-24));
    }

    #[test]
    fn transfer_functions() {
        // PQ: 0.58 ≈ 203 nits (BT.2408), 1.0 = 10000 nits.
        assert!((pq_to_linear(0.580_688) - 1.0).abs() < 0.01);
        assert!((pq_to_linear(1.0) - 10_000.0 / 203.0).abs() < 0.01);
        assert_eq!(pq_to_linear(0.0), 0.0);
        // HLG: 75% signal is reference white on a 1000 nit display (BT.2408): ~203 nits.
        let w = hlg_to_linear([0.75; 3]);
        assert!((w[1] - 1.0).abs() < 0.03, "{w:?}");
        let peak = hlg_to_linear([1.0; 3]);
        assert!((peak[0] - 1000.0 / 203.0).abs() < 0.01);
        for i in 0..=100 {
            let v = i as f32 / 100.0;
            assert!((srgb_to_linear(linear_to_srgb(v)) - v).abs() < 1e-5);
        }
    }

    #[test]
    fn matrices_keep_white() {
        for p in [Primaries::Bt709, Primaries::DisplayP3, Primaries::Bt2020] {
            let w = mat_mul(to_bt709(p), [1.0; 3]);
            for c in w {
                assert!((c - 1.0).abs() < 1e-3, "{p:?} {w:?}");
            }
        }
        for c in 0..64 {
            let e = HdrEncoding::from_code(c);
            if let Some(e) = e {
                assert_eq!(e.code(), c);
            }
        }
    }

    #[test]
    fn chromaticity_matrices() {
        let close = |a: [[f32; 3]; 3], b: [[f32; 3]; 3], tol: f32| {
            a.iter()
                .flatten()
                .zip(b.iter().flatten())
                .all(|(x, y)| (x - y).abs() < tol)
        };
        let d65 = [0.3127, 0.3290];
        let bt709 = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06], d65];
        let p3 = [[0.68, 0.32], [0.265, 0.69], [0.15, 0.06], d65];
        let bt2020 = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046], d65];
        let id = to_bt709(Primaries::Bt709);
        assert!(close(chromaticities_to_bt709(bt709).unwrap(), id, 1e-5));
        assert!(close(
            chromaticities_to_bt709(p3).unwrap(),
            to_bt709(Primaries::DisplayP3),
            1e-3
        ));
        assert!(close(
            chromaticities_to_bt709(bt2020).unwrap(),
            to_bt709(Primaries::Bt2020),
            1e-3
        ));
        // Another white point: white still maps to white (Bradford adaptation).
        let e_white = [
            [0.64, 0.33],
            [0.29, 0.60],
            [0.15, 0.06],
            [1.0 / 3.0, 1.0 / 3.0],
        ];
        let w = mat_mul(chromaticities_to_bt709(e_white).unwrap(), [1.0; 3]);
        assert!(w.iter().all(|c| (c - 1.0).abs() < 1e-3), "{w:?}");
        // XYZ of D65 white → RGB white.
        let w = mat_mul(
            xyz_to_bt709(),
            [0.3127 / 0.3290, 1.0, (1.0 - 0.3127 - 0.3290) / 0.3290],
        );
        assert!(w.iter().all(|c| (c - 1.0).abs() < 1e-3), "{w:?}");
        // Degenerate input.
        assert!(chromaticities_to_bt709([[0.3, 0.0]; 4]).is_none());
        assert!(chromaticities_to_bt709([[0.3, 0.3]; 4]).is_none());
        assert_eq!(primaries_matching(p3, 1e-3), Some(Primaries::DisplayP3));
        assert_eq!(primaries_matching(e_white, 1e-3), None);
    }

    #[test]
    fn tone_map_operator() {
        // No compression when the content fits.
        assert_eq!(tone_map_scale(3.0, 2.0, 4.0), 1.0);
        // Identity below the knee, peak onto peak, monotonic, C1 at the knee.
        assert_eq!(tone_map_scale(0.5, 4.93, 1.0), 1.0);
        let at = |x: f32| x * tone_map_scale(x, 4.93, 1.0);
        assert!((at(4.93) - 1.0).abs() < 1e-5);
        let mut last = 0.0;
        for i in 1..1000 {
            let y = at(i as f32 / 200.0);
            assert!(y >= last - 1e-6 && y <= 1.0 + 1e-6);
            last = y;
        }
        let slope = (at(0.8 + 1e-3) - at(0.8)) / 1e-3;
        assert!((slope - 1.0).abs() < 0.01, "{slope}");
        assert_eq!(at(100.0), 1.0);
    }

    /// 16-bit sRGB-coded samples are re-encoded, never tone mapped (thumbnails and the
    /// clipboard pass an HDR content peak for every wide raster); PQ above the knee is.
    #[test]
    fn wide_to_srgb8_tone_maps_hdr_only() {
        let px = |v: u16| -> Vec<u8> {
            [v, v, v, 65535]
                .iter()
                .flat_map(|c| c.to_ne_bytes())
                .collect()
        };
        let peak = peak_from_nits(None);
        for code in [0u16, 0x1234, 0xF0F0, 0xFFFF] {
            let want = (f32::from(code) * 255.0 / 65535.0).round() as u8;
            let got = wide_to_srgb8(PixelLayout::Rgba16(HdrEncoding::SRGB), &px(code), peak);
            assert!(
                got[0].abs_diff(want) <= 1 && got[3] == 255,
                "{code}: {got:?}"
            );
        }
        let pq = PixelLayout::Rgba16(HdrEncoding {
            transfer: Transfer::Pq,
            primaries: Primaries::Bt709,
        });
        // 1.0 (reference white): above the knee, compressed below 255.
        let white = (0.580_688_f32 * 65535.0) as u16;
        assert!(wide_to_srgb8(pq, &px(white), peak)[0] < 250);
    }

    #[test]
    fn gain_map_weights() {
        let p = GainMapParams {
            alt_headroom: 2.0,
            ..Default::default()
        };
        assert_eq!(p.weight(1.0), 0.0);
        assert!((p.weight(2.0) - 0.5).abs() < 1e-6);
        assert_eq!(p.weight(8.0), 1.0);
        // Full gain: (b + 1/64)·2 − 1/64.
        assert!((p.apply(0, 0.5, 1.0, 1.0) - 1.0 - 1.0 / 64.0).abs() < 1e-6);
        assert!((p.apply(0, 0.5, 0.0, 1.0) - 0.5).abs() < 1e-6);
        // HDR base: the weight runs the other way and is negative (the map holds the gain
        // towards HDR, applied in reverse).
        let q = GainMapParams {
            base_headroom: 2.0,
            alt_headroom: 0.0,
            base_is_hdr: true,
            ..Default::default()
        };
        assert_eq!(q.weight(4.0), 0.0);
        assert_eq!(q.weight(8.0), 0.0);
        assert_eq!(q.weight(1.0), -1.0);
        assert!((q.weight(2.0) + 0.5).abs() < 1e-6);
        assert!(p.is_sane() && q.is_sane());
        // Round trip: SDR → HDR with the SDR base's weight, back with the HDR base's.
        let q = GainMapParams {
            base_headroom: p.alt_headroom,
            alt_headroom: p.base_headroom,
            base_is_hdr: true,
            ..p
        };
        for (b, g) in [(0.0, 0.3), (0.2, 1.0), (0.9, 0.5)] {
            let hdr = p.apply(1, b, g, p.weight(4.0));
            let sdr = q.apply(1, hdr, g, q.weight(1.0));
            assert!((sdr - b).abs() < 1e-5, "{b} {g}: {hdr} {sdr}");
        }
        // Peaks: an SDR base grows with the weight, an HDR base shrinks towards SDR.
        assert_eq!(p.peak(1.0, 0.0), 1.0);
        assert!((p.peak(1.0, 1.0) - 2.0).abs() < 1e-6);
        let q = GainMapParams {
            base_headroom: 2.0,
            alt_headroom: 0.0,
            base_is_hdr: true,
            ..Default::default()
        };
        assert!((q.peak(4.0, 0.0) - 4.0).abs() < 1e-6);
        assert!((q.peak(4.0, -0.5) - 2.0).abs() < 1e-5);
        assert_eq!(q.peak(4.0, -1.0), 1.0);
        // A base whose MaxCLL is above its headroom: the SDR rendition is still SDR.
        assert_eq!(q.peak(10.0, -1.0), 1.0);
        assert!((q.peak(10.0, -0.5) - 2.0).abs() < 1e-5);
        assert!((q.peak(10.0, 0.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn gain_map_direction_and_partial_headroom() {
        // libavif's seine pair: the HDR base declares B = 1.3, the SDR alternate A = 0.
        let q = GainMapParams {
            base_headroom: 1.3,
            alt_headroom: 0.0,
            base_is_hdr: true,
            ..Default::default()
        };
        // SDR display: the full map down. A display with enough headroom: the base as is.
        assert_eq!(q.weight(1.0), -1.0);
        assert_eq!(q.weight(1.3f32.exp2()), 0.0);
        assert_eq!(q.weight(16.0), 0.0);
        // In between: linear in log2 headroom, always negative.
        let w = q.weight(0.65f32.exp2());
        assert!((w + 0.5).abs() < 1e-5, "{w}");
        // Below SDR clamps to SDR.
        assert_eq!(q.weight(0.25), -1.0);
        // The same map read from the SDR side (A and B swapped): the two weights always
        // differ by one full map.
        let p = GainMapParams {
            base_headroom: 0.0,
            alt_headroom: 1.3,
            base_is_hdr: false,
            ..q
        };
        for h in [1.0f32, 1.2, 1.6, 2.0, 2.4, 4.0] {
            assert!((p.weight(h) - q.weight(h) - 1.0).abs() < 1e-5, "{h}");
        }
        // A pixel lowered by the full map and lifted back by the SDR side's full weight.
        for g in [0.0, 0.25, 1.0] {
            let sdr = q.apply(0, 1.5, g, -1.0);
            assert!((p.apply(0, sdr, g, 1.0) - 1.5).abs() < 1e-4, "{g}");
        }
    }

    #[test]
    fn sdr_rendition_of_an_hdr_base() {
        use crate::RasterPlane;
        // A 2×1 PQ base: 406 nits (2.0) and 101.5 nits (0.5), opaque.
        let pq = |nits: f32| {
            let y = (nits / 10000.0).powf(2610.0 / 16384.0);
            let (c1, c2, c3) = (3424.0 / 4096.0, 2413.0 / 128.0, 2392.0 / 128.0);
            let e = ((c1 + c2 * y) / (1.0 + c3 * y)).powf(2523.0 / 32.0);
            (e * 65535.0 + 0.5) as u16
        };
        let mut data = Vec::new();
        for n in [406.0, 101.5] {
            for v in [pq(n), pq(n), pq(n), 65535] {
                data.extend_from_slice(&v.to_ne_bytes());
            }
        }
        let enc = HdrEncoding {
            transfer: Transfer::Pq,
            primaries: Primaries::Bt709,
        };
        let plane = |w: u32, stride: usize, data: Vec<u8>| RasterPlane {
            width: w,
            height: 1,
            stride,
            data,
            h_div: 1,
            v_div: 1,
        };
        let base = Raster {
            width: 2,
            height: 1,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba16(enc),
            planes: vec![plane(2, 16, data)],
            truncated: false,
        };
        // One map pixel of full gain: log2 gain 1 (×2) between the SDR and HDR renditions.
        let params = GainMapParams {
            base_headroom: 1.0,
            alt_headroom: 0.0,
            base_is_hdr: true,
            base_offset: [0.0; 3],
            alt_offset: [0.0; 3],
            ..Default::default()
        };
        let map = Raster {
            width: 1,
            height: 1,
            origin: (0, 0),
            scale: 1,
            layout: PixelLayout::Rgba,
            planes: vec![plane(1, 4, vec![255; 4])],
            truncated: false,
        };
        let gm = GainMap {
            map,
            params,
            source: "test",
        };
        let r = sdr_rendition(&base, &gm, 10.0).unwrap();
        assert_eq!(r.layout, PixelLayout::Rgba);
        let px = &r.planes[0].data;
        // 2.0 / 2 = reference white; 0.5 / 2 = 0.25 linear, not tone mapped any further.
        let srgb = |v: f32| (linear_to_srgb(v) * 255.0 + 0.5) as u8;
        assert!(px[0].abs_diff(255) <= 1, "{px:?}");
        assert!(px[4].abs_diff(srgb(0.25)) <= 1, "{px:?}");
        assert_eq!(px[3], 255);
        // 8-bit bases are left to the caller.
        let mut sdr = base.clone();
        sdr.layout = PixelLayout::Rgba;
        assert!(sdr_rendition(&sdr, &gm, 1.0).is_none());
    }

    #[test]
    fn gain_map_application_space() {
        for p in [Primaries::DisplayP3, Primaries::Bt2020] {
            let v = mat_mul(to_bt709(p), mat_mul(from_bt709(p), [0.2, 0.5, 0.9]));
            assert!(
                v.iter()
                    .zip([0.2, 0.5, 0.9])
                    .all(|(a, b)| (a - b).abs() < 1e-5)
            );
        }
        let rgb = [0.3, 0.6, 0.1];
        let p3 = GainMapParams {
            space: Primaries::DisplayP3,
            ..Default::default()
        };
        let bt709 = GainMapParams::default();
        assert!(p3.converts() && !bt709.converts());
        // Equal gains on every channel give the same result in any space.
        let a = p3.apply_rgb(rgb, [0.7; 3], 1.0);
        let b = bt709.apply_rgb(rgb, [0.7; 3], 1.0);
        assert!(
            a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5),
            "{a:?} {b:?}"
        );
        // Different ones do not.
        let g = [1.0, 0.5, 0.0];
        let a = p3.apply_rgb(rgb, g, 1.0);
        let b = bt709.apply_rgb(rgb, g, 1.0);
        assert!(
            a.iter().zip(b).any(|(x, y)| (x - y).abs() > 1e-3),
            "{a:?} {b:?}"
        );
    }
}

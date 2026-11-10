//! Sample conversion: stored precision to float RGBA in the requested transfer, the sRGB
//! transfer curve (fast table versions), and half floats.

use crate::parse::{Component, LayerType, PlaneKind};
use std::sync::OnceLock;

/// sRGB TRC decode, unbounded like babl's: the power segment continues above 1 and the
/// linear segment below 0.
pub(crate) fn srgb_to_linear_exact(v: f64) -> f64 {
    if v > 0.04045 {
        ((v + 0.055) / 1.055).powf(2.4)
    } else {
        v / 12.92
    }
}

/// sRGB TRC encode, unbounded like babl's.
pub(crate) fn linear_to_srgb_exact(v: f64) -> f64 {
    if v > 0.003_130_8 {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    } else {
        v * 12.92
    }
}

/// Piecewise-linear table over the f32 bit pattern of `[lo, 1)`: 1024 steps per octave, so
/// the step is relative to the value and the error stays far below 16-bit resolution.
struct BitTable {
    base: u32,
    table: Vec<f32>,
}

impl BitTable {
    fn new(lo: f32, f: impl Fn(f64) -> f64) -> BitTable {
        let base = lo.to_bits() >> 13;
        let end = 1.0f32.to_bits() >> 13;
        let table = (base..=end)
            .map(|i| f(f64::from(f32::from_bits(i << 13))) as f32)
            .collect();
        BitTable { base, table }
    }

    #[inline]
    fn get(&self, v: f32) -> f32 {
        let b = v.to_bits();
        let i = ((b >> 13) - self.base) as usize;
        let t = (b & 0x1fff) as f32 * (1.0 / 8192.0);
        let a = self.table[i];
        a + (self.table[i + 1] - a) * t
    }
}

fn to_linear_table() -> &'static BitTable {
    static T: OnceLock<BitTable> = OnceLock::new();
    // From 2^-5: below 0.04045 the curve is the linear segment.
    T.get_or_init(|| BitTable::new(0.031_25, srgb_to_linear_exact))
}

fn to_srgb_table() -> &'static BitTable {
    static T: OnceLock<BitTable> = OnceLock::new();
    // From 2^-9: below 0.0031308 the curve is the linear segment.
    T.get_or_init(|| BitTable::new(0.001_953_125, linear_to_srgb_exact))
}

#[inline]
pub(crate) fn srgb_to_linear(v: f32) -> f32 {
    if v > 0.040_45 && v < 1.0 {
        to_linear_table().get(v)
    } else if v <= 0.040_45 {
        v * (1.0 / 12.92)
    } else if v.is_nan() {
        0.0
    } else {
        srgb_to_linear_exact(f64::from(v)) as f32
    }
}

#[inline]
pub(crate) fn linear_to_srgb(v: f32) -> f32 {
    if v > 0.003_130_8 && v < 1.0 {
        to_srgb_table().get(v)
    } else if v <= 0.003_130_8 {
        v * 12.92
    } else if v.is_nan() {
        0.0
    } else {
        linear_to_srgb_exact(f64::from(v)) as f32
    }
}

/// Which transfer conversion loading applies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Trc {
    Keep,
    ToLinear,
    ToSrgb,
}

impl Trc {
    pub(crate) fn between(data_linear: bool, want_linear: bool) -> Trc {
        match (data_linear, want_linear) {
            (false, true) => Trc::ToLinear,
            (true, false) => Trc::ToSrgb,
            _ => Trc::Keep,
        }
    }

    #[inline]
    pub(crate) fn apply(self, v: f32) -> f32 {
        match self {
            Trc::Keep => v,
            Trc::ToLinear => srgb_to_linear(v),
            Trc::ToSrgb => linear_to_srgb(v),
        }
    }
}

fn lut8(trc: Trc) -> &'static [f32; 256] {
    static T: [OnceLock<[f32; 256]>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    T[trc as usize].get_or_init(|| {
        std::array::from_fn(|i| {
            let v = i as f64 / 255.0;
            (match trc {
                Trc::Keep => v,
                Trc::ToLinear => srgb_to_linear_exact(v),
                Trc::ToSrgb => linear_to_srgb_exact(v),
            }) as f32
        })
    })
}

fn lut16(trc: Trc) -> &'static [f32] {
    static T: [OnceLock<Vec<f32>>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    T[trc as usize].get_or_init(|| {
        (0..65536)
            .map(|i| {
                let v = f64::from(i) / 65535.0;
                (match trc {
                    Trc::Keep => v,
                    Trc::ToLinear => srgb_to_linear_exact(v),
                    Trc::ToSrgb => linear_to_srgb_exact(v),
                }) as f32
            })
            .collect()
    })
}

pub(crate) fn f16_to_f32(h: u16) -> f32 {
    let sign = u32::from(h >> 15) << 31;
    let exp = u32::from((h >> 10) & 0x1f);
    let man = u32::from(h & 0x3ff);
    let bits = if exp == 0 {
        if man == 0 {
            sign
        } else {
            // Subnormal: normalise.
            let shift = man.leading_zeros() - 21;
            let man = (man << shift) & 0x3ff;
            sign | ((113 - shift) << 23) | (man << 13)
        }
    } else if exp == 0x1f {
        sign | 0x7f80_0000 | (man << 13)
    } else {
        sign | ((exp + 112) << 23) | (man << 13)
    };
    f32::from_bits(bits)
}

pub(crate) fn f32_to_f16(v: f32) -> u16 {
    let b = v.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let exp = ((b >> 23) & 0xff) as i32;
    let man = b & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if man != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        // Subnormal half: shift the full mantissa, round to nearest even.
        let m = man | 0x80_0000;
        let shift = (14 - e) as u32;
        let half = 1u32 << (shift - 1);
        let rem = m & ((1 << shift) - 1);
        let mut r = m >> shift;
        if rem > half || (rem == half && r & 1 == 1) {
            r += 1;
        }
        return sign | r as u16;
    }
    let mut r = ((e as u32) << 10) | (man >> 13);
    let rem = man & 0x1fff;
    if rem > 0x1000 || (rem == 0x1000 && r & 1 == 1) {
        r += 1; // may carry into the exponent, which is correct (up to infinity)
    }
    sign | r as u16
}

/// How to read one plane's samples.
#[derive(Clone, Copy)]
pub(crate) struct Loader<'a> {
    pub kind: PlaneKind,
    pub component: Component,
    pub big_endian: bool,
    /// Stored colour samples are linear.
    pub data_linear: bool,
    pub colormap: &'a [[u8; 3]],
}

#[inline]
fn sample(c: Component, be: bool, s: &[u8]) -> f32 {
    match c {
        Component::U8 => f32::from(s[0]) * (1.0 / 255.0),
        Component::U16 => {
            let v = if be {
                u16::from_be_bytes([s[0], s[1]])
            } else {
                u16::from_le_bytes([s[0], s[1]])
            };
            f32::from(v) * (1.0 / 65535.0)
        }
        Component::U32 => {
            let a = [s[0], s[1], s[2], s[3]];
            let v = if be {
                u32::from_be_bytes(a)
            } else {
                u32::from_le_bytes(a)
            };
            (f64::from(v) / 4_294_967_295.0) as f32
        }
        Component::Half => {
            let v = if be {
                u16::from_be_bytes([s[0], s[1]])
            } else {
                u16::from_le_bytes([s[0], s[1]])
            };
            f16_to_f32(v)
        }
        Component::Float => {
            let a = [s[0], s[1], s[2], s[3]];
            f32::from_bits(if be {
                u32::from_be_bytes(a)
            } else {
                u32::from_le_bytes(a)
            })
        }
        Component::Double => {
            let a = [s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]];
            (if be {
                f64::from_be_bytes(a)
            } else {
                f64::from_le_bytes(a)
            }) as f32
        }
    }
}

#[inline]
fn colour(c: Component, be: bool, s: &[u8], trc: Trc) -> f32 {
    match c {
        Component::U8 => lut8(trc)[s[0] as usize],
        Component::U16 => {
            let v = if be {
                u16::from_be_bytes([s[0], s[1]])
            } else {
                u16::from_le_bytes([s[0], s[1]])
            };
            lut16(trc)[v as usize]
        }
        _ => trc.apply(sample(c, be, s)),
    }
}

impl Loader<'_> {
    /// Converts `src` (pixels of this plane) to RGBA floats in `out`, colour in linear light
    /// when `want_linear`, else sRGB-coded.
    pub(crate) fn rgba(&self, src: &[u8], want_linear: bool, out: &mut [f32]) {
        let PlaneKind::Layer(ty) = self.kind else {
            return;
        };
        let trc = Trc::between(self.data_linear, want_linear);
        let (c, be) = (self.component, self.big_endian);
        let n = c.bytes();
        match ty {
            LayerType::Rgb | LayerType::Rgba => {
                let alpha = ty == LayerType::Rgba;
                let bpp = n * if alpha { 4 } else { 3 };
                for (p, o) in src.chunks_exact(bpp).zip(out.chunks_exact_mut(4)) {
                    o[0] = colour(c, be, &p[..n], trc);
                    o[1] = colour(c, be, &p[n..2 * n], trc);
                    o[2] = colour(c, be, &p[2 * n..3 * n], trc);
                    o[3] = if alpha {
                        sample(c, be, &p[3 * n..])
                    } else {
                        1.0
                    };
                }
            }
            LayerType::Grey | LayerType::GreyA => {
                let alpha = ty == LayerType::GreyA;
                let bpp = n * if alpha { 2 } else { 1 };
                for (p, o) in src.chunks_exact(bpp).zip(out.chunks_exact_mut(4)) {
                    let v = colour(c, be, &p[..n], trc);
                    o[0] = v;
                    o[1] = v;
                    o[2] = v;
                    o[3] = if alpha { sample(c, be, &p[n..]) } else { 1.0 };
                }
            }
            LayerType::Indexed | LayerType::IndexedA => {
                // Colormaps are 8-bit sRGB-coded whatever the precision says.
                let lut = lut8(Trc::between(false, want_linear));
                let alpha = ty == LayerType::IndexedA;
                let bpp = if alpha { 2 } else { 1 };
                let last = self.colormap.len().saturating_sub(1);
                for (p, o) in src.chunks_exact(bpp).zip(out.chunks_exact_mut(4)) {
                    let e = self
                        .colormap
                        .get((p[0] as usize).min(last))
                        .copied()
                        .unwrap_or([0; 3]);
                    o[0] = lut[e[0] as usize];
                    o[1] = lut[e[1] as usize];
                    o[2] = lut[e[2] as usize];
                    o[3] = if alpha {
                        f32::from(p[1]) * (1.0 / 255.0)
                    } else {
                        1.0
                    };
                }
            }
        }
    }

    /// Converts mask samples to coverage 0..1.
    pub(crate) fn mask(&self, src: &[u8], out: &mut [f32]) {
        let (c, be) = (self.component, self.big_endian);
        for (p, o) in src.chunks_exact(c.bytes()).zip(out.iter_mut()) {
            *o = sample(c, be, p);
            if !self.data_linear {
                *o = srgb_to_linear(*o);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_round_trip() {
        for h in 0..=u16::MAX {
            let f = f16_to_f32(h);
            if f.is_nan() {
                assert!(f16_to_f32(f32_to_f16(f)).is_nan());
            } else {
                assert_eq!(f32_to_f16(f), h, "{h:#x}");
            }
        }
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(65520.0), 0x7c00);
        assert_eq!(f32_to_f16(1e-8), 0);
    }

    #[test]
    fn trc_tables_match_exact() {
        let mut worst = 0f64;
        for i in 0..=200_000 {
            let v = i as f32 / 200_000.0;
            let a = f64::from(linear_to_srgb(v)) - linear_to_srgb_exact(f64::from(v));
            let b = f64::from(srgb_to_linear(v)) - srgb_to_linear_exact(f64::from(v));
            worst = worst.max(a.abs()).max(b.abs());
        }
        assert!(worst < 2e-6, "{worst}");
        assert!((linear_to_srgb(2.0) - 1.353_256).abs() < 1e-5);
        assert!((srgb_to_linear(-0.5) + 0.5 / 12.92).abs() < 1e-7);
    }
}

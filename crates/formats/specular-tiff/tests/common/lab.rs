//! L*a*b* TIFF cases for tests/lab.rs and tests/lab_fallback.rs: synthetic files of every
//! encoding, depth, layout, alpha kind and compression, with the expected RGBA from lcms2 (the
//! test oracle) computed from the samples by the encodings' definitions.
#![allow(dead_code)]

use crate::common::{Comp, Spec, u16_bytes, write};

/// PhotometricInterpretation values.
pub const CIELAB: u16 = 8;
pub const ICCLAB: u16 = 9;
pub const ITULAB: u16 = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alpha {
    None,
    Associated,
    Unassociated,
}

#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub photo: u16,
    pub wide: bool,
    pub planar: bool,
    pub alpha: Alpha,
    pub comp: Comp,
    pub predictor: u16,
    pub le: bool,
    /// ITULab: a Decode tag (L, a*, b* ranges) instead of the defaults.
    pub decode: Option<[f64; 6]>,
    pub tile: Option<(u32, u32)>,
    /// Rows per strip (`None`: the whole image in one strip).
    pub rows: Option<u32>,
    /// Bits per sample other than 8 / 16 (`wide` is then ignored); with `float`, float
    /// samples of 32 or 64 bits holding L*, a*, b* themselves.
    pub bits: Option<u16>,
    pub float: bool,
}

impl Case {
    pub fn new(photo: u16, wide: bool) -> Case {
        Case {
            photo,
            wide,
            planar: false,
            alpha: Alpha::None,
            comp: Comp::None,
            predictor: 1,
            le: true,
            decode: None,
            tile: None,
            rows: Some(5),
            bits: None,
            float: false,
        }
    }
}

/// D50 L*a*b* (ICC v4 PCS) → 8-bit sRGB, relative colorimetric.
pub fn oracle(lab: &[[f64; 3]]) -> Vec<[u8; 3]> {
    let d50 = [0.9642f64, 1.0, 0.8249];
    let sum: f64 = d50.iter().sum();
    let white = lcms2::CIExyY {
        x: d50[0] / sum,
        y: d50[1] / sum,
        Y: 1.0,
    };
    let src = lcms2::Profile::new_lab4_context(lcms2::GlobalContext::new(), &white).unwrap();
    let t: lcms2::Transform<[f64; 3], [u8; 3], _, _> = lcms2::Transform::new_flags(
        &src,
        lcms2::PixelFormat::Lab_DBL,
        &lcms2::Profile::new_srgb(),
        lcms2::PixelFormat::RGB_8,
        lcms2::Intent::RelativeColorimetric,
        lcms2::Flags::NO_CACHE,
    )
    .unwrap();
    let mut out = vec![[0u8; 3]; lab.len()];
    t.transform_pixels(lab, &mut out);
    out
}

/// Value of channel `c` of a coded sample `s` (already sign-extended for CIELab a*/b*), by the
/// encodings' definitions.
fn value(case: &Case, c: usize, s: f64) -> f64 {
    let bits = case.depth();
    let max = ((1u64 << bits) - 1) as f64;
    match (case.photo, c) {
        (ITULAB, _) => {
            let d = case.decode.unwrap_or([
                0.0,
                100.0,
                -21760.0 / 255.0,
                21590.0 / 255.0,
                -19200.0 / 255.0,
                31800.0 / 255.0,
            ]);
            d[2 * c] + s * (d[2 * c + 1] - d[2 * c]) / max
        }
        (_, 0) => s * 100.0 / max,
        // CIELab: two's complement, 8-bit in whole units, 16-bit in 1/256 (other depths: see
        // `make`).
        (CIELAB, _) => s / 2f64.powi(bits as i32 - 8),
        // ICCLab: 8-bit a*/b* offset by 128, 16-bit in the ICC v4 coding (÷ 257 − 128), other
        // depths alike (× 255 / max − 128).
        _ => s * 255.0 / max - 128.0,
    }
}

impl Case {
    /// Bits per integer sample.
    fn depth(&self) -> u16 {
        self.bits.unwrap_or(if self.wide { 16 } else { 8 })
    }
}

/// A float L*a*b* file (CIELab values as stored) and the RGBA it should decode to.
fn make_float(case: &Case, w: u32, h: u32, mut rnd: impl FnMut() -> u32) -> (Vec<u8>, Vec<u8>) {
    let bits = case.bits.unwrap_or(32);
    let spp = 3 + (case.alpha != Alpha::None) as usize;
    let n = (w * h) as usize;
    let mut raw = Vec::new();
    let mut want = Vec::with_capacity(n * 4);
    let mut lab = Vec::with_capacity(n);
    let mut alpha = Vec::new();
    for _ in 0..n {
        let u = |r: u32| r as f64 / (1 << 24) as f64;
        let mut px = [
            u(rnd()) * 100.0,
            u(rnd()) * 255.0 - 128.0,
            u(rnd()) * 255.0 - 128.0,
        ];
        let mut vals = px.to_vec();
        if case.alpha != Alpha::None {
            let a = u(rnd());
            vals.push(a);
            // The decoder reads float alpha at 16 bits, then rounds to 8.
            let a16 = (a * 65535.0).round() as u32;
            alpha.push(((a16 * 255 + 32767) / 65535) as u8);
        } else {
            alpha.push(255);
        }
        for v in &vals {
            match bits {
                32 => raw.extend(if case.le {
                    (*v as f32).to_le_bytes()
                } else {
                    (*v as f32).to_be_bytes()
                }),
                _ => raw.extend(if case.le {
                    v.to_le_bytes()
                } else {
                    v.to_be_bytes()
                }),
            }
        }
        if bits == 32 {
            px = px.map(|v| v as f32 as f64);
        }
        lab.push(px);
    }
    let mut s = Spec::new(w, h, spp as u16, bits, case.photo, raw);
    s.sample_format = 3;
    s.comp = case.comp;
    s.planar = case.planar;
    s.tile = case.tile;
    s.rows_per_strip = case.rows.unwrap_or(h);
    if case.alpha == Alpha::Unassociated {
        s.extra.push((338, 3, vec![2]));
    }
    let file = write(&[s], false, case.le);
    for (p, a) in oracle(&lab).iter().zip(alpha) {
        want.extend_from_slice(&[p[0], p[1], p[2], a]);
    }
    (file, want)
}

/// A file for `case` (w × h, random samples) and the RGBA it should decode to.
pub fn make(case: &Case, w: u32, h: u32, seed: u32) -> (Vec<u8>, Vec<u8>) {
    let mut seed = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    let mut rnd = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed >> 8
    };
    if case.float {
        return make_float(case, w, h, rnd);
    }
    let bits = case.depth();
    let max = ((1u64 << bits) - 1) as u32;
    let odd = !matches!(bits, 8 | 16);
    assert!(!odd || case.alpha != Alpha::Associated);
    let half = max.div_ceil(2);
    let spp = 3 + (case.alpha != Alpha::None) as usize;
    let signed = |c: usize| case.photo == CIELAB && c > 0;
    let n = (w * h) as usize;
    let mut samples = Vec::with_capacity(n * spp);
    let mut lab = Vec::with_capacity(n);
    let mut alpha = Vec::with_capacity(n);
    for _ in 0..n {
        let a = if case.alpha == Alpha::None {
            max
        } else {
            // Some fully transparent and opaque pixels among the rest.
            match rnd() % 8 {
                0 => 0,
                1 => max,
                _ => rnd() % (max + 1),
            }
        };
        let af = a as f64 / max as f64;
        let mut px = [0.0f64; 3];
        for (c, v) in px.iter_mut().enumerate() {
            let raw = rnd() % (max + 1);
            // The coded value as a number (two's complement for CIELab a*/b*).
            let s = if signed(c) && raw >= half {
                raw as f64 - (2 * half) as f64
            } else {
                raw as f64
            };
            let (stored, s) = if case.alpha == Alpha::Associated {
                let p = (s * af).round();
                let back = if a > 0 && a < max { p / af } else { p };
                (p, back)
            } else {
                (s, s)
            };
            let st = if stored < 0.0 {
                (stored + (2 * half) as f64) as u32
            } else {
                stored as u32
            };
            samples.push(st);
            // Other depths of CIELab a*/b* (as ImageMagick writes them): the sample scaled to
            // 16 bits, then the 16-bit two's complement.
            let mut val = if odd && signed(c) {
                let q = (raw as u64 * 65535 + max as u64 / 2) / max as u64;
                let q = if q >= 32768 {
                    q as f64 - 65536.0
                } else {
                    q as f64
                };
                q / 256.0
            } else {
                value(case, c, s)
            };
            if case.alpha == Alpha::Associated {
                val = if c == 0 {
                    val.clamp(0.0, 100.0)
                } else {
                    val.clamp(-128.0, 127.0)
                };
            }
            *v = val;
        }
        if case.alpha != Alpha::None {
            samples.push(a);
        }
        lab.push(px);
        alpha.push(if odd {
            // Read at 16 bits first, then rounded to 8.
            let a16 = ((a as u64 * 65535 + max as u64 / 2) / max as u64) as u32;
            ((a16 * 255 + 32767) / 65535) as u8
        } else if case.wide {
            ((a * 255 + 32767) / 65535) as u8
        } else {
            a as u8
        });
    }
    let raw = if odd {
        let rb = crate::common::row_bytes(w, spp as u16, bits);
        let mut raw = vec![0u8; rb * h as usize];
        for (y, row) in raw.chunks_exact_mut(rb).enumerate() {
            for i in 0..w as usize * spp {
                let v = samples[y * w as usize * spp + i] as u64;
                crate::common::put_sample(row, i, bits, case.le, v);
            }
        }
        raw
    } else if case.wide {
        let v: Vec<u16> = samples.iter().map(|&s| s as u16).collect();
        u16_bytes(&v, case.le)
    } else {
        samples.iter().map(|&s| s as u8).collect()
    };
    let mut s = Spec::new(w, h, spp as u16, bits, case.photo, raw);
    s.comp = case.comp;
    s.predictor = case.predictor;
    s.planar = case.planar;
    s.tile = case.tile;
    s.rows_per_strip = case.rows.unwrap_or(h);
    match case.alpha {
        Alpha::None => {}
        Alpha::Associated => s.extra.push((338, 3, vec![1])),
        Alpha::Unassociated => s.extra.push((338, 3, vec![2])),
    }
    if let Some(d) = case.decode {
        // SRATIONAL, in 1/1000 steps.
        let v = d
            .iter()
            .map(|&x| ((((x * 1000.0).round() as i32) as u32 as u64) << 32) | 1000)
            .collect();
        s.extra.push((433, 10, v));
    }
    let file = write(&[s], false, case.le);
    let rgb = oracle(&lab);
    let want = rgb
        .iter()
        .zip(&alpha)
        .flat_map(|(p, &a)| [p[0], p[1], p[2], a])
        .collect();
    (file, want)
}

/// Largest difference of the colour channels of pixels that are not fully transparent, and
/// whether every alpha matches.
pub fn diff(got: &[u8], want: &[u8]) -> (u8, bool) {
    assert_eq!(got.len(), want.len());
    let mut d = 0;
    let mut alpha = true;
    for (g, w) in got.chunks_exact(4).zip(want.chunks_exact(4)) {
        alpha &= g[3] == w[3];
        if w[3] != 0 {
            for c in 0..3 {
                d = d.max(g[c].abs_diff(w[c]));
            }
        }
    }
    (d, alpha)
}

/// Every case of the matrix: three encodings × 8 / 16 bits × chunky / planar × no alpha /
/// associated / unassociated × every compression (LZW also with the predictor) × byte order
/// for 16 bits.
pub fn matrix() -> Vec<Case> {
    let mut v = Vec::new();
    for photo in [CIELAB, ICCLAB, ITULAB] {
        for wide in [false, true] {
            for planar in [false, true] {
                for alpha in [Alpha::None, Alpha::Associated, Alpha::Unassociated] {
                    for (comp, predictor) in [
                        (Comp::None, 1),
                        (Comp::Lzw, 1),
                        (Comp::Lzw, 2),
                        (Comp::Deflate, 1),
                        (Comp::PackBits, 1),
                    ] {
                        for le in if wide {
                            &[true, false][..]
                        } else {
                            &[true][..]
                        } {
                            v.push(Case {
                                planar,
                                alpha,
                                comp,
                                predictor,
                                le: *le,
                                ..Case::new(photo, wide)
                            });
                        }
                    }
                }
            }
        }
    }
    v
}

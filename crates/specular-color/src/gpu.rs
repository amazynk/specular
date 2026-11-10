//! GPU-ready descriptions of colour transforms: per-image source transforms (encoded file
//! values → linear BT.709) and the display transform (linear BT.709 → display device values).

use crate::curve::Curve;
use crate::icc::{ColorSpace, Profile};
use crate::math::{Mat3, bt709_to_pcs, f16_bits, linear_to_srgb, srgb_to_linear};
use crate::profiles;
use crate::transform::{Intent, Transform};

/// Grid points per axis of 3D LUTs.
pub const CUBE_SIZE: usize = 33;
/// Samples of 1D curve tables.
pub const CURVE_SIZE: usize = 1024;

/// Tolerance below which a transform counts as "already sRGB".
const SRGB_TOL: f32 = 2e-3;

/// How the convert shader turns a file's encoded values into linear BT.709.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceLut {
    /// Per-channel curves (`CURVE_SIZE` RGBA half floats, encoded → linear), then
    /// `rgb' = M · rgb + offset` with `matrix` as three rows of (m0, m1, m2, offset).
    Curves {
        /// Curve samples, RGBA f16 bits, `CURVE_SIZE` texels.
        data: Vec<u16>,
        /// Row-major 3×4 matrix with offsets in the fourth column.
        matrix: [f32; 12],
    },
    /// A `CUBE_SIZE`³ table of RGBA half floats, red fastest: encoded RGB → linear BT.709.
    Cube {
        /// Table texels, RGBA f16 bits.
        data: Vec<u16>,
    },
}

impl SourceLut {
    /// Bytes of texture data.
    pub fn bytes(&self) -> usize {
        match self {
            SourceLut::Curves { data, .. } | SourceLut::Cube { data } => data.len() * 2,
        }
    }
}

fn mat_diff(a: &Mat3, b: &Mat3) -> f64 {
    a.max_diff(b)
}

fn curves_texels(curves: [&dyn Fn(f32) -> f32; 3]) -> Vec<u16> {
    let mut data = Vec::with_capacity(CURVE_SIZE * 4);
    for i in 0..CURVE_SIZE {
        let x = i as f32 / (CURVE_SIZE - 1) as f32;
        for c in curves {
            data.push(f16_bits(c(x)));
        }
        data.push(f16_bits(1.0));
    }
    data
}

fn cube<F: FnMut([f32; 3]) -> [f32; 3]>(mut f: F) -> Vec<u16> {
    let n = CUBE_SIZE;
    let mut data = Vec::with_capacity(n * n * n * 4);
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let v = f([r, g, b].map(|i| i as f32 / (n - 1) as f32));
                data.extend([
                    f16_bits(v[0]),
                    f16_bits(v[1]),
                    f16_bits(v[2]),
                    f16_bits(1.0),
                ]);
            }
        }
    }
    data
}

/// The source transform for an embedded profile, or `None` when no conversion is needed
/// (sRGB or close to it), the profile is unusable, or the image isn't RGB/gray (CMYK and
/// L*a*b* are converted on the CPU while decoding).
pub fn source_lut(icc: &[u8], intent: Intent, bpc: bool) -> Option<SourceLut> {
    source_lut_for(&Profile::parse(icc).ok()?, intent, bpc)
}

/// [`source_lut`] for a parsed profile.
pub fn source_lut_for(p: &Profile, intent: Intent, bpc: bool) -> Option<SourceLut> {
    if !matches!(p.color_space, ColorSpace::Rgb | ColorSpace::Gray) {
        return None;
    }
    let dst = profiles::linear_bt709();
    let t = Transform::new(p, &dst, intent, bpc).ok()?;
    let w = bt709_to_pcs();
    let w_inv = w.inverse()?;
    let uses_matrix = p
        .a2b_for(match intent {
            Intent::Perceptual => 0,
            Intent::Saturation => 2,
            _ => 1,
        })
        .is_none();
    if p.color_space == ColorSpace::Rgb && uses_matrix {
        let (Some(m), Some(trc)) = (&p.matrix, &p.trc) else {
            return None;
        };
        let (scale, off) = t.pcs_adjust();
        let s = Mat3::diag([scale[0] as f64, scale[1] as f64, scale[2] as f64]);
        let full = w_inv.mul(&s).mul(m);
        let o = w_inv.apply64([off[0] as f64, off[1] as f64, off[2] as f64]);
        let srgb = Curve::srgb();
        let near_srgb = mat_diff(&full, &Mat3::IDENTITY) < SRGB_TOL as f64
            && o.iter().all(|v| v.abs() < 1e-4)
            && trc.iter().all(|c| c.max_diff(&srgb) < SRGB_TOL);
        if near_srgb {
            return None;
        }
        let f = full.to_f32();
        let matrix = [
            f[0],
            f[1],
            f[2],
            o[0] as f32, //
            f[3],
            f[4],
            f[5],
            o[1] as f32, //
            f[6],
            f[7],
            f[8],
            o[2] as f32,
        ];
        let data = curves_texels([&|x| trc[0].eval(x), &|x| trc[1].eval(x), &|x| {
            trc[2].eval(x)
        }]);
        return Some(SourceLut::Curves { data, matrix });
    }
    if p.color_space == ColorSpace::Gray {
        // Gray maps onto the neutral axis: one curve, identity matrix.
        let g = |x: f32| {
            let mut o = [0f32; 3];
            t.eval(&[x], &mut o);
            o[1]
        };
        let near_srgb = (0..=256).all(|i| {
            let x = i as f32 / 256.0;
            (g(x) - srgb_to_linear(x)).abs() < SRGB_TOL
        });
        if near_srgb {
            return None;
        }
        let data = curves_texels([&g, &g, &g]);
        let matrix = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        return Some(SourceLut::Curves { data, matrix });
    }
    let mut max_err = 0f32;
    let mut o = [0f32; 3];
    let data = cube(|v| {
        t.eval(&v, &mut o);
        for c in 0..3 {
            max_err = max_err.max((o[c] - srgb_to_linear(v[c])).abs());
        }
        o
    });
    if max_err < SRGB_TOL {
        return None;
    }
    Some(SourceLut::Cube { data })
}

/// What the final pass does to reach the display.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayKind {
    /// The display is sRGB (or colour management is off): plain sRGB encoding.
    Identity,
    /// `d = clamp(M · rgb)`, then per-channel inverse tone curves indexed by the
    /// sRGB-encoded `d` (`CURVE_SIZE` RGBA f16 texels).
    Curves {
        /// Linear BT.709 → linear display RGB, row-major.
        matrix: [f32; 9],
        /// Inverse curve samples, RGBA f16 bits.
        data: Vec<u16>,
    },
    /// A `CUBE_SIZE`³ table indexed by sRGB-encoded BT.709, giving display device values.
    Cube {
        /// Table texels, RGBA f16 bits.
        data: Vec<u16>,
    },
}

/// The display side of colour management.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    /// Final-pass transform.
    pub kind: DisplayKind,
    /// Linear BT.709 → storage space (the display's own primaries for matrix displays, so
    /// 8-bit textures keep the display's gamut), row-major.
    pub to_storage: [f32; 9],
    /// Storage space → linear BT.709.
    pub from_storage: [f32; 9],
    /// Hash of the display profile (0 for sRGB / none).
    pub hash: u64,
}

const IDENT9: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

impl Display {
    /// No display transform: sRGB.
    pub fn srgb() -> Display {
        Display {
            kind: DisplayKind::Identity,
            to_storage: IDENT9,
            from_storage: IDENT9,
            hash: 0,
        }
    }

    /// True when the storage space is linear BT.709.
    pub fn storage_is_bt709(&self) -> bool {
        self.to_storage == IDENT9
    }

    /// Builds the display transform for a display profile. Unusable or non-RGB profiles give
    /// [`Display::srgb`].
    pub fn new(icc: &[u8], intent: Intent, bpc: bool) -> Display {
        match Profile::parse(icc) {
            Ok(p) => Display::for_profile(&p, intent, bpc),
            Err(_) => Display::srgb(),
        }
    }

    /// [`Display::new`] for a parsed profile.
    pub fn for_profile(p: &Profile, intent: Intent, bpc: bool) -> Display {
        if p.color_space != ColorSpace::Rgb {
            return Display::srgb();
        }
        let w = bt709_to_pcs();
        let tag = match intent {
            Intent::Perceptual => 0,
            Intent::Saturation => 2,
            _ => 1,
        };
        // Black point compensation is a PCS offset the curves' 3×3 matrix can't carry: a
        // display whose black isn't zero (black offset in its curves) goes through the cube.
        let pcs_identity = Transform::new(&profiles::linear_bt709(), p, intent, bpc)
            .is_ok_and(|t| t.pcs_adjust() == ([1.0; 3], [0.0; 3]));
        if let (Some(m), Some(trc), None, true) = (&p.matrix, &p.trc, p.b2a_for(tag), pcs_identity)
        {
            let Some(m_inv) = m.inverse() else {
                return Display::srgb();
            };
            let to_d = m_inv.mul(&w);
            let Some(from_d) = to_d.inverse() else {
                return Display::srgb();
            };
            let srgb = Curve::srgb();
            let curves_srgb = trc.iter().all(|c| c.max_diff(&srgb) < SRGB_TOL);
            let matrix_ident = to_d.max_diff(&Mat3::IDENTITY) < SRGB_TOL as f64;
            if curves_srgb && matrix_ident {
                return Display::srgb();
            }
            let inv = [trc[0].inverse(), trc[1].inverse(), trc[2].inverse()];
            let data = curves_texels([
                &|x| inv[0].eval(srgb_to_linear(x)),
                &|x| inv[1].eval(srgb_to_linear(x)),
                &|x| inv[2].eval(srgb_to_linear(x)),
            ]);
            let (to_storage, from_storage) = if matrix_ident {
                (IDENT9, IDENT9)
            } else {
                (to_d.to_f32(), from_d.to_f32())
            };
            return Display {
                kind: DisplayKind::Curves {
                    matrix: to_d.to_f32(),
                    data,
                },
                to_storage,
                from_storage,
                hash: p.hash,
            };
        }
        let Ok(t) = Transform::new(&profiles::linear_bt709(), p, intent, bpc) else {
            return Display::srgb();
        };
        let mut o = [0f32; 3];
        let mut max_err = 0f32;
        let data = cube(|v| {
            t.eval(&v.map(srgb_to_linear), &mut o);
            for c in 0..3 {
                max_err = max_err.max((o[c] - v[c]).abs());
            }
            o
        });
        if max_err < SRGB_TOL {
            return Display::srgb();
        }
        Display {
            kind: DisplayKind::Cube { data },
            to_storage: IDENT9,
            from_storage: IDENT9,
            hash: p.hash,
        }
    }

    /// CPU reference of the final pass for one linear BT.709 colour (tests, readouts).
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let enc = |v: f32| linear_to_srgb(v.clamp(0.0, 1.0));
        match &self.kind {
            DisplayKind::Identity => rgb.map(enc),
            DisplayKind::Curves { matrix, data } => {
                let d = crate::math::mul3(matrix, rgb).map(enc);
                let mut out = [0f32; 3];
                for c in 0..3 {
                    let p = d[c] * (CURVE_SIZE - 1) as f32;
                    let i = (p as usize).min(CURVE_SIZE - 2);
                    let f = p - i as f32;
                    let a = crate::math::f16_to_f32(data[i * 4 + c]);
                    let b = crate::math::f16_to_f32(data[(i + 1) * 4 + c]);
                    out[c] = a + (b - a) * f;
                }
                out
            }
            DisplayKind::Cube { data } => {
                let e = rgb.map(enc);
                let n = CUBE_SIZE;
                let vals: Vec<f32> = data.iter().map(|&h| crate::math::f16_to_f32(h)).collect();
                let mut out = [0f32; 3];
                let p = e.map(|v| v * (n - 1) as f32);
                let i = p.map(|v| (v as usize).min(n - 2));
                let f = [p[0] - i[0] as f32, p[1] - i[1] as f32, p[2] - i[2] as f32];
                for (c, o) in out.iter_mut().enumerate() {
                    let mut acc = 0.0;
                    for corner in 0..8 {
                        let d = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
                        let w: f32 = (0..3)
                            .map(|k| if d[k] == 1 { f[k] } else { 1.0 - f[k] })
                            .product();
                        let idx = ((i[2] + d[2]) * n + i[1] + d[1]) * n + i[0] + d[0];
                        acc += w * vals[idx * 4 + c];
                    }
                    *o = acc;
                }
                out
            }
        }
    }
}

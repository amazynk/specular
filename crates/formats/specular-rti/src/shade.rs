//! Relighting: the CPU reference of the relight shader.
//!
//! For one light every basis and colour layout reduces to a [`Linear`] map of the
//! coefficient layers, so the shader needs no per-format code: it reads each layer's three
//! values, maps them with the layer's scale and offset, multiplies them by the layer's 3×3
//! matrix and sums. [`Rti::shade`] is the same arithmetic in `f32`, in the same order.

use crate::basis::{
    bilinear_weights, hsh_weights, normal_lights, ptm_weights, ptm_weights_relight, rbf_weights,
    sh_weights,
};
use crate::{Basis, Colour, Rti};

/// The light a view starts with: straight from the viewer.
pub const DEFAULT_LIGHT: [f32; 3] = [0.0, 0.0, 1.0];

/// What the relit view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The surface under the light.
    #[default]
    Default,
    /// The colour darkened and a white Blinn-Phong highlight added from the normals.
    Specular,
    /// The fitted surface normals as colours (`(n + 1) / 2`).
    Normals,
    /// Shape detail amplified: the difference from the best Lambertian fit is multiplied by
    /// the gain (Malzbender's diffuse gain, for any basis).
    DiffuseGain,
}

impl Mode {
    pub const ALL: [Mode; 4] = [
        Mode::Default,
        Mode::Specular,
        Mode::Normals,
        Mode::DiffuseGain,
    ];

    pub fn index(self) -> u32 {
        match self {
            Mode::Default => 0,
            Mode::Specular => 1,
            Mode::Normals => 2,
            Mode::DiffuseGain => 3,
        }
    }

    /// The next mode in [`Mode::ALL`].
    pub fn next(self) -> Mode {
        Mode::ALL[(self.index() as usize + 1) % Mode::ALL.len()]
    }
}

/// A light and how to show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Unit light direction (x right, y up, z towards the viewer).
    pub light: [f32; 3],
    pub mode: Mode,
    /// Diffuse gain factor ([`Mode::DiffuseGain`]).
    pub gain: f32,
    /// Diffuse and specular weights and the Phong exponent ([`Mode::Specular`]).
    pub kd: f32,
    pub ks: f32,
    pub exponent: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            light: DEFAULT_LIGHT,
            mode: Mode::Default,
            gain: 3.0,
            kd: 0.4,
            ks: 0.7,
            exponent: 40.0,
        }
    }
}

impl Look {
    /// The light for a point of the unit disk (the light's x and y), on the hemisphere.
    pub fn light_from_disk(x: f32, y: f32) -> [f32; 3] {
        let r2 = x * x + y * y;
        if r2 >= 1.0 {
            let r = r2.sqrt();
            [x / r, y / r, 0.0]
        } else {
            [x, y, (1.0 - r2).sqrt()]
        }
    }
}

/// `out = constant + Σ matrices[j] · layer_j` with each matrix row-major (`out[r] +=
/// m[r][c] · value[c]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Linear {
    pub constant: [f32; 3],
    pub matrices: Vec<[[f32; 3]; 3]>,
}

impl Linear {
    fn zero(layers: usize) -> Linear {
        Linear {
            constant: [0.0; 3],
            matrices: vec![[[0.0; 3]; 3]; layers],
        }
    }
}

/// Everything the shader needs for one look.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    /// The colour (LRGB: the luminance in all three rows, before the colour multiplies it).
    pub colour: Linear,
    /// The least-squares `g` with luminance ≈ `g · light`; its direction is the normal.
    pub normal: Linear,
    /// Multiply by layer 0 (LRGB).
    pub albedo: bool,
    pub light: [f32; 3],
    pub mode: Mode,
    pub gain: f32,
    pub kd: f32,
    pub ks: f32,
    pub exponent: f32,
}

impl Rti {
    /// Weight of each coefficient plane for a light (row `k` = output channel), as a linear map.
    fn light_map(&self, l: [f64; 3]) -> (Linear, bool) {
        let layers = self.layers.len();
        let mut out = Linear::zero(layers);
        let terms_of = |n: usize| -> Vec<f64> {
            let mut w = vec![0.0; n];
            match &self.basis {
                Basis::Ptm { relight_order } => {
                    let p = if *relight_order {
                        ptm_weights_relight(l)
                    } else {
                        ptm_weights(l)
                    };
                    w.copy_from_slice(&p[..n]);
                }
                Basis::Hsh { .. } => hsh_weights(l, &mut w),
                Basis::Sh { .. } => sh_weights(l, &mut w),
                _ => {}
            }
            w
        };
        match self.colour {
            Colour::Rgb => {
                let w = terms_of(self.basis.terms());
                for (j, m) in out.matrices.iter_mut().enumerate() {
                    let wj = w.get(j).copied().unwrap_or(0.0) as f32;
                    for (k, row) in m.iter_mut().enumerate() {
                        row[k] = wj;
                    }
                }
                (out, false)
            }
            Colour::Lrgb => {
                let terms = self.planes - 3;
                let w = terms_of(terms);
                for (j, m) in out.matrices.iter_mut().enumerate().skip(1) {
                    for c in 0..3 {
                        let t = 3 * (j - 1) + c;
                        let wt = w.get(t).copied().unwrap_or(0.0) as f32;
                        for row in m.iter_mut() {
                            row[c] = wt;
                        }
                    }
                }
                (out, true)
            }
            Colour::Mrgb | Colour::Mycc { .. } => {
                let lw = match &self.basis {
                    Basis::Rbf {
                        lights,
                        sigma,
                        basis,
                    } => rbf_weights(l, lights, *sigma, basis, self.planes),
                    Basis::Bilinear { resolution, basis } => {
                        bilinear_weights(l, *resolution, basis, self.planes)
                    }
                    _ => vec![[0.0; 3]; self.planes + 1],
                };
                out.constant = lw[0].map(|v| v as f32);
                let chroma = match self.colour {
                    Colour::Mycc { chroma } => Some(chroma),
                    _ => None,
                };
                for (j, m) in out.matrices.iter_mut().enumerate() {
                    for c in 0..3 {
                        let p = 3 * j + c;
                        if p >= self.planes {
                            continue;
                        }
                        let wp = lw[p + 1];
                        match chroma {
                            None => {
                                for (k, row) in m.iter_mut().enumerate() {
                                    row[c] = wp[k] as f32;
                                }
                            }
                            Some(ny) if j < ny => m[c][c] = wp[c] as f32,
                            Some(_) => m[0][c] = wp[0] as f32,
                        }
                    }
                }
                if chroma.is_some() {
                    // Relight's luma/chroma to RGB: g = y + b/2, b = y − b/2 − a/2, r = b + a.
                    const T: [[f32; 3]; 3] = [[1.0, 0.5, -0.5], [1.0, 0.0, 0.5], [1.0, -0.5, -0.5]];
                    let mul = |v: [f32; 3]| -> [f32; 3] {
                        std::array::from_fn(|r| T[r][0] * v[0] + T[r][1] * v[1] + T[r][2] * v[2])
                    };
                    out.constant = mul(out.constant);
                    for m in &mut out.matrices {
                        let cols: [[f32; 3]; 3] =
                            std::array::from_fn(|c| mul([m[0][c], m[1][c], m[2][c]]));
                        *m = std::array::from_fn(|r| std::array::from_fn(|c| cols[c][r]));
                    }
                }
                (out, false)
            }
        }
    }

    /// The linear maps and parameters for a look.
    pub fn prepare(&self, look: &Look) -> Prepared {
        let n = norm(look.light);
        let l = [n[0] as f64, n[1] as f64, n[2] as f64];
        let (colour, albedo) = self.light_map(l);
        Prepared {
            colour,
            normal: self.normal_map(albedo),
            albedo,
            light: n,
            mode: look.mode,
            gain: look.gain,
            kd: look.kd,
            ks: look.ks,
            exponent: look.exponent,
        }
    }

    /// The photometric-stereo projection: `g = (ΛᵀΛ)⁻¹ Λᵀ lum(Λ)` over the sample lights,
    /// linear in the coefficients.
    fn normal_map(&self, albedo: bool) -> Linear {
        let lights = normal_lights();
        let mut a = [[0.0f64; 3]; 3];
        for l in &lights {
            for r in 0..3 {
                for c in 0..3 {
                    a[r][c] += l[r] * l[c];
                }
            }
        }
        let inv = invert3(a);
        let layers = self.layers.len();
        let mut cst = [0.0f64; 3];
        let mut mats = vec![[[0.0f64; 3]; 3]; layers];
        for l in &lights {
            let (map, _) = self.light_map(*l);
            let ri: [f64; 3] = std::array::from_fn(|r| (0..3).map(|c| inv[r][c] * l[c]).sum());
            // Luminance row of this light: LRGB's rows are all the luminance, otherwise the
            // mean of the three channels.
            let lum_const = if albedo {
                map.constant[0] as f64
            } else {
                map.constant.iter().map(|&v| v as f64).sum::<f64>() / 3.0
            };
            for r in 0..3 {
                cst[r] += ri[r] * lum_const;
            }
            for (j, m) in map.matrices.iter().enumerate() {
                for c in 0..3 {
                    let lum = if albedo {
                        m[0][c] as f64
                    } else {
                        (m[0][c] + m[1][c] + m[2][c]) as f64 / 3.0
                    };
                    for r in 0..3 {
                        mats[j][r][c] += ri[r] * lum;
                    }
                }
            }
        }
        Linear {
            constant: cst.map(|v| v as f32),
            matrices: mats
                .into_iter()
                .map(|m| m.map(|row| row.map(|v| v as f32)))
                .collect(),
        }
    }

    /// The relit colour (display-encoded, nominally 0..1) of a pixel whose layer values are
    /// `values` (one entry per layer, already mapped by scale and offset).
    pub fn shade(p: &Prepared, values: &[[f32; 3]]) -> [f32; 3] {
        let mut c = p.colour.constant;
        let mut g = p.normal.constant;
        for (j, v) in values.iter().enumerate() {
            let m = &p.colour.matrices[j];
            let n = &p.normal.matrices[j];
            for r in 0..3 {
                c[r] += m[r][0] * v[0] + m[r][1] * v[1] + m[r][2] * v[2];
                g[r] += n[r][0] * v[0] + n[r][1] * v[1] + n[r][2] * v[2];
            }
        }
        let lum = if p.albedo {
            c[0]
        } else {
            (c[0] + c[1] + c[2]) / 3.0
        };
        let tint = if p.albedo { values[0] } else { [1.0; 3] };
        if p.albedo {
            c = [c[0] * tint[0], c[1] * tint[1], c[2] * tint[2]];
        }
        match p.mode {
            Mode::Default => c,
            Mode::Normals => {
                let n = normal_of(g);
                n.map(|v| v * 0.5 + 0.5)
            }
            Mode::Specular => {
                let n = normal_of(g);
                let l = p.light;
                let h = norm([l[0], l[1], l[2] + 1.0]);
                let d = (n[0] * h[0] + n[1] * h[1] + n[2] * h[2]).max(0.0);
                let s = p.ks * d.powf(p.exponent) * lum.max(0.0);
                c.map(|v| p.kd * v + s)
            }
            Mode::DiffuseGain => {
                let l = p.light;
                let lambert = g[0] * l[0] + g[1] * l[1] + g[2] * l[2];
                let d = (p.gain - 1.0) * (lum - lambert);
                [c[0] + d * tint[0], c[1] + d * tint[1], c[2] + d * tint[2]]
            }
        }
    }

    /// The layer values of pixel `(x, y)` into `out` (one entry per layer).
    pub fn values(&self, x: u32, y: u32, out: &mut Vec<[f32; 3]>) {
        out.clear();
        out.extend((0..self.layers.len()).map(|j| self.coef(j, x, y)));
    }

    /// The relit image, display-encoded, `width · height` RGB triples (top-down), not clamped.
    pub fn relight_image(&self, look: &Look) -> Vec<[f32; 3]> {
        let p = self.prepare(look);
        let mut v = Vec::with_capacity(self.layers.len());
        let mut out = Vec::with_capacity(self.width as usize * self.height as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                self.values(x, y, &mut v);
                out.push(Rti::shade(&p, &v));
            }
        }
        out
    }

    /// The relit image as RGBA8 (display-encoded, clamped, rounded).
    pub fn relight_rgba8(&self, look: &Look) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width as usize * self.height as usize * 4);
        for c in self.relight_image(look) {
            out.extend(c.map(to_byte));
            out.push(255);
        }
        out
    }
}

/// Clamps and rounds a display value to a byte.
pub fn to_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 {
        v.map(|c| c / l)
    } else {
        DEFAULT_LIGHT
    }
}

/// The normal from the fitted `g`: facing the viewer when degenerate.
fn normal_of(g: [f32; 3]) -> [f32; 3] {
    let l = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
    if l > 1e-6 {
        g.map(|c| c / l)
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn invert3(a: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    std::array::from_fn(|r| {
        std::array::from_fn(|c| {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            (a[r1][c1] * a[r2][c2] - a[r1][c2] * a[r2][c1]) / det
        })
    })
}

//! Surface blur: edge-aware smoothing, a
//! bilateral filter on a bilateral grid (Chen, Paris and Durand 2007).
//!
//! - **Range** is the lightness `L = ∛Y` (0 … [`LMAX`], brighter clamps), so an edge
//!   threshold means the same in shadows and highlights. Colours are averaged with
//!   weights from lightness differences: edges are found on brightness.
//! - **Grid.** Cells of `c = ⌊σ⌋` view pixels (aligned to the image, not the region, so
//!   tiles match the whole image) by bins of `σ_r` lightness. Each cell sums the RGB and
//!   count of its pixels, split linearly between the two nearest bins; the grid is blurred
//!   by Gaussians (σ/c cells in space, one bin in range) and sliced trilinearly at each
//!   pixel's position and lightness: `Σ rgb / Σ w`.
//! - **Small radii** (σ below [`GRID_MIN`] view pixels) filter directly: Gaussian spatial
//!   and range weights over ±2σ, every `⌊σ/3⌋`-th pixel.
//!
//! Radius is a fraction of the image's long side (shown in percent), so the fit view and the export blur the
//! same structures. Amount mixes the result in; 0 is the identity (the node is left out).

use super::diffuse::{FALLBACK_LONG, LONG_FIELD};
use super::kernels::{fw, gauss_taps};
use crate::graph::{Kernel, NodeKind, Op, Placement, Program, Roi};
use crate::model::{Fields, ModuleKind, Params, Value};

/// The lightness range of the grid (`∛Y`, Y up to about 4).
pub const LMAX: f32 = 1.6;
/// Spatial σ (view pixels) from which the grid is used.
pub const GRID_MIN: f32 = 8.0;
/// Grid cells (space) and bins (range) added on each side for the blur.
pub const PAD: u32 = 3;
/// Radius at most (fraction of the long side: 5 %).
pub const MAX_RADIUS: f32 = 0.05;
/// Range σ, least and most (lightness).
pub const MIN_EDGES: f32 = 0.04;
pub const MAX_EDGES: f32 = 0.5;

/// Surface blur params (fields of [`crate::model::ModuleKind::SURFACE_BLUR`], version 1).
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceBlur {
    /// Mix of the blurred image, 0 … 1.
    pub amount: f32,
    /// Spatial σ, a fraction of the long side (0.0001 … [`MAX_RADIUS`]).
    pub radius: f32,
    /// Range σ in lightness ([`MIN_EDGES`] … [`MAX_EDGES`]).
    pub edges: f32,
    /// The image's long side in graph pixels.
    pub long: f32,
}

impl Default for SurfaceBlur {
    fn default() -> Self {
        Self {
            amount: 0.0,
            radius: 0.005,
            edges: 0.12,
            long: FALLBACK_LONG,
        }
    }
}

impl SurfaceBlur {
    pub fn from_fields(f: &Fields) -> Self {
        let d = Self::default();
        let get = |n: &str, dv: f32, lo: f32, hi: f32| {
            let v = f.f32_or(n, dv);
            if v.is_finite() { v.clamp(lo, hi) } else { dv }
        };
        let long = f.f32_or(LONG_FIELD, d.long);
        Self {
            amount: get("amount", d.amount, 0.0, 1.0),
            radius: get("radius", d.radius, 0.0001, MAX_RADIUS),
            edges: get("edges", d.edges, MIN_EDGES, MAX_EDGES),
            long: if long.is_finite() && long >= 1.0 {
                long
            } else {
                d.long
            },
        }
    }

    pub fn default_fields() -> Fields {
        let d = Self::default();
        Fields::new()
            .with("amount", Value::Float(d.amount))
            .with("radius", Value::Float(d.radius))
            .with("edges", Value::Float(d.edges))
    }

    pub(crate) fn of(p: &Params) -> Self {
        Self::from_fields(&p.to_fields())
    }
}

/// How one render filters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plan {
    /// Direct: σ, tap radius and tap step.
    Direct { sigma: f32, radius: u32, step: u32 },
    /// Grid: σ, cell side, bins (with padding).
    Grid { sigma: f32, cell: u32, bins: u32 },
}

pub fn plan(s: &SurfaceBlur) -> Plan {
    let sigma = (s.radius * s.long).max(0.3);
    if sigma < GRID_MIN {
        Plan::Direct {
            sigma,
            radius: (2.0 * sigma).ceil() as u32,
            step: ((sigma / 3.0).floor() as u32).max(1),
        }
    } else {
        Plan::Grid {
            sigma,
            cell: sigma.floor() as u32,
            bins: bins(s.edges),
        }
    }
}

/// Range bins with padding.
pub fn bins(edges: f32) -> u32 {
    (LMAX / edges).ceil() as u32 + 1 + 2 * PAD
}

pub fn halo(s: &SurfaceBlur) -> u32 {
    match plan(s) {
        Plan::Direct { radius, .. } => radius,
        Plan::Grid { cell, .. } => (PAD + 2) * cell + 1,
    }
}

/// The grid over a region: first cell (absolute) and cells across and down.
pub fn grid_extent(input: Roi, cell: u32) -> (i32, i32, u32, u32) {
    let c = cell as i32;
    let gx0 = (input.x as i32).div_euclid(c) - PAD as i32;
    let gy0 = (input.y as i32).div_euclid(c) - PAD as i32;
    let gx1 = ((input.x + input.w - 1) as i32).div_euclid(c) + PAD as i32;
    let gy1 = ((input.y + input.h - 1) as i32).div_euclid(c) + PAD as i32;
    (gx0, gy0, (gx1 - gx0 + 1) as u32, (gy1 - gy0 + 1) as u32)
}

/// The grid program: splat into grid A (offset 0), blur x into B, y into A, range into B.
/// Four planes per grid (R, G, B sums and the count). The direct path has no program work.
pub fn program(s: &SurfaceBlur, input: Roi) -> Program {
    let mut prog = Program::default();
    let Plan::Grid { sigma, cell, bins } = plan(s) else {
        // The same steps doing no work, so crossing to the grid builds nothing new.
        prog.step(&SB_SPLAT, [0, 0], vec![0; 12]);
        for _ in 0..3 {
            prog.step(&SB_BLUR, [0, 0], vec![0; 7 + PAD as usize + 1]);
        }
        prog.words = 1;
        return prog;
    };
    let (gx0, gy0, gw, gh) = grid_extent(input, cell);
    let g = gw * gh * bins;
    let inv_r = 1.0 / s.edges;
    prog.step(
        &SB_SPLAT,
        [gw, gh * bins],
        vec![
            gx0 as u32,
            gy0 as u32,
            gw,
            gh,
            bins,
            g,
            cell,
            input.x,
            input.y,
            input.right(),
            input.bottom(),
            fw(inv_r),
        ],
    );
    let space = gauss_taps(sigma / cell as f32, PAD);
    let range = gauss_taps(1.0, PAD);
    for (axis, src, dst) in [(0u32, 0, 4 * g), (1, 4 * g, 0), (2, 0, 4 * g)] {
        let taps = if axis == 2 { &range } else { &space };
        let mut w = vec![src, dst, gw, gh, bins, g, axis];
        w.extend(taps.iter().map(|t| fw(*t)));
        prog.step(&SB_BLUR, [gw, gh * bins], w);
    }
    prog.words = 8 * u64::from(g);
    prog
}

/// Shared WGSL: lightness.
pub const SURFACE_WGSL: &str = "fn sb_l(c: vec3<f32>) -> f32 {
    let y = max(dot(c, vec3<f32>(0.2627, 0.6780, 0.0593)), 1e-12);
    return min(pow(y, 1.0 / 3.0), 1.6);
}
";

/// Words: first cell x, y (i32), cells across, down, bins, grid size, cell side, region
/// x0, y0, x1, y1, 1/σ_r. One thread per cell and bin: the sums of the cell's pixels,
/// each split linearly between its two nearest bins.
pub static SB_SPLAT: Kernel = Kernel {
    name: "surface_splat_v1",
    atomic: false,
    helpers: &[SURFACE_WGSL],
    body: "    let gw = pu(p + 2u);
    let gh = pu(p + 3u);
    let g = pu(p + 5u);
    let c = i32(pu(p + 6u));
    let gy = t.y % gh;
    let gz = t.y / gh;
    let ax = bitcast<i32>(pu(p)) + i32(t.x);
    let ay = bitcast<i32>(pu(p + 1u)) + i32(gy);
    let x0 = max(ax * c, i32(pu(p + 7u)));
    let x1 = min(ax * c + c, i32(pu(p + 9u)));
    let y0 = max(ay * c, i32(pu(p + 8u)));
    let y1 = min(ay * c + c, i32(pu(p + 10u)));
    let ir = pf(p + 11u);
    var acc = vec4<f32>(0.0);
    for (var y = y0; y < y1; y++) {
        for (var x = x0; x < x1; x++) {
            let v = src_at(x, y).rgb;
            let zf = sb_l(v) * ir + 3.0;
            let wt = max(0.0, 1.0 - abs(zf - f32(gz)));
            acc += wt * vec4<f32>(v, 1.0);
        }
    }
    let i = (gz * gh + gy) * gw + t.x;
    S[i] = acc.x;
    S[i + g] = acc.y;
    S[i + 2u * g] = acc.z;
    S[i + 3u * g] = acc.w;",
};

/// Words: source grid, destination grid, cells across, down, bins, grid size, axis (0 x,
/// 1 y, 2 range), taps (centre first, [`PAD`] + 1). One Gaussian pass along an axis.
pub static SB_BLUR: Kernel = Kernel {
    name: "surface_blur_v1",
    atomic: false,
    helpers: &[],
    body: "    let so = pu(p);
    let gw = i32(pu(p + 2u));
    let gh = i32(pu(p + 3u));
    let nb = i32(pu(p + 4u));
    let g = pu(p + 5u);
    let axis = pu(p + 6u);
    let gx = i32(t.x);
    let gy = i32(t.y) % gh;
    let gz = i32(t.y) / gh;
    var acc = vec4<f32>(0.0);
    for (var k = -3; k <= 3; k++) {
        var x = gx;
        var y = gy;
        var z = gz;
        if (axis == 0u) { x += k; } else if (axis == 1u) { y += k; } else { z += k; }
        if (x < 0 || y < 0 || z < 0 || x >= gw || y >= gh || z >= nb) { continue; }
        let wt = pf(p + 7u + u32(abs(k)));
        let i = so + u32((z * gh + y) * gw + x);
        acc += wt * vec4<f32>(S[i], S[i + g], S[i + 2u * g], S[i + 3u * g]);
    }
    let o = pu(p + 1u) + u32((gz * gh + gy) * gw + gx);
    S[o] = acc.x;
    S[o + g] = acc.y;
    S[o + 2u * g] = acc.z;
    S[o + 3u * g] = acc.w;",
};

/// Surface blur, param version 1.
pub struct SurfaceBlurOp;

/// Node words: amount, grid (1) or direct (0), σ, tap radius or cell, tap step or bins,
/// 1/σ_r.
const WORDS: u32 = 6;

impl Op for SurfaceBlurOp {
    fn name(&self) -> &'static str {
        "surface_blur_v1"
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Neighbourhood
    }
    fn halo(&self, p: &Params, _scale: f32) -> u32 {
        halo(&SurfaceBlur::of(p))
    }
    fn heavy(&self) -> bool {
        true
    }
    fn param_words(&self) -> u32 {
        WORDS
    }
    fn write_params(&self, p: &Params, _scale: f32, out: &mut [u32]) {
        let s = SurfaceBlur::of(p);
        out[0] = fw(s.amount);
        out[5] = fw(1.0 / s.edges);
        match plan(&s) {
            Plan::Direct {
                sigma,
                radius,
                step,
            } => {
                out[1] = 0;
                out[2] = fw(sigma);
                out[3] = radius;
                out[4] = step;
            }
            Plan::Grid { sigma, cell, bins } => {
                out[1] = 1;
                out[2] = fw(sigma);
                out[3] = cell;
                out[4] = bins;
            }
        }
    }
    fn helpers(&self) -> &'static [&'static str] {
        &[SURFACE_WGSL]
    }
    fn identity(&self, p: &Params) -> bool {
        SurfaceBlur::of(p).amount == 0.0
    }
    fn place(&self, p: &Params, at: &Placement<'_>) -> Option<Params> {
        let mut f = p.to_fields();
        f.set(LONG_FIELD, Value::Float(at.image.0.max(at.image.1) as f32));
        Some(Params::from_fields(ModuleKind::SURFACE_BLUR, f))
    }
    fn local(&self, p: &Params, input: Roi, _scale: f32) -> Option<Program> {
        Some(program(&SurfaceBlur::of(p), input))
    }
    fn wgsl(&self) -> &'static str {
        "    let c = src_at(x, y);
    let ir = pf(p + 5u);
    let lc0 = sb_l(c.rgb);
    var r = c.rgb;
    if (pu(p + 1u) == 0u) {
        let sg = pf(p + 2u);
        let rad = i32(pu(p + 3u));
        let stp = i32(pu(p + 4u));
        let is2 = 0.5 / (sg * sg);
        let ir2 = 0.5 * ir * ir;
        var acc = vec4<f32>(0.0);
        for (var j = -rad; j <= rad; j += stp) {
            for (var k = -rad; k <= rad; k += stp) {
                let v = src_at(x + k, y + j).rgb;
                let dl = sb_l(v) - lc0;
                let wt = exp(-f32(j * j + k * k) * is2 - dl * dl * ir2);
                acc += wt * vec4<f32>(v, 1.0);
            }
        }
        r = acc.xyz / acc.w;
    } else {
        let cl = i32(pu(p + 3u));
        let cf = f32(cl);
        let nb = i32(pu(p + 4u));
        let rx = i32(pu(16u));
        let ry = i32(pu(17u));
        let gx0 = rx / cl - 3;
        let gy0 = ry / cl - 3;
        let gw = (rx + i32(pu(18u)) - 1) / cl + 3 - gx0 + 1;
        let gh = (ry + i32(pu(19u)) - 1) / cl + 3 - gy0 + 1;
        let g = u32(gw * gh * nb);
        let fx = (f32(x) + 0.5) / cf - 0.5 - f32(gx0);
        let fy = (f32(y) + 0.5) / cf - 0.5 - f32(gy0);
        let fz = lc0 * ir + 3.0;
        let ix = i32(floor(fx));
        let iy = i32(floor(fy));
        let iz = i32(floor(fz));
        let tx = fx - f32(ix);
        let ty = fy - f32(iy);
        let tz = fz - f32(iz);
        var acc = vec4<f32>(0.0);
        for (var c8 = 0; c8 < 8; c8++) {
            let dx = c8 & 1;
            let dy = (c8 >> 1) & 1;
            let dz = (c8 >> 2) & 1;
            let qx = clamp(ix + dx, 0, gw - 1);
            let qy = clamp(iy + dy, 0, gh - 1);
            let qz = clamp(iz + dz, 0, nb - 1);
            let wt = select(1.0 - tx, tx, dx == 1) * select(1.0 - ty, ty, dy == 1)
                * select(1.0 - tz, tz, dz == 1);
            let i = 4u * g + u32((qz * gh + qy) * gw + qx);
            acc += wt * vec4<f32>(lc(i), lc(i + g), lc(i + 2u * g), lc(i + 3u * g));
        }
        if (acc.w > 1e-12) { r = acc.xyz / acc.w; }
    }
    return vec4<f32>(mix(c.rgb, r, pf(p)), c.a);"
    }
}

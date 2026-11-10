//! CPU references of the ops (tests only): the same f32 arithmetic as the WGSL, from the
//! same uniforms, so GPU = CPU within float rounding of `pow`, `exp2`, `log2`. Statistics
//! and local programs run through [`super::tone::cpu`], on the same grid as the engine.

use crate::graph::{Graph, Roi, stats_grid};

use super::colour::cpu as colour_cpu;
use super::tone::cpu::{self as tone_cpu, Img};
use super::{colour_calibration, exposure, output, view};

fn mat3(m: &[f32], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i * 3] * v[0] + m[i * 3 + 1] * v[1] + m[i * 3 + 2] * v[2])
}

fn compress_distance(d: f32, thr: f32, lim: f32, pwr: f32) -> f32 {
    if d <= thr || lim <= thr {
        return d;
    }
    let s = (lim - thr) / (((1.0 - thr) / (lim - thr)).powf(-pwr) - 1.0).powf(1.0 / pwr);
    let x = (d - thr) / s;
    thr + s * x / (1.0 + x.powf(pwr)).powf(1.0 / pwr)
}

/// `u`: threshold, strength, limits (3), power.
fn gamut(v: [f32; 3], u: &[f32]) -> [f32; 3] {
    let ach = v[0].max(v[1]).max(v[2]);
    let k = u[1];
    if ach <= 0.0 || k <= 0.0 {
        return v;
    }
    std::array::from_fn(|i| {
        let d = (ach - v[i]) / ach;
        let c = compress_distance(d, u[0], u[2 + i], u[5]);
        let d2 = d + (c - d) * k.min(1.0);
        ach - d2 * ach
    })
}

fn rational(x: f32, s: f32, l: f32, pw: f32) -> f32 {
    let a = s * x;
    if a <= 0.0 {
        return 0.0;
    }
    a / (1.0 + (a / l).powf(pw)).powf(1.0 / pw)
}

/// `u` from the method word on (as `dv_view`).
fn view_curve(x: f32, u: &[f32]) -> f32 {
    let m = u[0];
    let q = &u[2..];
    if m < 0.5 {
        let f = if x > 0.0 {
            (1.0 / (1.0 + (q[1] / x).powf(q[0]))).powf(q[2])
        } else {
            0.0
        };
        return q[4] + (q[3] - q[4]) * f;
    }
    if m < 1.5 {
        if x <= 0.0 {
            return q[10];
        }
        let u = (x / 0.1845).log2();
        return if u <= q[0] {
            q[10]
        } else if u >= q[1] {
            q[9]
        } else if u < q[3] {
            q[10] + (q[4] - q[10]) * ((u - q[0]) / (q[3] - q[0])).powf(q[5])
        } else if u > q[6] {
            q[9] - (q[9] - q[7]) * ((q[1] - u) / (q[1] - q[6])).powf(q[8])
        } else {
            0.1845 * (q[2] * u).exp2()
        };
    }
    let b = q[10];
    if x <= 0.0 {
        return b;
    }
    let u = (x / 0.1845).log2();
    let d = if u <= q[0] {
        0.0
    } else if u >= q[1] {
        q[4]
    } else if u >= 0.0 {
        q[3] + rational(u, q[2], q[6], q[8])
    } else {
        q[3] - rational(-u, q[2], q[5], q[7])
    };
    let d = d.clamp(0.0, q[4]);
    let e = if d > 0.0 { d.powf(q[11]) } else { 0.0 };
    b + (1.0 - b) * e
}

/// One op on one pixel, by WGSL name.
pub fn apply(name: &str, u: &[f32], c: [f32; 4]) -> [f32; 4] {
    let rgb = [c[0], c[1], c[2]];
    let out = match name {
        "exposure_v1" => rgb.map(|v| (v - u[1]) * u[0]),
        "colour_calibration_v1" => gamut(mat3(&u[..9], rgb), &u[9..15]),
        "view_v1" => {
            let v = mat3(&u[..9], rgb);
            let q = &u[18..];
            let pc = v.map(|x| view_curve(x, q));
            let mx = v[0].max(v[1]).max(v[2]);
            let mn = v[0].min(v[1]).min(v[2]);
            let h = q[1];
            let o = if h > 0.0 && mx > mn {
                let fx = view_curve(mx, q);
                let fnn = view_curve(mn, q);
                let hp = v.map(|x| fnn + (fx - fnn) * ((x - mn) / (mx - mn)));
                std::array::from_fn(|i| pc[i] + (hp[i] - pc[i]) * h)
            } else {
                pc
            };
            mat3(&u[9..18], o)
        }
        "output_v1" => gamut(mat3(&u[..9], rgb), &u[9..15]).map(|v| v.clamp(0.0, u[15])),
        n => panic!("no CPU reference for {n}"),
    };
    [out[0], out[1], out[2], c[3]]
}

/// The uniforms an op writes, as f32.
fn floats(words: &[u32]) -> Vec<f32> {
    words.iter().map(|w| f32::from_bits(*w)).collect()
}

/// One node over a whole `w`×`h` image at `scale`, with its statistics `st`.
fn apply_node(
    n: &crate::graph::Node,
    w: u32,
    h: u32,
    img: &[f32],
    st: &[f32],
    scale: f32,
) -> Vec<f32> {
    let mut words = vec![0u32; n.op.param_words() as usize];
    n.op.write_params(&n.params, scale, &mut words);
    let u = floats(&words);
    let name = n.op.name();
    let full = Roi::full(w, h);
    let lc =
        n.op.local(&n.params, full, scale)
            .map(|p| tone_cpu::run_program(&p, Img { w, h, px: img }, st, full))
            .unwrap_or_default();
    let mut out = img.to_vec();
    for (i, px) in out.chunks_exact_mut(4).enumerate() {
        let c = [px[0], px[1], px[2], px[3]];
        let (x, y) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
        let r = match name {
            "exposure_v1" if u[2] != 0.0 => {
                let g = (u[3] - st[2]).exp2();
                [
                    (c[0] - u[1]) * g,
                    (c[1] - u[1]) * g,
                    (c[2] - u[1]) * g,
                    c[3],
                ]
            }
            "tone_eq_v1" => tone_cpu::tone_eq(&u, c, st, x, y, w, h),
            "local_contrast_v1" => tone_cpu::local_contrast(&words, c, st, &lc, full, x, y, w, h),
            "curves_v1" => tone_cpu::curves(&words, c),
            "colour_balance_v1" => colour_cpu::balance(&u, c),
            "colour_equalizer_v1" => colour_cpu::equalizer(&u, c),
            "point_colour_v1" => colour_cpu::point(&u, c),
            "black_white_v1" => colour_cpu::black_white(&u, c),
            "lut_3d_v1" => colour_cpu::lut(&u, &n.data.as_ref().expect("a table").words, c),
            _ => apply(name, &u, c),
        };
        px.copy_from_slice(&r);
    }
    out
}

/// The statistics grid of a `w`×`h` RGBA image, as the engine reduces it.
pub fn grid(w: u32, h: u32, rgba: &[f32]) -> (u32, u32, Vec<f32>) {
    let (gw, gh) = stats_grid(w, h);
    let mut p = crate::graph::Program {
        words: u64::from(gw) * u64::from(gh) * 4,
        steps: Vec::new(),
        persist: None,
    };
    p.step(&crate::gpu::wgsl::GRID_REDUCE, [gw, gh], Vec::new());
    let g = tone_cpu::run_program(&p, Img { w, h, px: rgba }, &[], Roi::full(w, h));
    (gw, gh, g)
}

/// Runs every node of `g` on a whole `w`×`h` RGBA image at view scale 1: statistics on
/// the grid (the nodes before each one run over the grid first), then every node.
pub fn run_image(g: &Graph, w: u32, h: u32, rgba: &[f32]) -> Vec<f32> {
    let mut st: Vec<Vec<f32>> = vec![Vec::new(); g.nodes.len()];
    if g.nodes.iter().any(|n| n.stats) {
        let (gw, gh, mut cur) = grid(w, h, rgba);
        let gscale = gw as f32 / w as f32;
        for (i, n) in g.nodes.iter().enumerate() {
            if n.stats
                && let Some(p) = n.op.stats(&n.params, gw, gh, gscale)
            {
                let groi = Roi::full(gw, gh);
                st[i] = tone_cpu::run_program(
                    &p,
                    Img {
                        w: gw,
                        h: gh,
                        px: &cur,
                    },
                    &[],
                    groi,
                );
            }
            cur = apply_node(n, gw, gh, &cur, &st[i], gscale);
        }
    }
    let mut out = rgba.to_vec();
    for (i, n) in g.nodes.iter().enumerate() {
        out = apply_node(n, w, h, &out, &st[i], 1.0);
    }
    out
}

/// Runs every node of `g` on pixels that need no neighbours (each its own 1×1 image).
pub fn run(g: &Graph, rgba: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        out.extend(run_image(g, 1, 1, px));
    }
    out
}

/// The uniforms of each basic op for params (keeps the op modules' `uniforms` in use here).
pub fn uniforms_of(name: &str, p: &crate::model::Params) -> Vec<f32> {
    match name {
        "exposure_v1" => exposure::uniforms(p).to_vec(),
        "colour_calibration_v1" => colour_calibration::uniforms(p).to_vec(),
        "tone_eq_v1" => super::tone::equalizer::ToneEq::from_fields(&p.to_fields())
            .uniforms()
            .to_vec(),
        "view_v1" => view::uniforms(p).to_vec(),
        "output_v1" => output::uniforms(p).to_vec(),
        n => panic!("unknown op {n}"),
    }
}

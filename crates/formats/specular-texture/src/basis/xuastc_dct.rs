//! XUASTC LDR weight-grid DCT, written from Binomial's published "XUASTC LDR - Weight Grid DCT":
//! zigzag order, adaptive quantisation (JPEG luma table resampled, scaled by the endpoint span
//! and weight precision), deadzone dequantisation and an orthonormal float IDCT, then the
//! weights requantised to the block's ISE range.

use std::sync::OnceLock;

use super::astc_pack::{LogBlock, cem_values, weight_quant};
use super::xuastc_ep::decode_endpoints;

/// DC symbol levels: 9 for weight ranges up to 8 levels, else 33.
pub(super) fn dc_levels(wr: u8) -> u32 {
    if wr <= 5 { 9 } else { 33 }
}

const SCALE_QUANT_STEPS: [f32; 12] = [
    1.513_331_4,
    1.411_988_1,
    1.355_882_2,
    1.317_431_6,
    1.288_359_5,
    1.245_731,
    1.214_814_1,
    1.190_679_2,
    1.154_316_5,
    1.127_349_9,
    1.106_015_7,
    1.073_489_7,
];

const JPEG_Y: [[i32; 8]; 8] = [
    [4, 11, 10, 16, 24, 40, 51, 61],
    [12, 12, 14, 19, 26, 58, 60, 55],
    [14, 13, 16, 24, 40, 57, 69, 56],
    [14, 17, 22, 29, 51, 87, 80, 62],
    [18, 22, 37, 56, 68, 109, 103, 77],
    [24, 35, 55, 64, 81, 104, 113, 92],
    [49, 64, 78, 87, 103, 121, 120, 101],
    [72, 92, 95, 98, 112, 100, 103, 99],
];

/// Zigzag order of a `w`×`h` coefficient grid (anti-diagonals, odd ones reversed).
fn zigzag(w: usize, h: usize) -> Vec<u8> {
    let mut order = Vec::with_capacity(w * h);
    for s in 0..w + h - 1 {
        let x0 = if s < h { 0 } else { s - h + 1 };
        let x1 = if s < w { s } else { w - 1 };
        let diag: Vec<u8> = (x0..=x1).map(|x| (x + (s - x) * w) as u8).collect();
        if s & 1 == 1 {
            order.extend(diag.iter().rev());
        } else {
            order.extend(diag);
        }
    }
    order
}

struct Tables {
    /// Per (w − 2, h − 2): zigzag order.
    zig: Vec<Vec<u8>>,
    /// Per size n − 2: (alpha(u), cos(π(2x + 1)u / 2n) at [u·n + x]).
    basis: Vec<(Vec<f32>, Vec<f32>)>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut zig = Vec::new();
        for w in 2..=12 {
            for h in 2..=12 {
                zig.push(zigzag(w, h));
            }
        }
        let basis = (2..=12usize)
            .map(|n| {
                let mut t = vec![0f32; n * n];
                let inv = 1.0 / n as f64;
                let a: Vec<f32> = (0..n)
                    .map(|u| if u == 0 { inv.sqrt() } else { (2.0 * inv).sqrt() } as f32)
                    .collect();
                for u in 0..n {
                    for x in 0..n {
                        let ang =
                            std::f64::consts::PI * (2 * x + 1) as f64 * u as f64 / (2 * n) as f64;
                        t[u * n + x] = ang.cos() as f32;
                    }
                }
                (a, t)
            })
            .collect();
        Tables { zig, basis }
    })
}

/// The largest endpoint span the weight plane interpolates over.
fn span_len(b: &LogBlock, plane: usize) -> f32 {
    if b.dual {
        let (l, h) = decode_endpoints(b.cem, &b.endpoints, b.erange);
        let ccs = usize::from(b.ccs);
        let mut s = 0f32;
        for c in 0..4 {
            if (plane == 1) == (c == ccs) {
                let d = h[c] as f32 - l[c] as f32;
                s += d * d;
            }
        }
        s.sqrt()
    } else {
        let n = cem_values(b.cem);
        let mut best = 0f32;
        for p in 0..usize::from(b.parts) {
            let (l, h) = decode_endpoints(b.cem, &b.endpoints[p * n..], b.erange);
            let mut s = 0f32;
            for c in 0..4 {
                let d = h[c] as f32 - l[c] as f32;
                s += d * d;
            }
            best = best.max(s.sqrt());
        }
        best
    }
}

fn level_scale(q: f32, span: f32, wr: u8) -> f32 {
    let q = q.clamp(1.0, 100.0);
    let mut s = if q < 50.0 {
        5000.0 / q
    } else {
        200.0 - 2.0 * q
    };
    s *= 1.0 / 100.0;
    let f = 64.0 / span.max(14.0) * SCALE_QUANT_STEPS[usize::from(wr)];
    s * f
}

fn quant_step(q: f32, sx: f32, sy: f32, scale: f32, x: usize, y: usize) -> i32 {
    if q >= 100.0 {
        return 1;
    }
    let i = (x as f32 * sx).clamp(0.0, 7.0);
    let j = (y as f32 * sy).clamp(0.0, 7.0);
    let (i0, j0) = (i as usize, j as usize);
    let (i1, j1) = ((i0 + 1).min(7), (j0 + 1).min(7));
    let (ti, tj) = (i - i0 as f32, j - j0 as f32);
    let a = (1.0 - ti) * JPEG_Y[j0][i0] as f32 + ti * JPEG_Y[j0][i1] as f32;
    let b = (1.0 - ti) * JPEG_Y[j1][i0] as f32 + ti * JPEG_Y[j1][i1] as f32;
    let base = (1.0 - tj) * a + tj * b;
    ((base * scale + 0.5) as i32).max(1)
}

/// Rebuild plane `plane` of `b`'s weights from a DC symbol and (run, value) AC pairs; false on
/// a damaged coefficient list. `bw`×`bh` is the ASTC block size.
pub(super) fn decode_plane(
    b: &mut LogBlock,
    plane: usize,
    q: f32,
    bw: u8,
    bh: u8,
    dc: u32,
    coeffs: &[(u32, i32)],
) -> bool {
    let (gw, gh) = (usize::from(b.grid_w), usize::from(b.grid_h));
    if !(2..=12).contains(&gw) || !(2..=12).contains(&gh) {
        return false;
    }
    let total = gw * gh;
    let planes = if b.dual { 2 } else { 1 };
    let t = tables();
    let zig = &t.zig[(gw - 2) * 11 + (gh - 2)];
    let scale = level_scale(q, span_len(b, plane), b.wrange);
    let dc_scale = if b.wrange <= 5 { 1.0 / 8.0 } else { 0.5 };
    let mean = dc as f32 / dc_scale;
    let (sx, sy) = (8.0 / f32::from(bw), 8.0 / f32::from(bh));
    let mut c = [0f32; 144];
    let mut z = 1usize;
    for &(run, v) in coeffs {
        if z + run as usize > total {
            return false;
        }
        z += run as usize;
        if z >= total {
            break;
        }
        let idx = usize::from(zig[z]);
        let (x, y) = (idx % gw, idx / gw);
        let step = quant_step(q, sx, sy, scale, x, y);
        c[idx] = if (x, y) == (1, 0) || (x, y) == (0, 1) {
            (v * step) as f32
        } else if v == 0 || step <= 0 {
            0.0
        } else {
            let mag = 0.5 * step as f32 + v.unsigned_abs() as f32 * step as f32;
            if v < 0 { -mag } else { mag }
        };
        z += 1;
    }
    // Columns (vertical, grid height), then rows (horizontal, grid width).
    let ((ar, cr), (ac, cc)) = (&t.basis[gh - 2], &t.basis[gw - 2]);
    let mut work = [0f32; 144];
    for v in 0..gw {
        let mut sums = [0f32; 12];
        for u in 0..gh {
            let y = c[u * gw + v];
            if y == 0.0 {
                continue;
            }
            let y = y * ar[u];
            for (x, s) in sums.iter_mut().enumerate().take(gh) {
                *s += y * cr[u * gh + x];
            }
        }
        for x in 0..gh {
            work[x * gw + v] = sums[x];
        }
    }
    let wq = &weight_quant()[usize::from(b.wrange)];
    for x in 0..gh {
        for y in 0..gw {
            let mut s = 0f32;
            for v in 0..gw {
                s += (work[x * gw + v] * ac[v]) * cc[v * gw + y];
            }
            let w = (mean + s).round().clamp(0.0, 64.0) as usize;
            b.weights[(y + x * gw) * planes + plane] = wq.val_to_ise[w];
        }
    }
    true
}

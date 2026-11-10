//! A CPU rasteriser of Gaussian splats, for thumbnails and as the GPU view's reference: each
//! splat's covariance is projected to the screen (the EWA approximation: the projection's
//! Jacobian at the centre, plus a 0.3 px² low-pass), the splats are binned into 16 px tiles,
//! sorted front to back within each tile and blended until a pixel is opaque. Tiles render in
//! parallel. Colour is display-referred (as trained) and blended as such.

use rayon::prelude::*;

use crate::Splats;

const TILE: usize = 16;

/// Where the splats are seen from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub right: [f32; 3],
    pub down: [f32; 3],
    pub forward: [f32; 3],
    /// The eye in perspective; `None` for an orthographic view centred on `target`.
    pub eye: Option<[f32; 3]>,
    pub target: [f32; 3],
    /// Pixels per unit of x/z (perspective, the focal length in px) or per model unit
    /// (orthographic).
    pub focal: f32,
}

/// One splat projected to the screen.
#[derive(Clone, Copy)]
struct Projected {
    x: f32,
    y: f32,
    depth: f32,
    /// The inverse 2D covariance: a, b, c of a x² + 2 b x y + c y².
    conic: [f32; 3],
    radius: f32,
    color: [f32; 3],
    opacity: f32,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The 2D covariance of splat `i` under `v` at view-space position `t`, with the low-pass.
pub(crate) fn cov2d(s: &Splats, i: usize, v: &View, t: [f32; 3]) -> [f32; 3] {
    let c = crate::math::full(s.covariances[i]);
    let w = [v.right, v.down, v.forward];
    // Σ' = W Σ Wᵀ
    let ws: [[f32; 3]; 3] = std::array::from_fn(|r| {
        std::array::from_fn(|k| w[r][0] * c[0][k] + w[r][1] * c[1][k] + w[r][2] * c[2][k])
    });
    let sv = |r: usize, k: usize| dot(ws[r], w[k]);
    let (j0, j1) = match v.eye {
        Some(_) => {
            let f = v.focal;
            let z = t[2];
            (
                [f / z, 0.0, -f * t[0] / (z * z)],
                [0.0, f / z, -f * t[1] / (z * z)],
            )
        }
        None => ([v.focal, 0.0, 0.0], [0.0, v.focal, 0.0]),
    };
    let s3: [[f32; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|k| sv(r, k)));
    let js = |j: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|k| j[0] * s3[0][k] + j[1] * s3[1][k] + j[2] * s3[2][k])
    };
    let a = js(j0);
    let b = js(j1);
    [dot(a, j0) + 0.3, dot(a, j1), dot(b, j1) + 0.3]
}

/// The anti-aliasing compensation of a splat trained with it: √(det Σ / det (Σ + 0.3 I)).
pub(crate) fn aa_factor(cov: [f32; 3]) -> f32 {
    let det_f = cov[0] * cov[2] - cov[1] * cov[1];
    let det = (cov[0] - 0.3) * (cov[2] - 0.3) - cov[1] * cov[1];
    (det.max(0.0) / det_f.max(1e-12)).sqrt()
}

fn project(s: &Splats, v: &View, w: usize, h: usize) -> Vec<Option<Projected>> {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let origin = v.eye.unwrap_or(v.target);
    // Centres off screen by more than this share of the view are clamped for the Jacobian.
    let limx = 1.3 * cx / v.focal;
    let limy = 1.3 * cy / v.focal;
    (0..s.len())
        .into_par_iter()
        .map(|i| {
            let d = sub(s.positions[i], origin);
            let mut t = [dot(d, v.right), dot(d, v.down), dot(d, v.forward)];
            let (x, y) = match v.eye {
                Some(_) => {
                    if t[2] < 1e-3 {
                        return None;
                    }
                    let (x, y) = (cx + v.focal * t[0] / t[2], cy + v.focal * t[1] / t[2]);
                    t[0] = t[0].clamp(-limx * t[2], limx * t[2]);
                    t[1] = t[1].clamp(-limy * t[2], limy * t[2]);
                    (x, y)
                }
                None => (cx + v.focal * t[0], cy + v.focal * t[1]),
            };
            let cov = cov2d(s, i, v, t);
            let det = cov[0] * cov[2] - cov[1] * cov[1];
            if det.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) || !det.is_finite() {
                return None;
            }
            let mut opacity = s.colors[i][3];
            if s.antialiased {
                opacity *= aa_factor(cov);
            }
            // Beyond where opacity · e^(−r²/2σ²) drops under 1/255 nothing is drawn: at most
            // 3σ, less for faint splats (which keeps faint floaters out of most tiles).
            if opacity * 255.0 < 1.0 {
                return None;
            }
            let reach = (2.0 * (opacity * 255.0).ln()).sqrt().min(3.0);
            let mid = 0.5 * (cov[0] + cov[2]);
            let l1 = mid + (mid * mid - det).max(0.1).sqrt();
            let radius = (reach * l1.sqrt()).ceil();
            if x + radius < 0.0
                || y + radius < 0.0
                || x - radius > w as f32
                || y - radius > h as f32
            {
                return None;
            }
            let dir = match v.eye {
                Some(e) => {
                    let d = sub(s.positions[i], e);
                    let l = dot(d, d).sqrt().max(1e-20);
                    d.map(|c| c / l)
                }
                None => v.forward,
            };
            Some(Projected {
                x,
                y,
                depth: t[2],
                conic: [cov[2] / det, -cov[1] / det, cov[0] / det],
                radius,
                color: s.color_towards(i, dir),
                opacity,
            })
        })
        .collect()
}

/// Renders `s` seen from `v` to straight RGBA8 of `width × height` (transparent where nothing
/// is), colour as stored (sRGB-like).
pub fn render_view(s: &Splats, v: &View, width: u32, height: u32) -> Vec<u8> {
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let proj = project(s, v, w, h);
    let (tw, th) = (w.div_ceil(TILE), h.div_ceil(TILE));
    let mut bins: Vec<Vec<(f32, u32)>> = vec![Vec::new(); tw * th];
    for (i, p) in proj.iter().enumerate() {
        let Some(p) = p else { continue };
        let x0 = ((p.x - p.radius).max(0.0) as usize / TILE).min(tw - 1);
        let x1 = ((p.x + p.radius).max(0.0) as usize / TILE).min(tw - 1);
        let y0 = ((p.y - p.radius).max(0.0) as usize / TILE).min(th - 1);
        let y1 = ((p.y + p.radius).max(0.0) as usize / TILE).min(th - 1);
        for ty in y0..=y1 {
            for tx in x0..=x1 {
                bins[ty * tw + tx].push((p.depth, i as u32));
            }
        }
    }
    // Tile by tile in parallel; within a tile splat by splat, front to back, over the pixels
    // of its box that are not yet opaque.
    let tiles: Vec<Vec<u8>> = (0..tw * th)
        .into_par_iter()
        .map(|t| {
            let (tx, ty) = (t % tw, t / tw);
            let (x0, y0) = (tx * TILE, ty * TILE);
            let (x1, y1) = ((x0 + TILE).min(w), (y0 + TILE).min(h));
            let mut bin = bins[t].clone();
            bin.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            let mut c = [[0f32; 3]; TILE * TILE];
            let mut trans = [1f32; TILE * TILE];
            let mut open = (x1 - x0) * (y1 - y0);
            for &(_, i) in &bin {
                if open == 0 {
                    break;
                }
                let p = proj[i as usize].as_ref().expect("binned");
                let bx0 = ((p.x - p.radius).floor().max(x0 as f32) as usize).max(x0);
                let by0 = ((p.y - p.radius).floor().max(y0 as f32) as usize).max(y0);
                let bx1 = ((p.x + p.radius).ceil().max(0.0) as usize + 1).min(x1);
                let by1 = ((p.y + p.radius).ceil().max(0.0) as usize + 1).min(y1);
                for y in by0..by1 {
                    for x in bx0..bx1 {
                        let k = (y - y0) * TILE + (x - x0);
                        if trans[k] < 1e-4 {
                            continue;
                        }
                        let (dx, dy) = (x as f32 + 0.5 - p.x, y as f32 + 0.5 - p.y);
                        let power = -0.5 * (p.conic[0] * dx * dx + p.conic[2] * dy * dy)
                            - p.conic[1] * dx * dy;
                        if power > 0.0 {
                            continue;
                        }
                        let a = (p.opacity * power.exp()).min(0.99);
                        if a < 1.0 / 255.0 {
                            continue;
                        }
                        for (o, v) in c[k].iter_mut().zip(p.color) {
                            *o += v * a * trans[k];
                        }
                        trans[k] *= 1.0 - a;
                        if trans[k] < 1e-4 {
                            open -= 1;
                        }
                    }
                }
            }
            let mut px = vec![0u8; TILE * TILE * 4];
            for k in 0..TILE * TILE {
                let alpha = 1.0 - trans[k];
                if alpha > 0.0 {
                    for ch in 0..3 {
                        let v = (c[k][ch] / alpha).clamp(0.0, 1.0);
                        px[k * 4 + ch] = (v * 255.0).round() as u8;
                    }
                }
                px[k * 4 + 3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            px
        })
        .collect();
    let mut out = vec![0u8; w * h * 4];
    for (t, px) in tiles.iter().enumerate() {
        let (x0, y0) = ((t % tw) * TILE, (t / tw) * TILE);
        let x1 = (x0 + TILE).min(w);
        for y in y0..(y0 + TILE).min(h) {
            let src = &px[(y - y0) * TILE * 4..][..(x1 - x0) * 4];
            out[(y * w + x0) * 4..(y * w + x1) * 4].copy_from_slice(src);
        }
    }
    out
}

/// The orthographic isometric view (as the mesh previews use) fitting `s`'s bounds into
/// `width × height` px.
pub fn iso_view(s: &Splats, width: u32, height: u32) -> View {
    let [right, down, forward] = specular_mesh::preview::ISO;
    let b = s.bounds;
    let c = b.centre();
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for i in 0..8 {
        let p = [
            if i & 1 == 0 { b.min[0] } else { b.max[0] },
            if i & 2 == 0 { b.min[1] } else { b.max[1] },
            if i & 4 == 0 { b.min[2] } else { b.max[2] },
        ];
        let q = sub(p, c);
        for (k, a) in [right, down].into_iter().enumerate() {
            lo[k] = lo[k].min(dot(q, a));
            hi[k] = hi[k].max(dot(q, a));
        }
    }
    let ext = [(hi[0] - lo[0]).max(1e-20), (hi[1] - lo[1]).max(1e-20)];
    let focal = 0.92 * (width as f32 / ext[0]).min(height as f32 / ext[1]);
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    let target = [0, 1, 2].map(|k| c[k] + right[k] * mid[0] + down[k] * mid[1]);
    View {
        right,
        down,
        forward,
        eye: None,
        target,
        focal,
    }
}

/// The aspect (width over height) of the iso view of `s`'s bounds, at least 1:4.
pub fn aspect(s: &Splats) -> f32 {
    let v = iso_view(s, 1000, 1000);
    let b = s.bounds;
    let c = b.centre();
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for i in 0..8 {
        let p = [
            if i & 1 == 0 { b.min[0] } else { b.max[0] },
            if i & 2 == 0 { b.min[1] } else { b.max[1] },
            if i & 4 == 0 { b.min[2] } else { b.max[2] },
        ];
        let q = sub(p, c);
        for (k, a) in [v.right, v.down].into_iter().enumerate() {
            lo[k] = lo[k].min(dot(q, a));
            hi[k] = hi[k].max(dot(q, a));
        }
    }
    let (ew, eh) = (hi[0] - lo[0], hi[1] - lo[1]);
    if !(ew > 0.0 && eh > 0.0) {
        return 1.0;
    }
    (ew / eh).clamp(0.25, 4.0)
}

/// Renders `s` from the iso view to straight RGBA8.
pub fn render(s: &Splats, width: u32, height: u32) -> Vec<u8> {
    render_view(s, &iso_view(s, width, height), width, height)
}

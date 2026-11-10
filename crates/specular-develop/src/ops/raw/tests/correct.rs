//! The corrections on the GPU against a CPU reference of the same steps, and the
//! acceptance checks on synthetic sensors: highlight hue and no magenta, raw CA residual,
//! hot pixels, flat and dark frames, preview against full size, no pipelines per value.

use super::*;
use crate::ops::correct::ca::{self, CaFit};
use crate::ops::correct::calib::{self, Calibration};
use crate::ops::correct::highlights::{
    self, BLOCK_CELLS, BLOCK_WORDS, BlockStats, TILE_CELLS, TILE_WORDS, TileStats,
};
use crate::ops::correct::{CaMode, Corrections, HighlightMode};

// ---- Synthetic sensors ----------------------------------------------------------------------

/// A sensor whose camera-RGB response (before white balance; 1 = white) at crop pixel
/// (x, y) is `f(x, y)`, clipped at white. No border, simple black.
fn make(
    layout: Layout,
    w: u32,
    h: u32,
    wb: [f32; 3],
    f: &dyn Fn(u32, u32) -> [f32; 3],
) -> SensorData {
    let (black, white) = (256.0f32, 16000.0f32);
    let mut samples = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = f(x, y)[usize::from(layout.color(x, y)).min(2)];
            let v = black + c * (white - black);
            samples.push(v.round().clamp(0.0, white) as u16);
        }
    }
    SensorData {
        width: w,
        height: h,
        cpp: 1,
        samples,
        float: None,
        crop: Rect { x: 0, y: 0, w, h },
        meta: SensorMeta {
            layout,
            black: BlackLevels::simple([black; 4], 1),
            white,
            clip_white: white,
            linearization: None,
            opcodes: Default::default(),
            opcodes_unknown: Vec::new(),
            wb,
            wb_source: WbSource::AsShot,
            rgb_cam: [[1.6, -0.4, -0.2], [-0.2, 1.4, -0.2], [0.0, -0.5, 1.5]],
            matrix_source: MatrixSource::Generic,
            dng: None,
            profile: None,
            orientation: 1,
            lens: None,
            default_scale: None,
            truncated: false,
        },
    }
}

const WB: [f32; 3] = [2.0, 1.0, 1.5];

fn rggb() -> Layout {
    Layout::Bayer([0, 1, 1, 2])
}

fn xtrans() -> Layout {
    Layout::Pattern(xtrans_cfa())
}

/// A smooth hash noise in [-0.5, 0.5).
fn hash(x: u32, y: u32) -> f32 {
    ((x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)) % 1000) as f32 / 1000.0 - 0.5
}

/// Textured background with three glowing blobs (orange, sky blue, white) whose centres
/// clip; values are white-balanced camera RGB (`scene`), divided by WB for the sensor.
fn blobs(w: u32, h: u32) -> impl Fn(u32, u32) -> [f32; 3] {
    let spots: [([f32; 2], f32, [f32; 3]); 3] = [
        ([0.25, 0.35], 0.11, [1.0, 0.55, 0.25]),
        ([0.7, 0.3], 0.13, [0.45, 0.65, 1.0]),
        ([0.5, 0.72], 0.12, [1.0, 1.0, 1.0]),
    ];
    move |x, y| {
        let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
        let t = 0.5 + 0.3 * ((fx * 9.0).sin() * (fy * 7.0).cos());
        let mut c = [0.12 * t, 0.14 * t, 0.10 * t];
        for (p, r, col) in spots {
            let d2 = ((fx - p[0]) * w as f32 / h as f32).powi(2) + (fy - p[1]).powi(2);
            let g = 3.2 * (-d2 / (r * r)).exp();
            for k in 0..3 {
                c[k] += g * col[k];
            }
        }
        std::array::from_fn(|k| c[k] / WB[k])
    }
}

// ---- CPU reference of the correction run ----------------------------------------------------

fn refl1(v: i32, n: i32) -> i32 {
    let mut r = v.abs();
    if r >= n {
        r = 2 * n - 2 - r;
    }
    r.clamp(0, n - 1)
}

struct Ref<'a> {
    s: &'a SensorData,
    w: i32,
    h: i32,
    bayer: bool,
}

impl Ref<'_> {
    fn col(&self, x: i32, y: i32) -> usize {
        usize::from(
            self.s
                .meta
                .layout
                .color(self.s.crop.x + x as u32, self.s.crop.y + y as u32),
        )
        .min(2)
    }
    fn pos(&self, x: i32, y: i32) -> (i32, i32) {
        if self.bayer {
            (refl1(x, self.w), refl1(y, self.h))
        } else {
            (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1))
        }
    }
    fn i(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }
}

/// The corrected mosaic per `o` on the CPU (the same steps and f32 arithmetic as the GPU).
fn cpu_correct(s: &SensorData, o: &Options, cal: Option<&Calibration>) -> Vec<f32> {
    let m = &s.meta;
    let c = o.corrections;
    let r = Ref {
        s,
        w: s.crop.w as i32,
        h: s.crop.h as i32,
        bayer: matches!(m.layout, Layout::Bayer(_)),
    };
    let (w, h) = (r.w, r.h);
    let n = (w * h) as usize;
    let wb = super::super::data::wb(m, o);
    let clip = m.clip_white.min(m.white);
    let cal = cal.filter(|_| c.calibrate);
    let norm = |x: i32, y: i32, l: f32| -> f32 {
        let (sx, sy) = (s.crop.x + x as u32, s.crop.y + y as u32);
        let b = m.black.at(sx, sy, 0);
        let mut v = (l - b) / (m.white - b).max(1.0);
        if let Some(cb) = cal {
            let wd = cb.words[r.i(x, y)];
            let g = half::f16::from_bits(wd as u16).to_f32();
            let d = half::f16::from_bits((wd >> 16) as u16).to_f32();
            v = (v - d) * g;
        }
        v * wb[r.col(x, y)]
    };
    // 1. Prepare.
    let mut a = vec![[0f32; 2]; n];
    for y in 0..h {
        for x in 0..w {
            let raw = f32::from(s.sample(s.crop.x + x as u32, s.crop.y + y as u32, 0));
            a[r.i(x, y)] = [norm(x, y, raw), norm(x, y, clip)];
        }
    }
    // 2. Hot pixels.
    let t = c.hot.unwrap_or(0.0);
    let mut b = a.clone();
    if t > 0.0 {
        for y in 0..h {
            for x in 0..w {
                let c0 = a[r.i(x, y)];
                let col = r.col(x, y);
                let mut vals = Vec::new();
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let (qx, qy) = r.pos(x + dx, y + dy);
                        if (qx == x && qy == y) || r.col(qx, qy) != col {
                            continue;
                        }
                        vals.push(a[r.i(qx, qy)][0]);
                    }
                }
                if vals.len() < 2 {
                    continue;
                }
                let lo = vals.iter().copied().fold(f32::MAX, f32::min);
                let hi = vals.iter().copied().fold(f32::MIN, f32::max);
                let v = c0[0];
                let fl = 0.01 * c0[1];
                let hot = v - hi > t * hi.max(fl);
                let dead = v * (1.0 + t) < lo && lo > fl;
                if hot || dead {
                    vals.sort_by(f32::total_cmp);
                    let k = vals.len();
                    let med = if k % 2 == 1 {
                        vals[k / 2]
                    } else {
                        0.5 * (vals[k / 2 - 1] + vals[k / 2])
                    };
                    b[r.i(x, y)] = [med, c0[1]];
                }
            }
        }
    }
    // Statistics.
    let ht = c.highlight_threshold;
    let cs: i32 = if r.bayer { 4 } else { 6 };
    let (nw, nh) = ((w + cs - 1) / cs, (h + cs - 1) / cs);
    let mut grid: Option<(Vec<[f32; 3]>, u32, u32)> = None;
    let mut rebuild: Option<highlights::Rebuild> = None;
    if matches!(
        c.highlights,
        HighlightMode::Opposed | HighlightMode::Rebuild
    ) {
        let f16 = |v: f32| half::f16::from_f32(v).to_f32();
        let mut cells = vec![[0f32; 4]; (nw * nh) as usize];
        for j in 0..nh {
            for i in 0..nw {
                let (mut sm, mut cn, mut fl) = ([0f32; 3], [0f32; 3], 0u32);
                for dy in 0..cs {
                    for dx in 0..cs {
                        let x = (i * cs + dx).min(w - 1);
                        let y = (j * cs + dy).min(h - 1);
                        // From the prepared samples (no hot-pixel fix), as `rcell`.
                        let [v, sat] = a[r.i(x, y)];
                        let k = r.col(x, y);
                        if v >= ht * sat {
                            fl |= 1 << k;
                        }
                        sm[k] += v;
                        cn[k] += 1.0;
                    }
                }
                let mm: [f32; 3] = std::array::from_fn(|k| f16(sm[k] / cn[k].max(1.0)));
                cells[(j * nw + i) as usize] = [mm[0], mm[1], mm[2], fl as f32];
            }
        }
        let cell = |i: i32, j: i32| cells[(j.clamp(0, nh - 1) * nw + i.clamp(0, nw - 1)) as usize];
        let tc = TILE_CELLS as i32;
        let (tw, th) = ((nw + tc - 1) / tc, (nh + tc - 1) / tc);
        let mut tw_words = vec![0f32; (tw * th) as usize * TILE_WORDS];
        for ty in 0..th {
            for tx in 0..tw {
                let (i0, j0) = (tx * tc, ty * tc);
                let (i1, j1) = ((i0 + tc).min(nw), (j0 + tc).min(nh));
                let any = (j0 - 2..j1 + 2).any(|j| {
                    (i0 - 2..i1 + 2)
                        .any(|i| j >= 0 && i >= 0 && j < nh && i < nw && cell(i, j)[3] != 0.0)
                });
                let (mut acc, mut ws, mut cand, mut nclip) = ([0f32; 3], 0f32, 0f32, 0f32);
                if any {
                    for j in j0..j1 {
                        for i in i0..i1 {
                            let cc = cell(i, j);
                            if cc[3] != 0.0 {
                                nclip += 1.0;
                                continue;
                            }
                            let sum = cc[0] + cc[1] + cc[2];
                            if sum <= 1e-6 || cc[0].min(cc[1]).min(cc[2]) < 0.0 {
                                continue;
                            }
                            let near = (-2..=2).any(|dj| {
                                (-2..=2).any(|di| {
                                    let (qi, qj) = (i + di, j + dj);
                                    qi >= 0
                                        && qj >= 0
                                        && qi < nw
                                        && qj < nh
                                        && cell(qi, qj)[3] != 0.0
                                })
                            });
                            if !near {
                                continue;
                            }
                            let wt = sum * sum;
                            for k in 0..3 {
                                acc[k] += wt * cc[k] / sum;
                            }
                            ws += wt;
                            cand += 1.0;
                        }
                    }
                }
                let o = (ty * tw + tx) as usize * TILE_WORDS;
                tw_words[o..o + 6].copy_from_slice(&[acc[0], acc[1], acc[2], ws, cand, nclip]);
            }
        }
        let g = highlights::chroma_grid(&TileStats {
            w: tw as u32,
            h: th as u32,
            words: tw_words,
        });
        if c.highlights == HighlightMode::Rebuild {
            let bc = BLOCK_CELLS as i32;
            let (bw, bh) = ((nw + bc - 1) / bc, (nh + bc - 1) / bc);
            let mut bw_words = vec![0f32; (bw * bh) as usize * BLOCK_WORDS];
            for by in 0..bh {
                for bx in 0..bw {
                    let (mut blown, mut unc, mut rgb, mut total) = (0f32, 0f32, [0f32; 3], 0f32);
                    for j in by * bc..(by * bc + bc).min(nh) {
                        for i in bx * bc..(bx * bc + bc).min(nw) {
                            let cc = cell(i, j);
                            total += 1.0;
                            if cc[3] == 7.0 {
                                blown += 1.0;
                            }
                            if cc[3] == 0.0 && cc[0].min(cc[1]).min(cc[2]) >= 0.0 {
                                unc += 1.0;
                                for k in 0..3 {
                                    rgb[k] += cc[k];
                                }
                            }
                        }
                    }
                    let o = (by * bw + bx) as usize * BLOCK_WORDS;
                    bw_words[o..o + 6]
                        .copy_from_slice(&[blown, unc, rgb[0], rgb[1], rgb[2], total]);
                }
            }
            let rb = highlights::rebuild(
                &BlockStats {
                    w: bw as u32,
                    h: bh as u32,
                    words: bw_words,
                },
                &g,
                tw as u32,
                th as u32,
            );
            if !rb.chroma.is_empty() {
                rebuild = Some(rb);
            }
        }
        grid = Some((g, tw as u32, th as u32));
    }
    // 3. Highlights.
    let mode = c.highlights;
    let chroma_at = |x: i32, y: i32| -> [f32; 3] {
        let Some((g, gw, gh)) = &grid else {
            return [1.0 / 3.0; 3];
        };
        let ts = (TILE_CELLS as i32 * cs) as f32;
        let fx = ((x as f32 + 0.5) / ts - 0.5).clamp(0.0, (*gw - 1) as f32);
        let fy = ((y as f32 + 0.5) / ts - 0.5).clamp(0.0, (*gh - 1) as f32);
        let (x0, y0) = (fx as u32, fy as u32);
        let (x1, y1) = ((x0 + 1).min(gw - 1), (y0 + 1).min(gh - 1));
        let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
        let at = |xx: u32, yy: u32| g[(yy * gw + xx) as usize];
        std::array::from_fn(|k| {
            let top = at(x0, y0)[k] + (at(x1, y0)[k] - at(x0, y0)[k]) * ax;
            let bot = at(x0, y1)[k] + (at(x1, y1)[k] - at(x0, y1)[k]) * ax;
            top + (bot - top) * ay
        })
    };
    let rebuild_at = |x: i32, y: i32| -> Option<([f32; 3], f32)> {
        let rb = rebuild.as_ref()?;
        let bs = (BLOCK_CELLS as i32 * cs) as u32;
        let (bw, bh) = (rb.w, rb.h);
        let nx = (x as u32 / bs).min(bw - 1);
        let ny = (y as u32 / bs).min(bh - 1);
        let id = rb.area[(ny * bw + nx) as usize];
        if id == 0 {
            return None;
        }
        let bsf = bs as f32;
        let fx = ((x as f32 + 0.5) / bsf - 0.5).clamp(0.0, (bw - 1) as f32);
        let fy = ((y as f32 + 0.5) / bsf - 0.5).clamp(0.0, (bh - 1) as f32);
        let (x0, y0) = (fx as u32, fy as u32);
        let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
        let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
        let b = |xx: u32, yy: u32| rb.boost[(yy * bw + xx) as usize];
        let top = b(x0, y0) + (b(x1, y0) - b(x0, y0)) * ax;
        let bot = b(x0, y1) + (b(x1, y1) - b(x0, y1)) * ax;
        Some((rb.chroma[id as usize - 1], top + (bot - top) * ay))
    };
    let mut mo = vec![0f32; n];
    for y in 0..h {
        for x in 0..w {
            let c0 = b[r.i(x, y)];
            let v = c0[0];
            let mut minsat = f32::MAX;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (qx, qy) = r.pos(x + dx, y + dy);
                    minsat = minsat.min(b[r.i(qx, qy)][1]);
                }
            }
            mo[r.i(x, y)] = if mode == HighlightMode::Clip {
                v.min(ht * minsat)
            } else if mode == HighlightMode::Off || v < ht * c0[1] {
                v
            } else {
                let (mut um, mut un, mut cm) = ([0f32; 3], [0f32; 3], [0f32; 3]);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (qx, qy) = r.pos(x + dx, y + dy);
                        let sp = b[r.i(qx, qy)];
                        let k = r.col(qx, qy);
                        if sp[0] >= ht * sp[1] {
                            cm[k] = cm[k].max(sp[0]);
                        } else {
                            um[k] += sp[0];
                            un[k] += 1.0;
                        }
                    }
                }
                {
                    let mut chi = chroma_at(x, y);
                    let mut boost = 0.0;
                    if mode == HighlightMode::Rebuild
                        && let Some((cc, bo)) = rebuild_at(x, y)
                    {
                        chi = cc;
                        boost = bo;
                    }
                    let chi = chi.map(|v| v.max(0.02));
                    let (mut su, mut sc) = (0f32, 0f32);
                    for k in 0..3 {
                        if un[k] > 0.0 && cm[k] == 0.0 {
                            su += (um[k] / un[k]).max(0.0);
                            sc += chi[k];
                        }
                    }
                    let mut sv = if sc > 0.0 { su / sc } else { 0.0 };
                    for k in 0..3 {
                        if cm[k] > 0.0 {
                            sv = sv.max(cm[k] / chi[k]);
                        }
                    }
                    sv *= 1.0 + boost;
                    v.max((sv * chi[r.col(x, y)]).min(4.0 * c0[1]))
                }
            };
        }
    }
    // 4. Raw CA.
    let fit: Option<(ca::Radial, ca::Radial)> = match c.ca {
        CaMode::Off => None,
        CaMode::Auto => ca::estimate_mosaic(s).map(|f| (f.red, f.blue)),
        CaMode::Manual { red, blue } => Some((
            ca::Radial {
                a: red / 100.0,
                b: 0.0,
            },
            ca::Radial {
                a: blue / 100.0,
                b: 0.0,
            },
        )),
    };
    let Some((fr, fb)) = fit else { return mo };
    let at = |x: i32, y: i32| mo[r.i(refl1(x, w), refl1(y, h))];
    let (fw, fh) = (w as f32, h as f32);
    let (cx, cy, rr) = (
        (fw - 1.0) / 2.0,
        (fh - 1.0) / 2.0,
        (fw * fw + fh * fh).sqrt() / 2.0,
    );
    let cr = |t: f32| {
        let (t2, t3) = (t * t, t * t * t);
        [
            -0.5 * t3 + t2 - 0.5 * t,
            1.5 * t3 - 2.5 * t2 + 1.0,
            -1.5 * t3 + 2.0 * t2 + 0.5 * t,
            0.5 * t3 - 0.5 * t2,
        ]
    };
    let mut out = mo.clone();
    for y in 0..h {
        for x in 0..w {
            let col = r.col(x, y);
            if col == 1 {
                continue;
            }
            let f = if col == 0 { fr } else { fb };
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            let k = f.a + f.b * (dx * dx + dy * dy) / (rr * rr).max(1e-6);
            let d = [(dx * k).clamp(-6.0, 6.0), (dy * k).clamp(-6.0, 6.0)];
            if !r.bayer {
                let colr = |sx: i32, sy: i32| r.col(refl1(sx, w), refl1(sy, h));
                let tent = |qx: f32, qy: f32, c: usize, f: &dyn Fn(i32, i32) -> f32| {
                    let (ix, iy) = (qx.floor() as i32, qy.floor() as i32);
                    let (mut sum, mut n) = (0f32, 0f32);
                    for j in -1..=2 {
                        let sy = iy + j;
                        let wy = (1.0 - (sy as f32 - qy).abs() * 0.5).max(0.0);
                        for i in -1..=2 {
                            let sx = ix + i;
                            if colr(sx, sy) != c {
                                continue;
                            }
                            let wt = wy * (1.0 - (sx as f32 - qx).abs() * 0.5).max(0.0);
                            sum += wt * f(sx, sy);
                            n += wt;
                        }
                    }
                    sum / n.max(1e-6)
                };
                // The green plane (green's tent at every pixel), bilinear between pixels.
                let plane =
                    |px: i32, py: i32| tent(refl1(px, w) as f32, refl1(py, h) as f32, 1, &at);
                let green = |qx: f32, qy: f32| {
                    let (ix, iy) = (qx.floor() as i32, qy.floor() as i32);
                    let (fx, fy) = (qx - ix as f32, qy - iy as f32);
                    let mix = |a: f32, b: f32, t: f32| a * (1.0 - t) + b * t;
                    let a = mix(plane(ix, iy), plane(ix + 1, iy), fx);
                    let b = mix(plane(ix, iy + 1), plane(ix + 1, iy + 1), fx);
                    mix(a, b, fy)
                };
                const EPS: f32 = 0.002;
                let ratio = |sx: i32, sy: i32| {
                    let g = green(sx as f32 - d[0], sy as f32 - d[1]);
                    (at(sx, sy) + EPS) / (g + EPS)
                };
                let (qx, qy) = (x as f32 + d[0], y as f32 + d[1]);
                let mut o = (plane(x, y) + EPS) * tent(qx, qy, col, &ratio) - EPS;
                let (rx, ry) = (qx.floor() as i32, qy.floor() as i32);
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for j in -1..=2 {
                    for i in -1..=2 {
                        if colr(rx + i, ry + j) == col {
                            lo = lo.min(at(rx + i, ry + j));
                            hi = hi.max(at(rx + i, ry + j));
                        }
                    }
                }
                if lo <= hi {
                    o = o.clamp(lo, hi);
                }
                out[r.i(x, y)] = o;
                continue;
            }
            let (px, py) = (x & 1, y & 1);
            let u = (x as f32 + d[0] - px as f32) * 0.5;
            let vv = (y as f32 + d[1] - py as f32) * 0.5;
            let (iu, iw) = (u.floor() as i32, vv.floor() as i32);
            let (wx, wy) = (cr(u - iu as f32), cr(vv - iw as f32));
            let (mut acc, mut lo, mut hi) = (0f32, f32::MAX, f32::MIN);
            for j in 0..4 {
                let mut row = 0f32;
                for i in 0..4 {
                    let sv = at(2 * (iu + i - 1) + px, 2 * (iw + j - 1) + py);
                    row += wx[i as usize] * sv;
                    if (1..=2).contains(&i) && (1..=2).contains(&j) {
                        lo = lo.min(sv);
                        hi = hi.max(sv);
                    }
                }
                acc += wy[j as usize] * row;
            }
            out[r.i(x, y)] = acc.clamp(lo, hi);
        }
    }
    out
}

// ---- Helpers --------------------------------------------------------------------------------

fn corr(hl: HighlightMode, hot: Option<f32>, ca: CaMode, cal: bool) -> Corrections {
    Corrections {
        hot,
        highlights: hl,
        highlight_threshold: 1.0,
        ca,
        calibrate: cal,
    }
}

fn mosaic_opts(c: Corrections) -> Options {
    Options {
        precision: Precision::F32,
        output: Output::Mosaic,
        corrections: c,
        ..Options::new(Quality::Full)
    }
}

/// The GPU's corrected mosaic (red channel of the Mosaic output).
fn gpu_mosaic(
    fe: &mut FrontEnd,
    s: &SensorData,
    c: Corrections,
    cal: Option<&Calibration>,
) -> Vec<f32> {
    let mut g = fe.upload(s.clone()).unwrap();
    fe.set_calibration(&mut g, cal).unwrap();
    let out = fe.run(&g, &mosaic_opts(c)).unwrap();
    fe.read(&out)
        .unwrap()
        .chunks_exact(4)
        .map(|p| p[0])
        .collect()
}

fn worst_rel(a: &[f32], b: &[f32]) -> (f32, usize) {
    let mut worst = 0f32;
    let mut at = 0;
    for (i, (p, q)) in a.iter().zip(b).enumerate() {
        let d = (p - q).abs() / q.abs().max(1.0);
        assert!(d.is_finite(), "non-finite at {i}: {p} vs {q}");
        if d > worst {
            worst = d;
            at = i;
        }
    }
    (worst, at)
}

/// OkLab hue (degrees) and chroma of a Rec.2020 colour.
fn hue_chroma(c: [f32; 3]) -> (f32, f32) {
    let lab = crate::colour::oklab::from_rec2020_f32(c);
    (lab[2].atan2(lab[1]).to_degrees(), lab[1].hypot(lab[2]))
}

fn hue_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    if d > 180.0 { 360.0 - d } else { d }
}

// ---- Tests ----------------------------------------------------------------------------------

#[test]
fn corrections_on_the_gpu_match_the_cpu_reference() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (384u32, 256u32);
    let f = blobs(w, h);
    let with_hot = |x: u32, y: u32| {
        let mut c = f(x, y);
        if (x * 7 + y * 13).is_multiple_of(997) {
            c = c.map(|v| v * 4.0 + 0.3);
        }
        c
    };
    for (name, layout) in [("bayer", rggb()), ("x-trans", xtrans())] {
        let s = make(layout, w, h, WB, &with_hot);
        // A flat and a dark of the same sensor.
        let flat = make(s.meta.layout.clone(), w, h, WB, &|x, y| {
            let r2 =
                ((x as f32 / w as f32 - 0.5).powi(2) + (y as f32 / h as f32 - 0.5).powi(2)) * 4.0;
            [0.5 * (1.0 - 0.3 * r2); 3]
        });
        let dark = make(s.meta.layout.clone(), w, h, WB, &|x, y| {
            [0.002 + 0.002 * (hash(x, y) + 0.5); 3]
        });
        let cal = calib::build(&s, Some(&flat), Some(&dark), false).unwrap();
        for c in [
            corr(HighlightMode::Opposed, Some(1.0), CaMode::Off, false),
            corr(
                HighlightMode::Rebuild,
                Some(0.5),
                CaMode::Manual {
                    red: 0.2,
                    blue: -0.15,
                },
                true,
            ),
            corr(HighlightMode::Clip, None, CaMode::Auto, false),
            corr(HighlightMode::Off, Some(1.0), CaMode::Off, true),
        ] {
            let gpu = gpu_mosaic(&mut fe, &s, c, Some(&cal));
            let cpu = cpu_correct(&s, &mosaic_opts(c), Some(&cal));
            let (worst, at) = worst_rel(&gpu, &cpu);
            eprintln!("{name} {c:?}: worst {worst:.2e} at {at}");
            assert!(
                worst < 5e-4,
                "{name} {c:?}: {worst} at {at}: {} vs {}",
                gpu[at],
                cpu[at]
            );
        }
    }
}

#[test]
fn bands_equal_a_whole_run_with_corrections() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (256u32, 200u32);
    let s = make(rggb(), w, h, WB, &blobs(w, h));
    let c = corr(
        HighlightMode::Rebuild,
        Some(1.0),
        CaMode::Manual {
            red: 0.3,
            blue: -0.3,
        },
        false,
    );
    let g = fe.upload(s).unwrap();
    let mut o = Options {
        precision: Precision::F32,
        corrections: c,
        ..Options::new(Quality::Full)
    };
    let whole = {
        let r = fe.run(&g, &o).unwrap();
        fe.read(&r).unwrap()
    };
    o.band_rows = 37;
    let banded = fe.run(&g, &o).unwrap();
    assert!(banded.stats.bands > 1);
    assert_eq!(fe.read(&banded).unwrap(), whole);
}

/// Hue of reconstructed highlights against the unclipped truth, and no magenta.
#[test]
fn highlights_keep_hue_and_never_turn_magenta() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (512u32, 384u32);
    let f = blobs(w, h);
    for (name, layout) in [("bayer", rggb()), ("x-trans", xtrans())] {
        let s = make(layout, w, h, WB, &f);
        let mat = super::super::data::output_matrix(&s.meta);
        let g = fe.upload(s.clone()).unwrap();
        for mode in [
            HighlightMode::Off,
            HighlightMode::Opposed,
            HighlightMode::Rebuild,
            HighlightMode::Clip,
        ] {
            let o = Options {
                precision: Precision::F32,
                corrections: corr(mode, None, CaMode::Off, false),
                ..Options::new(Quality::Full)
            };
            let out = {
                let r = fe.run(&g, &o).unwrap();
                fe.read(&r).unwrap()
            };
            let (mut errs, mut white_ab, mut n_white) = (Vec::new(), [0f64; 2], 0usize);
            for y in 2..h - 2 {
                for x in 2..w - 2 {
                    let cam = f(x, y);
                    if cam.iter().all(|&v| v < 0.98) {
                        continue;
                    }
                    let truth = mat3(&mat, std::array::from_fn(|k| cam[k] * WB[k]));
                    let i = ((y * w + x) * 4) as usize;
                    let got = [out[i], out[i + 1], out[i + 2]];
                    let (ht, ct) = hue_chroma(truth);
                    let (hg, _) = hue_chroma(got);
                    if ct > 0.03 {
                        errs.push(hue_diff(ht, hg));
                    } else {
                        // The white blob: a* b* of the result.
                        let lab = crate::colour::oklab::from_rec2020_f32(got);
                        let l = f64::from(lab[0]).max(1e-6);
                        white_ab[0] += f64::from(lab[1]) / l;
                        white_ab[1] += f64::from(lab[2]) / l;
                        n_white += 1;
                    }
                }
            }
            errs.sort_by(f32::total_cmp);
            let mean = errs.iter().sum::<f32>() / errs.len().max(1) as f32;
            let p95 = errs[errs.len() * 95 / 100];
            let ab = white_ab.map(|v| v / n_white.max(1) as f64);
            // Magenta: a* > 0 and b* < 0 together, beyond a small tolerance.
            let magenta = ab[0] > 0.01 && ab[1] < -0.01;
            eprintln!(
                "{name} {mode:?}: hue error mean {mean:.2}° p95 {p95:.2}° over {} px; white a/L {:+.4} b/L {:+.4} ({n_white} px)",
                errs.len(),
                ab[0],
                ab[1]
            );
            match mode {
                HighlightMode::Off => assert!(magenta, "{name}: clipping should show magenta here"),
                HighlightMode::Opposed | HighlightMode::Rebuild => {
                    assert!(mean < 3.0, "{name} {mode:?}: mean hue error {mean}");
                    assert!(!magenta, "{name} {mode:?}: magenta {ab:?}");
                    assert!(
                        ab[0].abs() < 0.01 && ab[1].abs() < 0.01,
                        "{name} {mode:?}: {ab:?}"
                    );
                }
                HighlightMode::Clip => {
                    assert!(!magenta, "{name} clip: magenta {ab:?}");
                    assert!(
                        ab[0].abs() < 0.01 && ab[1].abs() < 0.01,
                        "{name} clip: {ab:?}"
                    );
                }
            }
        }
    }
}

fn mat3(m: &[[f32; 3]; 3], c: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2])
}

/// A chart of discs and slanted squares for the CA tests (camera RGB, before shifting).
fn chart(x: f32, y: f32) -> f32 {
    let (u, v) = (x / 23.0, y / 23.0);
    let (fu, fv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
    let r = (fu * fu + fv * fv).sqrt();
    // Soft edges (about one pixel) so the sampling is fair.
    let disc = 1.0 / (1.0 + ((r - 0.3) * 23.0 / 0.7).exp());
    0.08 + 0.6 * disc
}

/// R and B magnified by `kr`, `kb` about the centre (lateral CA).
fn shifted(w: u32, h: u32, kr: f32, kb: f32) -> impl Fn(u32, u32) -> [f32; 3] {
    let (cx, cy) = ((w as f32 - 1.0) / 2.0, (h as f32 - 1.0) / 2.0);
    move |x, y| {
        let (dx, dy) = (x as f32 - cx, y as f32 - cy);
        let at = |k: f32| chart(cx + dx / (1.0 + k), cy + dy / (1.0 + k));
        [at(kr) * 0.8 / WB[0], at(0.0) / WB[1], at(kb) * 0.9 / WB[2]]
    }
}

/// The corner shifts (red, blue; pixels) before and after automatic raw CA on a synthetic
/// sensor of `layout` whose red and blue are magnified against green.
fn raw_ca_residual(layout: Layout, limit: f32) {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (1536u32, 1024u32);
    let (kr, kb) = (0.0016f32, -0.0012f32);
    let s = make(layout.clone(), w, h, WB, &shifted(w, h, kr, kb));
    let radius = ((w * w + h * h) as f32).sqrt() / 2.0;
    // The estimate itself.
    let t0 = std::time::Instant::now();
    let fit: CaFit = ca::estimate_mosaic(&s).expect("a fit");
    eprintln!("estimate {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    let corner = |m: ca::Radial| radius * (m.a + m.b);
    eprintln!(
        "fit: red {:?} ({:.3} px at the corner, truth {:.3}), blue {:?} ({:.3}, truth {:.3}), tiles {:?}",
        fit.red,
        corner(fit.red),
        radius * kr,
        fit.blue,
        corner(fit.blue),
        radius * kb,
        fit.tiles
    );
    let g = fe.upload(s).unwrap();
    let mut residual = |c: CaMode| {
        let o = Options {
            precision: Precision::F32,
            output: Output::CameraRgb,
            corrections: corr(HighlightMode::Off, None, c, false),
            ..Options::new(Quality::Full)
        };
        let out = {
            let r = fe.run(&g, &o).unwrap();
            fe.read(&r).unwrap()
        };
        let k = ca::estimate_rgb(w, h, &out).expect("a measurement");
        [k[0] * radius, k[1] * radius]
    };
    let before = residual(CaMode::Off);
    let after = residual(CaMode::Auto);
    // What the demosaic itself reads as a shift (a sensor without CA).
    let clean = make(layout, w, h, WB, &shifted(w, h, 0.0, 0.0));
    let g = fe.upload(clean).unwrap();
    let o = Options {
        precision: Precision::F32,
        output: Output::CameraRgb,
        corrections: corr(HighlightMode::Off, None, CaMode::Off, false),
        ..Options::new(Quality::Full)
    };
    let r = fe.run(&g, &o).unwrap();
    let out = fe.read(&r).unwrap();
    let k = ca::estimate_rgb(w, h, &out).expect("a measurement");
    let base = [k[0] * radius, k[1] * radius];
    let after = [after[0] - base[0], after[1] - base[1]];
    eprintln!("corner shift before {before:?} px, after {after:?} px (less {base:?} without CA)");
    assert!(before[0].abs() > 1.0 && before[1].abs() > 0.8, "{before:?}");
    assert!(
        after[0].abs() < limit && after[1].abs() < limit,
        "residual {after:?}"
    );
}

#[test]
fn raw_ca_leaves_under_a_tenth_of_a_pixel() {
    raw_ca_residual(rggb(), 0.1);
}

/// The X-Trans estimate interpolates only the lattice points its tiles read, from fewer
/// tiles: the same fit as from the whole lattice with those tiles, within a few hundredths
/// of a pixel of the truth, in less time than before (whole lattice, 900 tiles).
#[test]
fn x_trans_estimate_on_the_decimated_lattice_equals_the_whole() {
    let (w, h) = (6000u32, 4000u32);
    let (kr, kb) = (0.0016f32, -0.0012f32);
    let s = make(xtrans(), w, h, WB, &shifted(w, h, kr, kb));
    let radius = ((w * w + h * h) as f32).sqrt() / 2.0;
    let time = |decimate: bool, tiles: usize| {
        let mut best = f64::MAX;
        let mut fit = None;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            fit = ca::estimate_pattern_on(&s, decimate, tiles);
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        (fit.expect("a fit"), best)
    };
    let (before, tb) = time(false, ca::MAX_TILES);
    let (whole, _) = time(false, ca::PATTERN_TILES);
    let (dec, td) = time(true, ca::PATTERN_TILES);
    let corner = |f: &CaFit| [radius * (f.red.a + f.red.b), radius * (f.blue.a + f.blue.b)];
    let truth = [radius * kr, radius * kb];
    eprintln!(
        "x-trans {w}x{h} estimate: before {tb:.0} ms (corner {:?}), now {td:.0} ms (corner {:?}), truth {truth:?}",
        corner(&before),
        corner(&dec)
    );
    assert_eq!(whole, dec);
    for (c, t) in corner(&dec).iter().zip(truth) {
        assert!((c - t).abs() < 0.05, "{:?}", corner(&dec));
    }
}

#[test]
fn raw_ca_on_x_trans_leaves_under_a_tenth_of_a_pixel() {
    raw_ca_residual(xtrans(), 0.1);
}

/// `s` with its samples re-encoded through a strong curve (`v^(1/2.2)` between black and
/// white) and the inverse as a LinearizationTable (`list2` false) or as an opcode list 2
/// MapTable (`list2` true), so the decoded linear values are unchanged.
fn encoded(s: &SensorData, list2: bool) -> SensorData {
    let mut e = s.clone();
    let (black, white) = (s.meta.black.at(0, 0, 0), s.meta.white);
    let g = 1.0 / 2.2f32;
    if list2 {
        // Normalised values encoded, the map decodes them after black and white.
        for v in &mut e.samples {
            let n = ((f32::from(*v) - black) / (white - black)).clamp(0.0, 1.0);
            *v = (black + n.powf(g) * (white - black)).round() as u16;
        }
        let t: Vec<u16> = (0..=65535u32)
            .map(|i| ((i as f32 / 65535.0).powf(2.2) * 65535.0).round() as u16)
            .collect();
        let full = Rect {
            x: 0,
            y: 0,
            w: s.width,
            h: s.height,
        };
        e.meta.opcodes[1].push(OpcodeInfo::Point {
            rect: full,
            plane: 0,
            planes: 1,
            row_pitch: 1,
            col_pitch: 1,
            bounds: s.crop,
            kind: PointKind::MapTable(t),
        });
    } else {
        // Stored values encoded over the full 16 bits; the table gives the linear value.
        for v in &mut e.samples {
            *v = ((f32::from(*v) / white).clamp(0.0, 1.0).powf(g) * 65535.0).round() as u16;
        }
        e.meta.linearization = Some(
            (0..=65535u32)
                .map(|i| ((i as f32 / 65535.0).powf(2.2) * white).round() as u16)
                .collect(),
        );
        e.meta.white = white;
        e.meta.clip_white = white;
    }
    e
}

#[test]
fn raw_ca_measures_through_linearisation_tables() {
    let (w, h) = (1536u32, 1024u32);
    let (kr, kb) = (0.0016f32, -0.0012f32);
    let radius = ((w * w + h * h) as f32).sqrt() / 2.0;
    let corner = |f: &CaFit| [radius * (f.red.a + f.red.b), radius * (f.blue.a + f.blue.b)];
    for (name, layout) in [("bayer", rggb()), ("x-trans", xtrans())] {
        let plain = make(layout, w, h, WB, &shifted(w, h, kr, kb));
        let reference = corner(&ca::estimate_mosaic(&plain).expect("a fit"));
        for list2 in [false, true] {
            let e = encoded(&plain, list2);
            let with = corner(&ca::estimate_mosaic(&e).expect("a fit"));
            // What the estimate saw before: the encoded samples taken as linear.
            let mut ignored = e.clone();
            ignored.meta.linearization = None;
            ignored.meta.opcodes = Default::default();
            if !list2 {
                ignored.meta.white = 65535.0;
                ignored.meta.clip_white = 65535.0;
            }
            let without = corner(&ca::estimate_mosaic(&ignored).expect("a fit"));
            let err = |c: [f32; 2]| (c[0] - reference[0]).abs().max((c[1] - reference[1]).abs());
            eprintln!(
                "{name} list2 {list2}: linear {reference:?}, through tables {with:?} (off by {:.4}), tables ignored {without:?} (off by {:.4})",
                err(with),
                err(without)
            );
            assert!(err(with) < 0.02, "{with:?} vs {reference:?}");
        }
    }
}

#[test]
fn preview_flags_pictures_made_before_the_highlight_statistics() {
    use crate::model::{Edit, Registry, SourceKey};
    use crate::preview::{Developer, Input, Orient};
    let Some(d) = dev() else { return };
    let _l = lock();
    let (w, h) = (768u32, 512u32);
    let s = make(rggb(), w, h, WB, &blobs(w, h));
    let mut dv = Developer::new(
        &d.device,
        &d.queue,
        crate::gpu::pool::Budget::fixed(1 << 30),
    );
    dv.set_input(Input::Raw(Box::new(s))).unwrap();
    // The first picture is made before its statistics come back, however fast the device:
    // the test waits on them below, not on which arrives first.
    dv.front_mut().stats_late = true;
    let e = Edit::default_for(Registry::builtin(), SourceKey::default(), true);
    let picture = |dv: &mut Developer| {
        dv.prepare(&e, 1024).unwrap();
        let r = dv
            .render(&e, None, &crate::ops::output::OutputSpace::rec709())
            .unwrap();
        let (ow, _, px) = dv.read_rgba8(r, Orient::default()).unwrap();
        // The orange blob's clipped centre.
        let (x, y) = ((0.25 * w as f32) as u32, (0.35 * h as f32) as u32);
        let i = ((y * ow + x) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    };
    let first = picture(&mut dv);
    assert!(dv.provisional(), "the first picture has no statistics yet");
    let t0 = std::time::Instant::now();
    while !dv.refine_due() {
        assert!(t0.elapsed().as_secs() < 30, "statistics never arrived");
        let _ = dv.wait();
    }
    let refined = picture(&mut dv);
    eprintln!(
        "clipped centre: provisional {first:?}, refined {refined:?} after {:.0} ms",
        t0.elapsed().as_secs_f64() * 1e3
    );
    assert!(!dv.provisional());
    // The held-back picture really was different (its clipped colour unrefined).
    let off = (0..3)
        .map(|c| (i32::from(first[c]) - i32::from(refined[c])).abs())
        .max()
        .unwrap_or(0);
    assert!(off > 8, "{first:?} vs {refined:?}");
}

/// X-Trans raw CA on the mosaic itself: with the true shifts, the corrected red and blue
/// sites against the scene without CA.
#[test]
fn raw_ca_on_x_trans_restores_the_sites() {
    let (w, h) = (768u32, 512u32);
    let (kr, kb) = (0.004f32, -0.003f32);
    let s = make(xtrans(), w, h, WB, &shifted(w, h, kr, kb));
    let c = corr(
        HighlightMode::Off,
        None,
        CaMode::Manual {
            red: kr * 100.0,
            blue: kb * 100.0,
        },
        false,
    );
    let none = cpu_correct(
        &s,
        &mosaic_opts(corr(HighlightMode::Off, None, CaMode::Off, false)),
        None,
    );
    let fixed = cpu_correct(&s, &mosaic_opts(c), None);
    let gain = [0.8f32, 1.0, 0.9];
    let mut e = [[0f64; 2]; 3];
    let mut n = [0f64; 3];
    for y in 16..h - 16 {
        for x in 16..w - 16 {
            let col = usize::from(s.meta.layout.color(x, y));
            if col == 1 {
                continue;
            }
            let ideal = chart(x as f32, y as f32) * gain[col];
            let i = (y * w + x) as usize;
            e[col][0] += f64::from((none[i] - ideal).powi(2));
            e[col][1] += f64::from((fixed[i] - ideal).powi(2));
            n[col] += 1.0;
        }
    }
    for col in [0, 2] {
        let (before, after) = ((e[col][0] / n[col]).sqrt(), (e[col][1] / n[col]).sqrt());
        eprintln!("x-trans sites, colour {col}: rms {before:.5} before, {after:.5} after");
        assert!(after < 0.3 * before, "{col}: {before} → {after}");
    }
}

/// The X-Trans CA pass's packed colour table gives the lookup through `ccolor` bit for
/// bit, cropped at every phase of the 6×6 pattern.
#[test]
fn x_trans_colour_table_equals_the_lookup() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (390u32, 262u32);
    let c = corr(
        HighlightMode::Off,
        None,
        CaMode::Manual {
            red: 0.4,
            blue: -0.3,
        },
        false,
    );
    for (cx, cy) in [(0u32, 0u32), (1, 4), (5, 3), (2, 1)] {
        let mut s = make(xtrans(), w, h, WB, &shifted(w, h, 0.004, -0.003));
        s.crop = Rect {
            x: cx,
            y: cy,
            w: w - 6,
            h: h - 6,
        };
        fe.slow_xtrans = false;
        let fast = gpu_mosaic(&mut fe, &s, c, None);
        fe.slow_xtrans = true;
        let slow = gpu_mosaic(&mut fe, &s, c, None);
        fe.slow_xtrans = false;
        let differ = fast
            .iter()
            .zip(&slow)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert_eq!(differ, 0, "crop at ({cx}, {cy})");
    }
}

/// DNG `BaselineExposure` brightens every sensor's output (not only float merges), and the
/// CPU reference agrees.
#[test]
fn baseline_exposure_applies_to_every_sensor() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (96u32, 64u32);
    let f = |x: u32, y: u32| [0.05 + 0.002 * x as f32, 0.08, 0.04 + 0.003 * y as f32];
    let plain = make(rggb(), w, h, WB, &f);
    let mut lifted = plain.clone();
    lifted.meta.dng = Some(specular_raw::sensor::DngColour {
        baseline_exposure: 1.0,
        ..Default::default()
    });
    let o = Options {
        precision: Precision::F32,
        ..Options::new(Quality::Full)
    };
    let mean = |fe: &mut FrontEnd, s: &SensorData| {
        let g = fe.upload(s.clone()).unwrap();
        let r = fe.run(&g, &o).unwrap();
        let px = fe.read(&r).unwrap();
        let gpu: f64 = px.chunks_exact(4).map(|p| f64::from(p[1])).sum::<f64>();
        let (_, _, cpu) = super::super::cpu_ref::develop(s, &o);
        let cpu: f64 = cpu.chunks_exact(4).map(|p| f64::from(p[1])).sum::<f64>();
        (gpu, cpu)
    };
    let (g0, c0) = mean(&mut fe, &plain);
    let (g1, c1) = mean(&mut fe, &lifted);
    eprintln!("baseline +1 EV: GPU ×{:.4}, CPU ×{:.4}", g1 / g0, c1 / c0);
    assert!((g1 / g0 - 2.0).abs() < 1e-3 && (c1 / c0 - 2.0).abs() < 1e-3);
}

#[test]
fn hot_pixels_fixed_and_clean_texture_kept() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (512u32, 384u32);
    let tex = |x: u32, y: u32| -> [f32; 3] {
        let (fx, fy) = (x as f32, y as f32);
        let t = 0.25
            + 0.12 * (fx * 0.11).sin() * (fy * 0.07).cos()
            + 0.04 * (fx * 0.31 + fy * 0.17).sin();
        let n = 0.004 * hash(x, y);
        [
            (t * 0.9 + n) / WB[0],
            (t + n) / WB[1],
            (t * 0.7 + n) / WB[2],
        ]
    };
    let mut rng = 12345u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut injected = std::collections::HashMap::new();
    while injected.len() < 400 {
        let (x, y) = (
            2 + (next() % u64::from(w - 4)) as u32,
            2 + (next() % u64::from(h - 4)) as u32,
        );
        // Singles: no other injected pixel within 2.
        if injected
            .keys()
            .any(|&(a, b): &(u32, u32)| a.abs_diff(x) <= 2 && b.abs_diff(y) <= 2)
        {
            continue;
        }
        let hot = next() % 2 == 0;
        injected.insert((x, y), hot);
    }
    let clean = make(rggb(), w, h, WB, &tex);
    let broken = make(rggb(), w, h, WB, &|x, y| match injected.get(&(x, y)) {
        Some(true) => [0.95; 3],
        Some(false) => [0.0; 3],
        None => tex(x, y),
    });
    let c = corr(
        HighlightMode::Off,
        Some(crate::ops::correct::HOT_DEFAULT),
        CaMode::Off,
        false,
    );
    let truth = gpu_mosaic(
        &mut fe,
        &clean,
        corr(HighlightMode::Off, None, CaMode::Off, false),
        None,
    );
    let fixed = gpu_mosaic(&mut fe, &broken, c, None);
    let mut ok = 0;
    for (&(x, y), hot) in &injected {
        let i = (y * w + x) as usize;
        // Fixed: back near the clean value (the neighbours' median on this texture is within
        // about 11 %; the injected values were 3 to 8 times off).
        if (fixed[i] - truth[i]).abs() < 0.2 * truth[i].max(0.05) {
            ok += 1;
        } else {
            eprintln!(
                "not fixed: ({x}, {y}) hot {hot}: {} vs {}",
                fixed[i], truth[i]
            );
        }
    }
    // Clean texture through the fix.
    let kept = gpu_mosaic(&mut fe, &clean, c, None);
    let changed = kept
        .iter()
        .zip(&truth)
        .filter(|(a, b)| (*a - *b).abs() > 1e-6)
        .count();
    let worst = kept
        .iter()
        .zip(&truth)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    eprintln!(
        "hot pixels: {ok}/{} fixed; clean texture: {changed} changed, worst {worst:.2e}",
        injected.len()
    );
    assert_eq!(ok, injected.len());
    assert_eq!(changed, 0);
}

#[test]
fn flat_and_dark_frames_restore_within_one_percent() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (480u32, 320u32);
    let vig = |x: u32, y: u32| {
        let r2 = ((x as f32 - w as f32 / 2.0).powi(2) + (y as f32 - h as f32 / 2.0).powi(2))
            / ((w * w + h * h) as f32 / 4.0);
        1.0 - 0.45 * r2
    };
    let dust = |x: u32, y: u32| {
        let d2 = (x as f32 - 300.0).powi(2) + (y as f32 - 100.0).powi(2);
        1.0 - 0.3 * (-d2 / 50.0).exp()
    };
    let darkv = |x: u32, y: u32| {
        0.004
            + 0.003 * (hash(x, y) + 0.5)
            + if (x + 3 * y).is_multiple_of(101) {
                0.05
            } else {
                0.0
            }
    };
    let scene = |x: u32, y: u32| -> [f32; 3] {
        let t = 0.3 + 0.1 * ((x as f32) * 0.05).sin() * ((y as f32) * 0.04).cos();
        [t * 0.8, t, t * 0.6]
    };
    let shot = make(rggb(), w, h, WB, &|x, y| {
        let g = vig(x, y) * dust(x, y);
        scene(x, y).map(|v| v * g + darkv(x, y))
    });
    let flat = make(rggb(), w, h, WB, &|x, y| [0.6 * vig(x, y) * dust(x, y); 3]);
    let dark = make(rggb(), w, h, WB, &|x, y| [darkv(x, y); 3]);
    let truth = make(rggb(), w, h, WB, &scene);
    let none = corr(HighlightMode::Off, None, CaMode::Off, false);
    let want = gpu_mosaic(&mut fe, &truth, none, None);
    let check = |got: &[f32], skip_dust: bool| -> (f32, f32) {
        // The flat is normalised to its centre: the restored picture is the truth times the
        // centre's falloff.
        let mut cs = (0f64, 0f64);
        for y in h * 3 / 8..h * 5 / 8 {
            for x in w * 3 / 8..w * 5 / 8 {
                let i = (y * w + x) as usize;
                cs.0 += f64::from(got[i]);
                cs.1 += f64::from(want[i]);
            }
        }
        let k = (cs.0 / cs.1) as f32;
        let mut errs: Vec<f32> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if skip_dust && (x as f32 - 300.0).hypot(y as f32 - 100.0) < 30.0 {
                    continue;
                }
                let i = (y * w + x) as usize;
                errs.push((got[i] / (k * want[i]) - 1.0).abs());
            }
        }
        errs.sort_by(f32::total_cmp);
        (errs[errs.len() / 2], errs[errs.len() * 999 / 1000])
    };
    let before = gpu_mosaic(&mut fe, &shot, none, None);
    let cal = calib::build(&shot, Some(&flat), Some(&dark), false).unwrap();
    let fixed = gpu_mosaic(
        &mut fe,
        &shot,
        corr(HighlightMode::Off, None, CaMode::Off, true),
        Some(&cal),
    );
    let blur = calib::build(&shot, Some(&flat), Some(&dark), true).unwrap();
    let smooth = gpu_mosaic(
        &mut fe,
        &shot,
        corr(HighlightMode::Off, None, CaMode::Off, true),
        Some(&blur),
    );
    let (b50, b999) = check(&before, false);
    let (f50, f999) = check(&fixed, false);
    let (s50, s999) = check(&smooth, true);
    eprintln!(
        "flat+dark: before median {b50:.4} p99.9 {b999:.4}; after {f50:.5} / {f999:.5}; vignetting only (dust left out) {s50:.5} / {s999:.5}"
    );
    assert!(b999 > 0.1);
    assert!(f999 < 0.01, "restored within 1%: {f999}");
    assert!(s999 < 0.01, "vignetting only: {s999}");
    // A dark frame of half the exposure time, scaled by the times (thermal signal grows
    // with time), calibrates as the matching one does.
    let half_dark = make(rggb(), w, h, WB, &|x, y| [0.5 * darkv(x, y); 3]);
    let scaled = calib::build_scaled(&shot, Some(&flat), Some(&half_dark), false, 2.0).unwrap();
    // The same within what the halved frame's whole-number samples lose (one step each,
    // doubled), in units of the white level.
    let dark_of = |c: &calib::Calibration| -> Vec<f32> {
        c.words
            .iter()
            .map(|w| half::f16::from_bits((w >> 16) as u16).to_f32())
            .collect()
    };
    let step = 2.5 / (16000.0 - 256.0);
    let worst = dark_of(&cal)
        .iter()
        .zip(dark_of(&scaled))
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert!(
        worst <= step,
        "half-time dark frame scaled: {worst} off (allowed {step})"
    );
    // A frame from another sensor size is refused.
    let other = make(rggb(), w - 2, h, WB, &scene);
    assert_eq!(
        calib::build(&shot, Some(&other), None, false),
        Err(calib::CalibError::Mismatch)
    );
}

#[test]
fn preview_matches_full_size_with_corrections() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (512u32, 384u32);
    let s = make(rggb(), w, h, WB, &blobs(w, h));
    let g = fe.upload(s).unwrap();
    let c = corr(HighlightMode::Opposed, Some(1.0), CaMode::Off, false);
    let run = |fe: &mut FrontEnd, q: Quality| {
        let o = Options {
            precision: Precision::F32,
            corrections: c,
            ..Options::new(q)
        };
        let out = fe.run(&g, &o).unwrap();
        // Half size corrects its own pixels (no full-size correction run).
        assert_eq!(
            out.stats.correction_dispatches == 0,
            q == Quality::Half,
            "{q:?}"
        );
        (out.width, fe.read(&out).unwrap())
    };
    let (hw, half) = run(&mut fe, Quality::Half);
    let (_, full) = run(&mut fe, Quality::Full);
    let mut d = Vec::new();
    let mut dh = Vec::new();
    for y in 0..h / 2 {
        for x in 0..hw {
            let mut m = [0f32; 3];
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let i = (((2 * y + dy) * w + 2 * x + dx) * 4) as usize;
                for k in 0..3 {
                    m[k] += 0.25 * full[i + k];
                }
            }
            let i = ((y * hw + x) * 4) as usize;
            let a = crate::colour::oklab::from_rec2020_f32(m);
            let b = crate::colour::oklab::from_rec2020_f32([half[i], half[i + 1], half[i + 2]]);
            let e = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            d.push(e);
            // Rebuilt highlights (above white): relative to their lightness.
            if a[0] > 1.0 {
                dh.push(e / a[0]);
            }
        }
    }
    d.sort_by(f32::total_cmp);
    dh.sort_by(f32::total_cmp);
    let (h50, h99) = (dh[dh.len() / 2], dh[dh.len() * 99 / 100]);
    eprintln!(
        "preview vs full size in rebuilt highlights ({} px, OkLab ΔE / L): median {h50:.4}, p99 {h99:.4}",
        dh.len()
    );
    assert!(h50 < 0.02 && h99 < 0.1, "{h50} {h99}");
    let (p50, p99) = (d[d.len() / 2], d[d.len() * 99 / 100]);
    eprintln!("preview vs full size (OkLab ΔE): median {p50:.4}, p99 {p99:.4}");
    assert!(p50 < 0.01 && p99 < 0.05, "{p50} {p99}");
}

#[test]
fn moving_a_correction_value_builds_no_pipeline() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (256u32, 192u32);
    let s = make(rggb(), w, h, WB, &blobs(w, h));
    let g = fe.upload(s).unwrap();
    let mut c = corr(
        HighlightMode::Rebuild,
        Some(1.0),
        CaMode::Manual {
            red: 0.1,
            blue: 0.1,
        },
        false,
    );
    let o = |c: Corrections| Options {
        corrections: c,
        ..Options::new(Quality::Half)
    };
    fe.run(&g, &o(c)).unwrap();
    let built = fe.pipelines_created;
    for t in [0.9, 0.95, 0.7] {
        c.highlight_threshold = t;
        c.hot = Some(t * 2.0);
        c.ca = CaMode::Manual {
            red: t / 10.0,
            blue: -t / 10.0,
        };
        fe.run(&g, &o(c)).unwrap();
    }
    c.highlights = HighlightMode::Opposed;
    fe.run(&g, &o(c)).unwrap();
    assert_eq!(fe.pipelines_created, built);
}

/// The preview never waits for the highlight statistics; they are kept per image and
/// settings, so only highlight and calibration changes take them again.
#[test]
fn statistics_refine_without_waiting_and_are_kept() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (512u32, 384u32);
    let s = make(rggb(), w, h, WB, &blobs(w, h));
    let g = fe.upload(s).unwrap();
    let mut c = corr(HighlightMode::Opposed, Some(1.0), CaMode::Off, false);
    let o = |c: Corrections| Options {
        precision: Precision::F32,
        corrections: c,
        wait_stats: false,
        ..Options::new(Quality::Half)
    };
    let first = fe.run(&g, &o(c)).unwrap();
    assert!(first.stats.stats_pending, "the first run does not wait");
    d.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert!(fe.stats_ready());
    let refined = fe.run(&g, &o(c)).unwrap();
    assert!(!refined.stats.stats_pending);
    let a = fe.read(&refined).unwrap();
    // The same as waiting for them.
    let waited = {
        let r = fe
            .run(
                &g,
                &Options {
                    wait_stats: true,
                    ..o(c)
                },
            )
            .unwrap();
        fe.read(&r).unwrap()
    };
    assert_eq!(a, waited);
    // Hot pixels and CA don't take them again.
    c.hot = Some(2.0);
    c.ca = CaMode::Manual {
        red: 0.1,
        blue: -0.1,
    };
    let r = fe.run(&g, &o(c)).unwrap();
    assert!(!r.stats.stats_pending && !fe.stats_pending());
    // A new clipping level does, and the run meanwhile uses the old grid.
    c.highlight_threshold = 0.9;
    let r = fe.run(&g, &o(c)).unwrap();
    assert!(r.stats.stats_pending && fe.stats_pending());
}

/// Front-end times with and without the corrections (prints; run with --ignored).
#[test]
#[ignore = "timing run: prints correction times for 24/45 MP"]
fn correction_timings() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    for (w, h) in [(6000u32, 4000u32), (8256, 5504)] {
        let s = make(rggb(), w, h, WB, &blobs(w, h));
        let g = fe.upload(s).unwrap();
        for (q, unfused) in [
            (Quality::Half, true),
            (Quality::Half, false),
            (Quality::Full, false),
        ] {
            fe.unfused_half = unfused;
            let pass = if unfused { " (three passes)" } else { "" };
            for (name, c) in [
                ("none", Corrections::default()),
                (
                    "default (opposed, hot)",
                    corr(HighlightMode::Opposed, Some(1.0), CaMode::Off, false),
                ),
                (
                    "rebuild + CA auto",
                    corr(HighlightMode::Rebuild, Some(1.0), CaMode::Auto, false),
                ),
                (
                    "hot only",
                    corr(HighlightMode::Off, Some(1.0), CaMode::Off, false),
                ),
                (
                    "clip only",
                    corr(HighlightMode::Clip, None, CaMode::Off, false),
                ),
            ] {
                let o = Options {
                    profile_gpu: true,
                    corrections: c,
                    ..Options::new(q)
                };
                // The statistics once (as the preview's first run starts them), then the
                // runs a slider on a correction makes (statistics kept).
                fe.run(&g, &o).unwrap();
                let stats = fe.last_stats_ms.unwrap_or(0.0);
                let mut walls = Vec::new();
                let mut gpu = Vec::new();
                for _ in 0..7 {
                    let t = std::time::Instant::now();
                    let out = fe.run(&g, &o).unwrap();
                    walls.push(t.elapsed().as_secs_f64() * 1e3);
                    gpu.push(out.stats.gpu_ms.unwrap_or(0.0));
                }
                walls.sort_by(f64::total_cmp);
                gpu.sort_by(f64::total_cmp);
                eprintln!(
                    "{w}x{h} {q:?}{pass} {name}: wall p50 {:.1} ms, GPU p50 {:.1} ms (statistics when taken {:.1} ms)",
                    walls[3], gpu[3], stats
                );
            }
        }
    }
    // X-Trans raw CA: the estimate (once per photo) and the full-size run with it.
    let (w, h) = (6000u32, 4000u32);
    let s = make(xtrans(), w, h, WB, &shifted(w, h, 0.0016, -0.0012));
    let t = std::time::Instant::now();
    let fit = ca::estimate_mosaic(&s);
    eprintln!(
        "x-trans {w}x{h} CA estimate: {:.0} ms ({})",
        t.elapsed().as_secs_f64() * 1e3,
        if fit.is_some() { "fitted" } else { "none" }
    );
    let g = fe.upload(s).unwrap();
    // "hot only" runs the correction passes without CA: the difference to "auto" is the
    // CA pass; "auto, ccolor" is the CA pass before the packed colour table.
    for (name, hot, ca, slow) in [
        ("off", None, CaMode::Off, false),
        ("off, hot only", Some(1.0), CaMode::Off, false),
        ("auto", None, CaMode::Auto, false),
        ("auto, ccolor", None, CaMode::Auto, true),
    ] {
        fe.slow_xtrans = slow;
        let o = Options {
            profile_gpu: true,
            corrections: corr(HighlightMode::Off, hot, ca, false),
            ..Options::new(Quality::Full)
        };
        fe.run(&g, &o).unwrap();
        let mut gpu: Vec<f64> = (0..5)
            .map(|_| fe.run(&g, &o).unwrap().stats.gpu_ms.unwrap_or(0.0))
            .collect();
        gpu.sort_by(f64::total_cmp);
        eprintln!("x-trans {w}x{h} Full, CA {name}: GPU p50 {:.1} ms", gpu[2]);
    }
    fe.slow_xtrans = false;
}

/// The fused half-size passes (`hfuse`, and `hfuseb` + `hout` with raw CA) give the three
/// passes' result bit for bit: Bayer and X-Trans, every highlight mode, hot pixels, raw CA,
/// whole and in bands.
#[test]
fn fused_half_passes_equal_the_three_passes() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (300u32, 212u32);
    for layout in [rggb(), xtrans()] {
        let s = make(layout, w, h, WB, &blobs(w, h));
        let g = fe.upload(s).unwrap();
        for c in [
            corr(HighlightMode::Opposed, Some(1.0), CaMode::Off, false),
            corr(HighlightMode::Clip, None, CaMode::Off, false),
            corr(
                HighlightMode::Rebuild,
                Some(0.5),
                CaMode::Manual {
                    red: 0.3,
                    blue: -0.3,
                },
                false,
            ),
        ] {
            for band_rows in [0, 37] {
                let o = Options {
                    precision: Precision::F32,
                    corrections: c,
                    band_rows,
                    ..Options::new(Quality::Half)
                };
                let mut out = Vec::new();
                for unfused in [true, false] {
                    fe.unfused_half = unfused;
                    let r = fe.run(&g, &o).unwrap();
                    out.push(fe.read(&r).unwrap());
                }
                fe.unfused_half = false;
                let same = out[0]
                    .iter()
                    .zip(&out[1])
                    .filter(|(a, b)| a.to_bits() == b.to_bits())
                    .count();
                assert_eq!(
                    same,
                    out[0].len(),
                    "{c:?}, bands of {band_rows}: {} of {} values differ",
                    out[0].len() - same,
                    out[0].len()
                );
            }
        }
    }
}

/// The fused full-size corrections (`cfuse`) give the three passes' corrected mosaic bit for
/// bit: Bayer and X-Trans, Fast and Full, every highlight mode, hot pixels, raw CA, whole
/// and in bands.
#[test]
fn fused_full_corrections_equal_the_three_passes() {
    let Some(d) = dev() else { return };
    let _l = lock();
    let mut fe = FrontEnd::new(&d.device, &d.queue);
    let (w, h) = (302u32, 214u32);
    for layout in [rggb(), xtrans()] {
        let s = make(layout, w, h, WB, &blobs(w, h));
        let g = fe.upload(s).unwrap();
        for c in [
            corr(HighlightMode::Opposed, Some(1.0), CaMode::Off, false),
            corr(HighlightMode::Clip, None, CaMode::Off, false),
            corr(
                HighlightMode::Rebuild,
                Some(0.5),
                CaMode::Manual {
                    red: 0.3,
                    blue: -0.3,
                },
                false,
            ),
        ] {
            for (q, band_rows) in [(Quality::Fast, 0), (Quality::Full, 0), (Quality::Full, 37)] {
                let o = Options {
                    precision: Precision::F32,
                    corrections: c,
                    band_rows,
                    ..Options::new(q)
                };
                let mut out = Vec::new();
                for unfused in [true, false] {
                    fe.unfused_full = unfused;
                    let r = fe.run(&g, &o).unwrap();
                    out.push(fe.read(&r).unwrap());
                }
                fe.unfused_full = false;
                let differ = out[0]
                    .iter()
                    .zip(&out[1])
                    .filter(|(a, b)| a.to_bits() != b.to_bits())
                    .count();
                assert_eq!(
                    differ, 0,
                    "{c:?}, {q:?}, bands of {band_rows}: values differ"
                );
            }
        }
    }
}

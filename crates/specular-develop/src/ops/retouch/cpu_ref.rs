//! CPU reference of retouch (tests only), in f64: the same layers, blur and mixing as the
//! kernels, with each Poisson problem solved by over-relaxed Gauss–Seidel to a residual of
//! 1e-12, and an f64 copy of the GPU's multigrid to check its convergence.

use super::layout::{COARSE_SWEEPS, COARSEST, CYCLES, Chunk, Layout, POST, PRE};
use super::{Layer, Placed, PlacedSpot, SpotMode};

type Px = [f64; 3];

/// An RGBA f32 picture.
pub struct Pic<'a> {
    pub w: u32,
    pub h: u32,
    pub px: &'a [f32],
}

impl Pic<'_> {
    /// The pixel at `(x, y)`, clamped to the picture (as `src_at`).
    pub fn at(&self, x: i64, y: i64) -> Px {
        let x = x.clamp(0, i64::from(self.w) - 1) as usize;
        let y = y.clamp(0, i64::from(self.h) - 1) as usize;
        let i = 4 * (y * self.w as usize + x);
        [
            f64::from(self.px[i]),
            f64::from(self.px[i + 1]),
            f64::from(self.px[i + 2]),
        ]
    }
}

fn add(a: Px, b: Px) -> Px {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Px, b: Px) -> Px {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(k: f64, a: Px) -> Px {
    [k * a[0], k * a[1], k * a[2]]
}

fn wt(k: f64, sigma: f64) -> f64 {
    (-(k * k) / (2.0 * sigma * sigma)).exp()
}

/// The low layer at any pixel: rows of clamped pixels blurred, then the rows blurred.
fn low(pic: &Pic<'_>, sigma: f64, x: i64, y: i64) -> Px {
    let m = (3.0 * sigma.max(0.3)).ceil() as i64;
    let mut acc = [0.0; 3];
    let mut ws = 0.0;
    for j in -m..=m {
        let mut row = [0.0; 3];
        let mut rs = 0.0;
        for k in -m..=m {
            let w = wt(k as f64, sigma);
            row = add(row, mul(w, pic.at(x + k, y + j)));
            rs += w;
        }
        let w = wt(j as f64, sigma);
        acc = add(acc, mul(w / rs, row));
        ws += w;
    }
    mul(1.0 / ws, acc)
}

fn layer(pic: &Pic<'_>, l: Layer, split: f64, x: i64, y: i64) -> Px {
    match l {
        Layer::All => pic.at(x, y),
        Layer::Low => low(pic, split, x, y),
        Layer::High => sub(pic.at(x, y), low(pic, split, x, y)),
    }
}

fn dist(s: &PlacedSpot, p: [f64; 2]) -> f64 {
    let d = |a: [f64; 2]| ((p[0] - a[0]).powi(2) + (p[1] - a[1]).powi(2)).sqrt();
    let f = |q: [f32; 2]| [f64::from(q[0]), f64::from(q[1])];
    match s.path.len() {
        0 => d(f(s.centre)),
        1 => d(f(s.path[0])),
        _ => s
            .path
            .windows(2)
            .map(|w| {
                let (a, b) = (f(w[0]), f(w[1]));
                let ab = [b[0] - a[0], b[1] - a[1]];
                let ap = [p[0] - a[0], p[1] - a[1]];
                let l2 = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12);
                let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0);
                d([a[0] + ab[0] * t, a[1] + ab[1] * t])
            })
            .fold(f64::MAX, f64::min),
    }
}

/// A Dirichlet problem on a grid: `free` cells solve `Σ neighbours − 4u = 0`, the others
/// hold their values.
#[derive(Clone)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub free: Vec<bool>,
    pub u: Vec<Px>,
}

impl Grid {
    fn nb(&self, u: &[Px], x: usize, y: usize) -> Px {
        let g = |xi: i64, yi: i64| {
            if xi < 0 || yi < 0 || xi >= self.w as i64 || yi >= self.h as i64 {
                [0.0; 3]
            } else {
                u[yi as usize * self.w + xi as usize]
            }
        };
        let (x, y) = (x as i64, y as i64);
        add(add(g(x - 1, y), g(x + 1, y)), add(g(x, y - 1), g(x, y + 1)))
    }

    /// Largest `|Σ neighbours − 4u|` over free cells.
    pub fn residual(&self) -> f64 {
        let mut worst: f64 = 0.0;
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                if self.free[i] {
                    let r = sub(self.nb(&self.u, x, y), mul(4.0, self.u[i]));
                    worst = worst.max(r.iter().fold(0.0f64, |a, v| a.max(v.abs())));
                }
            }
        }
        worst
    }

    /// Over-relaxed Gauss–Seidel until the residual is below `tol`.
    pub fn solve_sor(&mut self, tol: f64) {
        let n = self.w.max(self.h) as f64;
        let omega = 2.0 / (1.0 + (std::f64::consts::PI / n).sin());
        for _ in 0..200_000 {
            for y in 0..self.h {
                for x in 0..self.w {
                    let i = y * self.w + x;
                    if self.free[i] {
                        let gs = mul(0.25, self.nb(&self.u, x, y));
                        self.u[i] = add(self.u[i], mul(omega, sub(gs, self.u[i])));
                    }
                }
            }
            if self.residual() < tol {
                return;
            }
        }
        panic!("SOR did not converge");
    }

    /// The GPU's multigrid in f64: [`CYCLES`] V-cycles.
    pub fn solve_mg(&mut self) {
        let mut levels: Vec<Level> = vec![Level {
            w: self.w,
            h: self.h,
            free: self.free.clone(),
            u: std::mem::take(&mut self.u),
            rhs: None,
        }];
        let (mut w, mut h) = (self.w, self.h);
        while w.max(h) > COARSEST as usize {
            w = w.div_ceil(2);
            h = h.div_ceil(2);
            levels.push(Level {
                w,
                h,
                free: vec![false; w * h],
                u: vec![[0.0; 3]; w * h],
                rhs: Some(vec![[0.0; 3]; w * h]),
            });
        }
        let depth = levels.len() - 1;
        for _ in 0..CYCLES {
            for l in 0..depth {
                for _ in 0..PRE {
                    levels[l].sweep(l, 0);
                    levels[l].sweep(l, 1);
                }
                let (fine, coarse) = levels.split_at_mut(l + 1);
                restrict(&fine[l], &mut coarse[0], l);
            }
            levels[depth].coarse(depth);
            for l in (0..depth).rev() {
                let (fine, coarse) = levels.split_at_mut(l + 1);
                prolong(&mut fine[l], &coarse[0]);
                for _ in 0..POST {
                    levels[l].sweep(l, 0);
                    levels[l].sweep(l, 1);
                }
            }
        }
        self.u = std::mem::take(&mut levels[0].u);
    }
}

struct Level {
    w: usize,
    h: usize,
    free: Vec<bool>,
    u: Vec<Px>,
    rhs: Option<Vec<Px>>,
}

impl Level {
    fn grid_nb(&self, x: usize, y: usize) -> Px {
        let g = |xi: i64, yi: i64| {
            if xi < 0 || yi < 0 || xi >= self.w as i64 || yi >= self.h as i64 {
                [0.0; 3]
            } else {
                self.u[yi as usize * self.w + xi as usize]
            }
        };
        let (x, y) = (x as i64, y as i64);
        add(add(g(x - 1, y), g(x + 1, y)), add(g(x, y - 1), g(x, y + 1)))
    }

    fn rhs(&self, i: usize) -> Px {
        self.rhs.as_ref().map_or([0.0; 3], |r| r[i])
    }

    fn sweep(&mut self, l: usize, colour: usize) {
        let h2 = (1u64 << (2 * l)) as f64;
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                if (x + y) & 1 == colour && self.free[i] {
                    self.u[i] = mul(0.25, add(self.grid_nb(x, y), mul(h2, self.rhs(i))));
                }
            }
        }
    }

    fn coarse(&mut self, l: usize) {
        let h2 = (1u64 << (2 * l)) as f64;
        for _ in 0..COARSE_SWEEPS {
            for y in 0..self.h {
                for x in 0..self.w {
                    let i = y * self.w + x;
                    if self.free[i] {
                        let gs = mul(0.25, add(self.grid_nb(x, y), mul(h2, self.rhs(i))));
                        self.u[i] = add(self.u[i], mul(1.5, sub(gs, self.u[i])));
                    }
                }
            }
        }
    }
}

fn restrict(f: &Level, c: &mut Level, l: usize) {
    let h2 = (1u64 << (2 * l)) as f64;
    for cy in 0..c.h {
        for cx in 0..c.w {
            let mut acc = [0.0; 3];
            let mut n = 0;
            for (a, b) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (fx, fy) = (2 * cx + a, 2 * cy + b);
                if fx >= f.w || fy >= f.h {
                    continue;
                }
                let i = fy * f.w + fx;
                if !f.free[i] {
                    continue;
                }
                n += 1;
                let lap = sub(mul(4.0, f.u[i]), f.grid_nb(fx, fy));
                acc = add(acc, sub(f.rhs(i), mul(1.0 / h2, lap)));
            }
            let i = cy * c.w + cx;
            let fr = n == 4;
            c.free[i] = fr;
            c.u[i] = [0.0; 3];
            if let Some(r) = &mut c.rhs {
                r[i] = mul(0.25, acc);
            }
        }
    }
}

fn prolong(f: &mut Level, c: &Level) {
    for y in 0..f.h {
        for x in 0..f.w {
            let i = y * f.w + x;
            if !f.free[i] {
                continue;
            }
            let gx = (x as f64 * 0.5 - 0.25).clamp(0.0, (c.w - 1) as f64);
            let gy = (y as f64 * 0.5 - 0.25).clamp(0.0, (c.h - 1) as f64);
            let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(c.w - 1), (y0 + 1).min(c.h - 1));
            let (ax, ay) = (gx - x0 as f64, gy - y0 as f64);
            let cu = |xx: usize, yy: usize| c.u[yy * c.w + xx];
            let top = add(mul(1.0 - ax, cu(x0, y0)), mul(ax, cu(x1, y0)));
            let bot = add(mul(1.0 - ax, cu(x0, y1)), mul(ax, cu(x1, y1)));
            f.u[i] = add(f.u[i], add(mul(1.0 - ay, top), mul(ay, bot)));
        }
    }
}

/// A heal's source at picture pixel (`x`, `y`): its removal patch (as the kernel samples
/// it, in f32) when it has one, else the layer at its offset.
fn source(pic: &Pic<'_>, s: &PlacedSpot, split: f64, x: i64, y: i64) -> Px {
    if let Some(p) = &s.patch
        && let Some(t) = super::patch::store().lock().unwrap().get(&p.key)
    {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let a = p.affine;
        let v = t.sample(a[0] * px + a[1] * py + a[2], a[3] * px + a[4] * py + a[5]);
        return [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])];
    }
    layer(
        pic,
        s.layer,
        split,
        x + i64::from(s.offset[0]),
        y + i64::from(s.offset[1]),
    )
}

/// The edge problem of a heal or fill spot on its `G0` chunk (stride 1).
pub fn spot_grid(pic: &Pic<'_>, p: &Placed, s: &PlacedSpot, g0: &Chunk) -> Grid {
    assert_eq!(g0.s, 1, "the reference works on stride 1");
    let (w, h) = (g0.w as usize, g0.h as usize);
    let mut grid = Grid {
        w,
        h,
        free: vec![false; w * h],
        u: vec![[0.0; 3]; w * h],
    };
    let split = f64::from(p.split);
    for cy in 0..h {
        for cx in 0..w {
            let (x, y) = (i64::from(g0.ox) + cx as i64, i64::from(g0.oy) + cy as i64);
            let i = cy * w + cx;
            let border = cx == 0 || cy == 0 || cx + 1 == w || cy + 1 == h;
            if !border && dist(s, [x as f64, y as f64]) < f64::from(s.radius) {
                grid.free[i] = true;
                continue;
            }
            let t = layer(pic, s.layer, split, x, y);
            grid.u[i] = match s.mode {
                SpotMode::Heal | SpotMode::Remove => sub(t, source(pic, s, split, x, y)),
                _ => t,
            };
        }
    }
    grid
}

/// Retouch of `pic` with `p`, in f64 (stride 1 only).
pub fn retouch(pic: &Pic<'_>, p: &Placed) -> Vec<f32> {
    let lay = Layout::new(p);
    let split = f64::from(p.split);
    let live: Vec<&PlacedSpot> = p.spots.iter().filter(|s| s.opacity > 0.0).collect();
    assert_eq!(live.len(), lay.chunks.len(), "every spot is on the picture");
    // Solutions of heal and fill spots.
    let solved: Vec<Option<(Chunk, Grid)>> = live
        .iter()
        .zip(&lay.chunks)
        .map(|(s, c)| {
            c.g0.map(|g0| {
                let mut g = spot_grid(pic, p, s, &g0);
                g.solve_sor(1e-12);
                (g0, g)
            })
        })
        .collect();
    let mut out = pic.px.to_vec();
    for y in 0..i64::from(pic.h) {
        for x in 0..i64::from(pic.w) {
            let base = pic.at(x, y);
            let mut c = base;
            for (k, s) in live.iter().enumerate() {
                let d = dist(s, [x as f64, y as f64]);
                let r = f64::from(s.radius);
                if d >= r {
                    continue;
                }
                let f = f64::from(s.feather);
                let mut m = (r - d).clamp(0.0, 1.0);
                if f > 0.0 {
                    let e0 = r * (1.0 - f);
                    let t = ((d - e0) / (r - e0)).clamp(0.0, 1.0);
                    m = m.min(1.0 - t * t * (3.0 - 2.0 * t));
                }
                m *= f64::from(s.opacity);
                let q = (x + i64::from(s.offset[0]), y + i64::from(s.offset[1]));
                let lq = || layer(pic, s.layer, split, q.0, q.1);
                let g = || {
                    let (g0, grid) = solved[k].as_ref().expect("solved");
                    let (cx, cy) = (
                        (x - i64::from(g0.ox)) as usize,
                        (y - i64::from(g0.oy)) as usize,
                    );
                    grid.u[cy * grid.w + cx]
                };
                let patch = match s.mode {
                    SpotMode::Clone => lq(),
                    // Removal spots are placed as heals.
                    SpotMode::Heal | SpotMode::Remove => add(source(pic, s, split, x, y), g()),
                    SpotMode::Fill => {
                        let mut acc = [0.0; 3];
                        let mut ws = 0.0;
                        for j in -3..=3i64 {
                            for i in -3..=3i64 {
                                let w = wt(i as f64, 1.0) * wt(j as f64, 1.0);
                                acc =
                                    add(acc, mul(w, layer(pic, s.layer, split, q.0 + i, q.1 + j)));
                                ws += w;
                            }
                        }
                        add(g(), sub(lq(), mul(1.0 / ws, acc)))
                    }
                    SpotMode::Blur => {
                        let sg = f64::from(s.blur).max(0.3);
                        let m = (3.0 * sg).ceil() as i64;
                        let mut acc = [0.0; 3];
                        let mut ws = 0.0;
                        for j in -m..=m {
                            let mut row = [0.0; 3];
                            let mut rs = 0.0;
                            for i in -m..=m {
                                let w = wt(i as f64, sg);
                                row = add(row, mul(w, layer(pic, s.layer, split, x + i, y + j)));
                                rs += w;
                            }
                            let w = wt(j as f64, sg);
                            acc = add(acc, mul(w / rs, row));
                            ws += w;
                        }
                        mul(1.0 / ws, acc)
                    }
                };
                let other = match s.layer {
                    Layer::All => [0.0; 3],
                    Layer::Low => sub(base, low(pic, split, x, y)),
                    Layer::High => low(pic, split, x, y),
                };
                c = add(c, mul(m, sub(patch, sub(c, other))));
            }
            let i = 4 * (y as usize * pic.w as usize + x as usize);
            for ch in 0..3 {
                out[i + ch] = c[ch] as f32;
            }
        }
    }
    out
}

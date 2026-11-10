//! CPU reference of the raw front end (tests only): the same f32 arithmetic as the kernels,
//! over the whole image at once, read from the [`SensorData`] directly (not from the packed
//! data buffer), so the packing is checked too.

// Index loops mirror the kernels line by line.
#![allow(clippy::needless_range_loop)]

use specular_raw::SensorData;
use specular_raw::sensor::{HsvTableData, Layout, OpcodeInfo};

use super::data::{output_matrix, wb};
use super::linear::{points, run_list};
use super::{Options, Output, Quality};

struct Ctx<'a> {
    s: &'a SensorData,
    wb: [f32; 3],
    bad: Option<(u16, [u32; 4])>,
    l1: Vec<&'a OpcodeInfo>,
    l2: Vec<&'a OpcodeInfo>,
    l3: Vec<&'a OpcodeInfo>,
    mat: [[f32; 3]; 3],
    profile: Option<Profile>,
    output: Output,
}

struct Profile {
    pro_cam: [[f32; 3]; 3],
    post: [[f32; 3]; 3],
    hdr: bool,
    hsm: Option<HsvTableData>,
    lt: Option<HsvTableData>,
}

fn refl(v: i32, n: i32) -> i32 {
    let mut r = v.abs();
    if r >= n {
        r = 2 * n - 2 - r;
    }
    r.clamp(0, n - 1)
}

fn mat3(m: &[[f32; 3]; 3], c: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2])
}

fn rgb_to_hsv([r, g, b]: [f32; 3]) -> [f32; 3] {
    let v = r.max(g).max(b);
    let gap = v - r.min(g).min(b);
    if gap <= 0.0 {
        return [0.0, 0.0, v];
    }
    let h = if r == v {
        let h = (g - b) / gap;
        if h < 0.0 { h + 6.0 } else { h }
    } else if g == v {
        2.0 + (b - r) / gap
    } else {
        4.0 + (r - g) / gap
    };
    [h, gap / v, v]
}

fn hsv_to_rgb([h, s, v]: [f32; 3]) -> [f32; 3] {
    if s <= 0.0 {
        return [v; 3];
    }
    let h = if !(0.0..6.0).contains(&h) { 0.0 } else { h };
    let i = (h as u32).min(5);
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn srgb_enc(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_dec(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn hdr_enc(x: f32) -> f32 {
    x * (256.0 + x) / (256.0 * (1.0 + x))
}

fn hdr_dec(y: f32) -> f32 {
    16.0 * (8.0 * y - 8.0 + (64.0 * y * y - 127.0 * y + 64.0).max(0.0).sqrt())
}

fn mix3(a: [f32; 3], b: [f32; 3], f: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f)
}

fn lookup(t: &HsvTableData, h6: f32, s: f32, v: f32) -> [f32; 3] {
    let at = |v: usize, h: usize, s: usize| t.data[(v * t.hue as usize + h) * t.sat as usize + s];
    let nh = t.hue as usize;
    let hs = h6 * t.hue as f32 / 6.0;
    let mut h0 = (hs.max(0.0) as usize).min(nh - 1);
    let hf = (hs - h0 as f32).clamp(0.0, 1.0);
    let h1 = if h0 + 1 >= nh { 0 } else { h0 + 1 };
    if nh == 1 {
        h0 = 0;
    }
    let ss = s.clamp(0.0, 1.0) * (t.sat - 1) as f32;
    let s0 = (ss as usize).min(t.sat as usize - 2);
    let sf = ss - s0 as f32;
    let plane = |v: usize| {
        mix3(
            mix3(at(v, h0, s0), at(v, h1, s0), hf),
            mix3(at(v, h0, s0 + 1), at(v, h1, s0 + 1), hf),
            sf,
        )
    };
    if t.val > 1 {
        let vs = v.clamp(0.0, 1.0) * (t.val - 1) as f32;
        let v0 = (vs as usize).min(t.val as usize - 2);
        mix3(plane(v0), plane(v0 + 1), vs - v0 as f32)
    } else {
        plane(0)
    }
}

fn hsv_stage(t: &HsvTableData, hdr: bool, p: [f32; 3]) -> [f32; 3] {
    let enc = hdr && t.val > 1;
    let p = if enc { p.map(hdr_enc) } else { p };
    let [h0, s0, v0] = rgb_to_hsv(p);
    let ve = if t.srgb { srgb_enc(v0) } else { v0 };
    let d = lookup(t, h0, s0, ve);
    let mut h = h0 + d[0] / 60.0;
    h -= 6.0 * (h / 6.0).floor();
    if !(0.0..6.0).contains(&h) {
        h = 0.0;
    }
    let s = (s0 * d[1]).min(1.0);
    let mut v = (ve * d[2]).max(0.0);
    if t.srgb {
        v = srgb_dec(v);
    }
    let o = hsv_to_rgb([h, s, v]);
    if enc { o.map(hdr_dec) } else { o }
}

impl Ctx<'_> {
    fn raw(&self, sx: u32, sy: u32, c: u32) -> u16 {
        self.s.sample(sx, sy, c)
    }

    fn stored(&self, sx: u32, sy: u32, c: u32) -> f32 {
        if let Some(f) = &self.s.float {
            let cpp = self.s.cpp as usize;
            return f[(sy as usize * self.s.width as usize + sx as usize) * cpp + c as usize];
        }
        let v = self.raw(sx, sy, c);
        let Some((bv, r)) = self.bad else {
            return f32::from(v);
        };
        let inr = |x: u32, y: u32| x >= r[0] && y >= r[1] && x - r[0] < r[2] && y - r[1] < r[3];
        if v != bv || !inr(sx, sy) {
            return f32::from(v);
        }
        let layout = &self.s.meta.layout;
        let cfa = matches!(layout, Layout::Bayer(_) | Layout::Pattern(_));
        let col = layout.color(sx, sy);
        let (mut sw, mut sv) = (0f32, 0f32);
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (sx as i32 + dx, sy as i32 + dy);
                if nx < 0 || ny < 0 || !inr(nx as u32, ny as u32) {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if ny >= self.s.height || nx >= self.s.width {
                    continue;
                }
                if cfa && layout.color(nx, ny) != col {
                    continue;
                }
                let q = self.raw(nx, ny, c);
                if q == bv {
                    continue;
                }
                let w = 1.0 / (dx * dx + dy * dy) as f32;
                sw += w;
                sv += w * f32::from(q);
            }
        }
        if sw > 0.0 {
            (sv / sw + 0.5).floor()
        } else {
            f32::from(v)
        }
    }

    fn prep(&self, x: i32, y: i32, c: u32) -> f32 {
        let s = self.s;
        let m = &s.meta;
        let sx = s.crop.x + x as u32;
        let sy = s.crop.y + y as u32;
        let (xc, yc) = (sx as f32 + 0.5, sy as f32 + 0.5);
        let mut v = self.stored(sx, sy, c);
        if !self.l1.is_empty() {
            v = run_list(&self.l1, v / 65535.0, sx, sy, xc, yc, c, s.cpp) * 65535.0;
        }
        if let Some(t) = &m.linearization
            && !t.is_empty()
        {
            let i = ((v + 0.5).clamp(0.0, 65535.0) as usize).min(t.len() - 1);
            v = f32::from(t[i]);
        }
        let b = m.black.at(sx, sy, c);
        v = (v - b) / (m.white - b).max(1.0);
        if !self.l2.is_empty() {
            v = run_list(&self.l2, v, sx, sy, xc, yc, c, s.cpp);
        }
        let col = if s.cpp == 1 {
            u32::from(m.layout.color(sx, sy))
        } else {
            c
        };
        v * self.wb[col as usize]
    }

    fn color(&self, x: i32, y: i32) -> u8 {
        self.s
            .meta
            .layout
            .color(self.s.crop.x + x as u32, self.s.crop.y + y as u32)
    }

    fn finish(&self, c: [f32; 3], ix: u32, iy: u32, xc: f32, yc: f32) -> [f32; 4] {
        let mut c = c;
        if !self.l3.is_empty() {
            c = std::array::from_fn(|p| run_list(&self.l3, c[p], ix, iy, xc, yc, p as u32, 3));
        }
        if self.output == Output::CameraRgb {
            return [c[0], c[1], c[2], 1.0];
        }
        let o = match &self.profile {
            None => mat3(&self.mat, c),
            Some(p) => {
                let mut q = mat3(&p.pro_cam, c);
                if p.hsm.is_some() || p.lt.is_some() {
                    let neg = q.map(|v| v.min(0.0));
                    let mut r = q.map(|v| v.max(0.0));
                    if let Some(t) = &p.hsm {
                        r = hsv_stage(t, p.hdr, r);
                    }
                    if let Some(t) = &p.lt {
                        r = hsv_stage(t, p.hdr, r);
                    }
                    q = std::array::from_fn(|i| r[i] + neg[i]);
                }
                mat3(&p.post, q)
            }
        };
        [o[0], o[1], o[2], 1.0]
    }

    fn finish_full(&self, c: [f32; 3], x: u32, y: u32) -> [f32; 4] {
        let ix = self.s.crop.x + x;
        let iy = self.s.crop.y + y;
        self.finish(c, ix, iy, ix as f32 + 0.5, iy as f32 + 0.5)
    }
}

/// A whole-image plane with reads that reflect (Bayer) or clamp (X-Trans).
struct Plane<T> {
    w: i32,
    h: i32,
    v: Vec<T>,
    reflect: bool,
}

impl<T: Copy> Plane<T> {
    fn new(w: u32, h: u32, reflect: bool, f: impl Fn(i32, i32) -> T) -> Plane<T> {
        let mut v = Vec::with_capacity(w as usize * h as usize);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                v.push(f(x, y));
            }
        }
        Plane {
            w: w as i32,
            h: h as i32,
            v,
            reflect,
        }
    }
    fn at(&self, x: i32, y: i32) -> T {
        let (x, y) = if self.reflect {
            (refl(x, self.w), refl(y, self.h))
        } else {
            (x.clamp(0, self.w - 1), y.clamp(0, self.h - 1))
        };
        self.v[(y * self.w + x) as usize]
    }
}

/// Develops `s` per `o` on the CPU: (width, height, rgba).
pub(crate) fn develop(s: &SensorData, o: &Options) -> (u32, u32, Vec<f32>) {
    let m = &s.meta;
    let (l1, bad) = if o.opcodes {
        match m.opcodes[0].first() {
            Some(OpcodeInfo::FixBadPixelsConstant { value, bounds }) => (
                points(&m.opcodes[0][1..]),
                Some((*value, [bounds.x, bounds.y, bounds.w, bounds.h])),
            ),
            _ => (points(&m.opcodes[0]), None),
        }
    } else {
        (Vec::new(), None)
    };
    let lists = |i: usize| {
        if o.opcodes {
            points(&m.opcodes[i])
        } else {
            Vec::new()
        }
    };
    let profile = match (&m.profile, o.profile) {
        (Some(p), true) => Some(Profile {
            pro_cam: p.camera_to_prophoto(),
            post: super::data::profile_post(m, p),
            hdr: p.is_hdr(),
            hsm: p.hue_sat_map(),
            lt: p.look_table(),
        }),
        _ => None,
    };
    let ctx = Ctx {
        s,
        wb: wb(m, o),
        bad,
        l1,
        l2: lists(1),
        l3: lists(2),
        mat: output_matrix(m),
        profile,
        output: o.output,
    };
    let (cw, ch) = (s.crop.w, s.crop.h);
    let (ow, oh) = super::FrontEnd::output_size(s, o.quality, o.output);
    let mut out = Vec::with_capacity(ow as usize * oh as usize * 4);
    let mut put = |p: [f32; 4]| out.extend_from_slice(&p);
    if o.output == Output::Mosaic {
        for y in 0..ch as i32 {
            for x in 0..cw as i32 {
                put([ctx.prep(x, y, 0), 0.0, 0.0, 1.0]);
            }
        }
        return (ow, oh, out);
    }
    if o.quality == Quality::Half {
        let pattern = matches!(m.layout, Layout::Pattern(_));
        let (r0, r1) = if pattern { (-1, 3) } else { (0, 2) };
        for y in 0..oh as i32 {
            for x in 0..ow as i32 {
                let (mut sum, mut n) = ([0f32; 3], [0f32; 3]);
                for dy in r0..r1 {
                    for dx in r0..r1 {
                        let sx = (2 * x + dx).clamp(0, cw as i32 - 1);
                        let sy = (2 * y + dy).clamp(0, ch as i32 - 1);
                        match m.layout {
                            Layout::Bayer(_) | Layout::Pattern(_) => {
                                let c = ctx.color(sx, sy) as usize;
                                sum[c] += ctx.prep(sx, sy, 0);
                                n[c] += 1.0;
                            }
                            Layout::Linear => {
                                for c in 0..3 {
                                    sum[c] += ctx.prep(sx, sy, c as u32);
                                    n[c] += 1.0;
                                }
                            }
                            Layout::Mono => {
                                let v = ctx.prep(sx, sy, 0);
                                for c in 0..3 {
                                    sum[c] += v;
                                    n[c] += 1.0;
                                }
                            }
                        }
                    }
                }
                let c: [f32; 3] = std::array::from_fn(|i| sum[i] / n[i].max(1.0));
                let ix = s.crop.x + 2 * x as u32;
                let iy = s.crop.y + 2 * y as u32;
                put(ctx.finish(c, ix, iy, ix as f32 + 1.0, iy as f32 + 1.0));
            }
        }
        return (ow, oh, out);
    }
    match (&m.layout, o.quality) {
        (Layout::Linear | Layout::Mono, _) => {
            for y in 0..ch as i32 {
                for x in 0..cw as i32 {
                    let v = ctx.prep(x, y, 0);
                    let c = if s.cpp == 3 {
                        [v, ctx.prep(x, y, 1), ctx.prep(x, y, 2)]
                    } else {
                        [v; 3]
                    };
                    put(ctx.finish_full(c, x as u32, y as u32));
                }
            }
        }
        (Layout::Bayer(_), Quality::Fast) => {
            let mo = Plane::new(cw, ch, true, |x, y| ctx.prep(x, y, 0));
            for y in 0..ch as i32 {
                for x in 0..cw as i32 {
                    put(ctx.finish_full(mhc(&ctx, &mo, x, y), x as u32, y as u32));
                }
            }
        }
        (Layout::Bayer(_), _) => {
            for p in rcd(&ctx, cw, ch, o.dual.filter(|t| *t > 0.0)) {
                put(p);
            }
        }
        (Layout::Pattern(_), _) => {
            for p in xtrans(&ctx, cw, ch) {
                put(p);
            }
        }
    }
    (ow, oh, out)
}

fn mhc(ctx: &Ctx, mo: &Plane<f32>, x: i32, y: i32) -> [f32; 3] {
    let m = |dx: i32, dy: i32| mo.at(x + dx, y + dy);
    let c = ctx.color(x, y);
    let v = m(0, 0);
    let cross = m(0, -1) + m(0, 1) + m(-1, 0) + m(1, 0);
    let diag = m(-1, -1) + m(1, -1) + m(-1, 1) + m(1, 1);
    let far = m(0, -2) + m(0, 2) + m(-2, 0) + m(2, 0);
    let farh = m(-2, 0) + m(2, 0);
    let farv = m(0, -2) + m(0, 2);
    if c == 1 {
        let h = (5.0 * v + 4.0 * (m(-1, 0) + m(1, 0)) - diag - farh + 0.5 * farv) / 8.0;
        let w = (5.0 * v + 4.0 * (m(0, -1) + m(0, 1)) - diag - farv + 0.5 * farh) / 8.0;
        if ctx.color(x + 1, y) == 0 {
            [h, v, w]
        } else {
            [w, v, h]
        }
    } else {
        let g = (4.0 * v + 2.0 * cross - far) / 8.0;
        let o = (6.0 * v + 2.0 * diag - 1.5 * far) / 8.0;
        if c == 0 { [v, g, o] } else { [o, g, v] }
    }
}

/// Bilinear demosaic at (x, y) (the dual demosaic's smooth method).
fn bilinear(ctx: &Ctx, mo: &Plane<f32>, x: i32, y: i32) -> [f32; 3] {
    let m = |dx: i32, dy: i32| mo.at(x + dx, y + dy);
    let v = m(0, 0);
    if ctx.color(x, y) == 1 {
        let h = 0.5 * (m(-1, 0) + m(1, 0));
        let w = 0.5 * (m(0, -1) + m(0, 1));
        return if ctx.color(x + 1, y) == 0 {
            [h, v, w]
        } else {
            [w, v, h]
        };
    }
    let g = 0.25 * (m(0, -1) + m(0, 1) + m(-1, 0) + m(1, 0));
    let o = 0.25 * (m(-1, -1) + m(1, -1) + m(-1, 1) + m(1, 1));
    if ctx.color(x, y) == 0 {
        [v, g, o]
    } else {
        [o, g, v]
    }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn rcd(ctx: &Ctx, cw: u32, ch: u32, dual: Option<f32>) -> Vec<[f32; 4]> {
    let mo = Plane::new(cw, ch, true, |x, y| ctx.prep(x, y, 0));
    // Step 1.
    let s1 = Plane::new(cw, ch, true, |x, y| {
        let m = |dx: i32, dy: i32| mo.at(x + dx, y + dy);
        let hp = |px: i32, py: i32, dx: i32, dy: i32| {
            let a = (m(px - dx, py - dy) - m(px + dx, py + dy)).abs();
            let b =
                (2.0 * m(px, py) - m(px - 2 * dx, py - 2 * dy) - m(px + 2 * dx, py + 2 * dy)).abs();
            let g = a + 0.5 * b;
            g * g
        };
        let stat = |dx: i32, dy: i32| hp(-dx, -dy, dx, dy) + hp(0, 0, dx, dy) + hp(dx, dy, dx, dy);
        let lpf = 0.25 * m(0, 0)
            + 0.125 * (m(0, -1) + m(0, 1) + m(-1, 0) + m(1, 0))
            + 0.0625 * (m(-1, -1) + m(1, -1) + m(-1, 1) + m(1, 1));
        let e = 1e-10;
        let (v, h, p, q) = (stat(0, 1), stat(1, 0), stat(1, 1), stat(-1, 1));
        [
            m(0, 0),
            lpf,
            (v + e) / (v + h + 2.0 * e),
            (p + e) / (p + q + 2.0 * e),
        ]
    });
    // Step 2.
    let disc = |c: f32, n: f32| {
        if (0.5 - c).abs() < (0.5 - n).abs() {
            n
        } else {
            c
        }
    };
    let s2 = Plane::new(cw, ch, true, |x, y| {
        let a = |dx: i32, dy: i32| s1.at(x + dx, y + dy);
        let p = a(0, 0);
        let (d0, d1, d2, d3) = (a(-1, -1), a(1, -1), a(-1, 1), a(1, 1));
        let vh = disc(p[2], 0.25 * (d0[2] + d1[2] + d2[2] + d3[2]));
        let pq = disc(p[3], 0.25 * (d0[3] + d1[3] + d2[3] + d3[3]));
        let mut g = p[0];
        if ctx.color(x, y) != 1 {
            let lc = p[1].max(0.0);
            let est = |dx: i32, dy: i32| {
                let ln = a(2 * dx, 2 * dy)[1].max(0.0);
                a(dx, dy)[0] * (2.0 * lc + 1e-4) / (lc + ln + 1e-4)
            };
            let grad = |dx: i32, dy: i32| {
                1e-5 + (a(dx, dy)[0] - a(-dx, -dy)[0]).abs()
                    + (a(0, 0)[0] - a(2 * dx, 2 * dy)[0]).abs()
                    + (a(dx, dy)[0] - a(3 * dx, 3 * dy)[0]).abs()
                    + (a(2 * dx, 2 * dy)[0] - a(4 * dx, 4 * dy)[0]).abs()
            };
            let (gn, gs, gw, ge) = (grad(0, -1), grad(0, 1), grad(-1, 0), grad(1, 0));
            let v = (gs * est(0, -1) + gn * est(0, 1)) / (gn + gs);
            let h = (ge * est(-1, 0) + gw * est(1, 0)) / (gw + ge);
            g = v + (h - v) * vh;
        }
        [p[0], g, vh, pq]
    });
    // Step 3.
    let s3 = Plane::new(cw, ch, true, |x, y| {
        let a = |dx: i32, dy: i32| s2.at(x + dx, y + dy);
        let cd = |dx: i32, dy: i32| {
            let q = a(dx, dy);
            q[0] - q[1]
        };
        let p = a(0, 0);
        let c = ctx.color(x, y);
        let mut rgb = [0.0, p[1], 0.0];
        if c != 1 {
            let dgrad = |dx: i32, dy: i32| {
                1e-5 + (cd(dx, dy) - cd(-dx, -dy)).abs()
                    + (cd(dx, dy) - cd(3 * dx, 3 * dy)).abs()
                    + (a(0, 0)[1] - a(2 * dx, 2 * dy)[1]).abs()
            };
            let dir = |dx: i32, dy: i32| {
                let g0 = dgrad(-dx, -dy);
                let g1 = dgrad(dx, dy);
                (g1 * cd(-dx, -dy) + g0 * cd(dx, dy)) / (g0 + g1)
            };
            let (pe, qe) = (dir(1, 1), dir(-1, 1));
            let o = p[1] + pe + (qe - pe) * p[3];
            rgb = if c == 0 {
                [p[0], p[1], o]
            } else {
                [o, p[1], p[0]]
            };
        }
        [rgb[0], rgb[1], rgb[2], p[2]]
    });
    // Step 4.
    let mut out = Vec::with_capacity(cw as usize * ch as usize);
    for y in 0..ch as i32 {
        for x in 0..cw as i32 {
            let a = |dx: i32, dy: i32| s3.at(x + dx, y + dy);
            let p = a(0, 0);
            let mut rgb = [p[0], p[1], p[2]];
            if ctx.color(x, y) == 1 {
                let cdc = |dx: i32, dy: i32, k: usize| {
                    let q = a(dx, dy);
                    q[k] - q[1]
                };
                let chan = |k: usize| {
                    let grd = |dx: i32, dy: i32| {
                        1e-5 + (a(0, 0)[1] - a(2 * dx, 2 * dy)[1]).abs()
                            + (cdc(dx, dy, k) - cdc(-dx, -dy, k)).abs()
                            + (cdc(dx, dy, k) - cdc(3 * dx, 3 * dy, k)).abs()
                    };
                    let (gn, gs, gw, ge) = (grd(0, -1), grd(0, 1), grd(-1, 0), grd(1, 0));
                    let v = (gs * cdc(0, -1, k) + gn * cdc(0, 1, k)) / (gn + gs);
                    let h = (ge * cdc(-1, 0, k) + gw * cdc(1, 0, k)) / (gw + ge);
                    p[1] + v + (h - v) * p[3]
                };
                rgb = [chan(0), p[1], chan(2)];
            }
            if let Some(t) = dual {
                let (mut lo, mut hi) = (3.0e38f32, -3.0e38f32);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let mut g = 0.0;
                        for j in -1..=1 {
                            for i in -1..=1 {
                                g += a(dx + i, dy + j)[1];
                            }
                        }
                        lo = lo.min(g);
                        hi = hi.max(g);
                    }
                }
                let k = 4.0 * (hi - lo) / (hi.abs() + lo.abs() + 1e-3);
                let wr = smoothstep(0.5 * t, 1.5 * t, k);
                let sm = bilinear(ctx, &mo, x, y);
                rgb = std::array::from_fn(|i| sm[i] + (rgb[i] - sm[i]) * wr);
            }
            out.push(ctx.finish_full(rgb, x as u32, y as u32));
        }
    }
    out
}

const LINES: [(i32, i32); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];
const STEPS: [i32; 4] = [-2, -1, 1, 2];

fn xtrans(ctx: &Ctx, cw: u32, ch: u32) -> Vec<[f32; 4]> {
    let (w, h) = (cw as i32, ch as i32);
    let cc = |x: i32, y: i32| ctx.color(x.clamp(0, w - 1), y.clamp(0, h - 1));
    let mo = Plane::new(cw, ch, false, |x, y| ctx.prep(x, y, 0));
    let m = |x: i32, y: i32| mo.at(x, y);
    let fallback = |x: i32, y: i32| {
        const DIRS: [(i32, i32, f32); 8] = [
            (0, -1, 2.0),
            (0, 1, 2.0),
            (-1, 0, 2.0),
            (1, 0, 2.0),
            (-1, -1, 1.0),
            (-1, 1, 1.0),
            (1, -1, 1.0),
            (1, 1, 1.0),
        ];
        let (mut sw, mut sv) = (0f32, 0f32);
        for (dy, dx, base) in DIRS {
            if cc(x + dx, y + dy) != 1 {
                continue;
            }
            let nb = m(x + dx, y + dy);
            let g = if cc(x - dx, y - dy) == 1 {
                (nb - m(x - dx, y - dy)).abs()
            } else if cc(x + 2 * dx, y + 2 * dy) == 1 {
                2.0 * (nb - m(x + 2 * dx, y + 2 * dy)).abs()
            } else {
                (nb - m(x, y)).abs()
            };
            let t = 0.125 + g;
            let wgt = base / (t * t);
            sw += wgt;
            sv += wgt * nb;
        }
        if sw > 0.0 { sv / sw } else { m(x, y) }
    };
    let green = Plane::new(cw, ch, false, |x, y| {
        let v0 = m(x, y);
        let mut g = [v0; 4];
        if cc(x, y) == 1 {
            return g;
        }
        let mut fb: Option<f32> = None;
        for (l, (dy, dx)) in LINES.into_iter().enumerate() {
            let mut mask = 0u32;
            for (k, s) in STEPS.into_iter().enumerate() {
                if cc(x + s * dx, y + s * dy) == 1 {
                    mask |= 1 << k;
                }
            }
            let neg = [1, 0].into_iter().find(|&k| mask & (1 << k) != 0);
            let pos = [2, 3].into_iter().find(|&k| mask & (1 << k) != 0);
            g[l] = match (neg, pos) {
                (Some(a), Some(b)) => {
                    let mut v = 0f32;
                    for k in 0..4 {
                        if mask & (1 << k) == 0 {
                            continue;
                        }
                        let xk = STEPS[k] as f32;
                        let mut wt = 1f32;
                        for (j, &sj) in STEPS.iter().enumerate() {
                            if j == k || mask & (1 << j) == 0 {
                                continue;
                            }
                            let xj = sj as f32;
                            wt *= -xj / (xk - xj);
                        }
                        v += wt * m(x + STEPS[k] * dx, y + STEPS[k] * dy);
                    }
                    let va = m(x + STEPS[a] * dx, y + STEPS[a] * dy);
                    let vb = m(x + STEPS[b] * dx, y + STEPS[b] * dy);
                    v.clamp(va.min(vb), va.max(vb))
                }
                _ => *fb.get_or_insert_with(|| fallback(x, y)),
            };
        }
        g
    });
    let rgb = Plane::new(cw, ch, false, |x, y| {
        let c0 = cc(x, y);
        let gi = green.at(x, y);
        let (mut swr, mut sdr, mut swb, mut sdb) = ([0f32; 4], [0f32; 4], [0f32; 4], [0f32; 4]);
        for dy in -2..=2 {
            for dx in -2..=2 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let c = cc(x + dx, y + dy);
                if c == 1 || c == c0 {
                    continue;
                }
                let inv = 1.0 / (dx * dx + dy * dy) as f32;
                let gq = green.at(x + dx, y + dy);
                let mv = m(x + dx, y + dy);
                for l in 0..4 {
                    let s = 1.0 + (gq[l] - gi[l]).abs() * 8.0;
                    let wgt = inv / (s * s);
                    let d = wgt * (mv - gq[l]);
                    if c == 0 {
                        swr[l] += wgt;
                        sdr[l] += d;
                    } else {
                        swb[l] += wgt;
                        sdb[l] += d;
                    }
                }
            }
        }
        let v = m(x, y);
        let mut r: [f32; 4] = std::array::from_fn(|l| {
            if swr[l] > 0.0 {
                gi[l] + sdr[l] / swr[l]
            } else {
                0.0
            }
        });
        let mut b: [f32; 4] = std::array::from_fn(|l| {
            if swb[l] > 0.0 {
                gi[l] + sdb[l] / swb[l]
            } else {
                0.0
            }
        });
        if c0 == 0 {
            r = [v; 4];
        }
        if c0 == 2 {
            b = [v; 4];
        }
        [r, gi, b]
    });
    let pc = |v: f32| if v <= 0.0 { 0.0 } else { v.powf(1.0 / 3.0) };
    let opp = |c: [f32; 3]| {
        let (r, g, b) = (pc(c[0]), pc(c[1]), pc(c[2]));
        [(r + 2.0 * g + b) * 0.25, r - g, b - g]
    };
    let line = |x: i32, y: i32, l: usize| {
        let p = rgb.at(x, y);
        [p[0][l], p[1][l], p[2][l]]
    };
    let drv = Plane::new(cw, ch, false, |x, y| {
        std::array::from_fn::<f32, 4, _>(|l| {
            let (dy, dx) = LINES[l];
            let a = opp(line(x, y, l));
            let b = opp(line(x + dx, y + dy, l));
            let c = opp(line(x - dx, y - dy, l));
            (0..3).map(|k| (2.0 * a[k] - b[k] - c[k]).powi(2)).sum()
        })
    });
    let homo = Plane::new(cw, ch, false, |x, y| {
        let c = drv.at(x, y);
        let tr = 8.0 * c[0].min(c[1]).min(c[2].min(c[3]));
        let mut k = [0f32; 4];
        for dy in -1..=1 {
            for dx in -1..=1 {
                let d = drv.at(x + dx, y + dy);
                for l in 0..4 {
                    if d[l] <= tr {
                        k[l] += 1.0;
                    }
                }
            }
        }
        k
    });
    let hsum = Plane::new(cw, ch, false, |x, y| {
        let mut s = [0f32; 4];
        for dx in -2..=2 {
            let hh = homo.at(x + dx, y);
            for l in 0..4 {
                s[l] += hh[l];
            }
        }
        s
    });
    let mut out = Vec::with_capacity(cw as usize * ch as usize);
    for y in 0..h {
        for x in 0..w {
            let mut s = [0f32; 4];
            for dy in -2..=2 {
                let hh = hsum.at(x, y + dy);
                for l in 0..4 {
                    s[l] += hh[l];
                }
            }
            let best = s[0].max(s[1]).max(s[2].max(s[3]));
            let fl = best - (best / 8.0).floor();
            let (mut acc, mut n) = ([0f32; 3], 0f32);
            for l in 0..4 {
                if s[l] >= fl {
                    let p = line(x, y, l);
                    for k in 0..3 {
                        acc[k] += p[k];
                    }
                    n += 1.0;
                }
            }
            let c = acc.map(|v| v / n.max(1.0));
            out.push(ctx.finish_full(c, x as u32, y as u32));
        }
    }
    out
}

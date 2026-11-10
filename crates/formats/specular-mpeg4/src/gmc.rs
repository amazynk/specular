//! Global motion compensation of S-VOPs (ISO/IEC 14496-2 §7.8): the sprite trajectory's
//! warping points as an affine warp of the reference, sampled bilinearly at
//! 1/(2·accuracy) for each pixel, or as a translation at 1/16 sample when that is all it is;
//! and the vector a GMC macroblock stands for in the prediction of later vectors.

use crate::headers::Vol;
use crate::mc::Frame;

/// The warp of one S-VOP.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sprite {
    /// A translation only (one point, or a warp that reduces to one).
    translation: bool,
    /// warping_accuracy (0..3: half to sixteenth samples).
    acc: u32,
    /// Offsets for luma and chroma (x, y): in 1/(2·2^acc) samples for a translation, else in
    /// those units times 2^16 for the top-left sample.
    offset: [[i64; 2]; 2],
    /// d(source x, y)/d(x, y) times 2^16 (general warps).
    delta: [[i64; 2]; 2],
    /// The shift of a general warp's positions (16 after scaling).
    shift: u32,
}

fn rounded_div(a: i64, b: i64) -> i64 {
    if a > 0 {
        (a + b / 2) / b
    } else {
        (a - b / 2) / b
    }
}

fn rshift(a: i64, b: u32) -> i64 {
    if a > 0 {
        (a + ((1 << b) >> 1)) >> b
    } else {
        (a + ((1 << b) >> 1) - 1) >> b
    }
}

impl Sprite {
    /// The warp of `traj` (du, dv per warping point) for a VOP of the layer `vol`.
    pub fn new(vol: &Vol, traj: [[i32; 2]; 4]) -> Option<Sprite> {
        let acc = u32::from(vol.warping_accuracy);
        let a = 2i64 << acc;
        let rho = 3 - acc as i64;
        let r = 16 / a;
        let (w, h) = (i64::from(vol.width), i64::from(vol.height));
        let vop_ref = [[0, 0], [w, 0], [0, h]];
        let d = traj.map(|p| [i64::from(p[0]), i64::from(p[1])]);
        let mut alpha = 1i64;
        while (1 << alpha) < w {
            alpha += 1;
        }
        let mut beta = 0i64;
        while (1 << beta) < h {
            beta += 1;
        }
        let (w2, h2) = (1i64 << alpha, 1i64 << beta);
        let half = a >> 1;
        let sref = [
            [
                half * (2 * vop_ref[0][0] + d[0][0]),
                half * (2 * vop_ref[0][1] + d[0][1]),
            ],
            [
                half * (2 * vop_ref[1][0] + d[0][0] + d[1][0]),
                half * (2 * vop_ref[1][1] + d[0][1] + d[1][1]),
            ],
            [
                half * (2 * vop_ref[2][0] + d[0][0] + d[2][0]),
                half * (2 * vop_ref[2][1] + d[0][1] + d[2][1]),
            ],
        ];
        // The virtual sprite points at distances of a power of two (§7.8.4).
        let vref = [
            [
                16 * (vop_ref[0][0] + w2)
                    + rounded_div(
                        (w - w2) * (r * sref[0][0] - 16 * vop_ref[0][0])
                            + w2 * (r * sref[1][0] - 16 * vop_ref[1][0]),
                        w,
                    ),
                16 * vop_ref[0][1]
                    + rounded_div(
                        (w - w2) * (r * sref[0][1] - 16 * vop_ref[0][1])
                            + w2 * (r * sref[1][1] - 16 * vop_ref[1][1]),
                        w,
                    ),
            ],
            [
                16 * vop_ref[0][0]
                    + rounded_div(
                        (h - h2) * (r * sref[0][0] - 16 * vop_ref[0][0])
                            + h2 * (r * sref[2][0] - 16 * vop_ref[2][0]),
                        h,
                    ),
                16 * (vop_ref[0][1] + h2)
                    + rounded_div(
                        (h - h2) * (r * sref[0][1] - 16 * vop_ref[0][1])
                            + h2 * (r * sref[2][1] - 16 * vop_ref[2][1]),
                        h,
                    ),
            ],
        ];
        let mut offset = [[0i64; 2]; 2];
        let mut delta = [[a, 0], [0, a]];
        let mut shift = [0u32; 2];
        match vol.warping_points {
            0 => {}
            1 => {
                offset[0] = [
                    sref[0][0] - a * vop_ref[0][0],
                    sref[0][1] - a * vop_ref[0][1],
                ];
                offset[1] = [
                    ((sref[0][0] >> 1) | (sref[0][0] & 1)) - a * (vop_ref[0][0] / 2),
                    ((sref[0][1] >> 1) | (sref[0][1] & 1)) - a * (vop_ref[0][1] / 2),
                ];
            }
            2 => {
                let s = alpha + rho;
                let dx0 = -r * sref[0][0] + vref[0][0];
                let dy0 = r * sref[0][1] - vref[0][1];
                let dx1 = -r * sref[0][1] + vref[0][1];
                offset[0] = [
                    sref[0][0] * (1 << s)
                        + dx0 * -vop_ref[0][0]
                        + dy0 * -vop_ref[0][1]
                        + (1 << (s - 1)),
                    sref[0][1] * (1 << s)
                        + dx1 * -vop_ref[0][0]
                        + dx0 * -vop_ref[0][1]
                        + (1 << (s - 1)),
                ];
                offset[1] = [
                    dx0 * (-2 * vop_ref[0][0] + 1)
                        + dy0 * (-2 * vop_ref[0][1] + 1)
                        + 2 * w2 * r * sref[0][0]
                        - 16 * w2
                        + (1 << (s + 1)),
                    dx1 * (-2 * vop_ref[0][0] + 1)
                        + dx0 * (-2 * vop_ref[0][1] + 1)
                        + 2 * w2 * r * sref[0][1]
                        - 16 * w2
                        + (1 << (s + 1)),
                ];
                delta = [[dx0, dy0], [dx1, dx0]];
                shift = [s as u32, s as u32 + 2];
            }
            _ => {
                let min_ab = alpha.min(beta);
                let (w3, h3) = (w2 >> min_ab, h2 >> min_ab);
                let s = alpha + beta + rho - min_ab;
                let ex = -r * sref[0][0] + vref[0][0];
                let ey = -r * sref[0][0] + vref[1][0];
                let fx = -r * sref[0][1] + vref[0][1];
                let fy = -r * sref[0][1] + vref[1][1];
                offset[0] = [
                    sref[0][0] * (1 << s)
                        + ex * h3 * -vop_ref[0][0]
                        + ey * w3 * -vop_ref[0][1]
                        + (1 << (s - 1)),
                    sref[0][1] * (1 << s)
                        + fx * h3 * -vop_ref[0][0]
                        + fy * w3 * -vop_ref[0][1]
                        + (1 << (s - 1)),
                ];
                offset[1] = [
                    ex * h3 * (-2 * vop_ref[0][0] + 1)
                        + ey * w3 * (-2 * vop_ref[0][1] + 1)
                        + 2 * w2 * h3 * r * sref[0][0]
                        - 16 * w2 * h3
                        + (1 << (s + 1)),
                    fx * h3 * (-2 * vop_ref[0][0] + 1)
                        + fy * w3 * (-2 * vop_ref[0][1] + 1)
                        + 2 * w2 * h3 * r * sref[0][1]
                        - 16 * w2 * h3
                        + (1 << (s + 1)),
                ];
                delta = [[ex * h3, ey * w3], [fx * h3, fy * w3]];
                shift = [s as u32, s as u32 + 2];
            }
        }
        // A warp that is a translation after all.
        if delta[0][0] == a << shift[0]
            && delta[0][1] == 0
            && delta[1][0] == 0
            && delta[1][1] == a << shift[0]
        {
            for k in 0..2 {
                offset[0][k] >>= shift[0];
                offset[1][k] >>= shift[1];
            }
            return Some(Sprite {
                translation: true,
                acc,
                offset,
                delta: [[a, 0], [0, a]],
                shift: 0,
            });
        }
        // Positions at 1/2^16: scale up to a common shift of 16.
        let sy = 16i64 - i64::from(shift[0]);
        let sc = 16i64 - i64::from(shift[1]);
        if sy < 0 || sc < 0 {
            return None;
        }
        for k in 0..2 {
            offset[0][k] <<= sy;
            offset[1][k] <<= sc;
            delta[0][k] <<= sy;
            delta[1][k] <<= sy;
        }
        let limit = i64::from(i32::MAX);
        let fits = offset
            .iter()
            .flatten()
            .chain(delta.iter().flatten())
            .all(|v| v.abs() < limit)
            && (0..2).all(|i| {
                (offset[0][i] + delta[i][0] * (w + 16) + delta[i][1] * (h + 16)).abs() < limit
            });
        if !fits {
            return None;
        }
        Some(Sprite {
            translation: false,
            acc,
            offset,
            delta,
            shift: 16,
        })
    }

    /// The prediction of macroblock (`mbx`, `mby`) from `f`.
    #[allow(clippy::too_many_arguments)]
    pub fn predict(
        &self,
        f: &Frame,
        mbx: usize,
        mby: usize,
        rnd: bool,
        y: &mut [u8; 256],
        cb: &mut [u8; 64],
        cr: &mut [u8; 64],
    ) {
        let no_rnd = i64::from(rnd);
        if self.translation {
            for (c, dst, n) in [
                (0usize, &mut y[..], 16usize),
                (1, &mut cb[..], 8),
                (2, &mut cr[..], 8),
            ] {
                let o = self.offset[usize::from(c != 0)];
                let v = f.view(c, None);
                let base = [n as i64 * mbx as i64, n as i64 * mby as i64];
                let mut src = [0i64; 2];
                let mut frac = [0i64; 2];
                for k in 0..2 {
                    let lim = if k == 0 { v.w } else { v.h };
                    let s = (base[k] + (o[k] >> (self.acc + 1))).clamp(-(n as i64), i64::from(lim));
                    src[k] = s;
                    frac[k] = if s == i64::from(lim) {
                        0
                    } else {
                        (o[k] << (3 - self.acc)) & 15
                    };
                }
                let (a, b, cc, d) = (
                    (16 - frac[0]) * (16 - frac[1]),
                    frac[0] * (16 - frac[1]),
                    (16 - frac[0]) * frac[1],
                    frac[0] * frac[1],
                );
                let rounder = 128 - no_rnd;
                for i in 0..n {
                    for j in 0..n {
                        let (x0, y0) = (src[0] as i32 + j as i32, src[1] as i32 + i as i32);
                        let p = |dx: i32, dy: i32| i64::from(v.at(x0 + dx, y0 + dy));
                        let s = a * p(0, 0) + b * p(1, 0) + cc * p(0, 1) + d * p(1, 1) + rounder;
                        dst[i * n + j] = (s >> 8) as u8;
                    }
                }
            }
            return;
        }
        let s = 1i64 << (self.acc + 1);
        let shift = self.acc + 1;
        let r = (1i64 << (2 * self.acc + 1)) - no_rnd;
        for (c, dst, n) in [
            (0usize, &mut y[..], 16usize),
            (1, &mut cb[..], 8),
            (2, &mut cr[..], 8),
        ] {
            let v = f.view(c, None);
            let o = self.offset[usize::from(c != 0)];
            let (bx, by) = ((n * mbx) as i64, (n * mby) as i64);
            let ox = o[0] + self.delta[0][0] * bx + self.delta[0][1] * by;
            let oy = o[1] + self.delta[1][0] * bx + self.delta[1][1] * by;
            for i in 0..n as i64 {
                for j in 0..n as i64 {
                    let vx = ox + self.delta[0][0] * j + self.delta[0][1] * i;
                    let vy = oy + self.delta[1][0] * j + self.delta[1][1] * i;
                    let (sx, sy) = (vx >> self.shift, vy >> self.shift);
                    let (fx, fy) = (sx & (s - 1), sy & (s - 1));
                    let (sx, sy) = ((sx >> shift) as i32, (sy >> shift) as i32);
                    let p = |dx: i32, dy: i32| i64::from(v.at(sx + dx, sy + dy));
                    let val = ((p(0, 0) * (s - fx) + p(1, 0) * fx) * (s - fy)
                        + (p(0, 1) * (s - fx) + p(1, 1) * fx) * fy
                        + r)
                        >> (2 * shift);
                    dst[(i * n as i64 + j) as usize] = val.clamp(0, 255) as u8;
                }
            }
        }
    }

    /// The vector of a GMC macroblock for vector prediction (the warp's average over it, in
    /// half or quarter samples), limited to the VOP's f_code range.
    pub fn amv(&self, mbx: usize, mby: usize, qpel: bool, fcode: u8) -> [i32; 2] {
        let len = 1i64 << (u32::from(fcode) + 4);
        let q = u32::from(qpel);
        let mut out = [0i32; 2];
        for (n, o) in out.iter_mut().enumerate() {
            let sum = if self.translation {
                rshift(self.offset[0][n] * (1 << q), self.acc)
            } else {
                let mut dx = self.delta[n][0];
                let mut dy = self.delta[n][1];
                let shift = self.shift;
                if n == 1 {
                    dy -= 1 << (shift + self.acc + 1);
                } else {
                    dx -= 1 << (shift + self.acc + 1);
                }
                let mb_v = self.offset[0][n] + dx * 16 * mbx as i64 + dy * 16 * mby as i64;
                let mut sum = 0i64;
                for yy in 0..16i64 {
                    let mut v = mb_v + dy * yy;
                    for _ in 0..16 {
                        sum += v >> shift;
                        v += dx;
                    }
                }
                rshift(sum, self.acc + 8 - q)
            };
            *o = sum.clamp(-len, len - 1) as i32;
        }
        out
    }
}

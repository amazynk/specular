//! Intra sample prediction (8.4.5.2): reference filtering, planar, DC, angular with wide
//! angles, PDPC, MIP ([`mip`]) and CCLM ([`cclm`]).
//!
//! Reference samples come in two arrays indexed from the corner: `top[ 1 + r + x ]` is
//! `p[ x ][ −1 − r ]` for x = −1 − r..refW − 1, `left[ 1 + r + y ]` is `p[ −1 − r ][ y ]`,
//! where `r` is the reference line index; both start with the shared corner sample.

pub mod cclm;
pub mod mip;

pub const PLANAR: i32 = 0;
pub const DC: i32 = 1;
pub const LT_CCLM: i32 = 81;
pub const L_CCLM: i32 = 82;
pub const T_CCLM: i32 = 83;

/// intraPredAngle for modes −14..80 (Table 24), indexed by mode + 14.
const ANGLE: [i32; 95] = [
    512, 341, 256, 171, 128, 102, 86, 73, 64, 57, 51, 45, 39, 35, 0, 0, 32, 29, 26, 23, 20, 18, 16,
    14, 12, 10, 8, 6, 4, 3, 2, 1, 0, -1, -2, -3, -4, -6, -8, -10, -12, -14, -16, -18, -20, -23,
    -26, -29, -32, -29, -26, -23, -20, -18, -16, -14, -12, -10, -8, -6, -4, -3, -2, -1, 0, 1, 2, 3,
    4, 6, 8, 10, 12, 14, 16, 18, 20, 23, 26, 29, 32, 35, 39, 45, 51, 57, 64, 73, 86, 102, 128, 171,
    256, 341, 512,
];

pub fn angle(mode: i32) -> i32 {
    ANGLE[(mode + 14) as usize]
}

/// invAngle = Round( 512 * 32 / intraPredAngle ).
pub fn inv_angle(a: i32) -> i32 {
    let num = 512 * 32;
    let q = (2 * num + a.abs()) / (2 * a.abs());
    if a < 0 { -q } else { q }
}

/// fC and fG (Table 25).
const FC: [[i32; 4]; 32] = [
    [0, 64, 0, 0],
    [-1, 63, 2, 0],
    [-2, 62, 4, 0],
    [-2, 60, 7, -1],
    [-2, 58, 10, -2],
    [-3, 57, 12, -2],
    [-4, 56, 14, -2],
    [-4, 55, 15, -2],
    [-4, 54, 16, -2],
    [-5, 53, 18, -2],
    [-6, 52, 20, -2],
    [-6, 49, 24, -3],
    [-6, 46, 28, -4],
    [-5, 44, 29, -4],
    [-4, 42, 30, -4],
    [-4, 39, 33, -4],
    [-4, 36, 36, -4],
    [-4, 33, 39, -4],
    [-4, 30, 42, -4],
    [-4, 29, 44, -5],
    [-4, 28, 46, -6],
    [-3, 24, 49, -6],
    [-2, 20, 52, -6],
    [-2, 18, 53, -5],
    [-2, 16, 54, -4],
    [-2, 15, 55, -4],
    [-2, 14, 56, -4],
    [-2, 12, 57, -3],
    [-2, 10, 58, -2],
    [-1, 7, 60, -2],
    [0, 4, 62, -2],
    [0, 2, 63, -1],
];

const FG: [[i32; 4]; 32] = [
    [16, 32, 16, 0],
    [16, 32, 16, 0],
    [15, 31, 17, 1],
    [15, 31, 17, 1],
    [14, 30, 18, 2],
    [14, 30, 18, 2],
    [13, 29, 19, 3],
    [13, 29, 19, 3],
    [12, 28, 20, 4],
    [12, 28, 20, 4],
    [11, 27, 21, 5],
    [11, 27, 21, 5],
    [10, 26, 22, 6],
    [10, 26, 22, 6],
    [9, 25, 23, 7],
    [9, 25, 23, 7],
    [8, 24, 24, 8],
    [8, 24, 24, 8],
    [7, 23, 25, 9],
    [7, 23, 25, 9],
    [6, 22, 26, 10],
    [6, 22, 26, 10],
    [5, 21, 27, 11],
    [5, 21, 27, 11],
    [4, 20, 28, 12],
    [4, 20, 28, 12],
    [3, 19, 29, 13],
    [3, 19, 29, 13],
    [2, 18, 30, 14],
    [2, 18, 30, 14],
    [1, 17, 31, 15],
    [1, 17, 31, 15],
];

fn log2(v: usize) -> u32 {
    v.trailing_zeros()
}

/// Wide angle mode mapping (8.4.5.2.7); `nw x nh` is the block the mapping looks at.
pub fn wide_angle(mode: i32, nw: usize, nh: usize) -> i32 {
    if nw == nh || !(2..=66).contains(&mode) {
        return mode;
    }
    let r = (log2(nw) as i32 - log2(nh) as i32).abs();
    if nw > nh && mode < if r > 1 { 8 + 2 * r } else { 8 } {
        mode + 65
    } else if nh > nw && mode > if r > 1 { 60 - 2 * r } else { 60 } {
        mode - 67
    } else {
        mode
    }
}

/// refFilterFlag of 8.4.5.2.6.
pub fn ref_filter_flag(mode: i32) -> bool {
    matches!(
        mode,
        0 | -14 | -12 | -10 | -6 | 2 | 34 | 66 | 72 | 76 | 78 | 80
    )
}

/// Reference sample filtering (8.4.5.2.10) for refIdx 0: `top` has refW + 1 and `left`
/// refH + 1 samples (corner first).
pub fn filter_refs(top: &mut [i32], left: &mut [i32]) {
    let rw = top.len() - 1;
    let rh = left.len() - 1;
    let t: Vec<i32> = top.to_vec();
    let l: Vec<i32> = left.to_vec();
    let corner = (l[1] + 2 * t[0] + t[1] + 2) >> 2;
    for y in 0..rh - 1 {
        left[1 + y] = (l[2 + y] + 2 * l[1 + y] + l[y] + 2) >> 2;
    }
    for x in 0..rw - 1 {
        top[1 + x] = (t[x] + 2 * t[1 + x] + t[2 + x] + 2) >> 2;
    }
    top[0] = corner;
    left[0] = corner;
}

/// INTRA_PLANAR (8.4.5.2.11).
pub fn planar(top: &[i32], left: &[i32], w: usize, h: usize, out: &mut [i32]) {
    let (lw, lh) = (log2(w), log2(h));
    let tr = top[1 + w];
    let bl = left[1 + h];
    for y in 0..h {
        for x in 0..w {
            let v = (((h - 1 - y) as i32 * top[1 + x] + (y + 1) as i32 * bl) << lw)
                + (((w - 1 - x) as i32 * left[1 + y] + (x + 1) as i32 * tr) << lh);
            out[y * w + x] = (v + (w * h) as i32) >> (lw + lh + 1);
        }
    }
}

/// INTRA_DC (8.4.5.2.12); `r` the reference line.
pub fn dc(top: &[i32], left: &[i32], r: usize, w: usize, h: usize, out: &mut [i32]) {
    let st: i32 = top[1 + r..1 + r + w].iter().sum();
    let sl: i32 = left[1 + r..1 + r + h].iter().sum();
    let v = if w == h {
        (st + sl + w as i32) >> (log2(w) + 1)
    } else if w > h {
        (st + (w as i32 >> 1)) >> log2(w)
    } else {
        (sl + (h as i32 >> 1)) >> log2(h)
    };
    out[..w * h].fill(v);
}

/// INTRA_ANGULAR (8.4.5.2.13). `top`/`left` cover refW / refH plus the corner for line `r`.
#[allow(clippy::too_many_arguments)]
pub fn angular(
    mode: i32,
    r: usize,
    top: &[i32],
    left: &[i32],
    w: usize,
    h: usize,
    filter: bool,
    luma: bool,
    max: i32,
    out: &mut [i32],
) {
    let a = angle(mode);
    let ri = r as i32;
    let vertical = mode >= 34;
    // main = the reference row along the prediction direction; side = the other one.
    let (main, side, nmain, nside) = if vertical {
        (top, left, w, h)
    } else {
        (left, top, h, w)
    };
    let ref_main = main.len() as i32 - 1 - ri; // refW or refH
    // ref[ x ] for x = −nside..ref_main + r + extra; offset `o` so ref[x] = buf[x + o].
    let extra = (nmain / nside).max(1) as i32 * ri + 2;
    let o = nside as i32;
    let len = (o + ref_main + ri + extra + 1) as usize;
    let mut buf = vec![0i32; len];
    for x in 0..=(nmain as i32 + ri + 1) {
        buf[(x + o) as usize] = main[x as usize];
    }
    if a < 0 {
        let inv = inv_angle(a);
        for x in -(nside as i32)..0 {
            let k = ((x * inv + 256) >> 9).min(nside as i32);
            // p[ −1 − r ][ −1 − r + k ] along the side.
            buf[(x + o) as usize] = side[k as usize];
        }
    } else {
        for x in (nmain as i32 + 2 + ri)..=(ref_main + ri) {
            buf[(x + o) as usize] = main[x as usize];
        }
        let last = main[(ref_main + ri) as usize];
        for x in 1..=extra {
            buf[(ref_main + ri + x + o) as usize] = last;
        }
    }
    for j in 0..nside {
        let pos = (j as i32 + 1 + ri) * a;
        let idx = (pos >> 5) + ri;
        let fact = (pos & 31) as usize;
        for i in 0..nmain {
            let b = (i as i32 + idx + o) as usize;
            let v = if luma {
                let f = if filter { &FG[fact] } else { &FC[fact] };
                let s = f[0] * buf[b] + f[1] * buf[b + 1] + f[2] * buf[b + 2] + f[3] * buf[b + 3];
                ((s + 32) >> 6).clamp(0, max)
            } else if fact != 0 {
                ((32 - fact as i32) * buf[b + 1] + fact as i32 * buf[b + 2] + 16) >> 5
            } else {
                buf[b + 1]
            };
            if vertical {
                out[j * w + i] = v;
            } else {
                out[i * w + j] = v;
            }
        }
    }
}

/// filterFlag of 8.4.5.2.13 (refFilterFlag 0, refIdx 0, no ISP).
pub fn angular_filter_flag(mode: i32, w: usize, h: usize) -> bool {
    let nts = (log2(w) + log2(h)) >> 1;
    let thres = match nts {
        2 => 24,
        3 => 14,
        4 => 2,
        _ => 0,
    };
    let d = (mode - 50).abs().min((mode - 18).abs());
    d > thres
}

/// Position-dependent prediction filtering (8.4.5.2.15), refIdx 0.
pub fn pdpc(mode: i32, w: usize, h: usize, top: &[i32], left: &[i32], max: i32, out: &mut [i32]) {
    let (lw, lh) = (log2(w) as i32, log2(h) as i32);
    let fl = |v: i32| 31 - (v as u32).leading_zeros() as i32;
    let n_scale = if mode > 50 {
        let inv = inv_angle(angle(mode));
        (lh - fl(3 * inv - 2) + 8).min(2)
    } else if mode < 18 && mode != PLANAR && mode != DC {
        let inv = inv_angle(angle(mode));
        (lw - fl(3 * inv - 2) + 8).min(2)
    } else {
        (lw + lh - 2) >> 2
    };
    let tl = top[0];
    let main = |x: usize| top[1 + x];
    let side = |y: usize| left[1 + y];
    let wgt = |v: usize| 32 >> ((((v as i32) << 1) >> n_scale).min(31));
    if mode == PLANAR || mode == DC {
        for y in 0..h {
            let wt = wgt(y);
            for x in 0..w {
                let wl = wgt(x);
                let p = out[y * w + x];
                let v = side(y) * wl + main(x) * wt + (64 - wl - wt) * p + 32;
                out[y * w + x] = (v >> 6).clamp(0, max);
            }
        }
    } else if mode == 18 || mode == 50 {
        for y in 0..h {
            for x in 0..w {
                let p = out[y * w + x];
                let (rl, rt) = (side(y) - tl + p, main(x) - tl + p);
                let wt = if mode == 18 { wgt(y) } else { 0 };
                let wl = if mode == 50 { wgt(x) } else { 0 };
                let v = rl * wl + rt * wt + (64 - wl - wt) * p + 32;
                out[y * w + x] = (v >> 6).clamp(0, max);
            }
        }
    } else if mode < 18 && n_scale >= 0 {
        let inv = inv_angle(angle(mode));
        for y in 0..h {
            let dxi = (((y as i32) + 1) * inv + 256) >> 9;
            let wt = wgt(y);
            for x in 0..w {
                let p = out[y * w + x];
                let rt = if (y as i32) < (3 << n_scale) {
                    main((x as i32 + dxi) as usize)
                } else {
                    0
                };
                let v = rt * wt + (64 - wt) * p + 32;
                out[y * w + x] = (v >> 6).clamp(0, max);
            }
        }
    } else if mode > 50 && n_scale >= 0 {
        let inv = inv_angle(angle(mode));
        for x in 0..w {
            let dyi = (((x as i32) + 1) * inv + 256) >> 9;
            let wl = wgt(x);
            for y in 0..h {
                let p = out[y * w + x];
                let rl = if (x as i32) < (3 << n_scale) {
                    side((y as i32 + dyi) as usize)
                } else {
                    0
                };
                let v = rl * wl + (64 - wl) * p + 32;
                out[y * w + x] = (v >> 6).clamp(0, max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_and_wide_angle_mapping() {
        assert_eq!(angle(-14), 512);
        assert_eq!(angle(2), 32);
        assert_eq!(angle(18), 0);
        assert_eq!(angle(34), -32);
        assert_eq!(angle(50), 0);
        assert_eq!(angle(80), 512);
        assert_eq!(inv_angle(32), 512);
        assert_eq!(inv_angle(-32), -512);
        assert_eq!(inv_angle(29), 565);
        assert_eq!(wide_angle(2, 16, 4), 67);
        assert_eq!(wide_angle(66, 4, 16), -1);
        assert_eq!(wide_angle(30, 16, 4), 30);
    }

    #[test]
    fn dc_and_planar_of_flat_references() {
        let top = vec![100; 9];
        let left = vec![100; 9];
        let mut out = vec![0; 16];
        dc(&top, &left, 0, 4, 4, &mut out);
        assert!(out.iter().all(|&v| v == 100));
        planar(&top, &left, 4, 4, &mut out);
        assert!(out.iter().all(|&v| v == 100));
        angular(34, 0, &top, &left, 4, 4, false, true, 1023, &mut out);
        assert!(out.iter().all(|&v| v == 100));
    }
}

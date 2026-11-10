//! Intra prediction (8.3.1.2, 8.3.3, 8.3.4).

/// Neighbouring samples of a 4×4 block: `top[0..8]` is p[0..7, −1] (4..8 replicated from
/// p[3, −1] when the top-right is unavailable), `left[0..4]` is p[−1, 0..3], `tl` p[−1, −1].
#[derive(Clone, Copy, Debug, Default)]
pub struct Edge4 {
    pub top: [u8; 8],
    pub left: [u8; 4],
    pub tl: u8,
    pub has_top: bool,
    pub has_left: bool,
    pub has_tl: bool,
}

/// Is Intra_4x4 mode `m` usable with these neighbours?
pub fn mode4_ok(m: u8, e: &Edge4) -> bool {
    match m {
        0 | 3 | 7 => e.has_top,
        1 | 8 => e.has_left,
        2 => true,
        _ => e.has_top && e.has_left && e.has_tl,
    }
}

#[inline]
fn avg2(a: u8, b: u8) -> u8 {
    ((u16::from(a) + u16::from(b) + 1) >> 1) as u8
}
#[inline]
fn avg3(a: u8, b: u8, c: u8) -> u8 {
    ((u16::from(a) + 2 * u16::from(b) + u16::from(c) + 2) >> 2) as u8
}

/// Intra_4x4 prediction, raster order.
pub fn pred4(m: u8, e: &Edge4) -> [u8; 16] {
    let mut p = [0u8; 16];
    // Unified sample access: t(x) = p[x, −1] for x in −1..8, l(y) = p[−1, y] for y in −1..4.
    let t = |x: i32| -> u8 { if x < 0 { e.tl } else { e.top[x as usize] } };
    let l = |y: i32| -> u8 { if y < 0 { e.tl } else { e.left[y as usize] } };
    for y in 0..4i32 {
        for x in 0..4i32 {
            let v = match m {
                0 => t(x),
                1 => l(y),
                2 => {
                    let st: u32 = e.top[..4].iter().map(|&v| u32::from(v)).sum();
                    let sl: u32 = e.left.iter().map(|&v| u32::from(v)).sum();
                    (match (e.has_top, e.has_left) {
                        (true, true) => (st + sl + 4) >> 3,
                        (false, true) => (sl + 2) >> 2,
                        (true, false) => (st + 2) >> 2,
                        _ => 128,
                    }) as u8
                }
                3 => {
                    if x == 3 && y == 3 {
                        ((u16::from(t(6)) + 3 * u16::from(t(7)) + 2) >> 2) as u8
                    } else {
                        avg3(t(x + y), t(x + y + 1), t(x + y + 2))
                    }
                }
                4 => {
                    if x > y {
                        avg3(t(x - y - 2), t(x - y - 1), t(x - y))
                    } else if x < y {
                        avg3(l(y - x - 2), l(y - x - 1), l(y - x))
                    } else {
                        avg3(t(0), e.tl, l(0))
                    }
                }
                5 => {
                    let z = 2 * x - y;
                    if z >= 0 && z % 2 == 0 {
                        avg2(t(x - (y >> 1) - 1), t(x - (y >> 1)))
                    } else if z >= 0 {
                        avg3(t(x - (y >> 1) - 2), t(x - (y >> 1) - 1), t(x - (y >> 1)))
                    } else if z == -1 {
                        avg3(l(0), e.tl, t(0))
                    } else {
                        avg3(l(y - 1), l(y - 2), l(y - 3))
                    }
                }
                6 => {
                    let z = 2 * y - x;
                    if z >= 0 && z % 2 == 0 {
                        avg2(l(y - (x >> 1) - 1), l(y - (x >> 1)))
                    } else if z >= 0 {
                        avg3(l(y - (x >> 1) - 2), l(y - (x >> 1) - 1), l(y - (x >> 1)))
                    } else if z == -1 {
                        avg3(l(0), e.tl, t(0))
                    } else {
                        avg3(t(x - 1), t(x - 2), t(x - 3))
                    }
                }
                7 => {
                    if y % 2 == 0 {
                        avg2(t(x + (y >> 1)), t(x + (y >> 1) + 1))
                    } else {
                        avg3(t(x + (y >> 1)), t(x + (y >> 1) + 1), t(x + (y >> 1) + 2))
                    }
                }
                _ => {
                    let z = x + 2 * y;
                    if z < 5 && z % 2 == 0 {
                        avg2(l(y + (x >> 1)), l(y + (x >> 1) + 1))
                    } else if z < 5 {
                        avg3(l(y + (x >> 1)), l(y + (x >> 1) + 1), l(y + (x >> 1) + 2))
                    } else if z == 5 {
                        ((u16::from(l(2)) + 3 * u16::from(l(3)) + 2) >> 2) as u8
                    } else {
                        l(3)
                    }
                }
            };
            p[(y * 4 + x) as usize] = v;
        }
    }
    p
}

/// Neighbours of a 16×16 luma or 8×8 chroma block.
#[derive(Clone, Copy, Debug)]
pub struct EdgeN<const N: usize> {
    pub top: [u8; N],
    pub left: [u8; N],
    pub tl: u8,
    pub has_top: bool,
    pub has_left: bool,
    pub has_tl: bool,
}

/// Is Intra_16x16 mode `m` (0 V, 1 H, 2 DC, 3 plane) usable?
pub fn mode16_ok(m: u8, e: &EdgeN<16>) -> bool {
    match m {
        0 => e.has_top,
        1 => e.has_left,
        2 => true,
        _ => e.has_top && e.has_left && e.has_tl,
    }
}

/// Intra_16x16 prediction into `out` (raster 16×16).
pub fn pred16(m: u8, e: &EdgeN<16>, out: &mut [u8; 256]) {
    match m {
        0 => {
            for r in out.chunks_exact_mut(16) {
                r.copy_from_slice(&e.top);
            }
        }
        1 => {
            for (r, &v) in out.chunks_exact_mut(16).zip(&e.left) {
                r.fill(v);
            }
        }
        2 => {
            let st: u32 = e.top.iter().map(|&v| u32::from(v)).sum();
            let sl: u32 = e.left.iter().map(|&v| u32::from(v)).sum();
            let v = match (e.has_top, e.has_left) {
                (true, true) => (st + sl + 16) >> 5,
                (false, true) => (sl + 8) >> 4,
                (true, false) => (st + 8) >> 4,
                _ => 128,
            };
            out.fill(v as u8);
        }
        _ => {
            let t = |x: i32| -> i32 {
                if x < 0 {
                    i32::from(e.tl)
                } else {
                    i32::from(e.top[x as usize])
                }
            };
            let l = |y: i32| -> i32 {
                if y < 0 {
                    i32::from(e.tl)
                } else {
                    i32::from(e.left[y as usize])
                }
            };
            let mut h = 0;
            let mut v = 0;
            for k in 0..8 {
                h += (k + 1) * (t(8 + k) - t(6 - k));
                v += (k + 1) * (l(8 + k) - l(6 - k));
            }
            let a = 16 * (l(15) + t(15));
            let b = (5 * h + 32) >> 6;
            let c = (5 * v + 32) >> 6;
            for y in 0..16 {
                for x in 0..16 {
                    out[(y * 16 + x) as usize] =
                        ((a + b * (x - 7) + c * (y - 7) + 16) >> 5).clamp(0, 255) as u8;
                }
            }
        }
    }
}

/// Is intra chroma mode `m` (0 DC, 1 H, 2 V, 3 plane) usable?
pub fn mode_c_ok(m: u8, e: &EdgeN<8>) -> bool {
    match m {
        0 => true,
        1 => e.has_left,
        2 => e.has_top,
        _ => e.has_top && e.has_left && e.has_tl,
    }
}

/// Intra chroma prediction of one 8×8 component (4:2:0).
pub fn pred_chroma(m: u8, e: &EdgeN<8>, out: &mut [u8; 64]) {
    match m {
        0 => {
            for by in 0..2 {
                for bx in 0..2 {
                    let st: u32 = e.top[bx * 4..bx * 4 + 4]
                        .iter()
                        .map(|&v| u32::from(v))
                        .sum();
                    let sl: u32 = e.left[by * 4..by * 4 + 4]
                        .iter()
                        .map(|&v| u32::from(v))
                        .sum();
                    let both = (st + sl + 4) >> 3;
                    let top = (st + 2) >> 2;
                    let left = (sl + 2) >> 2;
                    let v = match (bx, by) {
                        (1, 0) => {
                            if e.has_top {
                                top
                            } else if e.has_left {
                                left
                            } else {
                                128
                            }
                        }
                        (0, 1) => {
                            if e.has_left {
                                left
                            } else if e.has_top {
                                top
                            } else {
                                128
                            }
                        }
                        _ => match (e.has_top, e.has_left) {
                            (true, true) => both,
                            (false, true) => left,
                            (true, false) => top,
                            _ => 128,
                        },
                    };
                    for y in 0..4 {
                        let r = (by * 4 + y) * 8 + bx * 4;
                        out[r..r + 4].fill(v as u8);
                    }
                }
            }
        }
        1 => {
            for (r, &v) in out.chunks_exact_mut(8).zip(&e.left) {
                r.fill(v);
            }
        }
        2 => {
            for r in out.chunks_exact_mut(8) {
                r.copy_from_slice(&e.top);
            }
        }
        _ => {
            let t = |x: i32| -> i32 {
                if x < 0 {
                    i32::from(e.tl)
                } else {
                    i32::from(e.top[x as usize])
                }
            };
            let l = |y: i32| -> i32 {
                if y < 0 {
                    i32::from(e.tl)
                } else {
                    i32::from(e.left[y as usize])
                }
            };
            let mut h = 0;
            let mut v = 0;
            for k in 0..4 {
                h += (k + 1) * (t(4 + k) - t(2 - k));
                v += (k + 1) * (l(4 + k) - l(2 - k));
            }
            let a = 16 * (l(7) + t(7));
            let b = (34 * h + 32) >> 6;
            let c = (34 * v + 32) >> 6;
            for y in 0..8 {
                for x in 0..8 {
                    out[(y * 8 + x) as usize] =
                        ((a + b * (x - 3) + c * (y - 3) + 16) >> 5).clamp(0, 255) as u8;
                }
            }
        }
    }
}

/// Neighbouring samples of an 8×8 luma block (Intra_8x8): `top[0..16]` is p[0..15, −1] (8..16
/// replicated from p[7, −1] when the top-right is unavailable), `left` p[−1, 0..7], `tl`
/// p[−1, −1]; unfiltered.
#[derive(Clone, Copy, Debug, Default)]
pub struct Edge8 {
    pub top: [u8; 16],
    pub left: [u8; 8],
    pub tl: u8,
    pub has_top: bool,
    pub has_left: bool,
    pub has_tl: bool,
}

/// Is Intra_8x8 mode `m` usable with these neighbours? (The same rule as Intra_4x4.)
pub fn mode8_ok(m: u8, e: &Edge8) -> bool {
    match m {
        0 | 3 | 7 => e.has_top,
        1 | 8 => e.has_left,
        2 => true,
        _ => e.has_top && e.has_left && e.has_tl,
    }
}

/// Filtered reference samples of an 8×8 block (8.3.2.2.1): top (16), left (8), top-left.
pub struct Filtered8 {
    pub top: [u8; 16],
    pub left: [u8; 8],
    pub tl: u8,
}

#[inline]
fn w31(a: u8, b: u8) -> u8 {
    ((3 * u16::from(a) + u16::from(b) + 2) >> 2) as u8
}

/// The reference sample filtering of 8.3.2.2.1.
pub fn filter8(e: &Edge8) -> Filtered8 {
    let mut f = Filtered8 {
        top: [0; 16],
        left: [0; 8],
        tl: 0,
    };
    if e.has_top {
        let p = &e.top;
        f.top[0] = if e.has_tl {
            avg3(e.tl, p[0], p[1])
        } else {
            w31(p[0], p[1])
        };
        for x in 1..15 {
            f.top[x] = avg3(p[x - 1], p[x], p[x + 1]);
        }
        f.top[15] = w31(p[15], p[14]);
    }
    if e.has_tl {
        f.tl = match (e.has_top, e.has_left) {
            (true, true) => avg3(e.top[0], e.tl, e.left[0]),
            (true, false) => w31(e.tl, e.top[0]),
            (false, true) => w31(e.tl, e.left[0]),
            (false, false) => e.tl,
        };
    }
    if e.has_left {
        let p = &e.left;
        f.left[0] = if e.has_tl {
            avg3(e.tl, p[0], p[1])
        } else {
            w31(p[0], p[1])
        };
        for y in 1..7 {
            f.left[y] = avg3(p[y - 1], p[y], p[y + 1]);
        }
        f.left[7] = w31(p[7], p[6]);
    }
    f
}

/// Intra_8x8 prediction (8.3.2.2.2–10) from filtered samples, raster order.
pub fn pred8(m: u8, e: &Edge8, f: &Filtered8) -> [u8; 64] {
    let t = |x: i32| -> u16 {
        if x < 0 {
            u16::from(f.tl)
        } else {
            u16::from(f.top[x as usize])
        }
    };
    let l = |y: i32| -> u16 {
        if y < 0 {
            u16::from(f.tl)
        } else {
            u16::from(f.left[y as usize])
        }
    };
    let a2 = |a: u16, b: u16| ((a + b + 1) >> 1) as u8;
    let a3 = |a: u16, b: u16, c: u16| ((a + 2 * b + c + 2) >> 2) as u8;
    let mut p = [0u8; 64];
    if m == 2 {
        let st: u32 = f.top[..8].iter().map(|&v| u32::from(v)).sum();
        let sl: u32 = f.left.iter().map(|&v| u32::from(v)).sum();
        let v = match (e.has_top, e.has_left) {
            (true, true) => (st + sl + 8) >> 4,
            (false, true) => (sl + 4) >> 3,
            (true, false) => (st + 4) >> 3,
            _ => 128,
        };
        p.fill(v as u8);
        return p;
    }
    let tl = u16::from(f.tl);
    for y in 0..8i32 {
        for x in 0..8i32 {
            let v = match m {
                0 => t(x) as u8,
                1 => l(y) as u8,
                3 => {
                    if x == 7 && y == 7 {
                        ((t(14) + 3 * t(15) + 2) >> 2) as u8
                    } else {
                        a3(t(x + y), t(x + y + 1), t(x + y + 2))
                    }
                }
                4 => {
                    if x > y {
                        a3(t(x - y - 2), t(x - y - 1), t(x - y))
                    } else if x < y {
                        a3(l(y - x - 2), l(y - x - 1), l(y - x))
                    } else {
                        a3(t(0), tl, l(0))
                    }
                }
                5 => {
                    let z = 2 * x - y;
                    if z >= 0 && z % 2 == 0 {
                        a2(t(x - (y >> 1) - 1), t(x - (y >> 1)))
                    } else if z >= 0 {
                        a3(t(x - (y >> 1) - 2), t(x - (y >> 1) - 1), t(x - (y >> 1)))
                    } else if z == -1 {
                        a3(l(0), tl, t(0))
                    } else {
                        a3(l(y - 2 * x - 1), l(y - 2 * x - 2), l(y - 2 * x - 3))
                    }
                }
                6 => {
                    let z = 2 * y - x;
                    if z >= 0 && z % 2 == 0 {
                        a2(l(y - (x >> 1) - 1), l(y - (x >> 1)))
                    } else if z >= 0 {
                        a3(l(y - (x >> 1) - 2), l(y - (x >> 1) - 1), l(y - (x >> 1)))
                    } else if z == -1 {
                        a3(l(0), tl, t(0))
                    } else {
                        a3(t(x - 2 * y - 1), t(x - 2 * y - 2), t(x - 2 * y - 3))
                    }
                }
                7 => {
                    if y % 2 == 0 {
                        a2(t(x + (y >> 1)), t(x + (y >> 1) + 1))
                    } else {
                        a3(t(x + (y >> 1)), t(x + (y >> 1) + 1), t(x + (y >> 1) + 2))
                    }
                }
                _ => {
                    let z = x + 2 * y;
                    if z < 13 && z % 2 == 0 {
                        a2(l(y + (x >> 1)), l(y + (x >> 1) + 1))
                    } else if z < 13 {
                        a3(l(y + (x >> 1)), l(y + (x >> 1) + 1), l(y + (x >> 1) + 2))
                    } else if z == 13 {
                        ((l(6) + 3 * l(7) + 2) >> 2) as u8
                    } else {
                        l(7) as u8
                    }
                }
            };
            p[(y * 8 + x) as usize] = v;
        }
    }
    p
}

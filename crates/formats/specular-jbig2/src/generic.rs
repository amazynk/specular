//! Generic region decoding (6.2) and generic refinement region decoding (6.3).
//!
//! The arithmetic generic decoder keeps the two rows above and the decoded part of the
//! current row in sliding 17-bit windows (bit `8 - dx` holds the pixel at column x + dx), so
//! each pixel's context is a few shifts and masks; adaptive pixels inside the windows are
//! one shift each, others are read from the bitmap.

use crate::bitmap::{Bitmap, Budget};
use crate::mq::Mq;
use crate::{Error, Result};

/// Number of contexts of generic template `t`.
pub(crate) fn contexts(t: u8) -> usize {
    match t {
        0 => 1 << 16,
        1 => 1 << 13,
        _ => 1 << 10,
    }
}

/// Number of contexts of refinement template `t`.
pub(crate) fn refine_contexts(t: u8) -> usize {
    if t == 0 { 1 << 13 } else { 1 << 10 }
}

/// Default adaptive pixels of generic template `t`.
pub(crate) fn default_at(t: u8) -> [(i32, i32); 4] {
    match t {
        0 => [(3, -1), (-3, -1), (2, -2), (-2, -2)],
        1 => [(3, -1), (0, 0), (0, 0), (0, 0)],
        _ => [(2, -1), (0, 0), (0, 0), (0, 0)],
    }
}

pub(crate) struct Generic<'a> {
    pub w: u32,
    pub h: u32,
    pub template: u8,
    pub tpgdon: bool,
    pub at: [(i32, i32); 4],
    /// Pixels set here are not coded and stay 0 (halftone grey-scale planes).
    pub skip: Option<&'a Bitmap>,
}

/// Where an adaptive pixel comes from.
#[derive(Clone, Copy)]
enum At {
    /// Window 0, 1 or 2 (current row, one up, two up) and the shift.
    Win(u8, u32),
    Direct(i64, i64),
}

fn at_source((dx, dy): (i32, i32)) -> At {
    let (dx, dy) = (dx as i64, dy as i64);
    if (-8..=8).contains(&dx) {
        match dy {
            0 if dx < 0 => return At::Win(0, (8 - dx) as u32),
            -1 => return At::Win(1, (8 - dx) as u32),
            -2 => return At::Win(2, (8 - dx) as u32),
            _ => {}
        }
    }
    At::Direct(dx, dy)
}

const MASK: u32 = 0x1FFFF;

#[inline]
fn px(row: Option<&[u8]>, x: usize) -> u32 {
    row.and_then(|r| r.get(x)).copied().unwrap_or(0) as u32
}

/// Decodes a generic region with the arithmetic coder (MMR is [`crate::mmr::decode`]).
pub(crate) fn decode(mq: &mut Mq, cx: &mut [u8], p: &Generic, budget: &Budget) -> Result<Bitmap> {
    let mut bm = Bitmap::new(p.w, p.h, budget)?;
    if cx.len() < contexts(p.template) {
        return Err(Error::Malformed("generic region contexts".into()));
    }
    let w = p.w as usize;
    let n_at = if p.template == 0 { 4 } else { 1 };
    let mut ats = [At::Direct(0, 0); 4];
    for (a, &at) in ats.iter_mut().zip(&p.at).take(n_at) {
        *a = at_source(at);
    }
    let sltp = match p.template {
        0 => 0x9B25,
        1 => 0x0795,
        2 => 0x00E5,
        _ => 0x0195,
    };
    let mut ltp = 0u32;
    for y in 0..p.h as usize {
        if p.tpgdon {
            ltp ^= mq.decode(&mut cx[sltp]);
            if ltp == 1 {
                if y > 0 {
                    let (a, b) = bm.d.split_at_mut(y * w);
                    b[..w].copy_from_slice(&a[(y - 1) * w..]);
                }
                continue;
            }
        }
        if mq.exhausted() {
            return Err(Error::Truncated("generic region data ends early".into()));
        }
        let (above, cur) = bm.d.split_at_mut(y * w);
        let cur = &mut cur[..w];
        let r1 = (y >= 1).then(|| &above[(y - 1) * w..y * w]);
        let r2 = (y >= 2).then(|| &above[(y - 2) * w..(y - 1) * w]);
        let mut w1 = 0u32;
        let mut w2 = 0u32;
        for c in 0..=8usize {
            w1 |= px(r1, c) << (8 - c);
            w2 |= px(r2, c) << (8 - c);
        }
        let mut w0 = 0u32;
        let skip = p.skip.map(|s| s.row(y as u32));
        for x in 0..w {
            let nominal = match p.template {
                0 => (((w2 >> 7) & 7) << 12) | (((w1 >> 6) & 0x1F) << 5) | ((w0 >> 9) & 0xF),
                1 => (((w2 >> 6) & 0xF) << 9) | (((w1 >> 6) & 0x1F) << 4) | ((w0 >> 9) & 7),
                2 => (((w2 >> 7) & 7) << 7) | (((w1 >> 7) & 0xF) << 3) | ((w0 >> 9) & 3),
                _ => (((w1 >> 7) & 0x1F) << 5) | ((w0 >> 9) & 0xF),
            };
            let at = |i: usize| -> u32 {
                match ats[i] {
                    At::Win(0, s) => (w0 >> s) & 1,
                    At::Win(1, s) => (w1 >> s) & 1,
                    At::Win(_, s) => (w2 >> s) & 1,
                    At::Direct(dx, dy) => {
                        let (xx, yy) = (x as i64 + dx, y as i64 + dy);
                        if yy < 0 || xx < 0 || xx >= w as i64 || yy > y as i64 {
                            0
                        } else if yy == y as i64 {
                            if xx < x as i64 {
                                cur[xx as usize] as u32
                            } else {
                                0
                            }
                        } else {
                            above[yy as usize * w + xx as usize] as u32
                        }
                    }
                }
            };
            let ctx = match p.template {
                0 => nominal | (at(0) << 4) | (at(1) << 10) | (at(2) << 11) | (at(3) << 15),
                1 => nominal | (at(0) << 3),
                2 => nominal | (at(0) << 2),
                _ => nominal | (at(0) << 4),
            };
            let v = if skip.is_some_and(|s| s[x] != 0) {
                0
            } else {
                mq.decode(&mut cx[ctx as usize])
            };
            cur[x] = v as u8;
            w0 = ((w0 | (v << 8)) << 1) & MASK;
            w1 = ((w1 << 1) | px(r1, x + 9)) & MASK;
            w2 = ((w2 << 1) | px(r2, x + 9)) & MASK;
        }
    }
    Ok(bm)
}

pub(crate) struct Refine<'a> {
    pub w: u32,
    pub h: u32,
    pub template: u8,
    pub reference: &'a Bitmap,
    pub dx: i64,
    pub dy: i64,
    pub tpgron: bool,
    pub at: [(i8, i8); 2],
}

/// Decodes a generic refinement region (6.3.5).
pub(crate) fn refine(mq: &mut Mq, cx: &mut [u8], p: &Refine, budget: &Budget) -> Result<Bitmap> {
    let mut bm = Bitmap::new(p.w, p.h, budget)?;
    if cx.len() < refine_contexts(p.template) {
        return Err(Error::Malformed("refinement contexts".into()));
    }
    let r = p.reference;
    let (a1x, a1y) = (p.at[0].0 as i64, p.at[0].1 as i64);
    let (a2x, a2y) = (p.at[1].0 as i64, p.at[1].1 as i64);
    let sltp = if p.template == 0 { 0x100 } else { 0x080 };
    let mut ltp = 0;
    let w = p.w as i64;
    for y in 0..p.h as i64 {
        if p.tpgron {
            ltp ^= mq.decode(&mut cx[sltp]);
        }
        if mq.exhausted() {
            return Err(Error::Truncated("refinement data ends early".into()));
        }
        for x in 0..w {
            let (rx, ry) = (x - p.dx, y - p.dy);
            if ltp == 1 {
                let v = r.get(rx, ry);
                let same = (-1..=1).all(|j| (-1..=1).all(|i| r.get(rx + i, ry + j) == v));
                if same {
                    bm.d[(y * w + x) as usize] = v;
                    continue;
                }
            }
            let g = |xx: i64, yy: i64| bm.get(xx, yy) as usize;
            let f = |xx: i64, yy: i64| r.get(xx, yy) as usize;
            let ctx = if p.template == 0 {
                g(x - 1, y)
                    | (g(x + 1, y - 1) << 1)
                    | (g(x, y - 1) << 2)
                    | (g(x + a1x, y + a1y) << 3)
                    | (f(rx + 1, ry + 1) << 4)
                    | (f(rx, ry + 1) << 5)
                    | (f(rx - 1, ry + 1) << 6)
                    | (f(rx + 1, ry) << 7)
                    | (f(rx, ry) << 8)
                    | (f(rx - 1, ry) << 9)
                    | (f(rx + 1, ry - 1) << 10)
                    | (f(rx, ry - 1) << 11)
                    | (f(rx + a2x, ry + a2y) << 12)
            } else {
                g(x - 1, y)
                    | (g(x + 1, y - 1) << 1)
                    | (g(x, y - 1) << 2)
                    | (g(x - 1, y - 1) << 3)
                    | (f(rx + 1, ry + 1) << 4)
                    | (f(rx, ry + 1) << 5)
                    | (f(rx + 1, ry) << 6)
                    | (f(rx, ry) << 7)
                    | (f(rx - 1, ry) << 8)
                    | (f(rx, ry - 1) << 9)
            };
            let v = mq.decode(&mut cx[ctx]);
            bm.d[(y * w + x) as usize] = v as u8;
        }
    }
    Ok(bm)
}

//! Bilevel bitmaps (one byte per pixel, 1 = black) and region combination.

use crate::{Error, Result};
use std::cell::Cell;

/// Largest width or height of a page.
pub const MAX_SIDE: u32 = 1 << 20;
/// Largest width or height of any bitmap (a height class's collective bitmap is as wide as
/// all its symbols together).
pub const MAX_BITMAP_SIDE: u32 = 1 << 26;
/// Largest pixel count of any one bitmap (the page included).
pub const MAX_PIXELS: u64 = 1 << 28;
/// Pixels all bitmaps of one decode may allocate together (symbols, patterns, regions, page).
pub const MAX_TOTAL_PIXELS: u64 = 1 << 30;

/// What one decode may still allocate, in pixels.
pub(crate) struct Budget(Cell<u64>);

impl Budget {
    pub(crate) fn new() -> Budget {
        Budget(Cell::new(MAX_TOTAL_PIXELS))
    }

    pub(crate) fn take(&self, n: u64) -> Result<()> {
        let left = self.0.get();
        if n > left {
            return Err(Error::TooLarge);
        }
        self.0.set(left - n);
        Ok(())
    }
}

/// Region combination operators (7.4.1.5 and 7.4.8.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Or,
    And,
    Xor,
    Xnor,
    Replace,
}

impl Op {
    pub(crate) fn from_bits(v: u8) -> Op {
        match v & 7 {
            0 => Op::Or,
            1 => Op::And,
            2 => Op::Xor,
            3 => Op::Xnor,
            _ => Op::Replace,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Bitmap {
    pub w: u32,
    pub h: u32,
    /// Row-major, one byte per pixel: 0 white, 1 black.
    pub d: Vec<u8>,
}

impl Bitmap {
    pub(crate) fn new(w: u32, h: u32, budget: &Budget) -> Result<Bitmap> {
        Bitmap::filled(w, h, 0, budget)
    }

    pub(crate) fn filled(w: u32, h: u32, v: u8, budget: &Budget) -> Result<Bitmap> {
        if w > MAX_BITMAP_SIDE || h > MAX_BITMAP_SIDE || w as u64 * h as u64 > MAX_PIXELS {
            return Err(Error::TooLarge);
        }
        let n = w as u64 * h as u64;
        budget.take(n)?;
        Ok(Bitmap {
            w,
            h,
            d: vec![v; n as usize],
        })
    }

    #[inline]
    pub(crate) fn get(&self, x: i64, y: i64) -> u8 {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            0
        } else {
            self.d[y as usize * self.w as usize + x as usize]
        }
    }

    #[inline]
    pub(crate) fn row(&self, y: u32) -> &[u8] {
        let w = self.w as usize;
        &self.d[y as usize * w..(y as usize + 1) * w]
    }

    /// Grows the height to `h` (when larger), new rows set to `v`.
    pub(crate) fn grow(&mut self, h: u32, v: u8, budget: &Budget) -> Result<()> {
        if h <= self.h {
            return Ok(());
        }
        if h > MAX_SIDE || self.w as u64 * h as u64 > MAX_PIXELS {
            return Err(Error::TooLarge);
        }
        budget.take(self.w as u64 * (h - self.h) as u64)?;
        self.d.resize(self.w as usize * h as usize, v);
        self.h = h;
        Ok(())
    }

    /// The part at (`x`, `y`) of size `w`×`h`; pixels outside are 0.
    pub(crate) fn crop(&self, x: i64, y: i64, w: u32, h: u32, budget: &Budget) -> Result<Bitmap> {
        let mut out = Bitmap::new(w, h, budget)?;
        for r in 0..h as i64 {
            let sy = y + r;
            if sy < 0 || sy >= self.h as i64 {
                continue;
            }
            let x0 = x.max(0);
            let x1 = (x + w as i64).min(self.w as i64);
            if x0 >= x1 {
                continue;
            }
            let src = &self.row(sy as u32)[x0 as usize..x1 as usize];
            let o = r as usize * w as usize + (x0 - x) as usize;
            out.d[o..o + src.len()].copy_from_slice(src);
        }
        Ok(out)
    }

    /// Combines `src` into `self` with its top-left corner at (`x`, `y`), clipped.
    pub(crate) fn compose(&mut self, src: &Bitmap, x: i64, y: i64, op: Op) {
        let x0 = x.max(0);
        let x1 = (x + src.w as i64).min(self.w as i64);
        let y0 = y.max(0);
        let y1 = (y + src.h as i64).min(self.h as i64);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let n = (x1 - x0) as usize;
        let sx = (x0 - x) as usize;
        let w = self.w as usize;
        for yy in y0..y1 {
            let s = &src.d[(yy - y) as usize * src.w as usize + sx..][..n];
            let o = &mut self.d[yy as usize * w + x0 as usize..][..n];
            match op {
                Op::Or => o.iter_mut().zip(s).for_each(|(o, s)| *o |= s),
                Op::And => o.iter_mut().zip(s).for_each(|(o, s)| *o &= s),
                Op::Xor => o.iter_mut().zip(s).for_each(|(o, s)| *o ^= s),
                Op::Xnor => o.iter_mut().zip(s).for_each(|(o, s)| *o = 1 ^ (*o ^ s)),
                Op::Replace => o.copy_from_slice(s),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_ops_and_clip() {
        let b = Budget::new();
        let mut page = Bitmap::new(4, 3, &b).unwrap();
        page.d[0] = 1;
        let src = Bitmap {
            w: 2,
            h: 2,
            d: vec![1, 0, 1, 1],
        };
        page.compose(&src, -1, -1, Op::Xor);
        assert_eq!(page.d[0], 0);
        page.compose(&src, 3, 2, Op::Or);
        assert_eq!(page.d[11], 1);
        page.compose(&src, 0, 0, Op::Xnor);
        assert_eq!(&page.d[..2], &[0, 1]);
        assert!(Bitmap::new(1 << 27, 1, &b).is_err());
    }
}

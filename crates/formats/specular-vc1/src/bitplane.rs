//! Bitplane coding (§8.7): one binary symbol per macroblock, coded in the picture header or,
//! in raw mode, one bit in each macroblock's header.

use crate::bits::Bits;
use crate::small::{IMODE, Imode, NORM2, imode};
use crate::tables::NORM6;
use crate::vlc::Vlc;
use crate::{Error, malformed};
use std::sync::LazyLock;

static NORM6_VLC: LazyLock<Vlc> = LazyLock::new(|| Vlc::new(&NORM6));

#[derive(Clone, Debug, Default)]
pub struct Bitplane {
    /// The symbols are in the macroblock layer.
    pub raw: bool,
    pub bits: Vec<u8>,
}

impl Bitplane {
    /// The symbol of macroblock `i`, reading it from the macroblock layer in raw mode.
    #[inline]
    pub fn get(&self, i: usize, r: &mut Bits) -> bool {
        if self.raw {
            r.flag()
        } else {
            self.bits.get(i).is_some_and(|&b| b != 0)
        }
    }

    /// Reads a bitplane of `w` × `h` macroblocks (§7.2).
    pub fn read(r: &mut Bits, w: usize, h: usize) -> Result<Bitplane, Error> {
        let invert = r.flag();
        let mode = imode(IMODE.read(r).ok_or_else(|| malformed("bad IMODE"))?);
        let n = w * h;
        let mut bits = vec![0u8; n];
        match mode {
            Imode::Raw => {
                return Ok(Bitplane {
                    raw: true,
                    bits: Vec::new(),
                });
            }
            Imode::Norm2 | Imode::Diff2 => {
                let mut i = 0;
                if n % 2 == 1 {
                    bits[0] = r.bit() as u8;
                    i = 1;
                }
                while i < n {
                    let s = NORM2.read(r).ok_or_else(|| malformed("bad Norm-2 code"))?;
                    bits[i] = (s >> 1) as u8;
                    bits[i + 1] = (s & 1) as u8;
                    i += 2;
                }
            }
            Imode::Norm6 | Imode::Diff6 => {
                let tile = |r: &mut Bits| -> Result<u16, Error> {
                    NORM6_VLC
                        .read(r)
                        .ok_or_else(|| malformed("bad Norm-6 code"))
                };
                if h.is_multiple_of(3) && !w.is_multiple_of(3) {
                    // 2x3 vertical tiles from column w % 2.
                    for ty in (0..h).step_by(3) {
                        for tx in (w % 2..w).step_by(2) {
                            let k = tile(r)?;
                            for i in 0..6 {
                                bits[(ty + i / 2) * w + tx + i % 2] = ((k >> i) & 1) as u8;
                            }
                        }
                    }
                    for x in 0..w % 2 {
                        col_skip(r, &mut bits, w, x, 0, h);
                    }
                } else {
                    // 3x2 horizontal tiles from row h % 2, column w % 3.
                    for ty in (h % 2..h).step_by(2) {
                        for tx in (w % 3..w).step_by(3) {
                            let k = tile(r)?;
                            for i in 0..6 {
                                bits[(ty + i / 3) * w + tx + i % 3] = ((k >> i) & 1) as u8;
                            }
                        }
                    }
                    for x in 0..w % 3 {
                        col_skip(r, &mut bits, w, x, 0, h);
                    }
                    if h % 2 == 1 {
                        row_skip(r, &mut bits, w, 0, w % 3, w);
                    }
                }
            }
            Imode::RowSkip => {
                for y in 0..h {
                    row_skip(r, &mut bits, w, y, 0, w);
                }
            }
            Imode::ColSkip => {
                for x in 0..w {
                    col_skip(r, &mut bits, w, x, 0, h);
                }
            }
        }
        if matches!(mode, Imode::Diff2 | Imode::Diff6) {
            // §8.7.3.8: the predictor is A (INVERT) at the origin and where the left and top
            // neighbours differ, the top neighbour in the first column, else the left one.
            let a = invert as u8;
            for y in 0..h {
                for x in 0..w {
                    let i = y * w + x;
                    let p = if x == 0 && y == 0 {
                        a
                    } else if y == 0 {
                        bits[i - 1]
                    } else if x == 0 {
                        bits[i - w]
                    } else if bits[i - 1] != bits[i - w] {
                        a
                    } else {
                        bits[i - 1]
                    };
                    bits[i] ^= p;
                }
            }
        } else if invert {
            for b in &mut bits {
                *b ^= 1;
            }
        }
        if r.overrun() {
            return Err(malformed("a bitplane runs past the picture"));
        }
        Ok(Bitplane { raw: false, bits })
    }
}

fn row_skip(r: &mut Bits, bits: &mut [u8], w: usize, y: usize, x0: usize, x1: usize) {
    if r.flag() {
        for x in x0..x1 {
            bits[y * w + x] = r.bit() as u8;
        }
    }
}

fn col_skip(r: &mut Bits, bits: &mut [u8], w: usize, x: usize, y0: usize, y1: usize) {
    if r.flag() {
        for y in y0..y1 {
            bits[y * w + x] = r.bit() as u8;
        }
    }
}

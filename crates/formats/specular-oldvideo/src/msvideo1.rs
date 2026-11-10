//! Microsoft Video 1 (`CRAM`, `MSVC`): 4×4 blocks from the bottom row of blocks up, each
//! skipped, one colour, two colours under a 16-bit mask, or eight (two per 2×2 quarter).
//! Palette indices at 8 bits, RGB555 at 16.

use crate::{Pixels, Reader, Result};

/// Paints one block: `pick(bit, quarter)` gives the colour of a pixel from its mask bit
/// (bit 0 the bottom-left pixel, rows bottom to top) and its quarter (0 bottom-left,
/// 1 bottom-right, 2 top-left, 3 top-right).
fn paint<T: Copy>(
    px: &mut [T],
    w: usize,
    h: usize,
    bx: usize,
    by: usize,
    mask: u16,
    pick: impl Fn(bool, usize) -> T,
) {
    for row in 0..4 {
        // Mask rows run from the block's bottom row.
        let y = by + 3 - row;
        if y >= h {
            continue;
        }
        for col in 0..4 {
            let x = bx + col;
            if x >= w {
                continue;
            }
            let bit = mask >> (row * 4 + col) & 1 != 0;
            let quarter = usize::from(col >= 2) + 2 * usize::from(row >= 2);
            px[y * w + x] = pick(bit, quarter);
        }
    }
}

pub fn decode(data: &[u8], w: usize, h: usize, canvas: &mut Pixels) -> Result<()> {
    let mut r = Reader::new(data);
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let total = bw * bh;
    let mut skip = 0usize;
    for n in 0..total {
        // Blocks from the bottom-left, left to right, rows upwards.
        let (bx, by) = (n % bw * 4, (bh - 1 - n / bw) * 4);
        if skip > 0 {
            skip -= 1;
            continue;
        }
        if r.left() < 2 {
            break;
        }
        let a = r.u8()?;
        let b = r.u8()?;
        if a == 0 && b == 0 && n + 1 >= total {
            break;
        }
        if (0x84..0x88).contains(&b) {
            skip = (usize::from(b - 0x84) << 8 | usize::from(a)).saturating_sub(1);
            continue;
        }
        let mask = u16::from(b) << 8 | u16::from(a);
        match canvas {
            Pixels::Pal8(px) => {
                if b < 0x80 {
                    let (ca, cb) = (r.u8()?, r.u8()?);
                    paint(px, w, h, bx, by, mask, |bit, _| if bit { ca } else { cb });
                } else if b >= 0x90 {
                    let q = r.bytes(8)?;
                    paint(px, w, h, bx, by, mask, |bit, k| {
                        q[2 * k + usize::from(!bit)]
                    });
                } else {
                    paint(px, w, h, bx, by, 0, |_, _| a);
                }
            }
            Pixels::Rgb555(px) => {
                if b < 0x80 {
                    let ca = r.u16le()?;
                    if ca & 0x8000 != 0 {
                        let mut q = [ca & 0x7FFF, 0, 0, 0, 0, 0, 0, 0];
                        for c in q.iter_mut().skip(1) {
                            *c = r.u16le()? & 0x7FFF;
                        }
                        paint(px, w, h, bx, by, mask, |bit, k| {
                            q[2 * k + usize::from(!bit)]
                        });
                    } else {
                        let cb = r.u16le()? & 0x7FFF;
                        paint(px, w, h, bx, by, mask, |bit, _| if bit { ca } else { cb });
                    }
                } else {
                    let c = mask & 0x7FFF;
                    paint(px, w, h, bx, by, 0, |_, _| c);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

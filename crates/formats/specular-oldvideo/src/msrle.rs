//! Microsoft RLE (BI_RLE8, BI_RLE4): pairs of a count and a colour from the bottom row up;
//! a zero count escapes to end of line, end of picture, a move, or a literal run padded to
//! 16 bits. Pixels a frame doesn't reach keep their colour.

use crate::{Pixels, Reader, Result};

pub fn decode(data: &[u8], w: usize, h: usize, depth: u16, canvas: &mut Pixels) -> Result<()> {
    let Pixels::Pal8(px) = canvas else {
        return Ok(());
    };
    let four = depth == 4;
    let mut r = Reader::new(data);
    // Line counted from the bottom.
    let (mut x, mut line) = (0usize, 0usize);
    let mut put = |x: usize, line: usize, c: u8| {
        if x < w && line < h {
            px[(h - 1 - line) * w + x] = c;
        }
    };
    while r.left() >= 2 && line < h {
        let n = r.u8()?;
        let c = r.u8()?;
        if n > 0 {
            for k in 0..usize::from(n) {
                let v = if four {
                    if k.is_multiple_of(2) { c >> 4 } else { c & 15 }
                } else {
                    c
                };
                put(x + k, line, v);
            }
            x += usize::from(n);
            continue;
        }
        match c {
            0 => {
                x = 0;
                line += 1;
            }
            1 => break,
            2 => {
                x += usize::from(r.u8()?);
                line += usize::from(r.u8()?);
            }
            n => {
                let n = usize::from(n);
                let bytes = if four { n.div_ceil(2) } else { n };
                let run = r.bytes(bytes)?;
                for k in 0..n {
                    let v = if four {
                        let b = run[k / 2];
                        if k.is_multiple_of(2) { b >> 4 } else { b & 15 }
                    } else {
                        run[k]
                    };
                    put(x + k, line, v);
                }
                x += n;
                if !bytes.is_multiple_of(2) {
                    r.skip(1).ok();
                }
            }
        }
    }
    Ok(())
}

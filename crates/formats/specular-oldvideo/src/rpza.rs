//! Apple Video (`rpza`): 4×4 RGB555 blocks left to right, top to bottom; runs of skipped
//! blocks, one colour, or four colours (two given, two between them) under 2-bit indices,
//! and single blocks of sixteen colours.

use crate::{Pixels, Reader, Result, bad};

/// The four colours between B and A: B, two thirds of the way and one third, A.
fn ramp(a: u16, b: u16) -> [u16; 4] {
    let mix = |wa: u16, wb: u16| {
        let ch = |s: u16| ((wa * (a >> s & 31) + wb * (b >> s & 31)) >> 5) << s;
        ch(10) | ch(5) | ch(0)
    };
    [b, mix(11, 21), mix(21, 11), a]
}

pub fn decode(data: &[u8], w: usize, h: usize, canvas: &mut Pixels) -> Result<()> {
    let Pixels::Rgb555(px) = canvas else {
        return Ok(());
    };
    if data.len() < 4 {
        return Ok(());
    }
    let mut r = Reader::new(data);
    if r.u8()? != 0xE1 {
        return Err(bad("RPZA chunk"));
    }
    let len = r.u24be()? as usize;
    let mut r = Reader::new(&data[..len.clamp(4, data.len())]);
    r.skip(4)?;
    let bw = w.div_ceil(4);
    let total = bw * h.div_ceil(4);
    let mut paint = |n: usize, f: &dyn Fn(usize) -> u16| {
        let (bx, by) = (n % bw * 4, n / bw * 4);
        for k in 0..16 {
            let (x, y) = (bx + k % 4, by + k / 4);
            if x < w && y < h {
                px[y * w + x] = f(k);
            }
        }
    };
    let four = |r: &mut Reader<'_>, set: [u16; 4]| -> Result<[u16; 16]> {
        let idx = r.bytes(4)?;
        let mut b = [0u16; 16];
        for (k, v) in b.iter_mut().enumerate() {
            *v = set[usize::from(idx[k / 4] >> (6 - 2 * (k % 4)) & 3)];
        }
        Ok(b)
    };
    let mut n = 0usize;
    while n < total && r.left() > 0 {
        let op = r.u8()?;
        let count = usize::from(op & 31) + 1;
        if op & 0x80 == 0 {
            // One block: colour A from this byte and the next.
            let a = u16::from(op) << 8 | u16::from(r.u8()?);
            let next = *r.data.get(r.at).ok_or_else(|| bad("truncated"))?;
            let b = if next & 0x80 != 0 {
                let b = r.u16be()? & 0x7FFF;
                four(&mut r, ramp(a, b))?
            } else {
                let mut b = [0u16; 16];
                b[0] = a;
                for v in b.iter_mut().skip(1) {
                    *v = r.u16be()? & 0x7FFF;
                }
                b
            };
            if n < total {
                paint(n, &|k| b[k]);
            }
            n += 1;
            continue;
        }
        match op & 0xE0 {
            0x80 => n += count,
            0xA0 => {
                let c = r.u16be()? & 0x7FFF;
                for _ in 0..count {
                    if n < total {
                        paint(n, &|_| c);
                    }
                    n += 1;
                }
            }
            0xC0 => {
                let a = r.u16be()? & 0x7FFF;
                let b = r.u16be()? & 0x7FFF;
                let set = ramp(a, b);
                for _ in 0..count {
                    let blk = four(&mut r, set)?;
                    if n < total {
                        paint(n, &|k| blk[k]);
                    }
                    n += 1;
                }
            }
            _ => return Err(bad("RPZA opcode")),
        }
    }
    Ok(())
}

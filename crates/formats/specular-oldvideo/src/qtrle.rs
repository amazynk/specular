//! QuickTime Animation (`rle `): a 4-byte chunk size, a header saying which lines change,
//! then per line a skip and run codes (literal runs, repeated units, more skips, end of
//! line). A unit is four pixels at 8 bits, one pixel at 16, 24 and 32.

use crate::{Pixels, Reader, Result, bad};

pub fn decode(data: &[u8], w: usize, h: usize, depth: u16, canvas: &mut Pixels) -> Result<()> {
    let mut r = Reader::new(data);
    if data.len() < 8 {
        // An unchanged frame.
        return Ok(());
    }
    r.skip(4)?;
    let header = r.u16be()?;
    let (start, lines) = if header & 0x0008 != 0 {
        let s = usize::from(r.u16be()?);
        r.skip(2)?;
        let n = usize::from(r.u16be()?);
        r.skip(2)?;
        (s, n)
    } else {
        (0, h)
    };
    if start >= h {
        return Err(bad("start line"));
    }
    // Bytes per unit and pixels per unit.
    let (unit_bytes, unit_px) = match depth {
        8 | 40 => (4, 4),
        16 => (2, 1),
        24 => (3, 1),
        _ => (4, 1),
    };
    // Writes `bytes` (whole units) at pixel `x` of row `y`, clipped to the row.
    let mut put = |y: usize, x: usize, bytes: &[u8]| {
        let n = (bytes.len() / unit_bytes * unit_px).min(w.saturating_sub(x));
        if n == 0 {
            return;
        }
        let i = y * w + x;
        match canvas {
            Pixels::Pal8(p) => p[i..i + n].copy_from_slice(&bytes[..n]),
            Pixels::Rgb555(p) => {
                for (k, d) in p[i..i + n].iter_mut().enumerate() {
                    *d = u16::from_be_bytes([bytes[2 * k], bytes[2 * k + 1]]) & 0x7FFF;
                }
            }
            Pixels::Rgb24(p) => p[3 * i..3 * (i + n)].copy_from_slice(&bytes[..3 * n]),
            Pixels::Argb(p) => p[4 * i..4 * (i + n)].copy_from_slice(&bytes[..4 * n]),
        }
    };
    let mut repeated = Vec::new();
    for y in start..(start + lines).min(h) {
        let skip = r.u8()?;
        if skip == 0 {
            break;
        }
        let mut x = (usize::from(skip) - 1) * unit_px;
        loop {
            let code = r.u8()? as i8;
            match code {
                -1 => break,
                0 => {
                    let s = r.u8()?;
                    x += usize::from(s.saturating_sub(1)) * unit_px;
                }
                n if n > 0 => {
                    let bytes = r.bytes(n as usize * unit_bytes)?;
                    put(y, x, bytes);
                    x += n as usize * unit_px;
                }
                n => {
                    let unit = r.bytes(unit_bytes)?;
                    let count = usize::from(n.unsigned_abs());
                    repeated.clear();
                    for _ in 0..count {
                        repeated.extend_from_slice(unit);
                    }
                    put(y, x, &repeated);
                    x += count * unit_px;
                }
            }
        }
    }
    Ok(())
}

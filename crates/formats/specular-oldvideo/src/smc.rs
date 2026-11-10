//! Apple SMC (QuickTime Graphics): 4×4 palette blocks left to right, top to bottom. Opcodes
//! skip blocks, repeat the last block or the last two, fill with one colour, or paint two,
//! four or eight colours (new or from a cache of the frame's earlier sets) under per-pixel
//! indices, or give all sixteen pixels.

use crate::{Pixels, Reader, Result, bad};

/// A cache of colour sets of `N` entries, written in a ring of 256. Each frame writes from
/// the start again; entries it doesn't reach keep the frames before's sets.
pub struct Cache<const N: usize> {
    sets: Box<[[u8; N]; 256]>,
    next: usize,
}

impl<const N: usize> Cache<N> {
    fn new() -> Self {
        Cache {
            sets: Box::new([[0; N]; 256]),
            next: 0,
        }
    }
    /// The set an opcode names: new from the stream (and cached), or a cached one.
    fn get(&mut self, r: &mut Reader<'_>, new: bool) -> Result<[u8; N]> {
        if new {
            let mut s = [0u8; N];
            s.copy_from_slice(r.bytes(N)?);
            self.sets[self.next] = s;
            self.next = (self.next + 1) % 256;
            Ok(s)
        } else {
            Ok(self.sets[usize::from(r.u8()?)])
        }
    }
}

/// The colour caches, kept from frame to frame.
pub struct Caches {
    pairs: Cache<2>,
    quads: Cache<4>,
    octets: Cache<8>,
}

impl Default for Caches {
    fn default() -> Self {
        Caches {
            pairs: Cache::new(),
            quads: Cache::new(),
            octets: Cache::new(),
        }
    }
}

pub fn decode(
    data: &[u8],
    w: usize,
    h: usize,
    caches: &mut Caches,
    canvas: &mut Pixels,
) -> Result<()> {
    let Pixels::Pal8(px) = canvas else {
        return Ok(());
    };
    let mut r = Reader::new(data);
    if data.len() < 4 {
        return Ok(());
    }
    r.skip(1)?;
    let len = r.u24be()? as usize;
    let mut r = Reader::new(&data[..len.clamp(4, data.len())]);
    r.skip(4)?;
    let bw = w.div_ceil(4);
    let total = bw * h.div_ceil(4);
    let origin = |n: usize| (n % bw * 4, n / bw * 4);
    // Block `n` as 16 samples (clipped pixels read as 0).
    let read = |px: &[u8], n: usize| -> [u8; 16] {
        let (bx, by) = origin(n);
        let mut b = [0u8; 16];
        for (k, v) in b.iter_mut().enumerate() {
            let (x, y) = (bx + k % 4, by + k / 4);
            if x < w && y < h {
                *v = px[y * w + x];
            }
        }
        b
    };
    let write = |px: &mut [u8], n: usize, b: &[u8; 16]| {
        let (bx, by) = origin(n);
        for (k, &v) in b.iter().enumerate() {
            let (x, y) = (bx + k % 4, by + k / 4);
            if x < w && y < h {
                px[y * w + x] = v;
            }
        }
    };
    let Caches {
        pairs,
        quads,
        octets,
    } = caches;
    pairs.next = 0;
    quads.next = 0;
    octets.next = 0;
    let mut n = 0usize;
    while n < total && r.left() > 0 {
        let op = r.u8()?;
        let low = usize::from(op & 15);
        // Counts of the opcodes that take one: the low nibble, or the next byte.
        let count = |r: &mut Reader<'_>| -> Result<usize> {
            Ok(if op & 0x10 != 0 {
                usize::from(r.u8()?) + 1
            } else {
                low + 1
            })
        };
        match op & 0xF0 {
            0x00 | 0x10 => n += count(&mut r)?,
            0x20 | 0x30 => {
                let c = count(&mut r)?;
                if n == 0 {
                    return Err(bad("repeat with no block before"));
                }
                let last = read(px, n - 1);
                for _ in 0..c.min(total - n) {
                    write(px, n, &last);
                    n += 1;
                }
            }
            0x40 | 0x50 => {
                let c = count(&mut r)?;
                if n < 2 {
                    return Err(bad("repeat with no blocks before"));
                }
                let (a, b) = (read(px, n - 2), read(px, n - 1));
                for _ in 0..c {
                    for blk in [&a, &b] {
                        if n < total {
                            write(px, n, blk);
                            n += 1;
                        }
                    }
                }
            }
            0x60 | 0x70 => {
                let c = count(&mut r)?;
                let v = r.u8()?;
                for _ in 0..c.min(total - n) {
                    write(px, n, &[v; 16]);
                    n += 1;
                }
            }
            0x80 | 0x90 => {
                let set = pairs.get(&mut r, op & 0x10 == 0)?;
                for _ in 0..=low {
                    let f = r.u16be()?;
                    let mut b = [0u8; 16];
                    for (k, v) in b.iter_mut().enumerate() {
                        *v = set[usize::from(f >> (15 - k) & 1)];
                    }
                    if n < total {
                        write(px, n, &b);
                        n += 1;
                    }
                }
            }
            0xA0 | 0xB0 => {
                let set = quads.get(&mut r, op & 0x10 == 0)?;
                for _ in 0..=low {
                    let f = r.u32be()?;
                    let mut b = [0u8; 16];
                    for (k, v) in b.iter_mut().enumerate() {
                        *v = set[(f >> (30 - 2 * k) & 3) as usize];
                    }
                    if n < total {
                        write(px, n, &b);
                        n += 1;
                    }
                }
            }
            0xC0 | 0xD0 => {
                let set = octets.get(&mut r, op & 0x10 == 0)?;
                for _ in 0..=low {
                    let s = r.bytes(6)?;
                    // Twelve nibbles rearranged into two 24-bit index sets.
                    let nb = |i: usize| {
                        u32::from(if i.is_multiple_of(2) {
                            s[i / 2] >> 4
                        } else {
                            s[i / 2] & 15
                        })
                    };
                    let fa = [0, 1, 2, 4, 5, 6].iter().fold(0, |a, &i| a << 4 | nb(i));
                    let fb = [8, 9, 10, 3, 7, 11].iter().fold(0, |a, &i| a << 4 | nb(i));
                    let mut b = [0u8; 16];
                    for (k, v) in b.iter_mut().enumerate() {
                        let f = if k < 8 { fa } else { fb };
                        *v = set[(f >> (21 - 3 * (k % 8)) & 7) as usize];
                    }
                    if n < total {
                        write(px, n, &b);
                        n += 1;
                    }
                }
            }
            0xE0 => {
                for _ in 0..=low {
                    let s = r.bytes(16)?;
                    let mut b = [0u8; 16];
                    b.copy_from_slice(s);
                    if n < total {
                        write(px, n, &b);
                        n += 1;
                    }
                }
            }
            _ => return Err(bad("SMC opcode 0xF0")),
        }
    }
    Ok(())
}

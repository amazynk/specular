//! PVRTC1 at 4 and 2 bits per pixel. A texel blends two colours, A and B, each bilinearly
//! upscaled from the four nearest block centres (wrapping at the image edges), by its
//! modulation value. Blocks are 64-bit little-endian words (modulation word, then colour
//! word) in Morton order; levels are at least 2×2 blocks and power-of-two sized.

/// Block width for the mode (block height is 4).
pub(crate) fn block_width(two_bpp: bool) -> usize {
    if two_bpp { 8 } else { 4 }
}

/// Blocks across and down a level of `w`×`h` texels.
pub(crate) fn blocks(w: u32, h: u32, two_bpp: bool) -> (usize, usize) {
    let bw = block_width(two_bpp) as u32;
    (
        w.div_ceil(bw).max(2).next_power_of_two() as usize,
        h.div_ceil(4).max(2).next_power_of_two() as usize,
    )
}

/// Byte offset of block (x, y) in a level of nx × ny blocks (both powers of two): the bits of
/// x and y interleaved (y in the even bits) over the smaller dimension, then the remaining
/// high bits of the larger one.
fn morton(nx: usize, ny: usize, x: usize, y: usize) -> usize {
    let min = nx.min(ny);
    let mut out = 0;
    let mut bit = 0;
    while (1 << bit) < min {
        out |= ((y >> bit) & 1) << (2 * bit);
        out |= ((x >> bit) & 1) << (2 * bit + 1);
        bit += 1;
    }
    let rest = if ny < nx { x } else { y };
    (out | ((rest >> bit) << (2 * bit))) * 8
}

#[derive(Clone, Copy, Default)]
struct Block {
    modulation: u32,
    colour: u32,
}

/// Colour A as 5-bit RGB and 4-bit alpha (opaque RGB554 or translucent ARGB3443).
fn colour_a(c: u32) -> [i32; 4] {
    if c & 0x8000 != 0 {
        let b = (c >> 1) & 15;
        [
            ((c >> 10) & 31) as i32,
            ((c >> 5) & 31) as i32,
            ((b << 1) | (b >> 3)) as i32,
            15,
        ]
    } else {
        let (r, g, b) = ((c >> 8) & 15, (c >> 4) & 15, (c >> 1) & 7);
        [
            ((r << 1) | (r >> 3)) as i32,
            ((g << 1) | (g >> 3)) as i32,
            ((b << 2) | (b >> 1)) as i32,
            (((c >> 12) & 7) << 1) as i32,
        ]
    }
}

/// Colour B (opaque RGB555 or translucent ARGB3444).
fn colour_b(c: u32) -> [i32; 4] {
    if c & 0x8000_0000 != 0 {
        [
            ((c >> 26) & 31) as i32,
            ((c >> 21) & 31) as i32,
            ((c >> 16) & 31) as i32,
            15,
        ]
    } else {
        let (r, g, b) = ((c >> 24) & 15, (c >> 20) & 15, (c >> 16) & 15);
        [
            ((r << 1) | (r >> 3)) as i32,
            ((g << 1) | (g >> 3)) as i32,
            ((b << 1) | (b >> 3)) as i32,
            (((c >> 28) & 7) << 1) as i32,
        ]
    }
}

struct Level<'a> {
    data: &'a [u8],
    nx: usize,
    ny: usize,
    two_bpp: bool,
}

impl Level<'_> {
    fn block(&self, x: usize, y: usize) -> Block {
        let o = morton(self.nx, self.ny, x, y);
        match self.data.get(o..o + 8) {
            Some(b) => Block {
                modulation: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                colour: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
            },
            None => Block::default(),
        }
    }

    /// The 2-bit stored modulation code of texel (lx, ly) of a 2 bpp block (direct mode codes
    /// are doubled to 0 or 3); in the interpolated mode only texels with even lx + ly are
    /// stored, and the flag bits of texels 0 and 10 are replaced by their high bits.
    fn code_2bpp(b: Block, lx: usize, ly: usize) -> usize {
        let m = b.modulation;
        if b.colour & 1 == 0 {
            return if (m >> (ly * 8 + lx)) & 1 != 0 { 3 } else { 0 };
        }
        let k = ly * 4 + lx / 2;
        let v = ((m >> (2 * k)) & 3) as usize;
        if k == 0 || (k == 10 && m & 1 != 0) {
            // One stored bit: the high bit doubled.
            if v & 2 != 0 { 3 } else { 0 }
        } else {
            v
        }
    }

    /// Modulation weight (0…8, + 10 for punch-through) of texel (x, y) of the level.
    fn weight(&self, x: usize, y: usize, cache: &mut BlockCache) -> i32 {
        if !self.two_bpp {
            let b = cache.get(self, x / 4, y / 4);
            let v = (b.modulation >> (2 * ((y & 3) * 4 + (x & 3)))) & 3;
            return if b.colour & 1 != 0 {
                [0, 4, 14, 8][v as usize]
            } else {
                [0, 3, 5, 8][v as usize]
            };
        }
        const REP: [i32; 4] = [0, 3, 5, 8];
        let b = cache.get(self, x / 8, y / 4);
        let (lx, ly) = (x & 7, y & 3);
        if b.colour & 1 == 0 || (lx ^ ly) & 1 == 0 {
            return REP[Self::code_2bpp(b, lx, ly)];
        }
        let (w, h) = (self.nx * 8, self.ny * 4);
        let mut at = |dx: isize, dy: isize| {
            let nx = (x as isize + dx).rem_euclid(w as isize) as usize;
            let ny = (y as isize + dy).rem_euclid(h as isize) as usize;
            let nb = cache.get(self, nx / 8, ny / 4);
            REP[Self::code_2bpp(nb, nx & 7, ny & 3)]
        };
        let m = b.modulation;
        if m & 1 == 0 {
            (at(0, -1) + at(0, 1) + at(-1, 0) + at(1, 0) + 2) / 4
        } else if m & (1 << 20) != 0 {
            (at(0, -1) + at(0, 1) + 1) / 2
        } else {
            (at(-1, 0) + at(1, 0) + 1) / 2
        }
    }
}

/// A few recently read blocks (neighbour lookups revisit the same ones).
struct BlockCache {
    keys: [(usize, usize); 4],
    vals: [Block; 4],
    next: usize,
}

impl BlockCache {
    fn new() -> BlockCache {
        BlockCache {
            keys: [(usize::MAX, 0); 4],
            vals: [Block::default(); 4],
            next: 0,
        }
    }

    fn get(&mut self, l: &Level, x: usize, y: usize) -> Block {
        for i in 0..4 {
            if self.keys[i] == (x, y) {
                return self.vals[i];
            }
        }
        let b = l.block(x, y);
        self.keys[self.next] = (x, y);
        self.vals[self.next] = b;
        self.next = (self.next + 1) & 3;
        b
    }
}

/// Decode the texels [x0, x1) × [y0, y1) of a level of `w`×`h` texels to RGBA8 in `out` (row
/// pitch `pitch`, starting at (x0, y0)). Missing blocks read as zero.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_rect(
    data: &[u8],
    w: u32,
    h: u32,
    two_bpp: bool,
    (x0, y0, x1, y1): (usize, usize, usize, usize),
    out: &mut [u8],
    pitch: usize,
) {
    let (nx, ny) = blocks(w, h, two_bpp);
    let l = Level {
        data,
        nx,
        ny,
        two_bpp,
    };
    let bw = block_width(two_bpp);
    let (hw, hh) = (bw / 2, 2);
    // Fixed-point scales: bilinear weights sum to bw × 4.
    let (cs, as_) = if two_bpp { (7, 5) } else { (6, 4) };
    let mut cache = BlockCache::new();
    // Windows between four block centres: window (wx, wy) has block (wx, wy) top-left and
    // covers texels from its centre to the centre of block (wx + 1, wy + 1).
    let wx0 = (x0 as isize - hw as isize).div_euclid(bw as isize);
    let mut wy = (y0 as isize - hh as isize).div_euclid(4);
    loop {
        let top = wy * 4 + hh as isize;
        if top >= y1 as isize {
            break;
        }
        let mut wx = wx0;
        loop {
            let left = wx * bw as isize + hw as isize;
            if left >= x1 as isize {
                break;
            }
            let bx = wx.rem_euclid(nx as isize) as usize;
            let by = wy.rem_euclid(ny as isize) as usize;
            let (bx1, by1) = ((bx + 1) % nx, (by + 1) % ny);
            let p = cache.get(&l, bx, by).colour;
            let q = cache.get(&l, bx1, by).colour;
            let r = cache.get(&l, bx, by1).colour;
            let s = cache.get(&l, bx1, by1).colour;
            let ca = [colour_a(p), colour_a(q), colour_a(r), colour_a(s)];
            let cb = [colour_b(p), colour_b(q), colour_b(r), colour_b(s)];
            for fy in 0..4isize {
                let y = top + fy;
                if y < y0 as isize || y >= y1 as isize {
                    continue;
                }
                for fx in 0..bw as isize {
                    let x = left + fx;
                    if x < x0 as isize || x >= x1 as isize {
                        continue;
                    }
                    let (gx, gy) = (fx as i32, fy as i32);
                    let bwi = bw as i32;
                    let lerp = |c: &[[i32; 4]; 4], k: usize| {
                        c[0][k] * (bwi - gx) * (4 - gy)
                            + c[1][k] * gx * (4 - gy)
                            + c[2][k] * (bwi - gx) * gy
                            + c[3][k] * gx * gy
                    };
                    // Upscaled colours in 8-bit-ish fixed point, as the 5/4-bit endpoints
                    // replicated: 5-bit c → (v >> cs) + (v >> (cs - 5)), alpha likewise.
                    let to8 = |c: &[[i32; 4]; 4]| {
                        let v = [lerp(c, 0), lerp(c, 1), lerp(c, 2), lerp(c, 3)];
                        [
                            (v[0] >> cs) + (v[0] >> (cs - 5)),
                            (v[1] >> cs) + (v[1] >> (cs - 5)),
                            (v[2] >> cs) + (v[2] >> (cs - 5)),
                            (v[3] >> as_) + (v[3] >> (as_ - 4)),
                        ]
                    };
                    let (a, b) = (to8(&ca), to8(&cb));
                    let mut m = l.weight(x as usize, y as usize, &mut cache);
                    let punch = m > 10;
                    if punch {
                        m -= 10;
                    }
                    let mix = |k: usize| ((a[k] * (8 - m) + b[k] * m) / 8) as u8;
                    let px = [mix(0), mix(1), mix(2), if punch { 0 } else { mix(3) }];
                    let o = (y as usize - y0) * pitch + (x as usize - x0) * 4;
                    out[o..o + 4].copy_from_slice(&px);
                }
            }
            wx += 1;
        }
        wy += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morton_order() {
        // Square: y in the even bits.
        assert_eq!(morton(4, 4, 1, 0), 2 * 8);
        assert_eq!(morton(4, 4, 0, 1), 8);
        // Wide (8×2): the low bits interleaved, then x's high bits.
        assert_eq!(morton(8, 2, 2, 0), 4 * 8);
        assert_eq!(morton(8, 2, 7, 1), 15 * 8);
    }

    /// Every PVRTC reference of `tools/corpus/texture/blocks.sh` (PowerVR SDK decodes),
    /// bit-exact, decoded whole and as a region not aligned to blocks.
    #[test]
    fn matches_powervr_sdk() {
        let mut n = 0;
        for dir in ["random", "files"] {
            for (mode, w, h, input, reference) in crate::block_refs(dir) {
                let two_bpp = match mode.as_str() {
                    "pvrtc2" => true,
                    "pvrtc4" => false,
                    _ => continue,
                };
                let (wu, hu) = (w as usize, h as usize);
                let mut out = vec![0u8; wu * hu * 4];
                decode_rect(&input, w, h, two_bpp, (0, 0, wu, hu), &mut out, wu * 4);
                let bad = out.iter().zip(&reference).filter(|(a, b)| a != b).count();
                assert_eq!(bad, 0, "{dir} {mode} {w}x{h}: {bad} bytes differ");
                let (x0, y0, x1, y1) = (wu / 3, hu / 3, wu - wu / 5, hu - hu / 7);
                let pitch = (x1 - x0) * 4;
                let mut part = vec![0u8; pitch * (y1 - y0)];
                decode_rect(&input, w, h, two_bpp, (x0, y0, x1, y1), &mut part, pitch);
                for y in y0..y1 {
                    assert_eq!(
                        &part[(y - y0) * pitch..][..pitch],
                        &reference[(y * wu + x0) * 4..][..pitch],
                        "{dir} {mode} {w}x{h}: region row {y}"
                    );
                }
                n += 1;
            }
        }
        eprintln!("{n} PVRTC references compared");
    }
}

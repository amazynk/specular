//! Frames and motion compensation (§8.3.6.5): bicubic and bilinear sub-pixel interpolation
//! from references whose out-of-bounds pixels repeat the edge (§8.3.2).

/// One plane, allocated at macroblock-aligned size.
#[derive(Clone)]
pub struct Plane {
    pub data: Vec<u8>,
    pub stride: usize,
    pub rows: usize,
}

impl Plane {
    pub fn new(stride: usize, rows: usize) -> Plane {
        Plane {
            data: vec![0; stride * rows],
            stride,
            rows,
        }
    }
}

/// A picture: Y, Cb, Cr at macroblock-aligned size.
#[derive(Clone)]
pub struct Frame {
    pub planes: [Plane; 3],
}

impl Frame {
    pub fn new(mb_w: usize, mb_h: usize) -> Frame {
        Frame {
            planes: [
                Plane::new(mb_w * 16, mb_h * 16),
                Plane::new(mb_w * 8, mb_h * 8),
                Plane::new(mb_w * 8, mb_h * 8),
            ],
        }
    }

    pub fn bytes(mb_w: usize, mb_h: usize) -> u64 {
        (mb_w * mb_h * 384) as u64
    }

    /// The frame with every sample mapped through `luts` (Y, then Cb and Cr).
    pub fn mapped(&self, luty: &[u8; 256], lutuv: &[u8; 256]) -> Frame {
        let map = |p: &Plane, lut: &[u8; 256]| Plane {
            data: p.data.iter().map(|&v| lut[v as usize]).collect(),
            stride: p.stride,
            rows: p.rows,
        };
        Frame {
            planes: [
                map(&self.planes[0], luty),
                map(&self.planes[1], lutuv),
                map(&self.planes[2], lutuv),
            ],
        }
    }
}

/// Where a reference's samples end for edge repetition, per plane: the visible size
/// (Advanced profile) or the macroblock-aligned one (Simple and Main).
#[derive(Clone, Copy)]
pub struct Edges {
    pub w: [usize; 3],
    pub h: [usize; 3],
}

/// Copies the `bw` × `bh` area at (`x`, `y`) of `p` to `out` (stride `bw`), repeating edge
/// samples outside `w` × `h`.
#[inline]
#[allow(clippy::too_many_arguments)]
fn fetch(p: &Plane, x: i32, y: i32, bw: usize, bh: usize, w: usize, h: usize, out: &mut [u8]) {
    let (wi, hi) = (w as i32, h as i32);
    if x >= 0 && y >= 0 && x + bw as i32 <= wi && y + bh as i32 <= hi {
        for r in 0..bh {
            let s = (y as usize + r) * p.stride + x as usize;
            out[r * bw..r * bw + bw].copy_from_slice(&p.data[s..s + bw]);
        }
        return;
    }
    for r in 0..bh {
        let sy = (y + r as i32).clamp(0, hi - 1) as usize;
        let row = &p.data[sy * p.stride..];
        for c in 0..bw {
            let sx = (x + c as i32).clamp(0, wi - 1) as usize;
            out[r * bw + c] = row[sx];
        }
    }
}

const TAPS: [[i32; 4]; 4] = [
    [0, 16, 0, 0],
    [-4, 53, 18, -3],
    [-1, 9, 9, -1],
    [-3, 18, 53, -4],
];
const SHIFT: [u32; 4] = [0, 6, 4, 6];

/// Predicts a `size` × `size` luma block at integer position (`x`, `y`) displaced by the
/// quarter-pel vector (`mx`, `my`), with the bicubic filter (or bilinear when `bilinear`).
#[allow(clippy::too_many_arguments)]
pub fn luma(
    p: &Plane,
    edges: &Edges,
    x: i32,
    y: i32,
    mx: i32,
    my: i32,
    size: usize,
    bilinear: bool,
    rnd: i32,
    out: &mut [u8],
    ostride: usize,
) {
    let ix = x + (mx >> 2);
    let iy = y + (my >> 2);
    let fx = (mx & 3) as usize;
    let fy = (my & 3) as usize;
    let mut src = [0u8; 19 * 19];
    let sw = size + 3;
    fetch(p, ix - 1, iy - 1, sw, sw, edges.w[0], edges.h[0], &mut src);
    let at = |r: usize, c: usize| i32::from(src[(r + 1) * sw + c + 1]);
    if fx == 0 && fy == 0 {
        for r in 0..size {
            for c in 0..size {
                out[r * ostride + c] = at(r, c) as u8;
            }
        }
        return;
    }
    if bilinear {
        bilinear_block(&src, sw, 1, fx, fy, size, size, rnd, out, ostride);
        return;
    }
    let tap = |r: isize, c: isize| i32::from(src[((r + 1) * sw as isize + c + 1) as usize]);
    if fy == 0 {
        let t = TAPS[fx];
        let sh = SHIFT[fx];
        let add = (1 << (sh - 1)) - rnd;
        for r in 0..size as isize {
            for c in 0..size as isize {
                let s = t[0] * tap(r, c - 1)
                    + t[1] * tap(r, c)
                    + t[2] * tap(r, c + 1)
                    + t[3] * tap(r, c + 2);
                out[r as usize * ostride + c as usize] = ((s + add) >> sh).clamp(0, 255) as u8;
            }
        }
        return;
    }
    if fx == 0 {
        let t = TAPS[fy];
        let sh = SHIFT[fy];
        let add = (1 << (sh - 1)) - 1 + rnd;
        for r in 0..size as isize {
            for c in 0..size as isize {
                let s = t[0] * tap(r - 1, c)
                    + t[1] * tap(r, c)
                    + t[2] * tap(r + 1, c)
                    + t[3] * tap(r + 2, c);
                out[r as usize * ostride + c as usize] = ((s + add) >> sh).clamp(0, 255) as u8;
            }
        }
        return;
    }
    // Vertical first over the columns the horizontal filter needs, then horizontal.
    let tv = TAPS[fy];
    let th = TAPS[fx];
    let shv = SHIFT[fy] + SHIFT[fx] - 7;
    let addv = (1 << (shv - 1)) - 1 + rnd;
    let mut tmp = [0i32; 16 * 19];
    for r in 0..size as isize {
        for c in -1..size as isize + 2 {
            let s = tv[0] * tap(r - 1, c)
                + tv[1] * tap(r, c)
                + tv[2] * tap(r + 1, c)
                + tv[3] * tap(r + 2, c);
            tmp[r as usize * sw + (c + 1) as usize] = (s + addv) >> shv;
        }
    }
    for r in 0..size {
        for c in 0..size {
            let t = &tmp[r * sw + c..];
            let s = th[0] * t[0] + th[1] * t[1] + th[2] * t[2] + th[3] * t[3];
            out[r * ostride + c] = ((s + 64 - rnd) >> 7).clamp(0, 255) as u8;
        }
    }
}

/// Bilinear interpolation of a block from `src` (stride `sw`, the block's origin at row and
/// column `off`), at quarter-sample shifts (`fx`, `fy`).
#[allow(clippy::too_many_arguments)]
fn bilinear_block(
    src: &[u8],
    sw: usize,
    off: usize,
    fx: usize,
    fy: usize,
    bw: usize,
    bh: usize,
    rnd: i32,
    out: &mut [u8],
    ostride: usize,
) {
    let (fx, fy) = (fx as i32, fy as i32);
    let w00 = (4 - fx) * (4 - fy);
    let w01 = fx * (4 - fy);
    let w10 = (4 - fx) * fy;
    let w11 = fx * fy;
    for r in 0..bh {
        let a = &src[(r + off) * sw + off..];
        let b = &src[(r + off + 1) * sw + off..];
        for c in 0..bw {
            let v = w00 * i32::from(a[c])
                + w01 * i32::from(a[c + 1])
                + w10 * i32::from(b[c])
                + w11 * i32::from(b[c + 1]);
            out[r * ostride + c] = ((v + 8 - rnd) >> 4) as u8;
        }
    }
}

/// Predicts an 8 × 8 chroma block of plane `pl` at (`x`, `y`) displaced by the quarter-pel
/// vector (`mx`, `my`), bilinear.
#[allow(clippy::too_many_arguments)]
pub fn chroma(
    p: &Plane,
    edges: &Edges,
    pl: usize,
    x: i32,
    y: i32,
    mx: i32,
    my: i32,
    rnd: i32,
    out: &mut [u8],
    ostride: usize,
) {
    let ix = x + (mx >> 2);
    let iy = y + (my >> 2);
    let fx = (mx & 3) as usize;
    let fy = (my & 3) as usize;
    let mut src = [0u8; 9 * 9];
    fetch(p, ix, iy, 9, 9, edges.w[pl], edges.h[pl], &mut src);
    if fx == 0 && fy == 0 {
        for r in 0..8 {
            out[r * ostride..r * ostride + 8].copy_from_slice(&src[r * 9..r * 9 + 8]);
        }
        return;
    }
    bilinear_block(&src, 9, 0, fx, fy, 8, 8, rnd, out, ostride);
}

/// Averages a second prediction into the first (B-picture interpolated and direct modes).
pub fn average(a: &mut [u8], b: &[u8]) {
    for (x, &y) in a.iter_mut().zip(b) {
        *x = ((u16::from(*x) + u16::from(y) + 1) >> 1) as u8;
    }
}

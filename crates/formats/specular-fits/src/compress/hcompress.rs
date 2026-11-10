//! `HCOMPRESS_1`: White's H-transform compression (White 1992, "High-performance compression of
//! astronomical images").
//!
//! The stream: the magic `DD 99`; the tile's slow and fast dimensions and the scale as
//! big-endian 32-bit integers; the sum of all pixels (64-bit); the number of bit planes of the
//! three coefficient groups (one byte each); then, as one MSB-first bitstream, the four
//! quadrants of the H-transform coefficient array, each as bit planes from the top, every plane
//! either direct 4-bit codes for each 2×2 cell or a quadtree of Huffman-coded 4-bit codes; a
//! zero nybble; and finally, from the next byte boundary, one sign bit per nonzero coefficient.
//!
//! Decoding multiplies the coefficients by the scale and inverts the H-transform level by level,
//! optionally smoothing the lossy result (the `SMOOTH` keyword) by adjusting the differences
//! within the bounds the quantisation allows. Arithmetic wraps rather than panics, so a corrupt
//! stream gives garbage values or an error but never a panic.

use super::CodecError;

/// The largest tile accepted (pixels).
const MAX_PIXELS: usize = 1 << 28;

/// A decoded `HCOMPRESS_1` tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hdecoded {
    /// Width: the FITS fast axis (`NAXIS1` / `ZTILE1`), which the stream calls `ny`.
    pub nx: usize,
    /// Height: the slow axis (`ZTILE2`), the stream's `nx`.
    pub ny: usize,
    /// The quantisation scale stored in the stream (0 or 1 for lossless).
    pub scale: i32,
    /// `nx · ny` values, row-major with the fast axis first.
    pub data: Vec<i64>,
}

/// Decodes one `HCOMPRESS_1` tile; `smooth` is the tile's `SMOOTH` parameter (only acts when
/// the scale is above 1). The caller checks `nx`/`ny` against the tile it expects.
pub fn hcompress(input: &[u8], smooth: bool) -> Result<Hdecoded, CodecError> {
    let mut r = Bits::new(input);
    if r.bytes::<2>() != Some([0xDD, 0x99]) {
        return Err(CodecError::Corrupt("HCOMPRESS magic missing"));
    }
    let header = (|| {
        let slow = i32::from_be_bytes(r.bytes()?);
        let fast = i32::from_be_bytes(r.bytes()?);
        let scale = i32::from_be_bytes(r.bytes()?);
        let sum = i64::from_be_bytes(r.bytes()?);
        let planes: [u8; 3] = r.bytes()?;
        Some((slow, fast, scale, sum, planes))
    })();
    let Some((slow, fast, scale, sum, planes)) = header else {
        return Err(CodecError::Corrupt("HCOMPRESS header truncated"));
    };
    let (Ok(nx), Ok(ny)) = (usize::try_from(slow), usize::try_from(fast)) else {
        return Err(CodecError::Corrupt("HCOMPRESS negative dimension"));
    };
    if nx == 0 || ny == 0 || nx.saturating_mul(ny) > MAX_PIXELS {
        return Err(CodecError::Corrupt("HCOMPRESS dimensions out of range"));
    }
    if planes.iter().any(|&p| p > 63) {
        return Err(CodecError::Corrupt("HCOMPRESS bit plane count above 63"));
    }

    // Stream order: `nx` rows (slow) of `ny` values (fast), as in the output.
    let mut a = vec![0i64; nx * ny];
    let (nx2, ny2) = (nx.div_ceil(2), ny.div_ceil(2));
    let mut scratch = Vec::new();
    for (start, qx, qy, p) in [
        (0, nx2, ny2, planes[0]),
        (ny2, nx2, ny / 2, planes[1]),
        (ny * nx2, nx / 2, ny2, planes[1]),
        (ny * nx2 + ny2, nx / 2, ny / 2, planes[2]),
    ] {
        qtree_decode(&mut r, &mut a[start..], ny, qx, qy, p, &mut scratch)?;
    }
    if r.nbits(4) != 0 {
        return Err(CodecError::Corrupt(
            "HCOMPRESS quadrants not followed by zero",
        ));
    }
    r.align();
    for v in &mut a {
        if *v != 0 && r.bit() != 0 {
            *v = v.wrapping_neg();
        }
    }
    if r.over {
        return Err(CodecError::Corrupt("HCOMPRESS stream truncated"));
    }
    a[0] = sum;

    if scale > 1 {
        let s = i64::from(scale);
        for v in &mut a {
            *v = v.wrapping_mul(s);
        }
    }
    hinv(&mut a, nx, ny, smooth, scale);
    Ok(Hdecoded {
        nx: ny,
        ny: nx,
        scale,
        data: a,
    })
}

/// MSB-first bits; reading past the end gives zeros and sets `over`.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    left: u32,
    over: bool,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits {
            data,
            pos: 0,
            buf: 0,
            left: 0,
            over: false,
        }
    }

    /// `N` whole bytes (only before any bits are read).
    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let b = self.data.get(self.pos..self.pos + N)?;
        self.pos += N;
        b.try_into().ok()
    }

    /// Drops the rest of the current byte.
    fn align(&mut self) {
        self.left = 0;
    }

    fn bit(&mut self) -> u32 {
        if self.left == 0 {
            match self.data.get(self.pos) {
                Some(&b) => self.buf = u32::from(b),
                None => {
                    self.over = true;
                    self.buf = 0;
                }
            }
            self.pos += 1;
            self.left = 8;
        }
        self.left -= 1;
        (self.buf >> self.left) & 1
    }

    /// The next `n` (≤ 8) bits.
    fn nbits(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |v, _| v << 1 | self.bit())
    }

    /// One quadtree code: 3 to 6 bits for a 4-bit value.
    fn huffman(&mut self) -> u8 {
        let c = self.nbits(3);
        if c < 4 {
            return 1 << c;
        }
        let c = c << 1 | self.bit();
        if c < 13 {
            return [3, 5, 10, 12, 15][c as usize - 8];
        }
        let c = c << 1 | self.bit();
        if c < 31 {
            return [6, 7, 9, 11, 13][c as usize - 26];
        }
        if (c << 1 | self.bit()) == 62 { 0 } else { 14 }
    }
}

/// Smallest `k` with `2^k ≥ n` (0 for n ≤ 1).
fn log2_ceil(n: usize) -> u32 {
    n.max(1).next_power_of_two().trailing_zeros()
}

/// Decodes the bit planes of one `qx × qy` quadrant whose rows start `stride` apart in `a`.
fn qtree_decode(
    r: &mut Bits<'_>,
    a: &mut [i64],
    stride: usize,
    qx: usize,
    qy: usize,
    planes: u8,
    scratch: &mut Vec<u8>,
) -> Result<(), CodecError> {
    let log2n = log2_ceil(qx.max(qy));
    let (qx2, qy2) = (qx.div_ceil(2), qy.div_ceil(2));
    let mut next = Vec::new();
    for bit in (0..u32::from(planes)).rev() {
        scratch.clear();
        match r.nbits(4) {
            0 => scratch.extend((0..qx2 * qy2).map(|_| r.nbits(4) as u8)),
            0xF => {
                scratch.push(r.huffman());
                // Grow the code array level by level to qx2 × qy2.
                let (mut nx, mut ny, mut fx, mut fy) = (1usize, 1usize, qx, qy);
                let mut c = 1usize << log2n;
                for _ in 1..log2n {
                    c >>= 1;
                    nx <<= 1;
                    ny <<= 1;
                    if fx <= c {
                        nx = nx.saturating_sub(1);
                    } else {
                        fx -= c;
                    }
                    if fy <= c {
                        ny = ny.saturating_sub(1);
                    } else {
                        fy -= c;
                    }
                    expand(r, scratch, nx, ny, &mut next);
                    std::mem::swap(scratch, &mut next);
                }
            }
            _ => return Err(CodecError::Corrupt("HCOMPRESS bad quadtree plane code")),
        }
        if r.over {
            return Err(CodecError::Corrupt("HCOMPRESS stream truncated"));
        }
        if scratch.len() < qx2 * qy2 {
            return Err(CodecError::Corrupt("HCOMPRESS quadtree size mismatch"));
        }
        // Each code covers a 2×2 cell: bit 3 at (0, 0), 2 at (0, 1), 1 at (1, 0), 0 at (1, 1).
        for i in 0..qx {
            let row = &mut a[i * stride..i * stride + qy];
            let codes = &scratch[(i / 2) * qy2..];
            let hi = if i % 2 == 0 { 3 } else { 1 };
            for (j, v) in row.iter_mut().enumerate() {
                let sh = hi - (j % 2) as u32;
                *v |= i64::from((codes[j / 2] >> sh) & 1) << bit;
            }
        }
    }
    Ok(())
}

/// Spreads the 4-bit codes of `codes` (`⌈nx/2⌉ × ⌈ny/2⌉`) into one bit per cell of an
/// `nx × ny` array, then replaces every 1 by the next Huffman code (read last cell first).
fn expand(r: &mut Bits<'_>, codes: &[u8], nx: usize, ny: usize, out: &mut Vec<u8>) {
    let ny2 = ny.div_ceil(2);
    out.clear();
    out.extend((0..nx * ny).map(|k| {
        let (i, j) = (k / ny, k % ny);
        let sh = if i % 2 == 0 { 3 } else { 1 } - (j % 2) as u32;
        (codes[(i / 2) * ny2 + j / 2] >> sh) & 1
    }));
    for v in out.iter_mut().rev() {
        if *v != 0 {
            *v = r.huffman();
        }
    }
}

#[inline]
fn round(h: i64, prnd: i64, nrnd: i64, mask: i64) -> i64 {
    h.wrapping_add(if h >= 0 { prnd } else { nrnd }) & mask
}

/// Interleaves `a[0]`, `a[step]`, … so that the first half lands on the even and the second
/// half on the odd positions.
fn unshuffle(a: &mut [i64], n: usize, step: usize, tmp: &mut Vec<i64>) {
    tmp.clear();
    tmp.extend((0..n).map(|i| a[i * step]));
    let half = n.div_ceil(2);
    for i in 0..n {
        a[i * step] = tmp[if i % 2 == 0 { i / 2 } else { half + i / 2 }];
    }
}

/// Inverse H-transform of `a` (`nx` rows of `ny`) in place.
fn hinv(a: &mut [i64], nx: usize, ny: usize, smooth: bool, scale: i32) {
    let log2n = log2_ceil(nx.max(ny));
    if log2n == 0 {
        return;
    }
    let mut tmp = Vec::with_capacity(nx.max(ny));
    let mut shift = 1;
    let mut bit0 = 1i64 << (log2n - 1);
    let mut bit1 = bit0 << 1;
    let mut mask0 = -bit0;
    let mut mask1 = mask0 << 1;
    let mask2 = mask0 << 2;
    let mut prnd0 = bit0 >> 1;
    let mut prnd1 = bit1 >> 1;
    let prnd2 = bit1;
    let mut nrnd0 = prnd0 - 1;
    let mut nrnd1 = prnd1 - 1;
    let nrnd2 = prnd2 - 1;
    a[0] = round(a[0], prnd2, nrnd2, mask2);

    let (mut nxtop, mut nytop, mut nxf, mut nyf) = (1, 1, nx, ny);
    let mut c = 1usize << log2n;
    for k in (0..log2n).rev() {
        c >>= 1;
        nxtop <<= 1;
        nytop <<= 1;
        if nxf <= c {
            nxtop -= 1;
        } else {
            nxf -= c;
        }
        if nyf <= c {
            nytop -= 1;
        } else {
            nyf -= c;
        }
        if k == 0 {
            nrnd0 = 0;
            shift = 2;
        }
        for i in 0..nxtop {
            unshuffle(&mut a[ny * i..], nytop, 1, &mut tmp);
        }
        for j in 0..nytop {
            unshuffle(&mut a[j..], nxtop, ny, &mut tmp);
        }
        if smooth {
            hsmooth(a, nxtop, nytop, ny, scale);
        }
        let (oddx, oddy) = (nxtop % 2, nytop % 2);
        let mut i = 0;
        while i < nxtop - oddx {
            let mut s00 = ny * i;
            let mut s10 = s00 + ny;
            let mut j = 0;
            while j < nytop - oddy {
                let mut h0 = a[s00];
                let mut hx = round(a[s10], prnd1, nrnd1, mask1);
                let mut hy = round(a[s00 + 1], prnd1, nrnd1, mask1);
                let hc = round(a[s10 + 1], prnd0, nrnd0, mask0);
                // The lowest bits of h0, hx and hy follow from hc's.
                let lowbit0 = hc & bit0;
                hx = if hx >= 0 {
                    hx.wrapping_sub(lowbit0)
                } else {
                    hx.wrapping_add(lowbit0)
                };
                hy = if hy >= 0 {
                    hy.wrapping_sub(lowbit0)
                } else {
                    hy.wrapping_add(lowbit0)
                };
                let lowbit1 = (hc ^ hx ^ hy) & bit1;
                h0 = if h0 >= 0 {
                    h0.wrapping_add(lowbit0 - lowbit1)
                } else if lowbit0 == 0 {
                    h0.wrapping_add(lowbit1)
                } else {
                    h0.wrapping_add(lowbit0 - lowbit1)
                };
                let (p, m) = (h0.wrapping_add(hx), h0.wrapping_sub(hx));
                a[s10 + 1] = p.wrapping_add(hy).wrapping_add(hc) >> shift;
                a[s10] = p.wrapping_sub(hy).wrapping_sub(hc) >> shift;
                a[s00 + 1] = m.wrapping_add(hy).wrapping_sub(hc) >> shift;
                a[s00] = m.wrapping_sub(hy).wrapping_add(hc) >> shift;
                s00 += 2;
                s10 += 2;
                j += 2;
            }
            if oddy == 1 {
                // Last column alone: only h0 and hx.
                let hx = round(a[s10], prnd1, nrnd1, mask1);
                let lowbit1 = hx & bit1;
                let h0 = a[s00];
                let h0 = if h0 >= 0 {
                    h0.wrapping_sub(lowbit1)
                } else {
                    h0.wrapping_add(lowbit1)
                };
                a[s10] = h0.wrapping_add(hx) >> shift;
                a[s00] = h0.wrapping_sub(hx) >> shift;
            }
            i += 2;
        }
        if oddx == 1 {
            // Last row alone: only h0 and hy.
            let mut s00 = ny * i;
            let mut j = 0;
            while j < nytop - oddy {
                let hy = round(a[s00 + 1], prnd1, nrnd1, mask1);
                let lowbit1 = hy & bit1;
                let h0 = a[s00];
                let h0 = if h0 >= 0 {
                    h0.wrapping_sub(lowbit1)
                } else {
                    h0.wrapping_add(lowbit1)
                };
                a[s00 + 1] = h0.wrapping_add(hy) >> shift;
                a[s00] = h0.wrapping_sub(hy) >> shift;
                s00 += 2;
                j += 2;
            }
            if oddy == 1 {
                a[s00] >>= shift;
            }
        }
        // Next level: one bit lower.
        bit1 = bit0;
        bit0 >>= 1;
        mask1 = mask0;
        mask0 >>= 1;
        prnd1 = prnd0;
        prnd0 >>= 1;
        nrnd1 = nrnd0;
        nrnd0 = prnd0 - 1;
    }
}

/// Smooths the differences of one level (`nxtop × nytop` of rows `ny` apart) before they are
/// applied: each moves towards the value its neighbouring sums suggest, by at most half the
/// quantisation step and only where those neighbours are monotonic.
fn hsmooth(a: &mut [i64], nxtop: usize, nytop: usize, ny: usize, scale: i32) {
    let smax = i64::from(scale >> 1);
    if smax <= 0 {
        return;
    }
    let ny2 = ny << 1;
    // Bounds on a difference from the slopes either side of h0 (`dmin < dmax` when monotonic).
    let bounds = |hm: i64, h0: i64, hp: i64| {
        let (u, d) = (hp.wrapping_sub(h0), h0.wrapping_sub(hm));
        (u.max(d).min(0) << 2, u.min(d).max(0) << 2)
    };
    let adjust = |v: &mut i64, target: i64, sh: u32| {
        let s = target.wrapping_sub(*v << sh);
        let s = if s >= 0 {
            s >> sh
        } else {
            s.wrapping_add((1 << sh) - 1) >> sh
        };
        *v = v.wrapping_add(s.clamp(-smax, smax));
    };
    // x differences (hx, one row below h0).
    for i in (2..nxtop.saturating_sub(2)).step_by(2) {
        for j in (0..nytop).step_by(2) {
            let s00 = ny * i + j;
            let (hm, h0, hp) = (a[s00 - ny2], a[s00], a[s00 + ny2]);
            let (dmin, dmax) = bounds(hm, h0, hp);
            if dmin < dmax {
                adjust(&mut a[s00 + ny], hp.wrapping_sub(hm).clamp(dmin, dmax), 3);
            }
        }
    }
    // y differences (hy, one column right of h0).
    for i in (0..nxtop).step_by(2) {
        for j in (2..nytop.saturating_sub(2)).step_by(2) {
            let s00 = ny * i + j;
            let (hm, h0, hp) = (a[s00 - 2], a[s00], a[s00 + 2]);
            let (dmin, dmax) = bounds(hm, h0, hp);
            if dmin < dmax {
                adjust(&mut a[s00 + 1], hp.wrapping_sub(hm).clamp(dmin, dmax), 3);
            }
        }
    }
    // Curvature (hc), bounded through the four diagonal neighbours.
    for i in (2..nxtop.saturating_sub(2)).step_by(2) {
        for j in (2..nytop.saturating_sub(2)).step_by(2) {
            let s00 = ny * i + j;
            let s10 = s00 + ny;
            let h0 = a[s00];
            let (hmm, hpm) = (a[s00 - ny2 - 2], a[s00 + ny2 - 2]);
            let (hmp, hpp) = (a[s00 - ny2 + 2], a[s00 + ny2 + 2]);
            let diff = hpp.wrapping_add(hmm).wrapping_sub(hmp).wrapping_sub(hpm);
            // Twice the cell's x and y slopes.
            let (hx2, hy2) = (a[s10] << 1, a[s00 + 1] << 1);
            let (nhx2, nhy2) = (hx2.wrapping_neg(), hy2.wrapping_neg());
            let e = |v: i64, sx: i64, sy: i64| v.wrapping_add(sx).wrapping_add(sy);
            let (pp, pm) = (hpp.wrapping_sub(h0), h0.wrapping_sub(hpm));
            let (mp, mm) = (h0.wrapping_sub(hmp), hmm.wrapping_sub(h0));
            let m1 = e(pp.max(0), nhx2, nhy2).min(e(pm.max(0), hx2, nhy2));
            let m2 = e(mp.max(0), nhx2, hy2).min(e(mm.max(0), hx2, hy2));
            let dmax = m1.min(m2) << 4;
            let m1 = e(pp.min(0), nhx2, nhy2).max(e(pm.min(0), hx2, nhy2));
            let m2 = e(mp.min(0), nhx2, hy2).max(e(mm.min(0), hx2, hy2));
            let dmin = m1.max(m2) << 4;
            if dmin < dmax {
                adjust(&mut a[s10 + 1], diff.clamp(dmin, dmax), 6);
            }
        }
    }
}

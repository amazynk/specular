//! Box-filter downscaling of streamed rows.
//!
//! 8-bit data is averaged in linear light (sRGB transfer curve) with premultiplied alpha, so
//! neither dark fringes around transparency nor darkened fine detail appear. 16-bit data is
//! averaged as stored (premultiplied by alpha, if any), keeping it exact for HDR transfer
//! functions.

use std::sync::OnceLock;

use crate::convert::Packed;

/// sRGB 8-bit -> linear 16-bit.
fn to_linear() -> &'static [u16; 256] {
    static T: OnceLock<[u16; 256]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u16; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f64 / 255.0;
            let l = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            *v = (l * 65535.0 + 0.5) as u16;
        }
        t
    })
}

/// Linear 16-bit -> sRGB 8-bit.
fn to_srgb() -> &'static [u8] {
    static T: OnceLock<Box<[u8]>> = OnceLock::new();
    T.get_or_init(|| {
        // Each 8-bit code covers the linear interval between the midpoints to its neighbours.
        let lin = to_linear();
        let mut t = vec![0u8; 65536].into_boxed_slice();
        let mut code = 0usize;
        for (l, v) in t.iter_mut().enumerate() {
            while code < 255 && (l as u32) * 2 > u32::from(lin[code]) + u32::from(lin[code + 1]) {
                code += 1;
            }
            *v = code as u8;
        }
        t
    })
}

pub(crate) struct Downscaler {
    s: usize,
    in_w: usize,
    out_w: usize,
    packed: Packed,
    acc: Vec<u64>,
    rows: usize,
    lin: &'static [u16; 256],
    srgb: &'static [u8],
}

impl Downscaler {
    pub(crate) fn new(packed: Packed, in_w: usize, s: usize) -> Downscaler {
        let out_w = in_w.div_ceil(s);
        Downscaler {
            s,
            in_w,
            out_w,
            packed,
            acc: vec![0; out_w * packed.channels()],
            rows: 0,
            lin: to_linear(),
            srgb: to_srgb(),
        }
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    /// Add one packed row of `in_w` pixels.
    pub(crate) fn add(&mut self, row: &[u8]) {
        let (s, lin, acc) = (self.s, self.lin, &mut self.acc[..]);
        match self.packed {
            Packed::Gray8 => add_plain::<1>(acc, &row[..self.in_w], s, lin),
            Packed::Rgb8 => add_plain::<3>(acc, &row[..self.in_w * 3], s, lin),
            Packed::Rgba8 => {
                // Per output pixel the partial sums of one row fit in u32 up to s = 257.
                for (acc, px) in acc
                    .chunks_exact_mut(4)
                    .zip(row[..self.in_w * 4].chunks(s * 4))
                {
                    let mut t = [0u64; 4];
                    for p in px.chunks_exact(4) {
                        let a = u32::from(p[3]);
                        t[0] += u64::from(u32::from(lin[p[0] as usize]) * a);
                        t[1] += u64::from(u32::from(lin[p[1] as usize]) * a);
                        t[2] += u64::from(u32::from(lin[p[2] as usize]) * a);
                        t[3] += u64::from(a);
                    }
                    for c in 0..4 {
                        acc[c] += t[c];
                    }
                }
            }
            Packed::Rgba16 => {
                let row = &row[..self.in_w * 8];
                for (acc, px) in acc.chunks_exact_mut(4).zip(row.chunks(s * 8)) {
                    for p in px.chunks_exact(8) {
                        let v = |c: usize| u64::from(u16::from_ne_bytes([p[c * 2], p[c * 2 + 1]]));
                        let a = v(3);
                        acc[0] += v(0) * a;
                        acc[1] += v(1) * a;
                        acc[2] += v(2) * a;
                        acc[3] += a;
                    }
                }
            }
            Packed::Gray16 => {
                let row = &row[..self.in_w * 2];
                for (acc, px) in acc.iter_mut().zip(row.chunks(s * 2)) {
                    for p in px.chunks_exact(2) {
                        *acc += u64::from(u16::from_ne_bytes([p[0], p[1]]));
                    }
                }
            }
            Packed::GrayA16 => {
                let row = &row[..self.in_w * 4];
                for (acc, px) in acc.chunks_exact_mut(2).zip(row.chunks(s * 4)) {
                    for p in px.chunks_exact(4) {
                        let a = u64::from(u16::from_ne_bytes([p[2], p[3]]));
                        acc[0] += u64::from(u16::from_ne_bytes([p[0], p[1]])) * a;
                        acc[1] += a;
                    }
                }
            }
        }
        self.rows += 1;
    }

    /// Add this downscaler's sums to `coarse`, whose factor is a multiple of this one's (same
    /// input width and layout). The sums are exact integers, so a coarse level built from a
    /// finer one is identical to one built from the input rows.
    pub(crate) fn fold_into(&self, coarse: &mut Downscaler) {
        let k = coarse.s / self.s;
        let ch = self.packed.channels();
        for (dst, src) in coarse.acc.chunks_exact_mut(ch).zip(self.acc.chunks(k * ch)) {
            for px in src.chunks_exact(ch) {
                for c in 0..ch {
                    dst[c] += px[c];
                }
            }
        }
        coarse.rows += self.rows;
    }

    /// Write the averaged packed row (`out_w` pixels) and reset. Call after `s` rows (or fewer
    /// at the bottom edge).
    pub(crate) fn emit(&mut self, out: &mut [u8]) {
        let rows = self.rows.max(1) as u64;
        let srgb = self.srgb;
        let (s, out_w) = (self.s as u64, self.out_w);
        let last_cols = (self.in_w - (out_w - 1) * self.s) as u64;
        // Division by multiplication with the reciprocal: rounding to nearest, the same as the
        // integer `(acc + n / 2) / n` but for exact ties (within f64 precision). Through i64:
        // x86-64 converts signed integers to and from f64 in one instruction, u64 takes a dozen
        // (sums stay far below 2^63).
        let inv_full = 1.0 / (s * rows) as f64;
        let inv_last = 1.0 / (last_cols * rows) as f64;
        let inv_n = |x: usize| if x + 1 == out_w { inv_last } else { inv_full };
        let div = |v: u64, inv: f64| ((v as i64) as f64 * inv + 0.5) as i64 as usize;
        let lut = |v: u64, inv: f64| srgb[div(v, inv).min(65535)];
        match self.packed {
            Packed::Gray8 => {
                for (x, (o, a)) in out[..out_w].iter_mut().zip(&self.acc).enumerate() {
                    *o = lut(*a, inv_n(x));
                }
            }
            Packed::Rgb8 => {
                let out = &mut out[..out_w * 3];
                for (x, (o, a)) in out
                    .chunks_exact_mut(3)
                    .zip(self.acc.chunks_exact(3))
                    .enumerate()
                {
                    let inv = inv_n(x);
                    o[0] = lut(a[0], inv);
                    o[1] = lut(a[1], inv);
                    o[2] = lut(a[2], inv);
                }
            }
            Packed::Rgba8 => {
                let out = &mut out[..out_w * 4];
                for (x, (o, a)) in out
                    .chunks_exact_mut(4)
                    .zip(self.acc.chunks_exact(4))
                    .enumerate()
                {
                    if a[3] == 0 {
                        o.fill(0);
                    } else {
                        let inv_a = 1.0 / (a[3] as i64) as f64;
                        o[0] = lut(a[0], inv_a);
                        o[1] = lut(a[1], inv_a);
                        o[2] = lut(a[2], inv_a);
                        o[3] = div(a[3], inv_n(x)).min(255) as u8;
                    }
                }
            }
            Packed::Rgba16 => {
                for (x, acc) in self.acc.chunks_exact(4).enumerate() {
                    let a = acc[3];
                    for c in 0..3 {
                        let v = (acc[c] + a / 2).checked_div(a).map_or(0, |v| v as u16);
                        out[x * 8 + c * 2..x * 8 + c * 2 + 2].copy_from_slice(&v.to_ne_bytes());
                    }
                    let n = if x + 1 == out_w { last_cols } else { s } * rows;
                    let v = ((a + n / 2) / n) as u16;
                    out[x * 8 + 6..x * 8 + 8].copy_from_slice(&v.to_ne_bytes());
                }
            }
            Packed::Gray16 => {
                for (x, &acc) in self.acc.iter().enumerate() {
                    let n = if x + 1 == out_w { last_cols } else { s } * rows;
                    let v = ((acc + n / 2) / n) as u16;
                    out[x * 2..x * 2 + 2].copy_from_slice(&v.to_ne_bytes());
                }
            }
            Packed::GrayA16 => {
                for (x, acc) in self.acc.chunks_exact(2).enumerate() {
                    let a = acc[1];
                    let g = (acc[0] + a / 2).checked_div(a).map_or(0, |v| v as u16);
                    let n = if x + 1 == out_w { last_cols } else { s } * rows;
                    let v = ((a + n / 2) / n) as u16;
                    out[x * 4..x * 4 + 2].copy_from_slice(&g.to_ne_bytes());
                    out[x * 4 + 2..x * 4 + 4].copy_from_slice(&v.to_ne_bytes());
                }
            }
        }
        self.acc.fill(0);
        self.rows = 0;
    }
}

/// Sum `N`-channel 8-bit pixels in linear light, `s` input pixels per output pixel.
#[inline(always)]
fn add_plain<const N: usize>(acc: &mut [u64], row: &[u8], s: usize, lin: &[u16; 256]) {
    for (acc, px) in acc.chunks_exact_mut(N).zip(row.chunks(s * N)) {
        // One row's partial sums fit in u32 up to s = 65537.
        let mut t = [0u32; N];
        for p in px.chunks_exact(N) {
            for c in 0..N {
                t[c] += u32::from(lin[p[c] as usize]);
            }
        }
        for c in 0..N {
            acc[c] += u64::from(t[c]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luts_round_trip() {
        let (lin, srgb) = (to_linear(), to_srgb());
        for i in 0..256 {
            assert_eq!(srgb[lin[i] as usize] as usize, i);
        }
    }

    #[test]
    fn averages() {
        let mut d = Downscaler::new(Packed::Rgba8, 3, 2);
        d.add(&[255, 0, 0, 255, 255, 0, 0, 255, 9, 9, 9, 0]);
        d.add(&[255, 0, 0, 255, 255, 0, 0, 0, 9, 9, 9, 0]);
        let mut out = [0u8; 8];
        d.emit(&mut out);
        // Transparent pixels do not darken; alpha is the plain average.
        assert_eq!(&out[..4], &[255, 0, 0, 191]);
        assert_eq!(out[7], 0);
    }

    #[test]
    fn averages_grey16() {
        let px = |v: &[u16]| v.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>();
        let mut d = Downscaler::new(Packed::Gray16, 3, 2);
        d.add(&px(&[1000, 2001, 50000]));
        d.add(&px(&[3000, 4000, 60000]));
        let mut out = [0u8; 4];
        d.emit(&mut out);
        // Averaged as stored, rounded; the last column averages what is there.
        assert_eq!(out, *px(&[2500, 55000]));

        let mut d = Downscaler::new(Packed::GrayA16, 2, 2);
        d.add(&px(&[40000, 65535, 9, 0]));
        d.add(&px(&[20000, 65535, 9, 0]));
        let mut out = [0u8; 4];
        d.emit(&mut out);
        // Transparent pixels do not darken; alpha is the plain average.
        assert_eq!(out, *px(&[30000, 32768]));
    }
}

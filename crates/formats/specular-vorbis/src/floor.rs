//! Floors (spec sections 6 and 7): the spectral envelope each channel's residue is scaled by.

use crate::Error;
use crate::bits::{BitReader, ilog};
use crate::codebook::Codebook;
use crate::tables::INVERSE_DB;

/// The most floor 1 X values a setup may declare.
pub(crate) const FLOOR1_MAX_VALUES: usize = 65;
/// The most floor 0 LSP coefficients (an 8-bit order).
pub(crate) const FLOOR0_MAX_ORDER: usize = 255;

fn malformed(s: &str) -> Error {
    Error::Malformed(s.to_string())
}

fn eop() -> Error {
    malformed("the setup header ends early")
}

/// Reads a book number that must name one of the `count` books.
fn book_number(br: &mut BitReader, count: usize) -> Result<u8, Error> {
    let b = br.read(8).ok_or_else(eop)?;
    if b as usize >= count {
        return Err(malformed("a floor names a missing codebook"));
    }
    Ok(b as u8)
}

pub(crate) enum Floor {
    Zero(Floor0),
    One(Box<Floor1>),
}

/// Floor type 0: an LSP filter's frequency response on a Bark scale.
pub(crate) struct Floor0 {
    order: usize,
    amp_bits: u32,
    amp_offset: u32,
    books: Vec<u8>,
    bark_map_size: u32,
    /// The Bark map for each block size's n/2 (short, long); ends with -1.
    maps: [Vec<i32>; 2],
}

/// Floor type 1: a piecewise linear curve in dB.
pub(crate) struct Floor1 {
    partition_class: Vec<u8>,
    class_dims: [u8; 16],
    class_subclasses: [u8; 16],
    class_master: [u8; 16],
    subclass_books: [[i16; 8]; 16],
    multiplier: i32,
    xs: Vec<i32>,
    /// Indices of `xs` in ascending X order.
    sorted: Vec<u8>,
    low: Vec<u8>,
    high: Vec<u8>,
}

impl Floor {
    /// Reads one floor configuration. `halves` are the two block sizes' n/2.
    pub(crate) fn parse(
        br: &mut BitReader,
        books: &[Codebook],
        halves: [usize; 2],
    ) -> Result<Floor, Error> {
        match br.read(16).ok_or_else(eop)? {
            0 => Floor0::parse(br, books, halves).map(Floor::Zero),
            1 => Floor1::parse(br, books).map(|f| Floor::One(Box::new(f))),
            _ => Err(Error::Unsupported(
                "Vorbis with a reserved floor type".to_string(),
            )),
        }
    }
}

/// `bark(x)` with the 2015 erratum's parenthesis.
fn bark(x: f64) -> f64 {
    13.1 * (0.00074 * x).atan() + 2.24 * (0.000_000_018_5 * x * x).atan() + 0.0001 * x
}

impl Floor0 {
    fn parse(br: &mut BitReader, books: &[Codebook], halves: [usize; 2]) -> Result<Floor0, Error> {
        let order = br.read(8).ok_or_else(eop)? as usize;
        let rate = br.read(16).ok_or_else(eop)?;
        let bark_map_size = br.read(16).ok_or_else(eop)?;
        let amp_bits = br.read(6).ok_or_else(eop)?;
        let amp_offset = br.read(8).ok_or_else(eop)?;
        let nbooks = br.read(4).ok_or_else(eop)? + 1;
        let mut list = Vec::new();
        for _ in 0..nbooks {
            let b = book_number(br, books.len())?;
            let book = &books[b as usize];
            if book.values.is_none() || book.dims == 0 {
                return Err(malformed("a floor 0 codebook has no values"));
            }
            list.push(b);
        }
        if rate == 0 || bark_map_size == 0 {
            return Err(malformed("a floor 0 with no rate or Bark map"));
        }
        let map = |n: usize| -> Vec<i32> {
            let scale = f64::from(bark_map_size) / bark(0.5 * f64::from(rate));
            let mut m: Vec<i32> = (0..n)
                .map(|i| {
                    let b = (bark(f64::from(rate) * i as f64 / (2.0 * n as f64)) * scale).floor();
                    (b as i64).min(i64::from(bark_map_size) - 1) as i32
                })
                .collect();
            m.push(-1);
            m
        };
        Ok(Floor0 {
            order,
            amp_bits,
            amp_offset,
            books: list,
            bark_map_size,
            maps: [map(halves[0]), map(halves[1])],
        })
    }

    /// Reads the amplitude and coefficients into `coefs` (at least `order` long). None:
    /// the channel is unused this frame (also at the end of the packet).
    pub(crate) fn decode(
        &self,
        br: &mut BitReader,
        books: &[Codebook],
        coefs: &mut [f32],
    ) -> Result<Option<u64>, Error> {
        let amp = if self.amp_bits > 32 {
            let lo = br.read(32);
            let hi = br.read(self.amp_bits - 32);
            match (lo, hi) {
                (Some(lo), Some(hi)) => u64::from(lo) | u64::from(hi) << 32,
                _ => return Ok(None),
            }
        } else {
            match br.read(self.amp_bits) {
                Some(a) => u64::from(a),
                None => return Ok(None),
            }
        };
        if amp == 0 {
            return Ok(None);
        }
        let Some(n) = br.read(ilog(self.books.len() as u32)) else {
            return Ok(None);
        };
        let Some(&b) = self.books.get(n as usize) else {
            return Err(malformed("a floor 0 book number is out of range"));
        };
        let book = &books[b as usize];
        let mut have = 0;
        let mut last = 0f32;
        loop {
            let Some(v) = book.decode_vq(br) else {
                return Ok(None);
            };
            for &x in v {
                if have < self.order {
                    coefs[have] = x + last;
                    have += 1;
                }
            }
            last += v.last().copied().unwrap_or(0.0);
            if have >= self.order {
                break;
            }
        }
        Ok(Some(amp))
    }

    /// Multiplies `v` (n/2 long) by the curve.
    pub(crate) fn apply(&self, amp: u64, coefs: &[f32], v: &mut [f32], long: bool) {
        let map = &self.maps[usize::from(long)];
        let order = self.order;
        let cos: Vec<f64> = coefs[..order].iter().map(|&c| f64::from(c).cos()).collect();
        let ampf = amp as f64;
        let offset = f64::from(self.amp_offset);
        let amp_max = 2f64.powi(self.amp_bits as i32) - 1.0;
        let mut i = 0;
        while i < v.len() {
            let m = map[i];
            let w = std::f64::consts::PI * f64::from(m) / f64::from(self.bark_map_size);
            let cw = w.cos();
            let (mut p, mut q);
            if order % 2 == 1 {
                p = 1.0 - cw * cw;
                q = 0.25;
            } else {
                p = (1.0 - cw) / 2.0;
                q = (1.0 + cw) / 2.0;
            }
            for (j, c) in cos.iter().enumerate() {
                let d = 4.0 * (c - cw) * (c - cw);
                if j % 2 == 1 {
                    p *= d;
                } else {
                    q *= d;
                }
            }
            let lin = (0.115_129_25 * (ampf * offset / (amp_max * (p + q).sqrt()) - offset)).exp();
            let lin = lin as f32;
            while i < v.len() && map[i] == m {
                v[i] *= lin;
                i += 1;
            }
        }
    }
}

/// `render_point` (spec 9.2.6).
fn render_point(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> i32 {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let off = dy.abs() * (x - x0) / adx;
    if dy < 0 { y0 - off } else { y0 + off }
}

/// `render_line` (spec 9.2.7) over [x0, x1), multiplying `v` by the dB table instead of
/// storing.
#[inline]
fn render_line(x0: i32, y0: i32, x1: i32, y1: i32, v: &mut [f32]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let base = dy / adx;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let ady = dy.abs() - base.abs() * adx;
    let end = (x1 as usize).min(v.len());
    let x0 = x0 as usize;
    if x0 >= end {
        return;
    }
    let mut y = y0;
    let mut err = 0;
    v[x0] *= INVERSE_DB[y.clamp(0, 255) as usize];
    for s in &mut v[x0 + 1..end] {
        err += ady;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
        *s *= INVERSE_DB[y.clamp(0, 255) as usize];
    }
}

const RANGES: [i32; 4] = [256, 128, 86, 64];

impl Floor1 {
    fn parse(br: &mut BitReader, books: &[Codebook]) -> Result<Floor1, Error> {
        let partitions = br.read(5).ok_or_else(eop)? as usize;
        let mut partition_class = Vec::with_capacity(partitions);
        for _ in 0..partitions {
            partition_class.push(br.read(4).ok_or_else(eop)? as u8);
        }
        let classes = partition_class
            .iter()
            .map(|&c| usize::from(c) + 1)
            .max()
            .unwrap_or(0);
        let mut f = Floor1 {
            partition_class,
            class_dims: [0; 16],
            class_subclasses: [0; 16],
            class_master: [0; 16],
            subclass_books: [[-1; 8]; 16],
            multiplier: 1,
            xs: Vec::new(),
            sorted: Vec::new(),
            low: Vec::new(),
            high: Vec::new(),
        };
        for c in 0..classes {
            f.class_dims[c] = br.read(3).ok_or_else(eop)? as u8 + 1;
            f.class_subclasses[c] = br.read(2).ok_or_else(eop)? as u8;
            if f.class_subclasses[c] != 0 {
                f.class_master[c] = book_number(br, books.len())?;
            }
            for j in 0..1usize << f.class_subclasses[c] {
                let b = br.read(8).ok_or_else(eop)? as i16 - 1;
                if b >= books.len() as i16 {
                    return Err(malformed("a floor names a missing codebook"));
                }
                f.subclass_books[c][j] = b;
            }
        }
        f.multiplier = br.read(2).ok_or_else(eop)? as i32 + 1;
        let rangebits = br.read(4).ok_or_else(eop)?;
        f.xs = vec![0, 1 << rangebits];
        for &c in &f.partition_class {
            for _ in 0..f.class_dims[usize::from(c)] {
                if f.xs.len() >= FLOOR1_MAX_VALUES {
                    return Err(malformed("a floor 1 with more than 65 points"));
                }
                f.xs.push(br.read(rangebits).ok_or_else(eop)? as i32);
            }
        }
        let mut sorted: Vec<u8> = (0..f.xs.len() as u8).collect();
        sorted.sort_by_key(|&i| f.xs[usize::from(i)]);
        if sorted
            .windows(2)
            .any(|w| f.xs[usize::from(w[0])] == f.xs[usize::from(w[1])])
        {
            return Err(malformed("a floor 1 repeats an X value"));
        }
        f.sorted = sorted;
        f.low = vec![0; f.xs.len()];
        f.high = vec![0; f.xs.len()];
        for i in 2..f.xs.len() {
            let x = f.xs[i];
            let (mut lo, mut hi) = (0usize, 1usize);
            for j in 0..i {
                let xj = f.xs[j];
                if xj < x && xj > f.xs[lo] {
                    lo = j;
                }
                if xj > x && xj < f.xs[hi] {
                    hi = j;
                }
            }
            f.low[i] = lo as u8;
            f.high[i] = hi as u8;
        }
        Ok(f)
    }

    /// Reads the Y values into `y`; false: the channel is unused this frame (also at the
    /// end of the packet).
    pub(crate) fn decode(
        &self,
        br: &mut BitReader,
        books: &[Codebook],
        y: &mut [i32; FLOOR1_MAX_VALUES],
    ) -> bool {
        self.try_decode(br, books, y).is_some()
    }

    fn try_decode(
        &self,
        br: &mut BitReader,
        books: &[Codebook],
        y: &mut [i32; FLOOR1_MAX_VALUES],
    ) -> Option<()> {
        if !br.bit()? {
            return None;
        }
        let range = RANGES[(self.multiplier - 1) as usize];
        let bits = ilog(range as u32 - 1);
        y[0] = br.read(bits)? as i32;
        y[1] = br.read(bits)? as i32;
        let mut offset = 2;
        for &class in &self.partition_class {
            let class = usize::from(class);
            let cdim = usize::from(self.class_dims[class]);
            let cbits = u32::from(self.class_subclasses[class]);
            let csub = (1u32 << cbits) - 1;
            let mut cval = 0;
            if cbits > 0 {
                cval = books[usize::from(self.class_master[class])].decode(br)?;
            }
            for j in 0..cdim {
                let book = self.subclass_books[class][(cval & csub) as usize];
                cval >>= cbits;
                y[offset + j] = if book >= 0 {
                    books[book as usize].decode(br)? as i32
                } else {
                    0
                };
            }
            offset += cdim;
        }
        Some(())
    }

    /// Computes the curve from the Y values and multiplies `v` (n/2 long) by it.
    pub(crate) fn apply(&self, y: &[i32; FLOOR1_MAX_VALUES], v: &mut [f32]) {
        let n = self.xs.len();
        let range = RANGES[(self.multiplier - 1) as usize];
        let mut fy = [0i32; FLOOR1_MAX_VALUES];
        let mut step2 = [false; FLOOR1_MAX_VALUES];
        fy[0] = y[0].clamp(0, range - 1);
        fy[1] = y[1].clamp(0, range - 1);
        step2[0] = true;
        step2[1] = true;
        for i in 2..n {
            let lo = usize::from(self.low[i]);
            let hi = usize::from(self.high[i]);
            let predicted = render_point(self.xs[lo], fy[lo], self.xs[hi], fy[hi], self.xs[i]);
            let val = y[i];
            let highroom = range - predicted;
            let lowroom = predicted;
            let room = if highroom < lowroom {
                highroom * 2
            } else {
                lowroom * 2
            };
            if val != 0 {
                step2[lo] = true;
                step2[hi] = true;
                step2[i] = true;
                fy[i] = if val >= room {
                    if highroom > lowroom {
                        val - lowroom + predicted
                    } else {
                        predicted - val + highroom - 1
                    }
                } else if val % 2 == 1 {
                    predicted - (val + 1) / 2
                } else {
                    predicted + val / 2
                };
            } else {
                fy[i] = predicted;
            }
            // Valid setups stay in range; guard against crafted ones (spec 7.2.4).
            fy[i] = fy[i].clamp(0, range - 1);
        }

        let mut lx = 0;
        let mut ly = fy[0] * self.multiplier;
        let mut hx = 0;
        let mut hy = ly;
        for &i in &self.sorted[1..] {
            let i = usize::from(i);
            if step2[i] {
                hy = fy[i] * self.multiplier;
                hx = self.xs[i];
                render_line(lx, ly, hx, hy, v);
                lx = hx;
                ly = hy;
            }
        }
        let len = v.len() as i32;
        if hx < len {
            render_line(hx, hy, len, hy, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor0_curve() {
        let f = Floor0 {
            order: 2,
            amp_bits: 6,
            amp_offset: 40,
            books: Vec::new(),
            bark_map_size: 64,
            maps: [vec![0, 0, 5, 63, -1], Vec::new()],
        };
        let mut v = [1.0f32; 4];
        let half_pi = std::f32::consts::FRAC_PI_2;
        f.apply(63, &[half_pi, half_pi], &mut v, false);
        // At w = 0 with both coefficients at π/2: p = 0, q = 4, so the curve is
        // exp(0.11512925 · (63·40 / (63·2) − 40)) = 0.1.
        assert!((v[0] - 0.1).abs() < 1e-5 && v[1] == v[0], "{v:?}");
        assert!(v.iter().all(|x| x.is_finite() && *x > 0.0));
    }

    #[test]
    fn lines() {
        let mut v = [1.0f32; 8];
        render_line(0, 0, 8, 7, &mut v);
        let want: Vec<f32> = [0, 0, 1, 2, 3, 4, 5, 6]
            .iter()
            .map(|&i| INVERSE_DB[i])
            .collect();
        assert_eq!(v.to_vec(), want);
        assert_eq!(render_point(0, 10, 10, 0, 3), 7);
    }
}

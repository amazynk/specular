//! SWF's readers: little-endian bytes, and bit fields most significant bit first (RECT,
//! MATRIX, CXFORM, shape records).

use specular_drawpage::Affine;

pub(crate) struct Bytes<'a> {
    pub d: &'a [u8],
    pub at: usize,
}

impl<'a> Bytes<'a> {
    pub fn new(d: &'a [u8]) -> Bytes<'a> {
        Bytes { d, at: 0 }
    }
    pub fn left(&self) -> usize {
        self.d.len().saturating_sub(self.at)
    }
    pub fn u8(&mut self) -> Option<u8> {
        let v = *self.d.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    pub fn u16(&mut self) -> Option<u16> {
        let b = self.d.get(self.at..self.at + 2)?;
        self.at += 2;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn i16(&mut self) -> Option<i16> {
        self.u16().map(|v| v as i16)
    }
    pub fn u32(&mut self) -> Option<u32> {
        let b = self.d.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.d.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(b)
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let b = self.d.get(self.at..).unwrap_or_default();
        self.at = self.d.len();
        b
    }
    /// A NUL-terminated string.
    pub fn string(&mut self) -> Option<String> {
        let rest = self.d.get(self.at..)?;
        let n = rest.iter().position(|&c| c == 0)?;
        self.at += n + 1;
        Some(String::from_utf8_lossy(&rest[..n]).into_owned())
    }
    /// `[r, g, b, 255]`.
    pub fn rgb(&mut self) -> Option<[u8; 4]> {
        Some([self.u8()?, self.u8()?, self.u8()?, 255])
    }
    pub fn rgba(&mut self) -> Option<[u8; 4]> {
        Some([self.u8()?, self.u8()?, self.u8()?, self.u8()?])
    }
    /// A bit reader from here; [`Bits::done`] gives the byte position after it.
    pub fn bits(&self) -> Bits<'a> {
        Bits {
            d: self.d,
            bit: self.at * 8,
        }
    }
    pub fn rect(&mut self) -> Option<(i32, i32, i32, i32)> {
        let mut b = self.bits();
        let n = b.u(5)?;
        let r = (b.s(n)?, b.s(n)?, b.s(n)?, b.s(n)?);
        self.at = b.done();
        Some(r)
    }
    pub fn matrix(&mut self) -> Option<Affine> {
        let mut b = self.bits();
        let m = b.matrix()?;
        self.at = b.done();
        Some(m)
    }
    /// A colour transform: (multipliers ×256, additions), RGBA.
    pub fn cxform(&mut self, alpha: bool) -> Option<Cxform> {
        let mut b = self.bits();
        let has_add = b.u(1)? == 1;
        let has_mult = b.u(1)? == 1;
        let n = b.u(4)?;
        let mut c = Cxform::default();
        let k = if alpha { 4 } else { 3 };
        if has_mult {
            for i in 0..k {
                c.mult[i] = b.s(n)?;
            }
        }
        if has_add {
            for i in 0..k {
                c.add[i] = b.s(n)?;
            }
        }
        self.at = b.done();
        Some(c)
    }
}

/// A colour transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cxform {
    pub mult: [i32; 4],
    pub add: [i32; 4],
}

impl Default for Cxform {
    fn default() -> Cxform {
        Cxform {
            mult: [256; 4],
            add: [0; 4],
        }
    }
}

impl Cxform {
    pub fn apply(&self, c: [u8; 4]) -> [u8; 4] {
        let f =
            |i: usize| ((i32::from(c[i]) * self.mult[i]) / 256 + self.add[i]).clamp(0, 255) as u8;
        [f(0), f(1), f(2), f(3)]
    }
    /// `self` after `inner` (a parent's transform over a child's).
    pub fn then(&self, inner: &Cxform) -> Cxform {
        let mut c = Cxform::default();
        for i in 0..4 {
            c.mult[i] = inner.mult[i] * self.mult[i] / 256;
            c.add[i] = inner.add[i] * self.mult[i] / 256 + self.add[i];
        }
        c
    }
}

pub(crate) struct Bits<'a> {
    d: &'a [u8],
    bit: usize,
}

impl Bits<'_> {
    pub fn u(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if n > 32 {
            return None;
        }
        let mut v = 0u64;
        for _ in 0..n {
            let byte = *self.d.get(self.bit / 8)?;
            v = v << 1 | u64::from((byte >> (7 - self.bit % 8)) & 1);
            self.bit += 1;
        }
        Some(v as u32)
    }
    pub fn s(&mut self, n: u32) -> Option<i32> {
        if n == 0 {
            return Some(0);
        }
        let v = self.u(n)?;
        let shift = 32 - n;
        Some(((v << shift) as i32) >> shift)
    }
    /// A 16.16 fixed number of `n` bits.
    pub fn fixed(&mut self, n: u32) -> Option<f64> {
        Some(f64::from(self.s(n)?) / 65536.0)
    }
    pub fn matrix(&mut self) -> Option<Affine> {
        let (mut a, mut d) = (1.0, 1.0);
        let (mut b, mut c) = (0.0, 0.0);
        if self.u(1)? == 1 {
            let n = self.u(5)?;
            a = self.fixed(n)?;
            d = self.fixed(n)?;
        }
        if self.u(1)? == 1 {
            let n = self.u(5)?;
            b = self.fixed(n)?;
            c = self.fixed(n)?;
        }
        let n = self.u(5)?;
        let tx = f64::from(self.s(n)?);
        let ty = f64::from(self.s(n)?);
        // ScaleX, RotateSkew0 (b), RotateSkew1 (c), ScaleY: x' = a x + c y + tx.
        Some(Affine::new([a, b, c, d, tx, ty]))
    }
    /// The byte after the last bit read (the next field starts byte-aligned).
    pub fn done(&self) -> usize {
        self.bit.div_ceil(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_bits() {
        // nbits 5 = 0b01111 (15): 0, 11000, 0, 8000 twips.
        let mut w = Vec::new();
        let vals = [0i32, 11000, 0, 8000];
        let mut bits = vec![0, 1, 1, 1, 1];
        for v in vals {
            for i in (0..15).rev() {
                bits.push(((v >> i) & 1) as u8);
            }
        }
        for c in bits.chunks(8) {
            let mut b = 0u8;
            for (i, &x) in c.iter().enumerate() {
                b |= x << (7 - i);
            }
            w.push(b);
        }
        let mut r = Bytes::new(&w);
        assert_eq!(r.rect(), Some((0, 11000, 0, 8000)));
        assert_eq!(r.at, w.len());
    }
}

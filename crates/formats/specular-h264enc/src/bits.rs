//! Bit writing: fixed-length and Exp-Golomb codes into an RBSP, and NAL unit framing with
//! emulation prevention.

/// An MSB-first bit writer.
#[derive(Default, Debug, Clone)]
pub struct BitWriter {
    out: Vec<u8>,
    /// Pending bits, left-aligned in the low `fill` bits.
    acc: u64,
    fill: u32,
}

impl BitWriter {
    pub fn new() -> BitWriter {
        BitWriter::default()
    }

    pub fn with_capacity(n: usize) -> BitWriter {
        BitWriter {
            out: Vec::with_capacity(n),
            acc: 0,
            fill: 0,
        }
    }

    /// Writes the low `n` bits of `v` (n ≤ 32).
    #[inline]
    pub fn put(&mut self, n: u32, v: u32) {
        debug_assert!(n <= 32);
        if n == 0 {
            return;
        }
        let v = u64::from(v) & ((1u64 << n) - 1);
        self.acc = (self.acc << n) | v;
        self.fill += n;
        while self.fill >= 8 {
            self.fill -= 8;
            self.out.push((self.acc >> self.fill) as u8);
        }
        self.acc &= (1u64 << self.fill) - 1;
    }

    #[inline]
    pub fn flag(&mut self, b: bool) {
        self.put(1, u32::from(b));
    }

    /// Unsigned Exp-Golomb, ue(v).
    #[inline]
    pub fn ue(&mut self, v: u32) {
        let x = u64::from(v) + 1;
        let len = 64 - x.leading_zeros(); // bits of x
        // len - 1 zeros, then x in len bits.
        if len <= 16 {
            self.put(2 * len - 1, x as u32);
        } else {
            self.put(len - 1, 0);
            self.put(len, x as u32);
        }
    }

    /// Signed Exp-Golomb, se(v).
    #[inline]
    pub fn se(&mut self, v: i32) {
        self.ue(se_code(v));
    }

    /// Bits written so far.
    #[cfg(test)]
    pub fn bit_len(&self) -> usize {
        self.out.len() * 8 + self.fill as usize
    }

    #[cfg(test)]
    pub fn byte_aligned(&self) -> bool {
        self.fill == 0
    }

    /// `rbsp_trailing_bits()`: a one, then zeros to the byte boundary.
    pub fn trailing(&mut self) {
        self.put(1, 1);
        if self.fill > 0 {
            let pad = 8 - self.fill;
            self.put(pad, 0);
        }
    }

    /// Pads with ones to the byte boundary (`cabac_alignment_one_bit`).
    pub fn align_ones(&mut self) {
        if self.fill > 0 {
            let pad = 8 - self.fill;
            self.put(pad, (1 << pad) - 1);
        }
    }

    /// Pads with zeros to the byte boundary.
    pub fn align_zeros(&mut self) {
        if self.fill > 0 {
            let pad = 8 - self.fill;
            self.put(pad, 0);
        }
    }

    /// The RBSP bytes (must be byte aligned).
    pub fn into_bytes(self) -> Vec<u8> {
        debug_assert!(self.fill == 0);
        self.out
    }
}

/// Where CAVLC syntax goes: the bit writer, or a counter of its length.
pub trait Sink {
    /// The low `n` bits of `v` (n ≤ 32).
    fn put(&mut self, n: u32, v: u32);

    #[inline]
    fn flag(&mut self, b: bool) {
        self.put(1, u32::from(b));
    }

    #[inline]
    fn ue(&mut self, v: u32) {
        let x = u64::from(v) + 1;
        let len = 64 - x.leading_zeros();
        if len <= 16 {
            self.put(2 * len - 1, x as u32);
        } else {
            self.put(len - 1, 0);
            self.put(len, x as u32);
        }
    }

    #[inline]
    fn se(&mut self, v: i32) {
        self.ue(se_code(v));
    }
}

impl Sink for BitWriter {
    #[inline]
    fn put(&mut self, n: u32, v: u32) {
        BitWriter::put(self, n, v);
    }
}

/// Counts bits, writing nothing.
#[derive(Default)]
pub struct BitCount(pub u32);

impl Sink for BitCount {
    #[inline]
    fn put(&mut self, n: u32, _v: u32) {
        self.0 += n;
    }
}

/// The codeNum of se(v).
#[inline]
pub fn se_code(v: i32) -> u32 {
    if v > 0 {
        (2 * v - 1) as u32
    } else {
        (-2 * i64::from(v)) as u32
    }
}

/// Bits of ue(v).
#[inline]
pub fn ue_bits(v: u32) -> u32 {
    2 * (32 - (v + 1).leading_zeros()) - 1
}

/// Bits of se(v).
#[inline]
pub fn se_bits(v: i32) -> u32 {
    ue_bits(se_code(v))
}

/// Appends a NAL unit (header byte + RBSP with emulation prevention) to `out`, preceded by
/// a 4-byte big-endian length (the AVCC / MP4 sample form).
pub fn push_nal_avcc(out: &mut Vec<u8>, nal_ref_idc: u8, nal_type: u8, rbsp: &[u8]) {
    let at = out.len();
    out.extend_from_slice(&[0, 0, 0, 0]);
    push_nal_body(out, nal_ref_idc, nal_type, rbsp);
    let len = (out.len() - at - 4) as u32;
    out[at..at + 4].copy_from_slice(&len.to_be_bytes());
}

/// Appends the NAL header byte and the escaped RBSP.
pub fn push_nal_body(out: &mut Vec<u8>, nal_ref_idc: u8, nal_type: u8, rbsp: &[u8]) {
    out.reserve(rbsp.len() + rbsp.len() / 64 + 2);
    out.push((nal_ref_idc & 3) << 5 | (nal_type & 31));
    let mut zeros = 0u32;
    for &b in rbsp {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        out.push(b);
        if b == 0 {
            zeros += 1;
        } else {
            zeros = 0;
        }
    }
}

/// The escaped NAL unit (header + payload) on its own.
pub fn nal(nal_ref_idc: u8, nal_type: u8, rbsp: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    push_nal_body(&mut v, nal_ref_idc, nal_type, rbsp);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_golomb() {
        let mut w = BitWriter::new();
        // 0 → 1, 1 → 010, 2 → 011, 3 → 00100
        w.ue(0);
        w.ue(1);
        w.ue(2);
        w.ue(3);
        w.trailing();
        // 1 010 011 00100 1 → 1010 0110 0100 1000 → padded
        assert_eq!(w.into_bytes(), vec![0b1010_0110, 0b0100_1000]);
        assert_eq!(ue_bits(0), 1);
        assert_eq!(ue_bits(3), 5);
        assert_eq!(se_code(1), 1);
        assert_eq!(se_code(-1), 2);
        assert_eq!(se_code(0), 0);
        let mut w = BitWriter::new();
        w.ue(100_000);
        assert_eq!(w.bit_len() as u32, ue_bits(100_000));
    }

    #[test]
    fn emulation_prevention() {
        let n = nal(3, 5, &[0, 0, 1, 0, 0, 0, 0, 0, 3, 0, 0, 4]);
        assert_eq!(n, vec![0x65, 0, 0, 3, 1, 0, 0, 3, 0, 0, 3, 0, 3, 0, 0, 4]);
    }
}

//! The entropy coding under LASzip: a range decoder over bytes with adaptive multi-symbol
//! and binary models, and the integer "compressor" that codes a value as a corrector to a
//! prediction (the corrector's bit length as a symbol, then its bits).
//!
//! Written from the published description of the format (Isenburg, "LASzip: lossless
//! compression of LiDAR data", 2013) and the behaviour of files it writes. A decoder past the
//! end of its bytes reads zeros and remembers that it did, so corrupt input yields wrong
//! points and an error at the end of the chunk, never a panic.

/// Below this the interval is renormalised (a byte shifted in).
const MIN_LENGTH: u32 = 0x0100_0000;

/// Binary models: probabilities in 13 bits, counts halved past 2¹³.
const BM_LENGTH_SHIFT: u32 = 13;
const BM_MAX_COUNT: u32 = 1 << 13;

/// Symbol models: distributions in 15 bits, counts halved past 2¹⁵.
const DM_LENGTH_SHIFT: u32 = 15;
const DM_MAX_COUNT: u32 = 1 << 15;

/// The range decoder over one byte slice.
pub(crate) struct Decoder<'a> {
    bytes: &'a [u8],
    pos: usize,
    value: u32,
    length: u32,
    /// Bytes read past the end (as zeros).
    pub(crate) overrun: usize,
}

impl<'a> Decoder<'a> {
    /// A decoder primed with the first four bytes of `bytes`.
    pub(crate) fn new(bytes: &'a [u8]) -> Decoder<'a> {
        let mut d = Decoder {
            bytes,
            pos: 0,
            value: 0,
            length: u32::MAX,
            overrun: 0,
        };
        for _ in 0..4 {
            d.value = (d.value << 8) | u32::from(d.byte());
        }
        d
    }

    #[inline(always)]
    fn byte(&mut self) -> u8 {
        match self.bytes.get(self.pos) {
            Some(&b) => {
                self.pos += 1;
                b
            }
            None => {
                self.overrun += 1;
                0
            }
        }
    }

    #[inline(always)]
    fn renorm(&mut self) {
        while self.length < MIN_LENGTH {
            self.value = (self.value << 8) | u32::from(self.byte());
            self.length <<= 8;
        }
    }

    pub(crate) fn bit(&mut self, m: &mut BitModel) -> u32 {
        let x = m.prob_0 * (self.length >> BM_LENGTH_SHIFT);
        let sym = u32::from(self.value >= x);
        if sym == 0 {
            self.length = x;
            m.count_0 += 1;
        } else {
            self.value = self.value.wrapping_sub(x);
            self.length -= x;
        }
        if self.length < MIN_LENGTH {
            self.renorm();
        }
        m.until_update -= 1;
        if m.until_update == 0 {
            m.update();
        }
        sym
    }

    #[inline]
    pub(crate) fn symbol(&mut self, m: &mut Model) -> u32 {
        let mut y = self.length;
        let (sym, x);
        self.length >>= DM_LENGTH_SHIFT;
        // A zero length only arises from corrupt input; keep going without dividing by it.
        let len = self.length.max(1);
        if m.table_shift != 0 {
            let dv = self.value / len;
            let t = ((dv >> m.table_shift) as usize).min(m.table.len() - 2);
            let mut s = m.table[t];
            let mut n = m.table[t + 1] + 1;
            while n > s + 1 {
                let k = (s + n) >> 1;
                if m.dist[k as usize] > dv {
                    n = k;
                } else {
                    s = k;
                }
            }
            sym = s.min(m.symbols - 1);
            x = m.dist[sym as usize].wrapping_mul(self.length);
            if sym != m.symbols - 1 {
                y = m.dist[sym as usize + 1].wrapping_mul(self.length);
            }
        } else {
            let (mut s, mut lo) = (0u32, 0u32);
            let mut n = m.symbols;
            let mut k = n >> 1;
            loop {
                let z = self.length.wrapping_mul(m.dist[k as usize]);
                if z > self.value {
                    n = k;
                    y = z;
                } else {
                    s = k;
                    lo = z;
                }
                k = (s + n) >> 1;
                if k == s {
                    break;
                }
            }
            sym = s;
            x = lo;
        }
        self.value = self.value.wrapping_sub(x);
        self.length = y.wrapping_sub(x);
        if self.length < MIN_LENGTH {
            self.renorm();
        }
        m.count[sym as usize] += 1;
        m.until_update -= 1;
        if m.until_update == 0 {
            m.update();
        }
        sym
    }

    /// `bits` raw bits (1–32).
    pub(crate) fn bits(&mut self, bits: u32) -> u32 {
        if bits > 19 {
            let lo = self.short();
            let hi = self.bits(bits - 16);
            return (hi << 16) | lo;
        }
        self.length >>= bits;
        let sym = self.value / self.length.max(1);
        self.value = self.value.wrapping_sub(self.length.wrapping_mul(sym));
        if self.length < MIN_LENGTH {
            self.renorm();
        }
        sym & ((1u32 << bits) - 1)
    }

    fn short(&mut self) -> u32 {
        self.length >>= 16;
        let sym = self.value / self.length.max(1);
        self.value = self.value.wrapping_sub(self.length.wrapping_mul(sym));
        if self.length < MIN_LENGTH {
            self.renorm();
        }
        sym & 0xFFFF
    }

    /// 32 raw bits, the low half first.
    pub(crate) fn int(&mut self) -> u32 {
        let lo = self.short();
        let hi = self.short();
        (hi << 16) | lo
    }
}

/// An adaptive binary model.
#[derive(Clone)]
pub(crate) struct BitModel {
    count_0: u32,
    count: u32,
    prob_0: u32,
    until_update: u32,
    cycle: u32,
}

impl BitModel {
    pub(crate) fn new() -> BitModel {
        BitModel {
            count_0: 1,
            count: 2,
            prob_0: 1 << (BM_LENGTH_SHIFT - 1),
            until_update: 4,
            cycle: 4,
        }
    }

    fn update(&mut self) {
        self.count += self.cycle;
        if self.count > BM_MAX_COUNT {
            self.count = (self.count + 1) >> 1;
            self.count_0 = (self.count_0 + 1) >> 1;
            if self.count_0 == self.count {
                self.count += 1;
            }
        }
        let scale = 0x8000_0000u32 / self.count;
        self.prob_0 = (self.count_0.wrapping_mul(scale)) >> (31 - BM_LENGTH_SHIFT);
        self.cycle = ((5 * self.cycle) >> 2).min(64);
        self.until_update = self.cycle;
    }
}

/// An adaptive model over `symbols` symbols (2–2048). Above 16 symbols a lookup table
/// narrows the search.
#[derive(Clone)]
pub(crate) struct Model {
    symbols: u32,
    dist: Vec<u32>,
    count: Vec<u32>,
    table: Vec<u32>,
    table_shift: u32,
    total: u32,
    cycle: u32,
    until_update: u32,
}

impl Model {
    pub(crate) fn new(symbols: u32) -> Model {
        let symbols = symbols.clamp(2, 2048);
        let (table, table_shift) = if symbols > 16 {
            let mut bits = 3;
            while symbols > (1 << (bits + 2)) {
                bits += 1;
            }
            (vec![0; (1 << bits) + 2], DM_LENGTH_SHIFT - bits)
        } else {
            (Vec::new(), 0)
        };
        let mut m = Model {
            symbols,
            dist: vec![0; symbols as usize],
            count: vec![1; symbols as usize],
            table,
            table_shift,
            total: 0,
            cycle: symbols,
            until_update: 0,
        };
        m.update();
        m.cycle = (symbols + 6) >> 1;
        m.until_update = m.cycle;
        m
    }

    fn update(&mut self) {
        self.total += self.cycle;
        if self.total > DM_MAX_COUNT {
            self.total = 0;
            for c in &mut self.count {
                *c = (*c + 1) >> 1;
                self.total += *c;
            }
        }
        let scale = 0x8000_0000u32 / self.total.max(1);
        let mut sum = 0u32;
        if self.table_shift == 0 {
            for (d, &c) in self.dist.iter_mut().zip(&self.count) {
                *d = scale.wrapping_mul(sum) >> (31 - DM_LENGTH_SHIFT);
                sum = sum.wrapping_add(c);
            }
        } else {
            let mut s = 0usize;
            let last = self.table.len() - 2;
            for k in 0..self.symbols as usize {
                let d = scale.wrapping_mul(sum) >> (31 - DM_LENGTH_SHIFT);
                self.dist[k] = d;
                sum = sum.wrapping_add(self.count[k]);
                let w = (d >> self.table_shift) as usize;
                while s < w.min(last) {
                    s += 1;
                    self.table[s] = k as u32 - 1;
                }
            }
            self.table[0] = 0;
            while s <= last {
                s += 1;
                self.table[s] = self.symbols - 1;
            }
        }
        self.cycle = ((5 * self.cycle) >> 2).min((self.symbols + 6) << 3);
        self.until_update = self.cycle;
    }
}

/// A five-value running median (a prediction for coordinate deltas).
#[derive(Clone, Copy, Default)]
pub(crate) struct Median5 {
    v: [i32; 5],
    low: bool,
}

impl Median5 {
    pub(crate) fn get(&self) -> i32 {
        self.v[2]
    }

    pub(crate) fn add(&mut self, x: i32) {
        let v = &mut self.v;
        if !self.low {
            if x < v[2] {
                v[4] = v[3];
                v[3] = v[2];
                if x < v[0] {
                    v[2] = v[1];
                    v[1] = v[0];
                    v[0] = x;
                } else if x < v[1] {
                    v[2] = v[1];
                    v[1] = x;
                } else {
                    v[2] = x;
                }
            } else {
                if x < v[3] {
                    v[4] = v[3];
                    v[3] = x;
                } else {
                    v[4] = x;
                }
                self.low = true;
            }
        } else if v[2] < x {
            v[0] = v[1];
            v[1] = v[2];
            if v[4] < x {
                v[2] = v[3];
                v[3] = v[4];
                v[4] = x;
            } else if v[3] < x {
                v[2] = v[3];
                v[3] = x;
            } else {
                v[2] = x;
            }
        } else {
            if v[1] < x {
                v[0] = v[1];
                v[1] = x;
            } else {
                v[0] = x;
            }
            self.low = false;
        }
    }
}

/// Integers coded as correctors to a prediction, with `contexts` separate models for the
/// corrector's bit length.
pub(crate) struct IntCoder {
    k_models: Vec<Model>,
    bit0: BitModel,
    /// Per bit length k ≥ 1: the corrector's high bits (at most `bits_high` of them).
    correctors: Vec<Model>,
    corr_bits: u32,
    corr_range: u32,
    corr_min: i32,
    bits_high: u32,
    /// The bit length of the last corrector.
    pub(crate) k: u32,
}

impl IntCoder {
    /// Values of `bits` bits (32: the full `i32` range).
    pub(crate) fn new(bits: u32, contexts: u32) -> IntCoder {
        let bits_high = 8;
        let (corr_bits, corr_range, corr_min) = if bits > 0 && bits < 32 {
            let range = 1u32 << bits;
            (bits, range, -((range / 2) as i32))
        } else {
            (32, 0, i32::MIN)
        };
        IntCoder {
            k_models: (0..contexts.max(1))
                .map(|_| Model::new(corr_bits + 1))
                .collect(),
            bit0: BitModel::new(),
            correctors: (1..=corr_bits)
                .map(|i| Model::new(1 << i.min(bits_high)))
                .collect(),
            corr_bits,
            corr_range,
            corr_min,
            bits_high,
            k: 0,
        }
    }

    /// The value after `pred` in `context`.
    #[inline]
    pub(crate) fn decode(&mut self, d: &mut Decoder, pred: i32, context: u32) -> i32 {
        let ctx = (context as usize).min(self.k_models.len() - 1);
        let c = self.corrector(d, ctx);
        let real = pred.wrapping_add(c);
        if self.corr_range == 0 {
            return real;
        }
        let r = self.corr_range as i32;
        if real < 0 {
            real.wrapping_add(r)
        } else if real >= r {
            real.wrapping_sub(r)
        } else {
            real
        }
    }

    fn corrector(&mut self, d: &mut Decoder, ctx: usize) -> i32 {
        let k = d.symbol(&mut self.k_models[ctx]).min(self.corr_bits);
        self.k = k;
        if k == 0 {
            return d.bit(&mut self.bit0) as i32;
        }
        if k >= 32 {
            return self.corr_min;
        }
        let model = &mut self.correctors[k as usize - 1];
        let c = if k <= self.bits_high {
            d.symbol(model)
        } else {
            let low = k - self.bits_high;
            let hi = d.symbol(model);
            (hi << low) | d.bits(low)
        };
        let c = i64::from(c);
        let half = 1i64 << (k - 1);
        (if c >= half {
            c + 1
        } else {
            c - ((1i64 << k) - 1)
        }) as i32
    }
}

/// `n` folded into 0‥255 (one wrap either way).
#[inline(always)]
pub(crate) fn fold(n: i32) -> u8 {
    n.rem_euclid(256) as u8
}

/// `n` clamped to 0‥255.
#[inline(always)]
pub(crate) fn clamp(n: i32) -> i32 {
    n.clamp(0, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_tracks_middle() {
        let mut m = Median5::default();
        for x in [5, 1, 9, 3, 7] {
            m.add(x);
        }
        assert_eq!(m.get(), 5);
    }

    #[test]
    fn garbage_does_not_panic() {
        let bytes: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let mut d = Decoder::new(&bytes);
        let mut ic = IntCoder::new(32, 4);
        let mut m = Model::new(256);
        let mut b = BitModel::new();
        for i in 0..20_000u32 {
            ic.decode(&mut d, i as i32, i % 7);
            d.symbol(&mut m);
            d.bit(&mut b);
            d.bits(1 + i % 32);
        }
        assert!(d.overrun > 0);
        // An empty stream too.
        let mut d = Decoder::new(&[]);
        for _ in 0..100 {
            ic.decode(&mut d, 0, 0);
        }
    }
}

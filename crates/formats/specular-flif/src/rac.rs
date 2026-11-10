//! FLIF's 24-bit binary range decoder, its adaptive 12-bit chances and the two integer
//! codings built on it: uniform (fixed 50 % bits) and near-zero (zero, sign, unary exponent,
//! mantissa, each bit position with its own adaptive chance).
//!
//! Past the end of the data the decoder reads all-one bytes, as the reference decoder's
//! `fgetc` returning `EOF` does, and remembers that it did ([`Rac::eof`]): a truncated
//! interlaced file is then finished by interpolation at the same row the reference stops at.

/// The range decoder over the bytes after the metadata chunks.
pub(crate) struct Rac<'a> {
    data: &'a [u8],
    pos: usize,
    range: u64,
    low: u64,
    eof: bool,
}

const MIN_RANGE: u64 = 1 << 16;
const BASE_RANGE: u64 = 1 << 24;

impl<'a> Rac<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Rac<'a> {
        let mut r = Rac {
            data,
            pos: 0,
            range: BASE_RANGE,
            low: 0,
            eof: false,
        };
        for _ in 0..3 {
            let b = r.byte();
            r.low = (r.low << 8) | b;
        }
        r
    }

    #[inline]
    fn byte(&mut self) -> u64 {
        match self.data.get(self.pos) {
            Some(&b) => {
                self.pos += 1;
                u64::from(b)
            }
            None => {
                self.eof = true;
                u64::MAX
            }
        }
    }

    /// Whether a byte past the end was asked for (the reference's `feof`).
    pub(crate) fn eof(&self) -> bool {
        self.eof
    }

    #[inline]
    fn input(&mut self) {
        for _ in 0..2 {
            if self.range <= MIN_RANGE {
                self.low = self.low.wrapping_shl(8);
                self.range <<= 8;
                let b = self.byte();
                self.low |= b;
            }
        }
    }

    #[inline]
    fn get(&mut self, chance: u64) -> bool {
        let split = self.range.wrapping_sub(chance);
        if self.low >= split {
            self.low = self.low.wrapping_sub(split);
            self.range = chance;
            self.input();
            true
        } else {
            self.range = split;
            self.input();
            false
        }
    }

    #[inline]
    pub(crate) fn read_12bit(&mut self, b12: u16) -> bool {
        let chance = (self.range * u64::from(b12) + 0x800) >> 12;
        self.get(chance)
    }

    #[inline]
    pub(crate) fn read_bit(&mut self) -> bool {
        let half = self.range >> 1;
        self.get(half)
    }

    /// A uniform integer in `min..=min + len` (the reference's `read_int(min, len)`).
    pub(crate) fn uniform(&mut self, mut min: i32, mut len: i32) -> i32 {
        while len > 0 {
            let med = len / 2;
            if self.read_bit() {
                min += med + 1;
                len -= med + 1;
            } else {
                len = med;
            }
        }
        min
    }
}

/// The chance update table for one cutoff and decay factor: the next 12-bit chance after a
/// zero bit (`next[0]`) or a one bit (`next[1]`).
#[derive(Clone)]
pub(crate) struct ChanceTable {
    next: Box<[[u16; 4096]; 2]>,
}

impl ChanceTable {
    /// `cut` is the cutoff (the lowest chance), `factor` the decay (`0xFFFFFFFF / divisor`).
    pub(crate) fn new(cut: u32, factor: u32) -> ChanceTable {
        const SIZE: usize = 4096;
        let max_p = SIZE as u32 - cut;
        let one: i64 = 1 << 32;
        let factor = i64::from(factor);
        let mut zero_state = [0u16; SIZE];
        let mut one_state = [0u16; SIZE];
        let mut last_p8: u32 = 0;
        let mut p: i64 = one / 2;
        for _ in 0..SIZE / 2 {
            let mut p8 = ((SIZE as i64).wrapping_mul(p).wrapping_add(one / 2) >> 32) as u32;
            if p8 <= last_p8 {
                p8 = last_p8 + 1;
            }
            if last_p8 != 0 && (last_p8 as usize) < SIZE && p8 <= max_p {
                one_state[last_p8 as usize] = p8 as u16;
            }
            p = p.wrapping_add((one - p).wrapping_mul(factor).wrapping_add(one / 2) >> 32);
            last_p8 = p8;
        }
        for i in (SIZE as u32 - max_p)..=max_p {
            if one_state[i as usize] != 0 {
                continue;
            }
            let mut p = (i64::from(i) * one + SIZE as i64 / 2) / SIZE as i64;
            p = p.wrapping_add((one - p).wrapping_mul(factor).wrapping_add(one / 2) >> 32);
            let mut p8 = ((SIZE as i64).wrapping_mul(p).wrapping_add(one / 2) >> 32) as u32;
            if p8 <= i {
                p8 = i + 1;
            }
            if p8 > max_p {
                p8 = max_p;
            }
            one_state[i as usize] = p8 as u16;
        }
        for i in 1..SIZE {
            zero_state[i] = (SIZE as u32 - u32::from(one_state[SIZE - i])) as u16;
        }
        // Chances must stay usable as table indices and as nonzero 12-bit chances, also on
        // hostile cutoffs; on the reference's tables they already do.
        for t in [&mut zero_state, &mut one_state] {
            for v in t.iter_mut() {
                *v = (*v).clamp(1, 4095);
            }
        }
        ChanceTable {
            next: Box::new([zero_state, one_state]),
        }
    }

    #[inline]
    fn update(&self, chance: u16, bit: bool) -> u16 {
        self.next[usize::from(bit)][usize::from(chance & 4095)]
    }
}

const EXP_CHANCES: [u16; 17] = [
    1000, 1200, 1500, 1750, 2000, 2300, 2800, 2400, 2300, 2048, 2048, 2048, 2048, 2048, 2048, 2048,
    2048,
];
const MANT_CHANCES: [u16; 18] = [
    1900, 1850, 1800, 1750, 1650, 1600, 1600, 2048, 2048, 2048, 2048, 2048, 2048, 2048, 2048, 2048,
    2048, 2048,
];

/// The chances of one near-zero context: zero, sign, 34 exponent bits (by position and
/// sign), 18 mantissa bits.
#[derive(Clone)]
pub(crate) struct SymbolChances {
    zero: u16,
    sign: u16,
    exp: [u16; 34],
    mant: [u16; 18],
}

impl Default for SymbolChances {
    fn default() -> Self {
        let mut exp = [0u16; 34];
        for i in 0..17 {
            exp[2 * i] = EXP_CHANCES[i];
            exp[2 * i + 1] = EXP_CHANCES[i];
        }
        SymbolChances {
            zero: 1000,
            sign: 2048,
            exp,
            mant: MANT_CHANCES,
        }
    }
}

#[inline]
fn ilog2(v: u32) -> u32 {
    if v == 0 { 0 } else { 31 - v.leading_zeros() }
}

#[inline]
fn bit(rac: &mut Rac, table: &ChanceTable, ch: &mut u16) -> bool {
    let b = rac.read_12bit(*ch);
    *ch = table.update(*ch, b);
    b
}

/// A near-zero integer in `min..=max` (the reference reader; for valid files
/// `min <= 0 <= max`, other bounds are read the way the reference reads them).
pub(crate) fn read_nz(
    rac: &mut Rac,
    table: &ChanceTable,
    ctx: &mut SymbolChances,
    min: i32,
    max: i32,
) -> i32 {
    if min == max {
        return min;
    }
    if bit(rac, table, &mut ctx.zero) {
        return 0;
    }
    let sign = if min < 0 {
        if max > 0 {
            bit(rac, table, &mut ctx.sign)
        } else {
            false
        }
    } else {
        true
    };
    let amax = if sign { max } else { min.wrapping_neg() };
    // Hostile bounds (the value cannot be non-zero with this sign): give up on the symbol.
    if amax <= 0 {
        return 0;
    }
    let emax = ilog2(amax as u32);
    let mut e = 0u32;
    while e < emax {
        if bit(
            rac,
            table,
            &mut ctx.exp[(((e << 1) + u32::from(sign)) as usize).min(33)],
        ) {
            break;
        }
        e += 1;
    }
    let mut have: i32 = 1 << e;
    let mut left: i32 = have - 1;
    let mut pos = e;
    while pos > 0 {
        left >>= 1;
        pos -= 1;
        let minabs1 = have | (1 << pos);
        let maxabs0 = have | left;
        if minabs1 > amax {
            continue;
        } else if maxabs0 >= 1 {
            if bit(rac, table, &mut ctx.mant[(pos as usize).min(17)]) {
                have = minabs1;
            }
        } else {
            have = minabs1;
        }
    }
    if sign { have } else { -have }
}

/// One self-contained near-zero coder: a context and its table (the reference's
/// `SimpleSymbolCoder` with the default cutoff 2 and decay 1/19).
pub(crate) struct SimpleCoder {
    ctx: SymbolChances,
}

impl SimpleCoder {
    pub(crate) fn new() -> SimpleCoder {
        SimpleCoder {
            ctx: SymbolChances::default(),
        }
    }

    pub(crate) fn read(&mut self, rac: &mut Rac, table: &ChanceTable, min: i32, max: i32) -> i32 {
        read_nz(rac, table, &mut self.ctx, min, max)
    }

    /// The generalised near-zero integer (`read_int2`): intervals that exclude zero are
    /// shifted to touch it.
    pub(crate) fn read2(&mut self, rac: &mut Rac, table: &ChanceTable, min: i32, max: i32) -> i32 {
        if min > 0 {
            self.read(rac, table, 0, max - min) + min
        } else if max < 0 {
            self.read(rac, table, min - max, 0) + max
        } else {
            self.read(rac, table, min, max)
        }
    }
}

/// The default table (cutoff 2, decay 1/19) used by the headers, transforms and trees.
pub(crate) fn default_table() -> ChanceTable {
    ChanceTable::new(2, 0xFFFF_FFFF / 19)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_stay_in_range() {
        for (cut, div) in [(2, 19), (1, 2), (128, 128), (4, 20)] {
            let t = ChanceTable::new(cut, 0xFFFF_FFFF / div);
            for b in 0..2 {
                for &v in t.next[b].iter() {
                    assert!((1..4096).contains(&v));
                }
            }
        }
    }

    #[test]
    fn uniform_reads_whole_interval() {
        // Every bit pattern lands inside the interval.
        for seed in 0..64u8 {
            let data: Vec<u8> = (0..64).map(|i| (i as u8).wrapping_mul(37) ^ seed).collect();
            let mut r = Rac::new(&data);
            for _ in 0..50 {
                let v = r.uniform(-3, 9);
                assert!((-3..=6).contains(&v));
            }
        }
    }

    #[test]
    fn near_zero_stays_in_bounds() {
        let t = default_table();
        let data: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let mut r = Rac::new(&data);
        let mut c = SymbolChances::default();
        for i in 0..2000 {
            let (lo, hi) = (-(i % 300), i % 517);
            let v = read_nz(&mut r, &t, &mut c, lo, hi);
            assert!((lo..=hi).contains(&v), "{v} not in {lo}..={hi}");
        }
    }
}

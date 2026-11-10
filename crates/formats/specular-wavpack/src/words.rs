//! WavPack's bit reader and its adaptive entropy coder: three running medians per channel
//! split the magnitude range into Golomb-like zones (a unary count of "ones" picks the zone,
//! a truncated binary code the value inside), runs of zeros are sent as one count, and in
//! hybrid mode a running error limit stops the value's refinement early (the `.wvc`
//! correction stream then carries the rest).

use std::sync::LazyLock;

/// LSB-first bit reader over a byte slice. Reading past the end gives zeros and sets
/// `overrun`.
pub(crate) struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    sr: u64,
    bc: u32,
    pub overrun: bool,
}

impl<'a> Bits<'a> {
    pub fn new(d: &'a [u8]) -> Bits<'a> {
        Bits {
            d,
            pos: 0,
            sr: 0,
            bc: 0,
            overrun: false,
        }
    }

    fn refill(&mut self) {
        let b = match self.d.get(self.pos) {
            Some(&b) => b,
            None => {
                self.overrun = true;
                0
            }
        };
        self.pos += 1;
        self.sr |= u64::from(b) << self.bc;
        self.bc += 8;
    }

    #[inline]
    pub fn bit(&mut self) -> u32 {
        if self.bc == 0 {
            self.refill();
        }
        let b = (self.sr & 1) as u32;
        self.sr >>= 1;
        self.bc -= 1;
        b
    }

    /// `n` bits (0..=32), the first read in the lowest bit.
    #[inline]
    pub fn bits(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        while self.bc < n {
            self.refill();
        }
        let v = (self.sr & ((1u64 << n) - 1)) as u32;
        self.sr >>= n;
        self.bc -= n;
        v
    }

    /// Ones before a zero, at most `max` (the zero is read unless `max` ones came first).
    #[inline]
    fn ones(&mut self, max: u32) -> u32 {
        let mut n = 0;
        while n < max && self.bit() == 1 {
            n += 1;
        }
        n
    }

    /// A value in 0..=`maxcode` in a truncated binary code.
    pub fn code(&mut self, maxcode: u32) -> u32 {
        if maxcode < 2 {
            return if maxcode == 1 { self.bit() } else { 0 };
        }
        let nbits = 32 - maxcode.leading_zeros();
        let extras = ((1u64 << nbits) - u64::from(maxcode) - 1) as u32;
        let code = self.bits(nbits - 1);
        if code >= extras {
            (code << 1).wrapping_sub(extras).wrapping_add(self.bit())
        } else {
            code
        }
    }

    /// A count sent as a run of ones (its bit length), a zero and the bits below the top one;
    /// `None` when the run is 33 long (damaged).
    fn escape(&mut self) -> Option<u32> {
        let n = self.ones(33);
        if n == 33 {
            return None;
        }
        if n < 2 {
            return Some(n);
        }
        let mut v = 0u32;
        let mut mask = 1u32;
        for _ in 1..n {
            if self.bit() == 1 {
                v |= mask;
            }
            mask <<= 1;
        }
        Some(v | mask)
    }
}

/// The base-2 logarithm and exponential tables (fractions in 1/256 steps), computed from
/// their definitions: `log2[i] = round(256·log2(1 + i/256))`, `exp2[i] = round(256·2^(i/256)) − 256`.
static TABLES: LazyLock<([u8; 256], [u8; 256])> = LazyLock::new(|| {
    let mut log = [0u8; 256];
    let mut exp = [0u8; 256];
    for i in 0..256 {
        let x = i as f64 / 256.0;
        log[i] = (256.0 * (1.0 + x).log2()).round() as u8;
        exp[i] = ((256.0 * x.exp2()).round() - 256.0) as u8;
    }
    (log, exp)
});

fn nbits(v: u32) -> i32 {
    32 - v.leading_zeros() as i32
}

/// WavPack's fixed-point log2 (8 fraction bits) of a magnitude.
pub(crate) fn log2(v: u32) -> i32 {
    let log = &TABLES.0;
    let v = v.wrapping_add(v >> 9);
    if v < 1 << 8 {
        let d = nbits(v);
        (d << 8) + i32::from(log[((v << (9 - d)) & 0xff) as usize])
    } else {
        let d = nbits(v);
        (d << 8) + i32::from(log[((v >> (d - 9)) & 0xff) as usize])
    }
}

/// The inverse of [`log2`], signed.
pub(crate) fn exp2s(log: i32) -> i32 {
    if log < 0 {
        return exp2s(log.wrapping_neg()).wrapping_neg();
    }
    let v = u32::from(TABLES.1[(log & 0xff) as usize]) | 0x100;
    let e = log >> 8;
    if e <= 9 {
        (v >> (9 - e)) as i32
    } else {
        (v << ((e - 9) & 0x1f)) as i32
    }
}

/// A decorrelation weight from its stored byte.
pub(crate) fn restore_weight(w: i8) -> i32 {
    let mut r = i32::from(w) * 8;
    if r > 0 {
        r += (r + 64) >> 7;
    }
    r
}

const SLS: u32 = 8;
const SLO: u32 = 1 << (SLS - 1);

#[derive(Clone, Copy, Default)]
pub(crate) struct Chan {
    pub median: [u32; 3],
    pub slow_level: u32,
    pub error_limit: u32,
}

impl Chan {
    fn med(&self, i: usize) -> u32 {
        (self.median[i] >> 4) + 1
    }
    fn inc(&mut self, i: usize) {
        let div = 128 >> i;
        let m = self.median[i];
        self.median[i] = m.wrapping_add((m.wrapping_add(div) / div).wrapping_mul(5));
    }
    fn dec(&mut self, i: usize) {
        let div = 128 >> i;
        let m = self.median[i];
        self.median[i] = m.wrapping_sub((m.wrapping_add(div - 2) / div).wrapping_mul(2));
    }
    fn slow_decay(&mut self) {
        self.slow_level = self
            .slow_level
            .wrapping_sub(self.slow_level.wrapping_add(SLO) >> SLS);
    }
}

/// Hybrid-mode flags the word reader needs.
#[derive(Clone, Copy)]
pub(crate) struct Mode {
    pub hybrid: bool,
    pub bitrate: bool,
    pub balance: bool,
    pub mono: bool,
}

/// The entropy decoder's state for one block.
#[derive(Default)]
pub(crate) struct Words {
    pub c: [Chan; 2],
    pub bitrate_acc: [u32; 2],
    pub bitrate_delta: [u32; 2],
    holding_one: u32,
    holding_zero: bool,
    zeros_acc: u32,
}

/// The stream ran out or a run was impossibly long.
pub(crate) struct Eof;

impl Words {
    fn update_error_limit(&mut self, m: Mode) {
        self.bitrate_acc[0] = self.bitrate_acc[0].wrapping_add(self.bitrate_delta[0]);
        let mut br0 = (self.bitrate_acc[0] >> 16) as i32;
        let limit = |slow: u32, br: i32| -> u32 {
            let sl = (slow.wrapping_add(SLO) >> SLS) as i32;
            if sl - br > -0x100 {
                exp2s(sl - br + 0x100) as u32
            } else {
                0
            }
        };
        if m.mono {
            self.c[0].error_limit = if m.bitrate {
                limit(self.c[0].slow_level, br0)
            } else {
                exp2s(br0) as u32
            };
            return;
        }
        self.bitrate_acc[1] = self.bitrate_acc[1].wrapping_add(self.bitrate_delta[1]);
        let mut br1 = (self.bitrate_acc[1] >> 16) as i32;
        if m.bitrate {
            if m.balance {
                let sl0 = (self.c[0].slow_level.wrapping_add(SLO) >> SLS) as i32;
                let sl1 = (self.c[1].slow_level.wrapping_add(SLO) >> SLS) as i32;
                let balance = (sl1 - sl0 + br1 + 1) >> 1;
                if balance > br0 {
                    br1 = br0 * 2;
                    br0 = 0;
                } else if -balance > br0 {
                    br0 *= 2;
                    br1 = 0;
                } else {
                    br1 = br0 + balance;
                    br0 -= balance;
                }
            }
            self.c[0].error_limit = limit(self.c[0].slow_level, br0);
            self.c[1].error_limit = limit(self.c[1].slow_level, br1);
        } else {
            self.c[0].error_limit = exp2s(br0) as u32;
            self.c[1].error_limit = exp2s(br1) as u32;
        }
    }

    /// The next residual of channel `chan`, and its correction from `wvc` (0 without one).
    pub fn word(
        &mut self,
        m: Mode,
        chan: usize,
        bs: &mut Bits,
        wvc: Option<&mut Bits>,
    ) -> Result<(i32, i32), Eof> {
        if self.c[0].median[0] & !1 == 0
            && !self.holding_zero
            && self.holding_one == 0
            && self.c[1].median[0] & !1 == 0
        {
            if self.zeros_acc != 0 {
                self.zeros_acc -= 1;
                if self.zeros_acc != 0 {
                    self.c[chan].slow_decay();
                    return Ok((0, 0));
                }
            } else {
                self.zeros_acc = bs.escape().ok_or(Eof)?;
                if self.zeros_acc != 0 {
                    self.c[chan].slow_decay();
                    self.c[0].median = [0; 3];
                    self.c[1].median = [0; 3];
                    return Ok((0, 0));
                }
            }
        }
        let mut ones;
        if self.holding_zero {
            ones = 0;
            self.holding_zero = false;
        } else {
            ones = bs.ones(17);
            if ones == 17 {
                return Err(Eof);
            }
            if ones == 16 {
                ones = 16 + bs.escape().ok_or(Eof)?;
            }
            if self.holding_one != 0 {
                self.holding_one = ones & 1;
                ones = (ones >> 1) + 1;
            } else {
                self.holding_one = ones & 1;
                ones >>= 1;
            }
            self.holding_zero = self.holding_one == 0;
        }
        if m.hybrid && chan == 0 {
            self.update_error_limit(m);
        }
        let c = &mut self.c[chan];
        let (mut low, mut high);
        if ones == 0 {
            low = 0;
            high = c.med(0).wrapping_sub(1);
            c.dec(0);
        } else {
            low = c.med(0);
            c.inc(0);
            if ones == 1 {
                high = low.wrapping_add(c.med(1)).wrapping_sub(1);
                c.dec(1);
            } else {
                low = low.wrapping_add(c.med(1));
                c.inc(1);
                if ones == 2 {
                    high = low.wrapping_add(c.med(2)).wrapping_sub(1);
                    c.dec(2);
                } else {
                    low = low.wrapping_add((ones - 2).wrapping_mul(c.med(2)));
                    high = low.wrapping_add(c.med(2)).wrapping_sub(1);
                    c.inc(2);
                }
            }
        }
        low &= 0x7fff_ffff;
        high &= 0x7fff_ffff;
        if low > high {
            high = low;
        }
        let mut mid = (high + low + 1) >> 1;
        if c.error_limit == 0 {
            mid = bs.code(high - low) + low;
        } else {
            while high - low > c.error_limit {
                if bs.bit() == 1 {
                    low = mid;
                } else {
                    high = mid - 1;
                }
                mid = (high + low + 1) >> 1;
            }
        }
        let sign = bs.bit() == 1;
        let mut corr = 0i32;
        if let Some(cb) = wvc
            && c.error_limit != 0
        {
            let value = cb.code(high - low) + low;
            corr = if sign {
                mid.wrapping_sub(value) as i32
            } else {
                value.wrapping_sub(mid) as i32
            };
        }
        if m.bitrate {
            c.slow_decay();
            c.slow_level = c.slow_level.wrapping_add(log2(mid) as u32);
        }
        Ok((if sign { !(mid as i32) } else { mid as i32 }, corr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_spot_values() {
        // Known values of the format's tables.
        assert_eq!(TABLES.0[1], 1);
        assert_eq!(TABLES.0[255], 255);
        assert_eq!(TABLES.1[0], 0);
        assert_eq!(TABLES.1[255], 255);
        assert_eq!(exp2s(9 << 8), 256);
        assert_eq!(log2(16), 5 << 8);
        assert_eq!(exp2s(5 << 8), 16);
        assert_eq!(exp2s(-(5 << 8)), -16);
    }

    #[test]
    fn truncated_binary_codes() {
        // maxcode 4 (5 values): 2 bits for 0..2, 3 bits for 3 and 4.
        let mut b = Bits::new(&[0b0000_0010]);
        assert_eq!(b.code(4), 2);
        let mut b = Bits::new(&[0b0000_0111]);
        assert_eq!(b.code(4), 4);
    }
}

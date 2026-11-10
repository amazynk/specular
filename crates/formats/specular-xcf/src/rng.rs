//! The random numbers GIMP's Dissolve mode draws, so dissolved layers scatter the same pixels
//! GIMP does. GLib's `GRand` is a 32-bit Mersenne Twister (MT19937), seeded per row from a
//! fixed table that a twister seeded with GIMP's constant fills.

use std::sync::OnceLock;

const N: usize = 624;
const M: usize = 397;

/// MT19937 as GLib's `GRand` runs it.
pub(crate) struct Rand {
    mt: [u32; N],
    i: usize,
}

impl Rand {
    pub(crate) fn new(seed: u32) -> Rand {
        let mut mt = [0u32; N];
        mt[0] = seed;
        for i in 1..N {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Rand { mt, i: N }
    }

    fn twist(&mut self) {
        const UPPER: u32 = 0x8000_0000;
        const LOWER: u32 = 0x7fff_ffff;
        const A: u32 = 0x9908_b0df;
        for k in 0..N {
            let y = (self.mt[k] & UPPER) | (self.mt[(k + 1) % N] & LOWER);
            self.mt[k] = self.mt[(k + M) % N] ^ (y >> 1) ^ if y & 1 != 0 { A } else { 0 };
        }
        self.i = 0;
    }

    /// `g_rand_int`.
    pub(crate) fn int(&mut self) -> u32 {
        if self.i >= N {
            self.twist();
        }
        let mut y = self.mt[self.i];
        self.i += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `g_rand_int_range (rand, 0, 255)`: rejection sampling to an unbiased `0..255`.
    pub(crate) fn below_255(&mut self) -> u32 {
        // Draws above the last whole multiple of 255 below 2^32 are rejected: 2^32 % 255 = 1,
        // so only 0xffffffff is.
        const MAX: u32 = u32::MAX - 1;
        loop {
            let v = self.int();
            if v <= MAX {
                return v % 255;
            }
        }
    }
}

/// GIMP's dissolve seed table: 4096 draws from a twister seeded with 314159265.
pub(crate) fn dissolve_table() -> &'static [u32; 4096] {
    static T: OnceLock<[u32; 4096]> = OnceLock::new();
    T.get_or_init(|| {
        let mut r = Rand::new(314_159_265);
        std::array::from_fn(|_| r.int())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mt19937_reference() {
        // The reference implementation's init_genrand(5489) stream starts 3499211612,
        // 581869302, 3890346734; its 10000th output is 4123659995.
        let mut r = Rand::new(5489);
        assert_eq!(
            [r.int(), r.int(), r.int()],
            [3_499_211_612, 581_869_302, 3_890_346_734]
        );
        let mut r = Rand::new(5489);
        let mut v = 0;
        for _ in 0..10_000 {
            v = r.int();
        }
        assert_eq!(v, 4_123_659_995);
    }
}

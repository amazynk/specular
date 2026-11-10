//! CABAC (9.3): context initialisation, the arithmetic decoding engine with VVC's two-rate
//! probability estimate, bypass and terminating bins.

#[rustfmt::skip]
#[allow(dead_code)]
pub mod init;

pub use init::*;

/// One context variable: the two probability estimates (pStateIdx0 / pStateIdx1, 10 and
/// 14 bits) and their adaptation shifts.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ctx {
    p0: u16,
    p1: u16,
    s0: u8,
    s1: u8,
}

impl Ctx {
    fn new(init: u8, shift: u8, qp: i32) -> Ctx {
        let slope = i32::from(init >> 3);
        let offset = i32::from(init & 7);
        let m = slope - 4;
        let n = offset * 18 + 1;
        let pre = (((m * (qp.clamp(0, 63) - 16)) >> 1) + n).clamp(1, 127);
        let s0 = (shift >> 2) + 2;
        let s1 = (shift & 3) + 3 + s0;
        Ctx {
            p0: (pre << 3) as u16,
            p1: (pre << 7) as u16,
            s0,
            s1,
        }
    }
}

/// All the context variables of a slice.
#[derive(Clone)]
pub struct Contexts(pub Box<[Ctx; NUM_CTX]>);

impl Contexts {
    pub fn new(init_type: usize, qp: i32) -> Contexts {
        let mut c = Box::new([Ctx::default(); NUM_CTX]);
        for (i, v) in c.iter_mut().enumerate() {
            let (iv, sh) = INIT[init_type][i];
            *v = Ctx::new(iv, sh, qp);
        }
        Contexts(c)
    }
}

/// The arithmetic decoder over one substream.
pub struct Engine<'a> {
    data: &'a [u8],
    pos: usize,
    range: u32,
    value: u32,
    bits_needed: i32,
}

impl<'a> Engine<'a> {
    /// Initialises the engine (9.3.2.5) at the start of `data`.
    pub fn new(data: &'a [u8]) -> Engine<'a> {
        let mut e = Engine {
            data,
            pos: 0,
            range: 510,
            value: 0,
            bits_needed: -8,
        };
        e.value = (e.byte() << 8) | e.byte();
        e
    }

    #[inline]
    fn byte(&mut self) -> u32 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        u32::from(b)
    }

    /// Whether the engine has read well past the end of its data (a damaged stream).
    pub fn overrun(&self) -> bool {
        self.pos > self.data.len() + 2
    }

    /// Byte position after a terminating bin equal to 1 (the start of what follows).
    pub fn position(&self) -> usize {
        self.pos
    }

    /// DecodeDecision (9.3.4.3.2).
    #[inline]
    pub fn decode(&mut self, ctx: &mut Ctx) -> u32 {
        let p = u32::from(ctx.p1) + 16 * u32::from(ctx.p0);
        let mps = p >> 14;
        let q = self.range >> 5;
        let lps = ((q * ((if mps != 0 { 32767 - p } else { p }) >> 9)) >> 1) + 4;
        self.range -= lps;
        let scaled = self.range << 7;
        let bin;
        if self.value < scaled {
            bin = mps;
            if scaled < (256 << 7) {
                self.range = scaled >> 6;
                self.value <<= 1;
                self.bits_needed += 1;
                if self.bits_needed == 0 {
                    self.bits_needed = -8;
                    self.value += self.byte();
                }
            }
        } else {
            bin = 1 - mps;
            let n = lps.leading_zeros() - 23;
            self.value = (self.value - scaled) << n;
            self.range = lps << n;
            self.bits_needed += n as i32;
            if self.bits_needed >= 0 {
                self.value += self.byte() << self.bits_needed;
                self.bits_needed -= 8;
            }
        }
        // State transition (9.3.4.3.2.2).
        let (s0, s1) = (ctx.s0, ctx.s1);
        ctx.p0 = ctx.p0 - (ctx.p0 >> s0) + ((1023 * bin as u16) >> s0);
        ctx.p1 = ctx.p1 - (ctx.p1 >> s1) + ((16383 * bin as u16) >> s1);
        bin
    }

    /// DecodeBypass (9.3.4.3.4).
    #[inline]
    pub fn bypass(&mut self) -> u32 {
        self.value <<= 1;
        self.bits_needed += 1;
        if self.bits_needed >= 0 {
            self.bits_needed = -8;
            self.value += self.byte();
        }
        let scaled = self.range << 7;
        if self.value >= scaled {
            self.value -= scaled;
            1
        } else {
            0
        }
    }

    /// `n` bypass bins, most significant first.
    #[inline]
    pub fn bypass_bits(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            v = (v << 1) | self.bypass();
        }
        v
    }

    /// DecodeTerminate (9.3.4.3.5).
    pub fn terminate(&mut self) -> u32 {
        self.range -= 2;
        let scaled = self.range << 7;
        if self.value >= scaled {
            1
        } else {
            if scaled < (256 << 7) {
                self.range = scaled >> 6;
                self.value <<= 1;
                self.bits_needed += 1;
                if self.bits_needed == 0 {
                    self.bits_needed = -8;
                    self.value += self.byte();
                }
            }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_init_matches_equations() {
        // initValue 62 (slope 7, offset 6), shiftIdx 0, SliceQpY 32.
        let c = Ctx::new(62, 0, 32);
        // preCtxState = Clip3(1, 127, ((7 - 4) * (32 - 16) >> 1) + (6 * 18 + 1)) = 127.
        let pre: u32 = 127;
        assert_eq!(c.p0, (pre << 3) as u16);
        assert_eq!(c.p1, (pre << 7) as u16);
        assert_eq!((c.s0, c.s1), (2, 5));
        let all = Contexts::new(0, 26);
        assert_eq!(all.0.len(), NUM_CTX);
    }

    #[test]
    fn bypass_reads_raw_bits() {
        // With value < range << 7 the first bypass bins of 0x00.. are zero.
        let d = [0u8; 8];
        let mut e = Engine::new(&d);
        assert_eq!(e.bypass_bits(8), 0);
        assert_eq!(e.terminate(), 0);
    }
}

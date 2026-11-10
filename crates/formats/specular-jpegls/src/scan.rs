//! Context modelling, prediction and run mode (T.87 Annex A) for one scan.

use crate::bits::Bits;
use crate::{Error, Result};

/// Run lengths per RUNindex (T.87 A.7.1.2).
const J: [u32; 32] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 9, 10, 11, 12, 13,
    14, 15,
];

#[derive(Clone, Copy)]
struct Ctx {
    a: i64,
    b: i32,
    c: i32,
    n: i32,
}

#[derive(Clone, Copy)]
struct RunCtx {
    a: i64,
    n: i32,
    nn: i32,
}

/// Coding parameters of a scan (T.87 A.2.1, C.2.4.1.1).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Params {
    pub maxval: i32,
    pub near: i32,
    pub t1: i32,
    pub t2: i32,
    pub t3: i32,
    pub reset: i32,
}

impl Params {
    /// Fills in defaults for the zero fields of an LSE preset and checks the result.
    pub(crate) fn new(precision: u8, near: u16, p: crate::header::Preset) -> Result<Params> {
        let maxval = if p.maxval != 0 {
            p.maxval as i32
        } else {
            (1i32 << precision) - 1
        };
        let near = near as i32;
        if near > maxval / 2 || near > 255 {
            return Err(Error::Malformed("NEAR too large for MAXVAL"));
        }
        // Default thresholds (T.87 C.2.4.1.1.1), each clamped as CLAMP_1..3 specify.
        let clamp = |v: i32, lo: i32| if v > maxval || v < lo { lo } else { v };
        let (d1, d2, d3) = if maxval >= 128 {
            let f = (maxval.min(4095) + 128) / 256;
            (
                f + 2 + 3 * near,
                4 * f + 3 + 5 * near,
                17 * f + 4 + 7 * near,
            )
        } else {
            let f = 256 / (maxval + 1);
            (
                (3 / f + 3 * near).max(2),
                (7 / f + 5 * near).max(3),
                (21 / f + 7 * near).max(4),
            )
        };
        let t1 = if p.t1 != 0 {
            p.t1 as i32
        } else {
            clamp(d1, near + 1)
        };
        let t2 = if p.t2 != 0 {
            p.t2 as i32
        } else {
            clamp(d2, t1)
        };
        let t3 = if p.t3 != 0 {
            p.t3 as i32
        } else {
            clamp(d3, t2)
        };
        if !(near < t1 && t1 <= t2 && t2 <= t3 && t3 <= maxval.max(near + 1)) {
            return Err(Error::Malformed("invalid thresholds"));
        }
        let reset = if p.reset != 0 { p.reset as i32 } else { 64 };
        if reset < 3 {
            return Err(Error::Malformed("RESET below 3"));
        }
        Ok(Params {
            maxval,
            near,
            t1,
            t2,
            t3,
            reset,
        })
    }
}

pub(crate) struct Coder {
    maxval: i32,
    near: i32,
    range: i32,
    qbpp: u32,
    limit: u32,
    reset: i32,
    /// 2 * NEAR + 1.
    step: i32,
    /// RANGE * step: the modulus of reconstructed values.
    modulo: i32,
    /// Mapped errors from this value up are refused. Conforming streams stay below 2 * RANGE;
    /// the bound is looser (only keeping the arithmetic in range) so streams from encoders that
    /// disagree about RANGE still decode as other decoders decode them.
    max_merr: u32,
    t3: i32,
    /// Quantized gradient for d in -T3..=T3.
    quant: Vec<i32>,
    a_init: i64,
    ctx: [Ctx; 365],
    run: [RunCtx; 2],
}

/// Smallest k with n << k >= a (k <= 32).
#[inline(always)]
fn golomb_k(n: i32, a: i64) -> u32 {
    let (n, a) = (n as u64, a as u64);
    let k0 = n.leading_zeros().saturating_sub(a.leading_zeros());
    (k0 + ((n << k0) < a) as u32).min(32)
}

impl Coder {
    pub(crate) fn new(p: &Params) -> Coder {
        let near = p.near;
        let range = if near == 0 {
            p.maxval + 1
        } else {
            (p.maxval + 2 * near) / (2 * near + 1) + 1
        };
        let qbpp = 32 - ((range - 1) as u32).leading_zeros();
        let bpp = (32 - (p.maxval as u32).leading_zeros()).max(2);
        let limit = 2 * (bpp + bpp.max(8));
        let t3 = p.t3;
        let quant = (-t3..=t3)
            .map(|d| {
                let a = d.abs();
                let q = if a >= p.t3 {
                    4
                } else if a >= p.t2 {
                    3
                } else if a >= p.t1 {
                    2
                } else if a > near {
                    1
                } else {
                    0
                };
                if d < 0 { -q } else { q }
            })
            .collect();
        let a_init = ((range + 32) / 64).max(2) as i64;
        let mut c = Coder {
            maxval: p.maxval,
            near,
            range,
            qbpp,
            limit,
            reset: p.reset,
            step: 2 * near + 1,
            modulo: range * (2 * near + 1),
            max_merr: (1 << 30) / (2 * near + 1) as u32,
            t3,
            quant,
            a_init,
            ctx: [Ctx {
                a: 0,
                b: 0,
                c: 0,
                n: 1,
            }; 365],
            run: [RunCtx { a: 0, n: 1, nn: 0 }; 2],
        };
        c.reset();
        c
    }

    /// Initial context state (start of a scan or restart interval).
    pub(crate) fn reset(&mut self) {
        self.ctx.fill(Ctx {
            a: self.a_init,
            b: 0,
            c: 0,
            n: 1,
        });
        self.run = [RunCtx {
            a: self.a_init,
            n: 1,
            nn: 0,
        }; 2];
    }

    #[inline(always)]
    fn q(&self, d: i32) -> i32 {
        self.quant[(d.clamp(-self.t3, self.t3) + self.t3) as usize]
    }

    /// Context number of a sample: 0 means "flat" (run mode outside sample interleave).
    #[inline(always)]
    fn context(&self, ra: i32, rb: i32, rc: i32, rd: i32) -> i32 {
        81 * self.q(rd - rb) + 9 * self.q(rb - rc) + self.q(rc - ra)
    }

    /// Modulo reduction and clamping of a reconstructed value (T.87 A.4.5).
    #[inline(always)]
    fn fix(&self, rx: i32) -> i32 {
        let mut rx = rx;
        if rx < -self.near {
            rx += self.modulo;
        } else if rx > self.maxval + self.near {
            rx -= self.modulo;
        }
        rx.clamp(0, self.maxval)
    }

    /// Regular mode for one sample with context `q` (may be 0 in sample interleave).
    #[inline(always)]
    fn regular<const LOSSLESS: bool>(
        &mut self,
        br: &mut Bits,
        q: i32,
        ra: i32,
        rb: i32,
        rc: i32,
    ) -> Result<i32> {
        let sign = q >> 31;
        let ctx = &mut self.ctx[((q ^ sign) - sign) as usize];
        // Median edge detector, then bias correction.
        let px = (ra + rb - rc).clamp(ra.min(rb), ra.max(rb));
        let px = (px + ((ctx.c ^ sign) - sign)).clamp(0, self.maxval);
        let k = golomb_k(ctx.n, ctx.a);
        let m = br.golomb(k, self.limit - self.qbpp - 1, self.qbpp)?;
        if m >= self.max_merr {
            return Err(br.corrupt("prediction error out of range"));
        }
        let mut e = ((m >> 1) as i32) ^ -((m & 1) as i32);
        if LOSSLESS && k == 0 && 2 * ctx.b <= -ctx.n {
            e = !e;
        }
        // Context update (A.6).
        let ed = if LOSSLESS { e } else { e * self.step };
        ctx.b += ed;
        ctx.a += e.unsigned_abs() as i64;
        if ctx.n == self.reset {
            ctx.a >>= 1;
            ctx.b >>= 1;
            ctx.n >>= 1;
        }
        ctx.n += 1;
        if ctx.b <= -ctx.n {
            ctx.b += ctx.n;
            if ctx.c > -128 {
                ctx.c -= 1;
            }
            if ctx.b <= -ctx.n {
                ctx.b = -ctx.n + 1;
            }
        } else if ctx.b > 0 {
            ctx.b -= ctx.n;
            if ctx.c < 127 {
                ctx.c += 1;
            }
            if ctx.b > 0 {
                ctx.b = 0;
            }
        }
        Ok(if LOSSLESS {
            let rx = px + ((e ^ sign) - sign);
            let rx = if rx < 0 {
                rx + self.range
            } else if rx > self.maxval {
                rx - self.range
            } else {
                rx
            };
            rx.clamp(0, self.maxval)
        } else {
            self.fix(px + ((ed ^ sign) - sign))
        })
    }

    /// Error value of a run interruption sample (A.7.2) for context `ritype`, with the context
    /// update. `ri` is RUNindex before the interruption.
    #[inline(never)]
    fn interruption_error(&mut self, br: &mut Bits, ritype: usize, ri: usize) -> Result<i32> {
        let c = &mut self.run[ritype];
        let temp = if ritype == 1 {
            c.a + (c.n >> 1) as i64
        } else {
            c.a
        };
        let k = golomb_k(c.n, temp);
        let glimit = self.limit - J[ri] - 1;
        let em = br.golomb(k, glimit - self.qbpp - 1, self.qbpp)?;
        if em >= self.max_merr {
            return Err(br.corrupt("run interruption error out of range"));
        }
        let t = em as i32 + ritype as i32;
        let mag = (t + 1) >> 1;
        let positive = (t & 1 == 1) == (k == 0 && 2 * c.nn < c.n);
        let e = if positive { mag } else { -mag };
        if e < 0 {
            c.nn += 1;
        }
        c.a += ((em as i32 + 1 - ritype as i32) >> 1) as i64;
        if c.n == self.reset {
            c.a >>= 1;
            c.n >>= 1;
            c.nn >>= 1;
        }
        c.n += 1;
        Ok(e)
    }

    /// Decodes one line of a component. `prev` and `cur` hold the line with one padding sample
    /// on each side; the caller sets `cur[0]` and `prev[w + 1]`.
    pub(crate) fn line<const LOSSLESS: bool>(
        &mut self,
        br: &mut Bits,
        prev: &[i32],
        cur: &mut [i32],
        ri: &mut usize,
    ) -> Result<()> {
        let w = cur.len() - 2;
        debug_assert_eq!(prev.len(), cur.len());
        let mut x = 1;
        let mut ra = cur[0];
        let mut rc = prev[0];
        let mut rb = prev[1];
        while x <= w {
            let rd = prev[x + 1];
            let q = self.context(ra, rb, rc, rd);
            if q != 0 {
                ra = self.regular::<LOSSLESS>(br, q, ra, rb, rc)?;
                cur[x] = ra;
                x += 1;
                rc = rb;
                rb = rd;
            } else {
                x = self.run_mode(br, prev, cur, x, ri)?;
                if x <= w {
                    ra = cur[x - 1];
                    rc = prev[x - 1];
                    rb = prev[x];
                }
            }
        }
        Ok(())
    }

    /// Run mode starting at `x` (A.7); returns the position after the run and its interruption.
    #[inline(never)]
    fn run_mode(
        &mut self,
        br: &mut Bits,
        prev: &[i32],
        cur: &mut [i32],
        mut x: usize,
        ri: &mut usize,
    ) -> Result<usize> {
        let w = cur.len() - 2;
        let ra = cur[x - 1];
        loop {
            let left = w + 1 - x;
            if br.bit() {
                let seg = 1usize << J[*ri];
                let n = seg.min(left);
                cur[x..x + n].fill(ra);
                x += n;
                if n == seg {
                    *ri = (*ri + 1).min(31);
                }
                if x > w {
                    return Ok(x);
                }
            } else {
                let n = br.read(J[*ri]) as usize;
                if n >= left {
                    return Err(br.corrupt("run longer than the line"));
                }
                cur[x..x + n].fill(ra);
                x += n;
                let rb = prev[x];
                let ritype = ((ra - rb).abs() <= self.near) as usize;
                let e = self.interruption_error(br, ritype, *ri)?;
                let (px, e) = if ritype == 1 {
                    (ra, e)
                } else if ra > rb {
                    (rb, -e)
                } else {
                    (rb, e)
                };
                cur[x] = self.fix(px + e * self.step);
                *ri = ri.saturating_sub(1);
                return Ok(x + 1);
            }
        }
    }

    /// Decodes one line of a sample-interleaved scan of `nc` components. Lines hold `nc` samples
    /// per position, with one padding position on each side.
    pub(crate) fn line_interleaved(
        &mut self,
        br: &mut Bits,
        prev: &[i32],
        cur: &mut [i32],
        nc: usize,
        ri: &mut usize,
    ) -> Result<()> {
        let w = cur.len() / nc - 2;
        let mut x = 1;
        let mut qs = [0i32; 4];
        while x <= w {
            let p = x * nc;
            let mut flat = true;
            for c in 0..nc {
                let (ra, rb, rc, rd) = (
                    cur[p - nc + c],
                    prev[p + c],
                    prev[p - nc + c],
                    prev[p + nc + c],
                );
                qs[c] = self.context(ra, rb, rc, rd);
                flat &= qs[c] == 0;
            }
            if !flat {
                for c in 0..nc {
                    let (ra, rb, rc) = (cur[p - nc + c], prev[p + c], prev[p - nc + c]);
                    cur[p + c] = if self.near == 0 {
                        self.regular::<true>(br, qs[c], ra, rb, rc)?
                    } else {
                        self.regular::<false>(br, qs[c], ra, rb, rc)?
                    };
                }
                x += 1;
                continue;
            }
            // Joint run of all components.
            let start = x;
            let interrupted = loop {
                let left = w + 1 - x;
                if br.bit() {
                    let seg = 1usize << J[*ri];
                    let n = seg.min(left);
                    x += n;
                    if n == seg {
                        *ri = (*ri + 1).min(31);
                    }
                    if x > w {
                        break false;
                    }
                } else {
                    let n = br.read(J[*ri]) as usize;
                    if n >= left {
                        return Err(br.corrupt("run longer than the line"));
                    }
                    x += n;
                    break true;
                }
            };
            for i in start..x {
                for c in 0..nc {
                    cur[i * nc + c] = cur[(start - 1) * nc + c];
                }
            }
            if interrupted {
                let p = x * nc;
                for c in 0..nc {
                    let (ra, rb) = (cur[p - nc + c], prev[p + c]);
                    let e = self.interruption_error(br, 0, *ri)?;
                    let e = if ra > rb { -e } else { e };
                    cur[p + c] = self.fix(rb + e * self.step);
                }
                *ri = ri.saturating_sub(1);
                x += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Preset;

    #[test]
    fn default_thresholds() {
        let p = Params::new(8, 0, Preset::default()).unwrap();
        assert_eq!((p.t1, p.t2, p.t3, p.reset), (3, 7, 21, 64));
        let p = Params::new(16, 0, Preset::default()).unwrap();
        assert_eq!((p.t1, p.t2, p.t3), (18, 67, 276));
        let p = Params::new(12, 0, Preset::default()).unwrap();
        assert_eq!((p.t1, p.t2, p.t3), (18, 67, 276));
        let p = Params::new(8, 3, Preset::default()).unwrap();
        assert_eq!((p.t1, p.t2, p.t3), (12, 22, 42));
        let p = Params::new(4, 0, Preset::default()).unwrap();
        assert_eq!((p.t1, p.t2, p.t3), (2, 3, 4));
    }

    #[test]
    fn golomb_k_matches_definition() {
        for n in 1..300 {
            for a in [1i64, 2, 3, 7, 64, 65, 1000, 1 << 20, (1 << 33) + 5] {
                let mut k = 0;
                while ((n as i64) << k) < a {
                    k += 1;
                }
                assert_eq!(golomb_k(n, a), k.min(32), "n {n} a {a}");
            }
        }
    }
}

//! HTJ2K (ITU-T T.814 / ISO/IEC 15444-15) code-block decoder: the HT cleanup pass and the
//! optional HT SigProp and MagRef passes of one HT set.
//!
//! The cleanup segment carries three byte streams: MagSgn (forward from the start), MEL
//! (forward from `Lcup - Scup`) and VLC (backward from the end). The block is scanned in line
//! pairs of 2x2 quads, two quads at a time: the VLC stream gives each quad's significance
//! pattern and exponent bounds (context-coded, MEL-assisted in zero contexts), then the U-VLC
//! residuals, then MagSgn gives the magnitude and sign bits. The refinement segment carries
//! SigProp forward and MagRef backward.
//!
//! Output uses the tier-1 magnitude format: twice the magnitude plus the half-interval bit,
//! sign in the top bit of the output word: `u32` (bit 31) for up to 30 magnitude bit-planes,
//! `u64` (bit 63) for deeper blocks such as 32-bit components. All reads are bounds-checked;
//! bytes past a stream's end read as its fill.

use crate::ht_vlc::{VLC0, VLC1};

/// Most bit-planes a block decoded into `u64` words can have.
pub(crate) const WIDE_BITPLANES: u8 = <u64 as Word>::MAX_P as u8 + 1;

/// An output sample word. Values are computed in `u64` and stored narrowed.
pub(crate) trait Word: Copy + Default + PartialEq {
    /// The sign bit.
    const SIGN: u64;
    /// Highest bit-plane a magnitude bit may reach (keeps the half bit and sign apart).
    const MAX_P: u32;
    fn get(self) -> u64;
    fn put(v: u64) -> Self;
}

impl Word for u32 {
    const SIGN: u64 = 1 << 31;
    const MAX_P: u32 = 30;
    #[inline(always)]
    fn get(self) -> u64 {
        self as u64
    }
    #[inline(always)]
    fn put(v: u64) -> Self {
        v as u32
    }
}

impl Word for u64 {
    const SIGN: u64 = 1 << 63;
    /// MagSgn reads one sample's bits in one go from a 57-bit window.
    const MAX_P: u32 = 56;
    #[inline(always)]
    fn get(self) -> u64 {
        self
    }
    #[inline(always)]
    fn put(v: u64) -> Self {
        v
    }
}

/// Forward reader, least significant bit first, for MagSgn and SigProp. A byte following
/// 0xFF carries 7 bits (its stuffed MSB is dropped).
struct Fwd<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    ff: bool,
    fill: u8,
}

impl<'a> Fwd<'a> {
    fn new(d: &'a [u8], fill: u8) -> Self {
        Fwd {
            d,
            pos: 0,
            acc: 0,
            n: 0,
            ff: false,
            fill,
        }
    }

    #[inline]
    fn refill(&mut self) {
        // Fast path: eight bytes without 0xFF and no pending stuffing.
        if !self.ff
            && self.n <= 8
            && let Some(b) = self.d.get(self.pos..self.pos + 7)
        {
            let mut w = [0u8; 8];
            w[..7].copy_from_slice(b);
            let v = u64::from_le_bytes(w);
            // Any 0xFF byte among the 7? (Zero-byte test on the complement.)
            let x = !v | 0xFF << 56;
            let zero = x.wrapping_sub(0x0101_0101_0101_0101) & !x & 0x8080_8080_8080_8080;
            if zero == 0 {
                self.acc |= v << self.n;
                self.n += 56;
                self.pos += 7;
                return;
            }
        }
        while self.n <= 56 {
            let b = match self.d.get(self.pos) {
                Some(&b) => {
                    self.pos += 1;
                    b
                }
                None => self.fill,
            };
            if self.ff {
                self.acc |= ((b & 0x7F) as u64) << self.n;
                self.n += 7;
            } else {
                self.acc |= (b as u64) << self.n;
                self.n += 8;
            }
            self.ff = b == 0xFF;
        }
    }

    /// The next 32 bits (at least) without consuming them.
    #[inline(always)]
    fn peek(&mut self) -> u64 {
        if self.n < 32 {
            self.refill();
        }
        self.acc
    }

    /// At least the next `k <= 57` bits without consuming them.
    #[inline(always)]
    fn peek_n(&mut self, k: u32) -> u64 {
        if self.n < k {
            self.refill();
        }
        self.acc
    }

    /// Consume `k` bits (after a [`Fwd::peek`] or [`Fwd::peek_n`] that covers them).
    #[inline(always)]
    fn skip(&mut self, k: u32) {
        self.acc >>= k;
        self.n -= k;
    }

    #[inline(always)]
    fn bit(&mut self) -> u32 {
        let b = (self.peek() & 1) as u32;
        self.skip(1);
        b
    }
}

/// Backward reader, least significant bit first, for VLC and MagRef. A byte carries only 7
/// bits when the byte above it was > 0x8F and its own low 7 bits are all ones.
struct Rev<'a> {
    d: &'a [u8],
    /// Number of bytes not yet read (the next byte is `d[left - 1]`).
    left: usize,
    acc: u64,
    n: u32,
    prev_big: bool,
}

impl<'a> Rev<'a> {
    #[inline]
    fn refill(&mut self) {
        while self.n <= 56 {
            let b = if self.left > 0 {
                self.left -= 1;
                self.d[self.left]
            } else {
                0
            };
            if self.prev_big && b & 0x7F == 0x7F {
                self.acc |= ((b & 0x7F) as u64) << self.n;
                self.n += 7;
            } else {
                self.acc |= (b as u64) << self.n;
                self.n += 8;
            }
            self.prev_big = b > 0x8F;
        }
    }

    #[inline(always)]
    fn peek(&mut self) -> u64 {
        if self.n < 32 {
            self.refill();
        }
        self.acc
    }

    #[inline(always)]
    fn skip(&mut self, k: u32) {
        self.acc >>= k;
        self.n -= k;
    }

    #[inline(always)]
    fn bit(&mut self) -> u32 {
        let b = (self.peek() & 1) as u32;
        self.skip(1);
        b
    }
}

/// MEL decoder (T.814 clause 7.3.3): adaptive run-length code read forward, most
/// significant bit first, with the same 0xFF stuffing as MagSgn; reads 0xFF past its end.
struct Mel<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    ff: bool,
    k: usize,
    zeros: u32,
    one: bool,
}

const MEL_E: [u8; 13] = [0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 4, 5];

impl<'a> Mel<'a> {
    fn bits(&mut self, e: u32) -> u32 {
        if self.n < e {
            while self.n <= 56 {
                let mut b = match self.d.get(self.pos) {
                    Some(&b) => {
                        self.pos += 1;
                        b
                    }
                    None => 0xFF,
                };
                // The last MEL byte shares its low nibble with Scup: read it as ones.
                if self.pos == self.d.len() {
                    b |= 0x0F;
                }
                let k = if self.ff { 7 } else { 8 };
                self.acc |= ((b as u64) & ((1 << k) - 1)) << (64 - self.n - k);
                self.n += k;
                self.ff = b == 0xFF;
            }
        }
        if e == 0 {
            return 0;
        }
        let v = (self.acc >> (64 - e)) as u32;
        self.acc <<= e;
        self.n -= e;
        v
    }

    /// The next MEL event: true for a significant quad (pair).
    #[inline]
    fn event(&mut self) -> bool {
        loop {
            if self.zeros > 0 {
                self.zeros -= 1;
                return false;
            }
            if self.one {
                self.one = false;
                return true;
            }
            let e = MEL_E[self.k] as u32;
            if self.bits(1) == 1 {
                self.zeros = 1 << e;
                self.k = (self.k + 1).min(12);
            } else {
                self.zeros = self.bits(e);
                self.one = true;
                self.k = self.k.saturating_sub(1);
            }
        }
    }
}

/// U-VLC prefix: `(u_pfx, suffix length)`.
#[inline(always)]
fn uvlc_prefix(vlc: &mut Rev) -> (u32, u32) {
    let b = vlc.peek() as u32;
    if b & 1 != 0 {
        vlc.skip(1);
        (1, 0)
    } else if b & 2 != 0 {
        vlc.skip(2);
        (2, 0)
    } else if b & 4 != 0 {
        vlc.skip(3);
        (3, 1)
    } else {
        vlc.skip(3);
        (5, 5)
    }
}

/// U-VLC suffix of `len` bits.
#[inline(always)]
fn uvlc_suffix(vlc: &mut Rev, len: u32) -> u32 {
    if len == 0 {
        return 0;
    }
    let v = vlc.peek() as u32 & ((1 << len) - 1);
    vlc.skip(len);
    v
}

/// U-VLC extension: a 5-bit suffix of 28 or more is followed (after the pair's suffixes) by
/// 4 bits worth four each.
#[inline(always)]
fn uvlc_ext(vlc: &mut Rev, len: u32, sfx: u32) -> u32 {
    if len == 5 && sfx >= 28 {
        let v = vlc.peek() as u32 & 0xF;
        vlc.skip(4);
        4 * v
    } else {
        0
    }
}

/// Decode one quad's CxtVLC codeword in context `c` (consulting MEL in context 0).
#[inline(always)]
fn quad_vlc(c: u32, lut: &[u16; 1024], vlc: &mut Rev, mel: &mut Mel) -> Option<u32> {
    if c == 0 && !mel.event() {
        return Some(0);
    }
    let e = lut[(c << 7 | (vlc.peek() as u32 & 0x7F)) as usize] as u32;
    let len = e & 7;
    if len == 0 {
        return None;
    }
    vlc.skip(len);
    Some(e)
}

#[inline(always)]
fn rho(t: u32) -> u32 {
    (t >> 4) & 0xF
}

/// Decode the magnitudes and signs of one quad at quad column `qx` of line pair `y0`.
/// `vb` receives the bottom-row exponent values (index `x + 1`).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn quad_magsgn<W: Word>(
    out: &mut [W],
    w: usize,
    h: usize,
    y0: usize,
    qx: usize,
    t: u32,
    uq: u32,
    p: u32,
    ms: &mut Fwd,
    vb: &mut [u64; 1028],
) {
    let r = rho(t);
    let x0 = 2 * qx;
    let mut v_bot = [0u64; 2];
    if r != 0 {
        for n in 0..4u32 {
            if r >> n & 1 == 0 {
                continue;
            }
            let m = uq - (t >> (12 + n) & 1);
            let bits = ms.peek_n(m);
            ms.skip(m);
            let v = (bits & ((1u64 << m) - 1)) | ((t >> (8 + n) & 1) as u64) << m;
            let val = if v & 1 != 0 { W::SIGN } else { 0 } | ((v | 1) + 2) << p;
            let (x, y) = (x0 + (n as usize >> 1), y0 + (n as usize & 1));
            if x < w && y < h {
                out[y * w + x] = W::put(val);
            }
            if n & 1 == 1 {
                v_bot[n as usize >> 1] = v | 1;
            }
        }
    }
    vb[x0 + 1] = v_bot[0];
    if x0 + 1 < w {
        vb[x0 + 2] = v_bot[1];
    }
}

/// HT cleanup pass. `p` is the bit-plane of the magnitudes' least significant bit.
fn cleanup<W: Word>(out: &mut [W], w: usize, h: usize, cup: &[u8], p: u32) -> Option<()> {
    let lcup = cup.len();
    if lcup < 2 {
        return None;
    }
    let scup = ((cup[lcup - 1] as usize) << 4) | (cup[lcup - 2] & 0xF) as usize;
    if !(2..=lcup.min(4079)).contains(&scup) {
        return None;
    }
    let pcup = lcup - scup;
    let mut ms = Fwd::new(&cup[..pcup], 0xFF);
    let mut mel = Mel {
        d: &cup[pcup..lcup - 1],
        pos: 0,
        acc: 0,
        n: 0,
        ff: false,
        k: 0,
        zeros: 0,
        one: false,
    };
    // VLC: the last byte holds only Scup bits; the one before contributes its upper nibble
    // (3 bits when it is a stuffed 0x?F byte after the notional 0xFF).
    let b = cup[lcup - 2] | 0x0F;
    let k = if b & 0x7F == 0x7F { 3 } else { 4 };
    let mut vlc = Rev {
        d: &cup[pcup..lcup - 2],
        left: scup - 2,
        acc: ((b >> 4) as u64) & ((1 << k) - 1),
        n: k,
        prev_big: b > 0x8F,
    };

    let nq = w.div_ceil(2);
    // Quad significance of the previous and current line pair (index qx + 1, zero-padded).
    let mut rho_up = [0u8; 516];
    let mut rho_cur = [0u8; 516];
    // Bottom-row `v | 1` values of the previous and current line pair (index x + 1).
    let mut v_up = [0u64; 1028];
    let mut v_cur = [0u64; 1028];

    for y0 in (0..h).step_by(2) {
        let first = y0 == 0;
        let lut = if first { &VLC0 } else { &VLC1 };
        let mut left = 0u32;
        let mut qx = 0;
        while qx < nq {
            let has1 = qx + 1 < nq;
            let ctx = |left: u32, qx: usize, up: &[u8; 516]| -> u32 {
                if first {
                    (left | left >> 1) & 1 | (left >> 1) & 2 | (left >> 1) & 4
                } else {
                    let (a, n, f) = (up[qx] as u32, up[qx + 1] as u32, up[qx + 2] as u32);
                    ((a >> 3 | n >> 1) & 1)
                        | ((left >> 2 | left >> 3) & 1) << 1
                        | ((n >> 3 | f >> 1) & 1) << 2
                }
            };
            let t0 = quad_vlc(ctx(left, qx, &rho_up), lut, &mut vlc, &mut mel)?;
            rho_cur[qx + 1] = rho(t0) as u8;
            let t1 = if has1 {
                quad_vlc(ctx(rho(t0), qx + 1, &rho_up), lut, &mut vlc, &mut mel)?
            } else {
                0
            };
            rho_cur[qx + 2] = rho(t1) as u8;
            left = rho(t1);

            // U-VLC: prefixes, then suffixes.
            // Order: both prefixes, both suffixes, then both extensions.
            let (uo0, uo1) = (t0 >> 3 & 1 != 0, t1 >> 3 & 1 != 0);
            let (mut u0, mut u1);
            let (l0, s0, l1, s1);
            if first && uo0 && uo1 {
                if mel.event() {
                    let (p0, ll0) = uvlc_prefix(&mut vlc);
                    let (p1, ll1) = uvlc_prefix(&mut vlc);
                    (l0, l1) = (ll0, ll1);
                    s0 = uvlc_suffix(&mut vlc, l0);
                    s1 = uvlc_suffix(&mut vlc, l1);
                    u0 = 2 + p0 + s0;
                    u1 = 2 + p1 + s1;
                } else {
                    let (p0, ll0) = uvlc_prefix(&mut vlc);
                    l0 = ll0;
                    if p0 > 2 {
                        u1 = 1 + vlc.bit();
                        (l1, s1) = (0, 0);
                        s0 = uvlc_suffix(&mut vlc, l0);
                        u0 = p0 + s0;
                    } else {
                        let (p1, ll1) = uvlc_prefix(&mut vlc);
                        l1 = ll1;
                        s0 = uvlc_suffix(&mut vlc, l0);
                        s1 = uvlc_suffix(&mut vlc, l1);
                        u0 = p0 + s0;
                        u1 = p1 + s1;
                    }
                }
            } else {
                let (p0, ll0) = if uo0 { uvlc_prefix(&mut vlc) } else { (0, 0) };
                let (p1, ll1) = if uo1 { uvlc_prefix(&mut vlc) } else { (0, 0) };
                (l0, l1) = (ll0, ll1);
                s0 = uvlc_suffix(&mut vlc, l0);
                s1 = uvlc_suffix(&mut vlc, l1);
                u0 = p0 + s0;
                u1 = p1 + s1;
            }
            u0 += uvlc_ext(&mut vlc, l0, s0);
            u1 += uvlc_ext(&mut vlc, l1, s1);

            // Exponent bound U_q = kappa_q + u_q.
            for (j, (t, u)) in [(t0, u0), (t1, u1)].into_iter().enumerate() {
                let q = qx + j;
                if j == 1 && !has1 {
                    break;
                }
                let r = rho(t);
                let kappa = if first || r.count_ones() < 2 {
                    1
                } else {
                    let x = 2 * q;
                    let e = v_up[x] | v_up[x + 1] | v_up[x + 2] | v_up[x + 3];
                    63 - (e | 2).leading_zeros()
                };
                let uq = kappa + u;
                if r != 0 && uq + p > W::MAX_P {
                    return None;
                }
                quad_magsgn(out, w, h, y0, q, t, uq, p, &mut ms, &mut v_cur);
            }
            qx += 2;
        }
        std::mem::swap(&mut rho_up, &mut rho_cur);
        std::mem::swap(&mut v_up, &mut v_cur);
    }
    Some(())
}

/// Is sample `(x, y)` significant (any non-zero value)?
#[inline(always)]
fn sig<W: Word>(out: &[W], w: usize, h: usize, x: isize, y: isize) -> bool {
    x >= 0
        && y >= 0
        && (x as usize) < w
        && (y as usize) < h
        && out[y as usize * w + x as usize] != W::default()
}

/// HT SigProp pass for bit-plane `p - 1`: newly significant samples get `3 << (p - 1)`
/// (bit `p - 1` set marks them for MagRef).
fn sigprop<W: Word>(out: &mut [W], w: usize, h: usize, seg: &[u8], p: u32, causal: bool) {
    let mut sp = Fwd::new(seg, 0);
    let val = W::put(3u64 << (p - 1));
    let mut new: [(usize, usize); 16] = [(0, 0); 16];
    for y0 in (0..h).step_by(4) {
        let y1 = (y0 + 4).min(h);
        for x0 in (0..w).step_by(4) {
            let x1 = (x0 + 4).min(w);
            let mut nn = 0;
            for x in x0..x1 {
                for y in y0..y1 {
                    if out[y * w + x] != W::default() {
                        continue;
                    }
                    let (xi, yi) = (x as isize, y as isize);
                    let below = !(causal && y == y1 - 1);
                    let mut any = sig(out, w, h, xi - 1, yi)
                        || sig(out, w, h, xi + 1, yi)
                        || sig(out, w, h, xi - 1, yi - 1)
                        || sig(out, w, h, xi, yi - 1)
                        || sig(out, w, h, xi + 1, yi - 1);
                    if !any && below {
                        any = sig(out, w, h, xi - 1, yi + 1)
                            || sig(out, w, h, xi, yi + 1)
                            || sig(out, w, h, xi + 1, yi + 1);
                    }
                    if any && sp.bit() == 1 {
                        // Mark significant now (sign follows) so later samples see it.
                        out[y * w + x] = val;
                        new[nn] = (x, y);
                        nn += 1;
                    }
                }
            }
            for &(x, y) in &new[..nn] {
                if sp.bit() == 1 {
                    out[y * w + x] = W::put(out[y * w + x].get() | W::SIGN);
                }
            }
        }
    }
}

/// HT MagRef pass for bit-plane `p - 1` over the cleanup-significant samples.
fn magref<W: Word>(out: &mut [W], w: usize, h: usize, seg: &[u8], p: u32) {
    let mut mr = Rev {
        d: seg,
        left: seg.len(),
        acc: 0,
        n: 0,
        prev_big: true,
    };
    let half = 1u64 << (p - 1);
    for y0 in (0..h).step_by(4) {
        let y1 = (y0 + 4).min(h);
        for x in 0..w {
            for y in y0..y1 {
                let v = out[y * w + x].get();
                if v != 0 && v & half == 0 {
                    // (2mu + 1) << p becomes (4mu + 2b + 1) << (p - 1).
                    out[y * w + x] = W::put(v - half + ((mr.bit() as u64) << p));
                }
            }
        }
    }
}

/// Decode one HT code-block into `out` (`w * h`, zeroed by the caller). `cup` is the cleanup
/// segment, `refine` the SigProp/MagRef segment holding `ref_passes` (0..=2) passes, and `p`
/// the bit-plane of the cleanup pass. Returns false on malformed data (the output is then
/// incomplete).
#[allow(clippy::too_many_arguments)]
pub(crate) fn decode<W: Word>(
    out: &mut [W],
    w: usize,
    h: usize,
    cup: &[u8],
    refine: &[u8],
    ref_passes: u32,
    p: u32,
    causal: bool,
) -> bool {
    if w == 0 || h == 0 {
        return true;
    }
    if w > 1024 || w * h > 4096 || out.len() < w * h || p > W::MAX_P {
        return false;
    }
    if cleanup(out, w, h, cup, p).is_none() {
        return false;
    }
    if ref_passes >= 1 && p >= 1 {
        sigprop(out, w, h, refine, p, causal);
        if ref_passes >= 2 {
            magref(out, w, h, refine, p);
        }
    }
    true
}

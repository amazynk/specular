//! Specular's own GSM 06.10 full-rate speech decoder (ETSI EN 300 961, "RPE-LTP"), written
//! from the standard's description of the decoder: 160 samples at 8 kHz from 260 bits.
//!
//! Two packings are read:
//! - the plain one (`.gsm` files, Sun audio, the toast tools): 33 bytes a frame, a 4-bit
//!   signature `0xD` then the 260 bits, most significant bit first;
//! - Microsoft's (WAVE format tag 0x31, "WAV49"): 65 bytes for two frames, the bits least
//!   significant first, no signature.
//!
//! The arithmetic is the standard's 16-bit fixed point (saturating adds, rounded
//! multiplies), so the output is bit-exact with any conforming decoder.

#![deny(unsafe_code)]
#![allow(clippy::needless_range_loop)]

/// Samples in a frame.
pub const FRAME_SAMPLES: usize = 160;
/// Bytes of a plain frame.
pub const FRAME_BYTES: usize = 33;
/// Bytes of a Microsoft block (two frames).
pub const MS_BLOCK_BYTES: usize = 65;

/// One frame's parameters (EN 300 961 table 1.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Params {
    pub larc: [u8; 8],
    pub nc: [u8; 4],
    pub bc: [u8; 4],
    pub mc: [u8; 4],
    pub xmaxc: [u8; 4],
    pub xmc: [[u8; 13]; 4],
}

/// Bit widths of the eight log-area ratios.
const LAR_BITS: [u32; 8] = [6, 6, 5, 5, 4, 4, 3, 3];

impl Params {
    /// Reads the 260 bits with `next(width)`.
    fn read(mut next: impl FnMut(u32) -> u8) -> Params {
        let mut p = Params::default();
        for (l, &w) in p.larc.iter_mut().zip(&LAR_BITS) {
            *l = next(w);
        }
        for s in 0..4 {
            p.nc[s] = next(7);
            p.bc[s] = next(2);
            p.mc[s] = next(2);
            p.xmaxc[s] = next(6);
            for x in p.xmc[s].iter_mut() {
                *x = next(3);
            }
        }
        p
    }

    /// A plain 33-byte frame (`None` unless it is 33 bytes with the `0xD` signature).
    pub fn plain(frame: &[u8]) -> Option<Params> {
        if frame.len() < FRAME_BYTES || frame[0] >> 4 != 0xD {
            return None;
        }
        let mut bit = 4usize;
        Some(Params::read(|w| {
            let mut v = 0u8;
            for _ in 0..w {
                let b = (frame[bit / 8] >> (7 - bit % 8)) & 1;
                v = (v << 1) | b;
                bit += 1;
            }
            v
        }))
    }

    /// The two frames of a Microsoft 65-byte block.
    pub fn ms(block: &[u8]) -> Option<[Params; 2]> {
        if block.len() < MS_BLOCK_BYTES {
            return None;
        }
        let mut bit = 0usize;
        let mut next = |w: u32| {
            let mut v = 0u8;
            for k in 0..w {
                let b = (block[bit / 8] >> (bit % 8)) & 1;
                v |= b << k;
                bit += 1;
            }
            v
        };
        let a = Params::read(&mut next);
        let b = Params::read(&mut next);
        Some([a, b])
    }
}

/// Whether `d` looks like a plain `.gsm` file: whole 33-byte frames (a cut last frame is
/// allowed), each with the `0xD` signature, at least two of them.
pub fn is_plain_stream(d: &[u8]) -> bool {
    let frames = d.len() / FRAME_BYTES;
    frames >= 2
        && d.chunks_exact(FRAME_BYTES)
            .take(64)
            .all(|f| f[0] >> 4 == 0xD)
}

const MIN: i16 = i16::MIN;
const MAX: i16 = i16::MAX;

fn add(a: i16, b: i16) -> i16 {
    a.saturating_add(b)
}
fn sub(a: i16, b: i16) -> i16 {
    a.saturating_sub(b)
}
/// The rounded Q15 product.
fn mult_r(a: i16, b: i16) -> i16 {
    if a == MIN && b == MIN {
        MAX
    } else {
        ((i32::from(a) * i32::from(b) + 16384) >> 15) as i16
    }
}
fn abs(a: i16) -> i16 {
    if a == MIN { MAX } else { a.abs() }
}

/// A decoder's memory between frames.
#[derive(Clone, Debug)]
pub struct Decoder {
    /// The previous frame's decoded log-area ratios.
    larpp_prev: [i16; 8],
    /// The short-term synthesis filter's state.
    v: [i16; 9],
    /// The reconstructed residual's last 120 samples.
    drp: [i16; 160],
    /// The last lag used.
    nrp: i16,
    /// The de-emphasis filter's state.
    msr: i16,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new()
    }
}

const QLB: [i16; 4] = [3277, 11469, 21299, 32767];
const FAC: [i16; 8] = [18431, 20479, 22527, 24575, 26623, 28671, 30719, 32767];

impl Decoder {
    pub fn new() -> Decoder {
        Decoder {
            larpp_prev: [0; 8],
            v: [0; 9],
            drp: [0; 160],
            nrp: 40,
            msr: 0,
        }
    }

    /// Back to the start state (after a seek).
    pub fn reset(&mut self) {
        *self = Decoder::new();
    }

    /// Decodes one frame into 160 samples (16-bit, the low three bits zero).
    pub fn decode(&mut self, p: &Params, out: &mut [i16; FRAME_SAMPLES]) {
        let mut wt = [0i16; FRAME_SAMPLES];
        for s in 0..4 {
            let erp = rpe_decode(p.xmaxc[s], p.mc[s], &p.xmc[s]);
            self.long_term(p.bc[s], p.nc[s], &erp);
            wt[s * 40..s * 40 + 40].copy_from_slice(&self.drp[120..160]);
        }
        let larpp = decode_lar(&p.larc);
        self.short_term(&larpp, &wt, out);
        for o in out.iter_mut() {
            self.msr = add(*o, mult_r(self.msr, 28180));
            *o = ((i32::from(add(self.msr, self.msr))) & !7) as i16;
        }
    }

    /// The long-term (pitch) synthesis of one sub-frame: `drp[120..160]` becomes its
    /// residual, the 120 before it the history.
    fn long_term(&mut self, bcr: u8, ncr: u8, erp: &[i16; 40]) {
        let nr = if (40..=120).contains(&i16::from(ncr)) {
            i16::from(ncr)
        } else {
            self.nrp
        };
        self.nrp = nr;
        let brp = QLB[usize::from(bcr & 3)];
        // Shift the history down by 40 first: drp[0..120] is d'(k-120..k).
        self.drp.copy_within(40..160, 0);
        for k in 0..40 {
            let past = self.drp[120 + k - nr as usize];
            let drpp = mult_r(brp, past);
            self.drp[120 + k] = add(erp[k], drpp);
        }
    }

    fn short_term(&mut self, larpp: &[i16; 8], wt: &[i16; 160], out: &mut [i16; 160]) {
        let prev = self.larpp_prev;
        let segs: [Segment; 4] = [
            (0, 13, |j1, j| add(add(j1 >> 2, j >> 2), j1 >> 1)),
            (13, 27, |j1, j| add(j1 >> 1, j >> 1)),
            (27, 40, |j1, j| add(add(j1 >> 2, j >> 2), j >> 1)),
            (40, 160, |_, j| j),
        ];
        for (start, end, interp) in segs {
            let mut rp = [0i16; 8];
            for i in 0..8 {
                rp[i] = lar_to_rp(interp(prev[i], larpp[i]));
            }
            for k in start..end {
                let mut sri = wt[k];
                for i in (1..=8).rev() {
                    let t = mult_r(rp[i - 1], self.v[i - 1]);
                    sri = sub(sri, t);
                    self.v[i] = add(self.v[i - 1], mult_r(rp[i - 1], sri));
                }
                self.v[0] = sri;
                out[k] = sri;
            }
        }
        self.larpp_prev = *larpp;
    }
}

/// A run of samples and how its log-area ratios mix the previous frame's and this one's.
type Segment = (usize, usize, fn(i16, i16) -> i16);

/// The coded log-area ratios to their decoded values (§5.2.8).
fn decode_lar(larc: &[u8; 8]) -> [i16; 8] {
    const B: [i16; 8] = [0, 0, 2048, -2560, 94, -1792, -341, -1144];
    const MIC: [i16; 8] = [-32, -32, -16, -16, -8, -8, -4, -4];
    const INVA: [i16; 8] = [13107, 13107, 13107, 13107, 19223, 17476, 31454, 29708];
    let mut out = [0i16; 8];
    for i in 0..8 {
        // The transmitted value is offset by -MIC.
        let lar = add(i16::from(larc[i]), MIC[i]);
        let t = sub(((i32::from(lar)) << 10) as i16, B[i] << 1);
        let t = mult_r(INVA[i], t);
        out[i] = add(t, t);
    }
    out
}

/// An interpolated log-area ratio to its reflection coefficient (§5.2.9.2).
fn lar_to_rp(lar: i16) -> i16 {
    let a = abs(lar);
    let t = if a < 11059 {
        a << 1
    } else if a < 20070 {
        a + 11059
    } else {
        add(a >> 2, 26112)
    };
    if lar < 0 { -t } else { t }
}

/// The RPE sequence of a sub-frame: the APCM inverse quantisation of its 13 pulses placed on
/// grid `mc` (§5.2.15–§5.2.17).
fn rpe_decode(xmaxc: u8, mc: u8, xmc: &[u8; 13]) -> [i16; 40] {
    let xmaxc = i16::from(xmaxc);
    let mut exp: i16 = 0;
    if xmaxc > 15 {
        exp = (xmaxc >> 3) - 1;
    }
    let mut mant = xmaxc - (exp << 3);
    if mant == 0 {
        exp = -4;
        mant = 7;
    } else {
        while mant <= 7 {
            mant = (mant << 1) | 1;
            exp -= 1;
        }
        mant -= 8;
    }
    let fac = FAC[mant as usize];
    let shift = 6 - exp;
    // The largest step (exponent 6) has no rounding term.
    let round = if shift >= 1 { 1i16 << (shift - 1) } else { 0 };
    let mut ep = [0i16; 40];
    for (i, &x) in xmc.iter().enumerate() {
        let t = ((i16::from(x) << 1) - 7) << 12;
        let t = add(mult_r(fac, t), round);
        ep[usize::from(mc & 3) + 3 * i] = t >> shift;
    }
    ep
}

/// Decodes a plain `.gsm` stream: every whole frame with the signature, in order.
pub fn decode_plain(d: &[u8]) -> Vec<i16> {
    let mut dec = Decoder::new();
    let mut out = Vec::with_capacity(d.len() / FRAME_BYTES * FRAME_SAMPLES);
    let mut buf = [0i16; FRAME_SAMPLES];
    for f in d.chunks_exact(FRAME_BYTES) {
        match Params::plain(f) {
            Some(p) => dec.decode(&p, &mut buf),
            None => buf = [0; FRAME_SAMPLES],
        }
        out.extend_from_slice(&buf);
    }
    out
}

/// Decodes a stream of Microsoft 65-byte blocks.
pub fn decode_ms(d: &[u8]) -> Vec<i16> {
    let mut dec = Decoder::new();
    let mut out = Vec::with_capacity(d.len() / MS_BLOCK_BYTES * 2 * FRAME_SAMPLES);
    let mut buf = [0i16; FRAME_SAMPLES];
    for b in d.chunks_exact(MS_BLOCK_BYTES) {
        if let Some(ps) = Params::ms(b) {
            for p in &ps {
                dec.decode(p, &mut buf);
                out.extend_from_slice(&buf);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packings_agree() {
        // The same parameters packed both ways give the same frames.
        let mut p = Params::default();
        for (i, l) in p.larc.iter_mut().enumerate() {
            *l = (i as u8 * 7) & ((1 << LAR_BITS[i]) - 1);
        }
        for s in 0..4 {
            p.nc[s] = 40 + s as u8 * 13;
            p.bc[s] = s as u8;
            p.mc[s] = 3 - s as u8;
            p.xmaxc[s] = 20 + s as u8 * 9;
            for (i, x) in p.xmc[s].iter_mut().enumerate() {
                *x = ((i + s) % 8) as u8;
            }
        }
        let mut fields = Vec::new();
        for (i, &l) in p.larc.iter().enumerate() {
            fields.push((u32::from(l), LAR_BITS[i]));
        }
        for s in 0..4 {
            fields.push((u32::from(p.nc[s]), 7));
            fields.push((u32::from(p.bc[s]), 2));
            fields.push((u32::from(p.mc[s]), 2));
            fields.push((u32::from(p.xmaxc[s]), 6));
            for &x in &p.xmc[s] {
                fields.push((u32::from(x), 3));
            }
        }
        // Plain: MSB first after 0xD.
        let mut plain = vec![0u8; 33];
        plain[0] = 0xD0;
        let mut bit = 4;
        for &(v, w) in &fields {
            for k in (0..w).rev() {
                if v >> k & 1 != 0 {
                    plain[bit / 8] |= 0x80 >> (bit % 8);
                }
                bit += 1;
            }
        }
        assert_eq!(bit, 264);
        assert_eq!(Params::plain(&plain), Some(p));
        let mut ms = vec![0u8; 65];
        let mut bit = 0;
        for _ in 0..2 {
            for &(v, w) in &fields {
                for k in 0..w {
                    if v >> k & 1 != 0 {
                        ms[bit / 8] |= 1 << (bit % 8);
                    }
                    bit += 1;
                }
            }
        }
        assert_eq!(bit, 520);
        assert_eq!(Params::ms(&ms), Some([p, p]));
    }

    #[test]
    fn plain_stream_signature() {
        let mut d = vec![0u8; 33 * 3];
        assert!(!is_plain_stream(&d));
        for f in d.chunks_exact_mut(33) {
            f[0] = 0xD5;
        }
        assert!(is_plain_stream(&d));
        assert!(!is_plain_stream(&d[..33]));
    }
}

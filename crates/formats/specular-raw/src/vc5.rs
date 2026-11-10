//! VC-5 (SMPTE ST 2073) as GoPro GPR uses it in DNG compression 9: a Bayer image split into
//! four component arrays of half size (green sum, red and blue differences from green, green
//! difference), each coded as a three-level 2-D wavelet pyramid.
//!
//! The bitstream is a sequence of big-endian 32-bit segments: a 16-bit tag (negative =
//! optional) and a 16-bit value. Header tags give the image size, the channel and subband
//! counts, the lowpass precision and the per-level prescale; chunk tags carry payloads whose
//! size (in segments) is in the value (plus the low tag byte for large chunks). Section
//! headers only group what follows. A large codeblock chunk holds one subband of the current
//! channel: subband 0 is the lowpass band of the smallest wavelet as plain unsigned values of
//! the lowpass precision; subbands 1-9 are the three highpass bands (LH, HL, HH) of the
//! wavelets from the smallest to the largest, as runs from the codebook ([`specular_cineform::vc5_codebook`], shared with CineForm):
//! each code gives a run length and a magnitude, a non-zero magnitude is followed by a sign
//! bit, runs continue across rows and a run length of 0 ends the band.
//!
//! Highpass values are expanded by the cubic companding curve `m + 768 m^3 / 255^3` and
//! multiplied by the band's quantisation. Each wavelet level is inverted vertically and then
//! horizontally with the 2/6 filter pair (3-tap lowpass prediction with rounding, special taps
//! at the edges). The component arrays then give the Bayer samples through a 12-bit log curve
//! (`65535 ((113^(v/4095) - 1) / 112)`) scaled to the DNG's 12 or 14 bits.

use rayon::prelude::*;

/// How the decoded samples are laid out: bits per sample (12 or 14) and the Bayer order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Params {
    pub bits: u32,
    /// GBRG instead of RGGB.
    pub gbrg: bool,
}

/// Bits from big-endian bytes.
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    over: bool,
}

impl<'a> Bits<'a> {
    fn at(d: &'a [u8], pos: usize) -> Self {
        Bits {
            d,
            pos,
            acc: 0,
            n: 0,
            over: false,
        }
    }

    #[inline(always)]
    fn fill(&mut self) {
        while self.n <= 56 {
            let b = match self.d.get(self.pos) {
                Some(&b) => b,
                None => {
                    self.over = true;
                    0
                }
            };
            self.pos += 1;
            self.acc |= (b as u64) << (56 - self.n);
            self.n += 8;
        }
    }

    #[inline(always)]
    fn peek(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.fill();
        }
        (self.acc >> (64 - k)) as u32
    }

    #[inline(always)]
    fn skip(&mut self, k: u32) {
        self.acc <<= k;
        self.n -= k;
    }

    #[inline(always)]
    fn get(&mut self, k: u32) -> u32 {
        if k == 0 {
            return 0;
        }
        let v = self.peek(k);
        self.skip(k);
        v
    }
}

/// Codebook lookup: codes up to [`FAST`] bits through a table, longer ones by search.
struct Codebook {
    /// Per `FAST`-bit window: code length (0: longer code), run length, magnitude.
    fast: Vec<(u8, u16, u16)>,
    /// Longer codes: (length, code, run, magnitude), shortest first.
    long: Vec<(u8, u32, u16, u16)>,
}

const FAST: u32 = 12;

impl Codebook {
    fn new() -> Codebook {
        let mut fast = vec![(0u8, 0u16, 0u16); 1 << FAST];
        let mut long = Vec::new();
        for &(len, code, run, mag) in specular_cineform::vc5_codebook() {
            if (len as u32) <= FAST {
                let first = (code << (FAST - len as u32)) as usize;
                for e in &mut fast[first..first + (1 << (FAST - len as u32))] {
                    *e = (len, run, mag);
                }
            } else {
                long.push((len, code, run, mag));
            }
        }
        long.sort_by_key(|e| e.0);
        Codebook { fast, long }
    }

    /// The next (run length, magnitude); None for an unknown code.
    #[inline(always)]
    fn next(&self, br: &mut Bits) -> Option<(u16, u16)> {
        let (len, run, mag) = self.fast[br.peek(FAST) as usize];
        if len != 0 {
            br.skip(len as u32);
            return Some((run, mag));
        }
        for &(len, code, run, mag) in &self.long {
            if br.peek(len as u32) == code {
                br.skip(len as u32);
                return Some((run, mag));
            }
        }
        None
    }
}

/// One wavelet: four bands of `w x h` (LL, LH, HL, HH), their quantisation and which are set.
#[derive(Clone, Default)]
struct Wavelet {
    w: usize,
    h: usize,
    bands: [Vec<i16>; 4],
    quant: [i32; 4],
    valid: u8,
}

const LEVELS: usize = 3;
/// Wavelet and band of each subband.
const SUBBAND: [(usize, usize); 10] = [
    (2, 0),
    (2, 1),
    (2, 2),
    (2, 3),
    (1, 1),
    (1, 2),
    (1, 3),
    (0, 1),
    (0, 2),
    (0, 3),
];

const START: u32 = 0x5643_2D35;

/// Decodes the VC-5 bitstream `src` into `w x h` Bayer samples. Returns whether data ran
/// short, or why the stream cannot be decoded.
pub(crate) fn decode(
    src: &[u8],
    w: usize,
    h: usize,
    p: Params,
    out: &mut [u16],
    parallel: bool,
) -> Result<bool, &'static str> {
    let seg = |pos: usize| -> Option<u32> {
        Some(u32::from_be_bytes(src.get(pos..pos + 4)?.try_into().ok()?))
    };
    if seg(0) != Some(START) {
        return Err("VC-5: no start marker");
    }
    let book = Codebook::new();
    let (mut width, mut height) = (0usize, 0usize);
    let (mut channels, mut subbands) = (0usize, 10usize);
    let (mut channel, mut subband) = (0usize, 0usize);
    let (mut precision, mut quant) = (16u32, 1i32);
    let mut prescale = [0u32; LEVELS];
    let mut chans: Vec<[Wavelet; LEVELS]> = Vec::new();
    let mut short = false;
    let mut pos = 4;
    while let Some(s) = seg(pos) {
        pos += 4;
        let mut tag = (s >> 16) as u16 as i16 as i32;
        let value = (s & 0xffff) as i32;
        let optional = tag < 0;
        if optional {
            tag = -tag;
        }
        match tag {
            12 => channels = value as usize,
            14 => subbands = value as usize,
            20 => width = value as usize,
            21 => height = value as usize,
            35 => precision = value as u32,
            48 => subband = value as usize,
            53 => quant = value,
            62 => channel = value as usize,
            109 => {
                for (k, ps) in prescale.iter_mut().enumerate() {
                    *ps = (value as u32 >> (14 - 2 * k)) & 3;
                }
            }
            t if t & 0x6000 == 0x6000 => {
                // A codeblock: one subband of the current channel.
                let size = (value as usize + ((t as usize & 0xff) << 16)) * 4;
                let end = pos.saturating_add(size);
                if chans.is_empty() {
                    if channels == 0 || channels > 4 || width < 16 || height < 16 {
                        return Err("VC-5: header");
                    }
                    if width != w || height != h {
                        return Err("VC-5: image size differs from the DNG's");
                    }
                    chans = (0..channels)
                        .map(|_| wavelets(width / 2, height / 2))
                        .collect();
                }
                if channel >= chans.len() || subband >= SUBBAND.len().min(subbands) {
                    return Err("VC-5: subband out of range");
                }
                let (k, band) = SUBBAND[subband];
                let wv = &mut chans[channel][k];
                let mut br = Bits::at(src, pos);
                if band == 0 {
                    if !(1..=16).contains(&precision) {
                        return Err("VC-5: lowpass precision");
                    }
                    for v in wv.bands[0].iter_mut() {
                        *v = br.get(precision) as i16;
                    }
                } else {
                    runs(&mut br, &book, &mut wv.bands[band])?;
                    wv.quant[band] = quant;
                }
                short |= br.over || end > src.len();
                wv.valid |= 1 << band;
                if wv.valid == 15 && k > 0 {
                    let (lower, upper) = chans[channel].split_at_mut(k);
                    let up = &upper[0];
                    let low = &mut lower[k - 1];
                    low.bands[0] = inverse(up, low.w, low.h, prescale[k], parallel)?;
                    low.valid |= 1;
                }
                pos = end;
                subband += 1;
                if subband >= subbands {
                    channel += 1;
                    subband = 0;
                }
                if channel >= channels && chans.iter().all(|c| c[0].valid == 15) {
                    break;
                }
            }
            // Section headers group what follows; other chunks are skipped.
            t if matches!(
                t & 0xff00,
                0x2000 | 0x2100 | 0x2400 | 0x2500 | 0x2600 | 0x2700
            ) => {}
            t if t & 0x2000 != 0 => {}
            t if t & 0x4000 != 0 => pos = pos.saturating_add(value as usize * 4),
            _ => {}
        }
    }
    if chans.len() != 4 {
        return Err("VC-5: not a four-component Bayer image");
    }
    let (cw, ch) = (width / 2, height / 2);
    let mut comps = Vec::with_capacity(4);
    for c in &chans {
        if c[0].valid != 15 {
            short = true;
        }
        comps.push(inverse(&c[0], cw, ch, prescale[0], parallel)?);
    }
    unpack(&comps, cw, ch, p, out, parallel);
    Ok(short)
}

/// The wavelet pyramid of a `cw x ch` component array.
fn wavelets(cw: usize, ch: usize) -> [Wavelet; LEVELS] {
    let (mut w, mut h) = (cw, ch);
    std::array::from_fn(|_| {
        w = w.div_ceil(2);
        h = h.div_ceil(2);
        Wavelet {
            w,
            h,
            bands: std::array::from_fn(|_| vec![0i16; w * h]),
            quant: [1; 4],
            valid: 0,
        }
    })
}

/// Fills `band` (row-major) from the run codes up to the band end code.
fn runs(br: &mut Bits, book: &Codebook, band: &mut [i16]) -> Result<(), &'static str> {
    let mut i = 0;
    while i < band.len() {
        let (run, mag) = book.next(br).ok_or("VC-5: unknown code")?;
        if run == 0 {
            return Err("VC-5: band ended early");
        }
        let mut v = mag as i32;
        if v != 0 && br.get(1) == 1 {
            v = -v;
        }
        let n = (run as usize).min(band.len() - i);
        band[i..i + n].fill(v as i16);
        i += n;
        if br.over {
            break;
        }
    }
    Ok(())
}

/// A highpass value through the companding curve and the quantisation.
#[inline(always)]
fn dequant(v: i16, q: i32) -> i32 {
    let m = (v as i32).abs();
    let cubic = (m as f64 * m as f64 * m as f64 * 768.0 / (255.0 * 255.0 * 255.0)) as i32;
    let r = (m + cubic) * q;
    (if v < 0 { -r } else { r }).clamp(i16::MIN as i32, i16::MAX as i32)
}

#[inline(always)]
fn clamp16(v: i32) -> i32 {
    v.clamp(i16::MIN as i32, i16::MAX as i32)
}

/// One step of the inverse 2/6 filter: the even and odd outputs at position `i` of a line of
/// `n >= 3` lowpass values `l` and the highpass value `hi` (before the final scaling).
#[inline(always)]
fn pair(l: impl Fn(usize) -> i32, n: usize, i: usize, hi: i32) -> (i32, i32) {
    if i == 0 {
        let e = ((11 * l(0) - 4 * l(1) + l(2) + 4) >> 3) + hi;
        let o = ((5 * l(0) + 4 * l(1) - l(2) + 4) >> 3) - hi;
        (e, o)
    } else if i + 1 == n {
        let e = ((5 * l(i) + 4 * l(i - 1) - l(i - 2) + 4) >> 3) + hi;
        let o = ((11 * l(i) - 4 * l(i - 1) + l(i - 2) + 4) >> 3) - hi;
        (e, o)
    } else {
        let e = ((l(i - 1) - l(i + 1) + 4) >> 3) + l(i) + hi;
        let o = ((l(i + 1) - l(i - 1) + 4) >> 3) + l(i) - hi;
        (e, o)
    }
}

/// Inverts one wavelet into an `ow x oh` array (the lowpass band of the next larger wavelet,
/// or a component array).
fn inverse(
    wv: &Wavelet,
    ow: usize,
    oh: usize,
    prescale: u32,
    parallel: bool,
) -> Result<Vec<i16>, &'static str> {
    let (w, h) = (wv.w, wv.h);
    if w < 3 || h < 3 || ow > 2 * w || oh > 2 * h {
        return Err("VC-5: wavelet too small");
    }
    let deq = |b: usize| -> Vec<i32> {
        let q = wv.quant[b];
        wv.bands[b].iter().map(|&v| dequant(v, q)).collect()
    };
    let ll: Vec<i32> = wv.bands[0].iter().map(|&v| v as i32).collect();
    let (lh, hl, hh) = (deq(1), deq(2), deq(3));
    let mut out = vec![0i16; ow * oh];
    let shift = if prescale == 2 { 1 } else { 0 };
    let rows = |(r, dst): (usize, &mut [i16])| {
        // Vertical: the lowpass (from LL, HL) and highpass (from LH, HH) columns of output
        // rows 2r and 2r + 1.
        let mut lo = [vec![0i32; w], vec![0i32; w]];
        let mut hi = [vec![0i32; w], vec![0i32; w]];
        for c in 0..w {
            let (e, o) = pair(|y| ll[y * w + c], h, r, hl[r * w + c]);
            lo[0][c] = clamp16(e >> 1);
            lo[1][c] = clamp16(o >> 1);
            let (e, o) = pair(|y| lh[y * w + c], h, r, hh[r * w + c]);
            hi[0][c] = clamp16(e >> 1);
            hi[1][c] = clamp16(o >> 1);
        }
        // Horizontal.
        for (k, line) in dst.chunks_mut(ow).enumerate() {
            for c in 0..w {
                let (e, o) = pair(|x| lo[k][x], w, c, hi[k][c]);
                let fin = |v: i32| -> i16 {
                    if prescale > 1 {
                        clamp16(v << shift) as i16
                    } else {
                        (v >> 1).clamp(0, 16383) as i16
                    }
                };
                if 2 * c < ow {
                    line[2 * c] = fin(e);
                }
                if 2 * c + 1 < ow {
                    line[2 * c + 1] = fin(o);
                }
            }
        }
    };
    if parallel {
        out.par_chunks_mut(2 * ow).enumerate().for_each(rows);
    } else {
        out.chunks_mut(2 * ow).enumerate().for_each(rows);
    }
    Ok(out)
}

/// The decoder's log curve: 12-bit code to 16-bit linear.
fn log_curve() -> Vec<u16> {
    (0..4096)
        .map(|i| {
            let v = 65535.0 * (113f64.powf(i as f64 / 4095.0) - 1.0) / 112.0;
            ((v as f32) as i32).min(65535) as u16
        })
        .collect()
}

/// Component arrays (green sum, red - green, blue - green, green difference; differences
/// around 2048) to Bayer samples.
fn unpack(comps: &[Vec<i16>], cw: usize, ch: usize, p: Params, out: &mut [u16], parallel: bool) {
    let curve = log_curve();
    let shift = 16 - p.bits.clamp(8, 16);
    let w = 2 * cw;
    let rows = |(r, dst): (usize, &mut [u16])| {
        let (top, bottom) = dst.split_at_mut(w);
        for c in 0..cw {
            let i = r * cw + c;
            let gs = comps[0][i] as i32;
            let rg = comps[1][i] as i32 - 2048;
            let bg = comps[2][i] as i32 - 2048;
            let gd = comps[3][i] as i32 - 2048;
            let v = |x: i32| curve[x.clamp(0, 4095) as usize] >> shift;
            let (red, blue) = (v((rg << 1) + gs), v((bg << 1) + gs));
            let (g1, g2) = (v(gs + gd), v(gs - gd));
            if p.gbrg {
                top[2 * c] = g1;
                top[2 * c + 1] = blue;
                bottom[2 * c] = red;
                bottom[2 * c + 1] = g2;
            } else {
                top[2 * c] = red;
                top[2 * c + 1] = g1;
                bottom[2 * c] = g2;
                bottom[2 * c + 1] = blue;
            }
        }
    };
    let out = &mut out[..w * 2 * ch];
    if parallel {
        out.par_chunks_mut(2 * w).enumerate().for_each(rows);
    } else {
        out.chunks_mut(2 * w).enumerate().for_each(rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codebook_is_prefix_free_and_complete() {
        let book = Codebook::new();
        // Every 12-bit window maps to a code or needs a longer one.
        assert!(book.fast.iter().any(|e| e.0 == 1));
        assert!(!book.long.is_empty());
        // 264 codes, the band end code (run 0) among them.
        let codes = specular_cineform::vc5_codebook();
        assert_eq!(codes.len(), 264);
        assert!(codes.iter().any(|e| e.2 == 0));
        // Kraft sum is at most 1.
        let k: f64 = codes.iter().map(|e| 0.5f64.powi(e.0 as i32)).sum();
        assert!(k <= 1.0 + 1e-12, "{k}");
    }

    #[test]
    fn companding_and_curve() {
        assert_eq!(dequant(0, 7), 0);
        assert_eq!(dequant(1, 3), 3);
        assert_eq!(dequant(-255, 1), -(255 + 768));
        let c = log_curve();
        assert_eq!((c[0], c[4095]), (0, 65535));
        assert!(c.windows(2).all(|p| p[0] <= p[1]));
    }

    #[test]
    fn flat_wavelet_inverts_to_flat() {
        let (w, h) = (4, 4);
        let wv = Wavelet {
            w,
            h,
            bands: [
                vec![1000; w * h],
                vec![0; w * h],
                vec![0; w * h],
                vec![0; w * h],
            ],
            quant: [1; 4],
            valid: 15,
        };
        let out = inverse(&wv, 8, 7, 0, false).unwrap();
        // The 2/6 lowpass preserves a constant (halved twice by the final scaling).
        assert!(out.iter().all(|&v| v == 250), "{out:?}");
        assert!(inverse(&Wavelet { w: 2, h: 2, ..wv }, 4, 4, 0, true).is_err());
    }

    #[test]
    fn hostile_streams() {
        let p = Params {
            bits: 14,
            gbrg: false,
        };
        let mut out = vec![0u16; 64 * 64];
        assert!(decode(&[], 64, 64, p, &mut out, false).is_err());
        let mut s = START.to_be_bytes().to_vec();
        for (t, v) in [(12u16, 4u16), (20, 64), (21, 64), (14, 10), (0x6000, 5)] {
            s.extend(t.to_be_bytes());
            s.extend(v.to_be_bytes());
        }
        s.extend([0xffu8; 20]);
        let _ = decode(&s, 64, 64, p, &mut out, true);
        let _ = decode(&s[..s.len() - 7], 64, 64, p, &mut out, false);
    }
}

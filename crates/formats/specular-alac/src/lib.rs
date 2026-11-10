//! Specular's own Apple Lossless (ALAC) decoder, written from the format Apple published
//! with its open-source encoder and decoder (Apache 2.0, 2011): the `ALACSpecificConfig`, the
//! frame's channel elements, adaptive Golomb-Rice residuals with zero runs, the adaptive FIR
//! predictor and the stereo un-mixing. No code or tables are copied; ALAC has no tables.
//!
//! - 16, 20, 24 and 32 bits, 1–8 channels (Apple's layouts, reordered to the WAVE order:
//!   L R C LFE back… side…), shifted-out low bytes, uncompressed (escaped) frames and part
//!   frames.
//! - Output is interleaved `f32` in −1..1. A damaged frame is an error; the caller puts
//!   silence of a frame's length in its place.

#![deny(unsafe_code)]

/// Why a stream or frame can't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unsupported(String),
    Malformed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported(s) => write!(f, "unsupported ALAC: {s}"),
            Error::Malformed(s) => write!(f, "damaged ALAC: {s}"),
        }
    }
}

impl std::error::Error for Error {}

fn bad(s: &str) -> Error {
    Error::Malformed(s.into())
}

/// The longest frame accepted, in samples a channel (Apple's encoder writes 4096; the
/// format allows any 32-bit length, which a damaged file must not turn into gigabytes).
pub const MAX_FRAME: u32 = 1 << 16;

/// The decoder's set-up (`ALACSpecificConfig`, 24 bytes, big-endian).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Samples a channel in a full frame.
    pub frame_length: u32,
    pub bit_depth: u8,
    /// Rice history multiplier, initial history and Rice parameter limit.
    pub pb: u8,
    pub mb: u8,
    pub kb: u8,
    pub channels: u8,
    pub max_run: u16,
    pub sample_rate: u32,
}

impl Config {
    /// Parses the 24-byte config, or the same preceded by an `alac` box header (36 bytes:
    /// size, `alac`, version and flags) or by version and flags alone (28 bytes).
    pub fn parse(b: &[u8]) -> Result<Config, Error> {
        let b = match b.len() {
            n if n >= 36 && &b[4..8] == b"alac" => &b[12..36],
            n if n >= 28 && b[..4] == [0; 4] && n < 36 => &b[4..28],
            n if n >= 24 => &b[..24],
            _ => return Err(bad("a config shorter than 24 bytes")),
        };
        let u32_at = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let c = Config {
            frame_length: u32_at(0),
            bit_depth: b[5],
            pb: b[6],
            mb: b[7],
            kb: b[8],
            channels: b[9],
            max_run: u16::from_be_bytes([b[10], b[11]]),
            sample_rate: u32_at(20),
        };
        if b[4] != 0 {
            return Err(Error::Unsupported(format!("version {}", b[4])));
        }
        if !matches!(c.bit_depth, 16 | 20 | 24 | 32) {
            return Err(Error::Unsupported(format!("{}-bit samples", c.bit_depth)));
        }
        if !(1..=8).contains(&c.channels) {
            return Err(Error::Unsupported(format!("{} channels", c.channels)));
        }
        if c.frame_length == 0 || c.frame_length > MAX_FRAME {
            return Err(Error::Unsupported(format!(
                "{}-sample frames",
                c.frame_length
            )));
        }
        if c.kb == 0 || c.kb > 24 {
            return Err(bad("Rice limit out of range"));
        }
        Ok(c)
    }
}

/// Big-endian bit reader; reads past the end give zeros and set `overrun`.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    overrun: bool,
}

impl<'a> Bits<'a> {
    /// 32 bits from bit `pos` on, zero-padded.
    #[inline]
    fn peek32(&self) -> u32 {
        let byte = self.pos >> 3;
        let mut w = [0u8; 8];
        if let Some(s) = self.data.get(byte..byte + 8) {
            w.copy_from_slice(s);
        } else {
            for (i, v) in w.iter_mut().enumerate() {
                *v = self.data.get(byte + i).copied().unwrap_or(0);
            }
        }
        (u64::from_be_bytes(w) << (self.pos & 7) >> 32) as u32
    }

    #[inline]
    fn skip(&mut self, n: usize) {
        self.pos += n;
        if self.pos > self.data.len() * 8 {
            self.overrun = true;
        }
    }

    /// `n` ≤ 32 bits, unsigned.
    #[inline]
    fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek32() >> (32 - n);
        self.skip(n as usize);
        v
    }

    /// `n` ≤ 32 bits, two's complement.
    fn read_signed(&mut self, n: u32) -> i32 {
        if n == 0 {
            return 0;
        }
        ((self.read(n) << (32 - n)) as i32) >> (32 - n)
    }

    fn byte_align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }
}

/// A Golomb-Rice value with Rice parameter `k`: a unary prefix of up to 8 ones (9 ones
/// escape to a plain `escape`-bit value), then `k` bits whose value `v` adds `v − 1` when
/// at least 2, else only `k − 1` of them are used. `m` is `2^k − 1`.
#[inline]
fn rice(b: &mut Bits, k: u32, m: u32, escape: u32) -> u32 {
    let w = b.peek32();
    let ones = (!w).leading_zeros();
    if ones >= 9 {
        b.skip(9);
        return b.read(escape);
    }
    b.skip(ones as usize + 1);
    if k <= 1 {
        return ones;
    }
    let v = b.peek32() >> (32 - k);
    let mut x = ones * m;
    if v >= 2 {
        x += v - 1;
        b.skip(k as usize);
    } else {
        b.skip(k as usize - 1);
    }
    x
}

/// The adaptive Golomb residuals of one channel (`out.len()` samples of `bits` bits).
fn residuals(b: &mut Bits, out: &mut [i32], c: &Config, mult: u32, bits: u32) -> Result<(), Error> {
    const QB: u32 = 1 << 9;
    let n = out.len();
    let kb = u32::from(c.kb);
    let wb = (1u32 << kb) - 1;
    let mut mean = u32::from(c.mb);
    let mut zmode = 0u32;
    let mut i = 0;
    while i < n {
        let k = (31 - ((mean >> 9) + 3).leading_zeros()).min(kb);
        let v = rice(b, k, (1 << k) - 1, bits);
        let x = v.wrapping_add(zmode);
        // The low bit is the sign.
        out[i] = ((x >> 1) as i32) ^ -((x & 1) as i32);
        i += 1;
        mean = if v > 0xFFFF {
            0xFFFF
        } else {
            (mult.wrapping_mul(x))
                .wrapping_add(mean)
                .wrapping_sub(mult.wrapping_mul(mean) >> 9)
        };
        zmode = 0;
        // A quiet stretch: a run of zeros follows, its length coded with a parameter from
        // how quiet it was.
        if mean << 2 < QB && i < n {
            let k = (mean.leading_zeros() as i32 - 24 + ((mean as i32 + 16) >> 6)).max(0) as u32;
            let k = k.min(31);
            let m = ((1u32 << k) - 1) & wb;
            let run = rice(b, k, m, 16) as usize;
            if i + run > n {
                return Err(bad("a zero run past the frame"));
            }
            out[i..i + run].fill(0);
            i += run;
            zmode = u32::from(run < 0xFFFF);
            mean = 0;
        }
        if b.overrun {
            return Err(bad("residuals past the frame"));
        }
    }
    Ok(())
}

#[inline]
fn sign_extend(v: i32, bits: u32) -> i32 {
    let s = 32 - bits;
    v.wrapping_shl(s).wrapping_shr(s)
}

/// The adaptive FIR predictor in place over `buf` (residuals in, samples out): `coefs`
/// (oldest-first order is the stream's reverse), quantised by `shift`, adapting a step at a
/// time towards the residual's sign. Order 31 is plain first-order prediction.
fn predict(buf: &mut [i32], coefs: &mut [i32], shift: u32, bits: u32) {
    let n = buf.len();
    let order = coefs.len();
    if n <= 1 || order == 0 {
        return;
    }
    if order == 31 {
        for i in 1..n {
            buf[i] = sign_extend(buf[i - 1].wrapping_add(buf[i]), bits);
        }
        return;
    }
    let warm = order.min(n - 1);
    for i in 1..=warm {
        buf[i] = sign_extend(buf[i - 1].wrapping_add(buf[i]), bits);
    }
    // The reference arithmetic is 32-bit and wraps.
    let shift = shift.min(31);
    let round = if shift > 0 { 1i32 << (shift - 1) } else { 0 };
    for i in order + 1..n {
        // `top` is the sample just before the window buf[i-order..i] (oldest first, as
        // `coefs` is).
        let top = buf[i - order - 1];
        let mut sum = 0i32;
        for j in 0..order {
            sum = sum.wrapping_add(buf[i - order + j].wrapping_sub(top).wrapping_mul(coefs[j]));
        }
        let pred = sum.wrapping_add(round) >> shift;
        let err = buf[i];
        buf[i] = sign_extend(pred.wrapping_add(top).wrapping_add(err), bits);
        if err != 0 {
            let sign = err.signum();
            let mut e = err;
            for j in 0..order {
                if e.wrapping_mul(sign) <= 0 {
                    break;
                }
                let d = top.wrapping_sub(buf[i - order + j]);
                let s = d.signum() * sign;
                coefs[j] -= s;
                e = e.wrapping_sub((d.wrapping_mul(s) >> shift).wrapping_mul(j as i32 + 1));
            }
        }
    }
}

/// An ALAC decoder for one stream.
pub struct Decoder {
    config: Config,
    /// Per channel: residuals and then samples.
    buf: Vec<Vec<i32>>,
    /// The shifted-out low bits, per channel.
    low: Vec<Vec<u32>>,
}

/// Apple's element order for each channel count, as positions in the WAVE order.
fn wave_position(channels: usize) -> &'static [usize] {
    match channels {
        1 => &[0],
        2 => &[0, 1],
        // C, L R
        3 => &[2, 0, 1],
        // C, L R, Cs
        4 => &[2, 0, 1, 3],
        // C, L R, Ls Rs
        5 => &[2, 0, 1, 3, 4],
        // C, L R, Ls Rs, LFE
        6 => &[2, 0, 1, 4, 5, 3],
        // C, L R, Ls Rs, Cs, LFE
        7 => &[2, 0, 1, 5, 6, 4, 3],
        // C, Lc Rc, L R, Ls Rs, LFE
        _ => &[2, 6, 7, 0, 1, 4, 5, 3],
    }
}

impl Decoder {
    pub fn new(config: Config) -> Decoder {
        let n = config.frame_length as usize;
        let ch = usize::from(config.channels);
        Decoder {
            config,
            buf: vec![vec![0; n]; ch],
            low: vec![vec![0; n]; ch],
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Bytes the decoder's buffers take (for the caller's memory budget).
    pub fn buffer_bytes(config: &Config) -> u64 {
        u64::from(config.frame_length) * u64::from(config.channels) * 8
    }

    /// Decodes one frame (one MP4 or Matroska sample), appending its samples interleaved in
    /// the WAVE channel order.
    pub fn decode(&mut self, frame: &[u8], out: &mut Vec<f32>) -> Result<(), Error> {
        let c = self.config;
        let channels = usize::from(c.channels);
        let mut b = Bits {
            data: frame,
            pos: 0,
            overrun: false,
        };
        let mut ch = 0usize;
        let mut frame_samples: Option<usize> = None;
        loop {
            if b.overrun {
                return Err(bad("the frame ends early"));
            }
            match b.read(3) {
                // Single channel, channel pair, LFE.
                tag @ (0 | 1 | 3) => {
                    let pair = tag == 1;
                    let count = if pair { 2 } else { 1 };
                    if ch + count > channels {
                        return Err(bad("more channels than the config says"));
                    }
                    let n = self.element(&mut b, ch, pair)?;
                    if frame_samples.is_some_and(|f| f != n) {
                        return Err(bad("elements of different lengths"));
                    }
                    frame_samples = Some(n);
                    ch += count;
                }
                // Coupling channel or programme config: not in ALAC streams.
                2 | 5 => return Err(Error::Unsupported("coupling or PCE elements".into())),
                // Data stream element: skipped.
                4 => {
                    b.read(4);
                    let align = b.read(1) != 0;
                    let mut count = b.read(8) as usize;
                    if count == 255 {
                        count += b.read(8) as usize;
                    }
                    if align {
                        b.byte_align();
                    }
                    b.skip(count * 8);
                }
                // Fill element: skipped.
                6 => {
                    let mut count = b.read(4) as usize;
                    if count == 15 {
                        count += b.read(8) as usize;
                        count = count.saturating_sub(1);
                    }
                    b.skip(count * 8);
                }
                _ => break,
            }
        }
        let Some(n) = frame_samples else {
            return Err(bad("a frame without channels"));
        };
        if ch != channels {
            return Err(bad("fewer channels than the config says"));
        }
        let scale = 1.0 / (1u64 << (c.bit_depth - 1)) as f32;
        let pos = wave_position(channels);
        let at = out.len();
        out.resize(at + n * channels, 0.0);
        for (s, &p) in pos.iter().enumerate().take(channels) {
            for i in 0..n {
                out[at + i * channels + p] = self.buf[s][i] as f32 * scale;
            }
        }
        Ok(())
    }

    /// One single-channel or pair element into channels `ch` (and `ch + 1`); returns its
    /// samples a channel.
    fn element(&mut self, b: &mut Bits, ch: usize, pair: bool) -> Result<usize, Error> {
        let c = self.config;
        let count = if pair { 2 } else { 1 };
        b.read(4); // element instance tag
        if b.read(12) != 0 {
            return Err(bad("reserved header bits set"));
        }
        let partial = b.read(1) != 0;
        let shift = b.read(2) * 8;
        let escaped = b.read(1) != 0;
        let n = if partial {
            b.read(32) as usize
        } else {
            c.frame_length as usize
        };
        if n == 0 || n > c.frame_length as usize {
            return Err(bad("a frame longer than the config says"));
        }
        let depth = u32::from(c.bit_depth);
        if escaped {
            // Uncompressed: the samples interleaved at full depth.
            for i in 0..n {
                for k in 0..count {
                    self.buf[ch + k][i] = b.read_signed(depth);
                }
            }
            if b.overrun {
                return Err(bad("samples past the frame"));
            }
            return Ok(n);
        }
        if shift >= depth {
            return Err(bad("all bits shifted out"));
        }
        // A pair's difference channel needs one bit more.
        let bits = depth - shift + u32::from(pair);
        let mix_shift = b.read(8);
        let mix_weight = b.read(8) as u8 as i8 as i32;
        struct Pred {
            mode: u32,
            quant: u32,
            mult: u32,
            coefs: [i32; 32],
            order: usize,
        }
        let mut preds = [(); 2].map(|_| Pred {
            mode: 0,
            quant: 0,
            mult: 0,
            coefs: [0; 32],
            order: 0,
        });
        for p in preds.iter_mut().take(count) {
            p.mode = b.read(4);
            p.quant = b.read(4);
            p.mult = b.read(3);
            p.order = b.read(5) as usize;
            // The stream lists the newest sample's coefficient first.
            for j in (0..p.order).rev() {
                p.coefs[j] = b.read_signed(16);
            }
            if p.mode != 0 && p.mode != 15 {
                return Err(Error::Unsupported(format!("prediction mode {}", p.mode)));
            }
        }
        if shift > 0 {
            for i in 0..n {
                for k in 0..count {
                    self.low[ch + k][i] = b.read(shift);
                }
            }
        }
        for (k, p) in preds.iter_mut().take(count).enumerate() {
            let buf = &mut self.buf[ch + k][..n];
            let mult = u32::from(c.pb) * p.mult / 4;
            residuals(b, buf, &c, mult, bits)?;
            if p.mode == 15 {
                let mut none = [0i32; 31];
                predict(buf, &mut none, 0, bits);
            }
            predict(buf, &mut p.coefs[..p.order], p.quant, bits);
        }
        if pair && mix_weight != 0 {
            if mix_shift > 31 {
                return Err(bad("stereo weight shift out of range"));
            }
            let (l, r) = self.buf.split_at_mut(ch + 1);
            for i in 0..n {
                let u = l[ch][i];
                let v = r[0][i];
                let right = u.wrapping_sub(v.wrapping_mul(mix_weight) >> mix_shift);
                l[ch][i] = right.wrapping_add(v);
                r[0][i] = right;
            }
        }
        if shift > 0 {
            for k in 0..count {
                for i in 0..n {
                    let v = self.buf[ch + k][i];
                    self.buf[ch + k][i] = v.wrapping_shl(shift) | self.low[ch + k][i] as i32;
                }
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(bits: u8, ch: u8) -> Vec<u8> {
        let mut c = Vec::new();
        c.extend_from_slice(&4096u32.to_be_bytes());
        c.extend_from_slice(&[0, bits, 40, 10, 14, ch]);
        c.extend_from_slice(&255u16.to_be_bytes());
        c.extend_from_slice(&0u32.to_be_bytes());
        c.extend_from_slice(&0u32.to_be_bytes());
        c.extend_from_slice(&44_100u32.to_be_bytes());
        c
    }

    #[test]
    fn config_forms() {
        let c = config(16, 2);
        let a = Config::parse(&c).unwrap();
        assert_eq!(
            (a.frame_length, a.bit_depth, a.channels, a.sample_rate),
            (4096, 16, 2, 44_100)
        );
        let mut boxed = 36u32.to_be_bytes().to_vec();
        boxed.extend_from_slice(b"alac");
        boxed.extend_from_slice(&[0; 4]);
        boxed.extend_from_slice(&c);
        assert_eq!(Config::parse(&boxed).unwrap(), a);
        let mut full = vec![0; 4];
        full.extend_from_slice(&c);
        assert_eq!(Config::parse(&full).unwrap(), a);
        assert!(Config::parse(&config(12, 2)).is_err());
        assert!(Config::parse(&config(16, 9)).is_err());
    }

    #[test]
    fn escaped_frame_reads_plain_samples() {
        let c = Config::parse(&config(16, 1)).unwrap();
        let mut d = Decoder::new(Config {
            frame_length: 4,
            ..c
        });
        // SCE, tag 0, 12 zero bits, partial 0, shift 0, escaped 1, four samples, END.
        let mut bits: Vec<bool> = Vec::new();
        let mut put = |v: u32, n: u32| {
            for i in (0..n).rev() {
                bits.push(v >> i & 1 != 0);
            }
        };
        put(0, 3);
        put(0, 4);
        put(0, 12);
        put(0, 1);
        put(0, 2);
        put(1, 1);
        for s in [1000i16, -1000, 32767, -32768] {
            put(u32::from(s as u16), 16);
        }
        put(7, 3);
        let bytes: Vec<u8> = bits
            .chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |a, (i, &b)| a | (u8::from(b) << (7 - i)))
            })
            .collect();
        let mut out = Vec::new();
        d.decode(&bytes, &mut out).unwrap();
        let want: Vec<f32> = [1000.0, -1000.0, 32767.0, -32768.0]
            .iter()
            .map(|v| v / 32768.0)
            .collect();
        assert_eq!(out, want);
    }

    #[test]
    fn junk_never_panics() {
        let mut seed = 5u32;
        let junk: Vec<u8> = (0..40_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        for ch in [1u8, 2, 6] {
            let mut d = Decoder::new(Config::parse(&config(24, ch)).unwrap());
            let mut out = Vec::new();
            for f in junk.chunks(997) {
                let mut f = f.to_vec();
                // Plausible element headers so the residual and predictor paths run.
                f[0] = 0x20;
                f[1] = 0;
                f[2] = 0;
                let _ = d.decode(&f, &mut out);
            }
            assert!(out.iter().all(|v| v.is_finite()));
        }
    }
}

//! Frame header, checksums and subframes (RFC 9639 section 9).

use crate::Error;
use crate::bits::Bits;

fn bad(what: &str) -> Error {
    Error::Malformed(what.to_string())
}

const fn crc8_table() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u8;
        let mut b = 0;
        while b < 8 {
            c = if c & 0x80 != 0 {
                (c << 1) ^ 0x07
            } else {
                c << 1
            };
            b += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

const fn crc16_table() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = (i as u16) << 8;
        let mut b = 0;
        while b < 8 {
            c = if c & 0x8000 != 0 {
                (c << 1) ^ 0x8005
            } else {
                c << 1
            };
            b += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

static CRC8: [u8; 256] = crc8_table();
static CRC16: [u16; 256] = crc16_table();

/// CRC-8 (polynomial x^8 + x^2 + x + 1, start 0) of the frame header.
pub(crate) fn crc8(d: &[u8]) -> u8 {
    d.iter().fold(0, |c, &b| CRC8[usize::from(c ^ b)])
}

/// CRC-16 step (polynomial x^16 + x^15 + x^2 + 1).
#[inline]
pub(crate) fn crc16_byte(c: u16, b: u8) -> u16 {
    (c << 8) ^ CRC16[usize::from((c >> 8) as u8 ^ b)]
}

/// CRC-16 (start 0) of the whole frame up to its footer.
pub(crate) fn crc16(d: &[u8]) -> u16 {
    d.iter().fold(0, |c, &b| crc16_byte(c, b))
}

/// How the channels of a frame are coded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Assign {
    Independent,
    LeftSide,
    SideRight,
    MidSide,
}

/// A parsed frame header. `rate` and `bits` are 0 when the frame defers to STREAMINFO.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Header {
    pub block: usize,
    pub rate: u32,
    pub channels: usize,
    pub assign: Assign,
    pub bits: u8,
    /// Header length in bytes, CRC-8 included.
    pub len: usize,
}

/// Whether `d` starts with the 15-bit frame sync code.
pub(crate) fn is_sync(d: &[u8]) -> bool {
    d.len() >= 2 && d[0] == 0xFF && d[1] & 0xFE == 0xF8
}

pub(crate) fn parse_header(d: &[u8]) -> Result<Header, Error> {
    let short = || bad("frame header cut short");
    if !is_sync(d) {
        return Err(bad("no frame sync code"));
    }
    let byte = |i: usize| d.get(i).copied().ok_or_else(short);
    let (b2, b3) = (byte(2)?, byte(3)?);
    if b3 & 1 != 0 {
        return Err(bad("reserved frame header bit set"));
    }
    // Coded frame or sample number, UTF-8 style, up to 7 bytes.
    let b4 = byte(4)?;
    let extra = match b4.leading_ones() {
        0 => 0,
        n @ 2..=7 => n as usize - 1,
        _ => return Err(bad("invalid coded frame number")),
    };
    for i in 0..extra {
        if byte(5 + i)? & 0xC0 != 0x80 {
            return Err(bad("invalid coded frame number"));
        }
    }
    let mut p = 5 + extra;
    let block = match b2 >> 4 {
        0 => return Err(bad("reserved block size")),
        1 => 192,
        n @ 2..=5 => 576 << (n - 2),
        6 => {
            p += 1;
            usize::from(byte(p - 1)?) + 1
        }
        7 => {
            p += 2;
            let v = usize::from(u16::from_be_bytes([byte(p - 2)?, byte(p - 1)?])) + 1;
            if v == 65536 {
                return Err(bad("block size 65536"));
            }
            v
        }
        n => 256 << (n - 8),
    };
    let rate = match b2 & 15 {
        0 => 0,
        1 => 88200,
        2 => 176400,
        3 => 192000,
        4 => 8000,
        5 => 16000,
        6 => 22050,
        7 => 24000,
        8 => 32000,
        9 => 44100,
        10 => 48000,
        11 => 96000,
        12 => {
            p += 1;
            u32::from(byte(p - 1)?) * 1000
        }
        13 | 14 => {
            p += 2;
            let v = u32::from(u16::from_be_bytes([byte(p - 2)?, byte(p - 1)?]));
            if b2 & 15 == 14 { v * 10 } else { v }
        }
        _ => return Err(bad("invalid sample rate code")),
    };
    let (channels, assign) = match b3 >> 4 {
        n @ 0..=7 => (usize::from(n) + 1, Assign::Independent),
        8 => (2, Assign::LeftSide),
        9 => (2, Assign::SideRight),
        10 => (2, Assign::MidSide),
        _ => return Err(bad("reserved channel assignment")),
    };
    let bits = match (b3 >> 1) & 7 {
        0 => 0,
        1 => 8,
        2 => 12,
        3 => return Err(bad("reserved bit depth code")),
        4 => 16,
        5 => 20,
        6 => 24,
        _ => 32,
    };
    let crc = byte(p)?;
    if crc8(&d[..p]) != crc {
        return Err(bad("frame header checksum mismatch"));
    }
    Ok(Header {
        block,
        rate,
        channels,
        assign,
        bits,
        len: p + 1,
    })
}

/// Sample storage of a subframe: `i32` for up to 32 bits, `i64` for the 33-bit side channel
/// of 32-bit stereo.
pub(crate) trait Sample: Copy + Default {
    fn to_i64(self) -> i64;
    /// Keeps the low bits (wraps); exact for valid streams.
    fn from_i64(v: i64) -> Self;
    fn from_i32(v: i32) -> Self;
    /// Linear prediction over the residual in `buf` for a subframe of `bits` bits.
    fn predict(order: usize, coefs: &[i64], shift: u32, bits: u32, buf: &mut [Self]);
}

impl Sample for i32 {
    #[inline(always)]
    fn to_i64(self) -> i64 {
        i64::from(self)
    }
    #[inline(always)]
    fn from_i64(v: i64) -> Self {
        v as i32
    }
    #[inline(always)]
    fn from_i32(v: i32) -> Self {
        v
    }
    fn predict(order: usize, coefs: &[i64], shift: u32, bits: u32, buf: &mut [Self]) {
        lpc_i32(order, coefs, shift, buf, fits_i32(bits, coefs));
    }
}

impl Sample for i64 {
    #[inline(always)]
    fn to_i64(self) -> i64 {
        self
    }
    #[inline(always)]
    fn from_i64(v: i64) -> Self {
        v
    }
    #[inline(always)]
    fn from_i32(v: i32) -> Self {
        i64::from(v)
    }
    fn predict(order: usize, coefs: &[i64], shift: u32, _: u32, buf: &mut [Self]) {
        lpc(order, coefs, shift, buf);
    }
}

/// Decodes one subframe of `bits` bits per sample (side channel included) into `buf`, whose
/// length is the block size.
pub(crate) fn subframe<S: Sample>(br: &mut Bits, bits: u32, buf: &mut [S]) -> Result<(), Error> {
    let h = br.read(8);
    if h & 0x80 != 0 {
        return Err(bad("subframe padding bit set"));
    }
    let kind = (h >> 1) & 63;
    let wasted = if h & 1 != 0 {
        br.unary().ok_or_else(|| bad("bad wasted bits count"))? + 1
    } else {
        0
    };
    if wasted >= bits {
        return Err(bad("wasted bits leave no bits"));
    }
    let bits = bits - wasted;
    let n = buf.len();
    match kind {
        0 => {
            let v = S::from_i64(br.read_signed(bits));
            buf.fill(v);
        }
        1 => {
            for s in buf.iter_mut() {
                *s = S::from_i64(br.read_signed(bits));
            }
        }
        8..=12 => {
            let order = (kind - 8) as usize;
            if order > n {
                return Err(bad("predictor order above block size"));
            }
            for s in &mut buf[..order] {
                *s = S::from_i64(br.read_signed(bits));
            }
            residual(br, order, buf)?;
            fixed(order, buf);
        }
        32..=63 => {
            let order = (kind - 31) as usize;
            if order > n {
                return Err(bad("predictor order above block size"));
            }
            for s in &mut buf[..order] {
                *s = S::from_i64(br.read_signed(bits));
            }
            let precision = br.read(4) as u32;
            if precision == 15 {
                return Err(bad("invalid predictor precision"));
            }
            let shift = br.read_signed(5);
            if shift < 0 {
                return Err(bad("negative predictor shift"));
            }
            let mut coefs = [0i64; 32];
            for c in &mut coefs[..order] {
                *c = br.read_signed(precision + 1);
            }
            residual(br, order, buf)?;
            S::predict(order, &coefs[..order], shift as u32, bits, buf);
        }
        _ => return Err(bad("reserved subframe type")),
    }
    if br.overrun() {
        return Err(bad("frame cut short"));
    }
    if wasted > 0 {
        for s in buf.iter_mut() {
            *s = S::from_i64(s.to_i64() << wasted);
        }
    }
    Ok(())
}

/// Reads the coded residual into `buf[order..]`.
fn residual<S: Sample>(br: &mut Bits, order: usize, buf: &mut [S]) -> Result<(), Error> {
    let pbits = match br.read(2) {
        0 => 4,
        1 => 5,
        _ => return Err(bad("reserved residual coding method")),
    };
    let escape = (1u32 << pbits) - 1;
    let porder = br.read(4) as u32;
    let n = buf.len();
    let psize = n >> porder;
    if psize << porder != n || psize < order {
        return Err(bad("partition order does not fit the block"));
    }
    let mut start = order;
    for p in 0..1usize << porder {
        let end = (p + 1) * psize;
        let part = &mut buf[start..end];
        let k = br.read(pbits) as u32;
        if k == escape {
            let nb = br.read(5) as u32;
            for s in part.iter_mut() {
                *s = S::from_i64(br.read_signed(nb));
            }
        } else {
            for s in part.iter_mut() {
                *s = S::from_i32(br.rice(k).ok_or_else(|| bad("bad Rice code"))?);
            }
        }
        if br.overrun() {
            return Err(bad("frame cut short"));
        }
        start = end;
    }
    Ok(())
}

/// Fixed predictors of order 0 to 4, in place over the residual. Wrapping arithmetic is exact:
/// the result fits the storage type and every step is exact modulo its width.
fn fixed<S: Sample>(order: usize, buf: &mut [S]) {
    let n = buf.len();
    match order {
        1 => {
            for i in 1..n {
                buf[i] = S::from_i64(buf[i].to_i64().wrapping_add(buf[i - 1].to_i64()));
            }
        }
        2 => {
            for i in 2..n {
                let p = buf[i - 1]
                    .to_i64()
                    .wrapping_mul(2)
                    .wrapping_sub(buf[i - 2].to_i64());
                buf[i] = S::from_i64(buf[i].to_i64().wrapping_add(p));
            }
        }
        3 => {
            for i in 3..n {
                let p = buf[i - 1]
                    .to_i64()
                    .wrapping_sub(buf[i - 2].to_i64())
                    .wrapping_mul(3)
                    .wrapping_add(buf[i - 3].to_i64());
                buf[i] = S::from_i64(buf[i].to_i64().wrapping_add(p));
            }
        }
        4 => {
            for i in 4..n {
                let p = buf[i - 1]
                    .to_i64()
                    .wrapping_add(buf[i - 3].to_i64())
                    .wrapping_mul(4)
                    .wrapping_sub(buf[i - 2].to_i64().wrapping_mul(6))
                    .wrapping_sub(buf[i - 4].to_i64());
                buf[i] = S::from_i64(buf[i].to_i64().wrapping_add(p));
            }
        }
        _ => {}
    }
}

/// Linear prediction of order `N` in place over the residual; `rc` holds the coefficients
/// oldest sample first.
#[inline(always)]
fn lpc_n<S: Sample, const N: usize>(rc: &[i64], shift: u32, buf: &mut [S]) {
    let mut c = [0i64; N];
    c.copy_from_slice(&rc[..N]);
    for i in N..buf.len() {
        let w = &buf[i - N..i];
        let mut sum = 0i64;
        for j in 0..N {
            sum = sum.wrapping_add(c[j].wrapping_mul(w[j].to_i64()));
        }
        buf[i] = S::from_i64((sum >> shift).wrapping_add(buf[i].to_i64()));
    }
}

/// The 32-bit form for streams whose sums fit: 16-bit audio with typical precisions.
#[inline(always)]
fn lpc_n32<const N: usize>(rc: &[i64], shift: u32, buf: &mut [i32]) {
    let mut c = [0i32; N];
    for (d, &s) in c.iter_mut().zip(rc) {
        *d = s as i32;
    }
    for i in N..buf.len() {
        let w = &buf[i - N..i];
        let mut sum = 0i32;
        for j in 0..N {
            sum = sum.wrapping_add(c[j].wrapping_mul(w[j]));
        }
        buf[i] = (sum >> shift).wrapping_add(buf[i]);
    }
}

/// Calls `$f::<[S,] N>(rc, shift, buf)` for the order `N` in 1..=32.
macro_rules! dispatch {
    ($order:expr, $call:ident; $($n:literal)*) => {
        match $order {
            $($n => $call!($n),)*
            _ => {}
        }
    };
}

fn lpc<S: Sample>(order: usize, coefs: &[i64], shift: u32, buf: &mut [S]) {
    let mut rc = [0i64; 32];
    for (d, &c) in rc[..order].iter_mut().zip(coefs.iter().rev()) {
        *d = c;
    }
    let rc = &rc[..order];
    macro_rules! call {
        ($n:literal) => {
            lpc_n::<S, $n>(rc, shift, buf)
        };
    }
    dispatch!(order, call;
        1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32);
}

/// Whether the 32-bit predictor sum cannot overflow for samples of `bits` bits: the sum of
/// the coefficient magnitudes times the largest sample magnitude fits.
fn fits_i32(bits: u32, coefs: &[i64]) -> bool {
    let total: i64 = coefs.iter().map(|c| c.abs()).sum();
    total << (bits - 1) <= i64::from(i32::MAX)
}

/// LPC on 32-bit storage, taking the 32-bit sum when it cannot overflow.
fn lpc_i32(order: usize, coefs: &[i64], shift: u32, buf: &mut [i32], narrow: bool) {
    if !narrow {
        return lpc(order, coefs, shift, buf);
    }
    let mut rc = [0i64; 32];
    for (d, &c) in rc[..order].iter_mut().zip(coefs.iter().rev()) {
        *d = c;
    }
    let rc = &rc[..order];
    macro_rules! call {
        ($n:literal) => {
            lpc_n32::<$n>(rc, shift, buf)
        };
    }
    dispatch!(order, call;
        1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32);
}

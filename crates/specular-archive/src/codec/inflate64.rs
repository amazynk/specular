//! Raw Deflate (RFC 1951) and Deflate64, PKWARE's "enhanced deflate" (ZIP method 9).
//!
//! Deflate64 is Deflate with a 64 KiB window: distance codes 30 and 31 (bases 32769 and 49153,
//! 14 extra bits) become valid, and length code 285 means 3 plus 16 extra bits instead of 258.
//! Nothing else changes, so one decoder reads both. ZIP's Deflate entries keep the faster
//! zlib-rs backend (`crate::inflate`); this one reads Deflate64 entries, and Deflate in tests,
//! where it is checked against zlib-rs.
//!
//! The decoder keeps a 64-bit bit buffer refilled eight bytes at a time and decodes Huffman
//! codes of up to [`FAST_BITS`] bits with one table lookup; longer codes (rare) are walked bit by
//! bit over the canonical code. Output goes straight into the caller's vector, which is also the
//! window.

use super::{CResult, CodecError, copy_match, room};

/// Bits looked up at once; longer codes take the canonical walk.
const FAST_BITS: u32 = 10;

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u32; 32] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 32769, 49153,
];
const DIST_EXTRA: [u8; 32] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13, 14, 14,
];
/// Order of the code-length code lengths in a dynamic block header.
const CLEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Least-significant-bit-first reader with a 64-bit buffer.
struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load into `buf`.
    pos: usize,
    buf: u64,
    n: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Bits<'a> {
        Bits {
            data,
            pos: 0,
            buf: 0,
            n: 0,
        }
    }

    /// At least 56 bits in the buffer (zeros past the end of the data).
    #[inline]
    fn refill(&mut self) {
        if let Some(w) = self.data.get(self.pos..self.pos + 8) {
            let w = u64::from_le_bytes(w.try_into().unwrap_or([0; 8]));
            self.buf |= w << self.n;
            self.pos += (63 - self.n as usize) >> 3;
            self.n |= 56;
        } else {
            while self.n <= 56 {
                let b = self.data.get(self.pos).copied().unwrap_or(0);
                self.buf |= u64::from(b) << self.n;
                self.pos += 1;
                self.n += 8;
            }
        }
    }

    /// Bits consumed past the end of the data (an error once a symbol needed them).
    #[inline]
    fn overrun(&self) -> bool {
        (self.pos * 8).saturating_sub(self.n as usize) > self.data.len() * 8
    }

    /// The next `k` (≤ 32) bits, after a refill.
    #[inline]
    fn take(&mut self, k: u32) -> u32 {
        let v = (self.buf & ((1u64 << k) - 1)) as u32;
        self.buf >>= k;
        self.n -= k;
        v
    }

    fn bits(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.refill();
        }
        self.take(k)
    }

    /// Drops the bits up to the next byte boundary and returns the byte position.
    fn align(&mut self) -> usize {
        let drop = self.n % 8;
        self.take(drop);
        let at = self.pos - (self.n / 8) as usize;
        self.buf = 0;
        self.n = 0;
        self.pos = at;
        at
    }
}

/// A canonical Huffman code: a table for codes up to [`FAST_BITS`], and the counts and sorted
/// symbols for the walk over longer ones.
struct Huffman {
    /// `symbol << 4 | length`, indexed by the next `FAST_BITS` bits; 0 for longer codes.
    fast: Vec<u16>,
    counts: [u16; 16],
    sorted: Vec<u16>,
}

impl Huffman {
    /// The code with these lengths (0 = unused). Incomplete codes are allowed (a lone distance
    /// code is legal); over-subscribed ones are not.
    fn new(lengths: &[u8]) -> CResult<Huffman> {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[usize::from(l)] += 1;
        }
        counts[0] = 0;
        let mut left: i32 = 1;
        for &c in &counts[1..] {
            left = left * 2 - i32::from(c);
            if left < 0 {
                return Err(CodecError::Corrupt("over-subscribed Huffman code"));
            }
        }
        let mut offs = [0u16; 16];
        for l in 1..15 {
            offs[l + 1] = offs[l] + counts[l];
        }
        let mut sorted = vec![0u16; lengths.len()];
        let mut next_code = [0u32; 16];
        let mut code = 0u32;
        for l in 1..16 {
            code = (code + u32::from(counts[l - 1])) << 1;
            next_code[l] = code;
        }
        // counts[0] was zeroed, so next_code[1] starts at 0 as it must.
        let mut fast = vec![0u16; 1 << FAST_BITS];
        for (sym, &l) in lengths.iter().enumerate() {
            if l == 0 {
                continue;
            }
            let l = u32::from(l);
            sorted[usize::from(offs[l as usize])] = sym as u16;
            offs[l as usize] += 1;
            let c = next_code[l as usize];
            next_code[l as usize] += 1;
            if l <= FAST_BITS {
                let rev = c.reverse_bits() >> (32 - l);
                let entry = ((sym as u16) << 4) | l as u16;
                let mut i = rev as usize;
                while i < fast.len() {
                    fast[i] = entry;
                    i += 1 << l;
                }
            }
        }
        Ok(Huffman {
            fast,
            counts,
            sorted,
        })
    }

    /// The next symbol; the buffer must hold at least 15 bits.
    #[inline]
    fn decode(&self, b: &mut Bits) -> CResult<u16> {
        let e = self.fast[(b.buf & ((1 << FAST_BITS) - 1)) as usize];
        if e != 0 {
            b.take(u32::from(e & 15));
            return Ok(e >> 4);
        }
        // The canonical walk, one bit at a time (codes are stored most significant bit first).
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for l in 1..16 {
            code |= b.take(1) as i32;
            let count = i32::from(self.counts[l]);
            if code - first < count {
                return Ok(self.sorted[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(CodecError::Corrupt("invalid Huffman code"))
    }
}

fn fixed() -> (Huffman, Huffman) {
    let mut l = [0u8; 288];
    l[..144].fill(8);
    l[144..256].fill(9);
    l[256..280].fill(7);
    l[280..].fill(8);
    // Lengths of fixed codes are valid by construction.
    let lit = Huffman::new(&l).unwrap_or_else(|_| unreachable!());
    let dist = Huffman::new(&[5u8; 32]).unwrap_or_else(|_| unreachable!());
    (lit, dist)
}

fn dynamic(b: &mut Bits, deflate64: bool) -> CResult<(Huffman, Huffman)> {
    b.refill();
    let hlit = b.take(5) as usize + 257;
    let hdist = b.take(5) as usize + 1;
    let hclen = b.take(4) as usize + 4;
    if hlit > 286 || hdist > if deflate64 { 32 } else { 30 } {
        return Err(CodecError::Corrupt("too many codes"));
    }
    let mut cl = [0u8; 19];
    for &i in &CLEN_ORDER[..hclen] {
        cl[i] = b.bits(3) as u8;
    }
    let clh = Huffman::new(&cl)?;
    let mut lens = [0u8; 286 + 32];
    let mut i = 0;
    while i < hlit + hdist {
        b.refill();
        let sym = clh.decode(b)?;
        let (val, rep) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                let prev = *lens[..i]
                    .last()
                    .ok_or(CodecError::Corrupt("repeat with no previous length"))?;
                (prev, 3 + b.take(2) as usize)
            }
            17 => (0, 3 + b.take(3) as usize),
            _ => (0, 11 + b.take(7) as usize),
        };
        if i + rep > hlit + hdist {
            return Err(CodecError::Corrupt("code lengths overflow"));
        }
        lens[i..i + rep].fill(val);
        i += rep;
        if b.overrun() {
            return Err(CodecError::Truncated);
        }
    }
    if lens[256] == 0 {
        return Err(CodecError::Corrupt("no end-of-block code"));
    }
    Ok((
        Huffman::new(&lens[..hlit])?,
        Huffman::new(&lens[hlit..hlit + hdist])?,
    ))
}

/// Inflates the raw Deflate (or, with `deflate64`, Deflate64) stream `input`, appending to `out`
/// without letting it pass `limit` bytes. Returns the input bytes consumed. Data after the final
/// block is ignored.
pub(crate) fn inflate(
    input: &[u8],
    deflate64: bool,
    out: &mut Vec<u8>,
    limit: usize,
) -> CResult<usize> {
    let mut b = Bits::new(input);
    let start = out.len();
    let mut fixed_codes: Option<(Huffman, Huffman)> = None;
    loop {
        b.refill();
        let last = b.take(1) == 1;
        let kind = b.take(2);
        match kind {
            0 => {
                let at = b.align();
                let hdr = input.get(at..at + 4).ok_or(CodecError::Truncated)?;
                let len = usize::from(u16::from_le_bytes([hdr[0], hdr[1]]));
                let nlen = u16::from_le_bytes([hdr[2], hdr[3]]);
                if len as u16 != !nlen {
                    return Err(CodecError::Corrupt("stored block length check"));
                }
                let data = input
                    .get(at + 4..at + 4 + len)
                    .ok_or(CodecError::Truncated)?;
                room(out, len, limit)?;
                out.extend_from_slice(data);
                b.pos = at + 4 + len;
            }
            1 | 2 => {
                let dynamic_codes;
                let (lit, dist) = if kind == 1 {
                    let f = fixed_codes.get_or_insert_with(fixed);
                    (&f.0, &f.1)
                } else {
                    dynamic_codes = dynamic(&mut b, deflate64)?;
                    (&dynamic_codes.0, &dynamic_codes.1)
                };
                loop {
                    b.refill();
                    if b.overrun() {
                        return Err(CodecError::Truncated);
                    }
                    let sym = lit.decode(&mut b)?;
                    if sym < 256 {
                        if out.len() >= limit {
                            return Err(CodecError::Limit);
                        }
                        out.push(sym as u8);
                        continue;
                    }
                    if sym == 256 {
                        break;
                    }
                    let k = usize::from(sym - 257);
                    if k >= 29 {
                        return Err(CodecError::Corrupt("invalid length code"));
                    }
                    let len = if deflate64 && k == 28 {
                        3 + b.take(16) as usize
                    } else {
                        usize::from(LEN_BASE[k]) + b.take(u32::from(LEN_EXTRA[k])) as usize
                    };
                    b.refill();
                    let d = usize::from(dist.decode(&mut b)?);
                    if d >= if deflate64 { 32 } else { 30 } {
                        return Err(CodecError::Corrupt("invalid distance code"));
                    }
                    let distance =
                        DIST_BASE[d] as usize + b.take(u32::from(DIST_EXTRA[d])) as usize;
                    if distance > out.len() - start {
                        return Err(CodecError::Corrupt("distance too far back"));
                    }
                    room(out, len, limit)?;
                    copy_match(out, distance, len);
                }
            }
            _ => return Err(CodecError::Corrupt("invalid block type")),
        }
        if b.overrun() {
            return Err(CodecError::Truncated);
        }
        if last {
            let used = (b.pos * 8 - b.n as usize).div_ceil(8);
            return Ok(used);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deflate(data: &[u8], level: i32) -> Vec<u8> {
        let mut out = vec![0u8; data.len() * 2 + 1024];
        let config = zlib_rs::DeflateConfig {
            level,
            window_bits: -15,
            ..Default::default()
        };
        let (c, rc) = zlib_rs::compress_slice(&mut out, data, config);
        assert_eq!(rc, zlib_rs::ReturnCode::Ok);
        c.to_vec()
    }

    fn sample(n: usize) -> Vec<u8> {
        let mut x = 0x1234_5678u32;
        (0..n)
            .map(|i| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                if i % 1000 < 600 {
                    b"the quick brown fox "[i % 20]
                } else {
                    (x >> 24) as u8
                }
            })
            .collect()
    }

    #[test]
    fn inflates_like_zlib() {
        let data = sample(300_000);
        for level in [0, 1, 6, 9] {
            let c = deflate(&data, level);
            let mut out = Vec::new();
            let used = inflate(&c, false, &mut out, data.len()).unwrap();
            assert_eq!(out, data, "level {level}");
            assert_eq!(used, c.len());
        }
        // Empty input, and runs (distance 1).
        let mut out = Vec::new();
        inflate(&deflate(b"", 6), false, &mut out, 0).unwrap();
        let zeros = vec![0u8; 100_000];
        inflate(&deflate(&zeros, 9), false, &mut out, zeros.len()).unwrap();
        assert_eq!(out, zeros);
    }

    #[test]
    fn hostile_streams() {
        let data = sample(50_000);
        let c = deflate(&data, 6);
        // Over the limit.
        let mut out = Vec::new();
        assert_eq!(
            inflate(&c, false, &mut out, data.len() - 1),
            Err(CodecError::Limit)
        );
        assert!(out.len() < data.len());
        // Every truncation and some corruptions: errors, never panics.
        for cut in (0..c.len()).step_by(97) {
            let mut out = Vec::new();
            assert!(inflate(&c[..cut], false, &mut out, data.len()).is_err());
        }
        let mut bad = c.clone();
        for i in (0..bad.len()).step_by(31) {
            bad[i] ^= 0x5A;
            let mut out = Vec::new();
            let _ = inflate(&bad, true, &mut out, data.len());
        }
        // A bomb: limited.
        let zeros = vec![0u8; 10 << 20];
        let c = deflate(&zeros, 9);
        let mut out = Vec::new();
        assert_eq!(
            inflate(&c, false, &mut out, 1 << 20),
            Err(CodecError::Limit)
        );
        assert!(out.capacity() <= 4 << 20);
    }

    #[test]
    fn deflate64_codes() {
        // Fixed-code block: literal 'a', length code 285 with 16 extra bits (Deflate64 length
        // 3 + 1000), distance code 0 (distance 1), end of block.
        let mut bits: Vec<(u32, u32)> = vec![(1, 1), (1, 2)];
        // Literal 'a' = 0x61: fixed codes 0..143 are 8 bits starting at 0x30.
        let push_code = |bits: &mut Vec<(u32, u32)>, code: u32, len: u32| {
            for i in (0..len).rev() {
                bits.push(((code >> i) & 1, 1));
            }
        };
        push_code(&mut bits, 0x30 + 0x61, 8);
        // 285: fixed codes 280..287 are 8 bits starting at 0xC0.
        push_code(&mut bits, 0xC0 + 5, 8);
        bits.push((1000, 16));
        push_code(&mut bits, 0, 5);
        // End of block: 256 is the 7-bit code 0.
        push_code(&mut bits, 0, 7);
        let mut bytes = Vec::new();
        let (mut acc, mut n) = (0u64, 0u32);
        for (v, k) in bits {
            acc |= u64::from(v) << n;
            n += k;
            while n >= 8 {
                bytes.push(acc as u8);
                acc >>= 8;
                n -= 8;
            }
        }
        if n > 0 {
            bytes.push(acc as u8);
        }
        let mut out = Vec::new();
        inflate(&bytes, true, &mut out, 2000).unwrap();
        assert_eq!(out, vec![b'a'; 1004]);
        // As plain Deflate the same bits mean something else: no panic either way.
        let mut out = Vec::new();
        let _ = inflate(&bytes, false, &mut out, 2000);
    }
}

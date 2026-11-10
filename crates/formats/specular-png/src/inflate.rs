//! DEFLATE (RFC 1951) and zlib (RFC 1950) decompression.
//!
//! Written for Specular in safe Rust, after the design of libdeflate's decompressor
//! (`lib/deflate_decompress.c`, Copyright 2016 Eric Biggers, MIT licence): a 64-bit bit buffer
//! refilled branchlessly with one unaligned 8-byte load, Huffman decode tables whose entries
//! carry everything needed to decode a symbol *and* its extra bits in one step (literal value,
//! or length/distance base with the extra-bit count folded into the bits to consume), 11-bit
//! (literal/length) and 8-bit (distance) main tables with subtables for longer codewords, and a
//! fast loop that decodes up to three literals per refill and copies matches in 16-byte words
//! (writing past the end of the match into slack space).
//!
//! Unlike libdeflate the decoder is *resumable* at symbol granularity: it stops when the output
//! window is (nearly) full and continues later, so PNG rows can be unfiltered while they are
//! still in cache and a whole image never has to be held in filtered form. The caller owns the
//! output buffer and keeps at least the last 32 KiB of output in front of the write position.

/// Bits of the literal/length main table (4096 entries, 16 KiB: stays in L1).
const LITLEN_TABLEBITS: u32 = 12;
/// Bits of the distance main table.
const DIST_TABLEBITS: u32 = 8;
/// Bits of the code-length (precode) table: precode codewords are at most 7 bits long.
const PRECODE_TABLEBITS: u32 = 7;
/// Table sizes including subtables. Distance: zlib's `enough` for 32 symbols with 8 table bits
/// (libdeflate's value). Literal/length: every subtable holds at least one of the 288 symbols
/// and at most `2^(15 - 12)` entries, which bounds the subtables.
const LITLEN_ENOUGH: usize = (1 << LITLEN_TABLEBITS) + 288 * (1 << (15 - LITLEN_TABLEBITS));
const DIST_ENOUGH: usize = 402;

/// Table entry layout (u32):
/// * bits 0..6: number of bits to consume (codeword length(s) plus extra bits; for a subtable
///   pointer, the main table bits). Six bits, so that a shift by the whole entry needs no mask
///   on x86-64 (`shrx` masks the count itself).
/// * bits 8..12: for a literal entry, the number of literals (1 or 2); otherwise the codeword
///   length (for extracting the extra bits), or for a subtable pointer the number of subtable
///   index bits,
/// * bits 12..16: flags,
/// * bits 16..32: value: one or two literal bytes (first in bits 16..24), a length or distance
///   base, or a subtable start.
///
/// Literal/length main-table entries hold *two* literals when both codewords fit in the table
/// bits together (fdeflate's "double literal" entries): filtered PNG data is dominated by short
/// literal codes, so this halves the serial lookup chain.
const F_INVALID: u32 = 1 << 12;
const F_SUBTABLE: u32 = 1 << 13;
const F_EOB: u32 = 1 << 14;
const F_LITERAL: u32 = 1 << 15;
/// Any entry that is neither a literal nor a length.
const F_EXCEPTIONAL: u32 = F_INVALID | F_SUBTABLE | F_EOB;
/// An entry that makes the decoder fail (unused codewords of incomplete codes, symbols 286/287
/// and 30/31). It consumes one bit so the slow path can never spin.
const INVALID: u32 = F_INVALID | 1;
/// Mask of the bit count in an entry.
const BITS: u32 = 63;

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const PRECODE_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Longest match.
pub(crate) const MAX_MATCH: usize = 258;
/// Bytes the decoder may write past the output limit it was given (match copies in 16-byte
/// words). Output buffers must be at least `limit + OUT_SLACK` long.
pub(crate) const OUT_SLACK: usize = 32;
/// Size of the DEFLATE window: output the caller must keep in front of the write position.
pub(crate) const WINDOW: usize = 32 * 1024;

/// Fast loop margins: room for two refills (input) and for six literals plus one maximal match
/// (output).
const IN_MARGIN: usize = 16;
const OUT_MARGIN: usize = 6 + MAX_MATCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InflateError {
    #[error("bad zlib header")]
    BadHeader,
    #[error("invalid deflate block type")]
    BadBlockType,
    #[error("stored block length check failed")]
    BadStoredLength,
    #[error("invalid Huffman code")]
    BadCode,
    #[error("invalid symbol")]
    BadSymbol,
    #[error("distance too far back")]
    BadDistance,
    #[error("compressed data ends early")]
    Truncated,
    /// The Adler-32 (zlib) or CRC-32 and length (gzip) trailer does not match.
    #[error("checksum mismatch")]
    BadChecksum,
    /// The output grew past what the memory budget has free.
    #[error("{0}")]
    NeedsMemory(specular_core::NeedsMemory),
}

impl From<specular_core::NeedsMemory> for InflateError {
    fn from(n: specular_core::NeedsMemory) -> Self {
        InflateError::NeedsMemory(n)
    }
}

/// Result of one [`Inflater::inflate`] call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Progress {
    /// Fewer than [`MAX_MATCH`] bytes of room before the limit remain; make room and call again.
    OutputFull,
    /// The final block has ended.
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    BlockHeader,
    Stored { remaining: usize },
    Huffman,
    Done,
}

/// A resumable raw-DEFLATE decoder over a complete input buffer.
pub(crate) struct Inflater<'a> {
    input: &'a [u8],
    /// Next input byte to load into the bit buffer. May exceed `input.len()` near the end: the
    /// input is virtually followed by zero bytes, and running into them is caught by
    /// [`Inflater::overrun`].
    pos: usize,
    bitbuf: u64,
    bitsleft: u32,
    state: State,
    final_block: bool,
    litlen: Box<[u32; LITLEN_ENOUGH]>,
    dist: Box<[u32; DIST_ENOUGH]>,
}

impl<'a> Inflater<'a> {
    /// A decoder for the raw DEFLATE stream `input`.
    pub(crate) fn new(input: &'a [u8]) -> Inflater<'a> {
        Inflater {
            input,
            pos: 0,
            bitbuf: 0,
            bitsleft: 0,
            state: State::BlockHeader,
            final_block: false,
            litlen: Box::new([INVALID; LITLEN_ENOUGH]),
            dist: Box::new([INVALID; DIST_ENOUGH]),
        }
    }

    /// A decoder for a zlib stream: checks and skips the 2-byte header.
    pub(crate) fn new_zlib(input: &'a [u8]) -> Result<Inflater<'a>, InflateError> {
        if input.len() < 2 {
            return Err(InflateError::Truncated);
        }
        let (cmf, flg) = (input[0], input[1]);
        if cmf & 0x0f != 8 || cmf >> 4 > 7 || (u16::from(cmf) << 8 | u16::from(flg)) % 31 != 0 {
            return Err(InflateError::BadHeader);
        }
        if flg & 0x20 != 0 {
            // Preset dictionary: not allowed in PNG.
            return Err(InflateError::BadHeader);
        }
        Ok(Inflater::new(&input[2..]))
    }

    /// Bytes of input consumed so far (rounded up to whole bytes).
    pub(crate) fn consumed(&self) -> usize {
        self.pos - (self.bitsleft / 8) as usize
    }

    /// The Adler-32 trailer of a zlib stream, once the final block was decoded.
    pub(crate) fn adler_trailer(&self) -> Option<u32> {
        let p = self.consumed();
        let b = self.input.get(p..p + 4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Whether more bits have been consumed than the input holds.
    #[inline]
    fn overrun(&self) -> bool {
        // Consumed bits = 8 * pos - bitsleft.
        (self.pos as u64) * 8 > (self.input.len() as u64) * 8 + u64::from(self.bitsleft)
    }

    /// Refill so that at least 56 bits are buffered (bytes past the end read as zero).
    #[inline]
    fn refill_slow(&mut self) {
        if self.pos + 8 <= self.input.len() {
            self.refill_fast();
            return;
        }
        while self.bitsleft <= 56 {
            let b = self.input.get(self.pos).copied().unwrap_or(0);
            self.bitbuf |= u64::from(b) << self.bitsleft;
            self.pos += 1;
            self.bitsleft += 8;
        }
    }

    /// Branchless refill (libdeflate's `REFILL_BITS_BRANCHLESS`); needs `pos + 8 <= len`.
    #[inline(always)]
    fn refill_fast(&mut self) {
        let w = u64::from_le_bytes(self.input[self.pos..self.pos + 8].try_into().unwrap());
        self.bitbuf |= w << self.bitsleft;
        self.pos += ((63 - self.bitsleft) >> 3) as usize;
        self.bitsleft |= 56;
    }

    #[inline(always)]
    fn consume(&mut self, n: u32) {
        self.bitbuf >>= n;
        self.bitsleft -= n;
    }

    /// Read `n <= 32` bits (slow path; refills as needed).
    fn bits(&mut self, n: u32) -> u32 {
        if self.bitsleft < n {
            self.refill_slow();
        }
        let v = (self.bitbuf & ((1u64 << n) - 1)) as u32;
        self.consume(n);
        v
    }

    /// Decode into `out[*out_pos..limit]` (writing at most `OUT_SLACK` bytes past `limit`, which
    /// `out` must hold). Back-references may reach anywhere into `out[..*out_pos]`.
    pub(crate) fn inflate(
        &mut self,
        out: &mut [u8],
        out_pos: &mut usize,
        limit: usize,
    ) -> Result<Progress, InflateError> {
        debug_assert!(out.len() >= limit + OUT_SLACK);
        loop {
            match self.state {
                State::Done => return Ok(Progress::Done),
                State::BlockHeader => {
                    if self.final_block {
                        self.state = State::Done;
                        continue;
                    }
                    self.read_block_header()?;
                }
                State::Stored { remaining } => {
                    let room = limit.saturating_sub(*out_pos);
                    let avail = self.input.len().saturating_sub(self.pos);
                    let n = remaining.min(room).min(avail);
                    out[*out_pos..*out_pos + n]
                        .copy_from_slice(&self.input[self.pos..self.pos + n]);
                    *out_pos += n;
                    self.pos += n;
                    if n == remaining {
                        self.state = State::BlockHeader;
                    } else if n == room {
                        self.state = State::Stored {
                            remaining: remaining - n,
                        };
                        return Ok(Progress::OutputFull);
                    } else {
                        return Err(InflateError::Truncated);
                    }
                }
                State::Huffman => {
                    if self.decode_huffman(out, out_pos, limit)? {
                        self.state = State::BlockHeader;
                    } else {
                        return Ok(Progress::OutputFull);
                    }
                }
            }
        }
    }

    fn read_block_header(&mut self) -> Result<(), InflateError> {
        self.refill_slow();
        self.final_block = self.bits(1) == 1;
        match self.bits(2) {
            0 => {
                // Stored: skip to a byte boundary, then LEN and NLEN.
                let skip = self.bitsleft & 7;
                self.consume(skip);
                self.refill_slow();
                let len = self.bits(16);
                let nlen = self.bits(16);
                if self.overrun() {
                    return Err(InflateError::Truncated);
                }
                if len != !nlen & 0xffff {
                    return Err(InflateError::BadStoredLength);
                }
                // Give back the whole bytes still in the bit buffer.
                debug_assert!(self.bitsleft.is_multiple_of(8));
                self.pos -= (self.bitsleft / 8) as usize;
                self.bitbuf = 0;
                self.bitsleft = 0;
                self.state = State::Stored {
                    remaining: len as usize,
                };
            }
            1 => {
                let mut lens = [0u8; 288 + 32];
                lens[..144].fill(8);
                lens[144..256].fill(9);
                lens[256..280].fill(7);
                lens[280..288].fill(8);
                lens[288..].fill(5);
                self.build_tables(&lens[..288], &lens[288..])?;
                self.state = State::Huffman;
            }
            2 => {
                self.read_dynamic_header()?;
                self.state = State::Huffman;
            }
            _ => return Err(InflateError::BadBlockType),
        }
        if self.overrun() {
            return Err(InflateError::Truncated);
        }
        Ok(())
    }

    fn read_dynamic_header(&mut self) -> Result<(), InflateError> {
        self.refill_slow();
        let hlit = self.bits(5) as usize + 257;
        let hdist = self.bits(5) as usize + 1;
        let hclen = self.bits(4) as usize + 4;
        let mut pre_lens = [0u8; 19];
        for &i in &PRECODE_ORDER[..hclen] {
            pre_lens[i] = self.bits(3) as u8;
        }
        let mut pre = [INVALID; 1 << PRECODE_TABLEBITS];
        build_table(
            &pre_lens,
            &mut pre,
            PRECODE_TABLEBITS,
            |s| (s as u32) << 16,
            false,
        )?;
        let mut lens = [0u8; 288 + 32];
        let total = hlit + hdist;
        let mut i = 0;
        while i < total {
            if self.bitsleft < 7 + 7 {
                self.refill_slow();
            }
            let e = pre[(self.bitbuf & ((1 << PRECODE_TABLEBITS) - 1)) as usize];
            if e & F_INVALID != 0 {
                return Err(InflateError::BadCode);
            }
            self.consume(e & BITS);
            let sym = (e >> 16) as u8;
            let (val, rep) = match sym {
                0..=15 => (sym, 1),
                16 => {
                    if i == 0 {
                        return Err(InflateError::BadCode);
                    }
                    (lens[i - 1], 3 + self.bits(2) as usize)
                }
                17 => (0, 3 + self.bits(3) as usize),
                _ => (0, 11 + self.bits(7) as usize),
            };
            if i + rep > total {
                return Err(InflateError::BadCode);
            }
            lens[i..i + rep].fill(val);
            i += rep;
            if self.overrun() {
                return Err(InflateError::Truncated);
            }
        }
        if lens[256] == 0 {
            // No end-of-block code.
            return Err(InflateError::BadCode);
        }
        // The literal/length lengths are followed directly by the distance lengths.
        let (ll, rest) = lens.split_at(hlit);
        let mut ll_full = [0u8; 288];
        ll_full[..hlit.min(288)].copy_from_slice(&ll[..hlit.min(288)]);
        self.build_tables(&ll_full, &rest[..hdist])
    }

    fn build_tables(&mut self, litlen: &[u8], dist: &[u8]) -> Result<(), InflateError> {
        build_table(
            litlen,
            &mut self.litlen[..],
            LITLEN_TABLEBITS,
            |s| match s {
                0..=255 => (s as u32) << 16 | F_LITERAL,
                256 => F_EOB,
                257..=285 => {
                    let i = s - 257;
                    u32::from(LEN_BASE[i]) << 16 | u32::from(LEN_EXTRA[i])
                }
                _ => F_INVALID,
            },
            true,
        )?;
        build_table(
            dist,
            &mut self.dist[..],
            DIST_TABLEBITS,
            |s| match s {
                0..=29 => u32::from(DIST_BASE[s]) << 16 | u32::from(DIST_EXTRA[s]),
                _ => F_INVALID,
            },
            false,
        )
    }

    /// Decode the current Huffman block. Returns `true` at the end of the block, `false` when
    /// the output is full.
    fn decode_huffman(
        &mut self,
        out: &mut [u8],
        out_pos: &mut usize,
        limit: usize,
    ) -> Result<bool, InflateError> {
        let mut o = *out_pos;
        let r = self.fast_loop(out, &mut o, limit);
        match r {
            Ok(Some(eob)) => {
                *out_pos = o;
                return Ok(eob);
            }
            Ok(None) => {}
            Err(e) => {
                *out_pos = o;
                return Err(e);
            }
        }
        let r = self.slow_loop(out, &mut o, limit);
        *out_pos = o;
        r
    }

    /// The fast loop: runs while there are at least [`IN_MARGIN`] input bytes and
    /// [`OUT_MARGIN`] bytes of output room left. `Some(true)` at the end of the block,
    /// `None` when it hits a margin (the slow loop continues).
    ///
    /// After a refill the bit buffer holds 64 real bits (the counter says 56..=63: whole bytes
    /// only), and consuming bits keeps the ones above the counter valid, so the next table entry
    /// is looked up *before* the refill that follows the consume and the load overlaps it (the
    /// refill only ORs in bits that are already there). Literal runs decode up to three main
    /// entries (up to six literals, at most 36 bits) per refill.
    #[inline(never)]
    fn fast_loop(
        &mut self,
        out: &mut [u8],
        o: &mut usize,
        limit: usize,
    ) -> Result<Option<bool>, InflateError> {
        const LMASK: u64 = (1 << LITLEN_TABLEBITS) - 1;
        const DMASK: u64 = (1 << DIST_TABLEBITS) - 1;
        if self.input.len() < IN_MARGIN || limit < OUT_MARGIN {
            return Ok(None);
        }
        let in_end = self.input.len() - IN_MARGIN;
        let out_end = limit - OUT_MARGIN;
        let litlen: &[u32; LITLEN_ENOUGH] = &self.litlen;
        let dist: &[u32; DIST_ENOUGH] = &self.dist;
        let input = self.input;
        let mut pos = *o;
        // Local copies of the bit state: the compiler keeps them in registers.
        let mut bitbuf = self.bitbuf;
        // Only the low six bits of `bitsleft` are meaningful here (libdeflate's trick): whole
        // entries are subtracted from it, and their higher bits only disturb its higher bits.
        // That keeps the `& 63` off the dependency chain of the literal loop.
        let mut bitsleft = self.bitsleft;
        let mut ip = self.pos;
        macro_rules! refill {
            () => {{
                let w = u64::from_le_bytes(load_u64(input, ip));
                bitbuf |= w.wrapping_shl(bitsleft);
                ip += ((!bitsleft & 63) >> 3) as usize;
                bitsleft |= 56;
            }};
        }
        macro_rules! consume {
            ($n:expr) => {{
                let n: u32 = $n;
                bitbuf >>= n;
                bitsleft = bitsleft.wrapping_sub(n);
            }};
        }
        // Consume the bits of entry `e` (its low six bits).
        macro_rules! consume_e {
            ($e:expr) => {{
                let e: u32 = $e;
                bitbuf = bitbuf.wrapping_shr(e);
                bitsleft = bitsleft.wrapping_sub(e);
            }};
        }
        // Write the literal(s) of entry `e` (always two bytes; the count says how many count).
        macro_rules! put {
            ($e:expr) => {{
                let e = $e;
                out[pos..pos + 2].copy_from_slice(&((e >> 16) as u16).to_le_bytes());
                pos += ((e >> 8) & 3) as usize;
            }};
        }
        if self.pos > in_end || pos > out_end {
            return Ok(None);
        }
        refill!();
        let mut e = litlen[(bitbuf & LMASK) as usize];
        let result = loop {
            // Invariant: the buffer was just refilled and `e` is the entry for its low bits.
            if ip > in_end || pos > out_end {
                break Ok(None);
            }
            if e & F_LITERAL != 0 {
                // Consume by shifting by the whole entry: `wrapping_shr` masks the count to six
                // bits, as `shrx` does, so the chain per entry is load -> shift -> mask -> load.
                put!(e);
                consume_e!(e);
                e = litlen[(bitbuf & LMASK) as usize];
                if e & F_LITERAL != 0 {
                    put!(e);
                    consume_e!(e);
                    e = litlen[(bitbuf & LMASK) as usize];
                    if e & F_LITERAL != 0 {
                        put!(e);
                        consume_e!(e);
                        e = litlen[(bitbuf & LMASK) as usize];
                        refill!();
                        continue;
                    }
                }
                refill!();
            }
            // `e` is a length, end of block, subtable pointer or invalid; >= 56 bits buffered.
            if e & F_EXCEPTIONAL != 0 {
                if e & F_SUBTABLE != 0 {
                    consume!(LITLEN_TABLEBITS);
                    let sub_bits = (e >> 8) & 0xf;
                    e = litlen[(e >> 16) as usize + (bitbuf & ((1u64 << sub_bits) - 1)) as usize];
                    if e & F_LITERAL != 0 {
                        consume!(e & BITS);
                        out[pos] = (e >> 16) as u8;
                        pos += 1;
                        e = litlen[(bitbuf & LMASK) as usize];
                        refill!();
                        continue;
                    }
                }
                if e & (F_EOB | F_INVALID) != 0 {
                    if e & F_INVALID != 0 {
                        break Err(InflateError::BadSymbol);
                    }
                    consume!(e & BITS);
                    break Ok(Some(true));
                }
            }
            // Length (at most 20 bits), then distance (at most 28): no refill in between.
            let saved = bitbuf;
            let total = e & BITS;
            let cw = (e >> 8) & 0xf;
            consume_e!(e);
            let len = (e >> 16) as usize + ((saved & ((1u64 << total) - 1)) >> cw) as usize;
            let mut d = dist[(bitbuf & DMASK) as usize];
            if d & F_EXCEPTIONAL != 0 {
                if d & F_SUBTABLE != 0 {
                    consume!(DIST_TABLEBITS);
                    let sub_bits = (d >> 8) & 0xf;
                    d = dist[(d >> 16) as usize + (bitbuf & ((1u64 << sub_bits) - 1)) as usize];
                }
                if d & F_INVALID != 0 {
                    break Err(InflateError::BadSymbol);
                }
            }
            let saved = bitbuf;
            let total = d & BITS;
            let cw = (d >> 8) & 0xf;
            consume_e!(d);
            let distance = (d >> 16) as usize + ((saved & ((1u64 << total) - 1)) >> cw) as usize;
            e = litlen[(bitbuf & LMASK) as usize];
            refill!();
            if distance > pos {
                break Err(InflateError::BadDistance);
            }
            copy_match_fast(out, pos, distance, len);
            pos += len;
        };
        self.bitbuf = bitbuf;
        self.bitsleft = bitsleft & 63;
        self.pos = ip;
        *o = pos;
        result
    }

    /// Symbol-at-a-time decoding with full checks, for the end of the input and of the output
    /// window. Returns `true` at the end of the block, `false` when the output is full.
    fn slow_loop(
        &mut self,
        out: &mut [u8],
        o: &mut usize,
        limit: usize,
    ) -> Result<bool, InflateError> {
        const LMASK: u64 = (1 << LITLEN_TABLEBITS) - 1;
        const DMASK: u64 = (1 << DIST_TABLEBITS) - 1;
        loop {
            if *o + MAX_MATCH > limit {
                return Ok(false);
            }
            self.refill_slow();
            let mut e = self.litlen[(self.bitbuf & LMASK) as usize];
            if e & F_SUBTABLE != 0 {
                self.consume(LITLEN_TABLEBITS);
                let sub_bits = (e >> 8) & 0xf;
                e = self.litlen
                    [(e >> 16) as usize + (self.bitbuf & ((1u64 << sub_bits) - 1)) as usize];
            }
            if e & F_INVALID != 0 {
                return Err(if self.overrun() {
                    InflateError::Truncated
                } else {
                    InflateError::BadSymbol
                });
            }
            if e & F_LITERAL != 0 {
                self.consume(e & BITS);
                if self.overrun() {
                    return Err(InflateError::Truncated);
                }
                out[*o..*o + 2].copy_from_slice(&((e >> 16) as u16).to_le_bytes());
                *o += ((e >> 8) & 3) as usize;
                continue;
            }
            if e & F_EOB != 0 {
                self.consume(e & BITS);
                if self.overrun() {
                    return Err(InflateError::Truncated);
                }
                return Ok(true);
            }
            let saved = self.bitbuf;
            let total = e & BITS;
            let cw = (e >> 8) & 0xf;
            self.consume(total);
            let len = (e >> 16) as usize + ((saved & ((1u64 << total) - 1)) >> cw) as usize;
            // A length uses at most 20 bits and a distance 28: refill between them.
            self.refill_slow();
            let mut d = self.dist[(self.bitbuf & DMASK) as usize];
            if d & F_SUBTABLE != 0 {
                self.consume(DIST_TABLEBITS);
                let sub_bits = (d >> 8) & 0xf;
                d = self.dist
                    [(d >> 16) as usize + (self.bitbuf & ((1u64 << sub_bits) - 1)) as usize];
            }
            if d & F_INVALID != 0 {
                return Err(if self.overrun() {
                    InflateError::Truncated
                } else {
                    InflateError::BadSymbol
                });
            }
            let saved = self.bitbuf;
            let total = d & BITS;
            let cw = (d >> 8) & 0xf;
            self.consume(total);
            if self.overrun() {
                return Err(InflateError::Truncated);
            }
            let distance = (d >> 16) as usize + ((saved & ((1u64 << total) - 1)) >> cw) as usize;
            if distance > *o {
                return Err(InflateError::BadDistance);
            }
            copy_match_exact(out, *o, distance, len);
            *o += len;
        }
    }
}

#[inline(always)]
fn load_u64(input: &[u8], ip: usize) -> [u8; 8] {
    input[ip..ip + 8].try_into().unwrap()
}

/// One `N`-byte word from `s` to `d` (read whole, then written: overlap-safe like `copy_within`).
/// Not `copy_within`: that goes through `core::slice::index::range`, and when a PGO build did
/// not inline it the constant length was lost and every word became a `memmove` call (the PNG
/// gate +30 %).
#[inline(always)]
fn copy_word<const N: usize>(out: &mut [u8], s: usize, d: usize) {
    // `first_chunk` rather than `s..s + N`: no overflow check on `s + N`.
    let w = *out[s..].first_chunk::<N>().unwrap();
    *out[d..].first_chunk_mut::<N>().unwrap() = w;
}

/// Copy a match of `len` bytes from `pos - dist` to `pos`; may write up to 15 bytes (< [`OUT_SLACK`])
/// past `pos + len`.
#[inline(always)]
fn copy_match_fast(out: &mut [u8], pos: usize, dist: usize, len: usize) {
    let end = pos + len;
    let mut d = pos;
    let mut s = pos - dist;
    if dist >= 16 {
        // Non-overlapping within a word; earlier words are written before later ones are read.
        let out = &mut out[s..end + 16];
        let (mut s, mut d, end) = (0, dist, end - (pos - dist));
        while d < end {
            copy_word::<16>(out, s, d);
            s += 16;
            d += 16;
        }
    } else if dist >= 8 {
        while d < end {
            copy_word::<8>(out, s, d);
            s += 8;
            d += 8;
        }
    } else if dist == 1 {
        let b = out[s];
        out[d..end].fill(b);
    } else {
        // Short period (2..7): write the first period-multiple of at least 8 bytes byte by byte,
        // then copy 8-byte words from one (enlarged) period back.
        let period = dist * 8usize.div_ceil(dist);
        let first = period.min(len);
        for i in 0..first {
            out[d + i] = out[s + i];
        }
        if first == len {
            return;
        }
        d += first;
        s = d - period;
        while d < end {
            copy_word::<8>(out, s, d);
            s += 8;
            d += 8;
        }
    }
}

/// Copy a match without writing past its end.
#[inline]
fn copy_match_exact(out: &mut [u8], pos: usize, dist: usize, len: usize) {
    let s = pos - dist;
    if dist >= len {
        out.copy_within(s..s + len, pos);
    } else {
        for i in 0..len {
            out[pos + i] = out[s + i];
        }
    }
}

/// Build a canonical-Huffman decode table for codeword lengths `lens` (0 = unused symbol).
/// `entry(sym)` gives the symbol's value / flags / extra-bit count (in bits 0..8); the codeword
/// length is added here. Incomplete codes are accepted (unused codewords decode as invalid),
/// over-subscribed ones are rejected. With `pairs`, main-table entries for two literals whose
/// codewords fit in `table_bits` together decode both.
///
/// The main table is built the way fdeflate builds it: level by level, doubling the filled part
/// (a `memcpy`) before adding the codewords of the next length, so each entry is written about
/// once and pair entries placed at level `len1 + len2` propagate to all longer indices.
fn build_table(
    lens: &[u8],
    table: &mut [u32],
    table_bits: u32,
    entry: impl Fn(usize) -> u32,
    pairs: bool,
) -> Result<(), InflateError> {
    let mut count = [0u16; 16];
    for &l in lens {
        if l > 15 {
            return Err(InflateError::BadCode);
        }
        count[l as usize] += 1;
    }
    count[0] = 0;
    let mut left: i32 = 1;
    for &c in &count[1..] {
        left = (left << 1) - i32::from(c);
        if left < 0 {
            return Err(InflateError::BadCode);
        }
    }
    // Symbols sorted by (length, symbol): length `l` occupies `sorted[offs[l]..offs[l + 1]]`.
    let mut offs = [0u16; 17];
    for l in 1..16 {
        offs[l + 1] = offs[l] + count[l];
    }
    let mut sorted = [0u16; 288 + 32];
    let mut fill = offs;
    for (s, &l) in lens.iter().enumerate() {
        if l != 0 {
            sorted[fill[l as usize] as usize] = s as u16;
            fill[l as usize] += 1;
        }
    }
    // First canonical code of each length.
    let mut next_code = [0u32; 16];
    let mut c = 0u32;
    for l in 1..16 {
        c = (c + u32::from(count[l - 1])) << 1;
        next_code[l] = c;
    }
    // The entry of `sym` whose codeword has `len` bits left to consume.
    let single = |sym: usize, len: u32| {
        let base = entry(sym);
        if base & F_LITERAL != 0 {
            base & !0xfff | 1 << 8 | len
        } else {
            base & !0xfff | len << 8 | (len + (base & 0xff))
        }
    };
    let main_size = 1usize << table_bits;
    // Bit-reversed codeword of each main-table symbol (for pairing).
    let mut rev_code = [0u16; 288 + 32];
    table[0] = INVALID;
    let mut size = 1usize;
    let mut next = 0usize;
    for len in 1..=table_bits {
        table.copy_within(0..size, size);
        size *= 2;
        for _ in 0..count[len as usize] {
            let sym = sorted[next] as usize;
            next += 1;
            let code = next_code[len as usize];
            next_code[len as usize] += 1;
            let rev = code.reverse_bits() >> (32 - len);
            rev_code[sym] = rev as u16;
            table[rev as usize] = single(sym, len);
        }
        if pairs {
            for l1 in 1..len {
                let l2 = len - l1;
                let r2 = offs[l2 as usize] as usize..offs[l2 as usize + 1] as usize;
                for &s1 in &sorted[offs[l1 as usize] as usize..offs[l1 as usize + 1] as usize] {
                    if s1 >= 256 {
                        break;
                    }
                    let c1 = rev_code[s1 as usize] as usize;
                    for &s2 in &sorted[r2.clone()] {
                        if s2 >= 256 {
                            break;
                        }
                        let i = c1 | (rev_code[s2 as usize] as usize) << l1;
                        table[i] =
                            u32::from(s1) << 16 | u32::from(s2) << 24 | F_LITERAL | 2 << 8 | len;
                    }
                }
            }
        }
    }
    // Codewords longer than the main table: subtables after it.
    let mut remaining = count;
    let mut next_sub = main_size;
    let mut cur_prefix = usize::MAX;
    let mut sub_start = 0usize;
    let mut sub_bits = 0u32;
    let n = offs[16] as usize;
    for &sym in &sorted[next..n] {
        let sym = sym as usize;
        let len = u32::from(lens[sym]);
        let code = next_code[len as usize];
        next_code[len as usize] += 1;
        let rev = code.reverse_bits() >> (32 - len);
        let prefix = (rev as usize) & (main_size - 1);
        if prefix != cur_prefix {
            // New subtable: size it for the remaining codewords sharing this prefix.
            let mut bits = len - table_bits;
            let mut left: i32 = 1 << bits;
            loop {
                left -= i32::from(remaining[(bits + table_bits) as usize]);
                if left <= 0 || bits + table_bits >= 15 {
                    break;
                }
                bits += 1;
                left <<= 1;
            }
            sub_bits = bits;
            sub_start = next_sub;
            next_sub += 1 << bits;
            if next_sub > table.len() {
                return Err(InflateError::BadCode);
            }
            table[sub_start..next_sub].fill(INVALID);
            table[prefix] = (sub_start as u32) << 16 | F_SUBTABLE | bits << 8 | table_bits;
            cur_prefix = prefix;
        }
        let r = len - table_bits;
        if r > sub_bits {
            return Err(InflateError::BadCode);
        }
        let e = single(sym, r);
        let step = 1usize << r;
        let mut i = (rev >> table_bits) as usize;
        while i < 1 << sub_bits {
            table[sub_start + i] = e;
            i += step;
        }
        remaining[len as usize] -= 1;
    }
    Ok(())
}

/// Decompress a whole zlib stream (iCCP profiles, etc.), giving up beyond `max_out` bytes.
pub(crate) fn zlib_to_vec(input: &[u8], max_out: usize) -> Result<Vec<u8>, InflateError> {
    to_vec(&mut Inflater::new_zlib(input)?, input.len(), max_out)
}

/// Decompress a whole raw DEFLATE stream (no zlib header), giving up beyond `max_out` bytes.
pub(crate) fn raw_to_vec(input: &[u8], max_out: usize) -> Result<Vec<u8>, InflateError> {
    to_vec(&mut Inflater::new(input), input.len(), max_out)
}

/// Decompress a raw DEFLATE stream whose back-references may reach into a preset dictionary
/// (the bytes before the output), giving up beyond `max_out` bytes of output.
pub(crate) fn raw_with_dictionary_to_vec(
    input: &[u8],
    dict: &[u8],
    max_out: usize,
) -> Result<Vec<u8>, InflateError> {
    // Only the last 32 KiB of a dictionary can be referenced.
    let dict = &dict[dict.len().saturating_sub(32 * 1024)..];
    let first = (dict.len() + (input.len() * 4).clamp(1024, max_out.max(1024))) + OUT_SLACK;
    let mut res = specular_core::budget::reserve(first as u64)?;
    let mut out = vec![0u8; first];
    out[..dict.len()].copy_from_slice(dict);
    let mut pos = dict.len();
    let cap = dict.len().saturating_add(max_out);
    let mut inf = Inflater::new(input);
    loop {
        let limit = out.len() - OUT_SLACK;
        match inf.inflate(&mut out, &mut pos, limit)? {
            Progress::Done => {
                out.truncate(pos);
                return Ok(out.split_off(dict.len()));
            }
            Progress::OutputFull => {
                let n = grown(out.len(), cap).ok_or(InflateError::BadSymbol)?;
                res.grow_to(n as u64)?;
                out.resize(n, 0);
            }
        }
    }
}

/// Length of the header of a gzip member (RFC 1952): magic, method 8 (DEFLATE), flags (no
/// reserved bits), mtime, extra flags and OS, then the optional extra field, NUL-terminated
/// name and comment, and header CRC.
fn gzip_header(input: &[u8]) -> Result<usize, InflateError> {
    const FHCRC: u8 = 2;
    const FEXTRA: u8 = 4;
    const FNAME: u8 = 8;
    const FCOMMENT: u8 = 16;
    let h = input.get(..10).ok_or(InflateError::Truncated)?;
    if h[..3] != [0x1f, 0x8b, 8] || h[3] & 0xe0 != 0 {
        return Err(InflateError::BadHeader);
    }
    let flags = h[3];
    let mut p = 10;
    if flags & FEXTRA != 0 {
        let n = input.get(p..p + 2).ok_or(InflateError::Truncated)?;
        p += 2 + usize::from(u16::from_le_bytes([n[0], n[1]]));
    }
    for f in [FNAME, FCOMMENT] {
        if flags & f != 0 {
            let rest = input.get(p..).ok_or(InflateError::Truncated)?;
            p += 1 + rest
                .iter()
                .position(|&b| b == 0)
                .ok_or(InflateError::Truncated)?;
        }
    }
    if flags & FHCRC != 0 {
        p += 2;
    }
    if p > input.len() {
        return Err(InflateError::Truncated);
    }
    Ok(p)
}

/// Decompress the first member of a gzip stream (RFC 1952), checking its CRC-32 and length
/// trailer, giving up beyond `max_out` bytes.
pub(crate) fn gzip_to_vec(input: &[u8], max_out: usize) -> Result<Vec<u8>, InflateError> {
    let body = &input[gzip_header(input)?..];
    let mut inf = Inflater::new(body);
    let out = to_vec(&mut inf, body.len(), max_out)?;
    let p = inf.consumed();
    let t = body.get(p..p + 8).ok_or(InflateError::Truncated)?;
    let crc = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
    let size = u32::from_le_bytes([t[4], t[5], t[6], t[7]]);
    if crc != crate::crc::crc32(&out) || size != out.len() as u32 {
        return Err(InflateError::BadChecksum);
    }
    Ok(out)
}

/// Decompress every member of a gzip stream (RFC 1952 allows several, as `cat a.gz b.gz`
/// and parallel compressors write), each member's CRC-32 and length checked; NUL padding
/// after the last member is allowed. Gives up beyond `max_out` bytes. The output is sized
/// from `hint` (the expected length, e.g. from a header) or else the last member's length
/// trailer, so it is allocated once in the common case.
pub(crate) fn gzip_members_to_vec(
    input: &[u8],
    max_out: usize,
    hint: Option<usize>,
) -> Result<Vec<u8>, InflateError> {
    let trailer = input
        .len()
        .checked_sub(4)
        .map(|p| u32::from_le_bytes([input[p], input[p + 1], input[p + 2], input[p + 3]]) as usize);
    let first = hint
        .or(trailer)
        .unwrap_or(input.len() * 4)
        .clamp(1024, max_out.max(1024));
    // The output is reserved from the memory budget as it grows.
    let mut res = specular_core::budget::reserve((first + OUT_SLACK) as u64)?;
    let mut out = vec![0u8; first + OUT_SLACK];
    let mut pos = 0;
    let mut at = 0;
    loop {
        let body = &input[at + gzip_header(&input[at..])?..];
        let mut inf = Inflater::new(body);
        let start = pos;
        loop {
            let limit = out.len() - OUT_SLACK;
            match inf.inflate(&mut out, &mut pos, limit)? {
                Progress::Done => break,
                Progress::OutputFull => {
                    let n = grown(out.len(), max_out).ok_or(InflateError::BadSymbol)?;
                    res.grow_to(n as u64)?;
                    out.resize(n, 0);
                }
            }
        }
        let p = inf.consumed();
        let t = body.get(p..p + 8).ok_or(InflateError::Truncated)?;
        let crc = u32::from_le_bytes([t[0], t[1], t[2], t[3]]);
        let size = u32::from_le_bytes([t[4], t[5], t[6], t[7]]);
        if crc != crate::crc::crc32(&out[start..pos]) || size != (pos - start) as u32 {
            return Err(InflateError::BadChecksum);
        }
        at = input.len() - body.len() + p + 8;
        let rest = &input[at..];
        if rest.len() < 2 || rest[..2] != [0x1f, 0x8b] {
            if rest.iter().all(|&b| b == 0) {
                out.truncate(pos);
                return Ok(out);
            }
            return Err(InflateError::BadHeader);
        }
    }
}

/// The first `n` bytes of a gzip stream's first member (fewer if it is shorter), for
/// reading a header without inflating the rest.
pub(crate) fn gzip_prefix(input: &[u8], n: usize) -> Result<Vec<u8>, InflateError> {
    let body = &input[gzip_header(input)?..];
    let limit = n.saturating_add(MAX_MATCH);
    let mut out = vec![0u8; limit.saturating_add(OUT_SLACK)];
    let mut pos = 0;
    Inflater::new(body).inflate(&mut out, &mut pos, limit)?;
    out.truncate(pos.min(n));
    Ok(out)
}

fn to_vec(
    inf: &mut Inflater<'_>,
    input_len: usize,
    max_out: usize,
) -> Result<Vec<u8>, InflateError> {
    let first = (input_len * 4).clamp(1024, max_out.max(1024)) + OUT_SLACK;
    // The output is reserved from the memory budget as it grows.
    let mut res = specular_core::budget::reserve(first as u64)?;
    let mut out = vec![0u8; first];
    let mut pos = 0;
    loop {
        let limit = out.len() - OUT_SLACK;
        match inf.inflate(&mut out, &mut pos, limit)? {
            Progress::Done => {
                out.truncate(pos);
                return Ok(out);
            }
            Progress::OutputFull => {
                let n = grown(out.len(), max_out).ok_or(InflateError::BadSymbol)?;
                res.grow_to(n as u64)?;
                out.resize(n, 0);
            }
        }
    }
}

/// The next size of an output buffer of `len` bytes (window plus [`OUT_SLACK`]) that filled
/// up, or `None` once the output is known to exceed `max_out`. Decoding stops with the window
/// full as soon as fewer than [`MAX_MATCH`] bytes are left in it, so a stream of exactly
/// `max_out` bytes needs a window of `max_out + MAX_MATCH`: it is grown to at most that, and
/// only a window that large filling up means more than `max_out` bytes.
fn grown(len: usize, max_out: usize) -> Option<usize> {
    let cap = max_out.saturating_add(MAX_MATCH);
    (len - OUT_SLACK < cap).then(|| len.saturating_mul(2).min(cap + OUT_SLACK))
}

/// Adler-32 of `data`, continuing from `adler` (1 for a new stream).
pub(crate) fn adler32(adler: u32, data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    // Largest n with 255 n (n + 1) / 2 + (n + 1) (MOD - 1) < 2^32, rounded down to 16.
    const NMAX: usize = 5552;
    let (mut a, mut b) = (adler & 0xffff, adler >> 16);
    for chunk in data.chunks(NMAX) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    b << 16 | a
}

#[cfg(test)]
mod tests {
    #[test]
    fn preset_dictionary() {
        // One fixed-Huffman block: a three-byte match three bytes back, into the dictionary.
        let out = super::raw_with_dictionary_to_vec(&[3, 34, 0], b"xyzabc", 100).unwrap();
        assert_eq!(out, b"abc");
        assert!(super::raw_to_vec(&[3, 34, 0], 100).is_err());
    }

    use super::*;

    fn inflate_all(z: &[u8]) -> Result<Vec<u8>, InflateError> {
        zlib_to_vec(z, 1 << 30)
    }

    /// Decompress through a small sliding window to exercise resumption.
    fn inflate_windowed(z: &[u8], chunk: usize) -> Result<Vec<u8>, InflateError> {
        let mut inf = Inflater::new_zlib(z)?;
        let mut buf = vec![0u8; WINDOW + chunk + MAX_MATCH + OUT_SLACK];
        let mut pos = 0;
        let mut all = Vec::new();
        loop {
            let limit = buf.len() - OUT_SLACK;
            let start = pos;
            let p = inf.inflate(&mut buf, &mut pos, limit)?;
            all.extend_from_slice(&buf[start..pos]);
            match p {
                Progress::Done => return Ok(all),
                Progress::OutputFull => {
                    let keep = pos.min(WINDOW);
                    buf.copy_within(pos - keep..pos, 0);
                    pos = keep;
                }
            }
        }
    }

    fn sample(n: usize, seed: u32) -> Vec<u8> {
        let mut s = seed;
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            match (s >> 28) % 4 {
                0 => v.push((s >> 8) as u8),
                1 => {
                    let r = ((s >> 10) % 40) as usize + 1;
                    let b = (s >> 3) as u8;
                    v.extend(std::iter::repeat_n(b, r));
                }
                2 if v.len() > 300 => {
                    let d = ((s >> 6) as usize % v.len().min(40000)).max(1);
                    let l = ((s >> 20) % 300) as usize;
                    for _ in 0..l {
                        let b = v[v.len() - d.min(v.len())];
                        v.push(b);
                    }
                }
                _ => v.extend_from_slice(b"pixels pixels "),
            }
        }
        v.truncate(n);
        v
    }

    #[test]
    fn round_trips_miniz() {
        for (i, n) in [0usize, 1, 100, 5000, 70_000, 400_000]
            .into_iter()
            .enumerate()
        {
            let data = sample(n, i as u32 * 7 + 1);
            for level in [0u8, 1, 6, 9] {
                let z = miniz_oxide::deflate::compress_to_vec_zlib(&data, level);
                assert_eq!(inflate_all(&z).unwrap(), data, "n={n} level={level}");
                assert_eq!(
                    inflate_windowed(&z, 1000).unwrap(),
                    data,
                    "n={n} level={level}"
                );
                // Adler trailer.
                let mut inf = Inflater::new_zlib(&z).unwrap();
                let mut out = vec![0u8; n + MAX_MATCH * 2 + OUT_SLACK];
                let mut pos = 0;
                let lim = out.len() - OUT_SLACK;
                assert_eq!(
                    inf.inflate(&mut out, &mut pos, lim).unwrap(),
                    Progress::Done
                );
                assert_eq!(inf.adler_trailer(), Some(adler32(1, &data)));
            }
        }
    }

    /// `max_out` equal to the exact length works for every length, including those just under
    /// the 1 KiB first buffer (once refused); far more than `max_out` is refused.
    #[test]
    fn exact_max_out_small() {
        for n in [
            1usize, 500, 766, 767, 796, 797, 1000, 1023, 1024, 1025, 3000,
        ] {
            let data = sample(n, n as u32);
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&data, 6);
            assert_eq!(zlib_to_vec(&z, n).unwrap(), data, "n={n}");
        }
        let data = sample(5000, 3);
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&data, 6);
        assert!(zlib_to_vec(&z, 2000).is_err());
    }

    /// A limit of exactly the stream's length is enough, however the output buffer grows
    /// towards it (an HDF4 deflate chunk of 27080 bytes from 1701 failed this), and one byte
    /// less is refused.
    #[test]
    fn exact_max_out() {
        for n in (1000..30_000).step_by(997).chain([27_080, 27_344, 6_804]) {
            let data = sample(n, n as u32);
            let z = miniz_oxide::deflate::compress_to_vec_zlib(&data, 9);
            assert_eq!(zlib_to_vec(&z, n).unwrap(), data, "n={n}");
            assert!(zlib_to_vec(&z, n - 1).is_err(), "n={n}");
            let g = gzip(&data, 0);
            assert_eq!(gzip_members_to_vec(&g, n, None).unwrap(), data, "n={n}");
        }
    }

    #[test]
    fn truncated_and_corrupt_never_panic() {
        let data = sample(50_000, 3);
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&data, 6);
        for cut in (0..z.len()).step_by(97) {
            let r = inflate_all(&z[..cut]);
            assert!(r.is_err() || cut + 4 >= z.len());
        }
        let mut s = 12345u32;
        for _ in 0..300 {
            let mut c = z.clone();
            for _ in 0..4 {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                let i = 2 + (s as usize >> 8) % (c.len() - 2);
                c[i] ^= 1 << (s >> 29);
            }
            let _ = inflate_all(&c);
            let _ = inflate_windowed(&c, 777);
        }
    }

    /// A gzip member around `data` (raw DEFLATE from miniz) with the header fields of `flags`.
    fn gzip(data: &[u8], flags: u8) -> Vec<u8> {
        let mut g = vec![0x1f, 0x8b, 8, flags, 1, 2, 3, 4, 0, 3];
        if flags & 4 != 0 {
            g.extend_from_slice(&[3, 0, b'a', b'b', b'c']);
        }
        if flags & 8 != 0 {
            g.extend_from_slice(b"name.bin\0");
        }
        if flags & 16 != 0 {
            g.extend_from_slice(b"a comment\0");
        }
        if flags & 2 != 0 {
            g.extend_from_slice(&[0xAB, 0xCD]);
        }
        g.extend(miniz_oxide::deflate::compress_to_vec(data, 6));
        g.extend_from_slice(&crate::crc::crc32(data).to_le_bytes());
        g.extend_from_slice(&(data.len() as u32).to_le_bytes());
        g
    }

    #[test]
    fn gzip_members() {
        let data = sample(20_000, 5);
        for flags in [0, 2, 4, 8, 16, 30] {
            let g = gzip(&data, flags);
            assert_eq!(gzip_to_vec(&g, 1 << 20).unwrap(), data, "flags {flags}");
            // A second member (or anything else) after the first is ignored.
            let mut two = g.clone();
            two.extend_from_slice(&g);
            assert_eq!(gzip_to_vec(&two, 1 << 20).unwrap(), data);
            for cut in 0..g.len() {
                assert!(gzip_to_vec(&g[..cut], 1 << 20).is_err(), "cut {cut}");
            }
        }
        let g = gzip(&data, 0);
        // Wrong CRC, wrong length, a reserved flag, another method.
        for (at, v) in [(g.len() - 8, 1u8), (g.len() - 4, 1), (3, 0x20), (2, 0x7)] {
            let mut c = g.clone();
            c[at] ^= v;
            assert!(gzip_to_vec(&c, 1 << 20).is_err(), "{at}");
        }
        // Over the output cap.
        assert!(gzip_to_vec(&g, 1000).is_err());
        assert_eq!(gzip_to_vec(&gzip(b"", 0), 10).unwrap(), b"");
    }

    #[test]
    fn adler() {
        assert_eq!(adler32(1, b"Wikipedia"), 0x11E60398);
    }
}

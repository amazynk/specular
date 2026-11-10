//! Zstandard (ZIP method 93, `.tar.zst`): own decoder, from RFC 8878.
//!
//! A frame is a header (window, optional content size and dictionary id, checksum flag), blocks
//! of at most 128 KiB each (raw, run-length or compressed) and an optional checksum (the low 32
//! bits of XXH64). A compressed block holds literals (raw, run-length, or Huffman-coded in one or
//! four streams, with a new or the previous table) and sequences (literal length, match length,
//! offset), coded by three interleaved FSE (tANS) decoders over one backward bitstream, with
//! predefined, run-length, new or repeated tables. Frames and skippable frames may be
//! concatenated. Frames needing a dictionary are refused; the decoder holds the whole output,
//! so any window size is accepted.

use super::check::xxh64;
use super::{CResult, CodecError, copy_match, room};

const MAGIC: u32 = 0xFD2F_B528;
const BLOCK_MAX: usize = 128 << 10;
const HUF_MAX_BITS: u32 = 11;

/// Whether `b` starts with a Zstandard frame (or a skippable frame).
pub(crate) fn detect(b: &[u8]) -> bool {
    match b.get(..4) {
        Some(m) => {
            let m = u32::from_le_bytes([m[0], m[1], m[2], m[3]]);
            m == MAGIC || m & 0xFFFF_FFF0 == 0x184D_2A50
        }
        None => false,
    }
}

const LL_DEFAULT: [i16; 36] = [
    4, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 2, 1, 1, 1, 1, 1,
    -1, -1, -1, -1,
];
const ML_DEFAULT: [i16; 53] = [
    1, 4, 3, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1, -1, -1,
];
const OF_DEFAULT: [i16; 29] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, -1, -1, -1, -1, -1,
];

/// Literal length codes 16–35: (base, extra bits); codes 0–15 are the length itself.
const LL_CODES: [(u32, u8); 20] = [
    (16, 1),
    (18, 1),
    (20, 1),
    (22, 1),
    (24, 2),
    (28, 2),
    (32, 3),
    (40, 3),
    (48, 4),
    (64, 6),
    (128, 7),
    (256, 8),
    (512, 9),
    (1024, 10),
    (2048, 11),
    (4096, 12),
    (8192, 13),
    (16384, 14),
    (32768, 15),
    (65536, 16),
];
/// Match length codes 32–52; codes 0–31 are the length minus 3.
const ML_CODES: [(u32, u8); 21] = [
    (35, 1),
    (37, 1),
    (39, 1),
    (41, 1),
    (43, 2),
    (47, 2),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 5),
    (131, 7),
    (259, 8),
    (515, 9),
    (1027, 10),
    (2051, 11),
    (4099, 12),
    (8195, 13),
    (16387, 14),
    (32771, 15),
    (65539, 16),
];

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |v, &x| (v << 8) | u64::from(x))
}

/// A backward bitstream: read from the end, most significant bit first, after the marker
/// (the highest set bit of the last byte).
struct Back<'a> {
    src: &'a [u8],
    /// Offset of the 8 bytes in `container` (0 once the start is reached).
    ptr: usize,
    container: u64,
    /// Bits of `container` read, from the top; past 64 means past the start of the stream.
    consumed: u32,
}

impl<'a> Back<'a> {
    fn new(src: &'a [u8]) -> CResult<Back<'a>> {
        let last = *src.last().ok_or(CodecError::Corrupt("empty bitstream"))?;
        if last == 0 {
            return Err(CodecError::Corrupt("bitstream without an end marker"));
        }
        let marker = last.leading_zeros() + 1;
        Ok(if src.len() >= 8 {
            let ptr = src.len() - 8;
            Back {
                src,
                ptr,
                container: le(&src[ptr..]),
                consumed: marker,
            }
        } else {
            // The bytes sit at the bottom of the container; its empty top counts as read.
            Back {
                src,
                ptr: 0,
                container: le(src),
                consumed: marker + 8 * (8 - src.len() as u32),
            }
        })
    }

    /// The next `n` bits (0–56; zeros past the start of the stream), not consumed.
    #[inline]
    fn peek(&self, n: u32) -> u64 {
        if n == 0 {
            return 0;
        }
        self.container.checked_shl(self.consumed).unwrap_or(0) >> (64 - n)
    }

    #[inline]
    fn read(&mut self, n: u32) -> u64 {
        let v = self.peek(n);
        self.consumed += n;
        v
    }

    /// Moves the window so at least 56 bits are buffered (or all that are left).
    #[inline]
    fn reload(&mut self) {
        if self.consumed > 64 || self.ptr == 0 {
            return;
        }
        let back = ((self.consumed >> 3) as usize).min(self.ptr);
        self.ptr -= back;
        self.consumed -= back as u32 * 8;
        self.container = le(&self.src[self.ptr..self.ptr + 8]);
    }

    /// Every bit read, none past the start.
    fn finished(&self) -> bool {
        self.ptr == 0 && self.consumed == 64
    }

    /// Bits read past the start of the stream.
    fn overflowed(&self) -> bool {
        self.ptr == 0 && self.consumed > 64
    }
}

/// An FSE decoding table: per state, the symbol, the bits to read and the base of the next
/// state.
#[derive(Clone)]
struct Fse {
    log: u32,
    sym: Vec<u8>,
    bits: Vec<u8>,
    base: Vec<u16>,
}

impl Fse {
    fn rle(sym: u8) -> Fse {
        Fse {
            log: 0,
            sym: vec![sym],
            bits: vec![0],
            base: vec![0],
        }
    }

    /// The table for normalised counts `norm` (−1: "less than one", given one state at the top).
    fn new(norm: &[i16], log: u32) -> CResult<Fse> {
        let size = 1usize << log;
        let mut sym = vec![0u8; size];
        let mut next = vec![0u32; norm.len()];
        let mut high = size;
        for (s, &c) in norm.iter().enumerate() {
            if c == -1 {
                high = high
                    .checked_sub(1)
                    .ok_or(CodecError::Corrupt("FSE counts overflow the table"))?;
                sym[high] = s as u8;
                next[s] = 1;
            }
        }
        let step = (size >> 1) + (size >> 3) + 3;
        let mask = size - 1;
        let mut pos = 0usize;
        for (s, &c) in norm.iter().enumerate() {
            if c <= 0 {
                continue;
            }
            next[s] = c as u32;
            if high == 0 {
                return Err(CodecError::Corrupt("FSE counts overflow the table"));
            }
            for _ in 0..c {
                sym[pos] = s as u8;
                pos = (pos + step) & mask;
                while pos >= high {
                    pos = (pos + step) & mask;
                }
            }
        }
        if pos != 0 {
            return Err(CodecError::Corrupt("FSE counts do not fill the table"));
        }
        let mut bits = vec![0u8; size];
        let mut base = vec![0u16; size];
        for u in 0..size {
            let s = usize::from(sym[u]);
            let n = next[s];
            next[s] += 1;
            let b = log - (31 - n.leading_zeros());
            bits[u] = b as u8;
            base[u] = ((n << b) as usize - size) as u16;
        }
        Ok(Fse {
            log,
            sym,
            bits,
            base,
        })
    }

    /// Reads a table description (forward, least significant bit first) from `src`: the table
    /// and the bytes it took.
    fn read(src: &[u8], max_log: u32, max_sym: usize) -> CResult<(Fse, usize)> {
        let mut bit = 0usize;
        let mut get = |n: u32, consume: u32| -> CResult<u32> {
            let at = bit / 8;
            let w = src.get(at..).ok_or(CodecError::Truncated)?;
            let v = (le(&w[..w.len().min(8)]) >> (bit % 8)) as u32 & ((1u32 << n) - 1);
            bit += consume as usize;
            if bit > src.len() * 8 {
                return Err(CodecError::Truncated);
            }
            Ok(v)
        };
        let log = get(4, 4)? + 5;
        if log > max_log {
            return Err(CodecError::Corrupt("FSE accuracy too high"));
        }
        let mut remaining = (1i32 << log) + 1;
        let mut threshold = 1i32 << log;
        let mut nbits = log + 1;
        let mut norm = Vec::with_capacity(max_sym + 1);
        while remaining > 1 {
            if norm.len() > max_sym {
                return Err(CodecError::Corrupt("too many FSE symbols"));
            }
            let max = 2 * threshold - 1 - remaining;
            let low = get(nbits - 1, 0)? as i32;
            let mut count = if low < max {
                get(nbits - 1, nbits - 1)? as i32
            } else {
                let v = get(nbits, nbits)? as i32;
                if v >= threshold { v - max } else { v }
            };
            count -= 1;
            remaining -= count.abs();
            norm.push(count as i16);
            if count == 0 {
                loop {
                    let r = get(2, 2)?;
                    norm.resize(norm.len() + r as usize, 0);
                    if norm.len() > max_sym + 1 {
                        return Err(CodecError::Corrupt("too many FSE symbols"));
                    }
                    if r != 3 {
                        break;
                    }
                }
            }
            while remaining < threshold && threshold > 1 {
                nbits -= 1;
                threshold >>= 1;
            }
        }
        if remaining != 1 || norm.len() > max_sym + 1 {
            return Err(CodecError::Corrupt("bad FSE counts"));
        }
        Ok((Fse::new(&norm, log)?, bit.div_ceil(8)))
    }
}

/// An FSE decoder's state.
struct State(usize);

impl State {
    fn init(t: &Fse, b: &mut Back) -> State {
        State(b.read(t.log) as usize)
    }

    #[inline]
    fn sym(&self, t: &Fse) -> u8 {
        t.sym[self.0]
    }

    #[inline]
    fn update(&mut self, t: &Fse, b: &mut Back) {
        self.0 = usize::from(t.base[self.0]) + b.read(u32::from(t.bits[self.0])) as usize;
    }
}

/// A Huffman decoding table for literals: `symbol << 8 | length`, by the next `bits` bits.
struct Huffman {
    bits: u32,
    table: Vec<u16>,
}

impl Huffman {
    /// Reads a Huffman tree description: the table and the bytes it took.
    fn read(src: &[u8]) -> CResult<(Huffman, usize)> {
        let head = usize::from(*src.first().ok_or(CodecError::Truncated)?);
        let mut weights = Vec::with_capacity(256);
        let used = if head < 128 {
            // FSE-compressed weights, two interleaved states.
            let data = src.get(1..1 + head).ok_or(CodecError::Truncated)?;
            let (fse, n) = Fse::read(data, 6, 255)?;
            let mut b = Back::new(data.get(n..).ok_or(CodecError::Truncated)?)?;
            let mut s1 = State::init(&fse, &mut b);
            let mut s2 = State::init(&fse, &mut b);
            loop {
                if weights.len() > 253 {
                    return Err(CodecError::Corrupt("too many Huffman weights"));
                }
                b.reload();
                weights.push(s1.sym(&fse));
                s1.update(&fse, &mut b);
                if b.overflowed() {
                    weights.push(s2.sym(&fse));
                    break;
                }
                b.reload();
                weights.push(s2.sym(&fse));
                s2.update(&fse, &mut b);
                if b.overflowed() {
                    weights.push(s1.sym(&fse));
                    break;
                }
            }
            1 + head
        } else {
            let n = head - 127;
            let data = src.get(1..1 + n.div_ceil(2)).ok_or(CodecError::Truncated)?;
            for i in 0..n {
                let b = data[i / 2];
                weights.push(if i % 2 == 0 { b >> 4 } else { b & 15 });
            }
            1 + n.div_ceil(2)
        };
        // The last symbol's weight makes the total a power of two.
        let mut total = 0u32;
        for &w in &weights {
            if u32::from(w) > HUF_MAX_BITS {
                return Err(CodecError::Corrupt("Huffman weight too large"));
            }
            if w > 0 {
                total += 1 << (w - 1);
            }
        }
        if total == 0 {
            return Err(CodecError::Corrupt("no Huffman weights"));
        }
        let bits = 32 - total.leading_zeros();
        let left = (1u32 << bits) - total;
        if bits > HUF_MAX_BITS || !left.is_power_of_two() {
            return Err(CodecError::Corrupt("bad Huffman weights"));
        }
        weights.push((left.trailing_zeros() + 1) as u8);
        let mut table = vec![0u16; 1 << bits];
        let mut at = 0usize;
        for w in 1..=bits as u8 {
            for (s, _) in weights.iter().enumerate().filter(|&(_, &x)| x == w) {
                let n = 1usize << (w - 1);
                let len = bits + 1 - u32::from(w);
                table[at..at + n].fill(((s as u16) << 8) | len as u16);
                at += n;
            }
        }
        Ok((Huffman { bits, table }, used))
    }

    /// Decodes the stream `src` into all of `dst`, which it must fill exactly.
    fn stream(&self, src: &[u8], dst: &mut [u8]) -> CResult<()> {
        let mut b = Back::new(src)?;
        for d in dst.iter_mut() {
            if b.consumed > 64 - HUF_MAX_BITS {
                b.reload();
            }
            let e = self.table[b.peek(self.bits) as usize];
            b.consumed += u32::from(e & 0xFF);
            *d = (e >> 8) as u8;
        }
        if !b.finished() {
            return Err(CodecError::Corrupt("Huffman stream size mismatch"));
        }
        Ok(())
    }
}

/// What carries over from block to block in a frame.
struct Frame {
    huffman: Option<Huffman>,
    ll: Option<Fse>,
    of: Option<Fse>,
    ml: Option<Fse>,
    rep: [usize; 3],
    literals: Vec<u8>,
}

/// Decodes the literals section at the start of `src` into `f.literals`: the bytes it took.
fn literals(src: &[u8], f: &mut Frame) -> CResult<usize> {
    let b0 = *src.first().ok_or(CodecError::Truncated)?;
    let kind = b0 & 3;
    let format = (b0 >> 2) & 3;
    f.literals.clear();
    if kind < 2 {
        let (size, head) = match format {
            0 | 2 => (usize::from(b0 >> 3), 1),
            1 => (
                le(src.get(..2).ok_or(CodecError::Truncated)?) as usize >> 4,
                2,
            ),
            _ => (
                le(src.get(..3).ok_or(CodecError::Truncated)?) as usize >> 4,
                3,
            ),
        };
        if size > BLOCK_MAX {
            return Err(CodecError::Corrupt("literals larger than a block"));
        }
        if kind == 0 {
            let raw = src.get(head..head + size).ok_or(CodecError::Truncated)?;
            f.literals.extend_from_slice(raw);
            return Ok(head + size);
        }
        let byte = *src.get(head).ok_or(CodecError::Truncated)?;
        f.literals.resize(size, byte);
        return Ok(head + 1);
    }
    let (head, regen, packed, streams) = match format {
        0 | 1 => {
            let v = le(src.get(..3).ok_or(CodecError::Truncated)?);
            (
                3,
                (v >> 4) & 0x3FF,
                (v >> 14) & 0x3FF,
                if format == 0 { 1 } else { 4 },
            )
        }
        2 => {
            let v = le(src.get(..4).ok_or(CodecError::Truncated)?);
            (4, (v >> 4) & 0x3FFF, (v >> 18) & 0x3FFF, 4)
        }
        _ => {
            let v = le(src.get(..5).ok_or(CodecError::Truncated)?);
            (5, (v >> 4) & 0x3FFFF, (v >> 22) & 0x3FFFF, 4)
        }
    };
    let (regen, packed) = (regen as usize, packed as usize);
    if regen > BLOCK_MAX {
        return Err(CodecError::Corrupt("literals larger than a block"));
    }
    let mut data = src.get(head..head + packed).ok_or(CodecError::Truncated)?;
    if kind == 2 {
        let (h, n) = Huffman::read(data)?;
        f.huffman = Some(h);
        data = &data[n..];
    }
    let huf = f.huffman.as_ref().ok_or(CodecError::Corrupt(
        "repeated Huffman table without a first one",
    ))?;
    f.literals.resize(regen, 0);
    if streams == 1 {
        huf.stream(data, &mut f.literals)?;
    } else {
        let jump = data.get(..6).ok_or(CodecError::Truncated)?;
        let s1 = le(&jump[..2]) as usize;
        let s2 = le(&jump[2..4]) as usize;
        let s3 = le(&jump[4..]) as usize;
        let rest = &data[6..];
        let s4 = rest
            .len()
            .checked_sub(s1 + s2 + s3)
            .ok_or(CodecError::Corrupt("bad literal stream sizes"))?;
        let part = regen.div_ceil(4);
        if regen < 3 * part {
            return Err(CodecError::Corrupt("too few literals for four streams"));
        }
        let (d1, d) = f.literals.split_at_mut(part);
        let (d2, d) = d.split_at_mut(part);
        let (d3, d4) = d.split_at_mut(part);
        let (a, r) = rest.split_at(s1);
        let (bb, r) = r.split_at(s2);
        let (c, dd) = r.split_at(s3);
        debug_assert_eq!(dd.len(), s4);
        huf.stream(a, d1)?;
        huf.stream(bb, d2)?;
        huf.stream(c, d3)?;
        huf.stream(dd, d4)?;
    }
    Ok(head + packed)
}

/// One of the three sequence tables, by its compression mode.
fn seq_table(
    mode: u8,
    src: &[u8],
    at: &mut usize,
    slot: &mut Option<Fse>,
    default: &[i16],
    default_log: u32,
    max_log: u32,
) -> CResult<()> {
    match mode {
        0 => *slot = Some(Fse::new(default, default_log)?),
        1 => {
            let s = *src.get(*at).ok_or(CodecError::Truncated)?;
            if usize::from(s) >= default.len() {
                return Err(CodecError::Corrupt("bad run-length code"));
            }
            *at += 1;
            *slot = Some(Fse::rle(s));
        }
        2 => {
            let (t, n) = Fse::read(
                src.get(*at..).ok_or(CodecError::Truncated)?,
                max_log,
                default.len() - 1,
            )?;
            *at += n;
            *slot = Some(t);
        }
        _ => {
            if slot.is_none() {
                return Err(CodecError::Corrupt("repeated table without a first one"));
            }
        }
    }
    Ok(())
}

/// Decodes a compressed block `src`, appending to `out` (the frame's output from `frame`).
fn compressed(
    src: &[u8],
    f: &mut Frame,
    out: &mut Vec<u8>,
    frame: usize,
    limit: usize,
) -> CResult<()> {
    let mut at = literals(src, f)?;
    let b0 = usize::from(*src.get(at).ok_or(CodecError::Truncated)?);
    let count = match b0 {
        0 => 0,
        1..128 => {
            at += 1;
            b0
        }
        128..255 => {
            let b1 = usize::from(*src.get(at + 1).ok_or(CodecError::Truncated)?);
            at += 2;
            ((b0 - 128) << 8) + b1
        }
        _ => {
            let w = src.get(at + 1..at + 3).ok_or(CodecError::Truncated)?;
            at += 3;
            le(w) as usize + 0x7F00
        }
    };
    let start = out.len();
    if count == 0 {
        if at + 1 != src.len() {
            return Err(CodecError::Corrupt("data after an empty sequence section"));
        }
        room(out, f.literals.len(), limit)?;
        out.extend_from_slice(&f.literals);
        return Ok(());
    }
    let modes = *src.get(at).ok_or(CodecError::Truncated)?;
    at += 1;
    if modes & 3 != 0 {
        return Err(CodecError::Corrupt("reserved sequence mode bits"));
    }
    seq_table(modes >> 6, src, &mut at, &mut f.ll, &LL_DEFAULT, 6, 9)?;
    seq_table((modes >> 4) & 3, src, &mut at, &mut f.of, &OF_DEFAULT, 5, 8)?;
    seq_table((modes >> 2) & 3, src, &mut at, &mut f.ml, &ML_DEFAULT, 6, 9)?;
    let (Some(ll), Some(of), Some(ml)) = (&f.ll, &f.of, &f.ml) else {
        return Err(CodecError::Corrupt("missing sequence table"));
    };
    let mut b = Back::new(src.get(at..).ok_or(CodecError::Truncated)?)?;
    let mut sll = State::init(ll, &mut b);
    let mut sof = State::init(of, &mut b);
    let mut sml = State::init(ml, &mut b);
    let lits = &f.literals;
    let mut lit = 0usize;
    let rep = &mut f.rep;
    for i in 0..count {
        let llc = sll.sym(ll);
        let ofc = sof.sym(of);
        let mlc = sml.sym(ml);
        if ofc > 31 || llc > 35 || mlc > 52 {
            return Err(CodecError::Corrupt("bad sequence code"));
        }
        b.reload();
        let mut offset = ((1u64 << ofc) + b.read(u32::from(ofc))) as usize;
        b.reload();
        let mlen = if mlc < 32 {
            usize::from(mlc) + 3
        } else {
            let (base, n) = ML_CODES[usize::from(mlc - 32)];
            (base as u64 + b.read(u32::from(n))) as usize
        };
        let llen = if llc < 16 {
            usize::from(llc)
        } else {
            let (base, n) = LL_CODES[usize::from(llc - 16)];
            (base as u64 + b.read(u32::from(n))) as usize
        };
        // Offsets 1–3 are repeats (shifted by one after an empty literal run).
        if offset > 3 {
            offset -= 3;
            *rep = [offset, rep[0], rep[1]];
        } else {
            let idx = offset + usize::from(llen == 0);
            offset = match idx {
                1 => rep[0],
                2 => rep[1],
                3 => rep[2],
                _ => rep[0].wrapping_sub(1),
            };
            if offset == 0 {
                return Err(CodecError::Corrupt("zero offset"));
            }
            match idx {
                1 => {}
                2 => *rep = [offset, rep[0], rep[2]],
                _ => *rep = [offset, rep[0], rep[1]],
            }
        }
        // Copy the literals, then the match.
        let run = lits
            .get(lit..lit + llen)
            .ok_or(CodecError::Corrupt("sequence past the literals"))?;
        room(out, llen + mlen, limit)?;
        out.extend_from_slice(run);
        lit += llen;
        if offset > out.len() - frame {
            return Err(CodecError::Corrupt("offset before the frame"));
        }
        copy_match(out, offset, mlen);
        if out.len() - start > BLOCK_MAX {
            return Err(CodecError::Corrupt("block larger than 128 KiB"));
        }
        if i + 1 < count {
            b.reload();
            sll.update(ll, &mut b);
            sml.update(ml, &mut b);
            b.reload();
            sof.update(of, &mut b);
        }
    }
    b.reload();
    if !b.finished() {
        return Err(CodecError::Corrupt("sequence bitstream size mismatch"));
    }
    let rest = &lits[lit..];
    room(out, rest.len(), limit)?;
    out.extend_from_slice(rest);
    if out.len() - start > BLOCK_MAX {
        return Err(CodecError::Corrupt("block larger than 128 KiB"));
    }
    Ok(())
}

/// One frame at the start of `src`, appended to `out`: the bytes it took.
fn frame(src: &[u8], out: &mut Vec<u8>, limit: usize) -> CResult<usize> {
    let fhd = *src.get(4).ok_or(CodecError::Truncated)?;
    let single = fhd & 0x20 != 0;
    if fhd & 0x08 != 0 {
        return Err(CodecError::Corrupt("reserved frame header bit"));
    }
    let mut at = 5 + usize::from(!single);
    let dict_len = [0, 1, 2, 4][usize::from(fhd & 3)];
    let dict = le(src.get(at..at + dict_len).ok_or(CodecError::Truncated)?);
    at += dict_len;
    if dict != 0 {
        return Err(CodecError::Unsupported("frames needing a dictionary"));
    }
    let fcs_len = match fhd >> 6 {
        0 => usize::from(single),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let fcs = (fcs_len > 0)
        .then(|| src.get(at..at + fcs_len).map(le))
        .map(|v| v.ok_or(CodecError::Truncated))
        .transpose()?
        .map(|v| if fcs_len == 2 { v + 256 } else { v });
    at += fcs_len;
    let base = out.len();
    if let Some(n) = fcs {
        room(
            out,
            usize::try_from(n).map_err(|_| CodecError::Limit)?,
            limit,
        )?;
        // A size the data cannot back is not reserved in full.
        out.reserve((n as usize).min(src.len().saturating_mul(64)).min(64 << 20));
    }
    let mut f = Frame {
        huffman: None,
        ll: None,
        of: None,
        ml: None,
        rep: [1, 4, 8],
        literals: Vec::new(),
    };
    loop {
        let h = le(src.get(at..at + 3).ok_or(CodecError::Truncated)?) as usize;
        at += 3;
        let last = h & 1 != 0;
        let size = h >> 3;
        match (h >> 1) & 3 {
            0 => {
                let raw = src.get(at..at + size).ok_or(CodecError::Truncated)?;
                if size > BLOCK_MAX {
                    return Err(CodecError::Corrupt("block larger than 128 KiB"));
                }
                room(out, size, limit)?;
                out.extend_from_slice(raw);
                at += size;
            }
            1 => {
                let byte = *src.get(at).ok_or(CodecError::Truncated)?;
                if size > BLOCK_MAX {
                    return Err(CodecError::Corrupt("block larger than 128 KiB"));
                }
                room(out, size, limit)?;
                out.resize(out.len() + size, byte);
                at += 1;
            }
            2 => {
                if size > BLOCK_MAX {
                    return Err(CodecError::Corrupt("block larger than 128 KiB"));
                }
                let body = src.get(at..at + size).ok_or(CodecError::Truncated)?;
                compressed(body, &mut f, out, base, limit)?;
                at += size;
            }
            _ => return Err(CodecError::Corrupt("reserved block type")),
        }
        if last {
            break;
        }
    }
    if fcs.is_some_and(|n| n != (out.len() - base) as u64) {
        return Err(CodecError::Corrupt("frame size differs from its header"));
    }
    if fhd & 0x04 != 0 {
        let want = le(src.get(at..at + 4).ok_or(CodecError::Truncated)?) as u32;
        if xxh64(&out[base..]) as u32 != want {
            return Err(CodecError::Checksum);
        }
        at += 4;
    }
    Ok(at)
}

/// Decodes the Zstandard data `data` (frames and skippable frames), appending to `out` without
/// letting it pass `limit` bytes. Bytes after the last frame that are not a frame are ignored.
pub(crate) fn decode(data: &[u8], out: &mut Vec<u8>, limit: usize) -> CResult<()> {
    if !detect(data) {
        return Err(CodecError::Corrupt("not a Zstandard stream"));
    }
    let mut at = 0usize;
    while let Some(m) = data.get(at..at + 4) {
        let m = le(m) as u32;
        if m == MAGIC {
            at += frame(&data[at..], out, limit)?;
        } else if m & 0xFFFF_FFF0 == 0x184D_2A50 {
            let n = le(data.get(at + 4..at + 8).ok_or(CodecError::Truncated)?) as usize;
            at = at
                .checked_add(8 + n)
                .filter(|&e| e <= data.len())
                .ok_or(CodecError::Truncated)?;
        } else {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tables_fill() {
        for (norm, log) in [
            (&LL_DEFAULT[..], 6),
            (&ML_DEFAULT[..], 6),
            (&OF_DEFAULT[..], 5),
        ] {
            let t = Fse::new(norm, log).unwrap();
            assert_eq!(t.sym.len(), 1 << log);
        }
        // The predefined literal-length table, state 0 and 1 (RFC 8878, appendix A).
        let t = Fse::new(&LL_DEFAULT, 6).unwrap();
        assert_eq!((t.sym[0], t.bits[0], t.base[0]), (0, 4, 0));
        assert_eq!((t.sym[1], t.bits[1], t.base[1]), (0, 4, 16));
        // The "less than one" symbols fill the top states, from symbol 32 down.
        assert_eq!((t.sym[63], t.bits[63], t.base[63]), (32, 6, 0));
        assert_eq!(t.sym[60], 35);
    }

    #[test]
    fn backward_bits() {
        // Bits from the end: marker in 0b0001_0110 is bit 4; then 0110, then the byte before.
        let b = [0b1010_0101, 0b0001_0110];
        let mut r = Back::new(&b).unwrap();
        assert_eq!(r.read(4), 0b0110);
        assert_eq!(r.read(8), 0b1010_0101);
        assert!(r.finished());
        r.read(1);
        assert!(r.overflowed());
        assert!(Back::new(&[1, 0]).is_err());
    }

    #[test]
    fn hand_made_frames() {
        // Single segment, size 5, one raw last block "hello", then an RLE frame of 4 'z'.
        let mut v = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, 5];
        v.extend_from_slice(&[(5 << 3) | 1, 0, 0]);
        v.extend_from_slice(b"hello");
        v.extend_from_slice(&[0x50, 0x2A, 0x4D, 0x18, 2, 0, 0, 0, 9, 9]);
        v.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD, 0x20, 4, (4 << 3) | 3, 0, 0, b'z']);
        let mut out = Vec::new();
        decode(&v, &mut out, 100).unwrap();
        assert_eq!(out, b"hellozzzz");
        out.clear();
        assert_eq!(decode(&v, &mut out, 8), Err(CodecError::Limit));
        for cut in 1..v.len() {
            out.clear();
            let _ = decode(&v[..cut], &mut out, 100);
        }
    }
}

//! bzip2 (ZIP method 12, `.tar.bz2`): own decoder, from the format's published descriptions.
//!
//! A stream is "BZh" and a level digit, then blocks, each starting with a 48-bit magic number at
//! any bit position and decoded on its own: Huffman codes (2–6 tables, switched every 50
//! symbols), the MTF/run-length stage (RUNA/RUNB), the inverse Burrows–Wheeler transform, and
//! the initial run-length stage (four equal bytes and a count). Each block carries the CRC of
//! its output and the stream a combination of them; both are checked. Concatenated streams (as
//! pbzip2 writes them) are read one after the other. Randomised blocks (written only by bzip2
//! 0.9.0) are refused.
//!
//! **Parallel blocks.** Blocks need nothing from each other, so with more than one thread the
//! input is scanned for the block magic (at every bit offset), and the blocks are decoded a batch
//! at a time on several threads, then joined in order. The magic can occur by chance inside
//! compressed data: such a false start is decoded too (and fails, or is simply not used), as
//! only the start where the previous block ended is taken.
//!
//! **Memory.** Each block reserves what it holds from the caller's memory budget before or
//! while it is decoded (its BWT vector up front, its output as runs expand it), on whichever
//! thread decodes it; the reservation goes with the block until it is appended. So a batch in
//! flight is counted, not only the output joined so far.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use parking_lot::Mutex;

use super::check::Crc32Be;
use specular_core::BudgetHandle;

use super::{CResult, CodecError, Threads, room};

/// The budget's refusal, as a codec error.
fn memory(e: specular_core::budget::NeedsMemory) -> CodecError {
    CodecError::Memory(e.to_string())
}

const BLOCK_MAGIC: u64 = 0x3141_5926_5359;
const END_MAGIC: u64 = 0x1772_4538_5090;
const MAX_GROUPS: usize = 6;
const MAX_SELECTORS: usize = 18002;
const MAX_CODE_LEN: u32 = 20;
const FAST_BITS: u32 = 10;
const GROUP_SIZE: usize = 50;

/// Whether `b` starts like a bzip2 stream ("BZh1"–"BZh9", then a block or the end marker).
pub(crate) fn detect(b: &[u8]) -> bool {
    b.len() >= 10
        && b.starts_with(b"BZh")
        && (b'1'..=b'9').contains(&b[3])
        && (b[4..10] == [0x31, 0x41, 0x59, 0x26, 0x53, 0x59]
            || b[4..10] == [0x17, 0x72, 0x45, 0x38, 0x50, 0x90])
}

/// Most-significant-bit-first reader with a 64-bit buffer (valid bits at the top).
struct Msb<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    n: u32,
}

impl<'a> Msb<'a> {
    fn at(data: &'a [u8], bit: usize) -> Msb<'a> {
        let mut r = Msb {
            data,
            pos: bit / 8,
            buf: 0,
            n: 0,
        };
        r.refill();
        r.skip((bit % 8) as u32);
        r
    }

    #[inline]
    fn refill(&mut self) {
        if let Some(w) = self.data.get(self.pos..self.pos + 8) {
            let w = u64::from_be_bytes(w.try_into().unwrap_or([0; 8]));
            self.buf |= w >> self.n;
            self.pos += (63 - self.n as usize) >> 3;
            self.n |= 56;
        } else {
            while self.n <= 56 {
                let b = self.data.get(self.pos).copied().unwrap_or(0);
                self.buf |= u64::from(b) << (56 - self.n);
                self.pos += 1;
                self.n += 8;
            }
        }
    }

    /// The next `k` bits (1–32) without consuming them; needs `k` bits buffered.
    #[inline]
    fn peek(&self, k: u32) -> u32 {
        (self.buf >> (64 - k)) as u32
    }

    #[inline]
    fn skip(&mut self, k: u32) {
        self.buf <<= k;
        self.n -= k;
    }

    /// The next `k` bits (1–32).
    #[inline]
    fn read(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.refill();
        }
        let v = self.peek(k);
        self.skip(k);
        v
    }

    fn bit(&mut self) -> bool {
        self.read(1) == 1
    }

    fn bitpos(&self) -> usize {
        self.pos * 8 - self.n as usize
    }

    fn overrun(&self) -> bool {
        self.bitpos() > self.data.len() * 8
    }
}

/// One Huffman table of a block.
struct Table {
    /// `symbol << 5 | length`, indexed by the next `FAST_BITS` bits; 0 for longer codes.
    fast: Vec<u16>,
    counts: [u16; MAX_CODE_LEN as usize + 1],
    sorted: Vec<u16>,
}

impl Table {
    fn new(lengths: &[u8]) -> CResult<Table> {
        let mut counts = [0u16; MAX_CODE_LEN as usize + 1];
        for &l in lengths {
            counts[usize::from(l)] += 1;
        }
        let mut left: i64 = 1;
        for &c in &counts[1..] {
            left = left * 2 - i64::from(c);
            if left < 0 {
                return Err(CodecError::Corrupt("over-subscribed Huffman code"));
            }
        }
        let mut offs = [0usize; MAX_CODE_LEN as usize + 2];
        for l in 1..=MAX_CODE_LEN as usize {
            offs[l + 1] = offs[l] + usize::from(counts[l]);
        }
        let mut sorted = vec![0u16; lengths.len()];
        let mut fast = vec![0u16; 1 << FAST_BITS];
        let mut code = 0u32;
        for l in 1..=MAX_CODE_LEN {
            for (sym, &len) in lengths.iter().enumerate() {
                if u32::from(len) != l {
                    continue;
                }
                sorted[offs[l as usize]] = sym as u16;
                offs[l as usize] += 1;
                if l <= FAST_BITS {
                    let lo = (code << (FAST_BITS - l)) as usize;
                    let hi = ((code + 1) << (FAST_BITS - l)) as usize;
                    fast[lo..hi].fill(((sym as u16) << 5) | l as u16);
                }
                code += 1;
            }
            code <<= 1;
        }
        Ok(Table {
            fast,
            counts,
            sorted,
        })
    }

    /// The next symbol; the reader must hold at least `MAX_CODE_LEN` bits.
    #[inline]
    fn decode(&self, r: &mut Msb) -> CResult<u16> {
        let e = self.fast[r.peek(FAST_BITS) as usize];
        if e != 0 {
            r.skip(u32::from(e & 31));
            return Ok(e >> 5);
        }
        let (mut code, mut first, mut index) = (0i64, 0i64, 0i64);
        for l in 1..=MAX_CODE_LEN as usize {
            code |= i64::from(r.peek(1));
            r.skip(1);
            let count = i64::from(self.counts[l]);
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

/// A decoded block.
struct Block {
    out: Vec<u8>,
    /// The memory budget's share of `out` (dropped once it is appended).
    _held: specular_core::Reservation,
    crc: u32,
    /// Bit position just after the block.
    end: usize,
}

/// Decodes the block whose magic starts at bit `bit`, for a stream of level `level` (block size
/// `level` × 100 000), producing at most `limit` bytes, reserving what it holds from `budget`.
fn decode_block(
    data: &[u8],
    bit: usize,
    level: usize,
    limit: usize,
    budget: &BudgetHandle,
) -> CResult<Block> {
    let mut r = Msb::at(data, bit);
    r.refill();
    let magic = (u64::from(r.read(24)) << 24) | u64::from(r.read(24));
    if magic != BLOCK_MAGIC {
        return Err(CodecError::Corrupt("bad block magic"));
    }
    let stored_crc = r.read(32);
    if r.bit() {
        return Err(CodecError::Unsupported("randomised blocks"));
    }
    let orig_ptr = r.read(24) as usize;
    // Symbols in use: a 16-bit map of 16-byte ranges, then a 16-bit map per range.
    let ranges = r.read(16);
    let mut seq = [0u8; 256];
    let mut in_use = 0usize;
    for i in 0..16 {
        if ranges & (0x8000 >> i) != 0 {
            let m = r.read(16);
            for j in 0..16 {
                if m & (0x8000 >> j) != 0 {
                    seq[in_use] = (i * 16 + j) as u8;
                    in_use += 1;
                }
            }
        }
    }
    if in_use == 0 {
        return Err(CodecError::Corrupt("no symbols in use"));
    }
    let alpha = in_use + 2;
    let groups = r.read(3) as usize;
    if !(2..=MAX_GROUPS).contains(&groups) {
        return Err(CodecError::Corrupt("bad number of Huffman tables"));
    }
    let selectors_n = r.read(15) as usize;
    if selectors_n == 0 {
        return Err(CodecError::Corrupt("no selectors"));
    }
    let mut mtf_groups: [u8; MAX_GROUPS] = [0, 1, 2, 3, 4, 5];
    let mut selectors = Vec::with_capacity(selectors_n.min(MAX_SELECTORS));
    for _ in 0..selectors_n {
        let mut j = 0usize;
        while r.bit() {
            j += 1;
            if j >= groups {
                return Err(CodecError::Corrupt("bad selector"));
            }
        }
        if r.overrun() {
            return Err(CodecError::Truncated);
        }
        let g = mtf_groups[j];
        mtf_groups.copy_within(0..j, 1);
        mtf_groups[0] = g;
        // Selectors past the most a block can use are read and ignored (as bzip2 1.0.8 does).
        if selectors.len() < MAX_SELECTORS {
            selectors.push(g);
        }
    }
    let mut tables = Vec::with_capacity(groups);
    let mut lens = [0u8; 258];
    for _ in 0..groups {
        let mut cur = r.read(5);
        for l in lens.iter_mut().take(alpha) {
            loop {
                if !(1..=MAX_CODE_LEN).contains(&cur) {
                    return Err(CodecError::Corrupt("bad code length"));
                }
                if !r.bit() {
                    break;
                }
                if r.bit() {
                    cur -= 1;
                } else {
                    cur += 1;
                }
            }
            *l = cur as u8;
        }
        if r.overrun() {
            return Err(CodecError::Truncated);
        }
        tables.push(Table::new(&lens[..alpha])?);
    }

    // Huffman + MTF + RUNA/RUNB: the BWT's last column, in the low byte of `tt`.
    let max = level * 100_000;
    let tt_cap = max.min(data.len().saturating_mul(64));
    let mut held = budget.reserve(tt_cap as u64 * 4).map_err(memory)?;
    let mut tt: Vec<u32> = Vec::with_capacity(tt_cap);
    let mut counts = [0u32; 256];
    let mut mtf: [u8; 256] = [0; 256];
    for (i, m) in mtf.iter_mut().enumerate().take(in_use) {
        *m = i as u8;
    }
    let eob = (alpha - 1) as u16;
    let mut run = 0usize;
    let mut run_bit = 0u32;
    let mut group = 0usize;
    let mut left = 0usize;
    let mut table = &tables[0];
    loop {
        if left == 0 {
            let s = *selectors
                .get(group)
                .ok_or(CodecError::Corrupt("ran out of selectors"))?;
            table = &tables[usize::from(s)];
            group += 1;
            left = GROUP_SIZE;
        }
        left -= 1;
        r.refill();
        if r.overrun() {
            return Err(CodecError::Truncated);
        }
        let sym = table.decode(&mut r)?;
        if sym <= 1 {
            // RUNA adds 1 << k, RUNB 2 << k (bijective base 2).
            if run_bit > 20 {
                return Err(CodecError::Corrupt("run too long"));
            }
            run += (usize::from(sym) + 1) << run_bit;
            run_bit += 1;
            continue;
        }
        if run > 0 {
            if tt.len() + run > max {
                return Err(CodecError::Corrupt("block longer than its level allows"));
            }
            let b = seq[usize::from(mtf[0])];
            counts[usize::from(b)] += run as u32;
            tt.resize(tt.len() + run, u32::from(b));
            run = 0;
            run_bit = 0;
        }
        if sym == eob {
            break;
        }
        if usize::from(sym) >= alpha {
            return Err(CodecError::Corrupt("bad symbol"));
        }
        let k = usize::from(sym - 1);
        let v = mtf[k];
        mtf.copy_within(0..k, 1);
        mtf[0] = v;
        if tt.len() >= max {
            return Err(CodecError::Corrupt("block longer than its level allows"));
        }
        let b = seq[usize::from(v)];
        counts[usize::from(b)] += 1;
        tt.push(u32::from(b));
    }
    let end = r.bitpos();
    let n = tt.len();
    if orig_ptr >= n {
        return Err(CodecError::Corrupt("BWT origin out of range"));
    }

    // Inverse BWT: tt[i] gets, above its own byte, the index of the next byte in the output.
    let mut start = [0u32; 256];
    let mut sum = 0u32;
    for (s, &c) in start.iter_mut().zip(&counts) {
        *s = sum;
        sum += c;
    }
    for i in 0..n {
        let b = (tt[i] & 0xFF) as usize;
        let at = start[b] as usize;
        tt[at] |= (i as u32) << 8;
        start[b] += 1;
    }

    // Walk it, undoing the first run-length stage (4 equal bytes, then a count of more).
    let worst = n + n / 5 * 255;
    let checked = worst > limit;
    let mut out = Vec::with_capacity(if checked {
        n.min(limit)
    } else {
        worst.min(n * 2)
    });
    // Reserved: the vector's capacity, which covers the `n` single bytes; runs grow it.
    let mut covered = out.capacity().max(n.min(limit));
    held.grow(covered as u64).map_err(memory)?;
    let mut pos = (tt[orig_ptr] >> 8) as usize;
    let mut last: i32 = -1;
    let mut same = 0u32;
    // Bytes added by runs so far (the output is at most `n` plus these).
    let mut extra = 0usize;
    for _ in 0..n {
        let e = tt[pos];
        let b = e as u8;
        pos = (e >> 8) as usize;
        if same == 4 {
            let k = usize::from(b);
            if checked {
                room(&out, k, limit)?;
            }
            extra += k;
            if n + extra > covered {
                let want = (n + extra)
                    .max(covered.saturating_mul(2))
                    .min(worst.min(limit))
                    .max(covered);
                held.grow((want - covered) as u64).map_err(memory)?;
                covered = want;
            }
            out.resize(out.len() + k, last as u8);
            same = 0;
            last = -1;
            continue;
        }
        if i32::from(b) == last {
            same += 1;
        } else {
            same = 1;
            last = i32::from(b);
        }
        if checked {
            room(&out, 1, limit)?;
        }
        out.push(b);
    }
    let mut crc = Crc32Be::new();
    crc.update(&out);
    if crc.finish() != stored_crc {
        return Err(CodecError::Checksum);
    }
    drop(tt);
    Ok(Block {
        out,
        _held: held,
        crc: stored_crc,
        end,
    })
}

/// Bit positions of every block magic in `data` (true blocks and chance occurrences alike).
fn scan(data: &[u8]) -> Vec<usize> {
    // The magic at bit offset s of byte i fully covers byte i + 1: one table lookup per byte
    // rules nearly every position out.
    let mut first = [0u8; 256];
    for s in 0..8 {
        let b = ((BLOCK_MAGIC >> (32 + s)) & 0xFF) as usize;
        first[b] |= 1 << s;
    }
    let mut found = Vec::new();
    for i in 0..data.len().saturating_sub(6) {
        let shifts = first[usize::from(data[i + 1])];
        if shifts == 0 {
            continue;
        }
        let mut w = [0u8; 8];
        let avail = (data.len() - i).min(8);
        w[..avail].copy_from_slice(&data[i..i + avail]);
        let w = u64::from_be_bytes(w);
        for s in 0..8 {
            if shifts & (1 << s) != 0 && (w >> (16 - s)) & 0xFFFF_FFFF_FFFF == BLOCK_MAGIC {
                found.push(i * 8 + s);
            }
        }
    }
    found
}

/// Blocks decoded ahead on several threads, a batch at a time.
struct Ahead<'a> {
    data: &'a [u8],
    starts: Vec<usize>,
    /// Blocks decoded ahead (errors are not kept: a start may have been decoded for the wrong
    /// stream, or be no block at all).
    done: HashMap<usize, Block>,
    threads: Threads,
    /// The caller's budget, which the helper threads reserve from.
    budget: BudgetHandle,
}

impl Ahead<'_> {
    /// The block at `bit`, decoded on this thread. Should the budget refuse it while blocks
    /// decoded ahead hold their reservations, those are given up (decoded again later).
    fn single(&mut self, bit: usize, level: usize, limit: usize) -> CResult<Block> {
        match decode_block(self.data, bit, level, limit, &self.budget) {
            Err(CodecError::Memory(_)) if !self.done.is_empty() => {
                self.done.clear();
                decode_block(self.data, bit, level, limit, &self.budget)
            }
            r => r,
        }
    }

    /// The block starting at bit `bit`: from the last batch, or decoded with the batch of
    /// starts beginning there.
    fn take(&mut self, bit: usize, level: usize, limit: usize) -> CResult<Block> {
        if let Some(b) = self.done.remove(&bit) {
            return Ok(b);
        }
        let Ok(first) = self.starts.binary_search(&bit) else {
            return self.single(bit, level, limit);
        };
        let batch: Vec<usize> = self.starts[first..]
            .iter()
            .copied()
            .take(self.threads.n * 2)
            .collect();
        let slots: Vec<Mutex<Option<Block>>> = batch.iter().map(|_| Mutex::new(None)).collect();
        // Output held by the batch: once it passes the limit, the rest is not needed (and a
        // bomb holds at most the limit plus a block per thread).
        let held = AtomicUsize::new(0);
        let data = self.data;
        let budget = &self.budget;
        self.threads.run("arc-bz2", batch.len(), |i| {
            if i > 0 && held.load(Ordering::Relaxed) > limit {
                return;
            }
            match decode_block(data, batch[i], level, limit, budget) {
                Ok(b) => {
                    held.fetch_add(b.out.len(), Ordering::Relaxed);
                    *slots[i].lock() = Some(b);
                }
                // Only the first start is surely a block; its error is found again below.
                Err(_) if i == 0 => {
                    held.fetch_add(limit.saturating_add(1), Ordering::Relaxed);
                }
                Err(_) => {}
            }
        });
        self.done.clear();
        for (b, s) in batch.iter().zip(slots) {
            if let Some(r) = s.into_inner() {
                self.done.insert(*b, r);
            }
        }
        match self.done.remove(&bit) {
            Some(b) => Ok(b),
            None => self.single(bit, level, limit),
        }
    }
}

/// Decodes the bzip2 data `data` (one or more concatenated streams), appending to `out` without
/// letting it pass `limit` bytes. Bytes after the last stream are ignored.
pub(crate) fn decode(
    data: &[u8],
    out: &mut Vec<u8>,
    limit: usize,
    threads: Threads,
) -> CResult<()> {
    if !detect(data) {
        return Err(CodecError::Corrupt("not a bzip2 stream"));
    }
    let budget_handle = specular_core::current_budget();
    let mut ahead = (threads.n > 1).then(|| Ahead {
        data,
        starts: scan(data),
        done: HashMap::new(),
        threads,
        budget: budget_handle.clone(),
    });
    let mut at = 0usize;
    loop {
        let level = usize::from(data[at + 3] - b'0');
        let mut bit = (at + 4) * 8;
        let mut combined = 0u32;
        loop {
            let mut r = Msb::at(data, bit);
            let magic = (u64::from(r.read(24)) << 24) | u64::from(r.read(24));
            if magic == END_MAGIC {
                let stored = r.read(32);
                if r.overrun() {
                    return Err(CodecError::Truncated);
                }
                if stored != combined {
                    return Err(CodecError::Checksum);
                }
                bit = r.bitpos();
                break;
            }
            if r.overrun() {
                return Err(CodecError::Truncated);
            }
            if magic != BLOCK_MAGIC {
                return Err(CodecError::Corrupt("bad block magic"));
            }
            let budget = limit - out.len();
            let block = match &mut ahead {
                Some(a) => a.take(bit, level, budget),
                None => decode_block(data, bit, level, budget, &budget_handle),
            }?;
            room(out, block.out.len(), limit)?;
            out.extend_from_slice(&block.out);
            combined = combined.rotate_left(1) ^ block.crc;
            bit = block.end;
        }
        at = bit.div_ceil(8);
        // Another stream may follow (pbzip2, `cat a.bz2 b.bz2`); anything else is ignored.
        if !data.get(at..).is_some_and(detect) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_scan_finds_every_offset() {
        for s in 0..8 {
            let mut v = vec![0u8; 20];
            let m = BLOCK_MAGIC << (16 - s);
            v[3..11].copy_from_slice(&m.to_be_bytes());
            // The low bits of the shifted magic spill into the next byte.
            assert_eq!(scan(&v), vec![3 * 8 + s], "shift {s}");
        }
        assert!(scan(&[0x31, 0x41]).is_empty());
    }

    #[test]
    fn tables_canonical() {
        // Lengths 1, 2, 3, 3: codes 0, 10, 110, 111.
        let t = Table::new(&[2, 1, 3, 3]).unwrap();
        let data = [0b0101_1011, 0b1000_0000, 0, 0, 0, 0, 0, 0, 0, 0];
        let mut r = Msb::at(&data, 0);
        r.refill();
        let syms: Vec<u16> = (0..5).map(|_| t.decode(&mut r).unwrap()).collect();
        // 0 | 10 | 110 | 111 | 0
        assert_eq!(syms, vec![1, 0, 2, 3, 1]);
        assert!(Table::new(&[1, 1, 1]).is_err());
    }

    #[test]
    fn empty_stream() {
        // "BZh9" and the end marker with a combined CRC of 0.
        let data = [
            b'B', b'Z', b'h', b'9', 0x17, 0x72, 0x45, 0x38, 0x50, 0x90, 0, 0, 0, 0,
        ];
        let mut out = Vec::new();
        decode(&data, &mut out, 10, Threads::ONE).unwrap();
        assert!(out.is_empty());
        assert!(decode(&data[..12], &mut out, 10, Threads::ONE).is_err());
    }
}

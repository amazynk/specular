//! A bzip2 decompressor (for `.xcf.bz2`): Huffman-coded MTF/RLE2 symbols, inverse
//! Burrows–Wheeler transform, then the initial run-length stage. Block CRCs are verified.
//! Written from the format's description; randomised blocks (not produced since bzip2 0.9.5)
//! are not supported.

use crate::{Error, Result};

struct Bits<'a> {
    data: &'a [u8],
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

    #[inline]
    fn refill(&mut self) {
        while self.n <= 56 {
            let b = self.data.get(self.pos).copied().unwrap_or(0);
            self.pos += 1;
            self.buf |= u64::from(b) << (56 - self.n);
            self.n += 8;
        }
    }

    /// Whether more bits were consumed than the input had.
    fn overrun(&self) -> bool {
        self.pos.saturating_sub((self.n / 8) as usize) > self.data.len()
    }

    #[inline]
    fn peek(&mut self, k: u32) -> u32 {
        if self.n < k {
            self.refill();
        }
        (self.buf >> (64 - k)) as u32
    }

    #[inline]
    fn skip(&mut self, k: u32) {
        self.buf <<= k;
        self.n -= k;
    }

    #[inline]
    fn bits(&mut self, k: u32) -> u32 {
        let v = self.peek(k);
        self.skip(k);
        v
    }

    fn bit(&mut self) -> bool {
        self.bits(1) == 1
    }

    /// Discards bits up to the next byte boundary.
    fn align(&mut self) {
        let r = self.n % 8;
        self.skip(r);
    }
}

fn bad(what: &str) -> Error {
    Error::Malformed(format!("bzip2: {what}"))
}

const BLOCK_MAGIC: u64 = 0x3141_5926_5359;
const END_MAGIC: u64 = 0x1772_4538_5090;
const PRIMARY: u32 = 10;

/// A canonical Huffman code (lengths 1..=20).
struct Huffman {
    /// Codes of up to `PRIMARY` bits: `(symbol << 5) | length`, 0 = longer code.
    fast: Vec<u16>,
    /// Per length: first code, index of its first symbol in `sorted`, count.
    first: [u32; 21],
    offset: [u32; 21],
    count: [u32; 21],
    sorted: Vec<u16>,
}

impl Huffman {
    /// None for over-subscribed lengths (more codes of some length than the code space has
    /// left), which no encoder writes.
    fn new(lengths: &[u8]) -> Option<Huffman> {
        let mut count = [0u32; 21];
        for &l in lengths {
            count[l as usize] += 1;
        }
        let mut first = [0u32; 21];
        let mut offset = [0u32; 21];
        let mut code = 0u32;
        let mut off = 0u32;
        for l in 1..=20 {
            first[l] = code;
            offset[l] = off;
            if code + count[l] > 1 << l {
                return None;
            }
            code = (code + count[l]) << 1;
            off += count[l];
        }
        let mut sorted = Vec::with_capacity(lengths.len());
        for l in 1..=20u8 {
            for (s, &sl) in lengths.iter().enumerate() {
                if sl == l {
                    sorted.push(s as u16);
                }
            }
        }
        let mut fast = vec![0u16; 1 << PRIMARY];
        for l in 1..=PRIMARY as usize {
            for i in 0..count[l] {
                let c = first[l] + i;
                let sym = sorted[(offset[l] + i) as usize];
                let shift = PRIMARY as usize - l;
                let base = (c as usize) << shift;
                for e in &mut fast[base..base + (1 << shift)] {
                    *e = (sym << 5) | l as u16;
                }
            }
        }
        Some(Huffman {
            fast,
            first,
            offset,
            count,
            sorted,
        })
    }

    #[inline]
    fn decode(&self, b: &mut Bits) -> Option<u16> {
        let e = self.fast[b.peek(PRIMARY) as usize];
        if e != 0 {
            b.skip(u32::from(e & 31));
            return Some(e >> 5);
        }
        let v = b.peek(20);
        for l in PRIMARY as usize + 1..=20 {
            let c = v >> (20 - l);
            let d = c.wrapping_sub(self.first[l]);
            if d < self.count[l] {
                b.skip(l as u32);
                return Some(self.sorted[(self.offset[l] + d) as usize]);
            }
        }
        None
    }
}

fn crc_table() -> &'static [u32; 256] {
    static T: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        std::array::from_fn(|i| {
            let mut c = (i as u32) << 24;
            for _ in 0..8 {
                c = if c & 0x8000_0000 != 0 {
                    (c << 1) ^ 0x04c1_1db7
                } else {
                    c << 1
                };
            }
            c
        })
    })
}

/// Decompresses all streams in `data`, stopping once `limit` bytes are out (the result is then
/// cut to `limit`).
pub(crate) fn decompress(data: &[u8], limit: u64) -> Result<(Vec<u8>, specular_core::Reservation)> {
    // The output is reserved before it grows (a block expands to at most ~46 MB), the block
    // vector once at its largest; the output's reservation goes to the caller with it.
    let mut res = specular_core::Reservation::none();
    let mut tt_res = specular_core::Reservation::none();
    let mut out = Vec::new();
    let mut b = Bits::new(data);
    let mut tt: Vec<u32> = Vec::new();
    loop {
        // Stream header.
        if b.bits(24) != 0x42_5a_68 {
            break;
        }
        let level = b.bits(8);
        if !(u32::from(b'1')..=u32::from(b'9')).contains(&level) {
            return Err(bad("block size"));
        }
        let max_block = (level - u32::from(b'0')) as usize * 100_000;
        loop {
            let magic = (u64::from(b.bits(24)) << 24) | u64::from(b.bits(24));
            if magic == END_MAGIC {
                b.bits(32); // combined CRC
                b.align();
                break;
            }
            if magic != BLOCK_MAGIC || b.overrun() {
                return Err(if b.overrun() {
                    Error::Truncated("bzip2 stream".into())
                } else {
                    bad("block magic")
                });
            }
            let crc = b.bits(32);
            let start = out.len();
            tt_res.grow_to(max_block as u64 * 4)?;
            block(&mut b, max_block, &mut tt, &mut out, limit, &mut res)?;
            crate::container::track(&out, &mut res)?;
            if out.len() as u64 >= limit {
                out.truncate(limit as usize);
                return Ok((out, res));
            }
            let mut c = !0u32;
            let t = crc_table();
            for &v in &out[start..] {
                c = (c << 8) ^ t[((c >> 24) as u8 ^ v) as usize];
            }
            if !c != crc {
                return Err(bad("block CRC mismatch"));
            }
        }
        if b.overrun() {
            break;
        }
    }
    if out.is_empty() {
        return Err(Error::Truncated("bzip2 stream".into()));
    }
    Ok((out, res))
}

fn block(
    b: &mut Bits,
    max_block: usize,
    tt: &mut Vec<u32>,
    out: &mut Vec<u8>,
    limit: u64,
    res: &mut specular_core::Reservation,
) -> Result<()> {
    if b.bit() {
        return Err(Error::Unsupported("randomised bzip2 block".into()));
    }
    let orig_ptr = b.bits(24) as usize;

    // Symbol map.
    let used = b.bits(16);
    let mut seq = Vec::with_capacity(256);
    for i in 0..16 {
        if used & (0x8000 >> i) != 0 {
            let m = b.bits(16);
            for j in 0..16 {
                if m & (0x8000 >> j) != 0 {
                    seq.push((i * 16 + j) as u8);
                }
            }
        }
    }
    if seq.is_empty() {
        return Err(bad("no symbols"));
    }
    let alpha = seq.len() + 2;

    let groups = b.bits(3) as usize;
    if !(2..=6).contains(&groups) {
        return Err(bad("table count"));
    }
    let nsel = b.bits(15) as usize;
    if nsel == 0 {
        return Err(bad("selector count"));
    }
    let mut mtf_g: Vec<u8> = (0..groups as u8).collect();
    let mut selectors = Vec::with_capacity(nsel);
    for _ in 0..nsel {
        let mut j = 0;
        while b.bit() {
            j += 1;
            if j >= groups {
                return Err(bad("selector"));
            }
            if b.overrun() {
                return Err(Error::Truncated("bzip2 selectors".into()));
            }
        }
        let v = mtf_g[j];
        mtf_g.copy_within(0..j, 1);
        mtf_g[0] = v;
        selectors.push(v);
    }

    let mut tables = Vec::with_capacity(groups);
    for _ in 0..groups {
        let mut lens = vec![0u8; alpha];
        let mut l = b.bits(5) as i32;
        for x in lens.iter_mut() {
            loop {
                if !(1..=20).contains(&l) || b.overrun() {
                    return Err(bad("code length"));
                }
                if !b.bit() {
                    break;
                }
                l += if b.bit() { -1 } else { 1 };
            }
            *x = l as u8;
        }
        tables.push(Huffman::new(&lens).ok_or_else(|| bad("over-subscribed code lengths"))?);
    }

    // Symbols → MTF → bytes (into the low byte of `tt`).
    tt.clear();
    tt.reserve(max_block);
    // All 256 byte values may be in use: count in usize, not u8.
    let mut mtf: Vec<u8> = (0..seq.len()).map(|i| i as u8).collect();
    let mut counts = [0u32; 256];
    let eob = (alpha - 1) as u16;
    let mut run = 0usize;
    let mut run_bit = 1usize;
    let mut sel = 0usize;
    let mut left = 0usize;
    let mut table = &tables[0];
    loop {
        if left == 0 {
            let s = *selectors.get(sel).ok_or_else(|| bad("selectors ran out"))?;
            table = &tables[s as usize];
            sel += 1;
            left = 50;
        }
        left -= 1;
        let sym = table.decode(b).ok_or_else(|| bad("Huffman code"))?;
        if b.overrun() {
            return Err(Error::Truncated("bzip2 block".into()));
        }
        if sym <= 1 {
            // RUNA / RUNB: bijective base-2 run length of the front symbol.
            run += run_bit << sym;
            run_bit <<= 1;
            if run > max_block {
                return Err(bad("run too long"));
            }
            continue;
        }
        if run > 0 {
            let v = seq[mtf[0] as usize];
            if tt.len() + run > max_block {
                return Err(bad("block too long"));
            }
            counts[v as usize] += run as u32;
            tt.extend(std::iter::repeat_n(u32::from(v), run));
            run = 0;
            run_bit = 1;
        }
        if sym == eob {
            break;
        }
        let i = (sym - 1) as usize;
        if i >= mtf.len() {
            return Err(bad("MTF index"));
        }
        let m = mtf[i];
        mtf.copy_within(0..i, 1);
        mtf[0] = m;
        let v = seq[m as usize];
        if tt.len() >= max_block {
            return Err(bad("block too long"));
        }
        counts[v as usize] += 1;
        tt.push(u32::from(v));
    }
    let n = tt.len();
    if n == 0 {
        return Ok(());
    }
    if orig_ptr >= n {
        return Err(bad("origin pointer"));
    }

    // Inverse BWT: link each position to its successor in the upper 24 bits.
    let mut start = [0u32; 256];
    let mut sum = 0;
    for (s, c) in start.iter_mut().zip(counts) {
        *s = sum;
        sum += c;
    }
    for i in 0..n {
        let v = (tt[i] & 0xff) as usize;
        let j = start[v] as usize;
        start[v] += 1;
        tt[j] |= (i as u32) << 8;
    }

    // Walk, undoing the initial RLE (4 equal bytes then a repeat count).
    let mut pos = (tt[orig_ptr] >> 8) as usize;
    let mut last = -1i32;
    let mut same = 0;
    for _ in 0..n {
        let e = tt[pos];
        pos = (e >> 8) as usize;
        let v = (e & 0xff) as u8;
        // A run adds at most 255 bytes, a literal one.
        if out.capacity() - out.len() < 256 {
            crate::container::make_room(out, 256, limit, res)?;
        }
        if same == 4 {
            out.extend(std::iter::repeat_n(last as u8, v as usize));
            same = 0;
            last = -1;
        } else {
            if i32::from(v) == last {
                same += 1;
            } else {
                same = 1;
                last = i32::from(v);
            }
            out.push(v);
        }
        if out.len() as u64 >= limit {
            return Ok(());
        }
    }
    Ok(())
}

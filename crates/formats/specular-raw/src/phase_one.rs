//! Phase One IIQ sensor codecs. Every row starts at its own offset (a table of little-endian
//! 32-bit offsets from the data start), so rows decode independently.
//!
//! * IIQ L (formats 3, 5 and 8): per pair of columns, a bit length chosen every eight samples
//!   by a short prefix code; each sample adds a biased difference to the previous sample of the
//!   same column parity, or is stored raw (16 bits) at the full length. Format 5 maps samples
//!   below 256 through a square curve. Values are 14-bit, stored shifted to 16 (except format 8).
//! * IIQ S (format 6): blocks of eight samples; per parity a step code adjusts the bit depth of
//!   that parity, and a block code adds extra precision for both; 14-bit values shifted to 16.
//! * Uncompressed (formats 1 and 2): 16-bit samples whose bits are swapped between pairs of
//!   samples under a key.
//!
//! All bit streams read big-endian bit order from little-endian 32-bit words.

use rayon::prelude::*;

/// IIQ variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// IIQ L; `format` 3, 5 or 8.
    L { format: u32 },
    /// IIQ S.
    S,
}

/// MSB-first bits from little-endian 32-bit words; reads past the end give zeros.
struct Bits<'a> {
    src: &'a [u8],
    pos: usize,
    buf: u64,
    n: u32,
    over: bool,
}

impl<'a> Bits<'a> {
    fn new(src: &'a [u8]) -> Self {
        Bits {
            src,
            pos: 0,
            buf: 0,
            n: 0,
            over: false,
        }
    }

    #[inline(always)]
    fn fill(&mut self) {
        if self.n <= 32 {
            let w = match self.src.get(self.pos..self.pos + 4) {
                Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                None => {
                    // A partial last word reads as if zero-padded.
                    let mut b = [0u8; 4];
                    let rest = self.src.get(self.pos..).unwrap_or(&[]);
                    b[..rest.len()].copy_from_slice(rest);
                    self.over |= rest.is_empty();
                    u32::from_le_bytes(b)
                }
            };
            self.pos += 4;
            self.buf = self.buf << 32 | w as u64;
            self.n += 32;
        }
    }

    /// The next `len` (0..=16) bits without consuming them.
    #[inline(always)]
    fn peek(&mut self, len: u32) -> u32 {
        if len > self.n {
            self.fill();
        }
        if len == 0 {
            return 0;
        }
        ((self.buf >> (self.n - len)) & ((1u64 << len) - 1)) as u32
    }

    #[inline(always)]
    fn skip(&mut self, len: u32) {
        if len > self.n {
            self.fill();
        }
        self.n -= len.min(self.n);
    }

    #[inline(always)]
    fn get(&mut self, len: u32) -> u32 {
        let v = self.peek(len);
        self.skip(len);
        v
    }
}

/// IIQ L bit lengths by prefix (zeros before a one, then one more bit).
const L_LEN: [u32; 10] = [8, 7, 6, 9, 11, 10, 5, 12, 14, 13];

fn row_l(src: &[u8], format: u32, curve: &[u16; 256], out: &mut [u16]) -> bool {
    let w = out.len();
    let mut br = Bits::new(src);
    let mut pred = [0i32; 2];
    let mut len = [14u32; 2];
    for (col, o) in out.iter_mut().enumerate() {
        if col >= w & !7 {
            len = [14; 2];
        } else if col & 7 == 0 {
            for l in &mut len {
                let mut zeros = 0;
                while zeros < 5 && br.get(1) == 0 {
                    zeros += 1;
                }
                if zeros > 0 {
                    *l = L_LEN[(zeros - 1) * 2 + br.get(1) as usize];
                }
            }
        }
        let p = col & 1;
        let n = len[p];
        if n == 14 {
            pred[p] = br.get(16) as i32;
        } else {
            pred[p] += br.get(n) as i32 + 1 - (1 << (n - 1));
        }
        let mut v = pred[p] as u16;
        if format == 5 && v < 256 {
            v = curve[v as usize];
        }
        *o = if format == 8 { v } else { v << 2 };
    }
    br.over
}

const S_EXTRA: [u32; 8] = [1, 2, 3, 4, 0, 0, 0, 0];
const S_USED: [u32; 8] = [3, 3, 3, 3, 1, 1, 1, 1];
/// Depth by 7-bit step code below 32, and how many bits that code takes after its first two.
const S_DEPTH: [u32; 32] = [
    9, 8, 0, 7, 6, 6, 5, 5, 1, 1, 1, 1, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2,
];
const S_SKIP: [u32; 32] = [
    5, 5, 5, 5, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];

#[inline(always)]
fn shr16(s: u32) -> i32 {
    if s >= 32 { 0 } else { (0xffffu32 >> s) as i32 }
}

fn row_s(src: &[u8], out: &mut [u16]) -> bool {
    let w = out.len();
    let mut br = Bits::new(src);
    let init = br.get(16) & 7;
    let sub_init = 17 - init;
    let mut depth = [0u32; 2];
    let mut prev = [0i32; 2];
    let mut done = 0;
    if w >= 8 {
        for block in out.chunks_exact_mut(8) {
            for d in &mut depth {
                let code = br.peek(7);
                br.skip(2);
                if code >= 32 {
                    *d = (*d + (code >> 5)).wrapping_sub(2);
                } else {
                    *d = S_DEPTH[code as usize];
                    br.skip(S_SKIP[code as usize]);
                }
            }
            let b = br.peek(3) as usize;
            br.skip(S_USED[b]);
            let take = init + S_EXTRA[b];
            let shift = depth.map(|d| d.wrapping_sub(S_EXTRA[b]));
            let sub = depth.map(|d| shr16(sub_init.wrapping_sub(d)));
            for (i, o) in block.iter_mut().enumerate() {
                let p = i & 1;
                let v = if depth[p] == 9 {
                    br.get(14) as i32
                } else {
                    let d = br.get(take.min(16));
                    prev[p]
                        .wrapping_add(d.checked_shl(shift[p]).unwrap_or(0) as i32)
                        .wrapping_sub(sub[p])
                };
                *o = v.wrapping_shl(2).clamp(0, 0xffff) as u16;
                prev[p] = v;
            }
            done += 8;
        }
    }
    for o in &mut out[done..] {
        *o = (br.get(14) << 2) as u16;
    }
    br.over
}

/// Decodes `w x h` samples. `data` holds the coded rows; `rows` the table of row offsets into
/// `data`. Returns true when data ran short.
pub(crate) fn decode(
    data: &[u8],
    rows: &[u8],
    kind: Kind,
    w: usize,
    h: usize,
    out: &mut [u16],
    parallel: bool,
) -> bool {
    let mut offsets: Vec<(usize, usize)> = (0..h)
        .map(|r| {
            let o = rows.get(4 * r..4 * r + 4).map_or(usize::MAX, |b| {
                u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize
            });
            (o, r)
        })
        .collect();
    // Each row ends where the next one (in file order) starts.
    let mut ends = vec![data.len(); h];
    offsets.sort_unstable();
    for k in 0..h {
        let next = offsets.get(k + 1).map_or(data.len(), |n| n.0);
        ends[offsets[k].1] = next.min(data.len());
    }
    let starts: Vec<usize> = {
        let mut s = vec![usize::MAX; h];
        for &(o, r) in &offsets {
            s[r] = o;
        }
        s
    };
    let mut curve = [0u16; 256];
    for (i, c) in curve.iter_mut().enumerate() {
        *c = ((i * i) as f32 / 3.969f32 + 0.5f32) as u16;
    }
    let work = |(r, row): (usize, &mut [u16])| -> bool {
        let Some(src) = data.get(starts[r]..ends[r].max(starts[r])) else {
            return true;
        };
        match kind {
            Kind::L { format } => row_l(src, format, &curve, row),
            Kind::S => row_s(src, row),
        }
    };
    let out = &mut out[..w * h];
    if parallel {
        out.par_chunks_mut(w)
            .enumerate()
            .map(work)
            .reduce(|| false, |a, b| a | b)
    } else {
        out.chunks_mut(w)
            .enumerate()
            .map(work)
            .fold(false, |a, b| a | b)
    }
}

/// Uncompressed formats 1 and 2: little-endian 16-bit samples, bits exchanged between the two
/// samples of each pair under `mask` after XOR with the two key halves.
pub(crate) fn decode_plain(src: &[u8], key: Option<(u16, u16, u16)>, out: &mut [u16]) -> bool {
    let n = out.len().min(src.len() / 2);
    for (o, b) in out.iter_mut().zip(src.chunks_exact(2)) {
        *o = u16::from_le_bytes([b[0], b[1]]);
    }
    if let Some((ka, kb, mask)) = key {
        for p in out[..n & !1].chunks_exact_mut(2) {
            let (a, b) = (p[0] ^ ka, p[1] ^ kb);
            p[0] = (a & mask) | (b & !mask);
            p[1] = (b & mask) | (a & !mask);
        }
    }
    n < out.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first writer into little-endian 32-bit words.
    #[derive(Default)]
    struct W {
        bits: Vec<bool>,
    }
    impl W {
        fn put(&mut self, v: u32, len: u32) {
            for k in (0..len).rev() {
                self.bits.push(v >> k & 1 == 1);
            }
        }
        fn bytes(&self) -> Vec<u8> {
            let mut out = Vec::new();
            for word in self.bits.chunks(32) {
                let mut v = 0u32;
                for (i, &b) in word.iter().enumerate() {
                    v |= (b as u32) << (31 - i);
                }
                out.extend(v.to_le_bytes());
            }
            out
        }
    }

    fn table(offsets: &[u32]) -> Vec<u8> {
        offsets.iter().flat_map(|o| o.to_le_bytes()).collect()
    }

    #[test]
    fn l_rows() {
        // Width 10: one group of eight with length 8 for both parities (prefix "1" + bit 0 →
        // zeros = 1, index 0 → 8), then two raw 16-bit samples.
        let mut w = W::default();
        for _ in 0..2 {
            w.put(0b01, 2);
            w.put(0, 1);
        }
        let mut want = Vec::new();
        let mut pred = [0i32; 2];
        for col in 0..8 {
            let d = (col as i32 * 7) % 50;
            w.put(d as u32, 8);
            pred[col & 1] += d + 1 - 128;
            want.push(pred[col & 1]);
        }
        w.put(300, 16);
        w.put(1000, 16);
        want.extend([300, 1000]);
        let row = w.bytes();
        let mut data = row.clone();
        data.extend(&row);
        let rows = table(&[0, row.len() as u32]);
        for format in [3, 5, 8] {
            let mut out = vec![0u16; 20];
            assert!(!decode(
                &data,
                &rows,
                Kind::L { format },
                10,
                2,
                &mut out,
                true
            ));
            for (i, &v) in want.iter().enumerate() {
                let mut v = v as u16;
                if format == 5 && v < 256 {
                    v = ((v as usize * v as usize) as f32 / 3.969 + 0.5) as u16;
                }
                let v = if format == 8 { v } else { v << 2 };
                assert_eq!(out[i], v, "format {format} col {i}");
                assert_eq!(out[10 + i], v);
            }
        }
    }

    #[test]
    fn s_rows() {
        // Width 9: one block (depth 9 = raw 14-bit samples via step code 0) and a raw tail.
        let mut w = W::default();
        w.put(3, 16); // init bits 3
        for _ in 0..2 {
            w.put(0, 7); // code 0: depth 9; 2 + 5 bits
        }
        w.put(1, 1); // block codes 4..7 take one bit: "1"
        let vals: Vec<u32> = (0..9).map(|i| 1000 + 37 * i).collect();
        for &v in &vals {
            w.put(v, 14);
        }
        let data = w.bytes();
        let mut out = vec![0u16; 9];
        assert!(!decode(&data, &table(&[0]), Kind::S, 9, 1, &mut out, false));
        assert_eq!(
            out,
            vals.iter().map(|v| (v << 2) as u16).collect::<Vec<_>>()
        );
    }

    #[test]
    fn plain_key_and_hostile() {
        let src: Vec<u8> = [0x1234u16, 0xabcd]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut out = [0u16; 2];
        assert!(!decode_plain(&src, None, &mut out));
        assert_eq!(out, [0x1234, 0xabcd]);
        assert!(!decode_plain(&src, Some((0, 0, 0xffff)), &mut out));
        assert_eq!(out, [0x1234, 0xabcd]);
        assert!(!decode_plain(&src, Some((0, 0, 0)), &mut out));
        assert_eq!(out, [0xabcd, 0x1234]);
        let mut out = [0u16; 64];
        assert!(decode_plain(&src, None, &mut out));
        // Bad offsets and empty data.
        for kind in [Kind::S, Kind::L { format: 5 }] {
            let _ = decode(
                &[0xff; 40],
                &table(&[u32::MAX, 3, 1 << 30]),
                kind,
                16,
                3,
                &mut out[..48],
                true,
            );
            assert!(decode(&[], &[], kind, 16, 3, &mut out[..48], false));
        }
    }
}

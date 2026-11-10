//! CCITT bilevel decoding (TIFF compressions 2, 3, 4 and 32771): Modified Huffman (T.4 1D),
//! Modified READ (T.4 2D) and Modified Modified READ (T.6, Group 4).
//!
//! Table driven: a run-length code is resolved by one lookup of the next 12 (white) or 13
//! (black) bits, a 2D mode by one lookup of the next 7. Rows are kept as lists of changing
//! elements (the positions where the colour changes), so a row costs per change, not per
//! pixel; the bits are written by filling whole bytes between changes.
//!
//! Output rows are MSB-first, one bit per pixel, with black runs as 1 bits (the libtiff
//! convention, whatever the photometric interpretation says; PhotometricInterpretation then
//! decides how the bits show).

use std::sync::OnceLock;

/// The flavour of a strip / tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Compression 2: Modified Huffman rows, no EOLs, each row starts on a byte boundary.
    Rle,
    /// Compression 32771: as `Rle`, rows on 16-bit boundaries.
    RleWord,
    /// Compression 3 with T4Options: bit 0 2D coding, bit 2 EOLs byte-aligned (fill bits).
    G3 { two_d: bool },
    /// Compression 4 with T6Options.
    G4,
}

/// Table entry kinds.
const INVALID: u8 = 0;
const TERM: u8 = 1;
const MAKEUP: u8 = 2;
const EOL: u8 = 3;

/// A decoded code: kind, code length in bits, run length.
#[derive(Clone, Copy, Default)]
struct Code {
    kind: u8,
    len: u8,
    run: u16,
}

/// White terminating codes 0..=63 (T.4 table 2).
const WHITE_TERM: [&str; 64] = [
    "00110101", "000111", "0111", "1000", "1011", "1100", "1110", "1111", "10011", "10100",
    "00111", "01000", "001000", "000011", "110100", "110101", "101010", "101011", "0100111",
    "0001100", "0001000", "0010111", "0000011", "0000100", "0101000", "0101011", "0010011",
    "0100100", "0011000", "00000010", "00000011", "00011010", "00011011", "00010010", "00010011",
    "00010100", "00010101", "00010110", "00010111", "00101000", "00101001", "00101010", "00101011",
    "00101100", "00101101", "00000100", "00000101", "00001010", "00001011", "01010010", "01010011",
    "01010100", "01010101", "00100100", "00100101", "01011000", "01011001", "01011010", "01011011",
    "01001010", "01001011", "00110010", "00110011", "00110100",
];

/// White makeup codes 64..=1728 in steps of 64.
const WHITE_MAKEUP: [&str; 27] = [
    "11011",
    "10010",
    "010111",
    "0110111",
    "00110110",
    "00110111",
    "01100100",
    "01100101",
    "01101000",
    "01100111",
    "011001100",
    "011001101",
    "011010010",
    "011010011",
    "011010100",
    "011010101",
    "011010110",
    "011010111",
    "011011000",
    "011011001",
    "011011010",
    "011011011",
    "010011000",
    "010011001",
    "010011010",
    "011000",
    "010011011",
];

/// Black terminating codes 0..=63 (T.4 table 2).
const BLACK_TERM: [&str; 64] = [
    "0000110111",
    "010",
    "11",
    "10",
    "011",
    "0011",
    "0010",
    "00011",
    "000101",
    "000100",
    "0000100",
    "0000101",
    "0000111",
    "00000100",
    "00000111",
    "000011000",
    "0000010111",
    "0000011000",
    "0000001000",
    "00001100111",
    "00001101000",
    "00001101100",
    "00000110111",
    "00000101000",
    "00000010111",
    "00000011000",
    "000011001010",
    "000011001011",
    "000011001100",
    "000011001101",
    "000001101000",
    "000001101001",
    "000001101010",
    "000001101011",
    "000011010010",
    "000011010011",
    "000011010100",
    "000011010101",
    "000011010110",
    "000011010111",
    "000001101100",
    "000001101101",
    "000011011010",
    "000011011011",
    "000001010100",
    "000001010101",
    "000001010110",
    "000001010111",
    "000001100100",
    "000001100101",
    "000001010010",
    "000001010011",
    "000000100100",
    "000000110111",
    "000000111000",
    "000000100111",
    "000000101000",
    "000001011000",
    "000001011001",
    "000000101011",
    "000000101100",
    "000001011010",
    "000001100110",
    "000001100111",
];

/// Black makeup codes 64..=1728 in steps of 64.
const BLACK_MAKEUP: [&str; 27] = [
    "0000001111",
    "000011001000",
    "000011001001",
    "000001011011",
    "000000110011",
    "000000110100",
    "000000110101",
    "0000001101100",
    "0000001101101",
    "0000001001010",
    "0000001001011",
    "0000001001100",
    "0000001001101",
    "0000001110010",
    "0000001110011",
    "0000001110100",
    "0000001110101",
    "0000001110110",
    "0000001110111",
    "0000001010010",
    "0000001010011",
    "0000001010100",
    "0000001010101",
    "0000001011010",
    "0000001011011",
    "0000001100100",
    "0000001100101",
];

/// Extended makeup codes 1792..=2560 in steps of 64, shared by both colours.
const EXT_MAKEUP: [&str; 13] = [
    "00000001000",
    "00000001100",
    "00000001101",
    "000000010010",
    "000000010011",
    "000000010100",
    "000000010101",
    "000000010110",
    "000000010111",
    "000000011100",
    "000000011101",
    "000000011110",
    "000000011111",
];

const EOL_CODE: &str = "000000000001";

/// Bits looked up at once for white and black runs.
const WHITE_BITS: u32 = 12;
const BLACK_BITS: u32 = 13;

/// 2D modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum M2 {
    Invalid,
    Pass,
    Horizontal,
    /// Vertical with offset a1 - b1.
    V(i8),
    /// Extension (uncompressed mode) or the start of an EOL.
    Ext,
}

/// 7-bit lookup of the 2D mode codes: mode and code length.
const MODE_BITS: u32 = 7;

struct Tables {
    white: Vec<Code>,
    black: Vec<Code>,
    mode: [(M2, u8); 128],
}

fn fill(table: &mut [Code], bits: u32, code: &str, kind: u8, run: u16) {
    let len = code.len() as u32;
    let v = u32::from_str_radix(code, 2).expect("binary code");
    let shift = bits - len;
    for i in 0..1u32 << shift {
        let slot = &mut table[((v << shift) | i) as usize];
        debug_assert_eq!(slot.kind, INVALID, "overlapping code {code}");
        *slot = Code {
            kind,
            len: len as u8,
            run,
        };
    }
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut white = vec![Code::default(); 1 << WHITE_BITS];
        let mut black = vec![Code::default(); 1 << BLACK_BITS];
        for (i, c) in WHITE_TERM.iter().enumerate() {
            fill(&mut white, WHITE_BITS, c, TERM, i as u16);
        }
        for (i, c) in BLACK_TERM.iter().enumerate() {
            fill(&mut black, BLACK_BITS, c, TERM, i as u16);
        }
        for (i, c) in WHITE_MAKEUP.iter().enumerate() {
            fill(&mut white, WHITE_BITS, c, MAKEUP, 64 * (i as u16 + 1));
        }
        for (i, c) in BLACK_MAKEUP.iter().enumerate() {
            fill(&mut black, BLACK_BITS, c, MAKEUP, 64 * (i as u16 + 1));
        }
        for (i, c) in EXT_MAKEUP.iter().enumerate() {
            fill(&mut white, WHITE_BITS, c, MAKEUP, 1792 + 64 * i as u16);
            fill(&mut black, BLACK_BITS, c, MAKEUP, 1792 + 64 * i as u16);
        }
        fill(&mut white, WHITE_BITS, EOL_CODE, EOL, 0);
        fill(&mut black, BLACK_BITS, EOL_CODE, EOL, 0);
        let mut mode = [(M2::Invalid, 0u8); 128];
        let modes: [(&str, M2); 10] = [
            ("1", M2::V(0)),
            ("011", M2::V(1)),
            ("010", M2::V(-1)),
            ("001", M2::Horizontal),
            ("0001", M2::Pass),
            ("000011", M2::V(2)),
            ("000010", M2::V(-2)),
            ("0000011", M2::V(3)),
            ("0000010", M2::V(-3)),
            ("0000001", M2::Ext),
        ];
        for (code, m) in modes {
            let len = code.len() as u32;
            let v = u32::from_str_radix(code, 2).expect("binary code");
            let shift = MODE_BITS - len;
            for i in 0..1u32 << shift {
                mode[((v << shift) | i) as usize] = (m, len as u8);
            }
        }
        // "0000000": an EOL (or garbage).
        mode[0] = (M2::Ext, 7);
        Tables { white, black, mode }
    })
}

/// MSB-first bit reader with a 64-bit window. Past the end it reads zeros and counts them.
struct Bits<'a> {
    data: &'a [u8],
    /// Next byte to load.
    pos: usize,
    acc: u64,
    /// Valid bits in `acc` (left-aligned).
    n: u32,
    /// Bits consumed in total.
    used: u64,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        let mut b = Bits {
            data,
            pos: 0,
            acc: 0,
            n: 0,
            used: 0,
        };
        b.refill();
        b
    }

    #[inline(always)]
    fn refill(&mut self) {
        if self.n > 32 {
            return;
        }
        if let Some(chunk) = self.data.get(self.pos..self.pos + 8) {
            let v = u64::from_be_bytes(chunk.try_into().expect("8 bytes"));
            // Bits past `n` are always the stream's own, so OR-ing them again is harmless.
            self.acc |= v >> self.n;
            let k = (63 - self.n) / 8;
            self.pos += k as usize;
            self.n += k * 8;
        } else {
            while self.n <= 55 {
                let b = self.data.get(self.pos).copied().unwrap_or(0);
                self.acc |= (b as u64) << (56 - self.n);
                self.pos += 1;
                self.n += 8;
            }
        }
    }

    #[inline(always)]
    fn peek(&self, k: u32) -> u32 {
        (self.acc >> (64 - k)) as u32
    }

    #[inline(always)]
    fn skip(&mut self, k: u32) {
        self.acc <<= k;
        self.n -= k;
        self.used += k as u64;
    }

    /// All the input has been consumed (and some zeros beyond it).
    fn past_end(&self) -> bool {
        self.used > self.data.len() as u64 * 8
    }

    /// Skip to the next multiple of `bits` (8 or 16) from the start.
    fn align(&mut self, bits: u64) {
        let r = self.used % bits;
        if r != 0 {
            let mut k = bits - r;
            while k > 0 {
                self.refill();
                let s = k.min(32) as u32;
                self.skip(s);
                k -= s as u64;
            }
            self.refill();
        }
    }

    /// At an EOL (11 or more zeros, then a one; fill bits allow more zeros): consume it.
    fn eol(&mut self) -> bool {
        self.refill();
        let zeros = self.acc.leading_zeros();
        if zeros >= 11 && zeros < self.n {
            self.skip(zeros + 1);
            self.refill();
            return true;
        }
        if zeros >= self.n && zeros >= 11 {
            // A long run of fill zeros: skip them and look again.
            let mut skipped = 0u64;
            loop {
                let z = self.acc.leading_zeros().min(self.n);
                if z < self.n {
                    self.skip(z + 1);
                    self.refill();
                    return true;
                }
                self.skip(z);
                self.refill();
                skipped += z as u64;
                if self.past_end() || skipped > 1 << 20 {
                    return false;
                }
            }
        }
        false
    }

    /// Search forward for the next EOL (resynchronisation after damage).
    fn seek_eol(&mut self) -> bool {
        while !self.past_end() {
            if self.eol() {
                return true;
            }
            self.skip(1);
            self.refill();
        }
        false
    }
}

/// Decode one run of `black` or white pixels (makeup codes then a terminating code).
#[inline(always)]
fn run(bits: &mut Bits, black: bool) -> Option<u32> {
    let t = tables();
    let mut total = 0u32;
    loop {
        bits.refill();
        let c = if black {
            t.black[bits.peek(BLACK_BITS) as usize]
        } else {
            t.white[bits.peek(WHITE_BITS) as usize]
        };
        match c.kind {
            TERM => {
                bits.skip(c.len as u32);
                return Some(total + c.run as u32);
            }
            MAKEUP => {
                bits.skip(c.len as u32);
                total += c.run as u32;
                if total > 1 << 24 {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

/// Set bits `x0..x1` of an MSB-first row.
#[inline]
fn set_bits(row: &mut [u8], x0: usize, x1: usize) {
    if x0 >= x1 {
        return;
    }
    let (b0, b1) = (x0 / 8, (x1 - 1) / 8);
    let m0 = 0xffu8 >> (x0 % 8);
    let m1 = 0xffu8 << (7 - (x1 - 1) % 8);
    if b0 == b1 {
        row[b0] |= m0 & m1;
    } else {
        row[b0] |= m0;
        row[b0 + 1..b1].fill(0xff);
        row[b1] |= m1;
    }
}

/// Write the black runs of a changing-element list (`[b0, w0, b1, w1, ...]`).
fn paint(row: &mut [u8], changes: &[u32], w: u32) {
    for pair in changes.chunks(2) {
        let x0 = pair[0].min(w) as usize;
        let x1 = pair.get(1).copied().unwrap_or(w).min(w) as usize;
        set_bits(row, x0, x1);
    }
}

/// One 1D (Modified Huffman) row into `cur`.
fn row_1d(bits: &mut Bits, w: u32, cur: &mut Vec<u32>) -> bool {
    cur.clear();
    let mut a0 = 0u32;
    let mut black = false;
    while a0 < w {
        let Some(r) = run(bits, black) else {
            return false;
        };
        a0 = (a0 + r).min(w);
        cur.push(a0);
        black = !black;
    }
    // Rows end white: the list holds pairs.
    if cur.len() % 2 == 1 {
        cur.push(w);
    }
    true
}

/// One 2D (READ) row against the reference row `refl` (with sentinels) into `cur`.
fn row_2d(bits: &mut Bits, w: u32, refl: &[u32], cur: &mut Vec<u32>) -> bool {
    let t = tables();
    cur.clear();
    let wi = w as i64;
    let mut a0: i64 = -1;
    let mut black = false;
    let mut bi = 0usize;
    // The last valid index of `refl` (sentinels included).
    let last = refl.len() - 1;
    let advance = |bi: &mut usize, a0: i64| {
        while *bi + 2 <= last && (refl[*bi] as i64) <= a0 && (refl[*bi] as i64) < wi {
            *bi += 2;
        }
    };
    while a0 < wi {
        bits.refill();
        let (m, len) = t.mode[bits.peek(MODE_BITS) as usize];
        // b1: the first changing element right of a0 of the colour opposite a0's.
        advance(&mut bi, a0);
        let b1 = refl[bi] as i64;
        match m {
            M2::V(d) => {
                bits.skip(len as u32);
                let a1 = b1 + d as i64;
                if a1 < a0.max(0) || a1 > wi {
                    return false;
                }
                cur.push(a1 as u32);
                a0 = a1;
                black = !black;
                // The next b1 has the other parity; it can be the element before this b1.
                bi = if bi > 0 { bi - 1 } else { 1 };
            }
            M2::Pass => {
                bits.skip(len as u32);
                let b2 = refl[(bi + 1).min(last)] as i64;
                a0 = b2;
                bi += 2;
            }
            M2::Horizontal => {
                bits.skip(len as u32);
                let (Some(r1), Some(r2)) = (run(bits, black), run(bits, !black)) else {
                    return false;
                };
                let a1 = (a0.max(0) + r1 as i64).min(wi);
                let a2 = (a1 + r2 as i64).min(wi);
                cur.push(a1 as u32);
                cur.push(a2 as u32);
                a0 = a2;
            }
            M2::Ext | M2::Invalid => return false,
        }
        bi = bi.min(last);
    }
    if cur.len() % 2 == 1 {
        cur.push(w);
    }
    true
}

/// Decode `rows` rows of `w` pixels into `out` (`rb` bytes per row, zeroed by the caller).
/// Returns the bytes of complete rows written and whether damaged rows were skipped over (G3
/// resynchronises at the next EOL; the others stop at the first error).
pub(crate) fn decode(
    input: &[u8],
    mode: Mode,
    w: u32,
    rows: usize,
    out: &mut [u8],
) -> (usize, bool) {
    let rb = (w as usize).div_ceil(8);
    if w == 0 || rb == 0 {
        return (0, false);
    }
    let rows = rows.min(out.len() / rb);
    let mut bits = Bits::new(input);
    // Reference row (all white to start) with sentinels, and the row being decoded.
    let mut refl: Vec<u32> = vec![w, w, w];
    let mut cur: Vec<u32> = Vec::with_capacity(64);
    let mut damaged = false;
    let sentinel = |v: &mut Vec<u32>| v.extend_from_slice(&[w, w, w]);
    for y in 0..rows {
        let row = &mut out[y * rb..(y + 1) * rb];
        let ok = match mode {
            Mode::Rle | Mode::RleWord => {
                let ok = row_1d(&mut bits, w, &mut cur);
                bits.align(if mode == Mode::Rle { 8 } else { 16 });
                ok
            }
            Mode::G3 { two_d } => {
                // EOLs (each with its tag bit in 2D files); two or more in a row are the
                // RTC that ends the data.
                let mut eols = 0;
                while bits.eol() {
                    eols += 1;
                    if two_d && bits.peek(13) == 0x1001 {
                        bits.skip(1);
                    }
                }
                if eols >= 2 || bits.past_end() {
                    return (y * rb, damaged);
                }
                let one_d = if two_d {
                    bits.refill();
                    let tag = bits.peek(1);
                    bits.skip(1);
                    tag == 1
                } else {
                    true
                };
                if one_d {
                    row_1d(&mut bits, w, &mut cur)
                } else {
                    row_2d(&mut bits, w, &refl, &mut cur)
                }
            }
            Mode::G4 => row_2d(&mut bits, w, &refl, &mut cur),
        };
        if bits.past_end() {
            // Ran out of data inside the row.
            return (y * rb, true);
        }
        if !ok {
            match mode {
                Mode::G3 { .. } => {
                    // Keep what the row decoded, then pick up at the next EOL.
                    damaged = true;
                    if cur.len() % 2 == 1 {
                        cur.push(w);
                    }
                    paint(row, &cur, w);
                    refl.clear();
                    refl.extend_from_slice(&cur);
                    sentinel(&mut refl);
                    if !bits.seek_eol() {
                        return ((y + 1) * rb, true);
                    }
                    // `seek_eol` consumed the EOL the next row starts with; the next row's
                    // EOL check finds none, which is fine.
                    continue;
                }
                _ => return (y * rb, true),
            }
        }
        paint(row, &cur, w);
        std::mem::swap(&mut refl, &mut cur);
        sentinel(&mut refl);
    }
    (rows * rb, damaged)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first bit writer for building test streams.
    #[derive(Default)]
    struct W {
        out: Vec<u8>,
        acc: u32,
        n: u32,
    }

    impl W {
        fn put(&mut self, code: &str) {
            for c in code.bytes() {
                self.acc = (self.acc << 1) | (c == b'1') as u32;
                self.n += 1;
                if self.n == 8 {
                    self.out.push(self.acc as u8);
                    self.acc = 0;
                    self.n = 0;
                }
            }
        }
        fn run(&mut self, black: bool, mut r: u32) {
            let (term, makeup) = if black {
                (&BLACK_TERM, &BLACK_MAKEUP)
            } else {
                (&WHITE_TERM, &WHITE_MAKEUP)
            };
            while r >= 2560 {
                self.put(EXT_MAKEUP[12]);
                r -= 2560;
            }
            if r >= 1792 {
                self.put(EXT_MAKEUP[(r as usize - 1792) / 64]);
                r %= 64;
            } else if r >= 64 {
                self.put(makeup[r as usize / 64 - 1]);
                r %= 64;
            }
            self.put(term[r as usize]);
        }
        fn finish(mut self) -> Vec<u8> {
            while self.n != 0 {
                self.put("0");
            }
            self.out
        }
    }

    #[test]
    fn tables_are_prefix_free() {
        // Building them checks overlaps in debug builds; every slot of a full code is set.
        let t = tables();
        assert!(t.white.iter().filter(|c| c.kind != INVALID).count() > 3000);
        assert_eq!(t.white[1].kind, EOL);
        assert_eq!(t.black[2].kind, EOL);
    }

    fn row_bits(row: &[u8], w: usize) -> Vec<bool> {
        (0..w)
            .map(|x| row[x / 8] & (0x80 >> (x % 8)) != 0)
            .collect()
    }

    #[test]
    fn mh_rows() {
        // Two 3000-pixel rows: runs that need extended makeup codes.
        let w = 3000u32;
        let runs: [&[u32]; 2] = [&[10, 2000, 990], &[0, 1, 2599, 400]];
        let mut wr = W::default();
        for r in runs {
            for (i, &n) in r.iter().enumerate() {
                wr.run(i % 2 == 1, n);
            }
            while wr.n != 0 {
                wr.put("0");
            }
        }
        let data = wr.finish();
        let rb = 375;
        let mut out = vec![0u8; rb * 2];
        assert_eq!(decode(&data, Mode::Rle, w, 2, &mut out), (rb * 2, false));
        for (y, r) in runs.iter().enumerate() {
            let got = row_bits(&out[y * rb..(y + 1) * rb], w as usize);
            let mut want = Vec::new();
            for (i, &n) in r.iter().enumerate() {
                want.extend(std::iter::repeat_n(i % 2 == 1, n as usize));
            }
            assert_eq!(got, want, "row {y}");
        }
    }

    #[test]
    fn g4_modes() {
        // Row 0: horizontal (white 2, black 3), then V0 against the all-white reference
        // ends the row; row 1: V0, VR1 (black run 2..6), VL... built by hand.
        let w = 16u32;
        let mut wr = W::default();
        // Row 0: H(2 white, 3 black) -> changes 2, 5; then V0: b1 = w -> a1 = 16.
        wr.put("001");
        wr.run(false, 2);
        wr.run(true, 3);
        wr.put("1");
        // Row 1: reference changes [2, 5]. V0 -> a1 = 2; VR1 -> b1 = 5, a1 = 6; V0 -> w.
        wr.put("1");
        wr.put("011");
        wr.put("1");
        // Row 2: reference [2, 6]. Pass (a0 = 6), then H(white 4, black 2) -> 10, 12; V0.
        wr.put("0001");
        wr.put("001");
        wr.run(false, 4);
        wr.run(true, 2);
        wr.put("1");
        let data = wr.finish();
        let mut out = vec![0u8; 2 * 3];
        assert_eq!(decode(&data, Mode::G4, w, 3, &mut out), (6, false));
        assert_eq!(&out[0..2], &[0b0011_1000, 0]);
        assert_eq!(&out[2..4], &[0b0011_1100, 0]);
        assert_eq!(&out[4..6], &[0, 0b0011_0000]);
    }

    #[test]
    fn g3_eol_resync() {
        // Two 1D rows with EOLs and fill bits; the first row is garbage and is skipped to
        // the next EOL.
        let w = 8u32;
        let mut wr = W::default();
        wr.put("0000000000001");
        wr.put("0000000010"); // invalid in the white table
        wr.put("0000000000001"); // EOL with a fill bit
        wr.run(false, 4);
        wr.run(true, 4);
        let data = wr.finish();
        let mut out = vec![0u8; 2];
        let (n, damaged) = decode(&data, Mode::G3 { two_d: false }, w, 2, &mut out);
        assert_eq!(n, 2);
        assert!(damaged);
        assert_eq!(out[1], 0x0f);
    }

    #[test]
    fn garbage_does_not_panic() {
        let mut s = 7u32;
        for len in [0usize, 1, 3, 17, 400] {
            let data: Vec<u8> = (0..len)
                .map(|_| {
                    s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    (s >> 16) as u8
                })
                .collect();
            for mode in [Mode::Rle, Mode::RleWord, Mode::G3 { two_d: true }, Mode::G4] {
                let mut out = vec![0u8; 13 * 40];
                let _ = decode(&data, mode, 100, 40, &mut out);
            }
        }
    }
}

//! Brotli decompression (RFC 7932), for the Brotli-compressed boxes (`brob`) of JPEG XL
//! containers (ISO/IEC 18181-2).
//!
//! The whole stream is in memory and so is the output: back-references copy from the output
//! itself, so there is no separate ring buffer. Every meta-block declares its length before
//! any of its data, so the output cap is enforced before anything past it is allocated or
//! decoded. Hostile input gives an [`Error`], never a panic.
//!
//! Upstream encoders and decoders are test and benchmark oracles only (dev-dependencies).

mod bits;
mod dict;
mod huffman;

use bits::BitReader;
use huffman::Huffman;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Brotli stream cut short")]
    Truncated,
    #[error("invalid Brotli stream: {0}")]
    Invalid(&'static str),
    #[error("Brotli output larger than {0} bytes")]
    TooLarge(usize),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Decompresses a complete Brotli stream into at most `max_out` bytes; a stream that would
/// produce more is an [`Error::TooLarge`]. Bytes after the end of the stream are ignored.
pub fn decompress(input: &[u8], max_out: usize) -> Result<Vec<u8>> {
    Decoder::new(input, max_out).run()
}

/// Insert length codes (§5): base and extra bits.
const INSERT: [(u32, u8); 24] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 1),
    (8, 1),
    (10, 2),
    (14, 2),
    (18, 3),
    (26, 3),
    (34, 4),
    (50, 4),
    (66, 5),
    (98, 5),
    (130, 6),
    (194, 7),
    (322, 8),
    (578, 9),
    (1090, 10),
    (2114, 12),
    (6210, 14),
    (22594, 24),
];

/// Copy length codes (§5): base and extra bits.
const COPY: [(u32, u8); 24] = [
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 1),
    (12, 1),
    (14, 2),
    (18, 2),
    (22, 3),
    (30, 3),
    (38, 4),
    (54, 4),
    (70, 5),
    (102, 5),
    (134, 6),
    (198, 7),
    (326, 8),
    (582, 9),
    (1094, 10),
    (2118, 24),
];

/// Block count codes (§6): base and extra bits.
const BLOCK_COUNT: [(u32, u8); 26] = [
    (1, 2),
    (5, 2),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 3),
    (41, 3),
    (49, 4),
    (65, 4),
    (81, 4),
    (97, 4),
    (113, 5),
    (145, 5),
    (177, 5),
    (209, 5),
    (241, 6),
    (305, 6),
    (369, 7),
    (497, 8),
    (753, 9),
    (1265, 10),
    (2289, 11),
    (4337, 12),
    (8433, 13),
    (16625, 24),
];

/// An insert-and-copy length code split up (§5).
#[derive(Clone, Copy, Default)]
struct Command {
    insert_base: u32,
    insert_extra: u8,
    copy_base: u32,
    copy_extra: u8,
    /// The distance is the last one, not coded.
    implicit_zero: bool,
}

const COMMANDS: [Command; 704] = {
    // Per 64-code cell: the first insert and copy length codes.
    const INS: [usize; 11] = [0, 0, 0, 0, 8, 8, 0, 16, 8, 16, 16];
    const CPY: [usize; 11] = [0, 8, 0, 8, 0, 8, 16, 0, 16, 8, 16];
    let mut t = [Command {
        insert_base: 0,
        insert_extra: 0,
        copy_base: 0,
        copy_extra: 0,
        implicit_zero: false,
    }; 704];
    let mut code = 0;
    while code < 704 {
        let cell = code >> 6;
        let i = INS[cell] + ((code >> 3) & 7);
        let c = CPY[cell] + (code & 7);
        t[code] = Command {
            insert_base: INSERT[i].0,
            insert_extra: INSERT[i].1,
            copy_base: COPY[c].0,
            copy_extra: COPY[c].1,
            implicit_zero: code < 128,
        };
        code += 1;
    }
    t
};

/// Literal context lookup (§7.1): per context mode, a table for p1 and one for p2, OR-ed.
const CONTEXT: [[[u8; 256]; 2]; 4] = {
    #[rustfmt::skip]
    const LUT0: [u8; 128] = [
         0,  0,  0,  0,  0,  0,  0,  0,  0,  4,  4,  0,  0,  4,  0,  0,
         0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
         8, 12, 16, 12, 12, 20, 12, 16, 24, 28, 12, 12, 32, 12, 36, 12,
        44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 32, 32, 24, 40, 28, 12,
        12, 48, 52, 52, 52, 48, 52, 52, 52, 48, 52, 52, 52, 52, 52, 48,
        52, 52, 52, 52, 52, 48, 52, 52, 52, 52, 52, 24, 12, 28, 12, 12,
        12, 56, 60, 60, 60, 56, 60, 60, 60, 56, 60, 60, 60, 60, 60, 56,
        60, 60, 60, 60, 60, 56, 60, 60, 60, 60, 60, 24, 12, 28, 12,  0,
    ];
    #[rustfmt::skip]
    const LUT1: [u8; 128] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1,
        1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1,
        1, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
        3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 1, 1, 1, 1, 0,
    ];
    let mut t = [[[0u8; 256]; 2]; 4];
    let mut b = 0;
    while b < 256 {
        // LSB6, MSB6.
        t[0][0][b] = (b & 0x3F) as u8;
        t[1][0][b] = (b >> 2) as u8;
        // UTF8: above 127, Lut0 alternates 0/1 below 192 and 2/3 from there; Lut1 is 2 for
        // bytes from 224 and 0 below.
        if b < 128 {
            t[2][0][b] = LUT0[b];
            t[2][1][b] = LUT1[b];
        } else {
            t[2][0][b] = (b & 1) as u8 + if b < 192 { 0 } else { 2 };
            t[2][1][b] = if b < 224 { 0 } else { 2 };
        }
        // Signed (Lut2).
        let lut2 = match b {
            0 => 0,
            1..=15 => 1,
            16..=63 => 2,
            64..=127 => 3,
            128..=191 => 4,
            192..=239 => 5,
            240..=254 => 6,
            _ => 7,
        };
        t[3][0][b] = lut2 << 3;
        t[3][1][b] = lut2;
        b += 1;
    }
    t
};

/// Block switching state of one category (§6).
struct Blocks {
    ntypes: usize,
    types: Huffman,
    counts: Huffman,
    /// Current and previous block type.
    cur: usize,
    prev: usize,
    /// Commands (or literals, distances) left in the current block.
    left: u32,
}

impl Blocks {
    fn read(br: &mut BitReader<'_>) -> Result<Self> {
        let ntypes = var_u8(br)? + 1;
        if ntypes < 2 {
            return Ok(Self {
                ntypes,
                types: Huffman::default(),
                counts: Huffman::default(),
                cur: 0,
                prev: 1,
                // Never runs out: a meta-block has fewer than 2^24 commands.
                left: u32::MAX,
            });
        }
        let types = Huffman::read_code(br, ntypes + 2)?;
        let counts = Huffman::read_code(br, 26)?;
        let left = block_count(&counts, br)?;
        Ok(Self {
            ntypes,
            types,
            counts,
            cur: 0,
            prev: 1,
            left,
        })
    }

    /// Reads a block switch command.
    #[cold]
    fn switch(&mut self, br: &mut BitReader<'_>) -> Result<()> {
        let t = match self.types.read(br)? as usize {
            0 => self.prev,
            1 => (self.cur + 1) % self.ntypes,
            s => s - 2,
        };
        self.prev = self.cur;
        self.cur = t;
        self.left = block_count(&self.counts, br)?;
        Ok(())
    }

    /// Counts one item of this category, switching blocks first when the current one is done.
    #[inline(always)]
    fn next(&mut self, br: &mut BitReader<'_>) -> Result<()> {
        if self.left == 0 {
            self.switch(br)?;
        }
        self.left -= 1;
        Ok(())
    }
}

fn block_count(code: &Huffman, br: &mut BitReader<'_>) -> Result<u32> {
    let (base, extra) = BLOCK_COUNT[usize::from(code.read(br)?).min(25)];
    Ok(base + br.read(u32::from(extra))?)
}

/// The variable-length code of NBLTYPES, NTREES (§9.2): 0..=255.
fn var_u8(br: &mut BitReader<'_>) -> Result<usize> {
    if !br.bit()? {
        return Ok(0);
    }
    let n = br.read(3)?;
    if n == 0 {
        return Ok(1);
    }
    Ok((1 << n) + br.read(n)? as usize)
}

/// A context map (§7.3) of `size` entries with values below `ntrees`.
fn context_map(br: &mut BitReader<'_>, size: usize, ntrees: usize) -> Result<Vec<u8>> {
    let mut map = vec![0u8; size];
    if ntrees < 2 {
        return Ok(map);
    }
    let rle_max = if br.bit()? {
        br.read(4)? as usize + 1
    } else {
        0
    };
    let code = Huffman::read_code(br, ntrees + rle_max)?;
    let mut i = 0;
    while i < size {
        let s = usize::from(code.read(br)?);
        if s == 0 {
            i += 1;
        } else if s <= rle_max {
            let run = (1 << s) + br.read(s as u32)? as usize;
            if i + run > size {
                return Err(Error::Invalid("context map run past the end"));
            }
            i += run;
        } else {
            // s - rle_max < ntrees <= 256.
            map[i] = (s - rle_max) as u8;
            i += 1;
        }
    }
    if br.bit()? {
        // Inverse move-to-front.
        let mut mtf: [u8; 256] = std::array::from_fn(|i| i as u8);
        for v in &mut map {
            let i = usize::from(*v);
            let value = mtf[i];
            *v = value;
            mtf.copy_within(..i, 1);
            mtf[0] = value;
        }
    }
    if map.iter().any(|&v| usize::from(v) >= ntrees) {
        return Err(Error::Invalid("context map value"));
    }
    Ok(map)
}

fn read_codes(br: &mut BitReader<'_>, n: usize, size: usize) -> Result<Vec<Huffman>> {
    (0..n).map(|_| Huffman::read_code(br, size)).collect()
}

struct Decoder<'a> {
    br: BitReader<'a>,
    out: Vec<u8>,
    max_out: usize,
    window: usize,
    /// The last four distances, most recent at `dist_i - 1`.
    dist: [usize; 4],
    dist_i: usize,
}

impl<'a> Decoder<'a> {
    fn new(input: &'a [u8], max_out: usize) -> Self {
        Self {
            br: BitReader::new(input),
            out: Vec::new(),
            max_out,
            window: 0,
            dist: [16, 15, 11, 4],
            dist_i: 4,
        }
    }

    fn run(mut self) -> Result<Vec<u8>> {
        self.window = (1 << self.window_bits()?) - 16;
        loop {
            let last = self.br.bit()?;
            if last && self.br.bit()? {
                // ISLASTEMPTY.
                break;
            }
            let nibbles = match self.br.read(2)? {
                3 => 0,
                n => n + 4,
            };
            if nibbles == 0 {
                self.metadata()?;
                if last {
                    break;
                }
                continue;
            }
            let mlen = self.br.read(nibbles * 4)? as usize;
            if nibbles > 4 && mlen >> ((nibbles - 1) * 4) == 0 {
                return Err(Error::Invalid(
                    "meta-block length with a leading zero nibble",
                ));
            }
            let mlen = mlen + 1;
            if self.out.len() + mlen > self.max_out {
                return Err(Error::TooLarge(self.max_out));
            }
            if !last && self.br.bit()? {
                self.br.align()?;
                let raw = self.br.bytes(mlen)?;
                self.out.extend_from_slice(raw);
                continue;
            }
            self.compressed(mlen)?;
            if last {
                break;
            }
        }
        // The rest of the last byte must be zero.
        self.br.align()?;
        Ok(self.out)
    }

    /// WBITS (§9.1).
    fn window_bits(&mut self) -> Result<u32> {
        let br = &mut self.br;
        if !br.bit()? {
            return Ok(16);
        }
        let n = br.read(3)?;
        if n != 0 {
            return Ok(17 + n);
        }
        match br.read(3)? {
            0 => Ok(17),
            1 => Err(Error::Invalid("window size")),
            m => Ok(8 + m),
        }
    }

    /// A metadata meta-block: skipped.
    fn metadata(&mut self) -> Result<()> {
        if self.br.bit()? {
            return Err(Error::Invalid("reserved bit set"));
        }
        let nbytes = self.br.read(2)?;
        let mut len = 0usize;
        if nbytes > 0 {
            len = self.br.read(nbytes * 8)? as usize;
            if nbytes > 1 && len >> ((nbytes - 1) * 8) == 0 {
                return Err(Error::Invalid("metadata length with a leading zero byte"));
            }
            len += 1;
        }
        self.br.align()?;
        self.br.bytes(len)?;
        Ok(())
    }

    fn compressed(&mut self, mlen: usize) -> Result<()> {
        let br = &mut self.br;
        let mut lit_blocks = Blocks::read(br)?;
        let mut cmd_blocks = Blocks::read(br)?;
        let mut dist_blocks = Blocks::read(br)?;
        let npostfix = br.read(2)?;
        let ndirect = (br.read(4)? << npostfix) as usize;
        let mut modes = Vec::with_capacity(lit_blocks.ntypes);
        for _ in 0..lit_blocks.ntypes {
            modes.push(br.read(2)? as usize);
        }
        let ntrees_lit = var_u8(br)? + 1;
        let lit_map = context_map(br, 64 * lit_blocks.ntypes, ntrees_lit)?;
        let ntrees_dist = var_u8(br)? + 1;
        let dist_map = context_map(br, 4 * dist_blocks.ntypes, ntrees_dist)?;
        let lit_codes = read_codes(br, ntrees_lit, 256)?;
        let cmd_codes = read_codes(br, cmd_blocks.ntypes, 704)?;
        let dist_size = 16 + ndirect + (48 << npostfix);
        let dist_codes = read_codes(br, ntrees_dist, dist_size)?;

        let out = &mut self.out;
        out.reserve(mlen);
        let end = out.len() + mlen;
        let (mut p1, mut p2) = match out.as_slice() {
            [.., a, b] => (*b, *a),
            [b] => (*b, 0),
            [] => (0, 0),
        };
        let mut lit_ctx = &CONTEXT[modes[0]];
        let mut lit_trees = &lit_map[..64];
        loop {
            cmd_blocks.next(br)?;
            let cmd = COMMANDS[usize::from(cmd_codes[cmd_blocks.cur].read(br)?)];
            let insert = cmd.insert_base as usize + br.read(u32::from(cmd.insert_extra))? as usize;
            let copy = cmd.copy_base as usize + br.read(u32::from(cmd.copy_extra))? as usize;
            if insert > end - out.len() {
                return Err(Error::Invalid("insert length past the meta-block"));
            }
            for _ in 0..insert {
                if lit_blocks.left == 0 {
                    lit_blocks.switch(br)?;
                    lit_ctx = &CONTEXT[modes[lit_blocks.cur]];
                    let t = 64 * lit_blocks.cur;
                    lit_trees = &lit_map[t..t + 64];
                }
                lit_blocks.left -= 1;
                let ctx = lit_ctx[0][usize::from(p1)] | lit_ctx[1][usize::from(p2)];
                let tree = &lit_codes[usize::from(lit_trees[usize::from(ctx)])];
                let b = tree.read(br)? as u8;
                out.push(b);
                p2 = p1;
                p1 = b;
            }
            if out.len() == end {
                break;
            }

            let distance = if cmd.implicit_zero {
                self.dist[(self.dist_i - 1) & 3]
            } else {
                dist_blocks.next(br)?;
                let ctx = if copy > 4 { 3 } else { copy - 2 };
                let tree = usize::from(dist_map[4 * dist_blocks.cur + ctx]);
                let code = usize::from(dist_codes[tree].read(br)?);
                let d = match code {
                    0..16 => {
                        // Last distances, possibly adjusted.
                        const BACK: [usize; 16] = [1, 2, 3, 4, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2];
                        const DELTA: [isize; 16] =
                            [0, 0, 0, 0, -1, 1, -2, 2, -3, 3, -1, 1, -2, 2, -3, 3];
                        let d = self.dist[(self.dist_i - BACK[code]) & 3] as isize + DELTA[code];
                        if d <= 0 {
                            return Err(Error::Invalid("distance below one"));
                        }
                        d as usize
                    }
                    _ if code < 16 + ndirect => code - 15,
                    _ => {
                        let c = code - ndirect - 16;
                        let nbits = 1 + (c >> (npostfix + 1)) as u32;
                        let hcode = c >> npostfix;
                        let lcode = c & ((1 << npostfix) - 1);
                        let offset = ((2 + (hcode & 1)) << nbits) - 4;
                        let extra = br.read(nbits)? as usize;
                        ((offset + extra) << npostfix) + lcode + ndirect + 1
                    }
                };
                let max = self.window.min(out.len());
                if code != 0 && d <= max {
                    self.dist[self.dist_i & 3] = d;
                    self.dist_i += 1;
                }
                d
            };

            let room = end - out.len();
            let max = self.window.min(out.len());
            if distance > max {
                dict::push_word(out, copy, distance - max - 1, room)?;
            } else {
                if copy > room {
                    return Err(Error::Invalid("copy length past the meta-block"));
                }
                let start = out.len() - distance;
                if distance >= copy {
                    out.extend_from_within(start..start + copy);
                } else {
                    // Overlapping: the source grows as it is copied.
                    let mut done = 0;
                    while done < copy {
                        let n = (copy - done).min(out.len() - start - done);
                        out.extend_from_within(start + done..start + done + n);
                        done += n;
                    }
                }
            }
            match out.as_slice() {
                [.., a, b] => (p1, p2) = (*b, *a),
                [b] => (p1, p2) = (*b, 0),
                [] => {}
            }
            if out.len() == end {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_tables_check_values() {
        // RFC 7932 §7.1 gives CRC-32s of Lut0, Lut1 and Lut2.
        let lut0: Vec<u8> = CONTEXT[2][0].to_vec();
        let lut1: Vec<u8> = CONTEXT[2][1].to_vec();
        let lut2: Vec<u8> = CONTEXT[3][1].to_vec();
        let crc = |v: &[u8]| {
            let mut crc = 0xFFFF_FFFFu32;
            for &b in v {
                let mut c = (crc ^ u32::from(b)) & 0xFF;
                for _ in 0..8 {
                    c = if c & 1 != 0 {
                        0xEDB8_8320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
                crc = c ^ (crc >> 8);
            }
            crc ^ 0xFFFF_FFFF
        };
        assert_eq!(crc(&lut0), 0x8E91_EFB7);
        assert_eq!(crc(&lut1), 0xD01A_32F4);
        assert_eq!(crc(&lut2), 0x0DD7_A0D6);
    }

    #[test]
    fn tiny_streams() {
        // The empty stream: WBITS 16 (one 0 bit), ISLAST, ISLASTEMPTY.
        assert_eq!(decompress(&[0b110], 0).unwrap(), b"");
        // Non-zero padding after it.
        assert!(matches!(decompress(&[0b1110], 0), Err(Error::Invalid(_))));
        assert_eq!(decompress(&[], 10), Err(Error::Truncated));
        // An invalid window size (0010001).
        assert!(matches!(
            decompress(&[0b0010001], 10),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn command_table() {
        let c = COMMANDS[0];
        assert!(c.implicit_zero && c.insert_base == 0 && c.copy_base == 2);
        // 703: insert code 23, copy code 23.
        let c = COMMANDS[703];
        assert!(!c.implicit_zero && c.insert_base == 22594 && c.copy_base == 2118);
        // 200 = cell 3 (insert 0..7, copy 8..15): insert code 1, copy code 8.
        let c = COMMANDS[200];
        assert_eq!((c.insert_base, c.copy_base, c.copy_extra), (1, 10, 1));
    }
}

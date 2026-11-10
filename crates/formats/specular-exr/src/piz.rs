//! PIZ: bitmap range reduction + Huffman + Haar-like wavelet (OpenEXR `ImfPizCompressor`,
//! `ImfHuf`, `ImfWav`), written from the format description.

use crate::codec::BlockGeom;

const USHORT_RANGE: usize = 1 << 16;
const BITMAP_SIZE: usize = USHORT_RANGE >> 3;
const HUF_ENCBITS: u32 = 16;
const HUF_DECBITS: i32 = 14;
const HUF_ENCSIZE: usize = (1 << HUF_ENCBITS) + 1;
const HUF_DECSIZE: usize = 1 << HUF_DECBITS;
const HUF_DECMASK: u64 = HUF_DECSIZE as u64 - 1;
const SHORT_ZEROCODE_RUN: u64 = 59;
const LONG_ZEROCODE_RUN: u64 = 63;
const SHORTEST_LONG_RUN: u64 = 2 + LONG_ZEROCODE_RUN - SHORT_ZEROCODE_RUN;

/// Reusable buffers (the tables are large; one set per thread).
pub(crate) struct PizScratch {
    lut: Vec<u16>,
    /// Entries of `lut` that may be non-zero.
    lut_len: usize,
    words: Vec<u16>,
    /// Encoding table: `code << 6 | length`.
    hcode: Vec<u64>,
    /// Primary decode table: `len << 24 | symbol` for short codes, 0 if empty / long.
    dec: Vec<u32>,
    /// Long codes: for primary entry `i`, symbols `long_syms[long_start[i]..long_start[i + 1]]`.
    long_start: Vec<u32>,
    long_syms: Vec<u32>,
    /// Symbols with a non-zero code length, ascending (table passes touch only these).
    syms: Vec<u32>,
    /// Fill cursors for `long_syms`.
    cursor: Vec<u32>,
}

impl Default for PizScratch {
    fn default() -> Self {
        PizScratch {
            lut: vec![0; USHORT_RANGE],
            lut_len: 0,
            words: Vec::new(),
            hcode: vec![0; HUF_ENCSIZE],
            dec: vec![0; HUF_DECSIZE],
            long_start: vec![0; HUF_DECSIZE + 1],
            long_syms: Vec::new(),
            syms: Vec::new(),
            cursor: Vec::new(),
        }
    }
}

fn u16_at(d: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

/// Decompress a PIZ chunk into `out` (planar layout, `g.total` bytes).
pub(crate) fn decompress(
    input: &[u8],
    g: &BlockGeom,
    s: &mut PizScratch,
    out: &mut [u8],
) -> Option<()> {
    if !g.total.is_multiple_of(2) {
        return None;
    }
    let min_nz = u16_at(input, 0)? as usize;
    let max_nz = u16_at(input, 2)? as usize;
    let mut pos = 4;
    // Reverse LUT from the bitmap: value 0, then every set bit in ascending order.
    s.lut[0] = 0;
    let mut k = 1usize;
    if min_nz <= max_nz {
        if max_nz >= BITMAP_SIZE {
            return None;
        }
        let n = max_nz - min_nz + 1;
        let bitmap = input.get(pos..pos + n)?;
        pos += n;
        for (j, &byte) in bitmap.iter().enumerate() {
            let at = (min_nz + j) * 8;
            let mut b = if at == 0 { byte & !1 } else { byte };
            while b != 0 {
                s.lut[k] = (at + b.trailing_zeros() as usize) as u16;
                k += 1;
                b &= b - 1;
            }
        }
    }
    let max_value = (k - 1) as u16;
    // Entries past `k` must read as 0 (corrupt words may index them); clear what the previous
    // block left there.
    if s.lut_len > k {
        s.lut[k..s.lut_len].fill(0);
    }
    s.lut_len = k;

    let len = u32_at(input, pos)? as usize;
    pos += 4;
    let huf = input.get(pos..pos.checked_add(len)?)?;
    let n_words = g.total / 2;
    s.words.clear();
    s.words.resize(n_words, 0);
    let PizScratch {
        hcode,
        dec,
        long_start,
        long_syms,
        syms,
        cursor,
        words,
        lut,
        ..
    } = s;
    let mut t = HufTables {
        hcode,
        dec,
        long_start,
        long_syms,
        syms,
        cursor,
    };
    huf_uncompress(huf, words, &mut t)?;

    // Wavelet, per channel and per 16-bit word of the sample.
    let mut base = 0usize;
    for ch in &g.chans {
        let size = ch.bytes / 2;
        let n = ch.nx * ch.ny * size;
        let plane = words.get_mut(base..base + n)?;
        for j in 0..size {
            wav2_decode(plane, j, ch.nx, size, ch.ny, ch.nx * size, max_value);
        }
        base += n;
    }
    for (o, &w) in out.chunks_exact_mut(2).zip(words.iter()) {
        o.copy_from_slice(&lut[w as usize].to_le_bytes());
    }
    Some(())
}

// ---------------------------------------------------------------------------------------------
// Wavelet

#[inline]
fn wdec14(l: u16, h: u16) -> (u16, u16) {
    let ls = l as i16 as i32;
    let hi = h as i16 as i32;
    let ai = ls + (hi & 1) + (hi >> 1);
    let a = ai as i16;
    let b = (ai - hi) as i16;
    (a as u16, b as u16)
}

#[inline]
fn wdec16(l: u16, h: u16) -> (u16, u16) {
    const A_OFFSET: i32 = 1 << 15;
    const MOD_MASK: i32 = (1 << 16) - 1;
    let m = l as i32;
    let d = h as i32;
    let bb = (m - (d >> 1)) & MOD_MASK;
    let aa = (d + bb - A_OFFSET) & MOD_MASK;
    (aa as u16, bb as u16)
}

/// Inverse 2D wavelet over an `nx` x `ny` grid starting at `buf[start]` with element stride
/// `ox` and row stride `oy`. Indices are always in bounds for a grid that fits in `buf`.
fn wav2_decode(buf: &mut [u16], start: usize, nx: usize, ox: usize, ny: usize, oy: usize, mx: u16) {
    let w14 = mx < (1 << 14);
    let dec = |a: u16, b: u16| if w14 { wdec14(a, b) } else { wdec16(a, b) };
    let n = nx.min(ny);
    let mut p = 1usize;
    while p <= n {
        p <<= 1;
    }
    p >>= 1;
    let mut p2 = p;
    p >>= 1;
    while p >= 1 {
        let oy1 = oy * p;
        let oy2 = oy * p2;
        let ox1 = ox * p;
        let ox2 = ox * p2;
        let ey = start + oy * (ny - p2);
        let mut py = start;
        while py <= ey {
            let mut px = py;
            let ex = py + ox * (nx - p2);
            while px <= ex {
                let p01 = px + ox1;
                let p10 = px + oy1;
                let p11 = p10 + ox1;
                let (i00, i10) = dec(buf[px], buf[p10]);
                let (i01, i11) = dec(buf[p01], buf[p11]);
                let (a, b) = dec(i00, i01);
                buf[px] = a;
                buf[p01] = b;
                let (a, b) = dec(i10, i11);
                buf[p10] = a;
                buf[p11] = b;
                px += ox2;
            }
            if nx & p != 0 {
                let p10 = px + oy1;
                let (i00, b) = dec(buf[px], buf[p10]);
                buf[p10] = b;
                buf[px] = i00;
            }
            py += oy2;
        }
        if ny & p != 0 {
            let mut px = py;
            let ex = py + ox * (nx - p2);
            while px <= ex {
                let p01 = px + ox1;
                let (i00, b) = dec(buf[px], buf[p01]);
                buf[p01] = b;
                buf[px] = i00;
                px += ox2;
            }
        }
        p2 = p;
        p >>= 1;
    }
}

// ---------------------------------------------------------------------------------------------
// Huffman

/// MSB-first bit reader state as in `ImfHuf.cpp`: `c` holds `lc` valid low bits.
struct Bits<'a> {
    d: &'a [u8],
    p: usize,
    c: u64,
    lc: i32,
}

impl Bits<'_> {
    #[inline]
    fn get_char(&mut self) -> Option<()> {
        let b = *self.d.get(self.p)?;
        self.p += 1;
        self.c = (self.c << 8) | b as u64;
        self.lc += 8;
        Some(())
    }
    fn get_bits(&mut self, n: i32) -> Option<u64> {
        while self.lc < n {
            self.get_char()?;
        }
        self.lc -= n;
        Some((self.c >> self.lc) & ((1u64 << n) - 1))
    }
}

/// Decode OpenEXR's static Huffman stream (as PIZ stores it) into exactly `out.len()` symbols.
/// DWAA/DWAB code their AC coefficients with it.
pub(crate) fn huf_decode(input: &[u8], out: &mut [u16], s: &mut PizScratch) -> Option<()> {
    let PizScratch {
        hcode,
        dec,
        long_start,
        long_syms,
        syms,
        cursor,
        ..
    } = s;
    let mut t = HufTables {
        hcode,
        dec,
        long_start,
        long_syms,
        syms,
        cursor,
    };
    huf_uncompress(input, out, &mut t)
}

struct HufTables<'a> {
    hcode: &'a mut [u64],
    dec: &'a mut [u32],
    long_start: &'a mut [u32],
    long_syms: &'a mut Vec<u32>,
    syms: &'a mut Vec<u32>,
    cursor: &'a mut Vec<u32>,
}

fn huf_uncompress(input: &[u8], out: &mut [u16], t: &mut HufTables) -> Option<()> {
    let HufTables {
        hcode,
        dec,
        long_start,
        long_syms,
        syms,
        cursor,
    } = t;
    if input.is_empty() {
        return out.is_empty().then_some(());
    }
    let im = u32_at(input, 0)? as usize;
    let i_m = u32_at(input, 4)? as usize;
    let n_bits = u32_at(input, 12)? as u64;
    if im >= HUF_ENCSIZE || i_m >= HUF_ENCSIZE || im > i_m {
        return None;
    }
    let table = input.get(20..)?;
    // Unpack the encoding table (code lengths, with zero runs). Only entries listed in `syms`
    // are meaningful afterwards; the rest of `hcode` is stale.
    syms.clear();
    let mut b = Bits {
        d: table,
        p: 0,
        c: 0,
        lc: 0,
    };
    let mut i = im;
    while i <= i_m {
        let l = b.get_bits(6)?;
        if l == LONG_ZEROCODE_RUN {
            let run = (b.get_bits(8)? + SHORTEST_LONG_RUN) as usize;
            if i + run > i_m + 1 {
                return None;
            }
            i += run;
        } else if l >= SHORT_ZEROCODE_RUN {
            let run = (l - SHORT_ZEROCODE_RUN + 2) as usize;
            if i + run > i_m + 1 {
                return None;
            }
            i += run;
        } else {
            if l > 0 {
                hcode[i] = l;
                syms.push(i as u32);
            }
            i += 1;
        }
    }
    let data = &table[b.p..];
    if n_bits > 8 * data.len() as u64 {
        return None;
    }
    // Canonical codes.
    let mut n = [0u64; 59];
    for &sym in syms.iter() {
        n[hcode[sym as usize] as usize] += 1;
    }
    let mut c = 0u64;
    for i in (1..=58).rev() {
        let nc = (c + n[i]) >> 1;
        n[i] = c;
        c = nc;
    }
    for &sym in syms.iter() {
        let h = &mut hcode[sym as usize];
        let l = *h as usize;
        *h = l as u64 | (n[l] << 6);
        n[l] += 1;
    }
    // Decoding table.
    dec.fill(0);
    long_start.fill(0);
    for &sym in syms.iter() {
        let h = hcode[sym as usize];
        let code = h >> 6;
        let l = (h & 63) as i32;
        if code >> l != 0 {
            return None;
        }
        if l > HUF_DECBITS {
            let e = (code >> (l - HUF_DECBITS)) as usize;
            if dec[e] != 0 {
                return None;
            }
            long_start[e + 1] += 1;
        } else {
            let first = (code << (HUF_DECBITS - l)) as usize;
            let cnt = 1usize << (HUF_DECBITS - l);
            for e in first..first + cnt {
                if dec[e] != 0 || long_start[e + 1] != 0 {
                    return None;
                }
                dec[e] = ((l as u32) << 24) | sym;
            }
        }
    }
    // Prefix sums of long-code counts, then fill.
    for i in 0..HUF_DECSIZE {
        long_start[i + 1] += long_start[i];
    }
    long_syms.clear();
    long_syms.resize(long_start[HUF_DECSIZE] as usize, 0);
    if !long_syms.is_empty() {
        cursor.clear();
        cursor.extend_from_slice(&long_start[..HUF_DECSIZE]);
        for &sym in syms.iter() {
            let h = hcode[sym as usize];
            let l = (h & 63) as i32;
            if l > HUF_DECBITS {
                let e = ((h >> 6) >> (l - HUF_DECBITS)) as usize;
                long_syms[cursor[e] as usize] = sym;
                cursor[e] += 1;
            }
        }
    }
    // Decode.
    let ie = n_bits.div_ceil(8) as usize;
    let mut b = Bits {
        d: &data[..ie],
        p: 0,
        c: 0,
        lc: 0,
    };
    let rlc = i_m as u32;
    let mut o = 0usize;
    let mut put = |sym: u32, b: &mut Bits, o: &mut usize| -> Option<()> {
        if sym == rlc {
            if b.lc < 8 {
                b.get_char()?;
            }
            b.lc -= 8;
            let cs = ((b.c >> b.lc) & 0xff) as usize;
            if *o + cs > out.len() || *o == 0 {
                return None;
            }
            let s = out[*o - 1];
            out[*o..*o + cs].fill(s);
            *o += cs;
        } else {
            *out.get_mut(*o)? = sym as u16;
            *o += 1;
        }
        Some(())
    };
    while b.p < ie {
        b.get_char()?;
        while b.lc >= HUF_DECBITS {
            let e = ((b.c >> (b.lc - HUF_DECBITS)) & HUF_DECMASK) as usize;
            let d = dec[e];
            if d != 0 {
                b.lc -= (d >> 24) as i32;
                put(d & 0xff_ffff, &mut b, &mut o)?;
            } else {
                let (s0, s1) = (long_start[e] as usize, long_start[e + 1] as usize);
                if s0 == s1 {
                    return None;
                }
                let mut found = false;
                for &sym in &long_syms[s0..s1] {
                    let h = hcode[sym as usize];
                    let l = (h & 63) as i32;
                    while b.lc < l && b.p < ie {
                        b.get_char()?;
                    }
                    if b.lc >= l && (h >> 6) == ((b.c >> (b.lc - l)) & ((1u64 << l) - 1)) {
                        b.lc -= l;
                        put(sym, &mut b, &mut o)?;
                        found = true;
                        break;
                    }
                }
                if !found {
                    return None;
                }
            }
        }
    }
    let i = ((8 - n_bits as i64) & 7) as i32;
    b.c >>= i;
    b.lc -= i;
    while b.lc > 0 {
        let e = ((b.c << (HUF_DECBITS - b.lc)) & HUF_DECMASK) as usize;
        let d = dec[e];
        let l = (d >> 24) as i32;
        if d == 0 || l > b.lc {
            return None;
        }
        b.lc -= l;
        put(d & 0xff_ffff, &mut b, &mut o)?;
    }
    (o == out.len()).then_some(())
}

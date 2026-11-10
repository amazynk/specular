//! LERC (TIFF compression 34887): Esri's Limited Error Raster Compression, version 2 blobs
//! (`Lerc2 `, versions 2 to 6), optionally wrapped in Deflate or Zstandard (`LercParameters`,
//! tag 50674). Written from the published format description.
//!
//! A blob holds a header, a validity mask (run-length coded bits) and the values, either
//! constant, raw ("one sweep"), Huffman coded (8-bit lossless) or in micro-blocks whose
//! values are quantised to `2 * maxZError` steps above a block offset and bit-stuffed.
//! Several depths (samples per pixel) share one mask; several blobs may follow each other
//! (bands). Invalid pixels decode to 0 (integers) or NaN (floats); when the blob carries one
//! sample fewer than the image and the image has alpha, the mask becomes that alpha.
//!
//! Version 6 adds a lossless float mode ("FPL", [`fpl`]): the values' bytes are split into
//! byte planes, each delta-coded up to five times and Huffman, run-length or PackBits coded,
//! after a bit-field delta of whole values along rows (and columns).
//!
//! Not supported: version 1 (`CntZImage`).

use crate::{Ifd, SampleFormat, codec};

/// LERC data types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dt {
    Char,
    Byte,
    Short,
    UShort,
    Int,
    UInt,
    Float,
    Double,
}

impl Dt {
    fn from(v: i32) -> Option<Dt> {
        Some(match v {
            0 => Dt::Char,
            1 => Dt::Byte,
            2 => Dt::Short,
            3 => Dt::UShort,
            4 => Dt::Int,
            5 => Dt::UInt,
            6 => Dt::Float,
            7 => Dt::Double,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Dt::Char | Dt::Byte => 1,
            Dt::Short | Dt::UShort => 2,
            Dt::Int | Dt::UInt | Dt::Float => 4,
            Dt::Double => 8,
        }
    }

    fn is_int(self) -> bool {
        !matches!(self, Dt::Float | Dt::Double)
    }

    /// The narrower type a block offset was stored as (type code `tc`, bits 6-7 of the
    /// block flag).
    fn reduced(self, tc: u8) -> Option<Dt> {
        Some(match (self, tc) {
            (_, 0) => self,
            (Dt::Short, 1) => Dt::Byte,
            (Dt::Short, 2) => Dt::Char,
            (Dt::UShort, 1) => Dt::Byte,
            (Dt::Int, 1) => Dt::UShort,
            (Dt::Int, 2) => Dt::Short,
            (Dt::Int, 3) => Dt::Byte,
            (Dt::UInt, 1) => Dt::UShort,
            (Dt::UInt, 2) => Dt::Byte,
            (Dt::Float, 1) => Dt::Short,
            (Dt::Float, 2) => Dt::Byte,
            (Dt::Double, 1) => Dt::Float,
            (Dt::Double, 2) => Dt::Int,
            (Dt::Double, 3) => Dt::Short,
            _ => return None,
        })
    }

    /// Cast as the C++ reference stores a computed value into this type.
    #[inline]
    fn cast(self, v: f64) -> f64 {
        match self {
            Dt::Char => (v as i8) as f64,
            Dt::Byte => (v as u8) as f64,
            Dt::Short => (v as i16) as f64,
            Dt::UShort => (v as u16) as f64,
            Dt::Int => (v as i32) as f64,
            Dt::UInt => (v as u32) as f64,
            Dt::Float => (v as f32) as f64,
            Dt::Double => v,
        }
    }

    /// Wrap-around cast for integer arithmetic (Huffman deltas).
    #[inline]
    fn wrap(self, v: i64) -> f64 {
        match self {
            Dt::Char => (v as i8) as f64,
            Dt::Byte => (v as u8) as f64,
            _ => v as f64,
        }
    }
}

/// Little-endian byte reader.
struct Rd<'a> {
    s: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let b = self.s.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(b)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }
    fn f64(&mut self) -> Option<f64> {
        Some(f64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
    }
    fn uint(&mut self, n: usize) -> Option<u32> {
        let b = self.bytes(n)?;
        Some(b.iter().rev().fold(0u32, |a, &x| (a << 8) | x as u32))
    }
    fn value(&mut self, dt: Dt) -> Option<f64> {
        let b = self.bytes(dt.size())?;
        Some(value(dt, b))
    }
}

#[inline]
fn value(dt: Dt, b: &[u8]) -> f64 {
    match dt {
        Dt::Char => b[0] as i8 as f64,
        Dt::Byte => b[0] as f64,
        Dt::Short => i16::from_le_bytes([b[0], b[1]]) as f64,
        Dt::UShort => u16::from_le_bytes([b[0], b[1]]) as f64,
        Dt::Int => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        Dt::UInt => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        Dt::Float => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        Dt::Double => f64::from_le_bytes(b[..8].try_into().unwrap_or([0; 8])),
    }
}

/// Unpack `n` values of `nbits` bits. Version 3 and later pack least significant bit first
/// (a little-endian bit stream); version 2 packs most significant bit first inside
/// little-endian 32-bit words, the last word holding only the bytes needed.
fn unstuff(r: &mut Rd, n: usize, nbits: u32, version: i32, out: &mut Vec<u32>) -> Option<()> {
    let total = n.checked_mul(nbits as usize)?;
    out.clear();
    out.reserve(n);
    let mask = if nbits >= 32 {
        u32::MAX
    } else {
        (1u32 << nbits) - 1
    };
    if version >= 3 {
        let b = r.bytes(total.div_ceil(8))?;
        let mut pos = 0usize;
        for _ in 0..n {
            let at = pos >> 3;
            let mut w = [0u8; 8];
            let end = (at + 8).min(b.len());
            w[..end - at].copy_from_slice(&b[at..end]);
            let v = (u64::from_le_bytes(w) >> (pos & 7)) as u32 & mask;
            out.push(v);
            pos += nbits as usize;
        }
    } else {
        let words = total.div_ceil(32);
        let tail = match (total & 31).div_ceil(8) {
            0 => 0,
            t => 4 - t,
        };
        let b = r.bytes(words * 4 - tail)?;
        let word = |i: usize| -> u32 {
            let s = &b[(i * 4).min(b.len())..(i * 4 + 4).min(b.len())];
            let mut w = [0u8; 4];
            w[..s.len()].copy_from_slice(s);
            let v = u32::from_le_bytes(w);
            if i + 1 == words { v << (8 * tail) } else { v }
        };
        let (mut wi, mut bit) = (0usize, 0u32);
        for _ in 0..n {
            let cur = word(wi);
            let v = if 32 - bit >= nbits {
                let v = (cur << bit) >> (32 - nbits);
                bit += nbits;
                if bit == 32 {
                    bit = 0;
                    wi += 1;
                }
                v
            } else {
                let mut v = (cur << bit) >> (32 - nbits);
                wi += 1;
                bit -= 32 - nbits;
                v |= word(wi) >> (32 - bit);
                v
            };
            out.push(v);
        }
    }
    Some(())
}

/// BitStuffer2: a count, a bit width and the packed values, optionally through a lookup
/// table of distinct values.
fn bitstuff2(r: &mut Rd, version: i32, max: usize, out: &mut Vec<u32>) -> Option<()> {
    let b = r.u8()?;
    let nb = match b >> 6 {
        0 => 4,
        1 => 2,
        2 => 1,
        _ => return None,
    };
    let lut = b & 0x20 != 0;
    let nbits = (b & 31) as u32;
    let n = r.uint(nb)? as usize;
    if n > max {
        return None;
    }
    if !lut {
        if nbits == 0 {
            out.clear();
            out.resize(n, 0);
            return Some(());
        }
        return unstuff(r, n, nbits, version, out);
    }
    if nbits == 0 {
        return None;
    }
    let nlut = (r.u8()? as usize).checked_sub(1).filter(|&k| k > 0)?;
    let mut table = Vec::new();
    unstuff(r, nlut, nbits, version, &mut table)?;
    table.insert(0, 0);
    let nbl = usize::BITS - nlut.leading_zeros();
    unstuff(r, n, nbl, version, out)?;
    for v in out.iter_mut() {
        *v = *table.get(*v as usize)?;
    }
    Some(())
}

/// Most significant bit first reader over little-endian 32-bit words (Huffman data).
struct Msb<'a> {
    s: &'a [u8],
    word: usize,
    bit: u32,
}

impl Msb<'_> {
    #[inline]
    fn w(&self, i: usize) -> u32 {
        let at = i * 4;
        match self.s.get(at..at + 4) {
            Some(b) => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            None => {
                let mut w = [0u8; 4];
                if at < self.s.len() {
                    let t = &self.s[at..];
                    w[..t.len()].copy_from_slice(t);
                }
                u32::from_le_bytes(w)
            }
        }
    }
    #[inline]
    fn peek32(&self) -> u32 {
        let a = self.w(self.word) << self.bit;
        if self.bit == 0 {
            a
        } else {
            a | self.w(self.word + 1) >> (32 - self.bit)
        }
    }
    #[inline]
    fn skip(&mut self, n: u32) {
        let b = self.bit + n;
        self.word += (b / 32) as usize;
        self.bit = b % 32;
    }
    fn past_end(&self) -> bool {
        self.word * 4 + self.bit.div_ceil(8) as usize > self.s.len()
    }
    fn used(&self) -> usize {
        (self.word + usize::from(self.bit > 0)) * 4
    }
}

/// Huffman code table and decoder.
struct Huffman {
    lut_bits: u32,
    /// (symbol, length) by the next `lut_bits` bits; length 0 = longer code.
    lut: Vec<(u16, u8)>,
    /// Codes longer than `lut_bits`: (code, length, symbol).
    long: Vec<(u32, u8, u16)>,
}

impl Huffman {
    fn read(r: &mut Rd, version: i32) -> Option<Huffman> {
        let _hv = r.i32()?;
        let size = r.i32()?;
        let i0 = r.i32()?;
        let i1 = r.i32()?;
        if !(1..=65536).contains(&size) || i0 < 0 || i1 <= i0 || i1 - i0 > size || i1 > 2 * size {
            return None;
        }
        let size = size as usize;
        let (i0, i1) = (i0 as usize, i1 as usize);
        let wrap = |i: usize| if i < size { i } else { i - size };
        let mut lens = Vec::new();
        bitstuff2(r, version, i1 - i0, &mut lens)?;
        if lens.len() != i1 - i0 {
            return None;
        }
        let mut table = vec![(0u8, 0u32); size];
        for i in i0..i1 {
            let l = lens[i - i0];
            if l > 32 {
                return None;
            }
            table[wrap(i)].0 = l as u8;
        }
        // The codes, packed most significant bit first.
        let mut m = Msb {
            s: &r.s[r.p..],
            word: 0,
            bit: 0,
        };
        for i in i0..i1 {
            let k = wrap(i);
            let len = table[k].0 as u32;
            if len > 0 {
                table[k].1 = m.peek32() >> (32 - len);
                m.skip(len);
            }
        }
        if m.past_end() {
            return None;
        }
        r.p += m.used();
        let max_len = table.iter().map(|t| t.0 as u32).max()?;
        if max_len == 0 {
            return None;
        }
        let lut_bits = max_len.min(12);
        let mut lut = vec![(0u16, 0u8); 1 << lut_bits];
        let mut long = Vec::new();
        for (sym, &(len, code)) in table.iter().enumerate() {
            let len32 = len as u32;
            if len == 0 {
                continue;
            }
            if len32 <= lut_bits {
                let shift = lut_bits - len32;
                let base = (code as usize) << shift;
                for e in &mut lut[base..base + (1 << shift)] {
                    *e = (sym as u16, len);
                }
            } else {
                long.push((code, len, sym as u16));
            }
        }
        Some(Huffman {
            lut_bits,
            lut,
            long,
        })
    }

    #[inline]
    fn next(&self, m: &mut Msb) -> Option<u16> {
        let bits = m.peek32();
        let (sym, len) = self.lut[(bits >> (32 - self.lut_bits)) as usize];
        if len > 0 {
            m.skip(len as u32);
            return Some(sym);
        }
        for &(code, len, sym) in &self.long {
            if bits >> (32 - len as u32) == code {
                m.skip(len as u32);
                return Some(sym);
            }
        }
        None
    }
}

/// One decoded blob.
struct Blob {
    rows: usize,
    cols: usize,
    depth: usize,
    dt: Dt,
    /// Validity per pixel, `None` = all valid.
    mask: Option<Vec<bool>>,
    /// `rows * cols * depth` values.
    data: Vec<f64>,
}

impl Blob {
    #[inline]
    fn valid(&self, k: usize) -> bool {
        self.mask.as_ref().is_none_or(|m| m[k])
    }
}

/// Decode one blob starting at `s`; returns it and its size in bytes. `prev_mask` is the
/// mask of the previous band, which a band may reuse. A blob larger than `max_pix` pixels
/// or `max_depth` samples (the unit's) is refused before anything is allocated for it.
fn blob(
    s: &[u8],
    prev_mask: Option<&Option<Vec<bool>>>,
    max_pix: usize,
    max_depth: usize,
) -> Option<(Blob, usize)> {
    let mut r = Rd { s, p: 0 };
    if r.bytes(6)? != b"Lerc2 " {
        return None;
    }
    let version = r.i32()?;
    if !(2..=6).contains(&version) {
        return None;
    }
    if version >= 3 {
        r.i32()?; // checksum
    }
    let rows = r.i32()?;
    let cols = r.i32()?;
    let depth = if version >= 4 { r.i32()? } else { 1 };
    let num_valid = r.i32()?;
    let mb = r.i32()?;
    let blob_size = r.i32()?;
    let dt = Dt::from(r.i32()?)?;
    if version >= 6 {
        r.i32()?; // number of blobs that follow
    }
    let mut pass_nodata = false;
    if version >= 6 {
        let flags = r.bytes(4)?;
        pass_nodata = flags[0] != 0;
    }
    let max_z_error = r.f64()?;
    let z_min = r.f64()?;
    let z_max = r.f64()?;
    let (mut nodata, mut nodata_orig) = (0.0, 0.0);
    if version >= 6 {
        nodata = r.f64()?;
        nodata_orig = r.f64()?;
    }
    if rows <= 0 || cols <= 0 || depth <= 0 || mb <= 0 || num_valid < 0 || blob_size < r.p as i32 {
        return None;
    }
    let (rows, cols, depth, mb) = (rows as usize, cols as usize, depth as usize, mb as usize);
    let npix = rows.checked_mul(cols)?;
    let nval = npix.checked_mul(depth)?;
    if nval > 1 << 30 || npix > max_pix || depth > max_depth || num_valid as usize > npix {
        return None;
    }
    let blob_size = (blob_size as usize).min(s.len());
    r.s = &s[..blob_size];

    // Validity mask.
    let nbytes = r.i32()?;
    let num_valid = num_valid as usize;
    let mask = if num_valid == 0 {
        Some(vec![false; npix])
    } else if num_valid == npix {
        None
    } else if nbytes > 0 {
        let bits = rle(r.bytes(nbytes as usize)?, npix.div_ceil(8))?;
        Some(
            (0..npix)
                .map(|k| bits[k >> 3] & (0x80 >> (k & 7)) != 0)
                .collect(),
        )
    } else {
        prev_mask?.clone()
    };
    let mut b = Blob {
        rows,
        cols,
        depth,
        dt,
        mask,
        data: vec![0.0; nval],
    };
    if num_valid == 0 {
        return Some((b, blob_size));
    }
    let fill_const = |b: &mut Blob, v: &dyn Fn(usize) -> f64| {
        for k in 0..npix {
            if b.valid(k) {
                for d in 0..depth {
                    b.data[k * depth + d] = v(d);
                }
            }
        }
    };
    if z_min == z_max {
        fill_const(&mut b, &|_| z_min);
        return Some((b, blob_size));
    }
    let mut zmax_vec = vec![z_max; depth];
    if version >= 4 {
        let mins: Vec<f64> = (0..depth).map(|_| r.value(dt)).collect::<Option<_>>()?;
        let maxs: Vec<f64> = (0..depth).map(|_| r.value(dt)).collect::<Option<_>>()?;
        if mins == maxs {
            fill_const(&mut b, &|d| mins[d]);
            return Some((b, blob_size));
        }
        zmax_vec = maxs;
    }

    let one_sweep = r.u8()?;
    if one_sweep != 0 {
        let sz = dt.size();
        for k in 0..npix {
            if b.valid(k) {
                let raw = r.bytes(sz * depth)?;
                for d in 0..depth {
                    b.data[k * depth + d] = value(dt, &raw[d * sz..]);
                }
            }
        }
    } else {
        let mut mode = 0;
        if matches!(dt, Dt::Char | Dt::Byte) && max_z_error == 0.5 {
            mode = r.u8()?;
            if mode > 2 || (version < 4 && mode > 1) {
                return None;
            }
        } else if version >= 6 && !dt.is_int() && max_z_error == 0.0 {
            mode = r.u8()?;
            if mode == 3 {
                // Lossless float: every pixel, depth values per pixel as the rows.
                let (w, h) = if depth == 1 {
                    (cols, rows)
                } else {
                    (depth, npix)
                };
                b.data = fpl(&mut r, dt == Dt::Double, w, h)?;
                return Some((b, blob_size));
            }
            if mode != 0 {
                return None;
            }
        }
        if mode == 1 || mode == 2 {
            huffman(&mut r, &mut b, version, mode == 1)?;
        } else {
            tiles(&mut r, &mut b, version, mb, max_z_error, &zmax_vec)?;
        }
    }
    if pass_nodata && depth > 1 {
        for k in 0..npix {
            if b.valid(k) {
                for v in &mut b.data[k * depth..(k + 1) * depth] {
                    if *v == nodata {
                        *v = nodata_orig;
                    }
                }
            }
        }
    }
    Some((b, blob_size))
}

/// LERC 6 lossless float ("FPL") values of a `w`×`h` slice, as numbers (row-major).
///
/// A predictor byte (0 none, 1 delta along rows, 2 delta along columns then rows), then one
/// entry per byte of the value: its position in the little-endian value, the number of times
/// the plane was delta-coded (at most 5), and the coded plane ([`fpl_plane`]). The deltas
/// between whole values are taken separately on the low mantissa bits and on the bits above
/// them (sign and exponent), each wrapping; floats have the sign moved below the exponent.
fn fpl(r: &mut Rd, double: bool, w: usize, h: usize) -> Option<Vec<f64>> {
    let pred = r.u8()?;
    if pred > 2 || w == 0 {
        return None;
    }
    let unit = if double { 8 } else { 4 };
    let n = w.checked_mul(h)?;
    let mut planes: Vec<Option<Vec<u8>>> = vec![None; unit];
    for _ in 0..unit {
        let idx = r.u8()? as usize;
        let level = r.u8()? as usize;
        let size = r.uint(4)? as usize;
        if idx >= unit || level > 5 || planes[idx].is_some() {
            return None;
        }
        let mut p = fpl_plane(r.bytes(size)?, n)?;
        for l in (1..=level).rev() {
            for i in l..n {
                p[i] = p[i].wrapping_add(p[i - 1]);
            }
        }
        planes[idx] = Some(p);
    }
    let planes: Vec<Vec<u8>> = planes.into_iter().collect::<Option<_>>()?;
    // Whole values, then the bit-field deltas undone.
    let mut v: Vec<u64> = (0..n)
        .map(|i| {
            planes
                .iter()
                .enumerate()
                .fold(0u64, |a, (b, p)| a | (p[i] as u64) << (8 * b))
        })
        .collect();
    let low = if double { 52 } else { 23 };
    let high_mask = if double { 0xfff } else { 0x1ff };
    let add = |a: u64, b: u64| {
        let m = (1u64 << low) - 1;
        ((a & m).wrapping_add(b & m) & m) | (((a >> low).wrapping_add(b >> low) & high_mask) << low)
    };
    if pred == 2 {
        for y in 1..h {
            for x in 0..w {
                v[y * w + x] = add(v[y * w + x], v[(y - 1) * w + x]);
            }
        }
    }
    if pred > 0 {
        for row in v.chunks_exact_mut(w) {
            for x in 1..w {
                row[x] = add(row[x], row[x - 1]);
            }
        }
    }
    Some(
        v.into_iter()
            .map(|x| {
                if double {
                    f64::from_bits(x)
                } else {
                    // Bits 24..31 exponent, 23 sign, 0..22 mantissa, to IEEE.
                    let x = x as u32;
                    let ieee = (x & 0x7f_ffff) | ((x >> 24) & 0xff) << 23 | ((x >> 23) & 1) << 31;
                    f32::from_bits(ieee) as f64
                }
            })
            .collect(),
    )
}

/// One FPL byte plane of `n` bytes: a flag byte, then Huffman codes (0: the LERC code table
/// of version 5 and most significant bit first 32-bit words), one repeated byte (1), the
/// bytes as they are (2) or PackBits-like runs (3: `c <= 127` copies `c + 1` bytes, `c > 127`
/// repeats the next byte `c - 126` times).
fn fpl_plane(src: &[u8], n: usize) -> Option<Vec<u8>> {
    let (&flag, rest) = src.split_first()?;
    match flag {
        0 => {
            let mut r = Rd { s: rest, p: 0 };
            let h = Huffman::read(&mut r, 5)?;
            let mut m = Msb {
                s: &rest[r.p..],
                word: 0,
                bit: 0,
            };
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(h.next(&mut m)? as u8);
            }
            (!m.past_end()).then_some(out)
        }
        1 => {
            let (&b, rest) = rest.split_first()?;
            let len = u32::from_le_bytes(rest.get(..4)?.try_into().ok()?) as usize;
            (len == n).then(|| vec![b; n])
        }
        2 => Some(rest.get(..n)?.to_vec()),
        3 => {
            let mut out = Vec::with_capacity(n);
            let mut i = 0;
            while i < rest.len() && out.len() < n {
                let c = rest[i] as usize;
                i += 1;
                if c <= 127 {
                    out.extend_from_slice(rest.get(i..i + c + 1)?);
                    i += c + 1;
                } else {
                    let b = *rest.get(i)?;
                    i += 1;
                    out.extend(std::iter::repeat_n(b, c - 126));
                }
            }
            out.truncate(n);
            (out.len() == n).then_some(out)
        }
        _ => None,
    }
}

/// The mask's run-length coding: int16 counts, positive = that many literal bytes,
/// negative = the next byte repeated, -32768 = end.
fn rle(src: &[u8], n: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(n);
    let mut p = 0usize;
    loop {
        let c = i16::from_le_bytes(src.get(p..p + 2)?.try_into().ok()?);
        p += 2;
        if c == i16::MIN {
            break;
        }
        if c > 0 {
            out.extend_from_slice(src.get(p..p + c as usize)?);
            p += c as usize;
        } else {
            let v = *src.get(p)?;
            p += 1;
            out.resize(out.len() + c.unsigned_abs() as usize, v);
        }
        if out.len() > n {
            return None;
        }
    }
    (out.len() == n).then_some(out)
}

fn huffman(r: &mut Rd, b: &mut Blob, version: i32, delta: bool) -> Option<()> {
    let h = Huffman::read(r, version)?;
    let offset: i64 = if b.dt == Dt::Char { 128 } else { 0 };
    let mut m = Msb {
        s: &r.s[r.p..],
        word: 0,
        bit: 0,
    };
    let (rows, cols, depth) = (b.rows, b.cols, b.depth);
    for d in 0..depth {
        let mut prev = 0i64;
        for i in 0..rows {
            for j in 0..cols {
                let k = i * cols + j;
                if !b.valid(k) {
                    continue;
                }
                let v = h.next(&mut m)? as i64 - offset;
                let z = if !delta {
                    v
                } else if j > 0 && b.valid(k - 1) {
                    v + prev
                } else if i > 0 && b.valid(k - cols) {
                    v + b.data[(k - cols) * depth + d] as i64
                } else {
                    v + prev
                };
                let z = b.dt.wrap(z);
                b.data[k * depth + d] = z;
                prev = z as i64;
            }
        }
        if m.past_end() {
            return None;
        }
    }
    r.p += m.used();
    Some(())
}

fn tiles(
    r: &mut Rd,
    b: &mut Blob,
    version: i32,
    mb: usize,
    max_z_error: f64,
    zmax: &[f64],
) -> Option<()> {
    let (rows, cols, depth, dt) = (b.rows, b.cols, b.depth, b.dt);
    let inv_scale = 2.0 * max_z_error;
    let mut buf = Vec::new();
    // `mb` is a header field (up to 2^31): never more than the blob's pixels.
    let mut idx: Vec<usize> = Vec::with_capacity(mb.saturating_mul(mb).min(rows * cols));
    for i0 in (0..rows).step_by(mb) {
        let i1 = (i0 + mb).min(rows);
        for j0 in (0..cols).step_by(mb) {
            let j1 = (j0 + mb).min(cols);
            idx.clear();
            for i in i0..i1 {
                for j in j0..j1 {
                    let k = i * cols + j;
                    if b.valid(k) {
                        idx.push(k);
                    }
                }
            }
            if zmax.len() < depth {
                return None;
            }
            for (d, &zm) in zmax.iter().enumerate().take(depth) {
                let flag = r.u8()?;
                let (test, diff) = if version >= 5 {
                    ((flag >> 3) & 7, flag & 4 != 0)
                } else {
                    ((flag >> 2) & 15, false)
                };
                // v5+ keeps the 4-bit block-column code in bits 2..5 but gives bit 2 to
                // the diff flag, so only the code's upper three bits are checked.
                let want = if version >= 5 {
                    (j0 >> 4) & 7
                } else {
                    (j0 >> 3) & 15
                };
                if test as usize != want || (diff && d == 0) {
                    return None;
                }
                let prev = |b: &Blob, k: usize| {
                    if diff { b.data[k * depth + d - 1] } else { 0.0 }
                };
                match flag & 3 {
                    2 => {
                        for &k in &idx {
                            b.data[k * depth + d] = prev(b, k);
                        }
                    }
                    0 => {
                        let sz = dt.size();
                        for &k in &idx {
                            b.data[k * depth + d] = value(dt, r.bytes(sz)?);
                        }
                    }
                    c => {
                        let base = if diff && dt.is_int() && dt.size() < 4 {
                            Dt::Int
                        } else {
                            dt
                        };
                        let used = base.reduced(flag >> 6)?;
                        let offset = r.value(used)?;
                        if c == 3 {
                            for &k in &idx {
                                b.data[k * depth + d] = dt.cast(offset + prev(b, k));
                            }
                        } else {
                            bitstuff2(r, version, idx.len(), &mut buf)?;
                            if buf.len() != idx.len() {
                                return None;
                            }
                            for (&k, &q) in idx.iter().zip(&buf) {
                                let z = offset + q as f64 * inv_scale + prev(b, k);
                                b.data[k * depth + d] = dt.cast(z.min(zm));
                            }
                        }
                    }
                }
            }
        }
    }
    Some(())
}

/// Decode one LERC unit into `out`: rows of `uw` pixels of the IFD's samples in file byte
/// order (`le`). Returns the bytes written (complete rows); 0 if it does not decode.
pub(crate) fn decode(src: &[u8], ifd: &Ifd, le: bool, uw: u32, out: &mut [u8]) -> usize {
    let limit = out.len().saturating_mul(2).saturating_add(1 << 20);
    let unpacked;
    let s: &[u8] = match ifd.lerc[1] {
        1 => {
            unpacked = codec::inflate_vec(src, out.len() + 4096, limit);
            &unpacked
        }
        2 => {
            unpacked = codec::zstd_vec(src, limit);
            &unpacked
        }
        _ => src,
    };
    let spp = if ifd.planar { 1 } else { ifd.samples as usize };
    let bps = (ifd.bits as usize / 8).max(1);
    let px = spp * bps;
    let rb = uw as usize * px;
    if rb == 0 {
        return 0;
    }
    // A blob covers one strip or tile: at most `uw` × `unit_h` pixels.
    let max_pix = (uw as usize).saturating_mul(ifd.unit_h as usize);
    // All the blobs (bands).
    let mut blobs: Vec<Blob> = Vec::new();
    let mut p = 0usize;
    while p < s.len() && blobs.iter().map(|b| b.depth).sum::<usize>() < spp {
        let Some((b, n)) = blob(&s[p..], blobs.last().map(|b| &b.mask), max_pix, spp) else {
            break;
        };
        if let Some(f) = blobs.first()
            && (f.rows != b.rows || f.cols != b.cols)
        {
            break;
        }
        blobs.push(b);
        p += n.max(1);
    }
    let Some(first) = blobs.first() else {
        return 0;
    };
    let rows = first.rows.min(out.len() / rb);
    let cols = first.cols.min(uw as usize);
    let have: usize = blobs.iter().map(|b| b.depth).sum();
    // A blob one sample short of an image with alpha: its mask is the alpha.
    let mask_alpha = have + 1 == spp && ifd.alpha.is_some() && spp > 1;
    let float = ifd.sample_format == SampleFormat::Float;
    let invalid = if float { f64::NAN } else { 0.0 };
    let alpha_max = match (ifd.sample_format, bps) {
        (SampleFormat::Float, _) => 1.0,
        (SampleFormat::Int, b) => ((1u64 << (8 * b - 1)) - 1) as f64,
        (_, b) => ((1u64 << (8 * b)) - 1) as f64,
    };
    let write = |d: &mut [u8], v: f64| {
        macro_rules! put {
            ($t:ty) => {{
                let x = v as $t;
                d.copy_from_slice(&if le { x.to_le_bytes() } else { x.to_be_bytes() });
            }};
        }
        match (ifd.sample_format, bps) {
            (SampleFormat::Float, 4) => put!(f32),
            (SampleFormat::Float, 8) => put!(f64),
            (SampleFormat::Int, 1) => put!(i8),
            (SampleFormat::Int, 2) => put!(i16),
            (SampleFormat::Int, 4) => put!(i32),
            (_, 1) => put!(u8),
            (_, 2) => put!(u16),
            (_, 4) => put!(u32),
            _ => {}
        }
    };
    for y in 0..rows {
        let row = &mut out[y * rb..(y + 1) * rb];
        for x in 0..cols {
            let k = y * first.cols + x;
            let pix = &mut row[x * px..(x + 1) * px];
            let mut s = 0usize;
            for b in &blobs {
                let ok = b.valid(k);
                for d in 0..b.depth {
                    if s < spp {
                        let v = if ok { b.data[k * b.depth + d] } else { invalid };
                        write(&mut pix[s * bps..(s + 1) * bps], v);
                    }
                    s += 1;
                }
            }
            if mask_alpha {
                let a = if first.valid(k) { alpha_max } else { 0.0 };
                write(&mut pix[(spp - 1) * bps..spp * bps], a);
            }
        }
    }
    rows * rb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unstuff_lsb_and_msb() {
        // Values 1, 2, 3, 4, 5 in 3 bits.
        let lsb = [0b1101_0001, 0b0101_1000];
        let mut out = Vec::new();
        unstuff(&mut Rd { s: &lsb, p: 0 }, 5, 3, 3, &mut out).unwrap();
        assert_eq!(out, [1, 2, 3, 4, 5]);
        // MSB first in a little-endian word: 001 010 011 100 101 then padding; 15 bits
        // need two bytes, stored in the word's high bytes.
        let word: u32 = 0b001_010_011_100_101 << 17;
        let b = word.to_le_bytes();
        let mut r = Rd { s: &b[2..], p: 0 };
        unstuff(&mut r, 5, 3, 2, &mut out).unwrap();
        assert_eq!(out, [1, 2, 3, 4, 5]);
        assert_eq!(r.p, 2);
    }

    #[test]
    fn mask_rle() {
        let src = [2, 0, 0xAA, 0x55, 0xFD, 0xFF, 0xFF, 0x00, 0x80];
        assert_eq!(rle(&src, 5).unwrap(), [0xAA, 0x55, 0xFF, 0xFF, 0xFF]);
        assert!(rle(&src, 4).is_none());
    }

    #[test]
    fn garbage_does_not_panic() {
        let mut s = b"Lerc2 ".to_vec();
        s.extend_from_slice(&3i32.to_le_bytes());
        for i in 0..200u32 {
            s.push((i.wrapping_mul(2_654_435_761) >> 13) as u8);
        }
        let _ = blob(&s, None, usize::MAX, usize::MAX);
    }

    mod hostile {
        use super::*;

        /// A version 3 float blob header, then a mask size of 0 and the one-sweep flag 0.
        fn header(rows: i32, cols: i32, mb: i32) -> Vec<u8> {
            let mut s = b"Lerc2 ".to_vec();
            for v in [3, 0, rows, cols, rows.saturating_mul(cols), mb, 1000, 6] {
                s.extend_from_slice(&i32::to_le_bytes(v));
            }
            for v in [0.0f64, 0.0, 1.0] {
                s.extend_from_slice(&v.to_le_bytes());
            }
            s.extend_from_slice(&0i32.to_le_bytes());
            s.push(0);
            s.resize(64, 0);
            s
        }

        #[test]
        fn blob_larger_than_its_unit_is_refused() {
            // 32768 × 32768 pixels (8 GiB of f64) in a blob for a 16-pixel tile.
            assert!(blob(&header(32768, 32768, 8), None, 16, 1).is_none());
        }

        #[test]
        fn huge_micro_block_size_does_not_overflow_capacity() {
            // One pixel with micro blocks of 2^31 - 1: capacity (2^31)^2 overflowed.
            let _ = blob(&header(1, 1, i32::MAX), None, 16, 1);
        }
    }
}

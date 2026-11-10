//! Panasonic RW2 raw format 8 (DC-G9M2, DC-S5M2 and later).
//!
//! The sensor is split into up to five vertical strips, each its own bit stream (bytes read
//! least-significant bit first). A strip codes 2x2 quads in raster order, the four samples in
//! the order top-left, bottom-left, top-right, bottom-right. Each sample is a prefix code from
//! a 17-entry table (tag 0x40: code length and code; tag 0x41: a shift) followed by
//! `index - shift` difference bits, added to the same position of the previous quad; the
//! first quad of a row pair predicts from the first quad of the pair above (or tags
//! 0x3C–0x3F). Values clamp to tag 0x3B and then pass a piecewise curve built from tags 0x39
//! and 0x3A. Strips are independent, so they decode in parallel.

use crate::tiff::Ifd;
use rayon::prelude::*;
use std::ops::Range;

const UNDEFINED: u16 = 7;
const SHORT: u16 = 3;
const CODES: usize = 17;

/// One vertical strip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Strip {
    /// Encoded bytes in the file (may run past its end; the decode then reports truncation).
    pub range: Range<usize>,
    pub left: usize,
    pub width: usize,
    pub height: usize,
}

/// Decoding parameters from the RW2 IFD.
#[derive(Clone)]
pub(crate) struct Params {
    /// (code length, shift) per table index.
    codes: [(u32, u32); CODES],
    /// Table index for every value of the next 16 stream bits (`CODES` = no match).
    lut: Vec<u8>,
    /// Output curve, absent when it is the identity.
    curve: Option<Vec<u16>>,
    max: u32,
    initial: [u32; 4],
    pub strips: Vec<Strip>,
}

impl std::fmt::Debug for Params {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Params")
            .field("codes", &self.codes)
            .field("curve", &self.curve.is_some())
            .field("max", &self.max)
            .field("initial", &self.initial)
            .field("strips", &self.strips)
            .finish()
    }
}

/// Raw tag values (before the tables are built).
#[derive(Clone, Debug, Default)]
pub(crate) struct Tags {
    pub curve_mode: [u32; 6],
    pub curve_knee: [u32; 6],
    pub max: u32,
    pub initial: [u32; 4],
    pub code_len: [u32; CODES],
    pub code: [u32; CODES],
    pub shift: [u32; CODES],
    pub strips: Vec<Strip>,
}

/// Reads the format 8 tags of `ifd` (the RW2 IFD0).
pub(crate) fn tags(d: &[u8], ifd: &Ifd, raw_w: usize, raw_h: usize) -> Option<Tags> {
    // Arrays are UNDEFINED blobs of an exact size: a 16-bit count, then the values.
    let blob = |tag: u16, len: u32| -> Option<(&[u8], bool)> {
        let e = ifd.get(tag)?;
        (e.typ == UNDEFINED && e.count == len).then_some(())?;
        Some((e.bytes(d)?, e.le))
    };
    let u16_at = |b: &[u8], le: bool, i: usize| -> u32 {
        let v = [b[i], b[i + 1]];
        u32::from(if le {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        })
    };
    let u32_at = |b: &[u8], le: bool, i: usize| -> u32 {
        let v = [b[i], b[i + 1], b[i + 2], b[i + 3]];
        if le {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        }
    };
    let count = |b: &[u8], le: bool, max: usize| (u16_at(b, le, 0) as usize).min(max);
    let short = |tag: u16| -> Option<u32> {
        let e = ifd.get(tag)?;
        (e.typ == SHORT && e.count == 1).then_some(())?;
        e.uint(d, 0)
    };

    let mut t = Tags {
        max: short(0x3b)?,
        ..Default::default()
    };
    if let Some((b, le)) = blob(0x39, 26) {
        for i in 0..count(b, le, 6) {
            t.curve_mode[i] = u32_at(b, le, 2 + 4 * i);
        }
    }
    if let Some((b, le)) = blob(0x3a, 26) {
        for i in 0..count(b, le, 6) {
            t.curve_knee[i] = u16_at(b, le, 4 + 4 * i);
        }
    }
    for (i, v) in t.initial.iter_mut().enumerate() {
        *v = short(0x3c + i as u16).unwrap_or(0);
    }
    let (b, le) = blob(0x40, 70)?;
    for i in 0..count(b, le, CODES) {
        t.code_len[i] = u16_at(b, le, 2 + 4 * i).min(16);
        t.code[i] = u16_at(b, le, 4 + 4 * i).min(0xfff);
    }
    let (b, le) = blob(0x41, 36)?;
    for i in 0..count(b, le, CODES) {
        t.shift[i] = u16_at(b, le, 2 + 2 * i).min(64);
    }

    let n = short(0x42)?.min(5) as usize;
    let list = |tag: u16, len: u32, wide: bool| -> Option<Vec<u32>> {
        let (b, le) = blob(tag, len)?;
        let c = count(b, le, 5);
        (c >= n).then_some(())?;
        Some(
            (0..n)
                .map(|i| {
                    if wide {
                        u32_at(b, le, 2 + 4 * i)
                    } else {
                        u16_at(b, le, 2 + 2 * i)
                    }
                })
                .collect(),
        )
    };
    let offsets = list(0x44, 50, true)?;
    let lefts = list(0x45, 50, true)?;
    let sizes = list(0x46, 50, true)?;
    let widths = list(0x47, 26, false)?;
    let heights = list(0x48, 26, false)?;
    for i in 0..n {
        let (left, width, height) = (lefts[i] as usize, widths[i] as usize, heights[i] as usize);
        if width < 2 || height < 2 || left + width > raw_w || height > raw_h {
            return None;
        }
        let start = offsets[i] as usize;
        let len = (sizes[i] as usize).div_ceil(8);
        t.strips.push(Strip {
            range: start..start.saturating_add(len),
            left,
            width,
            height,
        });
    }
    (!t.strips.is_empty()).then_some(t)
}

impl Params {
    pub(crate) fn new(t: Tags) -> Params {
        let mut codes = [(0, 0); CODES];
        for (i, c) in codes.iter_mut().enumerate() {
            *c = (t.code_len[i] & 0x1f, t.shift[i] & 0x1f);
        }
        // A code of length 0 matches anything; the first matching entry wins.
        let lut = (0..=u16::MAX)
            .map(|top| {
                (0..CODES)
                    .find(|&i| {
                        let len = codes[i].0;
                        len == 0 || u32::from(top) >> (16 - len) == t.code[i] & ((1 << len) - 1)
                    })
                    .unwrap_or(CODES) as u8
            })
            .collect();
        let curve: Vec<u16> = (0..=u16::MAX as u32)
            .map(|v| curve_at(&t, v) as u16)
            .collect();
        let identity = curve.iter().enumerate().all(|(i, &v)| i == v as usize);
        Params {
            codes,
            lut,
            curve: (!identity).then_some(curve),
            max: t.max,
            initial: t.initial.map(|v| v & 0xffff),
            strips: t.strips,
        }
    }

    /// The output value of a coded sample at the clamp limit (the saturation level).
    pub(crate) fn white(&self, limit: u32) -> u32 {
        let v = limit.min(self.max).min(0xffff) as usize;
        self.curve.as_ref().map_or(v as u32, |c| u32::from(c[v]))
    }

    /// Byte span of all strips (for truncation checks).
    pub(crate) fn span(&self) -> Range<usize> {
        let start = self.strips.iter().map(|s| s.range.start).min().unwrap_or(0);
        let end = self.strips.iter().map(|s| s.range.end).max().unwrap_or(0);
        start..end.max(start)
    }
}

/// The output curve: six segments starting at `curve_knee`, each a constant, a right shift
/// with rounding or a left shift of the distance into the segment.
fn curve_at(t: &Tags, v: u32) -> u32 {
    let knee = |i: usize| t.curve_knee[i] & 0xffff;
    let mut seg = 0;
    while seg < 5 && v >= knee(seg + 1) {
        seg += 1;
    }
    let base = t.curve_knee[seg] >> 16;
    let mode = t.curve_mode[seg];
    let mut x = v.wrapping_sub(knee(seg));
    let y = match mode & 0x1f {
        31 => {
            if seg == 5 {
                0xffff
            } else {
                t.curve_knee[seg + 1] >> 16
            }
        }
        15 => base,
        m if mode & 0x10 == 0 => {
            if m != 0 {
                x = x.wrapping_add(1 << (m - 1)) >> m;
            }
            x.wrapping_add(base)
        }
        _ => (x << (mode & 0xf)).wrapping_add(base),
    };
    y.min(t.max)
}

/// Decodes all strips into the `w x h` sensor `out`. Returns true when data ran short or was
/// corrupt (the rest of that strip stays 0).
pub(crate) fn decode(
    d: &[u8],
    p: &Params,
    w: usize,
    h: usize,
    out: &mut [u16],
    parallel: bool,
) -> bool {
    let one = |s: &Strip| -> (Vec<u16>, bool) {
        let src = d
            .get(s.range.start.min(d.len())..s.range.end.min(d.len()))
            .unwrap_or(&[]);
        let rows = (s.height & !1).min(h & !1);
        let mut buf = vec![0u16; s.width * rows];
        let short = decode_strip(p, src, s.width, rows, &mut buf) || s.range.end > d.len();
        (buf, short)
    };
    let done: Vec<(Vec<u16>, bool)> = if parallel {
        p.strips.par_iter().map(one).collect()
    } else {
        p.strips.iter().map(one).collect()
    };
    let mut short = false;
    for (s, (buf, t)) in p.strips.iter().zip(done) {
        short |= t;
        if s.left + s.width > w {
            continue;
        }
        for (src, dst) in buf.chunks_exact(s.width).zip(out.chunks_exact_mut(w)) {
            dst[s.left..s.left + s.width].copy_from_slice(src);
        }
    }
    short
}

/// Decodes one strip of `width x rows` (rows even) into `out`.
fn decode_strip(p: &Params, src: &[u8], width: usize, rows: usize, out: &mut [u16]) -> bool {
    // The stream as MSB-first 64-bit words (each byte bit-reversed), plus zero padding.
    let mut words: Vec<u64> = src
        .chunks(8)
        .map(|c| {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            u64::from_le_bytes(b).reverse_bits()
        })
        .collect();
    words.extend([0, 0]);
    let total = src.len() * 8;
    let max = p.max.min(0xffff) as i32;
    let mut pos = 0usize;
    let mut line_base = p.initial;
    for pair in out.chunks_exact_mut(2 * width).take(rows / 2) {
        let (row0, row1) = pair.split_at_mut(width);
        let mut base = line_base;
        for g in 0..width / 2 {
            let mut quad = [0u32; 4];
            for slot in [0, 2, 1, 3] {
                if pos > total {
                    return true;
                }
                let (wi, sh) = (pos >> 6, pos & 63);
                let win = if sh == 0 {
                    words[wi]
                } else {
                    (words[wi] << sh) | (words[wi + 1] >> (64 - sh))
                };
                let i = p.lut[(win >> 48) as usize] as usize;
                let Some(&(len, s)) = p.codes.get(i) else {
                    return true;
                };
                let v = win << len;
                let n = i as i32 - s as i32;
                let bits = if n > 0 {
                    (u32::from((v >> (64 - n)) as u16) << s) & 0xffff
                } else {
                    0
                } as i32;
                let d1 = if (v as i64) < 0 {
                    bits
                } else if i > 0 {
                    bits - (1 << i) + i32::from(s == 0)
                } else {
                    0
                };
                let d2 = if s > 0 { 1i32 << (s - 1) } else { 0 };
                let val = (base[slot] as i32)
                    .wrapping_add(d1.wrapping_add(d2))
                    .clamp(0, max);
                quad[slot] = val as u32;
                pos += len as usize + n.max(0) as usize;
            }
            base = quad;
            if g == 0 {
                line_base = quad;
            }
            let q = quad.map(|v| match &p.curve {
                Some(c) => c[v as usize],
                None => v as u16,
            });
            row0[2 * g] = q[0];
            row0[2 * g + 1] = q[1];
            row1[2 * g] = q[2];
            row1[2 * g + 1] = q[3];
        }
    }
    pos > total
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LSB-first bit writer (the format's byte order).
    struct W {
        out: Vec<u8>,
        n: usize,
    }
    impl W {
        fn put(&mut self, v: u32, len: u32) {
            for k in (0..len).rev() {
                if self.n.is_multiple_of(8) {
                    self.out.push(0);
                }
                let b = (v >> k) & 1;
                *self.out.last_mut().unwrap() |= (b as u8) << (self.n % 8);
                self.n += 1;
            }
        }
    }

    /// A table where index i has a length-5 code `i` and no shift.
    fn tags(max: u32) -> Tags {
        let mut t = Tags {
            max,
            ..Default::default()
        };
        for i in 0..CODES {
            t.code_len[i] = 5;
            t.code[i] = i as u32;
        }
        t
    }

    /// Writes the code for difference `d` (index = bit length, JPEG-style negatives).
    fn put_diff(w: &mut W, d: i32) {
        let i = 32 - d.unsigned_abs().leading_zeros();
        w.put(i, 5);
        if i > 0 {
            let v = if d < 0 { d + (1 << i) - 1 } else { d } as u32;
            w.put(v, i);
        }
    }

    #[test]
    fn quads_and_prediction() {
        let mut t = tags(65535);
        t.initial = [100, 200, 300, 400];
        t.strips = vec![Strip {
            range: 0..0,
            left: 0,
            width: 4,
            height: 4,
        }];
        let p = Params::new(t);
        assert!(p.curve.is_none());
        // Row pair 0: quad (TL, BL, TR, BR) diffs from the initial values, then from quad 0.
        let mut w = W { out: vec![], n: 0 };
        for d in [1, -2, 3, 0, 5, 0, -7, 8, 10, 0, 0, 0, 0, 0, 0, -1] {
            put_diff(&mut w, d);
        }
        let mut out = vec![0u16; 16];
        assert!(!decode_strip(&p, &w.out, 4, 4, &mut out));
        // Quad 0: TL 101, BL 298, TR 203, BR 400; quad 1 adds 5, 0, -7, 8 to quad 0.
        assert_eq!(&out[0..4], &[101, 203, 106, 196]);
        assert_eq!(&out[4..8], &[298, 400, 298, 408]);
        // Row pair 1 predicts from quad 0 of pair 0.
        assert_eq!(&out[8..12], &[111, 203, 111, 203]);
        assert_eq!(&out[12..16], &[298, 400, 298, 399]);
    }

    #[test]
    fn clamp_and_curve() {
        let mut t = tags(1000);
        t.curve_mode = [1; 6];
        let mut tt = t.clone();
        tt.strips = vec![Strip {
            range: 0..0,
            left: 0,
            width: 2,
            height: 2,
        }];
        let p = Params::new(tt);
        assert_eq!(p.white(65535), 500);
        let mut w = W { out: vec![], n: 0 };
        for d in [2000, 3, -5, 0] {
            put_diff(&mut w, d);
        }
        let mut out = vec![0u16; 4];
        assert!(!decode_strip(&p, &w.out, 2, 2, &mut out));
        // 1000 (clamped), 3 -> 2, 0 (clamped), 0 after the halving curve.
        assert_eq!(out, [500, 0, 2, 0]);
        assert_eq!(curve_at(&t, 7), 4);
    }

    #[test]
    fn short_and_bad_codes() {
        let mut t = tags(4095);
        t.strips = vec![Strip {
            range: 0..0,
            left: 0,
            width: 2,
            height: 2,
        }];
        // Index 16 needs code 16 only; everything else fails to match.
        for i in 0..CODES {
            t.code_len[i] = 16;
            t.code[i] = 0xfff;
        }
        let p = Params::new(t.clone());
        let mut out = vec![0u16; 4];
        assert!(decode_strip(&p, &[0u8; 8], 2, 2, &mut out));
        let p = Params::new(tags(4095));
        assert!(decode_strip(&p, &[], 2, 2, &mut out));
        // Hostile strip geometry and data past the end.
        let mut big = tags(4095);
        big.strips = vec![Strip {
            range: 10..1 << 40,
            left: 0,
            width: 4,
            height: 1 << 20,
        }];
        let p = Params::new(big);
        let mut out = vec![0u16; 16];
        assert!(decode(&[0u8; 32], &p, 4, 4, &mut out, true));
    }
}

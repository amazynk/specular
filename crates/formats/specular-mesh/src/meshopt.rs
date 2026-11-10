//! Compressed glTF buffer views: `EXT_meshopt_compression` and `KHR_meshopt_compression`.
//!
//! Written from the Khronos specification (KHR_meshopt_compression, Appendix A and B):
//! attribute streams version 0 (the EXT bitstream) and version 1 (control and channel
//! modes), triangle streams (`0xe1`, and version 0's `0xe0`), index sequences (`0xd1`,
//! `0xd0`), and the octahedral, quaternion, exponential and colour filters. Every read is
//! bounds-checked; a stream that ends early, leaves bytes before its tail or reads a FIFO
//! entry never written is refused.

/// How a view's bytes are compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Attributes,
    Triangles,
    Indices,
}

/// The transform applied after the attribute codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Filter {
    None,
    Octahedral,
    Quaternion,
    Exponential,
    Color,
}

impl Mode {
    pub(crate) fn parse(s: &str) -> Option<Mode> {
        Some(match s {
            "ATTRIBUTES" => Mode::Attributes,
            "TRIANGLES" => Mode::Triangles,
            "INDICES" => Mode::Indices,
            _ => return None,
        })
    }
}

impl Filter {
    pub(crate) fn parse(s: &str) -> Option<Filter> {
        Some(match s {
            "NONE" => Filter::None,
            "OCTAHEDRAL" => Filter::Octahedral,
            "QUATERNION" => Filter::Quaternion,
            "EXPONENTIAL" => Filter::Exponential,
            "COLOR" => Filter::Color,
            _ => return None,
        })
    }
}

type R<T> = std::result::Result<T, String>;

/// Decodes `count` elements of `stride` bytes from `src`.
pub(crate) fn decode(
    src: &[u8],
    count: usize,
    stride: usize,
    mode: Mode,
    filter: Filter,
) -> R<Vec<u8>> {
    let size = count
        .checked_mul(stride)
        .ok_or("compressed view too large")?;
    let mut out = Vec::new();
    out.try_reserve_exact(size)
        .map_err(|_| "not enough memory for a compressed view")?;
    out.resize(size, 0);
    match mode {
        Mode::Attributes => {
            vertex(src, count, stride, &mut out)?;
            apply_filter(filter, stride, &mut out)?;
        }
        Mode::Triangles => {
            if filter != Filter::None {
                return Err("a filter on an index stream".into());
            }
            triangles(src, count, stride, &mut out)?;
        }
        Mode::Indices => {
            if filter != Filter::None {
                return Err("a filter on an index stream".into());
            }
            indices(src, count, stride, &mut out)?;
        }
    }
    Ok(out)
}

// ----- Mode 0: attributes ------------------------------------------------------------------

/// Bit widths of the four header codes: version 0, then version 1 control modes 0 and 1.
const BITS_V0: [u8; 4] = [0, 2, 4, 8];
const BITS_V1: [[u8; 4]; 2] = [[0, 1, 2, 4], [1, 2, 4, 8]];

fn unzig8(v: u8) -> u8 {
    if v & 1 != 0 { !(v >> 1) } else { v >> 1 }
}

fn unzig16(v: u16) -> u16 {
    if v & 1 != 0 { !(v >> 1) } else { v >> 1 }
}

fn vertex(src: &[u8], count: usize, stride: usize, out: &mut [u8]) -> R<()> {
    if stride == 0 || stride > 256 || !stride.is_multiple_of(4) {
        return Err(format!("attribute stride {stride}"));
    }
    let version = match src.first() {
        Some(0xa0) => 0,
        Some(0xa1) => 1,
        Some(h) => return Err(format!("attribute stream header {h:#04x}")),
        None => return Err("empty attribute stream".into()),
    };
    let channels_len = if version == 1 { stride / 4 } else { 0 };
    let tail_block = stride + channels_len;
    let tail_size = tail_block.max(if version == 1 { 24 } else { 32 });
    if src.len() < 1 + tail_size {
        return Err("attribute stream shorter than its tail".into());
    }
    let tail = &src[src.len() - tail_block..];
    let (baseline, channels) = tail.split_at(stride);
    for &c in channels {
        let m = c & 15;
        if m >= 3 || (m < 2 && c >> 4 != 0) {
            return Err(format!("attribute channel mode {c:#04x}"));
        }
    }
    let end = src.len() - tail_size;
    let mut pos = 1;
    let max_block = ((8192 / stride) & !15).min(256);
    let mut last = baseline.to_vec();
    let mut deltas = vec![0u8; stride * max_block];
    let mut start = 0;
    while start < count {
        let n = max_block.min(count - start);
        let groups = n.div_ceil(16);
        let aligned = groups * 16;
        let control = if version == 1 {
            let c = src
                .get(pos..pos + stride / 4)
                .filter(|_| pos + stride / 4 <= end)
                .ok_or("attribute stream ends early")?;
            pos += stride / 4;
            Some(c)
        } else {
            None
        };
        for k in 0..stride {
            let dst = &mut deltas[k * aligned..(k + 1) * aligned];
            let bits = match control {
                None => &BITS_V0,
                Some(c) => match (c[k / 4] >> ((k % 4) * 2)) & 3 {
                    2 => {
                        dst.fill(0);
                        continue;
                    }
                    3 => {
                        let lit = src
                            .get(pos..pos + n)
                            .filter(|_| pos + n <= end)
                            .ok_or("attribute stream ends early")?;
                        dst[..n].copy_from_slice(lit);
                        pos += n;
                        continue;
                    }
                    m => &BITS_V1[m as usize],
                },
            };
            pos = bytes_groups(src, pos, end, dst, groups, bits)?;
        }
        let block = &mut out[start * stride..(start + n) * stride];
        for c in 0..stride / 4 {
            let mode = channels.get(c).copied().unwrap_or(0);
            let base = c * 4;
            match mode & 15 {
                0 => {
                    for k in base..base + 4 {
                        let mut p = last[k];
                        let d = &deltas[k * aligned..k * aligned + n];
                        for (i, &v) in d.iter().enumerate() {
                            p = p.wrapping_add(unzig8(v));
                            block[i * stride + k] = p;
                        }
                        last[k] = p;
                    }
                }
                1 => {
                    for k in [base, base + 2] {
                        let mut p = u16::from_le_bytes([last[k], last[k + 1]]);
                        for i in 0..n {
                            let d = u16::from_le_bytes([
                                deltas[k * aligned + i],
                                deltas[(k + 1) * aligned + i],
                            ]);
                            p = p.wrapping_add(unzig16(d));
                            block[i * stride + k..i * stride + k + 2]
                                .copy_from_slice(&p.to_le_bytes());
                        }
                        last[k..k + 2].copy_from_slice(&p.to_le_bytes());
                    }
                }
                _ => {
                    let r = u32::from(mode >> 4);
                    let mut p = u32::from_le_bytes([
                        last[base],
                        last[base + 1],
                        last[base + 2],
                        last[base + 3],
                    ]);
                    for i in 0..n {
                        let d = u32::from_le_bytes([
                            deltas[base * aligned + i],
                            deltas[(base + 1) * aligned + i],
                            deltas[(base + 2) * aligned + i],
                            deltas[(base + 3) * aligned + i],
                        ]);
                        p ^= d.rotate_right(r);
                        block[i * stride + base..i * stride + base + 4]
                            .copy_from_slice(&p.to_le_bytes());
                    }
                    last[base..base + 4].copy_from_slice(&p.to_le_bytes());
                }
            }
        }
        start += n;
    }
    if pos != end {
        return Err("unread bytes before the attribute stream's tail".into());
    }
    Ok(())
}

/// One byte position's groups: the 2-bit header codes, then each group's deltas.
fn bytes_groups(
    src: &[u8],
    mut pos: usize,
    end: usize,
    dst: &mut [u8],
    groups: usize,
    bits: &[u8; 4],
) -> R<usize> {
    let early = || "attribute stream ends early".to_string();
    let header_len = groups.div_ceil(4);
    if pos + header_len > end {
        return Err(early());
    }
    let header = &src[pos..pos + header_len];
    pos += header_len;
    for g in 0..groups {
        let code = (header[g / 4] >> ((g % 4) * 2)) & 3;
        let out = &mut dst[g * 16..g * 16 + 16];
        match bits[code as usize] {
            0 => out.fill(0),
            8 => {
                if pos + 16 > end {
                    return Err(early());
                }
                out.copy_from_slice(&src[pos..pos + 16]);
                pos += 16;
            }
            b => {
                let packed = 2 * b as usize;
                if pos + packed > end {
                    return Err(early());
                }
                let mut extra = pos + packed;
                let sentinel = (1u8 << b) - 1;
                for (i, o) in out.iter_mut().enumerate() {
                    let v = match b {
                        1 => (src[pos + i / 8] >> (i % 8)) & 1,
                        2 => (src[pos + i / 4] >> (6 - (i % 4) * 2)) & 3,
                        _ => (src[pos + i / 2] >> (4 - (i % 2) * 4)) & 15,
                    };
                    *o = if v == sentinel {
                        if extra >= end {
                            return Err(early());
                        }
                        extra += 1;
                        src[extra - 1]
                    } else {
                        v
                    };
                }
                pos = extra;
            }
        }
    }
    Ok(pos)
}

// ----- Filters -----------------------------------------------------------------------------

fn apply_filter(filter: Filter, stride: usize, out: &mut [u8]) -> R<()> {
    match filter {
        Filter::None => {}
        Filter::Octahedral => match stride {
            4 => out.chunks_exact_mut(4).for_each(|e| {
                let i = [e[0] as i8, e[1] as i8, e[2] as i8];
                let o = octahedral([i[0].into(), i[1].into(), i[2].into()], 127.0);
                for (b, v) in e.iter_mut().zip(o) {
                    *b = (v as i8) as u8;
                }
            }),
            8 => out.chunks_exact_mut(8).for_each(|e| {
                let g = |k: usize| f32::from(i16::from_le_bytes([e[k * 2], e[k * 2 + 1]]));
                let o = octahedral([g(0), g(1), g(2)], 32767.0);
                for (k, v) in o.into_iter().enumerate() {
                    e[k * 2..k * 2 + 2].copy_from_slice(&(v as i16).to_le_bytes());
                }
            }),
            s => return Err(format!("octahedral filter with stride {s}")),
        },
        Filter::Quaternion => {
            if stride != 8 {
                return Err(format!("quaternion filter with stride {stride}"));
            }
            for e in out.chunks_exact_mut(8) {
                let g = |k: usize| i16::from_le_bytes([e[k * 2], e[k * 2 + 1]]);
                let range = std::f32::consts::FRAC_1_SQRT_2;
                let one = f32::from(g(3) | 3);
                let x = f32::from(g(0)) / one * range;
                let y = f32::from(g(1)) / one * range;
                let z = f32::from(g(2)) / one * range;
                let w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
                let max = (g(3) & 3) as usize;
                let mut o = [0i16; 4];
                o[(max + 1) % 4] = (x * 32767.0).round() as i16;
                o[(max + 2) % 4] = (y * 32767.0).round() as i16;
                o[(max + 3) % 4] = (z * 32767.0).round() as i16;
                o[max] = (w * 32767.0).round() as i16;
                for (k, v) in o.into_iter().enumerate() {
                    e[k * 2..k * 2 + 2].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        Filter::Exponential => {
            if !stride.is_multiple_of(4) {
                return Err(format!("exponential filter with stride {stride}"));
            }
            for e in out.chunks_exact_mut(4) {
                let v = i32::from_le_bytes([e[0], e[1], e[2], e[3]]);
                let x = (v >> 24).clamp(-126, 127);
                let m = (v << 8) >> 8;
                let f = (f64::from(m) * 2f64.powi(x)) as f32;
                e.copy_from_slice(&f.to_le_bytes());
            }
        }
        Filter::Color => match stride {
            4 => out.chunks_exact_mut(4).for_each(|e| {
                let i = [e[0], e[1], e[2], e[3]].map(u32::from);
                let o = color(i, 8);
                for (b, v) in e.iter_mut().zip(o) {
                    *b = v as u8;
                }
            }),
            8 => out.chunks_exact_mut(8).for_each(|e| {
                let i =
                    [0, 1, 2, 3].map(|k| u32::from(u16::from_le_bytes([e[k * 2], e[k * 2 + 1]])));
                let o = color(i, 16);
                for (k, v) in o.into_iter().enumerate() {
                    e[k * 2..k * 2 + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
            }),
            s => return Err(format!("colour filter with stride {s}")),
        },
    }
    Ok(())
}

/// Three octahedral inputs (x, y and the K-bit 1.0) to three unit components scaled by `max`.
fn octahedral(i: [f32; 3], max: f32) -> [i32; 3] {
    let one = i[2];
    let mut x = i[0] / one;
    let mut y = i[1] / one;
    let z = 1.0 - x.abs() - y.abs();
    let t = z.min(0.0);
    x -= t.copysign(x);
    y -= t.copysign(y);
    let len = (x * x + y * y + z * z).sqrt();
    [x / len, y / len, z / len].map(|v| {
        let r = (v * max).round();
        if r.is_finite() { r as i32 } else { 0 }
    })
}

/// YCoCg with a K-bit scale read from the alpha's top bit, to RGBA at `n` bits.
fn color(i: [u32; 4], n: u32) -> [u32; 4] {
    let msb = 31 - i[3].leading_zeros().min(31) as i32;
    let msb = if i[3] == 0 { -1 } else { msb };
    let scale = ((1i64 << (msb + 1)) - 1).max(1);
    let signed = |v: u32| -> i64 {
        let s = 32 - n;
        i64::from(((v << s) as i32) >> s)
    };
    let y = i64::from(i[0]);
    let co = signed(i[1]);
    let cg = signed(i[2]);
    let r = y + co - cg;
    let g = y + cg;
    let b = y - co - cg;
    let a = i64::from(i[3]) & (scale >> 1);
    let a = (a << 1) | (a & 1);
    let max = ((1u64 << n) - 1) as f32;
    let ss = max / scale as f32;
    [r, g, b, a].map(|v| (v as f32 * ss).round().clamp(0.0, max) as u32)
}

// ----- Modes 1 and 2: indices --------------------------------------------------------------

fn put_index(out: &mut [u8], at: usize, stride: usize, v: u32) {
    if stride == 2 {
        out[at * 2..at * 2 + 2].copy_from_slice(&(v as u16).to_le_bytes());
    } else {
        out[at * 4..at * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// An unsigned LEB128 value of at most 5 bytes, below `end`.
fn varint(src: &[u8], pos: &mut usize, end: usize) -> R<u32> {
    let mut v = 0u32;
    for k in 0..5 {
        if *pos >= end {
            return Err("index stream ends early".into());
        }
        let b = src[*pos];
        *pos += 1;
        v |= u32::from(b & 0x7f).wrapping_shl(7 * k);
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("index varint longer than 5 bytes".into())
}

/// A 16-entry FIFO, newest first, that refuses reads of entries never written.
struct Fifo<T: Copy + Default> {
    items: [T; 16],
    next: usize,
    written: usize,
}

impl<T: Copy + Default> Fifo<T> {
    fn new() -> Self {
        Fifo {
            items: [T::default(); 16],
            next: 0,
            written: 0,
        }
    }
    fn push(&mut self, v: T) {
        self.items[self.next & 15] = v;
        self.next += 1;
        self.written = (self.written + 1).min(16);
    }
    fn get(&self, back: usize) -> R<T> {
        if back >= self.written {
            return Err("index stream reads a FIFO entry never written".into());
        }
        Ok(self.items[(self.next + 15 - back) & 15])
    }
}

fn triangles(src: &[u8], count: usize, stride: usize, out: &mut [u8]) -> R<()> {
    if stride != 2 && stride != 4 {
        return Err(format!("triangle stream stride {stride}"));
    }
    if !count.is_multiple_of(3) {
        return Err("triangle stream count not a multiple of 3".into());
    }
    // Version 0 (0xe0, older encoders) has no "last ± 1" codes: 13 and 14 are vertex FIFO
    // entries there.
    let fifo_max = match src.first() {
        Some(0xe1) => 12,
        Some(0xe0) => 14,
        Some(h) => return Err(format!("triangle stream header {h:#04x}")),
        None => return Err("empty triangle stream".into()),
    };
    let tris = count / 3;
    if src.len() < 1 + tris + 16 {
        return Err("triangle stream shorter than its codes".into());
    }
    let end = src.len() - 16;
    let codeaux = &src[end..];
    let codes = &src[1..1 + tris];
    let mut pos = 1 + tris;
    let mut edges: Fifo<(u32, u32)> = Fifo::new();
    let mut verts: Fifo<u32> = Fifo::new();
    let mut next = 0u32;
    let mut last = 0u32;
    let delta = |v: u32, last: &mut u32| {
        let d = if v & 1 != 0 { !(v >> 1) } else { v >> 1 };
        *last = last.wrapping_add(d);
        *last
    };
    for (t, &code) in codes.iter().enumerate() {
        let (hi, lo) = ((code >> 4) as usize, (code & 15) as usize);
        let (a, b, c);
        if hi < 15 {
            let (ea, eb) = edges.get(hi)?;
            (a, b) = (ea, eb);
            c = match lo {
                0 => {
                    let c = next;
                    next = next.wrapping_add(1);
                    verts.push(c);
                    c
                }
                1..=14 if lo <= fifo_max => verts.get(lo)?,
                13 | 14 => {
                    last = if lo == 13 {
                        last.wrapping_sub(1)
                    } else {
                        last.wrapping_add(1)
                    };
                    verts.push(last);
                    last
                }
                _ => {
                    let v = varint(src, &mut pos, end)?;
                    let c = delta(v, &mut last);
                    verts.push(c);
                    c
                }
            };
            edges.push((c, b));
            edges.push((a, c));
        } else {
            let (z, w, explicit_a);
            if lo < 14 {
                let zw = codeaux[lo];
                (z, w) = ((zw >> 4) as usize, (zw & 15) as usize);
                explicit_a = false;
            } else {
                if pos >= end {
                    return Err("triangle stream ends early".into());
                }
                let zw = src[pos];
                pos += 1;
                if zw == 0 {
                    next = 0;
                }
                (z, w) = ((zw >> 4) as usize, (zw & 15) as usize);
                explicit_a = lo == 15;
            }
            a = if explicit_a {
                let v = varint(src, &mut pos, end)?;
                delta(v, &mut last)
            } else {
                let a = next;
                next = next.wrapping_add(1);
                a
            };
            let mut third = |x: usize| -> R<(u32, bool)> {
                Ok(match x {
                    0 => {
                        let v = next;
                        next = next.wrapping_add(1);
                        (v, true)
                    }
                    15 if lo >= 14 => {
                        let v = varint(src, &mut pos, end)?;
                        (delta(v, &mut last), true)
                    }
                    _ => (verts.get(x - 1)?, false),
                })
            };
            let (bv, push_b) = third(z)?;
            let (cv, push_c) = third(w)?;
            (b, c) = (bv, cv);
            edges.push((b, a));
            edges.push((c, b));
            edges.push((a, c));
            verts.push(a);
            if push_b {
                verts.push(b);
            }
            if push_c {
                verts.push(c);
            }
        }
        put_index(out, t * 3, stride, a);
        put_index(out, t * 3 + 1, stride, b);
        put_index(out, t * 3 + 2, stride, c);
    }
    if pos != end {
        return Err("unread bytes before the triangle stream's tail".into());
    }
    Ok(())
}

fn indices(src: &[u8], count: usize, stride: usize, out: &mut [u8]) -> R<()> {
    if stride != 2 && stride != 4 {
        return Err(format!("index stream stride {stride}"));
    }
    match src.first() {
        // Versions 0 and 1 code sequences alike.
        Some(0xd0 | 0xd1) => {}
        Some(h) => return Err(format!("index stream header {h:#04x}")),
        None => return Err("empty index stream".into()),
    }
    if src.len() < 1 + 4 {
        return Err("index stream shorter than its tail".into());
    }
    let end = src.len() - 4;
    let mut pos = 1;
    let mut last = [0u32; 2];
    for i in 0..count {
        let v = varint(src, &mut pos, end)?;
        let base = (v & 1) as usize;
        let d = if v & 2 != 0 { !(v >> 2) } else { v >> 2 };
        last[base] = last[base].wrapping_add(d);
        put_index(out, i, stride, last[base]);
    }
    if pos != end {
        return Err("unread bytes before the index stream's tail".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification's 4-bit sentinel example.
    #[test]
    fn spec_four_bit_example() {
        let src = [0x17, 0x5f, 0xf0, 0xbc, 0x77, 0xa9, 0x21, 0x00, 0x34, 0xb5];
        let mut dst = [0u8; 16];
        let end = src.len();
        let pos = bytes_groups(
            &[&[0b10u8][..], &src[..]].concat(),
            0,
            end + 1,
            &mut dst,
            1,
            &BITS_V0,
        )
        .unwrap();
        assert_eq!(pos, end + 1);
        let d: Vec<i8> = dst.iter().map(|&v| unzig8(v) as i8).collect();
        assert_eq!(
            d,
            [-1, -4, -3, 26, -91, 0, -6, 6, -4, -4, 5, -5, 1, -1, 0, 0]
        );
    }

    #[test]
    fn spec_varints() {
        let mut p = 0;
        assert_eq!(varint(&[0x7f], &mut p, 1), Ok(0x7f));
        p = 0;
        assert_eq!(varint(&[0x81, 0x04], &mut p, 2), Ok(0x201));
        p = 0;
        // The specification prints 0x1fd005 here, which is the groups read most significant
        // first; its other examples and the encoder are LEB128 (least significant first).
        assert_eq!(varint(&[0xff, 0xa0, 0x05], &mut p, 3), Ok(0x1507f));
    }

    #[test]
    fn hostile_streams_are_refused() {
        for mode in [Mode::Attributes, Mode::Triangles, Mode::Indices] {
            for len in 0..80 {
                let src: Vec<u8> = (0..len).map(|i| (i * 37 + 0xa0) as u8).collect();
                let _ = decode(&src, 30, 4, mode, Filter::None);
            }
        }
        assert!(
            decode(
                &[0xe1, 0x10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                3,
                4,
                Mode::Triangles,
                Filter::None
            )
            .is_err()
        );
    }

    fn corpus() -> Option<std::path::PathBuf> {
        let dir = std::env::var_os("SPECULAR_MESHOPT_CORPUS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../tools/corpus/meshopt")
            });
        dir.join("streams").is_dir().then_some(dir)
    }

    /// Every stream of tools/corpus/gen-meshopt.py against meshoptimizer's own decoder and
    /// filters (the colour filter against the specification's reference); skips without it.
    #[test]
    fn oracle_streams() {
        let Some(dir) = corpus() else {
            eprintln!("no meshopt corpus; run tools/corpus/gen-meshopt.py");
            return;
        };
        let mut names: Vec<_> = std::fs::read_dir(dir.join("streams"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        names.sort();
        let (mut ok, mut bad, mut off_by_one) = (0, Vec::new(), 0);
        for p in &names {
            let j: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
            let s = |k: &str| j[k].as_str().unwrap().to_owned();
            let n = |k: &str| j[k].as_u64().unwrap() as usize;
            let src = std::fs::read(p.with_extension("bin")).unwrap();
            let want = std::fs::read(p.with_extension("out")).unwrap();
            let got = decode(
                &src,
                n("count"),
                n("byteStride"),
                Mode::parse(&s("mode")).unwrap(),
                Filter::parse(&s("filter")).unwrap(),
            );
            // Octahedral and quaternion results are specified to one unit in the last place.
            let ulp = matches!(s("filter").as_str(), "OCTAHEDRAL" | "QUATERNION");
            let comp = if n("byteStride") == 4 { 1 } else { 2 };
            let near = |g: &[u8]| {
                g.len() == want.len()
                    && g.chunks(comp).zip(want.chunks(comp)).all(|(a, b)| {
                        let v = |c: &[u8]| match c {
                            [x] => i32::from(*x as i8),
                            _ => i32::from(i16::from_le_bytes([c[0], c[1]])),
                        };
                        (v(a) - v(b)).abs() <= 1
                    })
            };
            match got {
                Ok(g) if g == want => ok += 1,
                Ok(g) if ulp && near(&g) => {
                    ok += 1;
                    off_by_one += g
                        .chunks(comp)
                        .zip(want.chunks(comp))
                        .filter(|(a, b)| a != b)
                        .count();
                }
                Ok(g) => {
                    let at = g.iter().zip(&want).position(|(a, b)| a != b);
                    bad.push(format!("{}: differs at byte {at:?}", p.display()));
                }
                Err(e) => bad.push(format!("{}: {e}", p.display())),
            }
        }
        eprintln!(
            "meshopt streams: {ok} match ({off_by_one} filtered components 1 ulp off), {} differ",
            bad.len()
        );
        assert!(bad.is_empty(), "{bad:#?}");
        assert!(ok > 100);
    }
}

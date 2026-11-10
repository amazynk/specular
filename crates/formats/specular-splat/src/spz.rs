//! Niantic's `.spz`. Versions 1–3: a gzip stream holding a 16-byte header ("NGSP", version,
//! count, harmonic degree, fractional bits, flags) and the attribute arrays one after another;
//! version 4: a 32-byte plain header, a table of contents and one Zstandard stream per
//! attribute. The arrays: positions (24-bit fixed point, or half floats in version 1),
//! opacities (8-bit, the logistic of the logit), colours (8-bit, (c / 255 − 0.5) / 0.15 as the
//! DC coefficient), log scales (8-bit, s / 16 − 10), rotations (version 2: x, y, z as 8-bit,
//! w ≥ 0 derived; 3 and 4: smallest three in 32 bits) and harmonics (8-bit, (b − 128) / 128,
//! coefficient-major with r, g, b interleaved). Up is +y.

use crate::half::{f16_to_f32, f32_to_f16};
use crate::math::SH_C0;
use crate::{Error, FileUp, Limits, Raw, Result, SplatFormat, bad, keep};

/// The colour quantisation scale of the format.
const COLOR_SCALE: f32 = 0.15;

struct Head {
    version: u32,
    n: u64,
    degree: u8,
    fractional: u8,
    flags: u8,
}

fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<Raw> {
    if bytes.starts_with(b"NGSP") {
        return read_v4(bytes, limits);
    }
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return bad("not an SPZ file (neither gzip nor NGSP)");
    }
    let data = specular_png::inflate_gzip(bytes, limits.max_unpacked)
        .map_err(|e| Error::Malformed(format!("gzip: {e}")))?;
    if data.len() < 16 || &data[..4] != b"NGSP" {
        return bad("no NGSP header inside the gzip stream");
    }
    let head = Head {
        version: u32le(&data, 4),
        n: u64::from(u32le(&data, 8)),
        degree: data[12],
        fractional: data[13],
        flags: data[14],
    };
    if !(1..=3).contains(&head.version) {
        return Err(Error::Unsupported(format!(
            "SPZ version {} inside gzip",
            head.version
        )));
    }
    let sizes = sizes(&head)?;
    let total: usize = sizes.iter().sum();
    if data.len() < 16 + total {
        return bad("SPZ data shorter than its header says");
    }
    let mut streams = Vec::new();
    let mut at = 16;
    for s in sizes {
        streams.push(&data[at..at + s]);
        at += s;
    }
    decode(&head, &streams, limits)
}

/// Bytes of each attribute array: positions, alphas, colours, scales, rotations, harmonics.
fn sizes(h: &Head) -> Result<[usize; 6]> {
    if h.n == 0 {
        return Err(Error::Empty);
    }
    if h.degree > 3 {
        return Err(Error::Unsupported(format!(
            "SPZ harmonics of degree {}",
            h.degree
        )));
    }
    let n = usize::try_from(h.n).map_err(|_| Error::TooLarge("count".into()))?;
    if n > 1 << 31 {
        return Err(Error::TooLarge(format!("{n} splats")));
    }
    let pos = if h.version == 1 { 6 } else { 9 };
    let rot = if h.version >= 3 { 4 } else { 3 };
    let sh = crate::sh_coeffs(h.degree) * 3;
    Ok([n * pos, n, n * 3, n * 3, n * rot, n * sh])
}

fn read_v4(bytes: &[u8], limits: &Limits) -> Result<Raw> {
    if bytes.len() < 32 {
        return bad("SPZ header cut short");
    }
    let head = Head {
        version: u32le(bytes, 4),
        n: u64::from(u32le(bytes, 8)),
        degree: bytes[12],
        fractional: bytes[13],
        flags: bytes[14],
    };
    if head.version != 4 {
        return Err(Error::Unsupported(format!(
            "SPZ version {} with a plain header",
            head.version
        )));
    }
    let streams_n = usize::from(bytes[15]);
    let toc = u32le(bytes, 16) as usize;
    let want = sizes(&head)?;
    let mut entries = Vec::new();
    for i in 0..streams_n {
        let at = toc + i * 16;
        let e = bytes
            .get(at..at + 16)
            .ok_or_else(|| Error::Malformed("SPZ table of contents cut short".into()))?;
        let c = u64::from_le_bytes(e[..8].try_into().expect("8"));
        let u = u64::from_le_bytes(e[8..].try_into().expect("8"));
        entries.push((c, u));
    }
    let mut at = toc + streams_n * 16;
    let mut out: Vec<Vec<u8>> = Vec::new();
    // The streams' sizes follow from the header: checked before anything is decompressed,
    // so a table of contents cannot make 255 streams of 4 GiB each.
    let nonzero: Vec<usize> = want.iter().copied().filter(|&s| s > 0).collect();
    if entries.len() < nonzero.len() {
        return Err(Error::Malformed("SPZ streams missing".into()));
    }
    // All the streams are held at once until the splats are built: reserved from the memory
    // budget before any is decompressed (the header's splat count alone sets their sizes).
    let total = nonzero
        .iter()
        .fold(0u64, |t, &n| t.saturating_add(n as u64));
    let _held = specular_core::budget::reserve(total)
        .map_err(|e| Error::TooLarge(format!("the SPZ streams: {e}")))?;
    for (k, (c, u)) in entries.into_iter().take(nonzero.len()).enumerate() {
        if u != nonzero[k] as u64 {
            return bad("an SPZ stream has the wrong size");
        }
        let c = usize::try_from(c).map_err(|_| Error::Malformed("stream size".into()))?;
        let u = usize::try_from(u).map_err(|_| Error::Malformed("stream size".into()))?;
        if u > limits.max_unpacked {
            return Err(Error::TooLarge(format!("an SPZ stream of {u} bytes")));
        }
        let src = at
            .checked_add(c)
            .and_then(|end| bytes.get(at..end))
            .ok_or_else(|| Error::Malformed("SPZ stream past the end".into()))?;
        out.push(zstd(src, u)?);
        at += c;
    }
    // Empty streams (no harmonics) are left out of the file.
    let mut streams: Vec<&[u8]> = Vec::new();
    let mut it = out.iter();
    for &size in &want {
        if size == 0 {
            streams.push(&[]);
            continue;
        }
        let s = it
            .next()
            .ok_or_else(|| Error::Malformed("SPZ streams missing".into()))?;
        if s.len() != size {
            return bad("an SPZ stream has the wrong size");
        }
        streams.push(s);
    }
    decode(&head, &streams, limits)
}

fn zstd(src: &[u8], size: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut dec = ruzstd::decoding::StreamingDecoder::new(src)
        .map_err(|e| Error::Malformed(format!("zstd: {e}")))?;
    let mut out = Vec::with_capacity(size);
    dec.by_ref()
        .take(size as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| Error::Malformed(format!("zstd: {e}")))?;
    if out.len() != size {
        return bad("an SPZ stream has the wrong size");
    }
    Ok(out)
}

fn decode(h: &Head, s: &[&[u8]], limits: &Limits) -> Result<Raw> {
    let [pos, alpha, color, scale, rot, sh] = [s[0], s[1], s[2], s[3], s[4], s[5]];
    let (kept, stride) = keep(h.n, limits.max_splats);
    let k = crate::sh_coeffs(h.degree);
    let mut raw = Raw::new(SplatFormat::Spz, h.n, FileUp::PosY, kept, k);
    raw.sh_degree = h.degree;
    raw.antialiased = h.flags & 1 != 0;
    if stride > 1 {
        raw.notes.push(format!(
            "every {stride}th of {} splats shown (the cap)",
            h.n
        ));
    }
    let frac = 1.0 / (1u64 << h.fractional.min(31)) as f32;
    for i in (0..h.n as usize).step_by(stride as usize) {
        let p = if h.version == 1 {
            let b = &pos[i * 6..];
            std::array::from_fn(|a| f16_to_f32(u16::from_le_bytes([b[a * 2], b[a * 2 + 1]])))
        } else {
            let b = &pos[i * 9..];
            std::array::from_fn(|a| {
                let v = u32::from(b[a * 3])
                    | u32::from(b[a * 3 + 1]) << 8
                    | u32::from(b[a * 3 + 2]) << 16;
                let v = if v & 0x80_0000 != 0 {
                    (v | 0xff00_0000) as i32
                } else {
                    v as i32
                };
                v as f32 * frac
            })
        };
        raw.positions.push(p);
        raw.scales.push(std::array::from_fn(|a| {
            (f32::from(scale[i * 3 + a]) / 16.0 - 10.0).exp()
        }));
        let q = if h.version >= 3 {
            smallest_three(u32le(rot, i * 4))
        } else {
            let b = &rot[i * 3..];
            let v = [b[0], b[1], b[2]].map(|x| f32::from(x) / 127.5 - 1.0);
            let w = (1.0 - v.iter().map(|x| x * x).sum::<f32>()).max(0.0).sqrt();
            [w, v[0], v[1], v[2]]
        };
        raw.rotations.push(q);
        let c = |a: usize| {
            let dc = (f32::from(color[i * 3 + a]) / 255.0 - 0.5) / COLOR_SCALE;
            0.5 + SH_C0 * dc
        };
        raw.colors
            .push([c(0), c(1), c(2), f32::from(alpha[i]) / 255.0]);
        for j in 0..k * 3 {
            let b = f32::from(sh[i * k * 3 + j]);
            raw.sh.push(f32_to_f16((b - 128.0) / 128.0));
        }
    }
    Ok(raw)
}

/// A smallest-three quaternion: bits 31–30 the index (x, y, z, w) of the dropped largest
/// component; then the others in order, 10 bits each (sign bit 9, magnitude 0–8 over 511,
/// times √½). Returned as w, x, y, z.
fn smallest_three(r: u32) -> [f32; 4] {
    let big = (r >> 30) as usize;
    let mut q = [0f32; 4];
    let mut sum = 0.0;
    let mut shift = 30;
    for (i, slot) in q.iter_mut().enumerate() {
        if i == big {
            continue;
        }
        shift -= 10;
        let v = (r >> shift) & 0x3ff;
        let m = (v & 0x1ff) as f32 / 511.0 * std::f32::consts::FRAC_1_SQRT_2;
        *slot = if v & 0x200 != 0 { -m } else { m };
        sum += *slot * *slot;
    }
    q[big] = (1.0 - sum).max(0.0).sqrt();
    // x, y, z, w → w, x, y, z
    [q[3], q[0], q[1], q[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table of contents claiming a 4 GiB stream for one splat: refused by its size before
    /// it is decompressed.
    #[test]
    fn stream_size_checked_first() {
        let mut b = vec![0u8; 32];
        b[4..8].copy_from_slice(&4u32.to_le_bytes());
        b[8..12].copy_from_slice(&1u32.to_le_bytes());
        b[15] = 5;
        b[16..20].copy_from_slice(&32u32.to_le_bytes());
        for _ in 0..5 {
            b.extend_from_slice(&8u64.to_le_bytes());
            b.extend_from_slice(&(4u64 << 30).to_le_bytes());
        }
        b.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD, 0, 0, 0, 0]);
        let e = read_v4(&b, &Limits::default()).map(|_| ()).unwrap_err();
        assert!(e.to_string().contains("wrong size"), "{e}");
    }

    /// 2²⁶ splats declare 1.3 GB of streams in a file of 120 bytes: the streams are reserved
    /// from the memory budget before the first is decompressed, so a small budget refuses
    /// the file as too large instead of allocating (or failing on the bogus data).
    #[test]
    fn streams_reserved_before_decompressing() {
        let n = 1u64 << 26;
        let mut b = vec![0u8; 32];
        b[4..8].copy_from_slice(&4u32.to_le_bytes());
        b[8..12].copy_from_slice(&(n as u32).to_le_bytes());
        b[15] = 5;
        b[16..20].copy_from_slice(&32u32.to_le_bytes());
        for per in [9u64, 1, 3, 3, 4] {
            b.extend_from_slice(&8u64.to_le_bytes());
            b.extend_from_slice(&(per * n).to_le_bytes());
        }
        for _ in 0..5 {
            b.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD, 0, 0, 0, 0]);
        }
        let budget = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(64 << 20));
        let e = specular_core::budget::with_budget(budget.clone(), || {
            read_v4(&b, &Limits::default()).map(|_| ())
        })
        .unwrap_err();
        assert!(matches!(e, Error::TooLarge(_)), "{e}");
        assert_eq!(budget.reserved(), 0);
    }

    #[test]
    fn smallest_three_decodes() {
        // Identity: w (index 3) dropped, x = y = z = 0.
        assert_eq!(smallest_three(3 << 30), [1.0, 0.0, 0.0, 0.0]);
        // x largest (index 0); y = −0.5·√½·511/511… take y = full negative magnitude.
        let r = (0x200 | 511) << 20;
        let q = smallest_three(r);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!((q[2] + h).abs() < 1e-6, "{q:?}");
        assert!((q[1] - h).abs() < 1e-6, "{q:?}");
    }
}

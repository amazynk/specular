//! Deep chunks (NONE, RLE, ZIPS, ZIP, ZSTD), flattened to one sample per pixel.
//!
//! A deep chunk is three `u64` sizes (packed sample-count table, packed sample data, unpacked
//! sample data), the sample-count table (per line, each pixel's running total of samples on
//! that line, as `i32`) and the sample data: line after line, and within a line channel after
//! channel, every sample of the line's pixels in pixel order. Either section is stored raw when
//! compression would not shrink it.
//!
//! Flattening sorts each pixel's samples by `Z` (stable, so ties keep file order) and composites
//! them front to back with "over" using `A`, in `f32`; `Z`, `ZBack` and UINT channels take the
//! front-most sample, and without `A` every channel does. Pixels without samples are zero.

use crate::codec::{inflate, rle, unpredict_interleave};
use crate::half::{f2h, h2f};
use crate::header::{Channel, Compression, PixelType};

/// Largest expansion zlib can reach (about 1032:1), with slack; RLE's is 64:1.
const MAX_RATIO: u64 = 1100;

/// Per-thread buffers.
#[derive(Default)]
pub(crate) struct DeepScratch {
    table: Vec<u8>,
    samples: Vec<u8>,
    tmp: Vec<u8>,
    /// Per pixel: index of its first sample and its sample count.
    first: Vec<(usize, usize)>,
    order: Vec<(f32, usize)>,
}

/// Unpack one section of `n` bytes from `input` (`input.len() == n` means stored raw). `rows`
/// are the section's packed rows (per line, per channel: samples, element bytes) for ZSTD.
fn unpack(
    c: Compression,
    input: &[u8],
    out: &mut Vec<u8>,
    tmp: &mut Vec<u8>,
    n: usize,
    rows: &[(usize, usize)],
) -> Option<()> {
    out.clear();
    out.resize(n, 0);
    if input.len() == n {
        out.copy_from_slice(input);
        return Some(());
    }
    if c == Compression::Zstd {
        return crate::zstd::decompress(input, rows, out);
    }
    tmp.clear();
    tmp.resize(n, 0);
    match c {
        Compression::Rle => rle(input, tmp)?,
        Compression::Zips | Compression::Zip => inflate(input, tmp)?,
        _ => return None,
    }
    unpredict_interleave(tmp, out);
    Some(())
}

/// Flatten one deep chunk of `w` x `h` pixels (`raw` starts at its three sizes) into `out` in
/// OpenEXR's flat uncompressed layout (per line, per channel, `w` samples of its type).
pub(crate) fn flatten(
    raw: &[u8],
    c: Compression,
    chans: &[Channel],
    w: usize,
    h: usize,
    s: &mut DeepScratch,
    out: &mut Vec<u8>,
) -> Option<()> {
    if chans.iter().any(|c| c.x_sampling != 1 || c.y_sampling != 1) {
        return None;
    }
    let size = |i: usize| -> Option<u64> {
        Some(u64::from_le_bytes(
            raw.get(i * 8..i * 8 + 8)?.try_into().ok()?,
        ))
    };
    let (table_size, packed_size, unpacked_size) = (size(0)?, size(1)?, size(2)?);
    let table_end = 24usize.checked_add(usize::try_from(table_size).ok()?)?;
    let table_in = raw.get(24..table_end)?;
    let samples_in =
        raw.get(table_end..table_end.checked_add(usize::try_from(packed_size).ok()?)?)?;
    let npx = w.checked_mul(h)?;
    let table_bytes = npx.checked_mul(4)?;
    // Uncompressed edge tiles keep a whole tile's table (OpenEXR 3.5 writes 32 x 16 entries
    // for a 26 x 16 tile): the table proper comes first, packed at the tile's own width.
    let table_in = if c == Compression::None && table_in.len() > table_bytes {
        &table_in[..table_bytes]
    } else {
        table_in
    };
    if table_in.len() > table_bytes {
        return None;
    }
    let rows = vec![(w, 4); h];
    unpack(c, table_in, &mut s.table, &mut s.tmp, table_bytes, &rows)?;

    // Sample counts; running totals restart on every line.
    s.first.clear();
    let mut total = 0usize;
    for line in s.table.chunks_exact(w * 4) {
        let mut prev = 0usize;
        for v in line.chunks_exact(4) {
            let run = usize::try_from(i32::from_le_bytes(v.try_into().ok()?)).ok()?;
            let n = run.checked_sub(prev)?;
            s.first.push((total + prev, n));
            prev = run;
        }
        total = total.checked_add(prev)?;
    }
    let sample_bytes: usize = chans.iter().map(|c| c.pixel_type.bytes()).sum();
    let unpacked = total.checked_mul(sample_bytes)?;
    if unpacked_size != unpacked as u64
        || packed_size > unpacked_size
        || unpacked_size > packed_size.saturating_mul(MAX_RATIO).saturating_add(1024)
    {
        return None;
    }
    let mut rows = Vec::with_capacity(h * chans.len());
    for line in s.table.chunks_exact(w * 4) {
        let n =
            usize::try_from(i32::from_le_bytes(line[line.len() - 4..].try_into().ok()?)).ok()?;
        rows.extend(chans.iter().map(|ch| (n, ch.pixel_type.bytes())));
    }
    unpack(c, samples_in, &mut s.samples, &mut s.tmp, unpacked, &rows)?;
    if h > 1 {
        // Line after line, channel after channel within a line: regroup channel after channel.
        s.tmp.clear();
        s.tmp.resize(unpacked, 0);
        let mut src = 0usize;
        let mut line_start = 0usize;
        for line in s.table.chunks_exact(w * 4) {
            let n = usize::try_from(i32::from_le_bytes(line[line.len() - 4..].try_into().ok()?))
                .ok()?;
            let mut dst = 0usize;
            for ch in chans {
                let b = ch.pixel_type.bytes();
                let d = dst + line_start * b;
                s.tmp[d..d + n * b].copy_from_slice(&s.samples[src..src + n * b]);
                src += n * b;
                dst += total * b;
            }
            line_start += n;
        }
        std::mem::swap(&mut s.tmp, &mut s.samples);
    }

    // Start of each channel's samples.
    let mut base = Vec::with_capacity(chans.len());
    let mut acc = 0usize;
    for ch in chans {
        base.push(acc);
        acc += total * ch.pixel_type.bytes();
    }
    let value = |ci: usize, k: usize| -> f32 {
        let ch = &chans[ci];
        let b = ch.pixel_type.bytes();
        let p = &s.samples[base[ci] + k * b..][..b];
        match ch.pixel_type {
            PixelType::Half => h2f(u16::from_le_bytes([p[0], p[1]])),
            PixelType::Float => f32::from_le_bytes([p[0], p[1], p[2], p[3]]),
            PixelType::Uint => u32::from_le_bytes([p[0], p[1], p[2], p[3]]) as f32,
        }
    };
    let find = |name: &str| chans.iter().position(|c| c.name == name);
    let (z, alpha) = (find("Z"), find("A"));
    let front: Vec<bool> = chans
        .iter()
        .map(|c| {
            alpha.is_none() || c.name == "Z" || c.name == "ZBack" || c.pixel_type == PixelType::Uint
        })
        .collect();

    let line_bytes: usize = chans.iter().map(|c| w * c.pixel_type.bytes()).sum();
    out.clear();
    out.resize(line_bytes * h, 0);
    let mut acc = vec![0f32; chans.len()];
    for (p, &(first, n)) in s.first.iter().enumerate() {
        if n == 0 {
            continue;
        }
        // Front to back; NaN depths last, -0 equal to +0.
        s.order.clear();
        s.order.extend((first..first + n).map(|k| {
            let d = z.map_or(0.0, |zc| value(zc, k));
            (if d.is_nan() { f32::INFINITY } else { d + 0.0 }, k)
        }));
        s.order
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if let Some(ai) = alpha {
            acc.fill(0.0);
            let mut a = 0f32;
            for &(_, k) in &s.order {
                let t = 1.0 - a;
                for (ci, v) in acc.iter_mut().enumerate() {
                    if !front[ci] && ci != ai {
                        *v += t * value(ci, k);
                    }
                }
                a += t * value(ai, k);
            }
            acc[ai] = a;
        }
        let (y, x) = (p / w, p % w);
        let mut o = y * line_bytes;
        for (ci, ch) in chans.iter().enumerate() {
            let b = ch.pixel_type.bytes();
            let d = &mut out[o + x * b..][..b];
            o += w * b;
            if front[ci] {
                let k = s.order[0].1;
                d.copy_from_slice(&s.samples[base[ci] + k * b..][..b]);
                continue;
            }
            match ch.pixel_type {
                PixelType::Half => d.copy_from_slice(&f2h(acc[ci]).to_le_bytes()),
                _ => d.copy_from_slice(&acc[ci].to_le_bytes()),
            }
        }
    }
    Some(())
}

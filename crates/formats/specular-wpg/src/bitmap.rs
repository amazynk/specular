//! Bitmap data: the two run-length codings, and packed pixels to RGBA.

use specular_core::NeedsMemory;
use specular_drawpage::Rgba;

/// Output size (RGBA bytes) above which a bitmap must have data for at least 1/64 of its rows.
const SPARSE_LIMIT: usize = 64 << 20;

/// Version 1 run-length coding, into rows of `stride` bytes, `rows` of them at most.
/// Opcodes: `1nnnnnnn v` repeats `v` n times (n = 0: the next byte counts 0xFF bytes);
/// `0nnnnnnn` copies n literal bytes (n = 0: the next byte repeats the previous row that many
/// times). Returns the bytes and the number of row repeats used.
pub(crate) fn rle1(
    data: &[u8],
    stride: usize,
    rows: usize,
) -> Result<(Vec<u8>, usize), NeedsMemory> {
    let total = stride.saturating_mul(rows);
    // Row repeats expand without bound in the data: the rows are reserved first, and a
    // bitmap the memory budget cannot hold is refused (the caller says so).
    let _res = specular_core::budget::reserve(total as u64)?;
    let mut out: Vec<u8> = Vec::with_capacity(total.min(1 << 28));
    let mut repeats = 0;
    let mut i = 0;
    while out.len() < total && i < data.len() {
        let op = data[i];
        i += 1;
        let n = usize::from(op & 0x7F);
        if op & 0x80 != 0 {
            let Some(&v) = data.get(i) else { break };
            i += 1;
            let (n, v) = if n == 0 {
                (usize::from(v), 0xFF)
            } else {
                (n, v)
            };
            let n = n.min(total - out.len());
            out.resize(out.len() + n, v);
        } else if n == 0 {
            let Some(&c) = data.get(i) else { break };
            i += 1;
            repeats += usize::from(c);
            let end = out.len() / stride.max(1) * stride;
            out.truncate(end);
            for _ in 0..c {
                if out.len() >= total {
                    break;
                }
                if end >= stride && stride > 0 {
                    out.extend_from_within(end - stride..end);
                } else {
                    out.resize(out.len() + stride, 0xFF);
                }
            }
        } else {
            let take = n.min(data.len() - i).min(total - out.len());
            out.extend_from_slice(&data[i..i + take]);
            i += n;
        }
    }
    Ok((out, repeats))
}

/// The row stride of version 1 bitmap data: the packed row size, or (some writers pad rows)
/// a byte or two more when the decoded data fills exactly that many rows.
pub(crate) fn rle1_stride(data: &[u8], row: usize, rows: usize) -> usize {
    if row == 0 || rows == 0 {
        return row;
    }
    // A probe the budget refuses: the packed size (the decode itself is refused after).
    let Ok((probe, repeats)) = rle1(data, row + 3, rows) else {
        return row;
    };
    if repeats > 0 {
        return row;
    }
    let len = probe.len();
    for extra in 1..=2 {
        let s = row + extra;
        if len == s * rows && len != row * rows {
            return s;
        }
    }
    row
}

/// Version 2 run-length coding (sample-oriented): `7D n` sets the sample size to n bytes;
/// `7F n` n+1 black samples; `FF n` n+1 white samples; `FD n` repeats the last sample n+1
/// times; `FE n` repeats the previous row n times; `1nnnnnnn s` repeats sample `s` n+1 times;
/// `0nnnnnnn …` copies n+1 literal samples. `7E` (XOR) is skipped.
pub(crate) fn rle2(
    data: &[u8],
    stride: usize,
    rows: usize,
    sample: usize,
) -> Result<Vec<u8>, NeedsMemory> {
    let total = stride.saturating_mul(rows);
    // As in `rle1`.
    let _res = specular_core::budget::reserve(total as u64)?;
    let mut out: Vec<u8> = Vec::with_capacity(total.min(1 << 28));
    let mut ss = sample.clamp(1, 8);
    let mut last: Vec<u8> = vec![0; ss];
    let mut i = 0;
    let push = |out: &mut Vec<u8>, v: &[u8], times: usize| {
        for _ in 0..times {
            if out.len() >= total {
                return;
            }
            let room = total - out.len();
            out.extend_from_slice(&v[..v.len().min(room)]);
        }
    };
    while out.len() < total && i < data.len() {
        let op = data[i];
        i += 1;
        let Some(&n) = data.get(i) else { break };
        match op {
            0x7D => {
                i += 1;
                if (1..=8).contains(&n) {
                    ss = usize::from(n);
                    last = vec![0; ss];
                }
            }
            0x7E => i += 1,
            0x7F | 0xFF => {
                i += 1;
                let v = if op == 0x7F { 0 } else { 0xFF };
                let n = (usize::from(n) + 1) * ss;
                let n = n.min(total - out.len());
                out.resize(out.len() + n, v);
            }
            0xFD => {
                i += 1;
                let l = last.clone();
                push(&mut out, &l, usize::from(n) + 1);
            }
            0xFE => {
                i += 1;
                let end = out.len() / stride.max(1) * stride;
                out.truncate(end);
                for _ in 0..n {
                    if out.len() >= total || stride == 0 {
                        break;
                    }
                    if end >= stride {
                        out.extend_from_within(end - stride..end);
                    } else {
                        out.resize(out.len() + stride, 0xFF);
                    }
                }
            }
            _ if op & 0x80 != 0 => {
                let Some(s) = data.get(i..i + ss) else { break };
                i += ss;
                last.copy_from_slice(s);
                let l = last.clone();
                push(&mut out, &l, usize::from(op & 0x7F) + 1);
            }
            _ => {
                let len = (usize::from(op) + 1) * ss;
                let end = (i + len).min(data.len());
                let s = &data[i..end];
                push(&mut out, s, 1);
                if s.len() >= ss {
                    last.copy_from_slice(&s[s.len() - ss..]);
                }
                i = end;
            }
        }
    }
    Ok(out)
}

/// Packed rows (top-down, `stride` bytes each; short data is white) to straight RGBA.
/// `bits` 1, 2, 4 or 8 index `palette`; 24 is RGB.
pub(crate) fn to_rgba(
    raw: &[u8],
    stride: usize,
    w: usize,
    h: usize,
    bits: u32,
    palette: &[Rgba],
) -> Result<Option<Vec<u8>>, NeedsMemory> {
    if !matches!(bits, 1 | 2 | 4 | 8 | 24) || w == 0 || h == 0 {
        return Ok(None);
    }
    let Some(bytes) = w.checked_mul(h).and_then(|n| n.checked_mul(4)) else {
        return Ok(None);
    };
    // A large bitmap whose data covers almost none of its rows is a header claiming a size
    // the file does not hold: refused rather than allocated white.
    if bytes > SPARSE_LIMIT && raw.len().saturating_mul(64) < stride.saturating_mul(h) {
        return Ok(None);
    }
    // The packed rows are still held while the RGBA is made.
    let _res = specular_core::budget::reserve((bytes + raw.len()) as u64)?;
    let mut out = vec![255u8; bytes];
    let get = |i: usize| -> Rgba {
        palette.get(i).copied().unwrap_or(if i == 0 {
            [0, 0, 0, 255]
        } else {
            [255, 255, 255, 255]
        })
    };
    for y in 0..h {
        let Some(row) = raw.get(y * stride..) else {
            break;
        };
        let o = &mut out[y * w * 4..(y + 1) * w * 4];
        for x in 0..w {
            let c: Rgba = if bits == 24 {
                let Some(p) = row.get(x * 3..x * 3 + 3) else {
                    break;
                };
                [p[0], p[1], p[2], 255]
            } else {
                let b = bits as usize;
                let bit = x * b;
                let Some(&byte) = row.get(bit / 8) else { break };
                let shift = 8 - b - (bit % 8);
                let idx = (usize::from(byte) >> shift) & ((1 << b) - 1);
                get(idx)
            };
            o[x * 4..x * 4 + 4].copy_from_slice(&c);
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    /// A 16 384 × 16 384 bitmap header with 16 bytes of data (1 GiB of white RGBA before)
    /// is refused; a small truncated bitmap still shows white below its data.
    #[test]
    fn sparse_bitmap_is_refused() {
        assert!(
            super::to_rgba(&[0; 16], 16_384, 16_384, 16_384, 8, &[])
                .unwrap()
                .is_none()
        );
        let small = super::to_rgba(&[0; 4], 4, 4, 4, 8, &[]).unwrap().unwrap();
        assert_eq!((small.len(), small[4 * 4 * 4 - 1]), (64, 255));
    }

    /// A few bytes of row repeats unpack to 256 MiB: refused by a 64 MiB budget before the
    /// rows are written (both codings), and the RGBA of a bitmap too large for it as well:
    /// each a memory refusal, which the drawing reports.
    #[test]
    fn unpacking_reserves_from_the_budget() {
        use specular_core::budget::with_limit;
        let (stride, rows) = (16_384, 16_384);
        // One literal byte, then the row repeated 255 times, over and over.
        let mut v1 = vec![0x81, 7, 0x00, 255];
        let mut v2 = vec![0x80, 7, 0xFE, 255];
        for _ in 0..64 {
            v1.extend([0x00, 255]);
            v2.extend([0xFE, 255]);
        }
        assert!(with_limit(64 << 20, || super::rle1(&v1, stride, rows)).is_err());
        assert!(with_limit(64 << 20, || super::rle2(&v2, stride, rows, 1)).is_err());
        let raw = vec![0u8; 4096 * 4096];
        assert!(with_limit(64 << 20, || super::to_rgba(&raw, 4096, 4096, 4096, 8, &[])).is_err());
        assert!(
            super::to_rgba(&raw, 4096, 4096, 4096, 8, &[])
                .unwrap()
                .is_some()
        );
    }
}

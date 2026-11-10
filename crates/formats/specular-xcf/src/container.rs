//! `.xcf.gz`, `.xcf.bz2` and `.xcf.xz`: the whole file compressed by an external tool (GIMP
//! saves these through its compressed-file plug-in).

use specular_core::Reservation;

use crate::{Error, MAX_DECOMPRESSED, Result};

/// What wraps the XCF bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Raw,
    Gzip,
    Bzip2,
    Xz,
}

/// Recognises the container by its magic bytes (does not look inside).
pub fn container(data: &[u8]) -> Container {
    if data.starts_with(&[0x1f, 0x8b]) {
        Container::Gzip
    } else if data.len() >= 4 && data.starts_with(b"BZh") && (b'1'..=b'9').contains(&data[3]) {
        Container::Bzip2
    } else if data.starts_with(&XZ_MAGIC) {
        Container::Xz
    } else {
        Container::Raw
    }
}

/// Decompresses a gzip, bzip2 or xz container; `Ok(None)` for anything else (taken as raw
/// XCF). The result is not checked to be XCF.
pub fn decompress(data: &[u8]) -> Result<Option<Vec<u8>>> {
    Ok(decompress_reserved(data)?.map(|(out, _)| out))
}

/// [`decompress`], with the memory budget's share of the output: reserved as the output
/// grew, and handed over with it for as long as the caller keeps the bytes.
pub fn decompress_reserved(data: &[u8]) -> Result<Option<(Vec<u8>, Reservation)>> {
    Ok(Some(match container(data) {
        Container::Raw => return Ok(None),
        Container::Gzip => gzip(data, MAX_DECOMPRESSED)?,
        Container::Bzip2 => crate::bzip2::decompress(data, MAX_DECOMPRESSED)?,
        Container::Xz => xz(data, MAX_DECOMPRESSED)?,
    }))
}

/// Decompresses a bzip2 stream (or several, concatenated) on its own, stopping once `limit`
/// bytes are out (the result is then cut to `limit`); for other formats' bzip2 blocks (ASDF).
pub fn bunzip2(data: &[u8], limit: u64) -> Result<Vec<u8>> {
    crate::bzip2::decompress(data, limit).map(|(out, _)| out)
}

/// Whether a compressed container holds an XCF file (decompresses just its start).
pub(crate) fn probe_compressed(data: &[u8]) -> bool {
    let head = match container(data) {
        Container::Raw => return false,
        Container::Gzip => gzip(data, 64),
        Container::Bzip2 => crate::bzip2::decompress(data, 64),
        Container::Xz => xz(data, 64),
    };
    head.is_ok_and(|(h, _)| crate::probe(&h))
}

/// Doubles the output, reserving it from the memory budget first.
fn grow(out: &mut Vec<u8>, limit: u64, res: &mut Reservation) -> Result<()> {
    if out.len() as u64 >= limit {
        return Err(Error::TooLarge);
    }
    let want = (out.len().max(1 << 16) * 2).min(limit as usize);
    res.grow_to(want as u64)?;
    out.resize(want, 0);
    Ok(())
}

/// Room for at least `n` more bytes in `out`, reserved from the budget before it is
/// allocated (doubling, at least 64 KiB, at most `limit` plus `n`).
#[cold]
pub(crate) fn make_room(
    out: &mut Vec<u8>,
    n: usize,
    limit: u64,
    res: &mut Reservation,
) -> Result<()> {
    let cap = (out.capacity().saturating_mul(2))
        .max(1 << 16)
        .min(
            usize::try_from(limit)
                .unwrap_or(usize::MAX)
                .saturating_add(n),
        )
        .max(out.len() + n);
    res.grow_to(cap as u64)?;
    out.reserve_exact(cap - out.len());
    Ok(())
}

/// Keeps the budget's reservation up with a buffer that grew by itself.
pub(crate) fn track(out: &Vec<u8>, res: &mut Reservation) -> Result<()> {
    if out.capacity() as u64 > res.bytes() {
        res.grow_to(out.capacity() as u64)?;
    }
    Ok(())
}

/// All gzip members, concatenated. Stops (keeping the output) at `limit` bytes; a truncated
/// or corrupt stream is an error unless at least `limit` bytes came out.
fn gzip(data: &[u8], limit: u64) -> Result<(Vec<u8>, Reservation)> {
    let mut res = Reservation::none();
    let mut out = Vec::new();
    let mut len = 0usize;
    let mut input = data;
    while input.starts_with(&[0x1f, 0x8b]) {
        // window bits 16 + 15: gzip header and CRC checked by the inflater.
        let mut inf = zlib_rs::Inflate::new(true, 31);
        loop {
            if len == out.len() {
                if len as u64 >= limit {
                    out.truncate(len);
                    return Ok((out, res));
                }
                grow(&mut out, limit, &mut res)?;
            }
            let before_in = inf.total_in();
            let before_out = inf.total_out();
            let r = inf.decompress(
                &input[before_in as usize..],
                &mut out[len..],
                zlib_rs::InflateFlush::NoFlush,
            );
            len += (inf.total_out() - before_out) as usize;
            match r {
                Ok(zlib_rs::Status::StreamEnd) => break,
                Ok(_) => {
                    if inf.total_in() == before_in
                        && inf.total_out() == before_out
                        && len < out.len()
                    {
                        // No progress with output room: the input ran out.
                        return truncated_or(out, len, limit, res);
                    }
                }
                Err(_) => return truncated_or(out, len, limit, res),
            }
        }
        input = &input[inf.total_in() as usize..];
    }
    if len == 0 {
        return Err(Error::NotXcf);
    }
    out.truncate(len);
    Ok((out, res))
}

fn truncated_or(
    mut out: Vec<u8>,
    len: usize,
    limit: u64,
    res: Reservation,
) -> Result<(Vec<u8>, Reservation)> {
    if len as u64 >= limit {
        out.truncate(len);
        Ok((out, res))
    } else {
        Err(Error::Truncated("compressed stream".into()))
    }
}

const XZ_MAGIC: [u8; 6] = [0xfd, b'7', b'z', b'X', b'Z', 0];

/// An xz stream (or several, concatenated): blocks of LZMA2 (the only filter accepted),
/// checks skipped (not verified), index and footer skipped.
fn xz(data: &[u8], limit: u64) -> Result<(Vec<u8>, Reservation)> {
    let mut res = Reservation::none();
    let mut out = Vec::new();
    let mut p = 0usize;
    let bad = || Error::Malformed("xz container".into());
    while data.get(p..).is_some_and(|d| d.starts_with(&XZ_MAGIC)) {
        let flags = data.get(p + 6..p + 8).ok_or_else(bad)?;
        let check = flags[1] & 0x0f;
        let check_size = match check {
            0 => 0,
            1..=3 => 4,
            4..=6 => 8,
            7..=9 => 16,
            _ => 32,
        };
        p += 12;
        // Blocks until the index indicator (0).
        loop {
            let hs = *data.get(p).ok_or_else(bad)?;
            if hs == 0 {
                break;
            }
            let hlen = (usize::from(hs) + 1) * 4;
            let h = data.get(p..p + hlen).ok_or_else(bad)?;
            let bflags = h[1];
            let filters = usize::from(bflags & 3) + 1;
            let mut q = 2;
            let varint = |q: &mut usize| -> Result<u64> {
                let mut v = 0u64;
                for i in 0..9 {
                    let b = *h.get(*q).ok_or_else(bad)?;
                    *q += 1;
                    v |= u64::from(b & 0x7f) << (7 * i);
                    if b & 0x80 == 0 {
                        return Ok(v);
                    }
                }
                Err(bad())
            };
            if bflags & 0x40 != 0 {
                varint(&mut q)?;
            }
            if bflags & 0x80 != 0 {
                varint(&mut q)?;
            }
            let mut dict = None;
            for _ in 0..filters {
                let id = varint(&mut q)?;
                let psize = varint(&mut q)? as usize;
                let props = h.get(q..q + psize).ok_or_else(bad)?;
                q += psize;
                if id != 0x21 || psize != 1 || filters != 1 {
                    return Err(Error::Unsupported(format!("xz filter {id:#x}")));
                }
                let bits = u32::from(props[0] & 0x3f);
                if bits > 40 {
                    return Err(bad());
                }
                dict = Some(if bits == 40 {
                    u32::MAX
                } else {
                    (2 | (bits & 1)) << (bits / 2 + 11)
                });
            }
            let dict = dict.ok_or_else(bad)?;
            p += hlen;
            // Raw LZMA2 to its end marker; the reader consumes exactly the block's bytes.
            let mut rest = &data[p..];
            // The reader allocates and zeroes the whole dictionary up front, and a tiny file
            // may declare 4 GiB. A dictionary past the block's output is never used, and LZMA
            // expands at most ~7,500:1 (a 273-byte match takes at least 13 range-coder
            // decisions of over 0.02 bits each): cap it there, with margin, and at the limit.
            let most = (rest.len() as u64)
                .saturating_mul(1 << 14)
                .saturating_add(1 << 20)
                .min(limit - out.len() as u64);
            let dict = dict.min(u32::try_from(most).unwrap_or(u32::MAX));
            // The dictionary, for the block.
            let _dict = specular_core::budget::reserve(u64::from(dict))?;
            let before = rest.len();
            {
                use std::io::Read;
                let mut rd = lzma_rust2::Lzma2Reader::new(&mut rest, dict, None);
                let mut buf = vec![0u8; 1 << 16];
                loop {
                    let room = (limit - out.len() as u64).min(buf.len() as u64) as usize;
                    if room == 0 {
                        return Ok((out, res));
                    }
                    match rd.read(&mut buf[..room]) {
                        Ok(0) => break,
                        Ok(n) => {
                            if out.capacity() - out.len() < n {
                                make_room(&mut out, n, limit, &mut res)?;
                            }
                            out.extend_from_slice(&buf[..n]);
                        }
                        Err(_) => {
                            let len = out.len();
                            return truncated_or(out, len, limit, res);
                        }
                    }
                }
            }
            p += before - rest.len();
            // Block padding to a multiple of four, then the check.
            p = p.next_multiple_of(4) + check_size;
        }
        // Index: indicator, record count, records (two varints each), padding, CRC32;
        // then the 12-byte footer whose backward size tells the index length.
        let Some(after_index) = index_end(data, p) else {
            break;
        };
        p = after_index + 12;
        // Stream padding (zeros, multiple of four).
        while data.get(p..p + 4) == Some(&[0, 0, 0, 0]) {
            p += 4;
        }
        if p >= data.len() {
            break;
        }
    }
    if out.is_empty() {
        return Err(Error::Truncated("xz stream".into()));
    }
    Ok((out, res))
}

/// End of the xz index starting at `p` (its indicator byte), or `None` when cut short.
fn index_end(data: &[u8], mut p: usize) -> Option<usize> {
    let start = p;
    p += 1;
    let varint = |p: &mut usize| -> Option<u64> {
        let mut v = 0u64;
        for i in 0..9 {
            let b = *data.get(*p)?;
            *p += 1;
            v |= u64::from(b & 0x7f) << (7 * i);
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    };
    let n = varint(&mut p)?;
    for _ in 0..n {
        varint(&mut p)?;
        varint(&mut p)?;
    }
    p = start + (p - start).next_multiple_of(4) + 4;
    (p <= data.len()).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; data.len() + 1024];
        let cfg = zlib_rs::DeflateConfig {
            window_bits: 31,
            ..Default::default()
        };
        let (c, rc) = zlib_rs::compress_slice(&mut out, data, cfg);
        assert_eq!(rc, zlib_rs::ReturnCode::Ok);
        c.to_vec()
    }

    #[test]
    fn gzip_members() {
        let a: Vec<u8> = (0..100_000u32).map(|i| (i * 7 / 13) as u8).collect();
        let mut f = gz(&a[..60_000]);
        f.extend(gz(&a[60_000..]));
        assert_eq!(container(&f), Container::Gzip);
        assert_eq!(decompress(&f).unwrap().unwrap(), a);
        assert_eq!(gzip(&f, 64).unwrap().0, &a[..64]);
        assert!(gzip(&f[..f.len() / 3], MAX_DECOMPRESSED).is_err());
    }

    #[test]
    fn gzip_within_memory_budget() {
        // 4 MiB of zeros in a 4 KiB file: refused under a 1 MiB budget, read under 64 MiB.
        let f = gz(&vec![0u8; 4 << 20]);
        let e = specular_core::budget::with_limit(1 << 20, || decompress(&f)).unwrap_err();
        let Error::NeedsMemory(n) = e else {
            panic!("{e:?}")
        };
        assert!(n.needed > n.free, "{n:?}");
        let ok = specular_core::budget::with_limit(64 << 20, || decompress(&f));
        assert_eq!(ok.unwrap().unwrap().len(), 4 << 20);
    }

    #[test]
    fn bzip2_all_byte_values() {
        // Made with Python's bz2 at level 9: every byte value in use (256 MTF symbols).
        let f = include_bytes!("../tests/data/all-bytes.bz2");
        let mut want: Vec<u8> = (0..=255u8).cycle().take(768).collect();
        want.extend(b"gimp xcf v011");
        assert_eq!(container(f), Container::Bzip2);
        assert_eq!(decompress(f).unwrap().unwrap(), want);
        assert!(decompress(&f[..f.len() - 10]).is_err());
    }
}

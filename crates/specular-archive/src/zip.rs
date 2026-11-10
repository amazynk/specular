//! ZIP / CBZ: central-directory parser (ZIP64, self-extractor prefixes, CP437 and Info-ZIP
//! Unicode names); stored and Deflate entries (zlib-rs), and Deflate64, bzip2, LZMA, Zstandard and
//! xz entries (the own decoders in `codec`).
//!
//! Opening reads only the end records and the central directory. Local headers are read when an
//! entry is read, so listing a 500-page CBZ touches a few pages of the file.

use crate::codec::{self, Threads};
use crate::inflate::inflater;
use crate::names::{decode_zip_name, is_dir_name, normalize};
use crate::zip_crypto::{self, Crypt};
use crate::{Entry, Error, Limits, Loc, Method, Password, Result};

const EOCD_SIG: u32 = 0x0605_4b50;
const EOCD64_SIG: u32 = 0x0606_4b50;
const LOCATOR64_SIG: u32 = 0x0706_4b50;
const CDH_SIG: u32 = 0x0201_4b50;
const LFH_SIG: u32 = 0x0403_4b50;
const EOCD_LEN: usize = 22;
const CDH_LEN: usize = 46;
const LFH_LEN: usize = 30;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("ZIP: {what}"))
}

/// Where an entry lives, from the central directory.
#[derive(Debug, Clone)]
pub(crate) struct ZipLoc {
    /// Offset of the local header, already corrected for any prefix.
    local: u64,
    crc: u32,
    /// How the entry is encrypted (`zip_crypto`), if it is.
    crypt: Option<Crypt>,
}

/// Archive-wide facts needed when reading.
#[derive(Debug)]
pub(crate) struct ZipInfo {
    /// Start of the central directory: entry data must end before it.
    cd_start: u64,
}

/// Position of the end-of-central-directory record, searching back over a comment of up to
/// 64 KiB.
pub(crate) fn find_eocd(b: &[u8]) -> Option<usize> {
    if b.len() < EOCD_LEN {
        return None;
    }
    let last = b.len() - EOCD_LEN;
    let first = last.saturating_sub(0xFFFF);
    (first..=last).rev().find(|&p| {
        u32_at(b, p) == Some(EOCD_SIG)
            && u16_at(b, p + 20).is_some_and(|c| p + EOCD_LEN + c as usize <= b.len())
    })
}

struct Directory {
    entries: u64,
    cd_size: u64,
    cd_offset: u64,
    /// Where the directory must end: the EOCD64 record if present, else the EOCD.
    cd_end: u64,
}

fn read_directory(b: &[u8]) -> Result<Directory> {
    let p = find_eocd(b).ok_or_else(|| malformed("no end-of-central-directory record"))?;
    let disk = u16_at(b, p + 4).unwrap_or(0);
    let cd_disk = u16_at(b, p + 6).unwrap_or(0);
    let mut dir = Directory {
        entries: u16_at(b, p + 10).unwrap_or(0) as u64,
        cd_size: u32_at(b, p + 12).unwrap_or(0) as u64,
        cd_offset: u32_at(b, p + 16).unwrap_or(0) as u64,
        cd_end: p as u64,
    };
    let needs64 =
        dir.entries == 0xFFFF || dir.cd_size == 0xFFFF_FFFF || dir.cd_offset == 0xFFFF_FFFF;
    // ZIP64 locator sits right before the EOCD.
    let loc = p
        .checked_sub(20)
        .filter(|&l| u32_at(b, l) == Some(LOCATOR64_SIG));
    if let Some(l) = loc {
        let declared = u64_at(b, l + 8).unwrap_or(u64::MAX);
        // With a prefix the declared offset is off; the record normally sits right before the
        // locator (56 bytes without extensible data).
        let at = [Some(declared), l.checked_sub(56).map(|x| x as u64)]
            .into_iter()
            .flatten()
            .filter_map(|o| usize::try_from(o).ok())
            .find(|&o| u32_at(b, o) == Some(EOCD64_SIG));
        match at {
            Some(o) => {
                let disk64 = u32_at(b, o + 16).unwrap_or(0);
                let cd_disk64 = u32_at(b, o + 20).unwrap_or(0);
                if disk64 != 0 || cd_disk64 != 0 {
                    return Err(Error::Unsupported("multi-volume ZIP".into()));
                }
                dir.entries = u64_at(b, o + 32).ok_or_else(|| malformed("short ZIP64 record"))?;
                dir.cd_size = u64_at(b, o + 40).ok_or_else(|| malformed("short ZIP64 record"))?;
                dir.cd_offset = u64_at(b, o + 48).ok_or_else(|| malformed("short ZIP64 record"))?;
                dir.cd_end = o as u64;
            }
            None if needs64 => return Err(malformed("ZIP64 end record missing")),
            None => {}
        }
    } else if (disk != 0 || cd_disk != 0) && !needs64 {
        return Err(Error::Unsupported("multi-volume ZIP".into()));
    }
    Ok(dir)
}

/// Lists the entries. Folders and symbolic links are left out.
pub(crate) fn list(b: &[u8], limits: &Limits) -> Result<(Vec<Entry>, ZipInfo)> {
    let dir = read_directory(b)?;
    if dir.entries > limits.max_entries as u64 {
        return Err(Error::Limit(format!(
            "{} entries exceed the limit of {}",
            dir.entries, limits.max_entries
        )));
    }
    if dir.cd_size > dir.cd_end || dir.cd_size < dir.entries.saturating_mul(CDH_LEN as u64) {
        return Err(malformed("central directory size inconsistent"));
    }
    // Self-extractors and files with a prefix: the declared offset misses the directory, which
    // really ends at the end record, so every offset is shifted by the same delta.
    let declared_ok = dir.entries == 0
        || usize::try_from(dir.cd_offset)
            .ok()
            .and_then(|o| u32_at(b, o))
            == Some(CDH_SIG);
    let actual_start = if declared_ok && dir.cd_offset <= dir.cd_end {
        dir.cd_offset
    } else {
        dir.cd_end - dir.cd_size
    };
    let delta = actual_start.wrapping_sub(dir.cd_offset);
    let start = usize::try_from(actual_start).map_err(|_| malformed("offset overflow"))?;
    let end = usize::try_from(dir.cd_end).map_err(|_| malformed("offset overflow"))?;
    let cd = b
        .get(start..end)
        .ok_or_else(|| malformed("directory out of bounds"))?;

    let mut entries = Vec::with_capacity(dir.entries.min(65_536) as usize);
    // (local offset, minimum span) for the overlap check.
    let mut spans: Vec<(u64, u64)> = Vec::with_capacity(entries.capacity());
    let mut p = 0usize;
    let mut seen = 0u64;
    while seen < dir.entries {
        if u32_at(cd, p) != Some(CDH_SIG) {
            return Err(malformed("bad central directory header"));
        }
        seen += 1;
        let h = cd
            .get(p..p + CDH_LEN)
            .ok_or_else(|| malformed("truncated directory"))?;
        let made_by = u16_at(h, 4).unwrap_or(0);
        let flags = u16_at(h, 8).unwrap_or(0);
        let method = u16_at(h, 10).unwrap_or(0);
        let time = u16_at(h, 12).unwrap_or(0);
        let crc = u32_at(h, 16).unwrap_or(0);
        let mut csize = u32_at(h, 20).unwrap_or(0) as u64;
        let mut usize_ = u32_at(h, 24).unwrap_or(0) as u64;
        let name_len = u16_at(h, 28).unwrap_or(0) as usize;
        let extra_len = u16_at(h, 30).unwrap_or(0) as usize;
        let comment_len = u16_at(h, 32).unwrap_or(0) as usize;
        let ext_attr = u32_at(h, 38).unwrap_or(0);
        let mut local = u32_at(h, 42).unwrap_or(0) as u64;
        let name_at = p + CDH_LEN;
        let raw_name = cd
            .get(name_at..name_at + name_len)
            .ok_or_else(|| malformed("truncated name"))?;
        let extra = cd
            .get(name_at + name_len..name_at + name_len + extra_len)
            .ok_or_else(|| malformed("truncated extra field"))?;
        p = name_at + name_len + extra_len + comment_len;
        if p > cd.len() {
            return Err(malformed("truncated directory"));
        }

        let mut unicode_name = None;
        let mut aes_extra = None;
        let mut x = 0usize;
        while x + 4 <= extra.len() {
            let id = u16_at(extra, x).unwrap_or(0);
            let len = u16_at(extra, x + 2).unwrap_or(0) as usize;
            let Some(data) = extra.get(x + 4..x + 4 + len) else {
                break;
            };
            match id {
                0x0001 => {
                    // ZIP64: only the fields saturated in the header are present, in this order.
                    let mut q = 0;
                    let mut next = || {
                        let v = u64_at(data, q);
                        q += 8;
                        v
                    };
                    if usize_ == 0xFFFF_FFFF {
                        usize_ = next().ok_or_else(|| malformed("short ZIP64 extra"))?;
                    }
                    if csize == 0xFFFF_FFFF {
                        csize = next().ok_or_else(|| malformed("short ZIP64 extra"))?;
                    }
                    if local == 0xFFFF_FFFF {
                        local = next().ok_or_else(|| malformed("short ZIP64 extra"))?;
                    }
                }
                0x7075 if data.len() > 5 && data[0] == 1 => {
                    // Info-ZIP Unicode Path: valid only while the CRC of the raw name matches.
                    if u32_at(data, 1) == Some(crc32fast::hash(raw_name))
                        && let Ok(s) = std::str::from_utf8(&data[5..])
                    {
                        unicode_name = Some(s.to_owned());
                    }
                }
                zip_crypto::AES_EXTRA => aes_extra = Some(data),
                _ => {}
            }
            x += 4 + len;
        }

        let host = made_by >> 8;
        let unix_mode = if host == 3 { ext_attr >> 16 } else { 0 };
        let is_dir = is_dir_name(raw_name)
            || (host == 0 && ext_attr & 0x10 != 0)
            || unix_mode & 0o170000 == 0o040000;
        let is_link = unix_mode & 0o170000 == 0o120000;
        let local = local.wrapping_add(delta);
        if local >= actual_start {
            return Err(malformed("entry data after the central directory"));
        }
        spans.push((local, LFH_LEN as u64 + name_len as u64 + csize));
        if is_dir || is_link {
            continue;
        }
        let name = unicode_name.unwrap_or_else(|| decode_zip_name(raw_name, flags & 0x0800 != 0));
        // An AES entry names its real method in its 0x9901 field.
        let crypt = zip_crypto::from_header(flags, method, crc, time, aes_extra);
        let method = crypt.map_or(method, |c| c.1);
        let method_enum = match method {
            0 => Method::Stored,
            8 => Method::Deflate,
            9 => Method::Deflate64,
            12 => Method::Bzip2,
            14 => Method::Lzma,
            // 20 is the id APPNOTE first gave Zstandard (deprecated, still written by some tools).
            93 | 20 => Method::Zstd,
            95 => Method::Xz,
            98 => Method::Other("PPMd".into()),
            99 => Method::Other("AES".into()),
            m => Method::Other(format!("ZIP method {m}")),
        };
        entries.push(Entry {
            name: normalize(&name),
            size: usize_,
            compressed_size: csize,
            method: method_enum,
            encrypted: crypt.is_some(),
            loc: Loc::Zip(ZipLoc {
                local,
                crc,
                crypt: crypt.map(|c| c.0),
            }),
        });
    }

    // Overlapping entries are the "non-recursive zip bomb" (one deflate stream referenced by many
    // headers); legitimate archives never overlap.
    spans.sort_unstable_by_key(|s| s.0);
    for w in spans.windows(2) {
        if w[0].0.saturating_add(w[0].1) > w[1].0 {
            return Err(Error::Limit("overlapping ZIP entries (zip bomb)".into()));
        }
    }
    if let Some(last) = spans.last()
        && last.0.saturating_add(last.1) > actual_start
    {
        return Err(malformed("entry data runs into the central directory"));
    }
    Ok((
        entries,
        ZipInfo {
            cd_start: actual_start,
        },
    ))
}

/// Start and length of the entry's compressed data, from its local header.
pub(crate) fn data_range(
    b: &[u8],
    info: &ZipInfo,
    loc: &ZipLoc,
    e: &Entry,
) -> Result<(usize, usize)> {
    let at = usize::try_from(loc.local).map_err(|_| malformed("offset overflow"))?;
    if u32_at(b, at) != Some(LFH_SIG) {
        return Err(malformed("bad local header"));
    }
    let n = u16_at(b, at + 26).ok_or_else(|| malformed("truncated local header"))? as usize;
    let x = u16_at(b, at + 28).ok_or_else(|| malformed("truncated local header"))? as usize;
    let start = at + LFH_LEN + n + x;
    let len = usize::try_from(e.compressed_size).map_err(|_| malformed("size overflow"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| malformed("size overflow"))?;
    if end as u64 > info.cd_start || end > b.len() {
        return Err(malformed("entry data out of bounds"));
    }
    Ok((start, len))
}

#[allow(clippy::too_many_arguments)] // The decryption hook added the password.
pub(crate) fn read(
    b: &[u8],
    info: &ZipInfo,
    loc: &ZipLoc,
    e: &Entry,
    limits: &Limits,
    verify_crc: bool,
    password: Option<&Password>,
    out: &mut Vec<u8>,
) -> Result<()> {
    limits.check_entry(e.size, e.compressed_size)?;
    let (start, len) = data_range(b, info, loc, e)?;
    let data = &b[start..start + len];
    let Some(crypt) = loc.crypt else {
        return decode(data, loc, e, limits, verify_crc, out);
    };
    let plain = zip_crypto::decrypt(crypt, data, password)?;
    decode(&plain, loc, e, limits, verify_crc && crypt.has_crc(), out).map_err(|e| crypt.explain(e))
}

/// Decompresses an entry's (decrypted) compressed stream `data` into `out`.
fn decode(
    data: &[u8],
    loc: &ZipLoc,
    e: &Entry,
    limits: &Limits,
    verify_crc: bool,
    out: &mut Vec<u8>,
) -> Result<()> {
    let len = data.len();
    let size = usize::try_from(e.size).map_err(|_| malformed("size overflow"))?;
    match e.method {
        Method::Stored => {
            if len != size {
                return Err(malformed("stored entry sizes differ"));
            }
            out.extend_from_slice(data);
        }
        Method::Deflate => {
            out.resize(size, 0);
            let n = inflater()
                .inflate(data, out)
                .map_err(|m| Error::Malformed(format!("ZIP: {m}")))?;
            if n != size {
                out.clear();
                return Err(malformed("inflated size differs from the declared size"));
            }
        }
        Method::Deflate64 => codec_read("Deflate64", size, out, |out, limit| {
            codec::inflate64::inflate(data, true, out, limit).map(|_| ())
        })?,
        Method::Bzip2 => codec_read("bzip2", size, out, |out, limit| {
            codec::bzip2::decode(data, out, limit, Threads::ONE)
        })?,
        Method::Lzma => codec_read("LZMA", size, out, |out, _| {
            codec::lzma::decode_zip(data, size, out, limits.max_decoder_memory)
        })?,
        Method::Zstd => codec_read("Zstandard", size, out, |out, limit| {
            codec::zstd::decode(data, out, limit)
        })?,
        Method::Xz => codec_read("xz", size, out, |out, limit| {
            codec::xz::decode(data, out, limit, limits.max_decoder_memory, Threads::ONE)
        })?,
        ref m => {
            return Err(Error::Unsupported(format!("ZIP compression {m:?}")));
        }
    }
    if verify_crc && crc32fast::hash(out) != loc.crc {
        out.clear();
        return Err(Error::Checksum);
    }
    Ok(())
}

/// Runs one of the own decoders (given `out` and the length it may grow to) for an entry
/// declaring `size` bytes, which it must then produce exactly, and maps its error.
fn codec_read(
    name: &str,
    size: usize,
    out: &mut Vec<u8>,
    decode: impl FnOnce(&mut Vec<u8>, usize) -> codec::CResult<()>,
) -> Result<()> {
    let base = out.len();
    let limit = base.saturating_add(size);
    if let Err(e) = decode(out, limit) {
        out.truncate(base);
        return Err(e.into_error(&format!("ZIP {name}"), size, true));
    }
    if out.len() != limit {
        out.truncate(base);
        return Err(malformed(
            "decompressed size differs from the declared size",
        ));
    }
    Ok(())
}

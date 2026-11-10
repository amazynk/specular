//! TAR / CBT: own header parser (v7, ustar, GNU long names, pax `path` and `size`).
//!
//! Why not the `tar` crate: it is built on `Read` streams, so every entry would be copied
//! through it, and listing would walk the stream. Here entries are slices of the memory-mapped
//! file: listing hops from header to header (one 512-byte read per entry) and reading an entry
//! is a borrow ([`crate::Archive::stored_slice`]) or one `memcpy`. The format is small enough
//! that owning the ~150 lines is cheaper than adapting a stream API.
//!
//! Gzip-compressed tars are inflated whole first (`gz`); other compressed tars (`.tar.xz`,
//! `.tar.bz2`) are not supported: they have no random access.

use crate::names::normalize;
use crate::{Entry, Error, Limits, Loc, Method, Result};

const BLOCK: usize = 512;

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("TAR: {what}"))
}

/// An octal (or GNU base-256) number field.
fn number(field: &[u8]) -> Option<u64> {
    if field.first().is_some_and(|&b| b & 0x80 != 0) {
        // Base-256, big-endian, top bit of the first byte is the marker. Negative values (0xFF)
        // are rejected.
        if field[0] & 0x40 != 0 {
            return None;
        }
        let mut v: u64 = (field[0] & 0x3F) as u64;
        for &b in &field[1..] {
            v = v.checked_mul(256)?.checked_add(b as u64)?;
        }
        return Some(v);
    }
    let mut v: u64 = 0;
    let mut any = false;
    for &b in field {
        match b {
            b'0'..=b'7' => {
                v = v.checked_mul(8)?.checked_add((b - b'0') as u64)?;
                any = true;
            }
            b' ' | 0 if !any => {}
            b' ' | 0 => break,
            _ => return None,
        }
    }
    Some(v)
}

/// The header's checksum is valid (unsigned or historic signed sum).
fn checksum_ok(h: &[u8]) -> bool {
    let Some(stored) = number(&h[148..156]) else {
        return false;
    };
    let (mut unsigned, mut signed) = (0u64, 0i64);
    for (i, &b) in h.iter().enumerate() {
        let b = if (148..156).contains(&i) { b' ' } else { b };
        unsigned += b as u64;
        signed += b as i8 as i64;
    }
    stored == unsigned || stored as i64 == signed
}

pub(crate) fn detect(b: &[u8]) -> bool {
    let Some(h) = b.get(..BLOCK) else {
        return false;
    };
    (&h[257..262] == b"ustar" || h[0] != 0) && checksum_ok(h)
}

fn cstr(field: &[u8]) -> &[u8] {
    let end = field.iter().position(|&c| c == 0).unwrap_or(field.len());
    &field[..end]
}

fn padded(len: u64) -> Option<u64> {
    len.checked_add(BLOCK as u64 - 1)
        .map(|v| v / BLOCK as u64 * BLOCK as u64)
}

/// pax extended header records: "<len> <key>=<value>\n".
fn pax(data: &[u8], path: &mut Option<String>, size: &mut Option<u64>) {
    let mut rest = data;
    while !rest.is_empty() {
        let Some(sp) = rest.iter().position(|&c| c == b' ') else {
            return;
        };
        let Some(len) = std::str::from_utf8(&rest[..sp])
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        else {
            return;
        };
        if len <= sp + 1 || len > rest.len() {
            return;
        }
        let rec = &rest[sp + 1..len];
        let rec = rec.strip_suffix(b"\n").unwrap_or(rec);
        if let Some(eq) = rec.iter().position(|&c| c == b'=') {
            let (k, v) = (&rec[..eq], &rec[eq + 1..]);
            match k {
                b"path" => *path = Some(String::from_utf8_lossy(v).into_owned()),
                b"size" => *size = std::str::from_utf8(v).ok().and_then(|s| s.parse().ok()),
                _ => {}
            }
        }
        rest = &rest[len..];
    }
}

pub(crate) fn list(b: &[u8], limits: &Limits) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut pos = 0usize;
    let mut long_name: Option<String> = None;
    let mut pax_path: Option<String> = None;
    let mut pax_size: Option<u64> = None;
    let mut headers = 0usize;
    while let Some(h) = b.get(pos..pos + BLOCK) {
        if h.iter().all(|&c| c == 0) {
            break; // end-of-archive marker
        }
        if !checksum_ok(h) {
            if headers == 0 {
                return Err(malformed("bad header checksum"));
            }
            break; // trailing garbage: keep what was listed
        }
        headers += 1;
        if headers > limits.max_entries.saturating_mul(2) {
            return Err(Error::Limit(format!(
                "more than {} entries",
                limits.max_entries
            )));
        }
        let kind = h[156];
        let mut size = number(&h[124..136]).ok_or_else(|| malformed("bad size field"))?;
        if matches!(kind, b'0' | 0 | b'7')
            && let Some(s) = pax_size.take()
        {
            size = s;
        }
        let data = pos + BLOCK;
        let Some(next) = padded(size)
            .and_then(|p| usize::try_from(p).ok())
            .and_then(|p| data.checked_add(p))
        else {
            return Err(malformed("size overflow"));
        };
        let body = b.get(data..data.saturating_add(size as usize).min(b.len()));
        match kind {
            b'L' => {
                let body = body.ok_or_else(|| malformed("truncated long name"))?;
                long_name = Some(String::from_utf8_lossy(cstr(body)).into_owned());
            }
            b'x' => {
                if let Some(body) = body {
                    pax(body, &mut pax_path, &mut pax_size);
                }
            }
            b'0' | 0 | b'7' => {
                if data as u64 + size > b.len() as u64 {
                    break; // truncated archive: the partial last entry is dropped
                }
                let raw = if let Some(n) = pax_path.take().or(long_name.take()) {
                    n
                } else {
                    let name = String::from_utf8_lossy(cstr(&h[0..100]));
                    let prefix = cstr(&h[345..500]);
                    if &h[257..262] == b"ustar" && !prefix.is_empty() {
                        format!("{}/{}", String::from_utf8_lossy(prefix), name)
                    } else {
                        name.into_owned()
                    }
                };
                // v7 tars mark folders with a trailing '/' on a regular entry.
                if !raw.ends_with('/') {
                    if entries.len() >= limits.max_entries {
                        return Err(Error::Limit(format!(
                            "more than {} entries",
                            limits.max_entries
                        )));
                    }
                    entries.push(Entry {
                        name: normalize(&raw),
                        size,
                        compressed_size: size,
                        method: Method::Stored,
                        encrypted: false,
                        loc: Loc::Plain {
                            offset: data as u64,
                        },
                    });
                }
            }
            _ => {
                // Folders, links, devices, global pax headers, sparse files: not pages.
                long_name = None;
                pax_path = None;
                pax_size = None;
            }
        }
        pos = next;
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number(b"0000644\0"), Some(0o644));
        assert_eq!(number(b"  123 \0"), Some(0o123));
        assert_eq!(number(b"\0\0\0"), Some(0));
        assert_eq!(number(b"12x"), None);
        assert_eq!(number(&[0x80, 0, 0, 1, 0]), Some(256));
        assert_eq!(number(&[0xFF; 12]), None);
    }

    #[test]
    fn pax_records() {
        let (mut p, mut s) = (None, None);
        pax(b"19 path=a/long.jpg\n11 size=42\n", &mut p, &mut s);
        assert_eq!(p.as_deref(), Some("a/long.jpg"));
        assert_eq!(s, Some(42));
        pax(b"999 path=x\n", &mut p, &mut s);
        pax(b"1 \n", &mut p, &mut s);
    }
}

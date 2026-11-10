//! ISO 9660 disc images (`.iso`): own directory walker, with Joliet (UCS-2 names) when the
//! image has a Joliet supplementary volume descriptor and Rock Ridge `NM` names otherwise.
//!
//! Files are contiguous extents of the image, so entries are zero-copy slices like a tar's.
//! The walk visits each directory extent once (a hostile image cannot loop), reads at most
//! [`MAX_DIR_BYTES`] of directory records in all, stops at [`MAX_DEPTH`] levels and at the
//! entry limit, and skips files whose extent lies outside the image. Multi-extent files
//! (larger than 4 GiB) are not listed. UDF-only images (no ISO 9660 descriptors) are not
//! recognised; UDF bridge images are read through their ISO 9660 side.

use crate::names::normalize;
use crate::{Entry, Error, Limits, Loc, Method, Result};
use std::collections::HashSet;

const SECTOR: usize = 2048;
/// The first volume descriptor's sector.
const FIRST_DESCRIPTOR: usize = 16;
/// Volume descriptors looked at, at most.
const MAX_DESCRIPTORS: usize = 64;
/// Directory record bytes read in all, at most.
const MAX_DIR_BYTES: u64 = 64 << 20;
/// Directory nesting followed, at most (ISO 9660 itself allows 8; real images go deeper).
const MAX_DEPTH: usize = 64;

const FLAG_DIR: u8 = 2;
const FLAG_MULTI_EXTENT: u8 = 0x80;

fn malformed(what: &str) -> Error {
    Error::Malformed(format!("ISO 9660: {what}"))
}

pub(crate) fn detect(b: &[u8]) -> bool {
    let at = FIRST_DESCRIPTOR * SECTOR;
    b.get(at + 1..at + 6) == Some(b"CD001")
}

fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The root directory's record and whether names are Joliet's.
struct Volume<'a> {
    root: &'a [u8],
    joliet: bool,
    block: usize,
}

fn volume(b: &[u8]) -> Result<Volume<'_>> {
    let mut primary = None;
    let mut joliet = None;
    for s in FIRST_DESCRIPTOR..FIRST_DESCRIPTOR + MAX_DESCRIPTORS {
        let Some(d) = b.get(s * SECTOR..(s + 1) * SECTOR) else {
            break;
        };
        if &d[1..6] != b"CD001" || d[0] == 255 {
            break;
        }
        let block = usize::from(u16::from_le_bytes([d[128], d[129]]));
        let block = if matches!(block, 512 | 1024 | 2048) {
            block
        } else {
            SECTOR
        };
        let root = &d[156..190];
        match d[0] {
            1 if primary.is_none() => primary = Some((root, block)),
            // Joliet: a supplementary descriptor with the UCS-2 level 1-3 escape sequence.
            2 if joliet.is_none() && d[88] == b'%' && d[89] == b'/' => {
                if matches!(d[90], b'@' | b'C' | b'E') {
                    joliet = Some((root, block));
                }
            }
            _ => {}
        }
    }
    match (joliet, primary) {
        (Some((root, block)), _) => Ok(Volume {
            root,
            joliet: true,
            block,
        }),
        (None, Some((root, block))) => Ok(Volume {
            root,
            joliet: false,
            block,
        }),
        _ => Err(malformed("no primary volume descriptor")),
    }
}

/// A Rock Ridge alternate name (`NM` entries) in a record's system use area.
fn rock_ridge_name(rec: &[u8], name_len: usize) -> Option<String> {
    let start = 33 + name_len + usize::from(name_len.is_multiple_of(2));
    let mut su = rec.get(start..)?;
    let mut name = Vec::new();
    let mut found = false;
    while su.len() >= 4 {
        let len = usize::from(su[2]);
        if len < 4 || len > su.len() {
            break;
        }
        let (e, rest) = su.split_at(len);
        match &e[..2] {
            b"NM" if len >= 5 => {
                // Flags: 1 continues in the next NM, 2 and 4 name "." and "..".
                if e[4] & 6 != 0 {
                    return None;
                }
                name.extend_from_slice(&e[5..]);
                found = true;
                if e[4] & 1 == 0 {
                    break;
                }
            }
            b"ST" => break,
            _ => {}
        }
        su = rest;
    }
    (found && !name.is_empty()).then(|| String::from_utf8_lossy(&name).into_owned())
}

/// A record's file name: Joliet UCS-2, Rock Ridge, or the plain ISO 9660 name without its
/// `;1` version and the `.` of an extension-less 8.3 name.
fn record_name(rec: &[u8], name_len: usize, joliet: bool) -> String {
    let raw = &rec[33..33 + name_len];
    let mut name = if joliet {
        let units = raw
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]));
        char::decode_utf16(units)
            .map(|c| c.unwrap_or('\u{FFFD}'))
            .collect()
    } else if let Some(n) = rock_ridge_name(rec, name_len) {
        return n;
    } else {
        raw.iter().map(|&c| char::from(c)).collect::<String>()
    };
    if let Some(i) = name.rfind(';')
        && name[i + 1..].bytes().all(|c| c.is_ascii_digit())
    {
        name.truncate(i);
    }
    if !joliet && name.ends_with('.') {
        name.pop();
    }
    name
}

pub(crate) fn list(b: &[u8], limits: &Limits) -> Result<Vec<Entry>> {
    let vol = volume(b)?;
    let extent = |rec: &[u8]| -> Option<(usize, usize)> {
        let lba = usize::try_from(u32le(rec, 2)?).ok()?;
        let size = usize::try_from(u32le(rec, 10)?).ok()?;
        Some((lba.checked_mul(vol.block)?, size))
    };
    let root = extent(vol.root).ok_or_else(|| malformed("root directory"))?;
    let mut entries = Vec::new();
    let mut queue = std::collections::VecDeque::from([(root, String::new(), 0usize)]);
    let mut seen = HashSet::from([root.0]);
    let mut dir_bytes = 0u64;
    while let Some(((start, size), prefix, depth)) = queue.pop_front() {
        dir_bytes += size as u64;
        if dir_bytes > MAX_DIR_BYTES {
            return Err(Error::Limit("ISO 9660 directories too large".into()));
        }
        let Some(dir) = start
            .checked_add(size)
            .and_then(|end| b.get(start..end.min(b.len())))
        else {
            continue; // a directory outside a truncated image
        };
        let mut pos = 0;
        let mut skip_next_part = false;
        while pos < dir.len() {
            let len = usize::from(dir[pos]);
            if len == 0 {
                // Records never cross a sector: the rest of this one is padding.
                pos = (pos / SECTOR + 1) * SECTOR;
                continue;
            }
            let Some(rec) = dir.get(pos..pos + len).filter(|_| len >= 34) else {
                break;
            };
            pos += len;
            let name_len = usize::from(rec[32]);
            if 33 + name_len > len {
                continue;
            }
            let flags = rec[25];
            if name_len == 1 && rec[33] <= 1 {
                continue; // "." and ".."
            }
            // The parts of a multi-extent file: the last part has the flag clear.
            if flags & FLAG_MULTI_EXTENT != 0 {
                skip_next_part = true;
                continue;
            }
            if std::mem::take(&mut skip_next_part) {
                continue;
            }
            let Some(ext) = extent(rec) else { continue };
            let name = record_name(rec, name_len, vol.joliet);
            let path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if flags & FLAG_DIR != 0 {
                if depth < MAX_DEPTH && seen.insert(ext.0) {
                    if seen.len() > limits.max_entries {
                        return Err(Error::Limit(format!(
                            "more than {} entries",
                            limits.max_entries
                        )));
                    }
                    queue.push_back((ext, path, depth + 1));
                }
                continue;
            }
            if ext.0.checked_add(ext.1).is_none_or(|end| end > b.len()) {
                continue; // outside a truncated image
            }
            if entries.len() >= limits.max_entries {
                return Err(Error::Limit(format!(
                    "more than {} entries",
                    limits.max_entries
                )));
            }
            entries.push(Entry {
                name: normalize(&path),
                size: ext.1 as u64,
                compressed_size: ext.1 as u64,
                method: Method::Stored,
                encrypted: false,
                loc: Loc::Plain {
                    offset: ext.0 as u64,
                },
            });
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let rec = |name: &[u8], su: &[u8]| {
            let mut r = vec![0u8; 33];
            r[32] = name.len() as u8;
            r.extend_from_slice(name);
            if name.len().is_multiple_of(2) {
                r.push(0);
            }
            r.extend_from_slice(su);
            r
        };
        let r = rec(b"PAGE01.JPG;1", &[]);
        assert_eq!(record_name(&r, 12, false), "PAGE01.JPG");
        let r = rec(b"README.;1", &[]);
        assert_eq!(record_name(&r, 9, false), "README");
        let j: Vec<u8> = "Été.png;1"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let r = rec(&j, &[]);
        assert_eq!(record_name(&r, j.len(), true), "Été.png");
        // Rock Ridge: PX skipped, NM in two parts.
        let mut su = vec![b'P', b'X', 4, 1];
        su.extend_from_slice(&[b'N', b'M', 9, 1, 1, b'l', b'o', b'n', b'g']);
        su.extend_from_slice(&[b'N', b'M', 9, 1, 0, b'.', b'p', b'n', b'g']);
        let r = rec(b"LONG.PNG;1", &su);
        assert_eq!(record_name(&r, 10, false), "long.png");
    }
}

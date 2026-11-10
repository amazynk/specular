//! Unix `compress` (`.Z`) streams: LZW with 9- to 16-bit codes, the way old beamline archives
//! store frames (`.img.Z`, `.edf.Z`, `.cbf.Z`).
//!
//! Header `1f 9d` and a flags byte (low five bits: the largest code width; `0x80`: block
//! mode, where code 256 clears the table). Codes are packed least significant bit first.
//! The writer emits codes in groups of eight of one width, so when the width grows (or the
//! table is cleared) the reader skips to the end of the current group, counted in bytes
//! from where that width started.

use crate::{Result, bad};

/// Whether `data` starts like a `.Z` stream.
pub fn sniff(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 0x1f && data[1] == 0x9d
}

/// Decompresses a `.Z` stream, stopping once `limit` bytes are out (cut to `limit`).
pub fn decompress(src: &[u8], limit: usize) -> Result<Vec<u8>> {
    decompress_reserved(src, limit).map(|(v, _)| v)
}

/// [`decompress`], with the output's reservation from the memory budget handed over: a
/// caller keeping the output keeps it reserved.
pub fn decompress_reserved(
    src: &[u8],
    limit: usize,
) -> Result<(Vec<u8>, specular_core::Reservation)> {
    if !sniff(src) {
        return Err(bad("not a compress (.Z) stream"));
    }
    let max_bits = u32::from(src[2] & 0x1f);
    let block = src[2] & 0x80 != 0;
    if !(9..=16).contains(&max_bits) {
        return Err(bad(format!(".Z code width {max_bits}")));
    }
    let data = &src[3..];
    let table = 1usize << max_bits;
    let mut prefix = vec![0u16; table];
    let mut suffix = vec![0u8; table];
    for (i, s) in suffix.iter_mut().enumerate().take(256) {
        *s = i as u8;
    }
    let first = if block { 257 } else { 256 };
    // The output is reserved from the memory budget before it grows.
    let start = src.len().saturating_mul(3).min(limit);
    let mut res = specular_core::budget::reserve(start as u64)?;
    let mut out: Vec<u8> = Vec::with_capacity(start);
    let mut stack: Vec<u8> = Vec::new();

    let mut bits = 9u32;
    let mut free = first;
    // Byte position, bit buffer, and where the current code width started.
    let (mut pos, mut buf, mut have, mut mark) = (0usize, 0u32, 0u32, 0usize);
    let mut prev: Option<u16> = None;
    let mut last = 0u8;

    // Skip to the end of the current group of eight codes and drop buffered bits.
    let align = |pos: &mut usize, buf: &mut u32, have: &mut u32, mark: &mut usize, bits: u32| {
        let rem = (*pos - *mark) % bits as usize;
        if rem != 0 {
            *pos = pos.saturating_add(bits as usize - rem);
        }
        *buf = 0;
        *have = 0;
        *mark = *pos;
    };

    loop {
        if free > (1usize << bits) - 1 && bits < max_bits {
            align(&mut pos, &mut buf, &mut have, &mut mark, bits);
            bits += 1;
        }
        while have < bits {
            let Some(&b) = data.get(pos) else {
                return Ok((out, res));
            };
            buf |= u32::from(b) << have;
            pos += 1;
            have += 8;
        }
        let code = (buf & ((1 << bits) - 1)) as usize;
        buf >>= bits;
        have -= bits;

        if block && code == 256 {
            align(&mut pos, &mut buf, &mut have, &mut mark, bits);
            bits = 9;
            free = first;
            prev = None;
            continue;
        }
        let Some(p) = prev else {
            if code > 255 {
                return Err(bad(format!(".Z first code {code}")));
            }
            last = code as u8;
            out.push(last);
            prev = Some(code as u16);
            continue;
        };
        let mut c = code;
        stack.clear();
        if c >= free {
            // The code being defined now (KwKwK): the previous string plus its first byte.
            if c > free {
                return Err(bad(format!(".Z code {c} past the table ({free})")));
            }
            stack.push(last);
            c = usize::from(p);
        }
        while c >= 256 {
            stack.push(suffix[c]);
            c = usize::from(prefix[c]);
        }
        last = suffix[c];
        stack.push(last);
        if out.len() + stack.len() > out.capacity() {
            let need = out.len() + stack.len();
            let want = need
                .max(out.capacity() * 2)
                .min(limit.saturating_add(stack.len()));
            res.grow_to(want as u64)?;
            out.reserve_exact(want - out.len());
        }
        out.extend(stack.iter().rev());
        if out.len() >= limit {
            out.truncate(limit);
            return Ok((out, res));
        }
        if free < table {
            prefix[free] = p;
            suffix[free] = last;
            free += 1;
        }
        prev = Some(code as u16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny writer from the same description, for round trips of every width.
    fn compress(input: &[u8], max_bits: u32) -> Vec<u8> {
        use std::collections::HashMap;
        let mut out = vec![0x1f, 0x9d, 0x80 | max_bits as u8];
        let mut codes: Vec<(u32, u32)> = Vec::new();
        let mut dict: HashMap<(u16, u8), u16> = HashMap::new();
        let mut free = 257u32;
        let mut bits = 9u32;
        let mut cur: Option<u16> = None;
        // Codes with the width they are written in; a width change ends a group of eight.
        let mut groups: Vec<Vec<(u32, u32)>> = vec![Vec::new()];
        let emit = |c: u32, bits: u32, groups: &mut Vec<Vec<(u32, u32)>>| {
            groups.last_mut().unwrap().push((c, bits));
        };
        for &b in input {
            match cur {
                None => cur = Some(u16::from(b)),
                Some(p) => {
                    if let Some(&c) = dict.get(&(p, b)) {
                        cur = Some(c);
                    } else {
                        emit(u32::from(p), bits, &mut groups);
                        if free < 1 << max_bits {
                            dict.insert((p, b), free as u16);
                            free += 1;
                            if free > (1 << bits) && bits < max_bits {
                                bits += 1;
                                groups.push(Vec::new());
                            }
                        }
                        cur = Some(u16::from(b));
                    }
                }
            }
        }
        if let Some(p) = cur {
            emit(u32::from(p), bits, &mut groups);
        }
        codes.clear();
        let n = groups.len();
        for (gi, g) in groups.into_iter().enumerate() {
            let (mut acc, mut nb) = (0u64, 0u32);
            let mut bytes = Vec::new();
            for &(c, w) in &g {
                acc |= u64::from(c) << nb;
                nb += w;
                while nb >= 8 {
                    bytes.push(acc as u8);
                    acc >>= 8;
                    nb -= 8;
                }
            }
            if nb > 0 {
                bytes.push(acc as u8);
            }
            // Pad all but the last group to a multiple of its width in bytes.
            if gi + 1 < n
                && let Some(&(_, w)) = g.first()
            {
                while bytes.len() % w as usize != 0 {
                    bytes.push(0);
                }
            }
            out.extend(bytes);
        }
        out
    }

    #[test]
    fn round_trips() {
        let mut text = Vec::new();
        for i in 0..20_000u32 {
            text.extend(format!("{} ", (i * 7919) % 613).bytes());
        }
        for bits in [9, 12, 16] {
            let z = compress(&text, bits);
            assert_eq!(decompress(&z, usize::MAX).unwrap(), text, "{bits} bits");
        }
        // KwKwK.
        let z = compress(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaab", 9);
        assert_eq!(
            decompress(&z, usize::MAX).unwrap(),
            b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaab"
        );
        assert_eq!(decompress(&z, 5).unwrap(), b"aaaaa");
    }

    #[test]
    fn output_reserves_from_the_budget() {
        // 32 MiB of zeros packs into a few hundred KiB; a 8 MiB budget stops it growing.
        let z = compress(&vec![0u8; 32 << 20], 16);
        assert!(z.len() < 1 << 20, "{}", z.len());
        let r = specular_core::budget::with_limit(8 << 20, || decompress(&z, usize::MAX));
        assert!(
            matches!(r, Err(crate::Error::NeedsMemory(_))),
            "{:?}",
            r.map(|v| v.len())
        );
        let b = std::sync::Arc::new(specular_core::budget::MemoryBudget::fixed(128 << 20));
        let out = specular_core::budget::with_budget(b.clone(), || decompress(&z, usize::MAX));
        assert_eq!(out.unwrap().len(), 32 << 20);
        assert_eq!(b.reserved(), 0);
        // A whole-file `.Z` handed to the caller stays reserved while it is kept.
        let kept = specular_core::budget::with_budget(b.clone(), || crate::uncompress(&z))
            .unwrap()
            .unwrap();
        assert_eq!(kept.0.len(), 32 << 20);
        assert!(b.reserved() >= 32 << 20, "{}", b.reserved());
        drop(kept);
        assert_eq!(b.reserved(), 0);
    }

    #[test]
    fn hostile() {
        assert!(decompress(b"\x1f\x9d\x90", 100).unwrap().is_empty());
        assert!(decompress(b"\x1f\x9d\x91\xff\xff\xff", 100).is_err());
        assert!(decompress(b"\x1f\x9d\x08\x00", 100).is_err());
        // A code past the table.
        assert!(decompress(&[0x1f, 0x9d, 0x90, 0x41, 0xfe, 0x03], 100).is_err());
        assert!(decompress(b"plain", 100).is_err());
    }
}

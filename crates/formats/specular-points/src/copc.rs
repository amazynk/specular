//! COPC (Cloud Optimized Point Cloud): a LAZ 1.4 file whose chunks are the nodes of an
//! octree, listed by a hierarchy of pages (EVLR "copc"/1000) from a root page that the info
//! VLR ("copc"/1) points at. Nodes are read coarse to fine, so a capped read keeps whole
//! levels and the view refines while loading.

use crate::las::{Unit, Vlr};
use crate::{Result, malformed};

/// Hierarchy pages read at most (a guard against loops in a corrupt file).
const MAX_PAGES: usize = 1 << 16;

/// The data nodes as `(level, unit)`, by level; `None` when the file is not COPC.
pub(crate) fn nodes(bytes: &[u8], vlrs: &[Vlr]) -> Result<Option<Vec<(u32, Unit)>>> {
    let Some(info) = vlrs.iter().find(|v| v.user == "copc" && v.id == 1) else {
        return Ok(None);
    };
    let d = info.data;
    if d.len() < 56 {
        return malformed("COPC info VLR truncated");
    }
    let u64_at = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"));
    let root = (u64_at(d, 40), u64_at(d, 48));
    let mut pages = vec![root];
    let mut out = Vec::new();
    let mut seen = 0usize;
    let mut read = 0usize;
    while let Some((off, size)) = pages.pop() {
        seen += 1;
        if seen > MAX_PAGES {
            return malformed("COPC hierarchy has too many pages");
        }
        let page = usize::try_from(off)
            .ok()
            .zip(usize::try_from(size).ok())
            .and_then(|(o, s)| bytes.get(o..o.checked_add(s)?));
        let Some(page) = page else {
            return malformed("COPC hierarchy page past the end of the file");
        };
        // Pages don't overlap, so together they fit in the file; a page listed over and
        // over (or pages overlapping) would multiply its entries.
        read += page.len();
        if read > bytes.len() {
            return malformed("COPC hierarchy pages overlap");
        }
        for e in page.chunks_exact(32) {
            let i32_at = |o: usize| i32::from_le_bytes(e[o..o + 4].try_into().expect("4 bytes"));
            let level = i32_at(0);
            let offset = u64_at(e, 16);
            let len = i32_at(24);
            let count = i32_at(28);
            if !(0..32).contains(&level) {
                return malformed(format!("COPC node at level {level}"));
            }
            match count {
                -1 => pages.push((offset, u64::try_from(len).unwrap_or(0))),
                0 => {}
                c if c > 0 && len > 0 => {
                    let (Ok(offset), Ok(len)) = (usize::try_from(offset), usize::try_from(len))
                    else {
                        return malformed("COPC node offset");
                    };
                    if offset.checked_add(len).is_none_or(|e| e > bytes.len()) {
                        return malformed("COPC node past the end of the file");
                    }
                    out.push((
                        level as u32,
                        Unit {
                            offset,
                            len,
                            count: c as u64,
                            first: 0,
                        },
                    ));
                }
                _ => return malformed(format!("COPC node with {count} points")),
            }
        }
    }
    // By level, then by file position (which writers keep spatially coherent).
    out.sort_by_key(|(l, u)| (*l, u.offset));
    Ok(Some(out))
}

#[cfg(test)]
mod hostile {
    use super::*;

    /// A hierarchy page of 64 entries: 63 data nodes and one link back to itself.
    #[test]
    fn page_listing_itself() {
        let mut file = vec![0u8; 4096];
        let page_at = 1024usize;
        let page_len = 64 * 32i32;
        for k in 0..64 {
            let e = &mut file[page_at + 32 * k..page_at + 32 * k + 32];
            let (offset, len, count) = if k == 0 {
                (page_at as u64, page_len, -1i32)
            } else {
                (0, 16, 1)
            };
            e[16..24].copy_from_slice(&offset.to_le_bytes());
            e[24..28].copy_from_slice(&len.to_le_bytes());
            e[28..32].copy_from_slice(&count.to_le_bytes());
        }
        let mut info = [0u8; 56];
        info[40..48].copy_from_slice(&(page_at as u64).to_le_bytes());
        info[48..56].copy_from_slice(&(page_len as u64).to_le_bytes());
        let vlrs = [Vlr {
            user: "copc".into(),
            id: 1,
            data: &info,
        }];
        // Refused on the second visit, not after 65 536 copies of the page's nodes.
        assert!(matches!(
            nodes(&file, &vlrs),
            Err(crate::Error::Malformed(m)) if m.contains("overlap")
        ));
    }
}

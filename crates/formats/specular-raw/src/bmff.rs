//! Minimal ISO-BMFF box iteration (for Canon CR3).

use std::ops::Range;

use crate::tiff::{be_u32, be_u64};

#[derive(Clone, Debug)]
pub(crate) struct BoxRef {
    pub typ: [u8; 4],
    /// For `uuid` boxes, the 16-byte user type.
    pub uuid: Option<[u8; 16]>,
    /// Payload (after the header and uuid).
    pub payload: Range<usize>,
}

/// Iterate the boxes in `d[range]`. Stops at the first malformed header.
pub(crate) fn boxes(d: &[u8], range: Range<usize>) -> Vec<BoxRef> {
    let mut out = Vec::new();
    let end = range.end.min(d.len());
    let mut p = range.start;
    while p + 8 <= end && out.len() < 4096 {
        let Some(size32) = be_u32(d, p) else { break };
        let typ: [u8; 4] = d[p + 4..p + 8].try_into().unwrap_or_default();
        let (size, mut hdr) = match size32 {
            0 => (end - p, 8),
            1 => match be_u64(d, p + 8) {
                Some(s) => (usize::try_from(s).unwrap_or(usize::MAX), 16),
                None => break,
            },
            s => (s as usize, 8),
        };
        if size < hdr || p.checked_add(size).is_none_or(|e| e > end) {
            break;
        }
        let mut uuid = None;
        if &typ == b"uuid" {
            if size < hdr + 16 {
                break;
            }
            uuid = d[p + hdr..p + hdr + 16].try_into().ok();
            hdr += 16;
        }
        out.push(BoxRef {
            typ,
            uuid,
            payload: p + hdr..p + size,
        });
        p += size;
    }
    out
}

/// First child box of type `typ`.
pub(crate) fn find(d: &[u8], range: Range<usize>, typ: &[u8; 4]) -> Option<BoxRef> {
    boxes(d, range).into_iter().find(|b| &b.typ == typ)
}

/// Follow a path of box types from `range`.
pub(crate) fn path(d: &[u8], mut range: Range<usize>, types: &[&[u8; 4]]) -> Option<BoxRef> {
    let mut cur = None;
    for t in types {
        let b = find(d, range, t)?;
        range = b.payload.clone();
        cur = Some(b);
    }
    cur
}

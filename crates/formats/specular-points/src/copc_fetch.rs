//! A COPC file read by byte ranges (from an HTTP server, through the caller's
//! [`RangeRead`]): the header, VLRs and hierarchy, then whole octree levels, coarse to fine,
//! as long as they fit the point budget. The levels fetched are put together as a smaller
//! COPC file of their own (the same header and VLRs, the chosen nodes' LAZ chunks, a new
//! hierarchy page), which the local reader then opens like any other.

use crate::{Error, Result, malformed};

/// Bytes by range.
pub trait RangeRead: Sync {
    fn read_range(&self, offset: u64, len: u64) -> std::result::Result<Vec<u8>, String>;
}

impl RangeRead for [u8] {
    fn read_range(&self, offset: u64, len: u64) -> std::result::Result<Vec<u8>, String> {
        let (o, l) = (offset as usize, len as usize);
        o.checked_add(l)
            .and_then(|e| self.get(o..e))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "range past the end of the file".into())
    }
}

/// Hierarchy pages read at most.
const MAX_PAGES: usize = 1 << 14;
/// Bytes of hierarchy read at most.
const MAX_HIERARCHY: u64 = 64 << 20;
/// Neighbouring chunks closer than this are fetched in one request.
const MERGE_GAP: u64 = 256 << 10;
/// One request asks for at most this much.
const MAX_REQUEST: u64 = 16 << 20;
/// Other EVLRs (WKT) copied when smaller than this.
const MAX_EVLR: u64 = 1 << 20;

/// What was fetched.
#[derive(Debug)]
pub struct Fetched {
    /// A COPC file holding the levels fetched.
    pub image: Vec<u8>,
    /// Levels fetched (0..levels) of those in the file.
    pub levels: u32,
    pub file_levels: u32,
    /// Points fetched, and in the whole file.
    pub points: u64,
    pub total: u64,
    /// Bytes read and requests made.
    pub bytes_read: u64,
    pub requests: u32,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    key: [i32; 4],
    offset: u64,
    len: u64,
    count: u64,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().expect("2 bytes"))
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().expect("4 bytes"))
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().expect("8 bytes"))
}

/// Fetches the levels of a COPC file that fit `max_points` (at least the root) and at most
/// `max_bytes` of chunks. `progress(level, points)` is told as each level comes in.
pub fn fetch_levels(
    src: &dyn RangeRead,
    max_points: u64,
    max_bytes: u64,
    progress: &mut dyn FnMut(u32, u64),
) -> Result<Fetched> {
    let mut requests = 0u32;
    let mut bytes_read = 0u64;
    let mut read = |o: u64, l: u64| -> Result<Vec<u8>> {
        requests += 1;
        bytes_read += l;
        src.read_range(o, l).map_err(Error::Malformed)
    };
    let head = read(0, 375)?;
    if head.len() < 375 || !head.starts_with(b"LASF") {
        return malformed("not a LAS file");
    }
    if (head[24], head[25]) != (1, 4) {
        return Err(Error::Unsupported(format!(
            "LAS {}.{}: COPC is LAS 1.4",
            head[24], head[25]
        )));
    }
    let header_size = u16_at(&head, 94) as u64;
    let data_offset = u64::from(u32_at(&head, 96));
    let n_vlrs = u32_at(&head, 100);
    let evlr_start = u64_at(&head, 235);
    let n_evlrs = u32_at(&head, 243);
    let total = u64_at(&head, 247);
    if !(375..=data_offset).contains(&header_size) || data_offset > 64 << 20 {
        return malformed("LAS header sizes");
    }
    let mut region = read(0, data_offset)?;
    // The VLRs: the COPC info VLR's place in `region`.
    let mut at = header_size as usize;
    let mut info_at = None;
    for _ in 0..n_vlrs {
        if at + 54 > region.len() {
            return malformed("VLRs past the point data");
        }
        let user = &region[at + 2..at + 18];
        let id = u16_at(&region, at + 18);
        let len = u16_at(&region, at + 20) as usize;
        if user.starts_with(b"copc") && user[4..].iter().all(|&b| b == 0) && id == 1 {
            info_at = Some(at + 54);
        }
        at += 54 + len;
    }
    let Some(info_at) = info_at.filter(|&i| i + 56 <= region.len()) else {
        return Err(Error::Unsupported(
            "not a COPC file (no COPC info record)".into(),
        ));
    };
    let root = (u64_at(&region, info_at + 40), u64_at(&region, info_at + 48));
    // The hierarchy.
    let mut nodes = Vec::new();
    let mut pages = vec![root];
    let mut seen = 0usize;
    let mut hier = 0u64;
    while let Some((off, size)) = pages.pop() {
        seen += 1;
        hier += size;
        if seen > MAX_PAGES || hier > MAX_HIERARCHY {
            return malformed("COPC hierarchy too large");
        }
        let page = read(off, size)?;
        for e in page.chunks_exact(32) {
            let key = [0, 4, 8, 12].map(|o| i32::from_le_bytes(e[o..o + 4].try_into().expect("4")));
            let offset = u64_at(e, 16);
            let len = i32::from_le_bytes(e[24..28].try_into().expect("4"));
            let count = i32::from_le_bytes(e[28..32].try_into().expect("4"));
            if !(0..32).contains(&key[0]) {
                return malformed(format!("COPC node at level {}", key[0]));
            }
            match count {
                -1 => pages.push((offset, u64::try_from(len).unwrap_or(0))),
                c if c > 0 && len > 0 => nodes.push(Node {
                    key,
                    offset,
                    len: len as u64,
                    count: c as u64,
                }),
                _ => {}
            }
        }
    }
    if nodes.is_empty() {
        return Err(Error::Empty);
    }
    nodes.sort_by_key(|n| (n.key[0], n.offset));
    let file_levels = nodes.last().map_or(0, |n| n.key[0] as u32 + 1);
    // Whole levels within the budgets (the root level always).
    let mut chosen = 0usize;
    let mut levels = 0u32;
    let (mut points, mut bytes) = (0u64, 0u64);
    while chosen < nodes.len() {
        let level = nodes[chosen].key[0];
        let end = nodes[chosen..]
            .iter()
            .position(|n| n.key[0] != level)
            .map_or(nodes.len(), |p| chosen + p);
        let (p, b): (u64, u64) = nodes[chosen..end]
            .iter()
            .fold((0, 0), |(p, b), n| (p + n.count, b + n.len));
        if levels > 0 && (points + p > max_points || bytes + b > max_bytes) {
            break;
        }
        points += p;
        bytes += b;
        levels += 1;
        chosen = end;
    }
    let mut picked = nodes[..chosen].to_vec();
    // Their chunks, neighbours merged into one request, level by level.
    let mut image = std::mem::take(&mut region);
    let mut new_offsets = vec![0u64; picked.len()];
    let mut level_start = 0usize;
    let mut fetched_points = 0u64;
    while level_start < picked.len() {
        let level = picked[level_start].key[0];
        let level_end = picked[level_start..]
            .iter()
            .position(|n| n.key[0] != level)
            .map_or(picked.len(), |p| level_start + p);
        let mut order: Vec<usize> = (level_start..level_end).collect();
        order.sort_by_key(|&i| picked[i].offset);
        let mut i = 0;
        while i < order.len() {
            let first = picked[order[i]];
            let mut end = first.offset + first.len;
            let mut j = i + 1;
            while j < order.len() {
                let n = picked[order[j]];
                if n.offset < end
                    || n.offset - end > MERGE_GAP
                    || n.offset + n.len - first.offset > MAX_REQUEST
                {
                    break;
                }
                end = n.offset + n.len;
                j += 1;
            }
            let span = read(first.offset, end - first.offset)?;
            for &k in &order[i..j] {
                let n = picked[k];
                let s = (n.offset - first.offset) as usize;
                let chunk = span
                    .get(s..s + n.len as usize)
                    .ok_or_else(|| Error::Malformed("COPC chunk past the end".into()))?;
                new_offsets[k] = image.len() as u64;
                image.extend_from_slice(chunk);
                fetched_points += n.count;
            }
            i = j;
        }
        progress(level as u32, fetched_points);
        level_start = level_end;
    }
    for (n, o) in picked.iter_mut().zip(&new_offsets) {
        n.offset = *o;
    }
    // Other EVLRs (the WKT, when it is one), small ones only.
    let first_evlr = image.len() as u64;
    let mut evlrs = 0u32;
    let mut pos = evlr_start;
    for _ in 0..n_evlrs.min(64) {
        if pos == 0 {
            break;
        }
        let h = read(pos, 60)?;
        if h.len() < 60 {
            break;
        }
        let len = u64_at(&h, 20);
        let is_hier = h[2..6] == *b"copc" && u16_at(&h, 18) == 1000;
        if !is_hier && len <= MAX_EVLR {
            let body = read(pos + 60, len)?;
            image.extend_from_slice(&h);
            image.extend_from_slice(&body);
            evlrs += 1;
        }
        pos = pos.saturating_add(60 + len);
    }
    // The new hierarchy: one page of the nodes kept.
    let mut page = Vec::with_capacity(picked.len() * 32);
    for n in &picked {
        for k in n.key {
            page.extend_from_slice(&k.to_le_bytes());
        }
        page.extend_from_slice(&n.offset.to_le_bytes());
        page.extend_from_slice(&(n.len as i32).to_le_bytes());
        page.extend_from_slice(&(n.count as i32).to_le_bytes());
    }
    let mut h = vec![0u8; 60];
    h[2..6].copy_from_slice(b"copc");
    h[18..20].copy_from_slice(&1000u16.to_le_bytes());
    h[20..28].copy_from_slice(&(page.len() as u64).to_le_bytes());
    h[28..39].copy_from_slice(b"EPT hierarc");
    image.extend_from_slice(&h);
    let page_at = image.len() as u64;
    image.extend_from_slice(&page);
    evlrs += 1;
    // Patch the header and the info VLR.
    image[235..243].copy_from_slice(&first_evlr.to_le_bytes());
    image[243..247].copy_from_slice(&evlrs.to_le_bytes());
    image[247..255].copy_from_slice(&points.to_le_bytes());
    image[107..111].copy_from_slice(&0u32.to_le_bytes());
    image[info_at + 40..info_at + 48].copy_from_slice(&page_at.to_le_bytes());
    image[info_at + 48..info_at + 56].copy_from_slice(&(page.len() as u64).to_le_bytes());
    Ok(Fetched {
        image,
        levels,
        file_levels,
        points,
        total,
        bytes_read,
        requests,
    })
}

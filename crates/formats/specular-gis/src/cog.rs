//! Cloud-Optimized GeoTIFFs read by byte ranges (from an HTTP server, through the caller's
//! [`RangeRead`]): the header and IFDs first, then the tiles of the finest level (full
//! resolution or an overview) that fits the pixel and byte budgets. The level fetched is put
//! together as a standalone GeoTIFF (its own tags, the georeference of the full-resolution
//! image scaled to the level, the tiles), which the local reader then opens like any other.
//! Classic TIFF and BigTIFF, either byte order.

use crate::pmtiles::RangeRead;

/// Bytes read in one go at the start (header, IFDs and small tag values of most COGs).
const HEAD: u64 = 64 << 10;
/// Neighbouring tiles closer than this are fetched in one request.
const MERGE_GAP: u64 = 256 << 10;
/// One request asks for at most this much.
const MAX_REQUEST: u64 = 16 << 20;
/// IFDs read at most.
const MAX_IFDS: usize = 64;
/// Tags of one IFD at most.
const MAX_TAGS: u64 = 1024;
/// Bytes of one tag's values at most (tile offset arrays of very large images).
const MAX_VALUES: u64 = 256 << 20;

/// What was fetched.
#[derive(Debug)]
pub struct Fetched {
    /// A GeoTIFF of the level fetched.
    pub image: Vec<u8>,
    /// The level (0 = full resolution) and how many the file has.
    pub level: usize,
    pub levels: usize,
    pub width: u32,
    pub height: u32,
    pub full: (u32, u32),
    pub bytes_read: u64,
    pub requests: u32,
}

/// The bytes fetched so far, by range, with what reading them cost.
struct Sparse<'a> {
    src: &'a dyn RangeRead,
    chunks: Vec<(u64, Vec<u8>)>,
    bytes: u64,
    requests: u32,
}

impl Sparse<'_> {
    fn get(&mut self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        if len > MAX_VALUES {
            return Err("a TIFF tag is too large".into());
        }
        for (o, d) in &self.chunks {
            if offset >= *o && offset + len <= *o + d.len() as u64 {
                let s = (offset - o) as usize;
                return Ok(d[s..s + len as usize].to_vec());
            }
        }
        let want = len.max(16 << 10);
        let d = self
            .src
            .read_range(offset, want)
            .or_else(|_| self.src.read_range(offset, len))?;
        self.requests += 1;
        self.bytes += d.len() as u64;
        if (d.len() as u64) < len {
            return Err("the server sent a short range".into());
        }
        let out = d[..len as usize].to_vec();
        self.chunks.push((offset, d));
        Ok(out)
    }
}

#[derive(Clone, Debug)]
struct Entry {
    tag: u16,
    ty: u16,
    count: u64,
    /// The value bytes (inline or fetched), in the file's byte order.
    data: Vec<u8>,
}

struct Ifd {
    entries: Vec<Entry>,
}

fn type_size(ty: u16) -> u64 {
    match ty {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 | 16 | 17 | 18 => 8,
        _ => 0,
    }
}

struct Order {
    le: bool,
    big: bool,
}

impl Order {
    fn u16(&self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.le {
            u16::from_le_bytes(a)
        } else {
            u16::from_be_bytes(a)
        }
    }
    fn u32(&self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.le {
            u32::from_le_bytes(a)
        } else {
            u32::from_be_bytes(a)
        }
    }
    fn u64(&self, b: &[u8]) -> u64 {
        let a: [u8; 8] = b[..8].try_into().unwrap_or_default();
        if self.le {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        }
    }
    fn f64(&self, b: &[u8]) -> f64 {
        f64::from_bits(self.u64(b))
    }
    /// The integer values of an entry (BYTE, SHORT, LONG, LONG8, IFD, IFD8).
    fn ints(&self, e: &Entry) -> Vec<u64> {
        let n = e.count as usize;
        match e.ty {
            1 | 7 => e.data.iter().take(n).map(|&v| u64::from(v)).collect(),
            3 => e
                .data
                .chunks_exact(2)
                .take(n)
                .map(|c| u64::from(self.u16(c)))
                .collect(),
            4 | 13 => e
                .data
                .chunks_exact(4)
                .take(n)
                .map(|c| u64::from(self.u32(c)))
                .collect(),
            16 | 18 => e
                .data
                .chunks_exact(8)
                .take(n)
                .map(|c| self.u64(c))
                .collect(),
            _ => Vec::new(),
        }
    }
    fn doubles(&self, e: &Entry) -> Vec<f64> {
        if e.ty != 12 {
            return Vec::new();
        }
        e.data.chunks_exact(8).map(|c| self.f64(c)).collect()
    }
    fn put_u16(&self, v: u16) -> [u8; 2] {
        if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }
    fn put_u32(&self, v: u32) -> [u8; 4] {
        if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }
    fn put_f64(&self, v: f64) -> [u8; 8] {
        if self.le {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    }
}

impl Ifd {
    fn get(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.tag == tag)
    }
    fn int(&self, o: &Order, tag: u16) -> Option<u64> {
        self.get(tag).and_then(|e| o.ints(e).first().copied())
    }
}

fn read_ifd(s: &mut Sparse<'_>, o: &Order, at: u64) -> Result<(Ifd, u64), String> {
    let (cnt_len, entry_len, next_len) = if o.big {
        (8u64, 20u64, 8u64)
    } else {
        (2, 12, 4)
    };
    let c = s.get(at, cnt_len)?;
    let n = if o.big {
        o.u64(&c)
    } else {
        u64::from(o.u16(&c))
    };
    if n == 0 || n > MAX_TAGS {
        return Err("a TIFF directory is damaged".into());
    }
    let body = s.get(at + cnt_len, n * entry_len + next_len)?;
    let mut entries = Vec::with_capacity(n as usize);
    for k in 0..n as usize {
        let e = &body[k * entry_len as usize..(k + 1) * entry_len as usize];
        let tag = o.u16(&e[0..2]);
        let ty = o.u16(&e[2..4]);
        let (count, val) = if o.big {
            (o.u64(&e[4..12]), &e[12..20])
        } else {
            (u64::from(o.u32(&e[4..8])), &e[8..12])
        };
        let size = type_size(ty)
            .checked_mul(count)
            .ok_or("a TIFF tag is too large")?;
        let inline = if o.big { 8 } else { 4 };
        let data = if size <= inline {
            val[..size as usize].to_vec()
        } else if matches!(tag, 324 | 325 | 273 | 279) {
            // Tile and strip arrays are fetched for the chosen level only.
            let off = if o.big {
                o.u64(val)
            } else {
                u64::from(o.u32(val))
            };
            let mut v = off.to_le_bytes().to_vec();
            v.extend_from_slice(&size.to_le_bytes());
            v
        } else {
            let off = if o.big {
                o.u64(val)
            } else {
                u64::from(o.u32(val))
            };
            s.get(off, size)?
        };
        entries.push(Entry {
            tag,
            ty,
            count,
            data,
        });
    }
    let nb = &body[(n * entry_len) as usize..];
    let next = if o.big {
        o.u64(nb)
    } else {
        u64::from(o.u32(nb))
    };
    Ok((Ifd { entries }, next))
}

/// An offsets or byte counts array, fetched now.
fn array(s: &mut Sparse<'_>, o: &Order, big: bool, e: &Entry) -> Result<Vec<u64>, String> {
    let inline = if big { 8 } else { 4 };
    let size = type_size(e.ty) * e.count;
    if size <= inline {
        return Ok(o.ints(e));
    }
    let off = u64::from_le_bytes(e.data[..8].try_into().unwrap_or_default());
    let data = s.get(off, size)?;
    Ok(o.ints(&Entry { data, ..e.clone() }))
}

/// The georeference tags carried over from the full-resolution image.
const GEO_TAGS: &[u16] = &[33550, 33922, 34264, 34735, 34736, 34737, 42112, 42113];

/// Fetches the finest level of the COG behind `src` with at most `max_pixels` pixels and
/// `max_bytes` of tiles. `progress(bytes)` is called as tiles arrive.
pub fn fetch_level(
    src: &dyn RangeRead,
    max_pixels: u64,
    max_bytes: u64,
    progress: &mut dyn FnMut(u64),
) -> Result<Fetched, String> {
    let mut s = Sparse {
        src,
        chunks: Vec::new(),
        bytes: 0,
        requests: 0,
    };
    let head = s.get(0, 16)?;
    // Prime the cache with the head of the file (IFDs of a COG come first).
    let _ = s.get(0, HEAD.min(1 << 16)).or_else(|_| s.get(0, 16));
    let o = match &head[..2] {
        b"II" => Order {
            le: true,
            big: false,
        },
        b"MM" => Order {
            le: false,
            big: false,
        },
        _ => return Err("not a TIFF file".into()),
    };
    let magic = o.u16(&head[2..4]);
    let o = Order {
        big: magic == 43,
        ..o
    };
    if magic != 42 && magic != 43 {
        return Err("not a TIFF file".into());
    }
    let mut at = if o.big {
        o.u64(&head[8..16])
    } else {
        u64::from(o.u32(&head[4..8]))
    };
    let mut ifds = Vec::new();
    while at != 0 && ifds.len() < MAX_IFDS {
        let (ifd, next) = read_ifd(&mut s, &o, at)?;
        ifds.push(ifd);
        if next == at {
            break;
        }
        at = next;
    }
    // Images (not masks): full resolution first.
    let images: Vec<&Ifd> = ifds
        .iter()
        .filter(|i| i.int(&o, 254).unwrap_or(0) & 4 == 0)
        .collect();
    let full = images.first().ok_or("the TIFF has no image")?;
    let size = |i: &Ifd| (i.int(&o, 256).unwrap_or(0), i.int(&o, 257).unwrap_or(0));
    let (fw, fh) = size(full);
    if fw == 0 || fh == 0 {
        return Err("the TIFF image has no size".into());
    }
    if full.get(322).is_none() {
        return Err("the TIFF is not tiled (not cloud-optimized)".into());
    }
    let level = images
        .iter()
        .position(|i| {
            let (w, h) = size(i);
            w * h <= max_pixels
        })
        .unwrap_or(images.len() - 1);
    let chosen = images[level];
    let (w, h) = size(chosen);
    let offsets_e = chosen.get(324).ok_or("the level has no tile offsets")?;
    let counts_e = chosen.get(325).ok_or("the level has no tile sizes")?;
    let offsets = array(&mut s, &o, o.big, offsets_e)?;
    let counts = array(&mut s, &o, o.big, counts_e)?;
    if offsets.len() != counts.len() {
        return Err("the tile tables disagree".into());
    }
    let total: u64 = counts.iter().sum();
    if total > max_bytes {
        return Err(format!(
            "the smallest level holds {total} bytes of tiles, more than the {max_bytes} allowed"
        ));
    }
    // Tiles in file order, merged into requests.
    let mut order: Vec<usize> = (0..offsets.len()).filter(|&i| counts[i] > 0).collect();
    order.sort_by_key(|&i| offsets[i]);
    let mut tiles: Vec<Vec<u8>> = vec![Vec::new(); offsets.len()];
    let mut k = 0;
    while k < order.len() {
        let start = offsets[order[k]];
        let mut end = start + counts[order[k]];
        let mut j = k + 1;
        while j < order.len() {
            let (so, sc) = (offsets[order[j]], counts[order[j]]);
            if so > end + MERGE_GAP || so + sc - start > MAX_REQUEST {
                break;
            }
            end = end.max(so + sc);
            j += 1;
        }
        let d = src.read_range(start, end - start)?;
        s.requests += 1;
        s.bytes += d.len() as u64;
        if (d.len() as u64) < end - start {
            return Err("the server sent a short range".into());
        }
        for &i in &order[k..j] {
            let a = (offsets[i] - start) as usize;
            tiles[i] = d[a..a + counts[i] as usize].to_vec();
        }
        progress(s.bytes);
        k = j;
    }
    let image = write(&o, chosen, full, (fw, fh), (w, h), &tiles)?;
    Ok(Fetched {
        image,
        level,
        levels: images.len(),
        width: w as u32,
        height: h as u32,
        full: (fw as u32, fh as u32),
        bytes_read: s.bytes,
        requests: s.requests,
    })
}

/// A classic TIFF (in the source's byte order) of one level: its tags, the full image's
/// georeference scaled to it, and its tiles.
fn write(
    o: &Order,
    level: &Ifd,
    full: &Ifd,
    (fw, fh): (u64, u64),
    (w, h): (u64, u64),
    tiles: &[Vec<u8>],
) -> Result<Vec<u8>, String> {
    let (sx, sy) = (fw as f64 / w as f64, fh as f64 / h as f64);
    let mut entries: Vec<Entry> = level
        .entries
        .iter()
        .filter(|e| !matches!(e.tag, 324 | 325 | 273 | 279 | 330 | 254))
        .filter(|e| !matches!(e.ty, 16..=18))
        .cloned()
        .collect();
    for &t in GEO_TAGS {
        if entries.iter().any(|e| e.tag == t) {
            continue;
        }
        let Some(e) = full.get(t) else { continue };
        let mut e = e.clone();
        match t {
            33550 => {
                let v = o.doubles(&e);
                if v.len() >= 2 {
                    let mut d = Vec::new();
                    for (k, x) in v.iter().enumerate() {
                        let x = match k {
                            0 => x * sx,
                            1 => x * sy,
                            _ => *x,
                        };
                        d.extend_from_slice(&o.put_f64(x));
                    }
                    e.data = d;
                }
            }
            33922 => {
                // Raster points (i, j) of the tie points scale with the level.
                let v = o.doubles(&e);
                let mut d = Vec::new();
                for (k, x) in v.iter().enumerate() {
                    let x = match k % 6 {
                        0 => x / sx,
                        1 => x / sy,
                        _ => *x,
                    };
                    d.extend_from_slice(&o.put_f64(x));
                }
                e.data = d;
            }
            34264 => {
                let v = o.doubles(&e);
                if v.len() == 16 {
                    let mut d = Vec::new();
                    for (k, x) in v.iter().enumerate() {
                        let x = match k {
                            0 | 4 => x * sx,
                            1 | 5 => x * sy,
                            _ => *x,
                        };
                        d.extend_from_slice(&o.put_f64(x));
                    }
                    e.data = d;
                }
            }
            _ => {}
        }
        entries.push(e);
    }
    // Tile offsets and sizes, LONG.
    let n = tiles.len() as u64;
    entries.push(Entry {
        tag: 324,
        ty: 4,
        count: n,
        data: Vec::new(),
    });
    entries.push(Entry {
        tag: 325,
        ty: 4,
        count: n,
        data: tiles
            .iter()
            .flat_map(|t| o.put_u32(t.len() as u32))
            .collect(),
    });
    entries.sort_by_key(|e| e.tag);
    let total: u64 = tiles.iter().map(|t| t.len() as u64).sum();
    if total > u64::from(u32::MAX) / 2 {
        return Err("the level is too large to keep".into());
    }
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(if o.le { b"II" } else { b"MM" });
    out.extend_from_slice(&o.put_u16(42));
    out.extend_from_slice(&o.put_u32(8));
    let ifd_len = 2 + 12 * entries.len() + 4;
    let mut extra: Vec<u8> = Vec::new();
    let extra_at = 8 + ifd_len;
    let mut ifd: Vec<u8> = Vec::new();
    ifd.extend_from_slice(&o.put_u16(entries.len() as u16));
    // Out-of-line values after the IFD, then the tiles.
    let mut offset_slot = None;
    for e in &entries {
        ifd.extend_from_slice(&o.put_u16(e.tag));
        ifd.extend_from_slice(&o.put_u16(e.ty));
        ifd.extend_from_slice(&o.put_u32(e.count as u32));
        if e.tag == 324 {
            offset_slot = Some((ifd.len(), extra.len()));
            if n == 1 {
                ifd.extend_from_slice(&[0; 4]);
            } else {
                ifd.extend_from_slice(&o.put_u32((extra_at + extra.len()) as u32));
                extra.resize(extra.len() + 4 * n as usize, 0);
            }
            continue;
        }
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            ifd.extend_from_slice(&v);
        } else {
            if extra.len() % 2 == 1 {
                extra.push(0);
            }
            ifd.extend_from_slice(&o.put_u32((extra_at + extra.len()) as u32));
            extra.extend_from_slice(&e.data);
        }
    }
    ifd.extend_from_slice(&[0; 4]);
    debug_assert_eq!(ifd.len(), ifd_len);
    out.extend_from_slice(&ifd);
    out.extend_from_slice(&extra);
    // The tiles, and their offsets patched in.
    let mut tile_at = Vec::with_capacity(tiles.len());
    for t in tiles {
        tile_at.push(out.len() as u32);
        out.extend_from_slice(t);
    }
    let (slot, at) = offset_slot.ok_or("no offsets tag")?;
    if n == 1 {
        out[8 + slot..8 + slot + 4].copy_from_slice(&o.put_u32(tile_at[0]));
    } else {
        let base = extra_at + at;
        for (k, v) in tile_at.iter().enumerate() {
            out[base + 4 * k..base + 4 * k + 4].copy_from_slice(&o.put_u32(*v));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{self, GeoGrid};

    struct Counted(Vec<u8>, std::sync::Mutex<u32>);

    impl RangeRead for Counted {
        fn read_range(&self, offset: u64, len: u64) -> Result<Vec<u8>, String> {
            *self.1.lock().unwrap() += 1;
            let (o, l) = (offset as usize, len as usize);
            Ok(self.0[o.min(self.0.len())..(o + l).min(self.0.len())].to_vec())
        }
    }

    #[test]
    fn fetches_the_level_that_fits() {
        let (w, h) = (1000usize, 600usize);
        let values = fixtures::dem_values(w, h, false);
        let grid = GeoGrid {
            epsg: 32633,
            origin: (500_000.0, 4_000_000.0),
            pixel: (30.0, 30.0),
        };
        let tif = fixtures::geotiff_f32(&values, w, h, grid, None, 128);
        let src = Counted(tif.clone(), Default::default());
        // Full resolution fits.
        let f = fetch_level(&src, u64::MAX, u64::MAX, &mut |_| {}).unwrap();
        assert_eq!((f.level, f.width, f.height), (0, 1000, 600));
        assert!(f.levels >= 3);
        let full = specular_tiff::Tiff::open(&f.image).unwrap();
        let page = &full.pages()[0];
        assert_eq!((page.ifd().width, page.ifd().height), (1000, 600));
        // Only a quarter of the pixels: the first overview, 500 × 300, at 60 m.
        let f = fetch_level(&src, 200_000, u64::MAX, &mut |_| {}).unwrap();
        assert_eq!((f.level, f.width, f.height), (1, 500, 300));
        let t = specular_tiff::Tiff::open(&f.image).unwrap();
        let ifd = t.pages()[0].ifd();
        let g = ifd.geo.as_ref().expect("geotags carried over");
        assert_eq!(&g.pixel_scale[..2], &[60.0, 60.0]);
        assert_eq!(&g.tiepoints[3..5], &[500_000.0, 4_000_000.0]);
        // The bytes budget refuses with a reason.
        let e = fetch_level(&src, 100, 10, &mut |_| {}).unwrap_err();
        assert!(e.contains("bytes of tiles"), "{e}");
        assert!(
            fetch_level(
                &Counted(b"not a tiff".to_vec(), Default::default()),
                1,
                1,
                &mut |_| {}
            )
            .is_err()
        );
    }
}

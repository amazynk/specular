//! Persistent display cache: screen-sized whole-image levels
//! kept on disk, so reopening a large image shows it sharp without decoding it again.
//!
//! - Entries are keyed by the file's path, length and modification time, and the level's scale;
//!   the key is stored in each entry and checked on read (the file name is only a hash).
//! - Planes are stored as decoded (YCbCr with subsampled chroma stays ~1.5 bytes per pixel),
//!   uncompressed. LZ4 was measured and removed: on 3–6 MP entries it saved 24 % of the bytes
//!   but doubled read+decode from the page cache (4.4 → 8.9 ms) and only tied on a cold read
//!   (NVMe at ~1 GB/s: 20 ms raw vs 15 ms read + 5 ms decompress), at 12× the encode cost.
//!   BC7 would be 4× smaller than RGBA but encoding it costs far more than the decode it saves,
//!   and the renderer uploads planes, not blocks.
//! - Writes go through one background thread: a temporary file renamed into place, so a crash
//!   never leaves a half-written entry. Reads run on the decode worker that wanted the level.
//! - The folder is bounded: least recently used entries go first. "Used" is the file's
//!   modification time, which reads refresh.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Sender, TrySendError};
use parking_lot::Mutex;
use specular_core::{PixelLayout, Raster, RasterPlane};
use specular_log::{CACHE, debug};

const MAGIC: &[u8; 4] = b"SPDC";
/// 2: planes always stored raw (1 allowed LZ4).
const VERSION: u32 = 2;
const EXT: &str = "sdc";
/// Entries larger than this share of the budget are not stored.
const MAX_SHARE: u64 = 8;
/// Pending writes; more are dropped (the next open stores them again).
const QUEUE: usize = 4;
/// The cache's folder inside the platform cache folder.
pub const SUBDIR: &str = "display";

/// Identifies one version of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub path: PathBuf,
    pub len: u64,
    /// Modification time in nanoseconds since the epoch (negative before it).
    pub mtime_ns: i128,
    /// Page of a multi-image file (ICNS size, texture layer, TIFF page); 0 for the first.
    pub page: u32,
}

impl Key {
    pub fn new(path: &Path, len: u64, modified: Option<SystemTime>) -> Option<Key> {
        let m = modified?;
        let mtime_ns = match m.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_nanos() as i128,
            Err(e) => -(e.duration().as_nanos() as i128),
        };
        Some(Key {
            path: path.to_path_buf(),
            len,
            mtime_ns,
            page: 0,
        })
    }

    /// The same file's page `page`.
    pub fn with_page(self, page: u32) -> Key {
        Key { page, ..self }
    }

    /// What names the image: the path, and the page after a NUL (which no path contains)
    /// past the first, so first pages keep the entries stored before pages existed.
    fn name_bytes(&self) -> Vec<u8> {
        let mut b = self.path.as_os_str().as_encoded_bytes().to_vec();
        if self.page > 0 {
            b.push(0);
            b.extend_from_slice(format!("page {}", self.page).as_bytes());
        }
        b
    }

    /// Stable across runs and Rust versions (FNV-1a).
    fn hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h ^= x as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(&self.name_bytes());
        eat(&self.len.to_le_bytes());
        eat(&self.mtime_ns.to_le_bytes());
        // CMYK and L*a*b* pixels are converted while decoding, under the colour policy: a
        // changed policy misses (the default adds nothing, keeping existing entries valid).
        let p = specular_color::policy();
        if p != specular_color::Policy::default() {
            eat(&[p.enabled as u8, p.intent as u8, p.bpc as u8]);
        }
        h
    }

    pub fn file_name(&self, scale: u32) -> String {
        format!("{:016x}-{scale}.{EXT}", self.hash())
    }
}

/// Entry sizes and last use, loaded from the folder on the first write.
#[derive(Default)]
struct Index {
    files: HashMap<String, (u64, SystemTime)>,
    total: u64,
}

pub struct DisplayCache {
    dir: PathBuf,
    budget: u64,
    index: Arc<Mutex<Option<Index>>>,
    writer: Sender<(Key, Arc<Raster>)>,
    /// Entries queued and not yet written (or failed), for [`DisplayCache::flush`].
    pending: Arc<AtomicUsize>,
}

impl DisplayCache {
    /// A cache in `dir` holding at most `budget` bytes. The folder is created on the first
    /// write.
    pub fn new(dir: PathBuf, budget: u64) -> Arc<DisplayCache> {
        let (tx, rx) = crossbeam_channel::bounded::<(Key, Arc<Raster>)>(QUEUE);
        let index: Arc<Mutex<Option<Index>>> = Arc::default();
        let (d, ix) = (dir.clone(), index.clone());
        let pending = Arc::new(AtomicUsize::new(0));
        let left = pending.clone();
        let _ = specular_thread::builder("disp-cache")
            .spawn(move || {
                for (key, r) in rx {
                    let t0 = Instant::now();
                    match write_entry(&d, &key, &r, budget, &ix) {
                        Ok(n) => debug!(target: CACHE, "display cache: stored {} 1/{} ({} KiB) in {:.1} ms", key.path.display(), r.scale, n >> 10, t0.elapsed().as_secs_f64() * 1e3),
                        Err(e) => debug!(target: CACHE, "display cache: cannot store {}: {e}", key.path.display()),
                    }
                    left.fetch_sub(1, Ordering::AcqRel);
                }
            });
        Arc::new(DisplayCache {
            dir,
            budget,
            index,
            writer: tx,
            pending,
        })
    }

    /// Whether a level of this size is worth storing.
    pub fn accepts(&self, r: &Raster) -> bool {
        r.scale >= 2
            && r.origin == (0, 0)
            && !r.truncated
            && (r.memory_bytes() as u64) <= self.budget / MAX_SHARE
    }

    /// Queues `r` for storing (dropped when the writer is behind).
    pub fn put(&self, key: &Key, r: Arc<Raster>) {
        if !self.accepts(&r) {
            return;
        }
        // Counted before sending, so the writer never takes it below zero.
        self.pending.fetch_add(1, Ordering::AcqRel);
        match self.writer.try_send((key.clone(), r)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                debug!(target: CACHE, "display cache: writer busy, {} not stored", key.path.display())
            }
            Err(TrySendError::Disconnected(_)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }

    /// The stored level `scale` of `key`, if present and intact.
    pub fn get(&self, key: &Key, scale: u32) -> Option<Raster> {
        let name = key.file_name(scale);
        let path = self.dir.join(&name);
        let mut f = fs::File::open(&path).ok()?;
        match decode_from(&mut f, key, scale) {
            Ok(r) => {
                // Mark it used (the LRU order survives restarts through the file time).
                let now = SystemTime::now();
                let _ = f.set_modified(now);
                if let Some(ix) = self.index.lock().as_mut()
                    && let Some(e) = ix.files.get_mut(&name)
                {
                    e.1 = now;
                }
                Some(r)
            }
            Err(e) => {
                debug!(target: CACHE, "display cache: {name} unusable ({e}); removed");
                drop(f);
                let _ = fs::remove_file(&path);
                if let Some(ix) = self.index.lock().as_mut()
                    && let Some((n, _)) = ix.files.remove(&name)
                {
                    ix.total -= n;
                }
                None
            }
        }
    }

    /// Removes every stored level of `key` (the file was rewritten without changing its size
    /// or modification time, so its key no longer tells the versions apart).
    pub fn forget(&self, key: &Key) {
        let mut ix = self.index.lock();
        for shift in 1..=8 {
            let name = key.file_name(1 << shift);
            if fs::remove_file(self.dir.join(&name)).is_ok()
                && let Some(ix) = ix.as_mut()
                && let Some((n, _)) = ix.files.remove(&name)
            {
                ix.total -= n;
            }
        }
    }

    /// Bytes stored (after the first write has loaded the index).
    #[cfg(test)]
    pub fn stored_bytes(&self) -> Option<u64> {
        self.index.lock().as_ref().map(|i| i.total)
    }

    /// Blocks until queued writes are done (tests).
    #[cfg(test)]
    pub fn flush(&self) {
        // Waits for every queued entry to be written (or to fail); a minute at most, so a
        // stuck disk cannot hang the caller.
        let t0 = Instant::now();
        while self.pending.load(Ordering::Acquire) > 0 && t0.elapsed().as_secs() < 60 {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
}

fn layout_code(l: PixelLayout) -> u8 {
    match l {
        PixelLayout::Gray => 0,
        PixelLayout::YCbCr => 1,
        PixelLayout::Rgb => 2,
        PixelLayout::Rgba => 3,
        PixelLayout::Gray16 => 4,
        PixelLayout::GrayAlpha16 => 5,
        PixelLayout::Rgba16(e) => 0x40 | e.code(),
        PixelLayout::Rgba16F(p) => 0x80 | specular_core::hdr::primaries_code(p),
        // YCbCr planes (HEIC levels): the encoding's four bytes follow.
        PixelLayout::Yuv(_) => YUV,
    }
}

/// The layout code of YCbCr planes, followed by [`specular_core::YuvEncoding::to_bytes`].
const YUV: u8 = 0x20;

fn layout_of(c: u8) -> Option<(PixelLayout, usize)> {
    Some(match c {
        0 => (PixelLayout::Gray, 1),
        1 => (PixelLayout::YCbCr, 3),
        2 => (PixelLayout::Rgb, 3),
        3 => (PixelLayout::Rgba, 1),
        4 => (PixelLayout::Gray16, 1),
        5 => (PixelLayout::GrayAlpha16, 1),
        0x40..=0x7f => (
            PixelLayout::Rgba16(specular_core::HdrEncoding::from_code(c & 0x3f)?),
            1,
        ),
        0x80..=0x83 => (
            PixelLayout::Rgba16F(specular_core::hdr::primaries_of(c & 3)?),
            1,
        ),
        _ => return None,
    })
}

fn encode_entry(key: &Key, r: &Raster) -> Vec<u8> {
    let mut out = Vec::with_capacity(r.memory_bytes() + 256);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    let p = key.name_bytes();
    out.extend_from_slice(&(p.len() as u32).to_le_bytes());
    out.extend_from_slice(&p);
    out.extend_from_slice(&key.len.to_le_bytes());
    out.extend_from_slice(&key.mtime_ns.to_le_bytes());
    out.extend_from_slice(&r.scale.to_le_bytes());
    out.extend_from_slice(&r.width.to_le_bytes());
    out.extend_from_slice(&r.height.to_le_bytes());
    out.push(layout_code(r.layout));
    if let PixelLayout::Yuv(e) = r.layout {
        out.extend_from_slice(&e.to_bytes());
    }
    out.push(r.planes.len() as u8);
    // Compression: always 0 (raw).
    out.push(0);
    for pl in &r.planes {
        out.extend_from_slice(&pl.width.to_le_bytes());
        out.extend_from_slice(&pl.height.to_le_bytes());
        out.extend_from_slice(&(pl.stride as u64).to_le_bytes());
        out.push(pl.h_div);
        out.push(pl.v_div);
        out.extend_from_slice(&(pl.data.len() as u64).to_le_bytes());
        out.extend_from_slice(&(pl.data.len() as u64).to_le_bytes());
    }
    for pl in &r.planes {
        out.extend_from_slice(&pl.data);
    }
    out
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        if self.0.len() < n {
            return Err("cut short");
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().unwrap_or_default(),
        ))
    }
    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().unwrap_or_default(),
        ))
    }
    fn i128(&mut self) -> Result<i128, &'static str> {
        Ok(i128::from_le_bytes(
            self.take(16)?.try_into().unwrap_or_default(),
        ))
    }
}

/// The header of an entry is read from this much of its start (the planes follow it).
const PREFIX: u64 = 64 << 10;

/// Reads an entry: the header from its first bytes, then each plane straight into its own
/// buffer (one copy, not a whole-file read and a copy per plane: 2–5 ms of a 4 ms hit).
fn decode_from(f: &mut impl Read, key: &Key, scale: u32) -> Result<Raster, &'static str> {
    let mut pre = Vec::new();
    f.by_ref()
        .take(PREFIX)
        .read_to_end(&mut pre)
        .map_err(|_| "unreadable")?;
    let mut r = Reader(&pre);
    if r.take(4)? != MAGIC || r.u32()? != VERSION {
        return Err("not a cache entry of this version");
    }
    let n = r.u32()? as usize;
    let path = r.take(n)?;
    if path != key.name_bytes()
        || r.u64()? != key.len
        || r.i128()? != key.mtime_ns
        || r.u32()? != scale
    {
        return Err("another file (hash collision)");
    }
    let (width, height) = (r.u32()?, r.u32()?);
    let (layout, nplanes) = match r.u8()? {
        YUV => {
            let b = r.take(4)?;
            let e = specular_core::YuvEncoding::from_bytes([b[0], b[1], b[2], b[3]])
                .ok_or("unknown encoding")?;
            (PixelLayout::Yuv(e), 3 + usize::from(e.alpha))
        }
        c => layout_of(c).ok_or("unknown layout")?,
    };
    if r.u8()? as usize != nplanes {
        return Err("wrong plane count");
    }
    if r.u8()? != 0 {
        return Err("unknown compression");
    }
    if width == 0 || height == 0 || width > 1 << 16 || height > 1 << 16 {
        return Err("bad size");
    }
    let mut heads = Vec::with_capacity(nplanes);
    for _ in 0..nplanes {
        let (w, h) = (r.u32()?, r.u32()?);
        let stride = r.u64()? as usize;
        let (h_div, v_div) = (r.u8()?, r.u8()?);
        let (raw, stored) = (r.u64()? as usize, r.u64()? as usize);
        let bpp = layout.pixel_bytes();
        if h_div == 0
            || v_div == 0
            || w != width.div_ceil(h_div as u32)
            || h != height.div_ceil(v_div as u32)
            || stride < w as usize * bpp
            || stride > (w as usize + 64) * bpp
            || raw != stride * h as usize
        {
            return Err("bad plane");
        }
        heads.push((w, h, stride, h_div, v_div, raw, stored));
    }
    let mut left = r.0;
    let mut planes = Vec::with_capacity(nplanes);
    for (w, h, stride, h_div, v_div, raw, stored) in heads {
        if stored != raw {
            return Err("corrupt");
        }
        let mut data = Vec::new();
        data.try_reserve_exact(stored).map_err(|_| "too big")?;
        let k = left.len().min(stored);
        data.extend_from_slice(&left[..k]);
        left = &left[k..];
        if data.len() < stored {
            f.by_ref()
                .take((stored - data.len()) as u64)
                .read_to_end(&mut data)
                .map_err(|_| "unreadable")?;
        }
        if data.len() != raw {
            return Err("truncated");
        }
        planes.push(RasterPlane {
            width: w,
            height: h,
            stride,
            data,
            h_div,
            v_div,
        });
    }
    if !left.is_empty() || f.read(&mut [0u8; 1]).map_or(true, |n| n > 0) {
        return Err("trailing data");
    }
    Ok(Raster {
        width,
        height,
        origin: (0, 0),
        scale,
        layout,
        planes,
        truncated: false,
    })
}

fn load_index(dir: &Path) -> Index {
    let mut ix = Index::default();
    let Ok(rd) = fs::read_dir(dir) else { return ix };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() {
            continue;
        }
        if !name.ends_with(EXT) {
            // A temporary file left by a crash.
            if name.ends_with(".tmp") {
                let _ = fs::remove_file(e.path());
            }
            continue;
        }
        let used = m.modified().unwrap_or(UNIX_EPOCH);
        ix.total += m.len();
        ix.files.insert(name, (m.len(), used));
    }
    ix
}

fn write_entry(
    dir: &Path,
    key: &Key,
    r: &Raster,
    budget: u64,
    index: &Mutex<Option<Index>>,
) -> io::Result<u64> {
    fs::create_dir_all(dir)?;
    if index.lock().is_none() {
        let ix = load_index(dir);
        *index.lock() = Some(ix);
    }
    let bytes = encode_entry(key, r);
    let name = key.file_name(r.scale);
    let tmp = dir.join(format!("{name}.{}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
    }
    fs::rename(&tmp, dir.join(&name)).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    let n = bytes.len() as u64;
    let mut g = index.lock();
    let ix = g.get_or_insert_with(Index::default);
    if let Some((old, _)) = ix.files.insert(name.clone(), (n, SystemTime::now())) {
        ix.total -= old;
    }
    ix.total += n;
    if ix.total > budget {
        evict(dir, ix, budget, &name);
    }
    Ok(n)
}

/// Removes the least recently used entries (never `keep`) until the folder fits `budget`.
fn evict(dir: &Path, ix: &mut Index, budget: u64, keep: &str) {
    let mut by_age: Vec<(SystemTime, String, u64)> = ix
        .files
        .iter()
        .filter(|(n, _)| n.as_str() != keep)
        .map(|(n, (s, t))| (*t, n.clone(), *s))
        .collect();
    by_age.sort();
    for (_, name, size) in by_age {
        if ix.total <= budget {
            break;
        }
        match fs::remove_file(dir.join(&name)) {
            Ok(()) => debug!(target: CACHE, "display cache: evicted {name} ({} KiB)", size >> 10),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => continue,
        }
        ix.files.remove(&name);
        ix.total -= size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-data/display-cache")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn raster(w: u32, h: u32, seed: u8) -> Raster {
        let plane = |pw: u32, ph: u32, div: u8, k: u8| RasterPlane {
            width: pw,
            height: ph,
            stride: pw as usize,
            data: (0..pw as usize * ph as usize)
                .map(|i| (i as u8).wrapping_mul(k).wrapping_add(seed))
                .collect(),
            h_div: div,
            v_div: div,
        };
        Raster {
            width: w,
            height: h,
            origin: (0, 0),
            scale: 4,
            layout: PixelLayout::YCbCr,
            planes: vec![
                plane(w, h, 1, 7),
                plane(w.div_ceil(2), h.div_ceil(2), 2, 3),
                plane(w.div_ceil(2), h.div_ceil(2), 2, 5),
            ],
            truncated: false,
        }
    }

    fn key(n: &str, mtime: i128) -> Key {
        Key {
            path: PathBuf::from(format!("/photos/{n}.jpg")),
            len: 1234,
            mtime_ns: mtime,
            page: 0,
        }
    }

    fn same(a: &Raster, b: &Raster) {
        assert_eq!(
            (a.width, a.height, a.scale, a.layout),
            (b.width, b.height, b.scale, b.layout)
        );
        for (p, q) in a.planes.iter().zip(&b.planes) {
            assert_eq!(
                (p.width, p.height, p.stride, p.h_div, p.v_div),
                (q.width, q.height, q.stride, q.h_div, q.v_div)
            );
            assert!(p.data == q.data);
        }
    }

    #[test]
    fn round_trips_and_checks_the_key() {
        {
            let c = DisplayCache::new(dir("rt"), 64 << 20);
            let r = raster(301, 199, 1);
            let k = key("a", 10);
            assert!(c.get(&k, 4).is_none());
            c.put(&k, Arc::new(r.clone()));
            c.flush();
            same(&c.get(&k, 4).expect("stored"), &r);
            // Another scale, a newer version of the file, another file: misses.
            assert!(c.get(&k, 8).is_none());
            assert!(c.get(&key("a", 11), 4).is_none());
            assert!(c.get(&key("b", 10), 4).is_none());
            // Another page of the same file misses, and is stored apart.
            let p2 = key("a", 10).with_page(2);
            assert!(c.get(&p2, 4).is_none());
            assert_ne!(p2.file_name(4), k.file_name(4));
            let r2 = raster(40, 30, 1);
            c.put(&p2, Arc::new(r2.clone()));
            c.flush();
            same(&c.get(&p2, 4).expect("page stored"), &r2);
            same(&c.get(&k, 4).expect("first page kept"), &r);
            // The first page's name is the one entries had before pages.
            assert_eq!(k.clone().with_page(0).file_name(4), k.file_name(4));
        }
    }

    /// HEIC levels: 10-bit YCbCr planes with alpha, their encoding kept.
    #[test]
    fn round_trips_ycbcr_planes() {
        let c = DisplayCache::new(dir("yuv"), 64 << 20);
        let e = specular_core::YuvEncoding {
            matrix: specular_core::YuvMatrix::Bt709,
            full_range: true,
            bits: 10,
            cosited_x: false,
            cosited_y: false,
            bottom_y: true,
            right_x: true,
            alpha: true,
            colour: specular_core::HdrEncoding::SRGB,
        };
        let mut r = raster(51, 37, 3);
        for p in &mut r.planes {
            // 16-bit samples, every plane at the level's size.
            (p.width, p.height, p.h_div, p.v_div) = (51, 37, 1, 1);
            p.stride = 102;
            p.data = (0..102 * 37).map(|i| i as u8).collect();
        }
        r.planes.push(r.planes[0].clone());
        r.layout = PixelLayout::Yuv(e);
        let k = key("heic", 1);
        c.put(&k, Arc::new(r.clone()));
        c.flush();
        same(&c.get(&k, 4).expect("stored"), &r);
    }

    #[test]
    fn round_trips_16_bit_grey() {
        let c = DisplayCache::new(dir("grey16"), 64 << 20);
        for (i, layout) in [PixelLayout::Gray16, PixelLayout::GrayAlpha16]
            .into_iter()
            .enumerate()
        {
            let (w, h) = (101u32, 67u32);
            let stride = w as usize * layout.pixel_bytes() + 6;
            let r = Raster {
                width: w,
                height: h,
                origin: (0, 0),
                scale: 4,
                layout,
                planes: vec![RasterPlane {
                    width: w,
                    height: h,
                    stride,
                    data: (0..stride * h as usize).map(|j| (j * 13) as u8).collect(),
                    h_div: 1,
                    v_div: 1,
                }],
                truncated: false,
            };
            let k = key(&format!("g{i}"), 1);
            c.put(&k, Arc::new(r.clone()));
            c.flush();
            same(&c.get(&k, 4).expect("stored"), &r);
        }
    }

    #[test]
    fn damaged_entries_are_dropped() {
        let d = dir("damaged");
        let c = DisplayCache::new(d.clone(), 64 << 20);
        let k = key("a", 1);
        c.put(&k, Arc::new(raster(64, 64, 2)));
        c.flush();
        let f = d.join(k.file_name(4));
        let mut bytes = fs::read(&f).unwrap();
        bytes.truncate(bytes.len() - 10);
        fs::write(&f, &bytes).unwrap();
        assert!(c.get(&k, 4).is_none());
        assert!(!f.exists(), "removed");
        // Another file's entry under this name (a hash collision) is refused but kept out.
        let other = key("b", 1);
        fs::write(&f, encode_entry(&other, &raster(64, 64, 2))).unwrap();
        assert!(c.get(&k, 4).is_none());
    }

    #[test]
    fn evicts_least_recently_used() {
        let d = dir("lru");
        let one = encode_entry(&key("k0", 0), &raster(128, 128, 0)).len() as u64;
        // Room for eight entries (the most an entry may take is 1/8 of the budget).
        let budget = one * 8 + one / 2;
        let c = DisplayCache::new(d.clone(), budget);
        let keys: Vec<Key> = (0..9).map(|i| key(&format!("k{i}"), i)).collect();
        for k in &keys[..8] {
            c.put(k, Arc::new(raster(128, 128, 3)));
            c.flush();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        // Using the first makes the second the oldest.
        assert!(c.get(&keys[0], 4).is_some());
        std::thread::sleep(std::time::Duration::from_millis(15));
        c.put(&keys[8], Arc::new(raster(128, 128, 3)));
        c.flush();
        assert!(c.get(&keys[1], 4).is_none(), "the least recently used went");
        for (i, k) in keys.iter().enumerate().filter(|(i, _)| *i != 1) {
            assert!(c.get(k, 4).is_some(), "entry {i} kept");
        }
        assert!(c.stored_bytes().unwrap() <= budget);
        // A new cache on the same folder sees the same entries.
        let again = DisplayCache::new(d, budget);
        assert!(again.get(&keys[8], 4).is_some());
    }

    #[test]
    fn only_screen_sized_whole_levels() {
        let c = DisplayCache::new(dir("accepts"), 64 << 20);
        let mut r = raster(64, 64, 0);
        assert!(c.accepts(&r));
        r.scale = 1;
        assert!(!c.accepts(&r), "full size");
        r.scale = 2;
        r.truncated = true;
        assert!(!c.accepts(&r), "truncated");
        r.truncated = false;
        r.origin = (8, 0);
        assert!(!c.accepts(&r), "a region");
        let small = DisplayCache::new(dir("accepts-small"), 8 << 10);
        assert!(!small.accepts(&raster(64, 64, 0)), "over 1/8 of the budget");
    }
}

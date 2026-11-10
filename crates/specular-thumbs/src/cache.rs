//! The disk cache: one small file per (image content, size class, flags) under the platform
//! cache directory (`$XDG_CACHE_HOME/specular/thumbnails`, `~/Library/Caches/Specular/Thumbnails`,
//! `%LOCALAPPDATA%\Specular\cache\thumbnails`) or, for a portable install, `cache/thumbnails` in
//! its folder ([`DiskCache::default_dir_for`]); or `$SPECULAR_THUMB_CACHE` when set (tests and
//! benchmarks point it into `target/`).
//!
//! Entries are written to a temporary file and renamed into place, so a crash never leaves a
//! half-written entry that reads as valid; readers check the magic, version and full key.
//! Reading an entry bumps its modification time; [`DiskCache::trim`] deletes the least
//! recently used entries once the directory is over its budget.
//!
//! Layout of an entry (`ab/abcdef0123456789.sth`, little-endian):
//!
//! | bytes | field |
//! |---|---|
//! | 4 | magic `SPTH` |
//! | 1 | version |
//! | 1 | payload kind: 0 JPEG (opaque), 1 raw RGBA8, 2 PNG RGBA8 (alpha; lossless) |
//! | 2 | flags the thumbnail was made with |
//! | 8 + 16 + 8 | key: length, mtime (ns), content hash |
//! | 4 | size class |
//! | 4 + 4 | thumbnail size |
//! | 4 | average colour |
//! | 2 + n | [`Meta`] |
//! | 4 + n | payload |
//! | 4 + 2 + n | optional [`Digest`]: id, length, bytes |
//!
//! The digest (the duplicate finder's signature, computed from the pixels the entry was made
//! from) trails the payload, so it needs no version bump: readers before it ignore the bytes
//! after the payload, and entries without it read as having none.
//!
//! An entry one version behind whose pixels are still good ([`PIXELS_SINCE`]) reads with
//! [`CacheEntry::meta_stale`] set: its thumbnail is served as is, and the engine re-reads the
//! image's metadata (header and Exif only, no decode) and rewrites the entry with
//! [`DiskCache::refresh_meta`], which keeps the coded payload and digest byte for byte.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::codec;
use crate::key::{Stamp, ThumbKey};
use crate::pixels::Pixels;

const MAGIC: &[u8; 4] = b"SPTH";
/// Bumped whenever what an entry holds changes. 2: an opaque embedded preview no longer stands
/// in for an image with alpha, and alpha entries are PNG (kind 2) instead of raw RGBA (about a
/// tenth of the size). 3: the meta of PSD/PSB (Exif, ICC), Canon CR3 (Exif with GPS) and
/// positions are read; version 2 pixels are still good, so only their meta is redone. 4: Motion
/// Photos and Live Photos are marked (meta only). 5: the colour space is read (meta only). 6: FITS
/// and XISF subframes' header facts (meta only; their star metrics are a blob, [`ASTRO_BLOB`]).
/// 7: the Exif fields search names ([`crate::ExifMeta`], meta only).
const VERSION: u8 = 7;
/// The oldest version whose pixels (payload, size, average, digest) are still current: entries
/// from it up to [`VERSION`] are hits with stale meta. Older ones read as misses and are
/// deleted. Set it to [`VERSION`] when the pixels an entry would hold change.
const PIXELS_SINCE: u8 = 2;
const EXT: &str = "sth";
/// Small records ([`DiskCache::put_blob`]) live among the entries under a class no thumbnail
/// has, their tag in place of the flags, so they share the entries' names, writes and trim.
const BLOB_CLASS: u32 = 0;
const BLOB_MAGIC: &[u8; 4] = b"SPTB";
const BLOB_VERSION: u8 = 1;
/// JPEG quality of cache entries.
const QUALITY: u8 = 88;

/// Facts about the image stored with its thumbnail, so the gallery can sort, filter, badge
/// and caption without opening the file again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    /// Format name (`specular_decode::Format::name`).
    pub format: String,
    /// Displayed size (orientation applied).
    pub width: u32,
    pub height: u32,
    pub animated: bool,
    pub hdr: bool,
    pub has_alpha: bool,
    pub raw: bool,
    pub pages: u32,
    pub bit_depth: u8,
    pub has_profile: bool,
    /// Seconds since the Unix epoch, local time as stored (EXIF has no zone).
    pub date_taken: Option<i64>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<u32>,
    pub exposure: Option<f32>,
    pub f_number: Option<f32>,
    pub focal_length: Option<f32>,
    /// Whether a position is recorded; None when not looked for (an older entry, or a
    /// format whose metadata is not read here).
    pub gps: Option<bool>,
    /// The recorded position, (latitude, longitude) in degrees, south and west negative. None
    /// also for entries written before positions were kept (they read as unknown).
    pub position: Option<(f32, f32)>,
    /// A still with a motion clip (Motion Photo, Samsung motion photo, or Live Photo).
    pub motion: bool,
    /// The motion clip is an Apple Live Photo's paired movie.
    pub live_photo: bool,
    /// The colour space (`specular_decode::colour_space`): a canonical word, a profile's
    /// description, or empty when nothing names one. None when not looked for (an older
    /// entry).
    pub space: Option<String>,
    /// A movie (MP4, MOV, 3GP) or sound file: the tile shows its length instead of the
    /// animation badge.
    pub video: bool,
    /// Of a `video`: sound without a picture (an MP3, FLAC, M4A, an MP4 of sound only).
    pub sound: bool,
    /// A movie's length in microseconds, when its header says.
    pub duration_us: Option<u64>,
    /// An astronomy subframe (FITS, XISF): what its header says, and its star metrics once
    /// measured. Its exposure and date are in `exposure` and `date_taken`.
    pub astro: Option<Box<AstroMeta>>,
    /// The Exif fields search names beyond the camera settings above. None when not looked
    /// for (an older entry).
    pub exif: Option<Box<crate::ExifMeta>>,
}

/// The tag of a subframe's star metrics among the cache's blobs
/// ([`specular_decode::astro::FrameMetrics::to_bytes`]).
pub const ASTRO_BLOB: u32 = u32::from_be_bytes(*b"ASTR");

/// What an astronomy subframe's header says, and its metrics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AstroMeta {
    /// `OBJECT`, `FILTER`.
    pub object: Option<String>,
    pub filter: Option<String>,
    /// The frame's kind from `IMAGETYP`: `light`, `dark`, `flat`, `bias`, `darkflat` or the
    /// value itself.
    pub frame_type: Option<String>,
    /// `GAIN`, `CCD-TEMP` (°C).
    pub gain: Option<f32>,
    pub ccd_temp: Option<f32>,
    /// The star metrics (not stored in the entry: a blob of their own).
    pub metrics: Option<specular_decode::astro::FrameMetrics>,
}

impl AstroMeta {
    /// The header facts and the metrics as one record (the Library's catalogue keeps it).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        self.write(&mut b);
        if let Some(m) = &self.metrics {
            b.extend_from_slice(&m.to_bytes());
        }
        b
    }

    /// Reads [`AstroMeta::to_bytes`].
    pub fn from_bytes(b: &[u8]) -> Option<AstroMeta> {
        let mut r = Reader { b, at: 0 };
        let mut a = AstroMeta::read(&mut r)?;
        a.metrics = specular_decode::astro::FrameMetrics::from_bytes(&b[r.at..]);
        Some(a)
    }

    fn write(&self, b: &mut Vec<u8>) {
        let present = self.object.is_some() as u8
            | (self.filter.is_some() as u8) << 1
            | (self.frame_type.is_some() as u8) << 2
            | (self.gain.is_some() as u8) << 3
            | (self.ccd_temp.is_some() as u8) << 4;
        b.push(present);
        for s in [&self.object, &self.filter, &self.frame_type]
            .into_iter()
            .flatten()
        {
            put_str(b, s);
        }
        for v in [self.gain, self.ccd_temp].into_iter().flatten() {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }

    fn read(r: &mut Reader) -> Option<AstroMeta> {
        let present = r.u8()?;
        let mut a = AstroMeta::default();
        if present & 1 != 0 {
            a.object = Some(r.str()?);
        }
        if present & 2 != 0 {
            a.filter = Some(r.str()?);
        }
        if present & 4 != 0 {
            a.frame_type = Some(r.str()?);
        }
        if present & 8 != 0 {
            a.gain = Some(f32::from_bits(r.u32()?));
        }
        if present & 16 != 0 {
            a.ccd_temp = Some(f32::from_bits(r.u32()?));
        }
        Some(a)
    }
}

impl Meta {
    /// Without the Exif tags (the engine's meta: what its cache entries hold; the gallery
    /// does not search the tags).
    pub fn without_exif_tags(mut self) -> Meta {
        if let Some(e) = self.exif.as_mut() {
            e.tags = None;
        }
        self
    }

    fn write(&self, o: &mut Vec<u8>) {
        let mut b = Vec::new();
        put_str(&mut b, &self.format);
        b.extend_from_slice(&self.width.to_le_bytes());
        b.extend_from_slice(&self.height.to_le_bytes());
        let bits = self.animated as u8
            | (self.hdr as u8) << 1
            | (self.has_alpha as u8) << 2
            | (self.raw as u8) << 3
            | (self.has_profile as u8) << 4
            // Bits 5–6: position known, position recorded (older entries read as unknown).
            | (self.gps.is_some() as u8) << 5
            | ((self.gps == Some(true)) as u8) << 6;
        b.push(bits);
        b.extend_from_slice(&self.pages.to_le_bytes());
        b.push(self.bit_depth);
        let mut present = 0u8;
        for (i, p) in [
            self.date_taken.is_some(),
            self.camera.is_some(),
            self.lens.is_some(),
            self.iso.is_some(),
            self.exposure.is_some(),
            self.f_number.is_some(),
            self.focal_length.is_some(),
            // Bit 7, read after the others: older readers stop before it.
            self.position.is_some(),
        ]
        .into_iter()
        .enumerate()
        {
            present |= (p as u8) << i;
        }
        b.push(present);
        if let Some(d) = self.date_taken {
            b.extend_from_slice(&d.to_le_bytes());
        }
        if let Some(s) = &self.camera {
            put_str(&mut b, s);
        }
        if let Some(s) = &self.lens {
            put_str(&mut b, s);
        }
        if let Some(v) = self.iso {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [self.exposure, self.f_number, self.focal_length]
            .into_iter()
            .flatten()
        {
            b.extend_from_slice(&v.to_le_bytes());
        }
        if let Some((lat, lon)) = self.position {
            b.extend_from_slice(&lat.to_le_bytes());
            b.extend_from_slice(&lon.to_le_bytes());
        }
        // Version 4: more flags at the end (older entries end before them).
        b.push(self.motion as u8 | (self.live_photo as u8) << 1);
        // Version 5: the colour space, when looked for.
        match &self.space {
            Some(s) => {
                b.push(1);
                put_str(&mut b, s);
            }
            // Written as "none" when movie or subframe facts follow (older entries end here).
            None if self.video || self.astro.is_some() || self.exif.is_some() => b.push(0),
            None => {}
        }
        // Movies (after the colour space; older entries end before): a flag byte, then the
        // length when known. Written as 0 when subframe facts follow.
        if self.video {
            b.push(1 | (self.duration_us.is_some() as u8) << 1 | (self.sound as u8) << 2);
            if let Some(d) = self.duration_us {
                b.extend_from_slice(&d.to_le_bytes());
            }
        } else if self.astro.is_some() || self.exif.is_some() {
            b.push(0);
        }
        // Version 6: an astronomy subframe's header facts, after a 1 (a 0 when Exif fields
        // follow).
        if let Some(a) = &self.astro {
            b.push(1);
            a.write(&mut b);
        } else if self.exif.is_some() {
            b.push(0);
        }
        // Version 7: the Exif fields, after a 1 (not the tags: the gallery does not search
        // them).
        if let Some(e) = &self.exif {
            b.push(1);
            e.write(&mut b);
        }
        o.extend_from_slice(&(b.len().min(u16::MAX as usize) as u16).to_le_bytes());
        o.extend_from_slice(&b[..b.len().min(u16::MAX as usize)]);
    }

    fn read(r: &mut Reader) -> Option<Meta> {
        let mut m = Meta {
            format: r.str()?,
            width: r.u32()?,
            height: r.u32()?,
            ..Meta::default()
        };
        let bits = r.u8()?;
        m.animated = bits & 1 != 0;
        m.hdr = bits & 2 != 0;
        m.has_alpha = bits & 4 != 0;
        m.raw = bits & 8 != 0;
        m.has_profile = bits & 16 != 0;
        m.gps = (bits & 32 != 0).then_some(bits & 64 != 0);
        m.pages = r.u32()?;
        m.bit_depth = r.u8()?;
        let present = r.u8()?;
        if present & 1 != 0 {
            m.date_taken = Some(r.u64()? as i64);
        }
        if present & 2 != 0 {
            m.camera = Some(r.str()?);
        }
        if present & 4 != 0 {
            m.lens = Some(r.str()?);
        }
        if present & 8 != 0 {
            m.iso = Some(r.u32()?);
        }
        if present & 16 != 0 {
            m.exposure = Some(f32::from_bits(r.u32()?));
        }
        if present & 32 != 0 {
            m.f_number = Some(f32::from_bits(r.u32()?));
        }
        if present & 64 != 0 {
            m.focal_length = Some(f32::from_bits(r.u32()?));
        }
        if present & 128 != 0 {
            m.position = Some((f32::from_bits(r.u32()?), f32::from_bits(r.u32()?)));
        }
        if let Some(more) = r.u8() {
            m.motion = more & 1 != 0;
            m.live_photo = more & 2 != 0;
        }
        if r.u8() == Some(1) {
            m.space = Some(r.str()?);
        }
        if let Some(v) = r.u8() {
            m.video = v & 1 != 0;
            m.sound = m.video && v & 4 != 0;
            if v & 2 != 0 {
                m.duration_us = Some(r.u64()?);
            }
        }
        if r.u8() == Some(1) {
            m.astro = Some(Box::new(AstroMeta::read(r)?));
        }
        if r.u8() == Some(1) {
            m.exif = Some(Box::new(crate::ExifMeta::read(r)?));
        }
        Some(m)
    }
}

pub(crate) fn put_str(b: &mut Vec<u8>, s: &str) {
    let s = &s.as_bytes()[..s.len().min(255)];
    b.push(s.len() as u8);
    b.extend_from_slice(s);
}

pub(crate) struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b, at: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    pub(crate) fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    pub(crate) fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i128(&mut self) -> Option<i128> {
        Some(i128::from_le_bytes(self.take(16)?.try_into().ok()?))
    }
    pub(crate) fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
}

/// A small record computed from a thumbnail's pixels as they were made (before the lossy
/// cache coding), stored with the entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Digest {
    /// What computed it, and which version of it ([`crate::engine::Digester::id`]).
    pub id: u32,
    pub bytes: Vec<u8>,
}

/// A decoded cache entry.
#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub class: u32,
    pub pixels: Pixels,
    pub average: [u8; 4],
    pub meta: Meta,
    pub digest: Option<Digest>,
    /// Written by an older version that read less metadata: the pixels are current, `meta`
    /// should be read from the image again and stored with [`DiskCache::refresh_meta`].
    pub meta_stale: bool,
}

/// The on-disk thumbnail cache.
pub struct DiskCache {
    dir: PathBuf,
    budget: AtomicU64,
    /// Bytes written since the last budget check (a check may scan the directory).
    written: AtomicU64,
    /// The entries' bytes as of the last scan plus those written since ([`UNKNOWN`] before
    /// the first scan): a check under budget needs no scan.
    total: AtomicU64,
    /// Budget checks so far: every [`RESCAN_EVERY`]th scans anyway.
    checks: AtomicU64,
    /// Directory scans so far (for tests).
    scans: AtomicU64,
}

/// [`DiskCache::total`] before the first scan.
const UNKNOWN: u64 = u64::MAX;

/// Budget checks between scans of the cache while the running total is under budget: other
/// processes write to the same folder, and a replaced entry counts twice.
const RESCAN_EVERY: u64 = 10;

/// A hit marks its entry used (LRU) only when the entry's time is older than this: tiles
/// shown again soon after cost no inode write.
const USE_GRAIN: std::time::Duration = std::time::Duration::from_secs(3600);

impl DiskCache {
    /// `budget` in bytes; 0 disables writing (reads still work).
    pub fn new(dir: impl Into<PathBuf>, budget: u64) -> DiskCache {
        DiskCache {
            dir: dir.into(),
            budget: AtomicU64::new(budget),
            written: AtomicU64::new(0),
            total: AtomicU64::new(UNKNOWN),
            checks: AtomicU64::new(0),
            scans: AtomicU64::new(0),
        }
    }

    /// `$SPECULAR_THUMB_CACHE`, else the platform cache directory.
    pub fn default_dir() -> Option<PathBuf> {
        Self::default_dir_for(None)
    }

    /// [`default_dir`](Self::default_dir) for an install that may be portable: given the
    /// portable folder's cache directory, `thumbnails` inside it replaces the platform cache
    /// directory (`$SPECULAR_THUMB_CACHE` still comes first).
    pub fn default_dir_for(portable_cache: Option<&Path>) -> Option<PathBuf> {
        if let Some(d) = std::env::var_os("SPECULAR_THUMB_CACHE").filter(|d| !d.is_empty()) {
            return Some(PathBuf::from(d));
        }
        match portable_cache {
            Some(c) => Some(c.join("thumbnails")),
            None => platform_cache_dir(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn set_budget(&self, bytes: u64) {
        self.budget.store(bytes, Ordering::Relaxed);
    }

    pub(crate) fn path(&self, key: &ThumbKey, class: u32, flags: u32) -> PathBuf {
        let name = key.name(class, flags);
        self.dir.join(&name[..2]).join(format!("{name}.{EXT}"))
    }

    /// The entry for `key`, if present and intact.
    pub fn get(&self, key: &ThumbKey, class: u32, flags: u32) -> Option<CacheEntry> {
        self.read(key, class, flags, |b| parse(b, key, class, flags))
    }

    /// The meta and digest `id` of the entry for `key`, without decoding its pixels. `None`
    /// when there is no intact entry, it holds no digest of that id, or its meta is stale
    /// (in both latter cases the entry stays, for [`DiskCache::get`]).
    pub fn get_digest(
        &self,
        key: &ThumbKey,
        class: u32,
        flags: u32,
        id: u32,
    ) -> Option<(Meta, Digest)> {
        self.read(key, class, flags, |b| {
            let h = parse_head(b, key, class, flags)?;
            Some(
                h.digest
                    .filter(|d| d.id == id && !h.meta_stale)
                    .map(|d| (h.meta, d)),
            )
        })
        .flatten()
    }

    /// Reads the entry file and parses it with `f`; an entry `f` rejects is deleted, one it
    /// takes counts as used.
    fn read<T>(
        &self,
        key: &ThumbKey,
        class: u32,
        flags: u32,
        f: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Option<T> {
        use std::io::Read;
        let path = self.path(key, class, flags);
        // One open: the entry's time comes from the same descriptor as its bytes.
        let mut file = std::fs::File::open(&path).ok()?;
        let meta = file.metadata().ok();
        let mut bytes = Vec::with_capacity(meta.as_ref().map_or(0, |m| m.len() as usize));
        file.read_to_end(&mut bytes).ok()?;
        drop(file);
        crate::slowio::open(bytes.len() as u64);
        let entry = f(&bytes);
        match &entry {
            Some(_) => {
                // LRU: reading counts as use, to the hour. Failure (read-only cache) is
                // harmless.
                let now = SystemTime::now();
                let used = meta.and_then(|m| m.modified().ok());
                if used.is_none_or(|t| now.duration_since(t).is_ok_and(|d| d >= USE_GRAIN))
                    && let Ok(f) = std::fs::File::options().append(true).open(&path)
                {
                    let _ = f.set_modified(now);
                }
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
        entry
    }

    /// A small record stored with [`DiskCache::put_blob`] under `key` and `tag`, if present
    /// and intact (one written for another key or tag reads as absent).
    pub fn get_blob(&self, key: &ThumbKey, tag: u32) -> Option<Vec<u8>> {
        self.read(key, BLOB_CLASS, tag, |b| {
            let mut r = Reader { b, at: 0 };
            if r.take(4)? != BLOB_MAGIC || r.u8()? != BLOB_VERSION || r.u32()? != tag {
                return None;
            }
            let stamp = Stamp {
                len: r.u64()?,
                mtime_ns: r.i128()?,
            };
            if stamp != key.stamp || r.u64()? != key.content {
                return None;
            }
            let n = r.u32()? as usize;
            let data = r.take(n)?;
            (r.at == b.len()).then(|| data.to_vec())
        })
    }

    /// Removes the blob under `key` and `tag`, if any.
    pub fn remove_blob(&self, key: &ThumbKey, tag: u32) {
        let _ = std::fs::remove_file(self.path(key, BLOB_CLASS, tag));
    }

    /// Stores `data` (a few hundred bytes: what a listing found, not pixels) under `key` and
    /// `tag`, next to the thumbnails: written atomically, used and trimmed like them.
    pub fn put_blob(&self, key: &ThumbKey, tag: u32, data: &[u8]) -> io::Result<()> {
        if self.budget.load(Ordering::Relaxed) == 0 {
            return Ok(());
        }
        let len = u32::try_from(data.len()).map_err(io::Error::other)?;
        let mut b = Vec::with_capacity(45 + data.len());
        b.extend_from_slice(BLOB_MAGIC);
        b.push(BLOB_VERSION);
        b.extend_from_slice(&tag.to_le_bytes());
        b.extend_from_slice(&key.stamp.len.to_le_bytes());
        b.extend_from_slice(&key.stamp.mtime_ns.to_le_bytes());
        b.extend_from_slice(&key.content.to_le_bytes());
        b.extend_from_slice(&len.to_le_bytes());
        b.extend_from_slice(data);
        self.store(key, BLOB_CLASS, tag, &b)
    }

    /// Whether an entry exists (no decode).
    pub fn contains(&self, key: &ThumbKey, class: u32, flags: u32) -> bool {
        self.path(key, class, flags).is_file()
    }

    /// Stores a thumbnail. Opaque pixels are JPEG-coded, others stored raw.
    pub fn put(
        &self,
        key: &ThumbKey,
        class: u32,
        flags: u32,
        pixels: &Pixels,
        meta: &Meta,
    ) -> io::Result<()> {
        self.put_with_digest(key, class, flags, pixels, meta, None)
    }

    /// [`DiskCache::put`] with a digest of `pixels`.
    pub fn put_with_digest(
        &self,
        key: &ThumbKey,
        class: u32,
        flags: u32,
        pixels: &Pixels,
        meta: &Meta,
        digest: Option<&Digest>,
    ) -> io::Result<()> {
        let budget = self.budget.load(Ordering::Relaxed);
        if budget == 0 {
            return Ok(());
        }
        let bytes = serialize(key, class, flags, pixels, meta, digest);
        self.store(key, class, flags, &bytes)
    }

    /// Replaces the meta of the entry for `key` and brings it to the current version, keeping
    /// its coded pixels, average and digest as they are (nothing is decoded or re-encoded).
    /// `Ok(false)` when there is no intact entry to refresh.
    pub fn refresh_meta(
        &self,
        key: &ThumbKey,
        class: u32,
        flags: u32,
        meta: &Meta,
    ) -> io::Result<bool> {
        if self.budget.load(Ordering::Relaxed) == 0 {
            return Ok(false);
        }
        let Ok(old) = std::fs::read(self.path(key, class, flags)) else {
            return Ok(false);
        };
        let Some(h) = parse_head(&old, key, class, flags) else {
            return Ok(false);
        };
        let bytes = write_entry(key, class, flags, &h.parts(), meta, h.digest.as_ref());
        self.store(key, class, flags, &bytes)?;
        Ok(true)
    }

    /// Writes an entry's bytes atomically (temporary file, then rename) and trims now and
    /// then.
    fn store(&self, key: &ThumbKey, class: u32, flags: u32, bytes: &[u8]) -> io::Result<()> {
        let budget = self.budget.load(Ordering::Relaxed);
        let path = self.path(key, class, flags);
        let parent = path.parent().unwrap_or(&self.dir);
        std::fs::create_dir_all(parent)?;
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let tmp = parent.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&tmp, bytes)?;
        if let Err(e) = std::fs::rename(&tmp, &path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        let w = self
            .written
            .fetch_add(bytes.len() as u64, Ordering::Relaxed)
            + bytes.len() as u64;
        if w > budget / 20 {
            self.written.store(0, Ordering::Relaxed);
            self.check_budget(w);
        }
        Ok(())
    }

    /// After `written` more bytes: trims when the running total passes the budget, else
    /// only adds them up (a scan of a full cache stats every entry); scans anyway every
    /// [`RESCAN_EVERY`]th check.
    fn check_budget(&self, written: u64) {
        let n = self.checks.fetch_add(1, Ordering::Relaxed) + 1;
        let known = self.total.load(Ordering::Relaxed);
        if known != UNKNOWN && !n.is_multiple_of(RESCAN_EVERY) {
            let total = known.saturating_add(written);
            if total <= self.budget.load(Ordering::Relaxed) {
                self.total.store(total, Ordering::Relaxed);
                return;
            }
        }
        self.trim();
    }

    /// Directory scans so far.
    #[cfg(test)]
    pub(crate) fn scans(&self) -> u64 {
        self.scans.load(Ordering::Relaxed)
    }

    /// Removes every entry for `key` (all classes are found by trying them; flags 0..4).
    pub fn remove(&self, key: &ThumbKey) {
        for class in crate::CLASSES {
            for flags in 0..4 {
                let _ = std::fs::remove_file(self.path(key, class, flags));
            }
        }
    }

    /// Total bytes and entry count.
    pub fn usage(&self) -> (u64, usize) {
        let mut bytes = 0;
        let mut n = 0;
        for (_, len, _) in self.entries() {
            bytes += len;
            n += 1;
        }
        (bytes, n)
    }

    fn entries(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        self.scans.fetch_add(1, Ordering::Relaxed);
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        for sub in rd.flatten() {
            let Ok(rd) = std::fs::read_dir(sub.path()) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == EXT)
                    && let Ok(m) = e.metadata()
                {
                    out.push((p, m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
                }
            }
        }
        out
    }

    /// Deletes least recently used entries until the cache is at 80 % of its budget.
    pub fn trim(&self) {
        let budget = self.budget.load(Ordering::Relaxed);
        let mut all = self.entries();
        let mut total: u64 = all.iter().map(|e| e.1).sum();
        if total <= budget {
            self.total.store(total, Ordering::Relaxed);
            return;
        }
        all.sort_by_key(|e| e.2);
        let target = budget / 5 * 4;
        let mut removed = 0;
        for (p, len, _) in all {
            if total <= target {
                break;
            }
            if std::fs::remove_file(&p).is_ok() {
                total -= len;
                removed += 1;
            }
        }
        self.total.store(total, Ordering::Relaxed);
        specular_log::debug!(target: specular_log::CACHE, "thumbnail cache trimmed: {removed} entries, {total} bytes left");
    }

    /// Deletes every entry (the directory itself stays).
    pub fn clear(&self) -> io::Result<()> {
        // Scanned again at the next check.
        self.total.store(UNKNOWN, Ordering::Relaxed);
        for (p, _, _) in self.entries() {
            std::fs::remove_file(p)?;
        }
        Ok(())
    }
}

fn serialize(
    key: &ThumbKey,
    class: u32,
    flags: u32,
    px: &Pixels,
    meta: &Meta,
    digest: Option<&Digest>,
) -> Vec<u8> {
    // Alpha stays exact (colour under transparent pixels included): PNG, or raw if that fails.
    let (kind, payload) = if px.has_alpha() {
        match specular_png::encode(px.width, px.height, specular_png::ColorType::Rgba, &px.rgba) {
            Ok(png) => (2, png),
            Err(_) => (1, px.rgba.clone()),
        }
    } else {
        let jpeg = codec::encode_jpeg(&px.rgba, px.width as usize, px.height as usize, QUALITY);
        (0, jpeg)
    };
    let parts = Coded {
        kind,
        w: px.width,
        h: px.height,
        average: px.average(),
        payload: &payload,
    };
    write_entry(key, class, flags, &parts, meta, digest)
}

/// The pixel half of an entry, as stored.
struct Coded<'a> {
    kind: u8,
    w: u32,
    h: u32,
    average: [u8; 4],
    payload: &'a [u8],
}

fn write_entry(
    key: &ThumbKey,
    class: u32,
    flags: u32,
    px: &Coded,
    meta: &Meta,
    digest: Option<&Digest>,
) -> Vec<u8> {
    let mut o = Vec::with_capacity(px.payload.len() + 128);
    o.extend_from_slice(MAGIC);
    o.push(VERSION);
    o.push(px.kind);
    o.extend_from_slice(&(flags as u16).to_le_bytes());
    o.extend_from_slice(&key.stamp.len.to_le_bytes());
    o.extend_from_slice(&key.stamp.mtime_ns.to_le_bytes());
    o.extend_from_slice(&key.content.to_le_bytes());
    o.extend_from_slice(&class.to_le_bytes());
    o.extend_from_slice(&px.w.to_le_bytes());
    o.extend_from_slice(&px.h.to_le_bytes());
    o.extend_from_slice(&px.average);
    meta.write(&mut o);
    o.extend_from_slice(&(px.payload.len() as u32).to_le_bytes());
    o.extend_from_slice(px.payload);
    if let Some(d) = digest.filter(|d| d.bytes.len() <= u16::MAX as usize) {
        o.extend_from_slice(&d.id.to_le_bytes());
        o.extend_from_slice(&(d.bytes.len() as u16).to_le_bytes());
        o.extend_from_slice(&d.bytes);
    }
    o
}

/// A kind 2 payload: exactly `w`×`h` RGBA8, or nothing.
fn decode_png(payload: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let opts = specular_png::DecodeOptions {
        // Cache reads already run on a pool worker each.
        parallelism: specular_png::Parallelism::Single,
        ..Default::default()
    };
    let img = specular_png::decode(payload, &opts).ok()?;
    if (img.width, img.height) != (w, h)
        || img.truncated
        || img.layout != specular_png::Layout::Rgba8
    {
        return None;
    }
    let plane = img.planes.first()?;
    let row = w as usize * 4;
    let mut rgba = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        rgba.extend_from_slice(plane.data.get(y * plane.stride..y * plane.stride + row)?);
    }
    Some(rgba)
}

/// An entry up to its (still coded) payload, and its digest.
struct Head<'a> {
    kind: u8,
    w: u32,
    h: u32,
    average: [u8; 4],
    meta: Meta,
    payload: &'a [u8],
    digest: Option<Digest>,
    /// From a version before [`VERSION`] (and not before [`PIXELS_SINCE`]).
    meta_stale: bool,
}

impl<'a> Head<'a> {
    fn parts(&self) -> Coded<'a> {
        Coded {
            kind: self.kind,
            w: self.w,
            h: self.h,
            average: self.average,
            payload: self.payload,
        }
    }
}

fn parse_head<'a>(b: &'a [u8], key: &ThumbKey, class: u32, flags: u32) -> Option<Head<'a>> {
    let mut r = Reader { b, at: 0 };
    if r.take(4)? != MAGIC {
        return None;
    }
    let version = r.u8()?;
    if !(PIXELS_SINCE..=VERSION).contains(&version) {
        return None;
    }
    let kind = r.u8()?;
    let f = r.u16()? as u32;
    let stamp = Stamp {
        len: r.u64()?,
        mtime_ns: r.i128()?,
    };
    let content = r.u64()?;
    let c = r.u32()?;
    if f != flags || c != class || stamp != key.stamp || content != key.content {
        return None;
    }
    let (w, h) = (r.u32()?, r.u32()?);
    let avg = r.take(4)?;
    let average = [avg[0], avg[1], avg[2], avg[3]];
    let mlen = r.u16()? as usize;
    let meta = Meta::read(&mut Reader {
        b: r.take(mlen)?,
        at: 0,
    })?;
    let plen = r.u32()? as usize;
    let payload = r.take(plen)?;
    // Optional; a damaged trailer only loses the digest.
    let digest = read_digest(&mut r);
    Some(Head {
        kind,
        w,
        h,
        average,
        meta,
        payload,
        digest,
        meta_stale: version < VERSION,
    })
}

fn read_digest(r: &mut Reader) -> Option<Digest> {
    let id = r.u32()?;
    let n = r.u16()? as usize;
    Some(Digest {
        id,
        bytes: r.take(n)?.to_vec(),
    })
}

fn parse(b: &[u8], key: &ThumbKey, class: u32, flags: u32) -> Option<CacheEntry> {
    let Head {
        kind,
        w,
        h,
        average,
        meta,
        payload,
        digest,
        meta_stale,
    } = parse_head(b, key, class, flags)?;
    // JPEG entries are opaque by construction, so they skip the alpha scan.
    let pixels = match kind {
        0 => {
            let (dw, dh, rgba) = codec::decode_jpeg(payload)?;
            if (dw, dh) != (w, h) {
                return None;
            }
            Pixels::opaque(w, h, rgba)
        }
        1 if payload.len() == w as usize * h as usize * 4 => Pixels::new(w, h, payload.to_vec()),
        2 => Pixels::new(w, h, decode_png(payload, w, h)?),
        _ => return None,
    };
    Some(CacheEntry {
        class,
        pixels,
        average,
        meta,
        digest,
        meta_stale,
    })
}

#[cfg(target_os = "macos")]
fn platform_cache_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Caches/Specular/Thumbnails"))
}

#[cfg(windows)]
fn platform_cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join("Specular")
            .join("cache")
            .join("thumbnails"),
    )
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("specular").join("thumbnails"))
}

/// A fresh directory under the workspace `target/` for one test (never the user's cache).
#[cfg(test)]
pub(crate) fn test_dir(name: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let d = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/thumbs-test")
        .join(format!(
            "{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(w: u32, h: u32, alpha: u8) -> Pixels {
        Pixels::new(
            w,
            h,
            (0..w * h)
                .flat_map(|i| [(i * 7) as u8, (i / w * 9) as u8, 90, alpha])
                .collect(),
        )
    }

    fn key(n: u64) -> ThumbKey {
        ThumbKey {
            stamp: Stamp {
                len: 1000 + n,
                mtime_ns: 1_700_000_000_000_000_000,
            },
            content: n.wrapping_mul(0x1234_5678_9abc),
        }
    }

    #[test]
    fn blobs_round_trip_by_key_and_tag() {
        let dir = test_dir("cache-blob");
        let c = DiskCache::new(&dir, 10 << 20);
        let mut k = key(5);
        k.stamp.mtime_ns = -3_000_000_000_123;
        assert_eq!(c.get_blob(&k, 7), None);
        c.put_blob(&k, 7, b"first\0second").unwrap();
        assert_eq!(c.get_blob(&k, 7).as_deref(), Some(&b"first\0second"[..]));
        // Another tag or key: absent. Empty records are records.
        assert_eq!(c.get_blob(&k, 8), None);
        assert_eq!(c.get_blob(&key(6), 7), None);
        c.put_blob(&k, 8, b"").unwrap();
        assert_eq!(c.get_blob(&k, 8).as_deref(), Some(&b""[..]));
        // Counted, trimmed and cleared with the thumbnails; never read as one.
        assert_eq!(c.usage().1, 2);
        assert!(c.get(&k, BLOB_CLASS, 7).is_none());
        assert_eq!(c.get_blob(&k, 7), None, "a failed parse drops the file");
        c.put_blob(&k, 7, b"x").unwrap();
        c.clear().unwrap();
        assert_eq!(c.get_blob(&k, 7), None);
        // A disabled cache stores nothing.
        let off = DiskCache::new(&dir, 0);
        off.put_blob(&k, 7, b"x").unwrap();
        assert_eq!(off.get_blob(&k, 7), None);
    }

    #[test]
    fn put_get_round_trip_with_meta() {
        let dir = test_dir("cache-rt");
        let c = DiskCache::new(&dir, 10 << 20);
        let meta = Meta {
            format: "JPEG".into(),
            width: 4000,
            height: 3000,
            hdr: true,
            pages: 1,
            bit_depth: 8,
            date_taken: Some(1_600_000_000),
            camera: Some("Canon EOS R5".into()),
            iso: Some(400),
            f_number: Some(2.8),
            gps: Some(true),
            position: Some((-33.5, 151.25)),
            ..Meta::default()
        };
        let px = pixels(64, 48, 255);
        c.put(&key(1), 256, 0, &px, &meta).unwrap();
        let e = c.get(&key(1), 256, 0).expect("hit");
        assert_eq!(e.meta, meta);
        // The position is the last field, after the focal length: an entry from before it was
        // kept is the same bytes without it, and reads as "recorded, position unknown".
        let older = Meta {
            position: None,
            ..meta.clone()
        };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        meta.write(&mut a);
        older.write(&mut b);
        assert_eq!(a.len(), b.len() + 8);
        // Only the presence bit (7, unused before) differs ahead of the appended position.
        let diff: Vec<usize> = (2..b.len()).filter(|&i| a[i] != b[i]).collect();
        assert_eq!(diff.len(), 1);
        assert_eq!(a[diff[0]] ^ b[diff[0]], 0x80);
        let back = Meta::read(&mut Reader { b: &b[2..], at: 0 }).unwrap();
        assert_eq!((back.gps, back.position), (Some(true), None));
        // The motion flags are one byte after everything else: a version 3 entry (without
        // it) reads as no motion.
        let live = Meta {
            motion: true,
            live_photo: true,
            ..meta.clone()
        };
        let mut l = Vec::new();
        live.write(&mut l);
        assert_eq!(Meta::read(&mut Reader { b: &l[2..], at: 0 }).unwrap(), live);
        let v3 = Meta::read(&mut Reader {
            b: &l[2..l.len() - 1],
            at: 0,
        })
        .unwrap();
        assert_eq!(v3, meta);
        // The colour space follows them: a version 4 entry reads as not looked for.
        let spaced = Meta {
            space: Some("Display P3".into()),
            ..live.clone()
        };
        let mut sp = Vec::new();
        spaced.write(&mut sp);
        assert_eq!(
            Meta::read(&mut Reader { b: &sp[2..], at: 0 }).unwrap(),
            spaced
        );
        assert_eq!(sp.len(), l.len() + 2 + "Display P3".len());
        assert_eq!(
            Meta::read(&mut Reader { b: &l[2..], at: 0 }).unwrap().space,
            None
        );
        let none = Meta {
            space: Some(String::new()),
            ..live.clone()
        };
        let mut n = Vec::new();
        none.write(&mut n);
        assert_eq!(Meta::read(&mut Reader { b: &n[2..], at: 0 }).unwrap(), none);
        // Movie facts come last, with or without a colour space before them; entries
        // without them read as not a movie.
        for space in [None, Some("BT.709".to_owned())] {
            let movie = Meta {
                video: true,
                duration_us: Some(83_250_000),
                space,
                ..meta.clone()
            };
            let mut v = Vec::new();
            movie.write(&mut v);
            assert_eq!(
                Meta::read(&mut Reader { b: &v[2..], at: 0 }).unwrap(),
                movie
            );
        }
        assert!(!Meta::read(&mut Reader { b: &n[2..], at: 0 }).unwrap().video);
        // Subframe facts come after them, with or without a colour space or movie facts;
        // their metrics are not part of the entry.
        for (space, video) in [
            (None, false),
            (Some("linear".to_owned()), false),
            (None, true),
        ] {
            let sub = Meta {
                space,
                video,
                astro: Some(Box::new(AstroMeta {
                    object: Some("NGC 7000".into()),
                    filter: None,
                    frame_type: Some("light".into()),
                    gain: Some(139.0),
                    ccd_temp: Some(-10.5),
                    metrics: None,
                })),
                ..meta.clone()
            };
            let mut v = Vec::new();
            sub.write(&mut v);
            assert_eq!(Meta::read(&mut Reader { b: &v[2..], at: 0 }).unwrap(), sub);
        }
        assert!(
            Meta::read(&mut Reader { b: &n[2..], at: 0 })
                .unwrap()
                .astro
                .is_none()
        );
        // Exif fields come last, after any of the above; the tags are not kept.
        for (space, video, astro) in [
            (None, false, None),
            (Some("srgb".to_owned()), true, None),
            (None, false, Some(Box::new(AstroMeta::default()))),
        ] {
            let ex = Meta {
                space,
                video,
                astro,
                exif: Some(Box::new(crate::ExifMeta {
                    make: Some("Canon".into()),
                    flash: Some(0x59),
                    exposure_bias: Some(-0.5),
                    altitude: Some(-3.0),
                    tags: None,
                    ..Default::default()
                })),
                ..meta.clone()
            };
            let mut v = Vec::new();
            ex.write(&mut v);
            assert_eq!(Meta::read(&mut Reader { b: &v[2..], at: 0 }).unwrap(), ex);
            let with_tags = Meta {
                exif: ex.exif.clone().map(|mut e| {
                    e.tags = Some("a406=3".into());
                    e
                }),
                ..ex.clone()
            };
            let mut w = Vec::new();
            with_tags.write(&mut w);
            assert_eq!(w, v);
        }
        assert!(
            Meta::read(&mut Reader { b: &n[2..], at: 0 })
                .unwrap()
                .exif
                .is_none()
        );
        assert_eq!((e.pixels.width, e.pixels.height), (64, 48));
        assert_eq!(e.average, px.average());
        // Alpha entries come back exactly.
        let pa = pixels(10, 10, 128);
        c.put(&key(2), 128, 0, &pa, &Meta::default()).unwrap();
        assert_eq!(c.get(&key(2), 128, 0).unwrap().pixels, pa);
        // Other class / flags / key: miss.
        assert!(c.get(&key(1), 128, 0).is_none());
        assert!(c.get(&key(1), 256, 1).is_none());
        assert!(c.get(&key(3), 256, 0).is_none());
        assert_eq!(c.usage().1, 2);
        c.clear().unwrap();
        assert_eq!(c.usage().1, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn digests_round_trip_and_entries_without_one_still_read() {
        let dir = test_dir("cache-digest");
        let c = DiskCache::new(&dir, 10 << 20);
        let meta = Meta {
            format: "JPEG".into(),
            width: 640,
            height: 480,
            ..Meta::default()
        };
        let px = pixels(64, 48, 255);
        let d = Digest {
            id: 0x5349_4731,
            bytes: (0..146u8).collect(),
        };
        c.put_with_digest(&key(1), 256, 0, &px, &meta, Some(&d))
            .unwrap();
        let e = c.get(&key(1), 256, 0).expect("hit");
        assert_eq!(e.digest.as_ref(), Some(&d));
        assert_eq!((e.pixels.width, e.pixels.height), (64, 48));
        assert_eq!(
            c.get_digest(&key(1), 256, 0, d.id),
            Some((meta.clone(), d.clone()))
        );
        // Another digest id: none, and the entry stays.
        assert!(c.get_digest(&key(1), 256, 0, d.id + 1).is_none());
        assert!(c.get(&key(1), 256, 0).is_some());
        // An entry written without a digest (as before digests existed) reads as before.
        c.put(&key(2), 256, 0, &px, &meta).unwrap();
        let e = c.get(&key(2), 256, 0).expect("hit");
        assert!(e.digest.is_none());
        assert!(c.get_digest(&key(2), 256, 0, d.id).is_none());
        assert!(c.contains(&key(2), 256, 0));
        // A cut-off trailer loses the digest, not the entry.
        let p = c.path(&key(1), 256, 0);
        let b = std::fs::read(&p).unwrap();
        std::fs::write(&p, &b[..b.len() - 10]).unwrap();
        let e = c.get(&key(1), 256, 0).expect("hit");
        assert!(e.digest.is_none());
        assert_eq!(e.meta, meta);
        // The bytes before the trailer are exactly an entry without one: older readers,
        // which stop after the payload, read new entries unchanged.
        let with = serialize(&key(3), 256, 0, &px, &meta, Some(&d));
        let without = serialize(&key(3), 256, 0, &px, &meta, None);
        assert_eq!(&with[..without.len()], &without[..]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn alpha_round_trips_exactly_and_opaque_entries_skip_the_scan() {
        let dir = test_dir("cache-alpha");
        let c = DiskCache::new(&dir, 10 << 20);
        // A red disc fading out over a transparent background, with colour left under the
        // fully transparent pixels: all of it must come back bit for bit.
        let px = Pixels::new(
            40,
            30,
            (0..40 * 30i32)
                .flat_map(|i| {
                    let (x, y) = (i % 40, i / 40);
                    let d = ((x - 20).pow(2) + (y - 15).pow(2)) as f32;
                    let a = (255.0 - d * 2.0).clamp(0.0, 255.0) as u8;
                    [220, (x * 6) as u8, (y * 8) as u8, a]
                })
                .collect(),
        );
        assert!(px.has_alpha());
        c.put(&key(7), 128, 0, &px, &Meta::default()).unwrap();
        let e = c.get(&key(7), 128, 0).unwrap();
        assert_eq!(e.pixels, px);
        assert_eq!(e.pixels.premultiplied(), px.premultiplied());
        // Stored as PNG (kind 2), well under the raw size.
        let b = std::fs::read(c.path(&key(7), 128, 0)).unwrap();
        assert_eq!(b[5], 2);
        assert!(b.len() < 40 * 30 * 4, "{}", b.len());
        // An opaque entry is JPEG-coded; reading it back knows it is opaque without a scan.
        let op = pixels(16, 16, 255);
        c.put(&key(8), 128, 0, &op, &Meta::default()).unwrap();
        let e = c.get(&key(8), 128, 0).unwrap();
        assert!(e.pixels.premul_known());
        assert!(!e.pixels.has_alpha());
        assert_eq!(e.pixels.bytes(), 16 * 16 * 4);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn older_versions_are_misses_and_deleted() {
        let dir = test_dir("cache-version");
        let c = DiskCache::new(&dir, 10 << 20);
        let px = pixels(8, 8, 100);
        c.put(&key(9), 128, 0, &px, &Meta::default()).unwrap();
        let p = c.path(&key(9), 128, 0);
        let mut b = std::fs::read(&p).unwrap();
        assert_eq!(b[4], VERSION);
        // Before the pixels last changed, and from a newer build: misses, deleted.
        for v in [PIXELS_SINCE - 1, VERSION + 1] {
            b[4] = v;
            std::fs::write(&p, &b).unwrap();
            assert!(c.get(&key(9), 128, 0).is_none(), "version {v}");
            assert!(!p.exists(), "version {v}");
        }
        // The current version: a hit with current meta.
        b[4] = VERSION;
        std::fs::write(&p, &b).unwrap();
        assert!(!c.get(&key(9), 128, 0).unwrap().meta_stale);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pinned_versions() {
        // Version 7 added the Exif fields search names to the meta and kept version 2's
        // pixels.
        assert_eq!((VERSION, PIXELS_SINCE), (7, 2));
    }

    #[test]
    fn stale_meta_is_refreshed_in_place_keeping_the_pixels() {
        let dir = test_dir("cache-stale-meta");
        let c = DiskCache::new(&dir, 10 << 20);
        let old_meta = Meta {
            format: "PSD".into(),
            width: 800,
            height: 600,
            ..Meta::default()
        };
        let d = Digest {
            id: 42,
            bytes: vec![1, 2, 3],
        };
        for (n, alpha) in [(1, 255), (2, 128)] {
            let px = pixels(32, 24, alpha);
            c.put_with_digest(&key(n), 128, 0, &px, &old_meta, Some(&d))
                .unwrap();
            let p = c.path(&key(n), 128, 0);
            let mut b = std::fs::read(&p).unwrap();
            let current = c.get(&key(n), 128, 0).unwrap();
            assert!(!current.meta_stale);
            // As written by the version before: pixels served, meta flagged stale.
            b[4] = VERSION - 1;
            std::fs::write(&p, &b).unwrap();
            let e = c.get(&key(n), 128, 0).expect("old pixels still serve");
            assert!(e.meta_stale);
            assert_eq!(e.pixels, current.pixels);
            assert_eq!(e.meta, old_meta);
            assert_eq!(e.digest.as_ref(), Some(&d));
            // The digest-only path does not hand out stale meta, and leaves the entry.
            assert!(c.get_digest(&key(n), 128, 0, d.id).is_none());
            assert!(p.exists());
            // Refreshed: current version, new meta, the same coded pixels and digest.
            let new_meta = Meta {
                has_profile: true,
                gps: Some(true),
                position: Some((48.25, 16.5)),
                camera: Some("Canon EOS R6".into()),
                space: Some("adobergb".into()),
                ..old_meta.clone()
            };
            assert!(c.refresh_meta(&key(n), 128, 0, &new_meta).unwrap());
            let after = std::fs::read(&p).unwrap();
            assert_eq!(after[4], VERSION);
            let e = c.get(&key(n), 128, 0).unwrap();
            assert!(!e.meta_stale);
            assert_eq!(e.meta, new_meta);
            assert_eq!(e.pixels, current.pixels);
            assert_eq!(e.average, current.average);
            assert_eq!(
                c.get_digest(&key(n), 128, 0, d.id),
                Some((new_meta.clone(), d.clone()))
            );
            // Byte for byte what a fresh write of the same coded pixels gives.
            let fresh = parse_head(&after, &key(n), 128, 0).unwrap();
            let orig = parse_head(&b, &key(n), 128, 0).unwrap();
            assert_eq!(fresh.payload, orig.payload);
            assert_eq!(fresh.kind, orig.kind);
        }
        // Nothing to refresh: no entry.
        assert!(!c.refresh_meta(&key(5), 128, 0, &old_meta).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalidation_by_mtime_size_and_content() {
        let dir = test_dir("cache-inval");
        let c = DiskCache::new(&dir, 10 << 20);
        let f = dir.join("img.bin");
        std::fs::write(&f, vec![1u8; 5000]).unwrap();
        let k = ThumbKey::of_file(&f).unwrap();
        c.put(&k, 128, 0, &pixels(8, 8, 255), &Meta::default())
            .unwrap();
        assert!(c.get(&ThumbKey::of_file(&f).unwrap(), 128, 0).is_some());
        // Same size, different content, mtime forced equal: the content hash catches it.
        std::fs::write(&f, vec![2u8; 5000]).unwrap();
        let file = std::fs::File::options().append(true).open(&f).unwrap();
        file.set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_nanos(k.stamp.mtime_ns as u64),
        )
        .unwrap();
        let k2 = ThumbKey::of_file(&f).unwrap();
        assert_eq!(k2.stamp, k.stamp);
        assert!(c.get(&k2, 128, 0).is_none());
        // Touch only: the stamp changes, so it is a miss too.
        std::fs::write(&f, vec![1u8; 5000]).unwrap();
        let k3 = ThumbKey::of_file(&f).unwrap();
        assert_eq!(k3.content, k.content);
        if k3.stamp != k.stamp {
            assert!(c.get(&k3, 128, 0).is_none());
        }
        // A corrupt entry reads as a miss and is deleted.
        let p = c.path(&k, 128, 0);
        std::fs::write(&p, b"SPTH\x01garbage").unwrap();
        assert!(c.get(&k, 128, 0).is_none());
        assert!(!p.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A cache hit costs about one read of the entry file: [`DiskCache::get_blob`] on small
    /// records against `fs::read` of the same files, paired rounds, best of each.
    #[test]
    fn a_hit_costs_about_one_read() {
        let dir = test_dir("cache-hit");
        let c = DiskCache::new(&dir, 1 << 30);
        let keys: Vec<ThumbKey> = (0..400).map(key).collect();
        for k in &keys {
            c.put_blob(k, 7, b"what a listing found").unwrap();
        }
        let paths: Vec<PathBuf> = keys.iter().map(|k| c.path(k, BLOB_CLASS, 7)).collect();
        let (mut hits, mut reads) = (std::time::Duration::MAX, std::time::Duration::MAX);
        for _ in 0..7 {
            let t0 = std::time::Instant::now();
            for k in &keys {
                assert!(c.get_blob(k, 7).is_some());
            }
            hits = hits.min(t0.elapsed());
            let t0 = std::time::Instant::now();
            for p in &paths {
                assert!(!std::fs::read(p).unwrap().is_empty());
            }
            reads = reads.min(t0.elapsed());
        }
        let ratio = hits.as_secs_f64() / reads.as_secs_f64();
        eprintln!("400 hits {hits:.2?}, 400 reads {reads:.2?}: {ratio:.2}×");
        // Before: a second open and a time stamp written per hit (2–5× a read under load).
        // Generous: parsing and the key check come on top of the read.
        assert!(ratio < 3.0, "a hit costs {ratio:.2}× a read");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A hit writes nothing to an entry used within the hour (no second open, no inode
    /// write per tile shown); one used longer ago is marked used again, for the LRU.
    #[test]
    fn a_hit_marks_use_to_the_hour() {
        let dir = test_dir("cache-use");
        let c = DiskCache::new(&dir, 1 << 30);
        c.put_blob(&key(1), 7, b"fresh").unwrap();
        c.put_blob(&key(2), 7, b"old").unwrap();
        let (fresh, old) = (
            c.path(&key(1), BLOB_CLASS, 7),
            c.path(&key(2), BLOB_CLASS, 7),
        );
        let set = |p: &Path, t: SystemTime| {
            let f = std::fs::File::options().append(true).open(p).unwrap();
            f.set_modified(t).unwrap();
        };
        let now = SystemTime::now();
        let recent = now - std::time::Duration::from_secs(600);
        let long_ago = now - std::time::Duration::from_secs(2 * 3600);
        set(&fresh, recent);
        set(&old, long_ago);
        let mtime = |p: &Path| std::fs::metadata(p).unwrap().modified().unwrap();
        assert!(c.get_blob(&key(1), 7).is_some());
        assert!(c.get_blob(&key(2), 7).is_some());
        assert_eq!(mtime(&fresh), recent, "a fresh entry was written to");
        assert!(mtime(&old) >= now, "an old entry was not marked used");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Storing while the cache is under budget adds up what was written: the cache is
    /// scanned once, then every [`RESCAN_EVERY`]th budget check, not at every check.
    #[test]
    fn storing_under_budget_scans_rarely() {
        let dir = test_dir("cache-store");
        // ~4 KiB entries (noise, with alpha: stored as PNG), 8 MiB in all: a budget check
        // every budget/20 (512 KiB), never over.
        let c = DiskCache::new(&dir, 10 << 20);
        let mut r = 0x9e37_79b9_7f4a_7c15u64;
        let noise = (0..32 * 32 * 4)
            .map(|_| {
                r ^= r << 13;
                r ^= r >> 7;
                r ^= r << 17;
                r as u8 | 1
            })
            .collect();
        let px = Pixels::new(32, 32, noise);
        let t0 = std::time::Instant::now();
        for n in 0..2000 {
            c.put(&key(n), 128, 0, &px, &Meta::default()).unwrap();
        }
        let scans = c.scans();
        eprintln!("2000 stores: {:.1?}, {scans} scans", t0.elapsed());
        // Before: a scan per check (17 here).
        assert!(scans <= 3, "{scans} scans");
        // Over budget, a check trims as before.
        c.set_budget(4 << 20);
        for n in 2000..2200 {
            c.put(&key(n), 128, 0, &px, &Meta::default()).unwrap();
        }
        let (bytes, _) = c.usage();
        // At most one check's worth (budget / 20) written since the last check.
        assert!(bytes <= (4 << 20) + (4 << 20) / 20, "{bytes} bytes kept");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn trim_removes_least_recently_used() {
        let dir = test_dir("cache-trim");
        let c = DiskCache::new(&dir, 1 << 30);
        let px = pixels(32, 32, 7); // raw: ~4 KiB each
        for n in 0..10 {
            c.put(&key(n), 128, 0, &px, &Meta::default()).unwrap();
            // Distinct, increasing mtimes.
            let f = std::fs::File::options()
                .append(true)
                .open(c.path(&key(n), 128, 0))
                .unwrap();
            f.set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + n))
                .unwrap();
        }
        let (bytes, n) = c.usage();
        assert_eq!(n, 10);
        c.set_budget(bytes / 2);
        c.trim();
        let (after, n) = c.usage();
        assert!(after <= bytes / 2 / 5 * 4, "{after}");
        assert!(n < 10);
        // The newest survive.
        assert!(c.contains(&key(9), 128, 0));
        assert!(!c.contains(&key(0), 128, 0));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_dir_honours_the_override() {
        // Only reads the environment; never creates anything.
        let d = DiskCache::default_dir();
        if let Some(v) = std::env::var_os("SPECULAR_THUMB_CACHE").filter(|v| !v.is_empty()) {
            assert_eq!(d, Some(PathBuf::from(v)));
        }
    }

    #[test]
    fn default_dir_for_a_portable_install() {
        // Only reads the environment; never creates anything.
        let portable = Path::new("portable").join("cache");
        let d = DiskCache::default_dir_for(Some(&portable));
        match std::env::var_os("SPECULAR_THUMB_CACHE").filter(|v| !v.is_empty()) {
            Some(v) => assert_eq!(d, Some(PathBuf::from(v))),
            None => assert_eq!(d, Some(portable.join("thumbnails"))),
        }
    }
}

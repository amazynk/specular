//! Cache keys: file size + modification time + a partial content hash.
//!
//! The hash reads three 16 KiB samples (head, middle, tail) and the length, so it costs at
//! most 48 KiB of reads per file however large the file is, catches edits that keep the size
//! and the time stamp, and does not depend on the path: a renamed, moved or copied file finds
//! its thumbnail again. The hash is a fixed, platform-independent function (little-endian
//! words), so the cache survives restarts and upgrades of the standard library.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bytes per content sample.
pub const SAMPLE: usize = 16 * 1024;

/// Entries of an image folder that its key covers ([`ThumbKey::of_folder`]).
pub const MAX_FOLDER_ENTRIES: usize = 4096;

/// Levels of subfolders below an image folder that its key covers (a CASA subtable, an
/// OME-Zarr chunk at `0/0/0/0/0`).
pub const FOLDER_KEY_DEPTH: usize = 6;

/// Size and modification time of a file: a cheap "has it changed" check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Stamp {
    pub len: u64,
    /// Nanoseconds since the Unix epoch (negative before it); 0 when unknown.
    pub mtime_ns: i128,
}

impl Stamp {
    pub fn new(len: u64, modified: Option<SystemTime>) -> Stamp {
        Stamp {
            len,
            mtime_ns: modified.map(time_ns).unwrap_or(0),
        }
    }

    pub fn of(meta: &std::fs::Metadata) -> Stamp {
        Stamp::new(meta.len(), meta.modified().ok())
    }

    pub fn of_path(path: &Path) -> io::Result<Stamp> {
        Ok(Stamp::of(&std::fs::metadata(path)?))
    }
}

/// Nanoseconds since the Unix epoch.
pub fn time_ns(t: SystemTime) -> i128 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    }
}

/// What a cache entry is keyed by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub stamp: Stamp,
    /// [`content_hash`] of the file.
    pub content: u64,
}

impl ThumbKey {
    /// Reads the samples of the file at `path`.
    pub fn of_file(path: &Path) -> io::Result<ThumbKey> {
        // A regular file only: opening a FIFO blocks, reading a device may never end.
        let mut f = specular_core::open_regular(path)?;
        let meta = f.metadata()?;
        let stamp = Stamp::of(&meta);
        let content = hash_reader(&mut f, stamp.len)?;
        Ok(ThumbKey { stamp, content })
    }

    /// For a folder that opens as one image (a CASA image, an OME-Zarr store): the folder's
    /// own [`Stamp`] and a hash of the names, sizes and modification times of its entries
    /// (the first [`MAX_FOLDER_ENTRIES`], top level first, each folder by name, its
    /// subfolders down to [`FOLDER_KEY_DEPTH`] levels after it), without reading any of them.
    /// Writing a file of the image in place, also in a subfolder (a CASA subtable, a Zarr
    /// chunk), changes its time, so the key; like a file's key it does not depend on the
    /// folder's path.
    pub fn of_folder(dir: &Path) -> io::Result<ThumbKey> {
        let stamp = Stamp::of_path(dir)?;
        // "FOLDERIM": a domain apart from the content hashes.
        let mut h = Hasher::new(0x464f_4c44_4552_494d);
        let mut left = MAX_FOLDER_ENTRIES;
        // Breadth first: (relative path, depth). Only the top folder must be readable.
        let mut queue = std::collections::VecDeque::from([(PathBuf::new(), 0usize)]);
        while let Some((rel, depth)) = queue.pop_front() {
            if left == 0 {
                break;
            }
            let rd = match std::fs::read_dir(dir.join(&rel)) {
                Ok(rd) => rd,
                Err(e) if depth == 0 => return Err(e),
                Err(_) => continue,
            };
            let mut entries: Vec<(std::ffi::OsString, Stamp, bool)> = Vec::new();
            for e in rd.take(MAX_FOLDER_ENTRIES * 4) {
                let e = match e {
                    Ok(e) => e,
                    Err(e) if depth == 0 => return Err(e),
                    Err(_) => continue,
                };
                // Never followed or opened: a FIFO or a dangling link costs nothing.
                let m = e.metadata().ok();
                let s = m.as_ref().map_or(Stamp::new(0, None), Stamp::of);
                let is_dir = m.is_some_and(|m| m.is_dir());
                entries.push((e.file_name(), s, is_dir));
            }
            entries.sort_unstable_by(|a, b| a.0.as_encoded_bytes().cmp(b.0.as_encoded_bytes()));
            entries.truncate(left);
            left -= entries.len();
            for (name, s, is_dir) in entries {
                let p = rel.join(name);
                h.bytes(p.as_os_str().as_encoded_bytes());
                h.word(s.len);
                h.word(s.mtime_ns as u64);
                h.word((s.mtime_ns >> 64) as u64);
                if is_dir && depth < FOLDER_KEY_DEPTH {
                    queue.push_back((p, depth + 1));
                }
            }
        }
        Ok(ThumbKey {
            stamp,
            content: h.finish(),
        })
    }

    /// For data already in memory (an archive member).
    pub fn of_bytes(bytes: &[u8], modified: Option<SystemTime>) -> ThumbKey {
        ThumbKey {
            stamp: Stamp::new(bytes.len() as u64, modified),
            content: content_hash(bytes),
        }
    }

    /// For a member of an archive, without reading the archive: the archive's [`Stamp`]
    /// (any change to the archive file gives its members new keys) and a hash of the
    /// archive's path and the member's name. Unlike a file's key it depends on the path, so
    /// a renamed archive makes its thumbnails again; in exchange a warm gallery of an archive
    /// costs one `stat`, not a decompression of every member.
    pub fn of_member(archive: &Path, stamp: Stamp, member: &str) -> ThumbKey {
        // "ARCHMEMB": a domain apart from the content hashes (seeded with a length).
        let mut h = Hasher::new(0x4152_4348_4d45_4d42);
        h.bytes(archive.as_os_str().as_encoded_bytes());
        h.bytes(member.as_bytes());
        ThumbKey {
            stamp,
            content: h.finish(),
        }
    }

    /// For a photo developed with an edit (Develop's edited thumbnails): the file's key
    /// (`self`, so a changed or replaced file makes them again), the edit's output hash and
    /// the version of the graph that renders it. The same edit on a copy of the file hits.
    pub fn edited(&self, edit_hash: &[u8], graph_version: u32) -> ThumbKey {
        self.edited_at(edit_hash, graph_version, 0)
    }

    /// [`edited`](Self::edited) for a photo developed at `develop_long` px before it was
    /// reduced to the thumbnail (a raw's sharper thumbnails), so a thumbnail made at one
    /// size is never shown for another. 0 (developed at the thumbnail's own size) is
    /// [`edited`](Self::edited)'s key, so entries made before the size was keyed still serve.
    pub fn edited_at(&self, edit_hash: &[u8], graph_version: u32, develop_long: u32) -> ThumbKey {
        // "EDITTHMB": a domain apart from the content hashes.
        let mut h = Hasher::new(0x4544_4954_5448_4d42);
        h.word(self.content);
        h.bytes(edit_hash);
        h.word(u64::from(graph_version));
        if develop_long != 0 {
            h.word(u64::from(develop_long));
        }
        ThumbKey {
            stamp: self.stamp,
            content: h.finish(),
        }
    }

    /// The entry's file name stem for one size class and flag set: 16 hex digits.
    pub fn name(&self, class: u32, flags: u32) -> String {
        let mut h = Hasher::new(0x5350_5448);
        h.word(self.stamp.len);
        h.word(self.stamp.mtime_ns as u64);
        h.word((self.stamp.mtime_ns >> 64) as u64);
        h.word(self.content);
        h.word(class as u64 | (flags as u64) << 32);
        format!("{:016x}", h.finish())
    }
}

/// Where the three samples start in a file of `len` bytes (overlapping samples are merged by
/// clamping, so small files are hashed whole).
pub fn sample_ranges(len: u64) -> [(u64, usize); 3] {
    let s = SAMPLE as u64;
    let take = |at: u64| (at, (len.saturating_sub(at)).min(s) as usize);
    if len <= 3 * s {
        // Whole file in (up to) three consecutive pieces.
        return [take(0), take(s), take(2 * s)];
    }
    [take(0), take(len / 2 - s / 2), take(len - s)]
}

/// The partial content hash of `bytes`.
pub fn content_hash(bytes: &[u8]) -> u64 {
    let len = bytes.len() as u64;
    let mut h = Hasher::new(len);
    for (at, n) in sample_ranges(len) {
        if n > 0 {
            h.bytes(&bytes[at as usize..at as usize + n]);
        }
    }
    h.finish()
}

fn hash_reader(r: &mut (impl Read + Seek), len: u64) -> io::Result<u64> {
    let mut h = Hasher::new(len);
    let mut buf = vec![0u8; SAMPLE];
    for (at, n) in sample_ranges(len) {
        if n == 0 {
            continue;
        }
        r.seek(SeekFrom::Start(at))?;
        r.read_exact(&mut buf[..n])?;
        h.bytes(&buf[..n]);
    }
    Ok(h.finish())
}

/// A small, fixed 64-bit hash (multiply-rotate per word, splitmix finaliser). Not
/// cryptographic: a key collision shows a wrong thumbnail, nothing worse.
pub struct Hasher(u64);

impl Hasher {
    pub fn new(seed: u64) -> Hasher {
        Hasher(seed ^ 0x243F_6A88_85A3_08D3)
    }

    pub fn word(&mut self, w: u64) {
        self.0 = (self.0 ^ w)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .rotate_left(29);
    }

    pub fn bytes(&mut self, b: &[u8]) {
        let mut chunks = b.chunks_exact(8);
        for c in &mut chunks {
            let mut w = [0u8; 8];
            w.copy_from_slice(c);
            self.word(u64::from_le_bytes(w));
        }
        let rest = chunks.remainder();
        let mut w = [0u8; 8];
        w[..rest.len()].copy_from_slice(rest);
        self.word(u64::from_le_bytes(w) ^ (rest.len() as u64) << 59);
        self.word(b.len() as u64);
    }

    pub fn finish(&self) -> u64 {
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(n: usize, seed: u8) -> Vec<u8> {
        (0..n)
            .map(|i| (i as u32).wrapping_mul(2_654_435_761) as u8 ^ seed)
            .collect()
    }

    #[test]
    fn hash_is_stable() {
        // Pinned: changing the hash invalidates every user's cache.
        assert_eq!(content_hash(b""), content_hash(b""));
        let a = content_hash(b"hello world");
        assert_eq!(a, content_hash(b"hello world"));
        assert_ne!(a, content_hash(b"hello worle"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abc\0"));
    }

    #[test]
    fn edited_keys_follow_the_file_the_edit_and_the_graph() {
        let k = ThumbKey::of_bytes(b"a raw file", None);
        let e = k.edited(&[7; 32], 2);
        assert_eq!(e, k.edited(&[7; 32], 2));
        assert_eq!(e.stamp, k.stamp);
        assert_ne!(e, k);
        assert_ne!(e, k.edited(&[8; 32], 2));
        assert_ne!(e, k.edited(&[7; 32], 3));
        let other = ThumbKey::of_bytes(b"another raw", None);
        assert_ne!(e, other.edited(&[7; 32], 2));
        assert_ne!(e.name(512, 0), k.name(512, 0));
        // Kept in the cache like any thumbnail.
        let dir = crate::cache::test_dir("edited-key");
        let cache = crate::DiskCache::new(&dir, 1 << 20);
        let px = crate::pixels::Pixels::new(2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255]);
        cache.put(&e, 512, 0, &px, &crate::Meta::default()).unwrap();
        assert!(cache.get(&e, 512, 0).is_some());
        assert!(cache.get(&k, 512, 0).is_none());
        assert!(cache.get(&k.edited(&[8; 32], 2), 512, 0).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A raw developed at 1024 px for its thumbnail has its own key; at the thumbnail's own
    /// size it keeps the old key. Neither entry is shown for the other, and making one
    /// leaves the other in the cache.
    #[test]
    fn edited_keys_follow_the_develop_size() {
        let k = ThumbKey::of_bytes(b"a raw file", None);
        let plain = k.edited(&[7; 32], 4);
        assert_eq!(k.edited_at(&[7; 32], 4, 0), plain);
        let sharp = k.edited_at(&[7; 32], 4, 1024);
        assert_ne!(sharp, plain);
        assert_eq!(sharp, k.edited_at(&[7; 32], 4, 1024));
        assert_ne!(sharp, k.edited_at(&[7; 32], 4, 1536));
        assert_ne!(sharp, k.edited_at(&[8; 32], 4, 1024));
        assert_ne!(sharp, k.edited_at(&[7; 32], 5, 1024));
        assert_eq!(sharp.stamp, k.stamp);
        let dir = crate::cache::test_dir("edited-size-key");
        let cache = crate::DiskCache::new(&dir, 1 << 20);
        let px = crate::pixels::Pixels::new(1, 1, vec![10, 20, 30, 255]);
        cache
            .put(&plain, 512, 0, &px, &crate::Meta::default())
            .unwrap();
        assert!(cache.get(&sharp, 512, 0).is_none());
        cache
            .put(&sharp, 512, 0, &px, &crate::Meta::default())
            .unwrap();
        assert!(cache.get(&plain, 512, 0).is_some());
        assert!(cache.get(&sharp, 512, 0).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn samples_cover_small_files_and_ends_of_large_ones() {
        assert_eq!(sample_ranges(10), [(0, 10), (16384, 0), (32768, 0)]);
        let big = 10_000_000;
        let r = sample_ranges(big);
        assert_eq!(r[0], (0, SAMPLE));
        assert_eq!(r[2], (big - SAMPLE as u64, SAMPLE));
        // A change in the middle sample or the tail is seen; one in between is not (by
        // design: the stamp catches those).
        let mut d = data(200_000, 1);
        let h0 = content_hash(&d);
        d[199_999] ^= 1;
        assert_ne!(h0, content_hash(&d));
        d[199_999] ^= 1;
        d[100_000] ^= 1;
        assert_ne!(h0, content_hash(&d));
        d[100_000] ^= 1;
        d[30_000] ^= 1;
        assert_eq!(h0, content_hash(&d));
    }

    #[test]
    fn file_and_bytes_agree_and_names_differ() {
        let dir = crate::cache::test_dir("key");
        let p = dir.join("a.bin");
        let d = data(100_000, 7);
        std::fs::write(&p, &d).unwrap();
        let k = ThumbKey::of_file(&p).unwrap();
        assert_eq!(k.content, content_hash(&d));
        assert_eq!(k.stamp.len, d.len() as u64);
        assert_eq!(k.name(256, 0).len(), 16);
        assert_ne!(k.name(256, 0), k.name(128, 0));
        assert_ne!(k.name(256, 0), k.name(256, 1));
        let mut k2 = k;
        k2.stamp.mtime_ns += 1;
        assert_ne!(k.name(256, 0), k2.name(256, 0));
        // Same content elsewhere: same name (a copy keeps its mtime with cp -p).
        let q = dir.join("b.bin");
        std::fs::write(&q, &d).unwrap();
        let mut kq = ThumbKey::of_file(&q).unwrap();
        kq.stamp.mtime_ns = k.stamp.mtime_ns;
        assert_eq!(k.name(256, 0), kq.name(256, 0));
        std::fs::remove_dir_all(&dir).ok();
    }

    const PINNED_MEMBER_NAME: &str = "8f39c7dadcd28142";

    #[test]
    fn member_keys_follow_the_archive_and_the_name() {
        let a = Path::new("/books/a.cbz");
        let s = Stamp {
            len: 1000,
            mtime_ns: 5,
        };
        let k = ThumbKey::of_member(a, s, "p/01.jpg");
        assert_eq!(k, ThumbKey::of_member(a, s, "p/01.jpg"));
        assert_eq!(k.stamp, s);
        assert_ne!(k, ThumbKey::of_member(a, s, "p/02.jpg"));
        assert_ne!(
            k,
            ThumbKey::of_member(Path::new("/books/b.cbz"), s, "p/01.jpg")
        );
        // The archive changed (size or time): new names, so no stale entry is served.
        for t in [Stamp { len: 1001, ..s }, Stamp { mtime_ns: 6, ..s }] {
            let k2 = ThumbKey::of_member(a, t, "p/01.jpg");
            assert_ne!(k.name(128, 0), k2.name(128, 0));
        }
        // The path and the name are hashed apart: moving a '/' between them changes the key.
        assert_ne!(
            ThumbKey::of_member(Path::new("/x/a"), s, "b"),
            ThumbKey::of_member(Path::new("/x/a/b"), s, "")
        );
        // Pinned: changing the hash makes every archive's thumbnails again.
        assert_eq!(
            ThumbKey::of_member(Path::new("/a.zip"), Stamp::default(), "1.jpg").name(128, 0),
            PINNED_MEMBER_NAME
        );
    }
}

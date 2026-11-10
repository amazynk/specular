//! A bounded on-disk cache of downloaded files.
//!
//! Entries are files under one root (the app's cache dir, `remote/`), named by the caller with
//! relative keys such as `dicomweb/<server>/<study>/<series>/<sop>.dcm` (each component is
//! sanitised) or by URL hash ([`DiskCache::url_key`]). Files are written next to their final
//! name and renamed, so an entry is whole or absent. When the total exceeds the budget the
//! least recently used entries go, except those under a pinned prefix (an open series).
//!
//! Opening scans the folder once to rebuild the index (last use from modification times), so
//! open it on a worker thread.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use specular_core::CancelToken;

use crate::http::{self, Client, Progress, Request};

#[derive(Debug)]
struct Entry {
    size: u64,
    tick: u64,
}

#[derive(Debug, Default)]
struct Index {
    entries: HashMap<String, Entry>,
    order: BTreeMap<u64, String>,
    total: u64,
    tick: u64,
    pins: HashMap<String, u32>,
}

impl Index {
    fn touch(&mut self, rel: &str) -> bool {
        let Some(e) = self.entries.get_mut(rel) else {
            return false;
        };
        self.order.remove(&e.tick);
        self.tick += 1;
        e.tick = self.tick;
        self.order.insert(self.tick, rel.to_string());
        true
    }
    fn add(&mut self, rel: &str, size: u64) {
        self.drop_entry(rel);
        self.tick += 1;
        self.entries.insert(
            rel.to_string(),
            Entry {
                size,
                tick: self.tick,
            },
        );
        self.order.insert(self.tick, rel.to_string());
        self.total += size;
    }
    fn drop_entry(&mut self, rel: &str) -> bool {
        match self.entries.remove(rel) {
            Some(e) => {
                self.order.remove(&e.tick);
                self.total -= e.size;
                true
            }
            None => false,
        }
    }
    fn pinned(&self, rel: &str) -> bool {
        self.pins.keys().any(|p| rel.starts_with(p.as_str()))
    }
}

/// Counters for logs and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub entries: usize,
    pub bytes: u64,
    pub hits: u64,
    pub misses: u64,
}

/// The on-disk cache. Share it as `Arc<DiskCache>`.
#[derive(Debug)]
pub struct DiskCache {
    root: PathBuf,
    budget: AtomicU64,
    index: Mutex<Index>,
    hits: AtomicU64,
    misses: AtomicU64,
}

/// Keeps entries under a prefix from being evicted while alive.
#[derive(Debug)]
pub struct CachePin {
    cache: Arc<DiskCache>,
    prefix: String,
}

impl Drop for CachePin {
    fn drop(&mut self) {
        let mut ix = self.cache.index.lock();
        if let Some(n) = ix.pins.get_mut(&self.prefix) {
            *n -= 1;
            if *n == 0 {
                ix.pins.remove(&self.prefix);
            }
        }
        drop(ix);
        self.cache.evict();
    }
}

fn is_side_file(name: &str) -> bool {
    name.ends_with(".part") || name.ends_with(".meta") || name.ends_with(".tmp")
}

impl DiskCache {
    /// Opens (creating) the cache at `root`, holding at most `budget` bytes.
    pub fn open(root: impl Into<PathBuf>, budget: u64) -> io::Result<Arc<DiskCache>> {
        let root = root.into();
        std::fs::create_dir_all(&root)?;
        let mut found = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let Ok(ft) = e.file_type() else { continue };
                let p = e.path();
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() {
                    let name = e.file_name();
                    let name = name.to_string_lossy();
                    if is_side_file(&name) {
                        continue;
                    }
                    let Ok(m) = e.metadata() else { continue };
                    let Ok(rel) = p.strip_prefix(&root) else {
                        continue;
                    };
                    let rel = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    found.push((m.modified().ok(), rel, m.len()));
                }
            }
        }
        found.sort();
        let mut ix = Index::default();
        for (_, rel, size) in found {
            ix.add(&rel, size);
        }
        let c = Arc::new(DiskCache {
            root,
            budget: AtomicU64::new(budget),
            index: Mutex::new(ix),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        });
        c.evict();
        Ok(c)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn set_budget(&self, bytes: u64) {
        self.budget.store(bytes, Ordering::Relaxed);
        self.evict();
    }

    /// A relative key from components, each made safe as a file name.
    pub fn key(parts: &[&str]) -> String {
        parts
            .iter()
            .map(|p| sanitize(p))
            .collect::<Vec<_>>()
            .join("/")
    }

    /// The key for a whole URL (`url/<hash>`); credentials never take part.
    pub fn url_key(url: &str) -> String {
        let h = blake3::hash(url.as_bytes()).to_hex();
        format!("url/{}/{}", &h[..2], &h[2..32])
    }

    /// Where the entry `rel` lives (whether or not it exists).
    pub fn path(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for c in rel.split('/') {
            p.push(sanitize(c));
        }
        p
    }

    /// The file for `rel` if cached (marks it used).
    pub fn get(&self, rel: &str) -> Option<PathBuf> {
        let p = self.path(rel);
        let known = self.index.lock().touch(rel);
        if known && p.is_file() {
            self.hits.fetch_add(1, Ordering::Relaxed);
            // Survives restarts: the index is rebuilt from modification times.
            if let Ok(f) = std::fs::File::options().append(true).open(&p) {
                let _ = f.set_modified(std::time::SystemTime::now());
            }
            return Some(p);
        }
        if known {
            self.index.lock().drop_entry(rel);
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Whether `rel` is cached, without counting a hit or touching it.
    pub fn contains(&self, rel: &str) -> bool {
        self.index.lock().entries.contains_key(rel)
    }

    /// Stores `bytes` as `rel` (atomically) and returns its path.
    pub fn insert_bytes(&self, rel: &str, bytes: &[u8]) -> io::Result<PathBuf> {
        let p = self.path(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut tmp = p.as_os_str().to_owned();
        tmp.push(format!(".{}.tmp", fastrand::u32(..)));
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, bytes)?;
        if let Err(e) = std::fs::rename(&tmp, &p) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        self.index.lock().add(rel, bytes.len() as u64);
        self.evict();
        Ok(p)
    }

    /// Records a file someone wrote at [`path`](Self::path)`(rel)`.
    pub fn record(&self, rel: &str) -> io::Result<PathBuf> {
        let p = self.path(rel);
        let len = std::fs::metadata(&p)?.len();
        self.index.lock().add(rel, len);
        self.evict();
        Ok(p)
    }

    /// The cached file for `rel`, downloading `req` into it first if needed.
    pub fn fetch(
        &self,
        client: &Client,
        req: &Request,
        rel: &str,
        cancel: &CancelToken,
        on_progress: impl FnMut(&Progress),
    ) -> Result<PathBuf, http::Error> {
        if let Some(p) = self.get(rel) {
            return Ok(p);
        }
        let p = self.path(rel);
        client.download_to(req, cancel, &p, on_progress)?;
        Ok(self.record(rel)?)
    }

    /// Small text stored beside an entry (ETag, content type); not counted, removed with it.
    pub fn meta(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(side(&self.path(rel), ".meta")).ok()
    }
    pub fn set_meta(&self, rel: &str, text: &str) -> io::Result<()> {
        let p = side(&self.path(rel), ".meta");
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(p, text)
    }

    pub fn remove(&self, rel: &str) {
        self.index.lock().drop_entry(rel);
        let p = self.path(rel);
        let _ = std::fs::remove_file(side(&p, ".meta"));
        let _ = std::fs::remove_file(&p);
    }

    /// Removes every entry under `prefix` (a server, when the user removes it) and the folder.
    pub fn remove_prefix(&self, prefix: &str) {
        let prefix = prefix.trim_end_matches('/');
        let gone: Vec<String> = {
            let ix = self.index.lock();
            ix.entries
                .keys()
                .filter(|k| k.starts_with(prefix) && k[prefix.len()..].starts_with('/'))
                .cloned()
                .collect()
        };
        for k in gone {
            self.index.lock().drop_entry(&k);
        }
        if !prefix.is_empty() {
            let _ = std::fs::remove_dir_all(self.path(prefix));
        }
    }

    /// Keeps entries under `prefix` while the returned guard lives.
    pub fn pin(self: &Arc<Self>, prefix: &str) -> CachePin {
        *self
            .index
            .lock()
            .pins
            .entry(prefix.to_string())
            .or_insert(0) += 1;
        CachePin {
            cache: self.clone(),
            prefix: prefix.to_string(),
        }
    }

    pub fn stats(&self) -> CacheStats {
        let ix = self.index.lock();
        CacheStats {
            entries: ix.entries.len(),
            bytes: ix.total,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    fn evict(&self) {
        let budget = self.budget.load(Ordering::Relaxed);
        let mut gone = Vec::new();
        {
            let mut ix = self.index.lock();
            if ix.total <= budget {
                return;
            }
            let candidates: Vec<String> = ix.order.values().cloned().collect();
            for rel in candidates {
                if ix.total <= budget {
                    break;
                }
                if ix.pinned(&rel) {
                    continue;
                }
                ix.drop_entry(&rel);
                gone.push(rel);
            }
        }
        for rel in gone {
            let p = self.path(&rel);
            let _ = std::fs::remove_file(side(&p, ".meta"));
            let _ = std::fs::remove_file(&p);
        }
    }
}

fn side(p: &Path, ext: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(ext);
    PathBuf::from(s)
}

/// A safe file-name component: ASCII letters, digits, `.`, `-`, `_`; others become `_`;
/// never empty, `.` or `..`; at most 120 bytes (longer names keep a hash).
pub fn sanitize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.chars().all(|c| c == '.') {
        out = format!("_{}", out.len());
    }
    if is_side_file(&out) {
        out.push('_');
    }
    if out.len() > 120 {
        let h = blake3::hash(s.as_bytes()).to_hex();
        out.truncate(100);
        out.push('-');
        out.push_str(&h[..16]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_uids_and_blocks_traversal() {
        assert_eq!(sanitize("1.2.840.113619.2"), "1.2.840.113619.2");
        assert_eq!(sanitize(".."), "_2");
        assert_eq!(sanitize(""), "_0");
        assert_eq!(sanitize("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize("x.part"), "x.part_");
        assert!(sanitize(&"9".repeat(300)).len() <= 120);
        assert_eq!(DiskCache::key(&["srv", "../etc", "x"]), "srv/.._etc/x");
    }
}

//! The signature pass: each image's pixel signature, for duplicates and similar shots.
//!
//! It runs after the facts pass, a batch at a time on the same low-priority threads, and
//! backs off the same way while the user browses. A signature costs nothing when the
//! thumbnail cache already has one (the gallery's engine stores it with every 256 px
//! thumbnail it makes, [`specular_similar::scan::digester`]), a cache read and about a
//! millisecond of arithmetic when the cache has the thumbnail without it, and otherwise one
//! thumbnail decode, whose result goes into the cache for the gallery to show later.
//!
//! The work is on the CPU: the decode is the cost, and the decoders run there. The signature
//! of a 256 px thumbnail is a few small resamples and a 32×32 DCT (well under a millisecond),
//! and matching is popcounts over a few candidates; a GPU round trip per image would cost
//! more than either.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rusqlite::{Connection, params};
use specular_core::CancelToken;
use specular_similar::scan::SIG_DIGEST_ID;
use specular_similar::{Luma, SIG_CLASS, Signature};
use specular_thumbs::generate::{self, GenOptions};
use specular_thumbs::{Digest, DiskCache, Stamp, ThumbKey};

use crate::{Config, Shared};

/// Makes the signatures (and the thumbnails the duplicate check compares).
pub trait Signer: Send + Sync {
    /// The signature of the image at `path` (`key`: its thumbnail key), None when its pixels
    /// cannot be read.
    fn signature(
        &self,
        path: &Path,
        key: &ThumbKey,
        cache: Option<&DiskCache>,
        flags: u32,
    ) -> Option<Signature>;

    /// Its 256 px thumbnail's luma, for the SSIM check of a duplicate candidate.
    fn luma(
        &self,
        path: &Path,
        key: &ThumbKey,
        cache: Option<&DiskCache>,
        flags: u32,
    ) -> Option<Luma>;

    /// BLAKE3 of the whole file (byte-identical candidates only).
    fn file_hash(&self, path: &Path) -> Option<[u8; 32]> {
        specular_similar::file_hash(path).ok()
    }
}

/// Signatures from the thumbnail cache, else from a thumbnail made with the decoders.
pub struct DecodeSigner {
    pub registry: Arc<specular_decode::Registry>,
}

impl DecodeSigner {
    /// The 256 px thumbnail, made now and stored in the cache with its signature.
    fn make(
        &self,
        path: &Path,
        key: &ThumbKey,
        cache: Option<&DiskCache>,
        flags: u32,
    ) -> Option<(specular_thumbs::pixels::Pixels, Signature)> {
        let s = self.registry.open_path(path).ok()?;
        let meta = generate::meta_of(s.as_ref());
        let px = generate::generate(
            s.as_ref(),
            SIG_CLASS,
            GenOptions::default(),
            &CancelToken::new(),
            &mut |_| {},
        )
        .ok()?;
        px.prepare();
        let sig = Signature::of_rgba(px.width, px.height, &px.rgba);
        if let Some(c) = cache {
            let d = Digest {
                id: SIG_DIGEST_ID,
                bytes: sig.to_bytes().to_vec(),
            };
            if let Err(e) = c.put_with_digest(key, SIG_CLASS, flags, &px, &meta, Some(&d)) {
                specular_log::debug!(target: specular_log::CACHE, "thumbnail cache write failed: {e}");
            }
        }
        Some((px, sig))
    }
}

impl Signer for DecodeSigner {
    fn signature(
        &self,
        path: &Path,
        key: &ThumbKey,
        cache: Option<&DiskCache>,
        flags: u32,
    ) -> Option<Signature> {
        if let Some(c) = cache {
            if let Some((_, d)) = c.get_digest(key, SIG_CLASS, flags, SIG_DIGEST_ID)
                && let Some(sig) = signature_of(&d.bytes)
            {
                return Some(sig);
            }
            if let Some(e) = c.get(key, SIG_CLASS, flags)
                && e.class == SIG_CLASS
            {
                // Hashed after the cache's lossy coding: close to the engine's, not equal
                // (the gallery's scan does the same for entries written before signatures).
                return Some(Signature::of_rgba(
                    e.pixels.width,
                    e.pixels.height,
                    &e.pixels.rgba,
                ));
            }
        }
        self.make(path, key, cache, flags).map(|x| x.1)
    }

    fn luma(
        &self,
        path: &Path,
        key: &ThumbKey,
        cache: Option<&DiskCache>,
        flags: u32,
    ) -> Option<Luma> {
        if let Some(e) = cache.and_then(|c| c.get(key, SIG_CLASS, flags))
            && e.class == SIG_CLASS
        {
            return Some(Luma::from_rgba(
                e.pixels.width,
                e.pixels.height,
                &e.pixels.rgba,
            ));
        }
        let (px, _) = self.make(path, key, cache, flags)?;
        Some(Luma::from_rgba(px.width, px.height, &px.rgba))
    }
}

pub(crate) fn signature_of(b: &[u8]) -> Option<Signature> {
    (b.len() == specular_similar::sig::SIG_BYTES)
        .then(|| Signature::from_bytes(b))
        .flatten()
}

/// A file waiting for its signature.
struct Todo {
    id: i64,
    path: PathBuf,
    key: ThumbKey,
}

/// Files in online locations whose facts are read and whose signature is missing or was
/// made from another version of the file or of the signature.
const PENDING: &str = "FROM files f JOIN dirs d ON d.id = f.dir_id
     JOIN folders o ON o.id = d.folder_id JOIN thumbs th ON th.file_id = f.id
     LEFT JOIN image_hashes h ON h.file_id = f.id
     WHERE f.state = 1 AND o.offline = 0
       AND (h.file_id IS NULL OR h.size <> f.size OR h.mtime_ns <> f.mtime_ns OR h.version <> ?1)";

/// The key a file's thumbnail is cached under, from what the catalogue knows.
pub(crate) fn key_of(size: i64, mtime_ns: i64, content: i64) -> ThumbKey {
    ThumbKey {
        stamp: Stamp {
            len: size as u64,
            mtime_ns: i128::from(mtime_ns),
        },
        content: content as u64,
    }
}

/// The next files waiting for their signature, in file order after `after`.
fn todo(c: &Connection, cfg: &Config, after: i64) -> rusqlite::Result<Vec<Todo>> {
    let mut st = c.prepare_cached(&format!(
        "SELECT f.id, d.path, f.name, f.size, f.mtime_ns, th.content {PENDING}
           AND f.id > ?3 ORDER BY f.id LIMIT ?2"
    ))?;
    let todo = st
        .query_map(
            params![i64::from(SIG_DIGEST_ID), cfg.batch as i64, after],
            |r| {
                Ok(Todo {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?).join(r.get::<_, String>(2)?),
                    key: key_of(r.get(3)?, r.get(4)?, r.get(5)?),
                })
            },
        )?
        .collect();
    crate::work_counts::todo_steps(&st);
    todo
}

/// Signs and writes one batch. Returns how many files were done (None: nothing waits; 0: the
/// user is browsing).
///
/// `from` is where the pass is (a file id): each batch goes on after it rather than looking
/// past every file already signed again (a million files took one scan of the catalogue per
/// batch). Files that became pending behind it are found when it starts over from the
/// beginning, once nothing is left ahead.
pub fn batch(
    c: &Connection,
    cfg: &Config,
    signer: &dyn Signer,
    shared: &Shared,
    seq: &mut i64,
    from: &mut i64,
) -> rusqlite::Result<Option<usize>> {
    let mut todo = todo(c, cfg, *from)?;
    if todo.is_empty() && *from > 0 {
        *from = 0;
        todo = self::todo(c, cfg, 0)?;
    }
    if todo.is_empty() {
        return Ok(None);
    }
    let next = AtomicUsize::new(0);
    let threads = cfg.threads.clamp(1, todo.len());
    let cache = cfg.cache.as_deref();
    let mut results: Vec<(usize, Option<Signature>)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|wi| {
                specular_thread::spawn_scoped(s, format!("lib-hash-{wi}"), || {
                    if let Some(f) = cfg.on_thread_start {
                        f();
                    }
                    let mut out = Vec::new();
                    loop {
                        if shared.held() || shared.halting() || shared.removal_waits() {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(t) = todo.get(i) else { break };
                        // A panic on this file (a decoder bug) leaves it without a
                        // signature, written as done: it is not tried again on every pass.
                        let what = format_args!("signature of {}", t.path.display());
                        let sig = specular_core::isolate::run(what, || {
                            signer.signature(&t.path, &t.key, cache, cfg.cache_flags)
                        });
                        out.push((i, sig.unwrap_or(None)));
                        shared.beat();
                    }
                    out
                })
            })
            .collect();
        hs.into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    results.sort_by_key(|r| r.0);
    if results.is_empty() {
        return Ok(Some(0));
    }
    c.execute_batch("BEGIN IMMEDIATE")?;
    let r = write(c, &todo, &results, seq);
    // Files of the batch left for later (the user browsed) are behind it now: done when the
    // pass starts over.
    if let Some(&(i, _)) = results.last() {
        *from = (*from).max(todo[i].id);
    }
    if r.is_err() {
        let _ = c.execute_batch("ROLLBACK");
        return r.map(|_| None);
    }
    c.execute_batch("COMMIT")?;
    Ok(Some(results.len()))
}

fn write(
    c: &Connection,
    todo: &[Todo],
    results: &[(usize, Option<Signature>)],
    seq: &mut i64,
) -> rusqlite::Result<()> {
    for (i, sig) in results {
        let t = &todo[*i];
        *seq += 1;
        c.prepare_cached(
            "INSERT INTO image_hashes (file_id, size, mtime_ns, version, sig, blake3, seq)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)
             ON CONFLICT(file_id) DO UPDATE SET size = excluded.size,
                mtime_ns = excluded.mtime_ns, version = excluded.version, sig = excluded.sig,
                blake3 = NULL, seq = excluded.seq",
        )?
        .execute(params![
            t.id,
            t.key.stamp.len as i64,
            t.key
                .stamp
                .mtime_ns
                .clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            i64::from(SIG_DIGEST_ID),
            sig.map(|s| s.to_bytes().to_vec()),
            *seq,
        ])?;
        // What checking its candidates found was about its old pixels.
        c.prepare_cached("DELETE FROM match_verify WHERE a = ?1 OR b = ?1")?
            .execute([t.id])?;
    }
    Ok(())
}
